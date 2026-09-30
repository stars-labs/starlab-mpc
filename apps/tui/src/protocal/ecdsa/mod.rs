//! Threshold-ECDSA engine (cggmp24, via `starlab_core::ecdsa`): the
//! Ethereum key of a wallet.
//!
//! - **DKG** ([`dkg`]): after the FROST ceremonies of a secp256k1 or unified
//!   DKG, the same participants run ECDSA aux info then keygen; keygen index
//!   = position in the sorted participant list (FROST's canonical order).
//!   `DKGFinalized` fires only once the ECDSA share is persisted too.
//! - **Signing** ([`signing`]): an Ethereum account child
//!   (`{root}-ethereum-{n}`) signs through the ECDSA protocol with exactly
//!   `threshold` signers, fixed by the proposer (same rule as FROST's
//!   `SIGN_SET`).
//! - **Transport** ([`wire`]): frames ride the existing WebRTC data channels
//!   as `SimpleMessage` texts; see `wire` for the exact format.
//! - **Threads** ([`worker`]): each ceremony runs on its own OS thread, never
//!   on the tokio runtime; primes come from a background generator
//!   ([`primes`]).

pub mod dkg;
pub mod primes;
pub mod signing;
pub mod wire;
pub mod worker;

pub use primes::PrimeSupply;

use crate::elm::message::Message;
use crate::protocal::signal::WebRTCMessage;
use crate::utils::appstate_compat::AppState;
use frost_core::Ciphersuite;
use starlab_core::ecdsa::EcdsaKeyShare;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{info, warn};

/// Payloads kept for a ceremony that hasn't started locally yet.
const MAX_PENDING: usize = 1024;

/// ECDSA part of a node's `AppState` (not generic over the FROST suite).
#[derive(Default)]
pub struct EcdsaState {
    /// Where aux-info primes come from.
    pub primes: PrimeSupply,
    /// Inbox of the running ceremony worker, if any.
    worker: Option<worker::Inbox>,
    /// Reassembled payloads that arrived while no worker ran (a faster peer
    /// already started the next ceremony); replayed into the next worker,
    /// which drops the ones for other executions.
    pending: Vec<(String, Vec<u8>)>,
    reassembler: wire::Reassembler,
    /// The unlocked ECDSA share, keyed by ROOT wallet id.
    pub key_share: Option<(String, EcdsaKeyShare)>,
    /// The running signing, if any.
    pub(crate) signing: Option<signing::SigningCtx>,
    /// Control frames for a signing this node hasn't started (yet).
    pub(crate) pending_control: Vec<(String, &'static str, wire::Control)>,
    pub(crate) signing_epoch: u64,
}

impl EcdsaState {
    /// Route payloads to `inbox` from now on, starting with the buffered ones.
    pub(crate) fn install_worker(&mut self, inbox: worker::Inbox) {
        for payload in self.pending.drain(..) {
            let _ = inbox.send(payload);
        }
        self.worker = Some(inbox);
    }

    /// Stop routing to the worker (drops our inbox handle: a still-running
    /// ceremony is cancelled once the caller's clone is gone too).
    pub(crate) fn remove_worker(&mut self) {
        self.worker = None;
    }

    fn deliver(&mut self, from: String, payload: Vec<u8>) {
        if let Some(inbox) = &self.worker {
            match inbox.send((from, payload)) {
                Ok(()) => {}
                Err(std::sync::mpsc::SendError(back)) => {
                    self.worker = None;
                    self.buffer(back);
                }
            }
        } else {
            self.buffer((from, payload));
        }
    }

    fn buffer(&mut self, item: (String, Vec<u8>)) {
        if self.pending.len() >= MAX_PENDING {
            self.pending.remove(0);
        }
        self.pending.push(item);
    }

    /// A new DKG ceremony: forget everything of the last one.
    pub(crate) fn reset_for_new_dkg(&mut self) {
        self.worker = None;
        self.pending.clear();
        self.reassembler = wire::Reassembler::default();
    }
}

/// Whether a `SimpleMessage` text is an ECDSA engine frame.
pub fn is_ecdsa_frame(text: &str) -> bool {
    wire::PREFIXES.iter().any(|p| text.starts_with(p))
}

/// Handle one ECDSA frame from `from` (already de-duplicated by the caller).
pub async fn on_frame<C>(
    state: &Arc<Mutex<AppState<C>>>,
    from: &str,
    text: &str,
    ui_tx: Option<&UnboundedSender<Message>>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    if let Some(rest) = text.strip_prefix(wire::FRAME_PREFIX) {
        let chunk = match wire::parse_chunk(rest) {
            Ok(c) => c,
            Err(e) => {
                warn!("ECDSA frame from {from}: {e}");
                return;
            }
        };
        let mut guard = state.lock().await;
        match guard.ecdsa.reassembler.push(from, chunk) {
            Ok(Some(payload)) => guard.ecdsa.deliver(from.to_string(), payload),
            Ok(None) => {}
            Err(e) => warn!("{e}"),
        }
        return;
    }
    for prefix in [
        wire::SIGN_READY_PREFIX,
        wire::SIGN_SET_PREFIX,
        wire::SIGN_DONE_PREFIX,
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            match wire::Control::decode(rest) {
                Ok(control) => signing::on_control(state, from, prefix, control, ui_tx).await,
                Err(e) => warn!("{prefix} from {from}: {e}"),
            }
            return;
        }
    }
}

/// Send each worker output to its recipients, chunked.
pub(crate) fn spawn_transport<C>(
    state: Arc<Mutex<AppState<C>>>,
    mut rx: UnboundedReceiver<worker::Outgoing>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    tokio::spawn(async move {
        while let Some(out) = rx.recv().await {
            let frames = wire::encode_frames(&out.payload);
            for peer in &out.to {
                send_texts(&state, peer, &frames).await;
            }
        }
    });
}

/// Forward worker status notes to the UI as info toasts.
pub(crate) fn spawn_status(mut rx: UnboundedReceiver<String>, ui_tx: UnboundedSender<Message>) {
    tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            info!("ECDSA: {text}");
            let _ = ui_tx.send(Message::ShowNotification {
                text,
                kind: crate::elm::model::NotificationKind::Info,
            });
        }
    });
}

/// Send frames to one peer, in order. A channel that isn't open yet gets a
/// few retries (shared by the whole batch); after that every remaining
/// frame is still handed to `send_webrtc_message` once, which records it
/// for the reconnect resend (`network::peer_recovery`).
pub(crate) async fn send_texts<C>(state: &Arc<Mutex<AppState<C>>>, peer: &str, texts: &[String])
where
    C: Ciphersuite + Send + Sync + 'static,
{
    const RETRIES: u32 = 20;
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(500);
    let mut retries_left = RETRIES;
    for text in texts {
        let message = WebRTCMessage::<C>::SimpleMessage { text: text.clone() };
        loop {
            match crate::utils::device::send_webrtc_message(peer, &message, state.clone()).await {
                Ok(()) => break,
                Err(e)
                    if retries_left > 0
                        && (e.contains("Data channel not found")
                            || e.contains("Data channel for")
                            || e.contains("is not open")) =>
                {
                    retries_left -= 1;
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Err(e) => {
                    warn!("ECDSA: send to {peer} failed: {e}");
                    break;
                }
            }
        }
    }
}
