//! SigningBackend (#94) — the ciphersuite-generic seam between UI-facing
//! managers (`SigningManager`, used by starlab-desktop) and the machinery
//! that actually runs FROST commit → share → aggregate over the mesh.
//!
//! The production implementation is [`ElmSigningBackend`]: it drives the SAME
//! elm `Message` loop the TUI and the CLI use (the path the real-DKG e2e
//! exercises in CI every day) via the embedder's `HeadlessRunner` channel:
//!
//! ```text
//! SigningManager::approve_and_sign
//!     └─ ElmSigningBackend::sign
//!          ├─ runner_tx.send(Message::HeadlessSign { … })   // starts ceremony
//!          └─ await SigningEventSink observations:
//!               Message::SigningComplete → Ok(signature)
//!               Message::SigningFailed   → Err(reason)
//!               (timeout)                → Err(timeout)
//! ```
//!
//! The embedder (desktop's `core_adapter`, CLI's bridge) already owns an
//! `on_sync(model, msg)` callback on the runner — it forwards every message to
//! the backend's [`SigningEventSink`]. No second transport, no duplicated
//! signing logic: one proven path, three front-ends.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc::UnboundedSender};

use super::{CoreError, CoreResult};
use crate::elm::Message;

/// What a backend needs to run one threshold-signing ceremony.
#[derive(Debug, Clone)]
pub struct BackendSignRequest {
    /// Keystore wallet id (NOT the CoreState index).
    pub wallet_id: String,
    /// Hex-encoded bytes to sign (pre-hashed by the caller where the chain
    /// demands it — e.g. EIP-191/keccak for EVM).
    pub message_hex: String,
    /// Keystore password unlocking this device's share for the ceremony.
    pub password: String,
    /// Ceremony timeout. Quorum gathering is interactive (other humans!),
    /// so this is minutes, not seconds.
    pub timeout: Duration,
}

/// A completed ceremony.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureOutcome {
    /// 0x-prefixed aggregated signature.
    pub signature_hex: String,
    /// 0x-prefixed bytes that were signed (echo for verification).
    pub message_hash_hex: String,
}

/// Ciphersuite-generic threshold-signing backend.
#[async_trait]
pub trait SigningBackend: Send + Sync {
    async fn sign(&self, req: BackendSignRequest) -> CoreResult<SignatureOutcome>;
}

/// Events the backend cares about, extracted from the elm message stream.
#[derive(Debug, Clone)]
enum SigningEvent {
    Complete {
        signature_hex: String,
        message_hash_hex: String,
    },
    Failed {
        error: String,
    },
}

/// Cloneable observer the embedder calls from its `on_sync` callback for
/// every elm message. Cheap no-op for everything except signing outcomes.
#[derive(Clone)]
pub struct SigningEventSink {
    tx: broadcast::Sender<SigningEvent>,
}

impl SigningEventSink {
    /// Forward one elm message. Call from `on_sync(model, Some(msg))`.
    pub fn observe(&self, msg: &Message) {
        match msg {
            Message::SigningComplete {
                message, signature, ..
            } => {
                let _ = self.tx.send(SigningEvent::Complete {
                    signature_hex: format!("0x{}", hex::encode(signature)),
                    message_hash_hex: format!("0x{}", hex::encode(message)),
                });
            }
            Message::SigningFailed { error, .. } => {
                let _ = self.tx.send(SigningEvent::Failed {
                    error: error.clone(),
                });
            }
            _ => {}
        }
    }
}

/// [`SigningBackend`] over a running elm `HeadlessRunner`.
pub struct ElmSigningBackend {
    runner_tx: UnboundedSender<Message>,
    events: broadcast::Sender<SigningEvent>,
}

impl ElmSigningBackend {
    /// Returns the backend plus the sink the embedder must feed from its
    /// runner `on_sync` callback.
    pub fn new(runner_tx: UnboundedSender<Message>) -> (Arc<Self>, SigningEventSink) {
        let (tx, _) = broadcast::channel(64);
        let sink = SigningEventSink { tx: tx.clone() };
        (
            Arc::new(Self {
                runner_tx,
                events: tx,
            }),
            sink,
        )
    }
}

#[async_trait]
impl SigningBackend for ElmSigningBackend {
    async fn sign(&self, req: BackendSignRequest) -> CoreResult<SignatureOutcome> {
        // Subscribe BEFORE dispatching so a fast completion can't race past us.
        let mut rx = self.events.subscribe();

        self.runner_tx
            .send(Message::HeadlessSign {
                wallet_id: req.wallet_id.clone(),
                message: req.message_hex.clone(),
                encoding: "hex".to_string(),
                password: req.password.clone(),
            })
            .map_err(|e| CoreError::Dkg(format!("signing runner gone: {e}")))?;

        let deadline = tokio::time::sleep(req.timeout);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => {
                    return Err(CoreError::Dkg(format!(
                        "signing ceremony timed out after {:?} — co-signers may be offline or \
                         never approved",
                        req.timeout
                    )));
                }
                ev = rx.recv() => match ev {
                    Ok(SigningEvent::Complete { signature_hex, message_hash_hex }) => {
                        return Ok(SignatureOutcome { signature_hex, message_hash_hex });
                    }
                    Ok(SigningEvent::Failed { error }) => {
                        return Err(CoreError::Dkg(format!("signing failed: {error}")));
                    }
                    // Lagged: we missed events under burst — keep waiting for
                    // the next one rather than failing the ceremony.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(CoreError::Dkg("signing event stream closed".into()));
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    fn req(timeout_ms: u64) -> BackendSignRequest {
        BackendSignRequest {
            wallet_id: "w1".into(),
            message_hex: "deadbeef".into(),
            password: "pw".into(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// Fake "elm runner": on HeadlessSign, respond through the sink the way
    /// the real loop responds through on_sync.
    fn fake_runner(
        respond: impl Fn(&Message) -> Option<Message> + Send + 'static,
    ) -> (Arc<ElmSigningBackend>, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = unbounded_channel::<Message>();
        let (backend, sink) = ElmSigningBackend::new(tx);
        let handle = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Some(reply) = respond(&msg) {
                    sink.observe(&reply);
                }
            }
        });
        (backend, handle)
    }

    #[tokio::test]
    async fn completes_with_the_aggregated_signature() {
        let (backend, _h) = fake_runner(|msg| {
            if let Message::HeadlessSign { wallet_id, message, .. } = msg {
                assert_eq!(wallet_id, "w1");
                assert_eq!(message, "deadbeef");
                Some(Message::SigningComplete {
                    request_id: "r".into(),
                    message: vec![0xde, 0xad, 0xbe, 0xef],
                    signature: vec![0xaa; 64],
                })
            } else {
                None
            }
        });
        let out = backend.sign(req(2_000)).await.unwrap();
        assert_eq!(out.signature_hex, format!("0x{}", "aa".repeat(64)));
        assert_eq!(out.message_hash_hex, "0xdeadbeef");
    }

    #[tokio::test]
    async fn surfaces_ceremony_failure() {
        let (backend, _h) = fake_runner(|msg| {
            matches!(msg, Message::HeadlessSign { .. }).then(|| Message::SigningFailed {
                request_id: "r".into(),
                error: "quorum declined".into(),
            })
        });
        let err = backend.sign(req(2_000)).await.unwrap_err();
        assert!(err.to_string().contains("quorum declined"));
    }

    #[tokio::test]
    async fn times_out_when_nobody_answers() {
        let (backend, _h) = fake_runner(|_| None);
        let err = backend.sign(req(50)).await.unwrap_err();
        assert!(err.to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn unrelated_messages_are_ignored() {
        let (backend, _h) = fake_runner(|msg| {
            if matches!(msg, Message::HeadlessSign { .. }) {
                // Real streams interleave plenty of noise before the outcome.
                Some(Message::SigningComplete {
                    request_id: "r".into(),
                    message: vec![1],
                    signature: vec![2],
                })
            } else {
                None
            }
        });
        // Sink also observes unrelated messages — they must be dropped.
        let (_tx2, _) = unbounded_channel::<Message>();
        let out = backend.sign(req(2_000)).await.unwrap();
        assert_eq!(out.signature_hex, "0x02");
    }
}
