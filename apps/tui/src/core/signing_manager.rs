//! SigningManager — thin facade over the FROST threshold-signing
//! flow for non-Elm GUI front-ends (e.g. the desktop app in
//! stars-labs/starlab-desktop). Mirrors the shape of
//! DkgManager / SessionManager.
//!
//! Status: STRUCTURAL STUB. The methods below update CoreState and
//! fire UICallback events so downstream UIs can render approval /
//! progress / completion states correctly. The actual FROST
//! commit / share / aggregate logic lives in
//! `apps/tui/src/protocal/signing.rs` and is driven by the elm
//! `Message` loop on `AppState<C>`; a future commit will route
//! approved requests from here into that machinery (or extract a
//! ciphersuite-generic backend both can share).
//!
//! Until then, `request_signing()` creates the UI-facing request
//! (so the confirm modal works end-to-end), `approve()` just
//! flips state to `Complete` with a placeholder signature, and
//! `reject()` flips to `Idle`. Callers get clear error-surface
//! behaviour and the UI wiring for Phase 4-complete is already
//! in place.

use std::sync::Arc;
use std::time::Duration;

use super::signing_backend::{BackendSignRequest, SigningBackend};
use super::{CoreError, CoreResult, CoreState, SigningRequest, SigningState, UICallback};
use crate::elm::{Message, Model};
use crate::protocal::signal::SessionType;
use starlab_core::accounts::parse_child_wallet_id;

pub struct SigningManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
    /// Real FROST backend (#94). When set, `approve_and_sign` runs the actual
    /// ceremony; when absent, only the legacy `approve()` stub is available.
    backend: Option<Arc<dyn SigningBackend>>,
}

impl SigningManager {
    pub fn new(state: Arc<CoreState>, ui_callback: Arc<dyn UICallback>) -> Self {
        Self {
            state,
            ui_callback,
            backend: None,
        }
    }

    /// Attach the real signing backend (e.g. `ElmSigningBackend` over the
    /// embedder's HeadlessRunner).
    pub fn with_backend(mut self, backend: Arc<dyn SigningBackend>) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Approve the pending request and run the REAL threshold ceremony via
    /// the attached backend (#94). `password` unlocks this device's share —
    /// the desktop passes it from its unlock flow; it is handed to the
    /// backend and never stored here. Returns the aggregated signature.
    pub async fn approve_and_sign(&self, request_id: &str, password: String) -> CoreResult<String> {
        let Some(backend) = self.backend.as_ref() else {
            return Err(CoreError::Dkg(
                "no signing backend attached — construct SigningManager::with_backend".into(),
            ));
        };

        let request = self.state.active_signing_request.lock().await.clone();
        let Some(req) = request else {
            return Err(CoreError::Dkg("no pending signing request".into()));
        };
        if req.id != request_id {
            return Err(CoreError::Dkg(format!(
                "signing request id mismatch: expected {}, got {}",
                req.id, request_id
            )));
        }

        // Resolve the keystore wallet id from the request's wallet index. A
        // peer's request needs none: the announce already names the wallet.
        let wallet_id = if req.session_id.is_some() {
            String::new()
        } else {
            let wallets = self.state.wallets.lock().await;
            wallets
                .get(req.wallet_index)
                .map(|w| w.id.clone())
                .ok_or_else(|| {
                    CoreError::Dkg(format!("wallet index {} out of range", req.wallet_index))
                })?
        };

        // The elm loop drives commitment→share→aggregate internally; surface
        // the coarse states so the desktop progress UI moves.
        *self.state.signing_state.lock().await = SigningState::Commitment;
        self.ui_callback
            .update_signing_state(SigningState::Commitment)
            .await;

        let outcome = backend
            .sign(BackendSignRequest {
                wallet_id,
                session_id: req.session_id.clone(),
                message_hex: req.message_hex.clone(),
                password,
                // Quorum approval is human-interactive: minutes, not seconds.
                timeout: Duration::from_secs(300),
            })
            .await;

        match outcome {
            Ok(out) => {
                *self.state.last_signature.lock().await = Some(out.signature_hex.clone());
                *self.state.signing_state.lock().await = SigningState::Complete;
                *self.state.active_signing_request.lock().await = None;
                self.ui_callback
                    .update_signing_state(SigningState::Complete)
                    .await;
                self.ui_callback
                    .update_signing_complete(out.signature_hex.clone())
                    .await;
                self.ui_callback.update_signing_request(None).await;
                Ok(out.signature_hex)
            }
            Err(e) => {
                let reason = e.to_string();
                *self.state.signing_state.lock().await = SigningState::Failed(reason.clone());
                self.ui_callback
                    .update_signing_state(SigningState::Failed(reason.clone()))
                    .await;
                self.ui_callback.show_message(reason, true).await;
                Err(e)
            }
        }
    }

    /// Start a new signing flow. Surfaces the request to the UI
    /// via `update_signing_request` and transitions the core
    /// signing_state to `AwaitingApproval`. Returns the generated
    /// request id so a dApp / batch caller can correlate.
    pub async fn request_signing(
        &self,
        message_hex: String,
        chain: String,
        display_label: Option<String>,
    ) -> CoreResult<String> {
        if message_hex.is_empty() {
            return Err(CoreError::Dkg("empty message_hex".into()));
        }

        // Use the active wallet by default; callers that want to
        // pin a specific wallet can extend this later.
        let wallet_index = *self.state.active_wallet_index.lock().await;

        let request = SigningRequest {
            id: format!("sign-{}", uuid::Uuid::new_v4()),
            wallet_index,
            message_hex,
            display_label,
            chain,
            created_at: chrono::Utc::now().to_rfc3339(),
            session_id: None,
        };
        let id = request.id.clone();

        *self.state.active_signing_request.lock().await = Some(request.clone());
        *self.state.signing_state.lock().await = SigningState::AwaitingApproval;

        self.ui_callback.update_signing_request(Some(request)).await;
        self.ui_callback
            .update_signing_state(SigningState::AwaitingApproval)
            .await;

        Ok(id)
    }

    /// Approve the pending signing request. Drives the flow
    /// through Commitment → Share → Aggregating → Complete. The
    /// actual FROST rounds are TODO; this stub transitions state
    /// and emits a placeholder signature so the UI surfaces the
    /// correct sequence.
    pub async fn approve(&self, request_id: &str) -> CoreResult<()> {
        let request = self.state.active_signing_request.lock().await.clone();
        let Some(req) = request else {
            return Err(CoreError::Dkg("no pending signing request".into()));
        };
        if req.id != request_id {
            return Err(CoreError::Dkg(format!(
                "signing request id mismatch: expected {}, got {}",
                req.id, request_id
            )));
        }

        // Commitment phase
        *self.state.signing_state.lock().await = SigningState::Commitment;
        self.ui_callback
            .update_signing_state(SigningState::Commitment)
            .await;

        // TODO(starlab-desktop): route `req` into protocal::signing's
        // handle_start_signing / process_signing_round1 /
        // process_signing_round2. Needs either:
        //   (a) a ciphersuite-generic backend shared between the
        //       elm Message loop and this manager, or
        //   (b) a message-channel bridge that lets this manager
        //       drive an AppState<C> instance behind the scenes.
        // Until then: fast-forward through the states to Complete
        // with a placeholder signature so the UI flow is exercisable.

        *self.state.signing_state.lock().await = SigningState::Share;
        self.ui_callback
            .update_signing_state(SigningState::Share)
            .await;

        *self.state.signing_state.lock().await = SigningState::Aggregating;
        self.ui_callback
            .update_signing_state(SigningState::Aggregating)
            .await;

        let placeholder_sig = format!("0x{}", "0".repeat(128));
        *self.state.last_signature.lock().await = Some(placeholder_sig.clone());
        *self.state.signing_state.lock().await = SigningState::Complete;
        *self.state.active_signing_request.lock().await = None;

        self.ui_callback
            .update_signing_state(SigningState::Complete)
            .await;
        self.ui_callback
            .update_signing_complete(placeholder_sig)
            .await;
        self.ui_callback.update_signing_request(None).await;
        self.ui_callback
            .show_message(
                "Signing flow completed (placeholder signature — FROST rounds pending)".into(),
                false,
            )
            .await;

        Ok(())
    }

    /// Reject the pending signing request and reset to Idle.
    pub async fn reject(&self, request_id: &str) -> CoreResult<()> {
        let request = self.state.active_signing_request.lock().await.clone();
        let Some(req) = request else {
            return Err(CoreError::Dkg("no pending signing request".into()));
        };
        if req.id != request_id {
            return Err(CoreError::Dkg(format!(
                "signing request id mismatch: expected {}, got {}",
                req.id, request_id
            )));
        }

        // A peer's request: tell the engine we won't co-sign, so the invite
        // is dropped instead of lingering in the session list.
        if let (Some(session_id), Some(backend)) = (req.session_id.as_deref(), &self.backend) {
            backend.decline(session_id).await?;
        }

        *self.state.active_signing_request.lock().await = None;
        *self.state.signing_state.lock().await = SigningState::Idle;
        self.ui_callback.update_signing_request(None).await;
        self.ui_callback
            .update_signing_state(SigningState::Idle)
            .await;
        self.ui_callback
            .show_message("Signing request rejected".into(), false)
            .await;

        Ok(())
    }

    /// Feed every message the embedder's `HeadlessRunner` processed (call
    /// from its `on_sync(model, Some(msg))`). A PEER's signing-session
    /// announce for a wallet we hold becomes the pending `SigningRequest`
    /// (pushed via `update_signing_request`, so the approval prompt opens
    /// without the user hunting for the session); the proposer withdrawing
    /// that session clears it again.
    pub fn observe(&self, model: &Model, msg: &Message) {
        let Some(update) = incoming_update(model, msg) else {
            return;
        };
        let state = self.state.clone();
        let ui_callback = self.ui_callback.clone();
        tokio::spawn(async move { apply_incoming(&state, ui_callback.as_ref(), update).await });
    }

    pub async fn current_state(&self) -> SigningState {
        self.state.signing_state.lock().await.clone()
    }

    pub async fn last_signature(&self) -> Option<String> {
        self.state.last_signature.lock().await.clone()
    }
}

/// What a runner message means for the pending-request slot.
#[derive(Debug)]
enum IncomingUpdate {
    /// A peer asks us to co-sign.
    Offer(SigningRequest),
    /// The session with this id is gone (proposer cancelled / it ended).
    Withdrawn(String),
}

/// Pure detection: is `msg` a peer's signing session for a wallet this
/// device holds (same test as the TUI's pushed signing-request modal — we're
/// a participant and not the proposer — plus the wallet being in our
/// keystore), or the removal of one?
fn incoming_update(model: &Model, msg: &Message) -> Option<IncomingUpdate> {
    match msg {
        Message::SessionDiscovered { session } => {
            let SessionType::Signing {
                wallet_name,
                blockchain,
                ..
            } = &session.session_type
            else {
                return None;
            };
            if session.proposer_id == model.device_id
                || !session.participants.contains(&model.device_id)
            {
                return None;
            }
            let wallets = &model.wallet_state.wallets;
            let root_id = parse_child_wallet_id(wallet_name)
                .map(|(parent, _, _)| parent)
                .unwrap_or(wallet_name);
            let wallet_index = wallets.iter().position(|w| w.session_id == root_id)?;
            let wallet_label = wallets[wallet_index].display_name();
            Some(IncomingUpdate::Offer(SigningRequest {
                id: format!("sign-{}", uuid::Uuid::new_v4()),
                wallet_index,
                message_hex: session.signing_message_hex.clone().unwrap_or_default(),
                display_label: Some(format!(
                    "{} asks to sign with {} ({}-of-{})",
                    session.proposer_id, wallet_label, session.threshold, session.total
                )),
                chain: blockchain.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                session_id: Some(session.session_id.clone()),
            }))
        }
        Message::RemoveSession { session_id } => {
            Some(IncomingUpdate::Withdrawn(session_id.clone()))
        }
        _ => None,
    }
}

async fn apply_incoming(state: &CoreState, ui_callback: &dyn UICallback, update: IncomingUpdate) {
    match update {
        IncomingUpdate::Offer(request) => {
            {
                // Never clobber a request the user is already looking at or
                // a ceremony in flight (re-announces / replays of the same
                // session land here too).
                let mut active = state.active_signing_request.lock().await;
                if active.is_some() {
                    return;
                }
                *active = Some(request.clone());
            }
            *state.signing_state.lock().await = SigningState::AwaitingApproval;
            ui_callback.update_signing_request(Some(request)).await;
            ui_callback
                .update_signing_state(SigningState::AwaitingApproval)
                .await;
        }
        IncomingUpdate::Withdrawn(session_id) => {
            {
                let mut active = state.active_signing_request.lock().await;
                let awaiting = *state.signing_state.lock().await == SigningState::AwaitingApproval;
                let ours = active
                    .as_ref()
                    .is_some_and(|r| r.session_id.as_deref() == Some(session_id.as_str()));
                // Only an unanswered prompt is withdrawn; once approved, the
                // ceremony's own outcome settles the request.
                if !(ours && awaiting) {
                    return;
                }
                *active = None;
            }
            *state.signing_state.lock().await = SigningState::Idle;
            ui_callback.update_signing_request(None).await;
            ui_callback.update_signing_state(SigningState::Idle).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CoreState, UICallback};
    use async_trait::async_trait;

    struct NoopUi;

    #[async_trait]
    impl UICallback for NoopUi {
        async fn update_connection_status(&self, _: bool, _: bool) {}
        async fn update_mesh_connections(&self, _: Vec<super::super::ConnectionInfo>) {}
        async fn update_operation_mode(&self, _: super::super::OperationMode) {}
        async fn update_wallets(&self, _: Vec<super::super::WalletInfo>) {}
        async fn update_active_wallet(&self, _: usize) {}
        async fn update_available_sessions(&self, _: Vec<super::super::SessionInfo>) {}
        async fn update_active_session(&self, _: Option<super::super::SessionInfo>) {}
        async fn update_dkg_status(&self, _: bool, _: u8, _: f32) {}
        async fn update_dkg_participants(&self, _: Vec<super::super::ParticipantInfo>) {}
        async fn update_offline_status(&self, _: bool, _: bool) {}
        async fn update_sd_operations(&self, _: Vec<super::super::SDCardOperation>) {}
        async fn show_message(&self, _: String, _: bool) {}
        async fn show_progress(&self, _: String, _: f32) {}
        async fn request_confirmation(&self, _: String) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn request_then_reject_round_trips_state() {
        let state = Arc::new(CoreState::new());
        let ui: Arc<dyn UICallback> = Arc::new(NoopUi);
        let mgr = SigningManager::new(state.clone(), ui);

        let id = mgr
            .request_signing("deadbeef".into(), "ethereum".into(), None)
            .await
            .unwrap();
        assert_eq!(mgr.current_state().await, SigningState::AwaitingApproval);

        mgr.reject(&id).await.unwrap();
        assert_eq!(mgr.current_state().await, SigningState::Idle);
        assert!(state.active_signing_request.lock().await.is_none());
    }

    #[tokio::test]
    async fn request_then_approve_reaches_complete() {
        let state = Arc::new(CoreState::new());
        let ui: Arc<dyn UICallback> = Arc::new(NoopUi);
        let mgr = SigningManager::new(state.clone(), ui);

        let id = mgr
            .request_signing("deadbeef".into(), "ethereum".into(), None)
            .await
            .unwrap();
        mgr.approve(&id).await.unwrap();

        assert_eq!(mgr.current_state().await, SigningState::Complete);
        assert!(mgr.last_signature().await.is_some());
        assert!(state.active_signing_request.lock().await.is_none());
    }

    #[tokio::test]
    async fn approve_unknown_id_errors() {
        let state = Arc::new(CoreState::new());
        let ui: Arc<dyn UICallback> = Arc::new(NoopUi);
        let mgr = SigningManager::new(state, ui);

        let _id = mgr
            .request_signing("deadbeef".into(), "ethereum".into(), None)
            .await
            .unwrap();
        let wrong = mgr.approve("not-a-real-id").await;
        assert!(wrong.is_err());
    }

    #[tokio::test]
    async fn empty_message_hex_rejected() {
        let state = Arc::new(CoreState::new());
        let ui: Arc<dyn UICallback> = Arc::new(NoopUi);
        let mgr = SigningManager::new(state, ui);

        let err = mgr
            .request_signing("".into(), "ethereum".into(), None)
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn approve_and_sign_runs_the_real_backend_end_to_end() {
        use crate::core::signing_backend::ElmSigningBackend;
        use crate::elm::Message;
        use tokio::sync::mpsc::unbounded_channel;

        let state = Arc::new(CoreState::new());
        // Seed a wallet so wallet_index 0 resolves to a keystore id.
        state.wallets.lock().await.push(super::super::WalletInfo {
            id: "wallet-abc".into(),
            name: "Treasury".into(),
            address: "0xabc".into(),
            balance: "0".into(),
            chain: "ethereum".into(),
            threshold: "2/3".into(),
            participants: vec![],
        });

        // Fake elm runner: HeadlessSign → SigningComplete via the sink,
        // exactly how the embedder's on_sync feeds the real loop's output.
        let (tx, mut rx) = unbounded_channel::<Message>();
        let (backend, sink) = ElmSigningBackend::new(tx);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Message::HeadlessSign {
                    wallet_id,
                    password,
                    ..
                } = msg
                {
                    assert_eq!(wallet_id, "wallet-abc");
                    assert_eq!(password, "hunter2");
                    sink.observe(&Message::SigningComplete {
                        request_id: "r".into(),
                        message: vec![0xde, 0xad],
                        signature: vec![0xbb; 64],
                    });
                }
            }
        });

        let ui: Arc<dyn UICallback> = Arc::new(NoopUi);
        let mgr = SigningManager::new(state.clone(), ui).with_backend(backend);

        let id = mgr
            .request_signing("dead".into(), "ethereum".into(), None)
            .await
            .unwrap();
        let sig = mgr.approve_and_sign(&id, "hunter2".into()).await.unwrap();

        assert_eq!(sig, format!("0x{}", "bb".repeat(64)));
        assert_eq!(mgr.current_state().await, SigningState::Complete);
        assert_eq!(mgr.last_signature().await, Some(sig));
        assert!(state.active_signing_request.lock().await.is_none());
    }

    #[tokio::test]
    async fn approve_and_sign_without_backend_errors_clearly() {
        let state = Arc::new(CoreState::new());
        let ui: Arc<dyn UICallback> = Arc::new(NoopUi);
        let mgr = SigningManager::new(state, ui);
        let id = mgr
            .request_signing("dead".into(), "ethereum".into(), None)
            .await
            .unwrap();
        let err = mgr.approve_and_sign(&id, "pw".into()).await.unwrap_err();
        assert!(err.to_string().contains("no signing backend"));
    }

    #[tokio::test]
    async fn approve_and_sign_surfaces_failure_state() {
        use crate::core::signing_backend::ElmSigningBackend;
        use crate::elm::Message;
        use tokio::sync::mpsc::unbounded_channel;

        let state = Arc::new(CoreState::new());
        state.wallets.lock().await.push(super::super::WalletInfo {
            id: "w".into(),
            name: "n".into(),
            address: "a".into(),
            balance: "0".into(),
            chain: "ethereum".into(),
            threshold: "2/3".into(),
            participants: vec![],
        });
        let (tx, mut rx) = unbounded_channel::<Message>();
        let (backend, sink) = ElmSigningBackend::new(tx);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if matches!(msg, Message::HeadlessSign { .. }) {
                    sink.observe(&Message::SigningFailed {
                        request_id: "r".into(),
                        error: "co-signer declined".into(),
                    });
                }
            }
        });
        let ui: Arc<dyn UICallback> = Arc::new(NoopUi);
        let mgr = SigningManager::new(state.clone(), ui).with_backend(backend);
        let id = mgr
            .request_signing("dead".into(), "ethereum".into(), None)
            .await
            .unwrap();
        let err = mgr.approve_and_sign(&id, "pw".into()).await.unwrap_err();
        assert!(err.to_string().contains("co-signer declined"));
        assert!(matches!(mgr.current_state().await, SigningState::Failed(_)));
    }

    // ---- Incoming (peer-initiated) signing requests ----

    use crate::keystore::WalletMetadata;
    use crate::protocal::signal::SessionInfo as WireSession;
    use std::sync::Mutex as StdMutex;

    /// Records every `update_signing_request` push, in order.
    #[derive(Default)]
    struct RecordingUi {
        requests: StdMutex<Vec<Option<SigningRequest>>>,
    }

    #[async_trait]
    impl UICallback for RecordingUi {
        async fn update_connection_status(&self, _: bool, _: bool) {}
        async fn update_mesh_connections(&self, _: Vec<super::super::ConnectionInfo>) {}
        async fn update_operation_mode(&self, _: super::super::OperationMode) {}
        async fn update_wallets(&self, _: Vec<super::super::WalletInfo>) {}
        async fn update_active_wallet(&self, _: usize) {}
        async fn update_available_sessions(&self, _: Vec<super::super::SessionInfo>) {}
        async fn update_active_session(&self, _: Option<super::super::SessionInfo>) {}
        async fn update_dkg_status(&self, _: bool, _: u8, _: f32) {}
        async fn update_dkg_participants(&self, _: Vec<super::super::ParticipantInfo>) {}
        async fn update_offline_status(&self, _: bool, _: bool) {}
        async fn update_sd_operations(&self, _: Vec<super::super::SDCardOperation>) {}
        async fn update_signing_request(&self, request: Option<SigningRequest>) {
            self.requests.lock().unwrap().push(request);
        }
        async fn show_message(&self, _: String, _: bool) {}
        async fn show_progress(&self, _: String, _: f32) {}
        async fn request_confirmation(&self, _: String) -> bool {
            true
        }
    }

    impl RecordingUi {
        fn pushes(&self) -> Vec<Option<SigningRequest>> {
            self.requests.lock().unwrap().clone()
        }

        /// `observe` applies on a spawned task — wait for push #`n`.
        async fn wait_for_pushes(&self, n: usize) -> Vec<Option<SigningRequest>> {
            for _ in 0..200 {
                if self.requests.lock().unwrap().len() >= n {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            self.pushes()
        }
    }

    /// A model for device `me` holding root wallet `w1`.
    fn model_holding_w1() -> Model {
        let mut model = Model::new("me".into());
        let wallet: WalletMetadata = serde_json::from_value(serde_json::json!({
            "session_id": "w1",
            "device_id": "me",
            "curve_type": "secp256k1",
            "threshold": 2,
            "total_participants": 3,
            "participant_index": 2,
            "group_public_key": "",
            "created_at": "",
            "last_modified": "",
        }))
        .unwrap();
        model.wallet_state.wallets.push(wallet);
        model
    }

    fn signing_announce(proposer: &str, participants: &[&str], wallet: &str) -> Message {
        Message::SessionDiscovered {
            session: WireSession {
                session_id: "sign-session".into(),
                proposer_id: proposer.into(),
                total: 3,
                threshold: 2,
                participants: participants.iter().map(|p| p.to_string()).collect(),
                session_type: SessionType::Signing {
                    wallet_name: wallet.into(),
                    curve_type: "secp256k1".into(),
                    blockchain: "ethereum".into(),
                    group_public_key: String::new(),
                },
                curve_type: "secp256k1".into(),
                coordination_type: "network".into(),
                signing_message_hex: Some("deadbeef".into()),
            },
        }
    }

    fn manager(ui: Arc<RecordingUi>) -> (SigningManager, Arc<CoreState>) {
        let state = Arc::new(CoreState::new());
        (SigningManager::new(state.clone(), ui), state)
    }

    #[tokio::test]
    async fn peer_signing_announce_pushes_a_signing_request() {
        let ui = Arc::new(RecordingUi::default());
        let (mgr, state) = manager(ui.clone());

        mgr.observe(
            &model_holding_w1(),
            &signing_announce("peer", &["peer", "me", "third"], "w1-ethereum-0"),
        );

        let pushes = ui.wait_for_pushes(1).await;
        let req = pushes[0]
            .clone()
            .expect("an incoming request must be pushed");
        assert_eq!(req.session_id.as_deref(), Some("sign-session"));
        assert_eq!(req.message_hex, "deadbeef");
        assert_eq!(req.chain, "ethereum");
        assert_eq!(req.wallet_index, 0);
        assert!(req.display_label.unwrap().contains("peer"));
        assert_eq!(mgr.current_state().await, SigningState::AwaitingApproval);
        assert_eq!(
            state
                .active_signing_request
                .lock()
                .await
                .as_ref()
                .map(|r| r.id.clone()),
            Some(req.id)
        );
    }

    #[test]
    fn announces_not_addressed_to_us_are_ignored() {
        let model = model_holding_w1();
        // Our own announce.
        assert!(incoming_update(&model, &signing_announce("me", &["me", "peer"], "w1")).is_none());
        // We're not in the signer list.
        assert!(incoming_update(&model, &signing_announce("peer", &["peer", "x"], "w1")).is_none());
        // A wallet we don't hold.
        assert!(
            incoming_update(&model, &signing_announce("peer", &["peer", "me"], "w9")).is_none()
        );
        // A DKG invite is not a signing request.
        let Message::SessionDiscovered { mut session } =
            signing_announce("peer", &["peer", "me"], "w1")
        else {
            unreachable!()
        };
        session.session_type = SessionType::DKG;
        assert!(incoming_update(&model, &Message::SessionDiscovered { session }).is_none());
    }

    #[tokio::test]
    async fn repeat_announce_does_not_clobber_the_pending_request() {
        let ui = Arc::new(RecordingUi::default());
        let (mgr, state) = manager(ui.clone());
        let id = mgr
            .request_signing("aa".into(), "ethereum".into(), None)
            .await
            .unwrap();

        let Some(update) = incoming_update(
            &model_holding_w1(),
            &signing_announce("peer", &["peer", "me"], "w1"),
        ) else {
            panic!("expected an offer")
        };
        apply_incoming(&state, ui.as_ref(), update).await;

        assert_eq!(ui.pushes().len(), 1, "only the local request was pushed");
        assert_eq!(
            state
                .active_signing_request
                .lock()
                .await
                .as_ref()
                .unwrap()
                .id,
            id
        );
    }

    #[tokio::test]
    async fn proposer_withdrawing_the_session_clears_the_prompt() {
        let ui = Arc::new(RecordingUi::default());
        let (mgr, state) = manager(ui.clone());
        let model = model_holding_w1();

        mgr.observe(&model, &signing_announce("peer", &["peer", "me"], "w1"));
        ui.wait_for_pushes(1).await;
        mgr.observe(
            &model,
            &Message::RemoveSession {
                session_id: "sign-session".into(),
            },
        );

        let pushes = ui.wait_for_pushes(2).await;
        assert!(pushes[1].is_none(), "withdrawal must clear the prompt");
        assert!(state.active_signing_request.lock().await.is_none());
        assert_eq!(mgr.current_state().await, SigningState::Idle);
    }

    /// Fake runner that answers HeadlessJoinSession with a completed
    /// ceremony and reports every message it received.
    fn peer_backend() -> (
        Arc<crate::core::signing_backend::ElmSigningBackend>,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
    ) {
        use crate::core::signing_backend::ElmSigningBackend;
        use tokio::sync::mpsc::unbounded_channel;

        let (tx, mut rx) = unbounded_channel::<Message>();
        let (seen_tx, seen_rx) = unbounded_channel::<Message>();
        let (backend, sink) = ElmSigningBackend::new(tx);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if matches!(msg, Message::HeadlessJoinSession { .. }) {
                    sink.observe(&Message::SigningComplete {
                        request_id: "r".into(),
                        message: vec![0xde, 0xad, 0xbe, 0xef],
                        signature: vec![0xdd; 64],
                    });
                }
                let _ = seen_tx.send(msg);
            }
        });
        (backend, seen_rx)
    }

    #[tokio::test]
    async fn approving_a_peer_request_joins_its_session() {
        let ui = Arc::new(RecordingUi::default());
        let state = Arc::new(CoreState::new());
        let (backend, mut seen) = peer_backend();
        let mgr = SigningManager::new(state.clone(), ui.clone()).with_backend(backend);

        mgr.observe(
            &model_holding_w1(),
            &signing_announce("peer", &["peer", "me"], "w1"),
        );
        let req = ui.wait_for_pushes(1).await[0].clone().unwrap();

        let sig = mgr.approve_and_sign(&req.id, "pw".into()).await.unwrap();

        assert_eq!(sig, format!("0x{}", "dd".repeat(64)));
        match seen.recv().await {
            Some(Message::HeadlessJoinSession {
                session_id,
                password,
                ..
            }) => {
                assert_eq!(session_id, "sign-session");
                assert_eq!(password, "pw");
            }
            other => panic!("approve must join the peer session, got {other:?}"),
        }
        assert!(ui.pushes().last().unwrap().is_none(), "prompt closes");
        assert!(state.active_signing_request.lock().await.is_none());
    }

    #[tokio::test]
    async fn rejecting_a_peer_request_declines_its_session() {
        let ui = Arc::new(RecordingUi::default());
        let state = Arc::new(CoreState::new());
        let (backend, mut seen) = peer_backend();
        let mgr = SigningManager::new(state.clone(), ui.clone()).with_backend(backend);

        mgr.observe(
            &model_holding_w1(),
            &signing_announce("peer", &["peer", "me"], "w1"),
        );
        let req = ui.wait_for_pushes(1).await[0].clone().unwrap();

        mgr.reject(&req.id).await.unwrap();

        match seen.recv().await {
            Some(Message::DeclineSigningRequest { session_id }) => {
                assert_eq!(session_id, "sign-session")
            }
            other => panic!("reject must decline the peer session, got {other:?}"),
        }
        assert!(ui.pushes().last().unwrap().is_none(), "prompt closes");
        assert_eq!(mgr.current_state().await, SigningState::Idle);
    }
}
