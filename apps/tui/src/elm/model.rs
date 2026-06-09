//! Model - The application state
//!
//! The Model represents the complete state of the application following
//! the Elm Architecture pattern. All state is centralized here.

use crate::keystore::{Keystore, WalletMetadata};
use crate::protocal::signal::SessionInfo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The complete application state
#[derive(Debug, Clone)]
pub struct Model {
    /// Core application state
    pub wallet_state: WalletState,
    pub network_state: NetworkState,
    pub ui_state: UIState,
    
    /// Navigation
    pub navigation_stack: Vec<Screen>,
    pub current_screen: Screen,
    
    /// Session management
    pub active_session: Option<SessionInfo>,
    pub pending_operations: Vec<Operation>,
    pub session_invites: Vec<SessionInfo>,
    
    /// User context
    pub selected_wallet: Option<String>,
    pub device_id: String,
    
    /// Application metadata
    pub app_version: String,
    pub last_saved: Option<DateTime<Utc>>,
}

impl Model {
    pub fn new(device_id: String) -> Self {
        Self {
            wallet_state: WalletState::default(),
            network_state: NetworkState::default(),
            ui_state: UIState::default(),
            navigation_stack: Vec::new(),
            current_screen: Screen::Welcome,
            active_session: None,
            pending_operations: Vec::new(),
            session_invites: Vec::new(),
            selected_wallet: None,
            device_id,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            last_saved: None,
        }
    }
    
    /// Push a screen to the navigation stack
    pub fn push_screen(&mut self, screen: Screen) {
        self.navigation_stack.push(self.current_screen.clone());
        self.current_screen = screen;
    }
    
    /// Pop a screen from the navigation stack
    pub fn pop_screen(&mut self) -> bool {
        if let Some(prev_screen) = self.navigation_stack.pop() {
            self.current_screen = prev_screen;
            true
        } else {
            false
        }
    }
    
    /// Clear navigation stack and go to main menu
    pub fn go_home(&mut self) {
        self.navigation_stack.clear();
        self.current_screen = Screen::MainMenu;
    }
}

impl WalletState {
    /// Zero-out the PasswordPrompt draft buffers + associated UI state.
    /// Called on every exit from `Screen::PasswordPrompt` (Esc, go_home,
    /// successful submit) so cleartext never outlives the screen. Also
    /// resets `password_prompt_purpose` so the next push starts from the
    /// `SetNew` default — the cold-start-sign path explicitly overrides
    /// this to `Unlock` right before its `push_screen`.
    pub fn clear_password_draft(&mut self) {
        self.password_draft.clear();
        self.confirm_draft.clear();
        self.password_error = None;
        self.password_focus_confirm = false;
        self.wallet_name_draft.clear();
        self.wallet_name_focus = false;
        self.password_prompt_purpose = PasswordPromptPurpose::default();
    }

    /// Zero-out the SignTransaction draft. Called on exit + on successful
    /// submit. No security need to wipe this (the message is not secret),
    /// but clearing it stops stale input from bleeding into the next
    /// sign attempt.
    pub fn clear_sign_draft(&mut self) {
        self.sign_message_draft.clear();
    }
}

/// Wallet-related state
#[derive(Clone, Default)]
pub struct WalletState {
    pub wallets: Vec<WalletMetadata>,
    pub keystore_initialized: bool,
    pub keystore_path: String,
    pub keystore: Option<std::sync::Arc<Keystore>>,
    pub selected_wallet: Option<String>,
    pub creating_wallet: Option<CreateWalletState>,
    pub dkg_in_progress: bool,
    /// Current phase of the FROST DKG protocol. The DKGProgress component
    /// is rebuilt from Model on every remount, so we keep the round here
    /// rather than inside the component so it survives remounts. Updated
    /// by Update handlers at each protocol transition and consumed by
    /// `App::mount_screen_components` when rendering DKGProgress.
    pub dkg_round: crate::elm::message::DKGRound,
    /// Password for encrypting this device's share before it's written to
    /// the keystore. Captured on the `PasswordPrompt` screen; consumed by
    /// the wallet-finalization Command once DKG produces a KeyPackage; then
    /// cleared so we don't keep the cleartext password sitting in process
    /// memory any longer than necessary. `None` outside the wallet-creation
    /// window.
    pub pending_password: Option<String>,
    /// Live input buffer for the optional wallet-name field shown on
    /// `Screen::PasswordPrompt` in the `SetNew` (wallet-creation) flow.
    /// Persisted as the keystore's `metadata.label` at finalize time;
    /// empty → the UI falls back to the deterministic wallet id. Never
    /// shown in the `Unlock` flow.
    pub wallet_name_draft: String,
    /// `true` iff the wallet-name field currently has keyboard focus.
    /// When set it overrides `password_focus_confirm`. Only meaningful in
    /// the `SetNew` flow; reset to `false` for `Unlock`.
    pub wallet_name_focus: bool,
    /// Live input buffer for the password field on `Screen::PasswordPrompt`.
    /// Mutated keystroke-by-keystroke through the `Password*` messages and
    /// cleared the moment `PasswordSubmitDraft` validates — the cleartext
    /// is not allowed to linger after handoff to `pending_password`.
    pub password_draft: String,
    /// Live input buffer for the confirm field. Same lifetime as
    /// `password_draft`.
    pub confirm_draft: String,
    /// Which of the two fields the keyboard currently types into.
    /// `false` = password field, `true` = confirm field. Toggled by Tab /
    /// BackTab via `Message::PasswordToggleField`.
    pub password_focus_confirm: bool,
    /// Inline validation error from the most recent submit attempt; cleared
    /// the moment the user types anything (stale errors are worse than
    /// none).
    pub password_error: Option<String>,
    /// Why the `PasswordPrompt` screen is currently mounted.
    /// Defaults to `SetNew` (creator/joiner DKG paths) and is flipped to
    /// `Unlock` only by `Message::ConfirmSigningRequest` when a cold-start
    /// signing flow lands on a wallet whose key share is still on disk.
    /// Drives both rendering (single field + different copy in `Unlock`)
    /// and validation (`PasswordSubmitDraft` skips the confirm-match
    /// check in `Unlock`). Reset to default on every exit from the
    /// screen via `clear_password_draft`.
    pub password_prompt_purpose: PasswordPromptPurpose,
    /// Snapshot of the most recently finalised wallet, populated by
    /// `Message::DKGFinalized` and rendered by the `WalletComplete`
    /// screen. We keep this on `WalletState` rather than re-deriving at
    /// render time because the per-chain address list comes off
    /// `AppState.blockchain_addresses` during the finalize Command —
    /// the UI layer has no access to that. Cleared on next
    /// `NavigateHome` so stale data doesn't bleed into a later flow.
    pub last_finalized_wallet: Option<CompletedWalletInfo>,
    /// The FROST ciphersuite this binary is running — `"secp256k1"` or
    /// `"ed25519"`. The Elm `update()` function is plain data (no
    /// generic `C: Ciphersuite`), so it can't call `C::curve_type()`
    /// itself. `ElmApp::new<C>` sets this once at boot, and every
    /// update-layer site that used to hardcode `"unified"` now reads
    /// from here instead — keeps session announcements honest about
    /// what curve is actually running.
    pub curve_type: &'static str,
    /// `true` when this node runs a UNIFIED ceremony (ed25519 + secp256k1 from
    /// one DKG). Set on the creator from `Message::SetUnifiedMode` (CLI
    /// `--curve unified`); the announce then carries `curve_type: "unified"`
    /// and joiners learn it from the announce. Independent of `curve_type`
    /// (which stays the runner's concrete curve).
    pub unified: bool,
    /// Live draft of the message the user is typing on the
    /// `SignTransaction` screen. Cleared on submit and on every exit
    /// from the screen (same discipline as `password_draft`).
    pub sign_message_draft: String,
    /// Stashed bytes-to-sign for the signing flow that threads through
    /// PasswordPrompt → UnlockWallet → JoinSigning. Set by the
    /// SubmitPassword handler when it sees a `SessionType::Signing`;
    /// consumed by `WalletUnlocked` once the KeyPackage is loaded.
    pub pending_sign_message: Option<Vec<u8>>,
    /// The USER-facing message (pre-EIP-191-hash) that the creator
    /// typed on SignTransaction. Stashed alongside
    /// `pending_sign_message` (which is the hash bytes for
    /// secp256k1) so `SigningComplete` can display both. `None` on
    /// the joiner path — joiners only have the hash from the announce.
    pub pending_raw_message: Option<Vec<u8>>,
    /// Which wallet to unlock for the pending signing flow. Same
    /// lifecycle as `pending_sign_message`.
    pub pending_sign_wallet_id: Option<String>,
    /// Which signing session we're about to join. Same lifecycle.
    pub pending_sign_session_id: Option<String>,
    /// Snapshot of the most recently produced signature. Populated by
    /// `Message::SigningComplete`; rendered by the `SignatureComplete`
    /// screen. Cleared on `NavigateHome` so a second signing attempt
    /// doesn't render stale data.
    pub last_completed_signature: Option<CompletedSignatureInfo>,
    /// Wallet id whose `KeyPackage` is currently loaded in
    /// `AppState.key_package`. Set by `Message::DKGFinalized` (DKG
    /// leaves the key live on AppState) and by `Message::WalletUnlocked`
    /// (explicit unlock from disk). `update.rs` uses this as a proxy
    /// for "is this wallet unlockable-free for the next signing
    /// ceremony?" since the update function can't reach `AppState`
    /// synchronously.
    ///
    /// Cleared by `NavigateHome` and on process restart (default
    /// `None`). An explicit lock-wallet action would clear this too;
    /// we don't have one today.
    pub wallet_unlocked_id: Option<String>,
    /// Short-lived "user hit Enter on SignTransaction, awaiting
    /// confirmation modal" staging area. SignSubmit computes the
    /// hash + the warm/cold branch decision and stashes it here;
    /// ConfirmSigningRequest executes the real dispatch;
    /// CancelSigningRequest clears it and dismisses the modal. The
    /// window is one render frame typically, but storing on Model
    /// (rather than inside Message::Confirm's boxed payload) avoids
    /// threading `Vec<u8>` through the modal plumbing.
    pub pending_sign_preview: Option<PendingSignPreview>,
    /// Device ids of co-signers whose FROST round-1 commitment has
    /// landed on this node. Used by the SigningProgress screen to
    /// render a live acceptance roster ("Bob ✓ committed"). Populated
    /// by `Message::ProcessSigningRound1`; cleared on InitiateSigning
    /// (fresh ceremony) and on SigningComplete / SigningFailed /
    /// NavigateHome.
    pub signing_commitments_received: std::collections::HashSet<String>,
    /// Device ids of co-signers whose FROST round-2 signature share
    /// has landed. Tracks the second ceremony phase; shown on the
    /// progress screen as "✓✓" to differentiate from commit-only.
    pub signing_shares_received: std::collections::HashSet<String>,
}

/// Discriminator for the three flows that share `Screen::PasswordPrompt`.
/// `SetNew` covers creator + joiner DKG (collecting a new password to
/// encrypt this device's freshly-generated key share). `Unlock` covers
/// cold-start signing, where the wallet exists on disk and the user
/// must enter the password they originally chose at DKG time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PasswordPromptPurpose {
    /// Two-field "Set + Confirm" flow used by DKG. This is the default
    /// because the screen has historically only existed for DKG; the
    /// signing path opted in by setting `Unlock` before its push.
    #[default]
    SetNew,
    /// Single-field unlock flow used by cold-start signing.
    Unlock,
}

/// Ephemeral snapshot of a signing request that's awaiting user
/// confirmation in the creator-side Modal::Confirm. Written by
/// `Message::SignSubmit`, consumed by `Message::ConfirmSigningRequest`,
/// cleared by `Message::CancelSigningRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSignPreview {
    /// Wallet that owns the key share; the confirm step routes
    /// through this wallet's unlock state.
    pub wallet_id: String,
    /// The bytes FROST will actually sign. For secp256k1 this is the
    /// 32-byte EIP-191 hash; for ed25519 it's the raw message.
    pub bytes_to_sign: Vec<u8>,
    /// The user-typed message (pre-hash), kept so SignatureComplete
    /// can render both "what the user typed" and "what was signed".
    /// `None` for ed25519 / raw-bytes paths where they're the same.
    pub raw_message: Option<Vec<u8>>,
    /// `true` = the wallet is already unlocked (warm path, dispatch
    /// InitiateSigning on confirm); `false` = cold path, route
    /// through PasswordPrompt on confirm.
    pub warm: bool,
}

/// Snapshot of the data the `WalletComplete` screen needs to render.
/// Populated by `Message::DKGFinalized` after the keystore write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedWalletInfo {
    pub wallet_id: String,
    /// Hex-encoded compressed group verifying key (33 bytes → 66 hex chars
    /// for secp256k1, 32 → 64 for ed25519). This is what signers would
    /// publish to the chain.
    pub group_pubkey_hex: String,
    /// `"secp256k1"` or `"ed25519"`. Used to pick the icon / address
    /// format in the view; the keystore already knows this via
    /// `metadata.curve_type`, but the component doesn't have a keystore
    /// handle so we pass it through.
    pub curve_type: String,
    /// `(chain_id, address)` pairs as emitted by the finalize Command.
    /// Guaranteed to be in the order returned by
    /// `blockchain_config::get_compatible_chains`, which is stable per
    /// curve.
    pub addresses: Vec<(String, String)>,
}

/// Snapshot of the data the `SignatureComplete` screen needs. Populated
/// by the `Message::SigningComplete` handler after FROST `aggregate`
/// produces a signature that verifies under the group key. The raw
/// `signature` + `message` bytes are retained so the component can
/// show both hex renderings; the `verified` flag is the outcome of the
/// explicit verifying_key check the protocol layer runs before
/// emitting SigningComplete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedSignatureInfo {
    pub request_id: String,
    pub wallet_id: String,
    /// The USER-visible message — what they typed on SignTransaction.
    /// Held separately from `signed_hash` because for secp256k1 we
    /// sign the EIP-191 hash of this message (what ecrecover
    /// expects), not the message bytes themselves.
    pub message: Vec<u8>,
    /// What FROST actually signed. For secp256k1 this is the 32-byte
    /// EIP-191 hash of `message`; for ed25519 it's the message bytes
    /// themselves (ed25519 signs variable-length input directly).
    /// `None` means "same as `message`" and preserves the pre-EIP-191
    /// semantics for raw-bytes signing.
    pub signed_hash: Option<Vec<u8>>,
    /// Aggregated FROST signature as the FROST library returned it.
    /// For secp256k1 that's 65 bytes (compressed group-key prefix +
    /// 32-byte z); ed25519 is 64 bytes.
    pub signature: Vec<u8>,
    /// Result of `verifying_key.verify(&message, &signature)` the
    /// protocol layer ran before emitting SigningComplete. Always
    /// `true` on the happy path; a `false` here means something went
    /// wrong before the emit and this screen shouldn't actually be
    /// reachable — but we surface the flag defensively.
    pub verified: bool,
}

// Manual Debug implementation for WalletState
impl std::fmt::Debug for WalletState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalletState")
            .field("wallets", &self.wallets)
            .field("keystore_initialized", &self.keystore_initialized)
            .field("keystore_path", &self.keystore_path)
            .field("keystore", &self.keystore.is_some())  // Just show if present
            .field("selected_wallet", &self.selected_wallet)
            .field("creating_wallet", &self.creating_wallet)
            .field("dkg_in_progress", &self.dkg_in_progress)
            .field("dkg_round", &self.dkg_round)
            // Never log the actual password, even at debug level — just
            // report whether one is currently staged.
            .field("pending_password", &self.pending_password.as_ref().map(|_| "<redacted>"))
            // Redact cleartext but keep lengths/focus/error visible for debugging.
            .field("password_draft_len", &self.password_draft.len())
            .field("confirm_draft_len", &self.confirm_draft.len())
            .field("password_focus_confirm", &self.password_focus_confirm)
            .field("password_error", &self.password_error)
            .field("password_prompt_purpose", &self.password_prompt_purpose)
            .finish()
    }
}

/// Network-related state
#[derive(Debug, Clone)]
pub struct NetworkState {
    pub connected: bool,
    pub peers: Vec<String>,
    pub websocket_url: String,
    pub connection_status: ConnectionStatus,
    pub last_ping: Option<DateTime<Utc>>,
    pub reconnect_attempts: u32,
    pub max_reconnect_attempts: u32,
    pub participant_webrtc_status: std::collections::HashMap<String, (bool, bool)>, // (webrtc_connected, data_channel_open)
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            connected: false,
            peers: Vec::new(),
            websocket_url: "wss://panda.qzz.io".to_string(),
            connection_status: ConnectionStatus::Disconnected,
            last_ping: None,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            participant_webrtc_status: std::collections::HashMap::new(),
        }
    }
}

/// UI-related state
#[derive(Debug, Clone)]
pub struct UIState {
    pub focus: ComponentId,
    pub modal: Option<Modal>,
    pub notifications: Vec<Notification>,
    pub input_buffer: String,
    pub scroll_position: u16,
    pub selected_indices: HashMap<ComponentId, usize>,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub is_busy: bool,
    pub progress: Option<ProgressInfo>,
    pub join_session_tab: usize, // 0 = DKG, 1 = Signing
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            focus: ComponentId::MainMenu,
            modal: None,
            notifications: Vec::new(),
            input_buffer: String::new(),
            scroll_position: 0,
            selected_indices: HashMap::new(),
            error_message: None,
            success_message: None,
            is_busy: false,
            progress: None,
            join_session_tab: 0, // Default to DKG tab
        }
    }
}

/// Represents different screens in the application
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screen {
    Welcome,
    MainMenu,
    
    // Wallet management
    CreateWallet(CreateWalletState),
    ManageWallets,
    WalletDetail { wallet_id: String },
    ImportWallet,
    ExportWallet { wallet_id: String },
    
    // DKG flow
    PathSelection,
    ModeSelection,
    ThresholdConfig,
    TemplateSelection,
    WalletConfiguration(WalletConfig),
    /// Collects a password for encrypting this device's key share before DKG
    /// starts. Entered from either the creator path (post-ThresholdConfig)
    /// or the joiner path (post-AcceptSession); on submit the password is
    /// stashed in `Model.wallet_state.pending_password` and the screen
    /// advances to `DKGProgress`. Creator-vs-joiner is inferred from
    /// `Model.active_session` at the transition point — no need to carry
    /// that distinction in the variant itself.
    PasswordPrompt,
    DKGProgress { session_id: String },
    WalletComplete { wallet_id: String },
    
    // Session management
    JoinSession,
    SessionDetail { session_id: String },
    AcceptSession { sessions: Vec<SessionInfo> },
    
    // Signing flow
    SignTransaction { wallet_id: String },
    SigningProgress { request_id: String },
    /// Terminal screen shown after a successful FROST aggregate.
    /// `request_id` is the signing ceremony id used by
    /// `Message::SigningComplete`; the full signature payload lives
    /// on `Model.wallet_state.last_completed_signature` so the
    /// rendered component can pull it via `set_from_model` without
    /// the variant carrying unbounded data.
    SignatureComplete { request_id: String },
    
    // Settings
    Settings,
    NetworkSettings,
    SecuritySettings,
    About,
}

/// State for wallet creation flow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CreateWalletState {
    pub mode: Option<WalletMode>,
    pub template: Option<WalletTemplate>,
    pub custom_config: Option<WalletConfig>,
}

/// Wallet creation mode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WalletMode {
    #[default]
    Online,
    Offline,
    Hybrid,
}

/// Supported curve types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CurveType {
    #[default]
    Secp256k1,
    Ed25519,
}

/// Wallet templates
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletTemplate {
    pub name: String,
    pub description: String,
    pub total_participants: u16,
    pub threshold: u16,
    pub security_level: String,
    pub use_case: String,
}

/// Wallet configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WalletConfig {
    pub name: String,
    pub total_participants: u16,
    pub threshold: u16,
    pub mode: WalletMode,
}

/// Component identifiers for focus management
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComponentId {
    MainMenu,
    WalletList,
    SessionList,
    InputField,
    Modal,
    Notification,
    CreateWallet,
    ModeSelection,
    ThresholdConfig,
    JoinSession,
    DKGProgress,
    /// Focus target for the pre-DKG password-capture screen.
    PasswordPrompt,
    /// Focus target for the post-DKG wallet-complete screen.
    WalletComplete,
    /// Focus target for the SignTransaction screen (Phase C).
    SignTransaction,
    /// Focus target for the SignatureComplete success screen (Phase C.5).
    SignatureComplete,
    Custom(String),
}

/// Modal dialog types
#[derive(Debug, Clone)]
pub enum Modal {
    Confirm {
        title: String,
        message: String,
        on_confirm: Box<Message>,
        on_cancel: Box<Message>,
    },
    Progress {
        title: String,
        message: String,
        progress: f32,
    },
    Error {
        title: String,
        message: String,
    },
    Success {
        title: String,
        message: String,
    },
    Input {
        title: String,
        prompt: String,
        default_value: String,
        on_submit: Box<fn(String) -> Message>,
    },
}

// Manual PartialEq implementation for Modal (ignoring function pointers and comparing f32)
impl PartialEq for Modal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Modal::Confirm { title: t1, message: m1, .. }, 
             Modal::Confirm { title: t2, message: m2, .. }) => t1 == t2 && m1 == m2,
            (Modal::Progress { title: t1, message: m1, progress: p1 }, 
             Modal::Progress { title: t2, message: m2, progress: p2 }) => t1 == t2 && m1 == m2 && (p1 - p2).abs() < f32::EPSILON,
            (Modal::Error { title: t1, message: m1 }, 
             Modal::Error { title: t2, message: m2 }) => t1 == t2 && m1 == m2,
            (Modal::Success { title: t1, message: m1 }, 
             Modal::Success { title: t2, message: m2 }) => t1 == t2 && m1 == m2,
            (Modal::Input { title: t1, prompt: p1, default_value: d1, .. }, 
             Modal::Input { title: t2, prompt: p2, default_value: d2, .. }) => t1 == t2 && p1 == p2 && d1 == d2,
            _ => false,
        }
    }
}

/// Notification types
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: String,
    pub text: String,
    pub kind: NotificationKind,
    pub timestamp: DateTime<Utc>,
    pub dismissible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationKind {
    Info,
    Success,
    Warning,
    Error,
}

/// Progress information for long-running operations
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub operation: String,
    pub progress: f32,
    pub message: String,
    pub started_at: DateTime<Utc>,
    pub estimated_completion: Option<DateTime<Utc>>,
}

/// Connection status
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}

/// Pending operations
#[derive(Debug, Clone)]
pub enum Operation {
    CreateWallet(WalletConfig),
    ImportWallet { path: String },
    ExportWallet { wallet_id: String, path: String },
    DeleteWallet { wallet_id: String },
    StartDKG { config: WalletConfig },
    JoinDKG { session_id: String },
    SignTransaction { wallet_id: String, data: Vec<u8> },
}

use crate::elm::message::Message;

/// Persistent state that can be saved/loaded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    pub device_id: String,
    pub websocket_url: String,
    pub selected_wallet: Option<String>,
    pub keystore_path: String,
    pub last_screen: Screen,
}

impl Model {
    /// Convert to persistent state for saving
    pub fn to_persistent(&self) -> PersistentState {
        PersistentState {
            device_id: self.device_id.clone(),
            websocket_url: self.network_state.websocket_url.clone(),
            selected_wallet: self.selected_wallet.clone(),
            keystore_path: self.wallet_state.keystore_path.clone(),
            last_screen: self.current_screen.clone(),
        }
    }
    
    /// Create from persistent state
    pub fn from_persistent(state: PersistentState) -> Self {
        let mut model = Self::new(state.device_id);
        model.network_state.websocket_url = state.websocket_url;
        model.selected_wallet = state.selected_wallet;
        model.wallet_state.keystore_path = state.keystore_path;
        model.current_screen = Screen::MainMenu; // Always start at main menu
        model
    }
}