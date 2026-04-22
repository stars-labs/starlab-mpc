//! Message - All possible events in the application
//!
//! Messages represent all user actions, system events, and state transitions
//! that can occur in the application. They are the only way to trigger state changes.

use crate::elm::model::{Screen, WalletConfig, WalletMode, WalletTemplate, Modal, NotificationKind, ComponentId};
use crate::protocal::signal::SessionInfo;
use crate::utils::state::PendingSigningRequest;

/// All possible messages in the application
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    // Navigation messages
    Navigate(Screen),
    NavigateBack,
    NavigateHome,
    PushScreen(Screen),
    PopScreen,
    ForceRemount,
    
    // Wallet management messages
    CreateWallet { config: WalletConfig },
    SelectWallet { wallet_id: String },
    ListWallets,
    WalletsLoaded { wallets: Vec<crate::keystore::WalletMetadata> },
    DeleteWallet { wallet_id: String },
    WalletDeleted { wallet_id: String },
    ExportWallet { wallet_id: String },
    WalletExported { wallet_id: String, path: String },
    ImportWallet { data: Vec<u8> },
    WalletImported { wallet_id: String },
    
    // Wallet creation flow
    SelectMode(WalletMode),
    SelectTemplate(WalletTemplate),
    SetWalletName(String),
    SetThreshold(u16),
    SetTotalParticipants(u16),
    ConfirmWalletCreation,
    /// Emitted by the app-level keyboard handler when the user confirms a
    /// valid password. Handler stashes it on `Model.wallet_state.pending_password`
    /// and navigates forward to DKGProgress. `value` is the cleartext
    /// password — it gets cleared after keystore write in Stage 2.
    ///
    /// In the normal flow this is dispatched by
    /// [`Message::PasswordSubmitDraft`] after validation; tests can call it
    /// directly to skip the typing/validation step.
    SubmitPassword { value: String },
    // ----- Keystroke-level password-prompt messages -----
    // The PasswordPrompt screen's draft lives on `Model.wallet_state` rather
    // than inside the component, because tuirealm's per-component `on()` is
    // bypassed by the app-level `handle_key_event` in this codebase. Keys
    // reach the Model through these four messages; the component only
    // renders.
    /// Append a character to whichever field currently has focus (see
    /// `password_focus_confirm`). Clears any stale `password_error`.
    PasswordTypeChar(char),
    /// Pop one character from the focused field. Clears stale error.
    PasswordBackspace,
    /// Flip `password_focus_confirm`. Emitted on Tab / BackTab.
    PasswordToggleField,
    /// Run validation on the current `password_draft` / `confirm_draft`.
    /// On success: clear drafts + dispatch `SubmitPassword { value }`.
    /// On failure: set `password_error` so the view can render it.
    PasswordSubmitDraft,
    
    // DKG operations
    InitiateDKG { params: DKGParams },
    JoinSession { session_id: String },
    /// Bulk refresh: replace `session_invites` with the caller's snapshot.
    /// Emitted by explicit discovery queries (e.g. `Command::LoadSessions`).
    SessionsLoaded { sessions: Vec<SessionInfo> },
    /// Incremental add/update: merge a single session into `session_invites`
    /// (dedupe by `session_id`). Emitted by the primary WebSocket reader when
    /// the server pushes a `SessionAvailable` broadcast.
    SessionDiscovered { session: SessionInfo },
    /// Incremental drop: remove a session from `session_invites`. Emitted by
    /// the primary WebSocket reader when the server pushes a `SessionRemoved`.
    RemoveSession { session_id: String },
    UpdateDKGProgress { round: DKGRound, progress: f32 },
    UpdateDKGSessionId { real_session_id: String },
    UpdateParticipants { participants: Vec<String> },
    // WebRTC connection status updates for DKG
    UpdateParticipantWebRTCStatus {
        device_id: String,
        webrtc_connected: bool,
        data_channel_open: bool,
    },
    UpdateMeshStatus {
        ready_count: usize,
        total_count: usize,
        all_connected: bool,
    },
    DKGComplete { result: DKGResult },
    DKGFailed { error: String },
    CancelDKG,
    StartDKGProtocol,  // Trigger the actual DKG protocol when mesh is ready
    ProcessDKGRound1 { from_device: String, package_bytes: Vec<u8> },  // Process received DKG Round 1 package
    ProcessDKGRound2 { from_device: String, package_bytes: Vec<u8> },  // Process received DKG Round 2 package
    DKGKeyGenerated { group_pubkey_hex: String },                      // Final FROST key ready
    /// Fires after `Command::UnlockWallet` successfully decrypted the
    /// wallet file and stashed `KeyPackage` + `PublicKeyPackage` on
    /// AppState. The handler pushes the next screen in the signing
    /// flow (usually SignTransaction or SigningProgress, depending on
    /// which path kicked off the unlock).
    WalletUnlocked {
        wallet_id: String,
    },
    /// Emitted on any failure in `Command::UnlockWallet` — wrong
    /// password, unknown wallet id, decrypt error, or deserialize
    /// error. The update handler surfaces this as a user-visible modal
    /// and drops the user back to WalletDetail / Manage Wallets. No
    /// panic under any condition.
    WalletUnlockFailed {
        error: String,
    },

    /// Fires after `Command::FinalizeWalletFromDkg` has encrypted the key
    /// share and written the wallet file to disk. Terminates the DKG flow:
    /// the update handler clears `pending_password` / `creating_wallet`
    /// and navigates to `Screen::WalletComplete`. `addresses` is the
    /// per-chain list the user sees on that screen; deriving them up-front
    /// rather than lazily keeps the screen itself pure rendering.
    DKGFinalized {
        wallet_id: String,
        group_pubkey_hex: String,
        curve_type: String,
        addresses: Vec<(String, String)>, // (chain_id, address)
    },

    // Signing operations
    InitiateSigning { request: SigningRequest },
    SigningRequestsLoaded { requests: Vec<PendingSigningRequest> },
    ApproveSignature { request_id: String },
    RejectSignature { request_id: String },
    UpdateSigningProgress { request_id: String, progress: f32 },
    SigningComplete {
        request_id: String,
        /// Raw bytes that were signed. Embedded here so the handler can
        /// stash them on `wallet_state.last_completed_signature`
        /// without re-fetching — the protocol layer has already cleared
        /// the AppState-side copy by the time this fires.
        message: Vec<u8>,
        signature: Vec<u8>,
    },
    SigningFailed { request_id: String, error: String },
    /// Received a peer's Round 1 signing commitment over the WebRTC mesh.
    /// Dispatched by the primary data-channel reader after decoding
    /// `SIGN_COMMIT:<base64>`; the handler forwards to
    /// `Command::ProcessSigningRound1` which drives the FROST accumulator
    /// in `protocal::signing`.
    ProcessSigningRound1 { from_device: String, commitment_bytes: Vec<u8> },
    /// Received a peer's Round 2 signature share over the WebRTC mesh.
    /// Shape mirrors `ProcessSigningRound1`.
    ProcessSigningRound2 { from_device: String, share_bytes: Vec<u8> },
    // ----- SignTransaction screen input (Phase C.3) -----
    // Same routing pattern as the PasswordPrompt screen: keystrokes
    // don't reach the component's `on()` — they go through the app-level
    // handler into these messages, which mutate
    // `Model.wallet_state.sign_message_draft`. The component renders
    // from that draft.
    SignTypeChar(char),
    SignBackspace,
    /// Run validation + dispatch `InitiateSigning`. On failure, populate
    /// an inline error; on success, navigate forward (C.5 adds
    /// SignatureComplete; today we stay on the screen and rely on the
    /// protocol-layer notifications).
    SignSubmit,
    /// Generic "copy this text to the system clipboard" — reused by the
    /// WalletComplete / SignatureComplete success screens so the user
    /// can grab the group pubkey / signature hex with a single keypress.
    /// `label` describes what was copied (used in the notification).
    CopyToClipboard { text: String, label: String },
    
    // Network events
    WebSocketConnected,
    WebSocketDisconnected,
    TriggerReconnect,
    WebSocketError { error: String },
    PeerDiscovered { peer_id: String },
    PeerDisconnected { peer_id: String },
    NetworkMessage { from: String, data: Vec<u8> },
    InitiateWebRTCWithParticipants { participants: Vec<String> },
    CheckWebRTCConnections,
    VerifyMeshConnectivity,
    ConnectionStatusChanged { connected: bool },
    
    // Keystore events
    KeystoreInitialized { path: String },
    KeystoreError { error: String },
    KeystoreLocked,
    KeystoreUnlocked,
    
    // UI events
    KeyPressed(crossterm::event::KeyEvent),
    FocusChanged { component: ComponentId },
    InputChanged { value: String },
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ScrollTo { position: u16 },
    SelectItem { index: usize },
    
    // Modal management
    ShowModal(Modal),
    CloseModal,
    ConfirmModal,
    CancelModal,
    ModalInputSubmitted { value: String },
    
    // Notifications
    ShowNotification { text: String, kind: NotificationKind },
    ClearNotification { id: String },
    ClearAllNotifications,
    
    // Progress updates
    StartProgress { operation: String, message: String },
    UpdateProgress { progress: f32, message: Option<String> },
    CompleteProgress,
    
    // Settings
    UpdateWebSocketUrl { url: String },
    UpdateDeviceId { device_id: String },
    SaveSettings,
    LoadSettings,
    SettingsLoaded { websocket_url: String, device_id: String },
    
    // System messages
    Initialize,
    Shutdown,
    Quit,
    Refresh,
    Error { message: String },
    Success { message: String },
    Warning { message: String },
    Info { message: String },
    
    // Command execution results
    CommandCompleted { command: String },
    CommandFailed { command: String, error: String },
    
    // Time-based events
    Tick,
    Heartbeat,
    
    // No operation
    None,
}

/// DKG parameters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DKGParams {
    pub wallet_config: WalletConfig,
    pub session_id: Option<String>,
    pub coordinator: bool,
}

/// DKG round information
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DKGRound {
    #[default]
    Initialization,
    WaitingForParticipants,
    Round1,
    Round2,
    Finalization,
    /// Terminal state: `part3` returned a valid `KeyPackage` and
    /// `PublicKeyPackage`. The progress bar should read 100% and the
    /// status line should read "done" rather than "in progress".
    Complete,
}

/// DKG result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DKGResult {
    pub wallet_id: String,
    pub group_public_key: String,
    pub participant_index: u16,
    pub addresses: Vec<(String, String)>, // (chain, address)
}

/// Signing request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningRequest {
    pub wallet_id: String,
    /// The bytes FROST should actually sign — for secp256k1 Ethereum
    /// flows this is the 32-byte EIP-191 hash; for raw-bytes mode
    /// it's the user's message itself.
    pub transaction_data: Vec<u8>,
    pub chain: String,
    pub metadata: Option<String>,
    /// The user-visible message the hash was derived from. `None`
    /// means "same as `transaction_data`" (raw-bytes mode, pre-D.1
    /// behaviour). Preserved so the SignatureComplete screen can
    /// show the user what they typed alongside the cryptographic
    /// hash.
    pub raw_message: Option<Vec<u8>>,
}

impl Message {
    /// Create a key pressed message from a key event
    pub fn from_key_event(key: crossterm::event::KeyEvent) -> Self {
        Message::KeyPressed(key)
    }
    
    /// Check if this is a navigation message
    pub fn is_navigation(&self) -> bool {
        matches!(
            self,
            Message::Navigate(_)
            | Message::NavigateBack
            | Message::NavigateHome
            | Message::PushScreen(_)
            | Message::PopScreen
        )
    }
    
    /// Check if this is an error message
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Message::Error { .. }
            | Message::DKGFailed { .. }
            | Message::SigningFailed { .. }
            | Message::WebSocketError { .. }
            | Message::KeystoreError { .. }
            | Message::CommandFailed { .. }
        )
    }
    
    /// Check if this is a success message
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            Message::Success { .. }
            | Message::DKGComplete { .. }
            | Message::SigningComplete { .. }
            | Message::WalletImported { .. }
            | Message::WalletExported { .. }
            | Message::CommandCompleted { .. }
        )
    }
    
    // Removed from_global_key - using direct key handling in app.rs instead (KISS)
}

impl From<crossterm::event::KeyEvent> for Message {
    fn from(key: crossterm::event::KeyEvent) -> Self {
        Message::KeyPressed(key)
    }
}

impl Default for Message {
    fn default() -> Self {
        Message::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_is_navigation() {
        assert!(Message::NavigateBack.is_navigation());
        assert!(Message::Navigate(Screen::MainMenu).is_navigation());
        assert!(!Message::Quit.is_navigation());
        println!("✅ Navigation message detection works");
    }
    
    #[test]
    fn test_message_is_error() {
        assert!(Message::Error { message: "test".to_string() }.is_error());
        assert!(!Message::Success { message: "test".to_string() }.is_error());
        println!("✅ Error message detection works");
    }
    
    #[test]
    fn test_message_is_success() {
        assert!(Message::Success { message: "test".to_string() }.is_success());
        assert!(!Message::Error { message: "test".to_string() }.is_success());
        println!("✅ Success message detection works");
    }
}