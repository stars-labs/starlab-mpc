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

use super::{CoreError, CoreResult, CoreState, SigningRequest, SigningState, UICallback};

pub struct SigningManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
}

impl SigningManager {
    pub fn new(state: Arc<CoreState>, ui_callback: Arc<dyn UICallback>) -> Self {
        Self { state, ui_callback }
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
        };
        let id = request.id.clone();

        *self.state.active_signing_request.lock().await = Some(request.clone());
        *self.state.signing_state.lock().await = SigningState::AwaitingApproval;

        self.ui_callback
            .update_signing_request(Some(request))
            .await;
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
        self.ui_callback
            .update_signing_request(None)
            .await;
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

    pub async fn current_state(&self) -> SigningState {
        self.state.signing_state.lock().await.clone()
    }

    pub async fn last_signature(&self) -> Option<String> {
        self.state.last_signature.lock().await.clone()
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
        async fn request_confirmation(&self, _: String) -> bool { true }
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
}
