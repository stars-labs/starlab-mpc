//! Shared core logic for both TUI and native nodes
//! This module contains all the business logic that can be reused across different UI implementations

pub mod dkg_manager;
pub mod session_manager;
pub mod offline_manager;
pub mod wallet_manager;
pub mod connection_manager;
pub mod signing_manager;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Common result type for core operations
pub type CoreResult<T> = Result<T, CoreError>;

/// Core error types
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("DKG error: {0}")]
    Dkg(String),
    
    #[error("Session error: {0}")]
    Session(String),
    
    #[error("Wallet error: {0}")]
    Wallet(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Offline mode error: {0}")]
    Offline(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Wallet information shared between UIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub id: String,
    pub name: String,
    pub address: String,
    pub balance: String,
    pub chain: String,
    pub threshold: String,
    pub participants: Vec<String>,
}

/// Session information shared between UIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub initiator: String,
    pub participants: Vec<String>,
    pub threshold: (u16, u16),
    pub status: SessionStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Waiting,
    InProgress,
    Completed,
    Failed,
}

/// DKG participant information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub id: String,
    pub name: String,
    pub status: ParticipantStatus,
    pub round_completed: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParticipantStatus {
    Ready,
    Processing,
    Completed,
    Failed,
    Offline,
}

/// Connection information for mesh networking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub peer_id: String,
    pub status: ConnectionStatus,
    pub latency_ms: u32,
    pub quality: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Failed,
}

/// Operation mode for the wallet
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationMode {
    Online,
    Offline,
    Hybrid,
}

/// SD Card operation for offline mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SDCardOperation {
    pub operation_type: SDOperationType,
    pub data_type: String,
    pub participant: String,
    pub timestamp: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SDOperationType {
    Export,
    Import,
}

/// A pending threshold-signing request surfaced to the UI.
///
/// Shape mirrors what the browser extension's `RpcHandler` puts on
/// `pendingDappRequests` — a caller (dApp, signing UI, or SD-card
/// import) prepares the transaction/message bytes; the core awaits
/// user approval before running the FROST commit/share/aggregate
/// rounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningRequest {
    /// Opaque id; the UI echoes it back when approving/rejecting.
    pub id: String,
    /// Which wallet to sign with (index into CoreState.wallets).
    pub wallet_index: usize,
    /// Raw bytes to sign, hex-encoded. For EIP-191 / personal_sign
    /// the caller is responsible for pre-hashing via viem's
    /// hashMessage equivalent.
    pub message_hex: String,
    /// Human-readable label for the preview modal ("Send 0.1 ETH",
    /// "Sign in with Ethereum", etc.). Optional.
    pub display_label: Option<String>,
    /// Chain id for the rendering layer ("ethereum", "solana",
    /// "polygon", …).
    pub chain: String,
    pub created_at: String,
}

/// Lifecycle state of the current signing ceremony.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SigningState {
    Idle,
    /// UI is showing the confirm modal, waiting for
    /// approve / reject.
    AwaitingApproval,
    /// Broadcasting our round-1 commitment.
    Commitment,
    /// Collecting commitments, broadcasting our round-2 share.
    Share,
    /// Aggregating shares into the final signature.
    Aggregating,
    /// Final signature is in `CoreState.last_signature`.
    Complete,
    Failed(String),
}

/// Core state that's shared between different UI implementations
#[derive(Clone)]
pub struct CoreState {
    // Connection state
    pub websocket_connected: Arc<Mutex<bool>>,
    pub webrtc_connected: Arc<Mutex<bool>>,
    pub mesh_connections: Arc<Mutex<Vec<ConnectionInfo>>>,
    pub operation_mode: Arc<Mutex<OperationMode>>,
    
    // Wallet state
    pub wallets: Arc<Mutex<Vec<WalletInfo>>>,
    pub active_wallet_index: Arc<Mutex<usize>>,
    
    // Session state
    pub available_sessions: Arc<Mutex<Vec<SessionInfo>>>,
    pub active_session: Arc<Mutex<Option<SessionInfo>>>,
    
    // DKG state
    pub dkg_active: Arc<Mutex<bool>>,
    pub dkg_round: Arc<Mutex<u8>>,
    pub dkg_progress: Arc<Mutex<f32>>,
    pub dkg_participants: Arc<Mutex<Vec<ParticipantInfo>>>,
    
    // Offline state
    pub offline_enabled: Arc<Mutex<bool>>,
    pub sd_card_detected: Arc<Mutex<bool>>,
    pub pending_sd_operations: Arc<Mutex<Vec<SDCardOperation>>>,

    // Signing state
    pub signing_state: Arc<Mutex<SigningState>>,
    pub active_signing_request: Arc<Mutex<Option<SigningRequest>>>,
    /// Last aggregated signature (hex-encoded). Set once the
    /// FROST ceremony completes; UI reads this to display / copy.
    pub last_signature: Arc<Mutex<Option<String>>>,
}

impl Default for CoreState {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreState {
    pub fn new() -> Self {
        Self {
            websocket_connected: Arc::new(Mutex::new(false)),
            webrtc_connected: Arc::new(Mutex::new(false)),
            mesh_connections: Arc::new(Mutex::new(Vec::new())),
            operation_mode: Arc::new(Mutex::new(OperationMode::Online)),
            wallets: Arc::new(Mutex::new(Vec::new())),
            active_wallet_index: Arc::new(Mutex::new(0)),
            available_sessions: Arc::new(Mutex::new(Vec::new())),
            active_session: Arc::new(Mutex::new(None)),
            dkg_active: Arc::new(Mutex::new(false)),
            dkg_round: Arc::new(Mutex::new(0)),
            dkg_progress: Arc::new(Mutex::new(0.0)),
            dkg_participants: Arc::new(Mutex::new(Vec::new())),
            offline_enabled: Arc::new(Mutex::new(false)),
            sd_card_detected: Arc::new(Mutex::new(false)),
            pending_sd_operations: Arc::new(Mutex::new(Vec::new())),
            signing_state: Arc::new(Mutex::new(SigningState::Idle)),
            active_signing_request: Arc::new(Mutex::new(None)),
            last_signature: Arc::new(Mutex::new(None)),
        }
    }
}

/// UI update callback trait that both TUI and native implementations must provide
#[async_trait]
pub trait UICallback: Send + Sync {
    // Connection updates
    async fn update_connection_status(&self, websocket: bool, webrtc: bool);
    async fn update_mesh_connections(&self, connections: Vec<ConnectionInfo>);
    async fn update_operation_mode(&self, mode: OperationMode);
    
    // Wallet updates
    async fn update_wallets(&self, wallets: Vec<WalletInfo>);
    async fn update_active_wallet(&self, index: usize);
    
    // Session updates
    async fn update_available_sessions(&self, sessions: Vec<SessionInfo>);
    async fn update_active_session(&self, session: Option<SessionInfo>);
    
    // DKG updates
    async fn update_dkg_status(&self, active: bool, round: u8, progress: f32);
    async fn update_dkg_participants(&self, participants: Vec<ParticipantInfo>);
    
    // Offline mode updates
    async fn update_offline_status(&self, enabled: bool, sd_card_detected: bool);
    async fn update_sd_operations(&self, operations: Vec<SDCardOperation>);

    // Signing updates. Default no-op impls so existing UICallback
    // implementors (TUI's elm UIProvider, NativeUICallback) compile
    // without change; richer UIs override to render approval modals
    // and progress indicators.
    async fn update_signing_request(&self, _request: Option<SigningRequest>) {}
    async fn update_signing_state(&self, _state: SigningState) {}
    async fn update_signing_complete(&self, _signature_hex: String) {}

    // General updates
    async fn show_message(&self, message: String, is_error: bool);
    async fn show_progress(&self, title: String, progress: f32);
    async fn request_confirmation(&self, message: String) -> bool;
}