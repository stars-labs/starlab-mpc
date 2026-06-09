// AppState Compatibility Layer
// Temporary wrapper to help migrate from AppState to StateMachine

use std::sync::Arc;
use tokio::sync::Mutex;
use frost_core::Ciphersuite;
use crate::protocal::signal::SessionInfo;
use super::state::{DkgState, MeshStatus, SigningState};

/// Application state management
/// Central state container for the MPC wallet application
pub struct AppState<C: Ciphersuite> {
    pub device_id: String,
    pub signal_server_url: String,
    pub session: Option<SessionInfo>,
    pub keystore: Option<Arc<crate::keystore::Keystore>>,
    // Legacy fields for compatibility - adding comprehensive set
    pub blockchain_addresses: Vec<crate::keystore::BlockchainInfo>,
    pub solana_public_key: Option<String>,
    pub etherum_public_key: Option<String>,
    pub pending_signatures: usize,
    pub log: Vec<String>,
    pub devices: Vec<String>,
    pub invites: Vec<SessionInfo>,
    pub available_sessions: Vec<crate::protocal::signal::SessionAnnouncement>,
    pub joining_session_id: Option<String>,
    pub current_wallet_id: Option<String>,
    pub device_connections: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Arc<webrtc::peer_connection::RTCPeerConnection>>>>,
    pub data_channels: std::collections::HashMap<String, Arc<webrtc::data_channel::RTCDataChannel>>,
    pub device_statuses: std::collections::HashMap<String, webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState>,
    pub pending_ice_candidates: std::collections::HashMap<String, Vec<webrtc::ice_transport::ice_candidate::RTCIceCandidateInit>>,
    pub making_offer: std::collections::HashMap<String, bool>,
    pub mesh_status: MeshStatus,
    pub dkg_state: DkgState,
    pub received_dkg_packages: std::collections::HashMap<String, Vec<u8>>,
    pub received_dkg_round2_packages: std::collections::HashMap<String, Vec<u8>>,
    pub webrtc_initiation_in_progress: bool,
    pub webrtc_initiation_started_at: Option<std::time::Instant>,
    pub signing_state: SigningState<C>,
    pub pending_signing_requests: Vec<super::state::PendingSigningRequest>,
    /// Raw `SIGN_COMMIT` payloads (from_device_id, commitment_bytes) that
    /// arrived BEFORE this node had a signing session — e.g. a cold-started
    /// co-signer whose `JoinSigning` (which rebuilds the session from keystore
    /// metadata) hasn't run yet when the initiator's commit lands over the
    /// freshly-formed mesh. Can't be keyed by `Identifier` yet (no
    /// participants list), so they're held raw and re-fed through
    /// `process_signing_round1` once the session exists.
    pub pending_pre_session_commitments: Vec<(String, Vec<u8>)>,
    // Additional DKG and other fields
    pub reconnection_tracker: std::collections::HashMap<String, std::time::Instant>,
    pub dkg_part1_public_package: Option<Vec<u8>>,
    pub dkg_part1_secret_package: Option<Vec<u8>>,
    pub dkg_part2_secret_package: Option<Vec<u8>>,
    pub dkg_round1_packages: std::collections::BTreeMap<frost_core::Identifier<C>, frost_core::keys::dkg::round1::Package<C>>,
    pub dkg_round2_packages: std::collections::BTreeMap<frost_core::Identifier<C>, frost_core::keys::dkg::round2::Package<C>>,
    pub key_package: Option<frost_core::keys::KeyPackage<C>>,
    pub group_public_key: Option<frost_core::VerifyingKey<C>>,
    pub public_key_package: Option<frost_core::keys::PublicKeyPackage<C>>,
    pub frost_commitments: std::collections::BTreeMap<frost_core::Identifier<C>, frost_core::round1::SigningCommitments<C>>,
    pub frost_signature_shares: std::collections::BTreeMap<frost_core::Identifier<C>, frost_core::round2::SignatureShare<C>>,
    pub frost_nonces: Option<frost_core::round1::SigningNonces<C>>,
    /// Raw bytes being signed in the current signing ceremony. Written by
    /// the coordinator-side `handle_start_signing` call, then referenced
    /// by every `process_signing_round1` / `process_signing_round2` call
    /// on this node so the FROST `SigningPackage` can be constructed
    /// identically across participants. Cleared when signing completes
    /// or fails. `None` outside a signing ceremony.
    pub signing_message: Option<Vec<u8>>,
    // More compatibility fields
    pub identifier_map: Option<std::collections::HashMap<String, frost_core::Identifier<C>>>,
    pub offline_sessions: std::collections::HashMap<String, crate::offline::OfflineSession>,
    pub offline_config: Option<crate::offline::OfflineConfig>,
    pub log_scroll: usize,
    pub round2_secret_package: Option<frost_core::keys::dkg::round2::SecretPackage<C>>,
    // --- Reshare (share refresh / resharing) ceremony state (#45). Distinct
    // from the dkg_* fields so a reshare can't be confused with a fresh DKG.
    // Refresh reuses frost's dkg round1/round2 Package + SecretPackage types.
    // The OLD share+group key live in `key_package` / `public_key_package`
    // (loaded when the wallet is unlocked); refresh_dkg_shares consumes them. ---
    pub reshare_in_progress: bool,
    pub reshare_round1_secret: Option<frost_core::keys::dkg::round1::SecretPackage<C>>,
    pub reshare_round2_secret: Option<frost_core::keys::dkg::round2::SecretPackage<C>>,
    pub reshare_round1_packages:
        std::collections::BTreeMap<frost_core::Identifier<C>, frost_core::keys::dkg::round1::Package<C>>,
    pub reshare_round2_packages:
        std::collections::BTreeMap<frost_core::Identifier<C>, frost_core::keys::dkg::round2::Package<C>>,
    /// The wallet's ORIGINAL participant list (from keystore metadata), used to
    /// map device_id → original FROST id during a reshare (design §3 — must NOT
    /// recompute over the retained set). Set when the reshare loads the wallet.
    pub reshare_original_participants: Vec<String>,
    /// Wallet id being reshared (so finalize knows which keystore to overwrite).
    pub reshare_wallet_id: Option<String>,
    /// Password to re-encrypt the refreshed share at finalize (#45 persist).
    pub reshare_password: Option<String>,
    /// Keystore base path for the refreshed-share write at finalize.
    pub reshare_keystore_path: Option<String>,
    pub pending_mesh_ready_signals: std::collections::HashSet<String>,
    // Additional fields for UI compatibility
    pub websocket_connected: bool,
    pub websocket_connecting: bool,
    pub websocket_reconnecting: bool,
    pub websocket_listener_active: bool, // Track if listener task is running
    pub dkg_in_progress: bool, // Prevents duplicate DKG sessions
    pub selected_wallet: Option<String>,
    pub own_mesh_ready_sent: bool,
    pub dkg_mode: Option<crate::protocal::dkg::DkgMode>,
    pub offline_mode: bool,
    pub session_start_time: Option<std::time::Instant>,
    pub webrtc_pending_participants: Vec<String>,
    pub websocket_error: Option<String>,
    pub websocket_internal_cmd_tx: Option<tokio::sync::mpsc::UnboundedSender<super::state::InternalCommand<C>>>,
    // Primary outbound WebSocket channel — every subsystem that needs to send
    // over the signal WebSocket (`AnnounceSession`, `RequestActiveSessions`,
    // relay frames, …) enqueues a serialized JSON string here. A single sender
    // task drains it into the socket. There is only one of these per process.
    pub websocket_msg_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    // Primary inbound fan-out — the single WebSocket reader parses each server
    // frame once and broadcasts an `Arc<ServerMsg>` on this channel. Any task
    // that needs to react (Elm-side bridge, DKG WebRTC signaling handler,
    // future subscribers) calls `subscribe()` to get its own receiver.
    // `broadcast` chosen over `mpsc` so late subscribers can be added without
    // knowing about each other.
    pub server_msg_broadcast_tx:
        Option<tokio::sync::broadcast::Sender<Arc<starlab_signal_server::ServerMsg>>>,
    // ICE candidate queue for handling race conditions
    pub ice_candidate_queue: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Vec<webrtc::ice_transport::ice_candidate::RTCIceCandidateInit>>>>,

    // --- Unified DKG (ed25519 + secp256k1 from one ceremony) ---
    /// True when this node is running a UNIFIED ceremony (curve_type == "unified").
    /// Set by the creator (from the model flag) or the joiner (from the announce).
    /// When set, the mesh-ready trigger runs `protocal::unified_dkg` instead of
    /// the single-curve `protocal::dkg`, and the generic `C` DKG fields are unused.
    pub unified_mode: bool,
    /// The concrete unified-DKG driver state (both curves). Lives here because
    /// it is NOT generic over `C`; the unified path ignores `C` entirely.
    pub unified_dkg: Option<starlab_core::unified_dkg::UnifiedDkg>,
    /// Finalize context (password, keystore_path, optional label) captured when
    /// the unified ceremony is triggered, so the round-2 completion can persist
    /// without re-threading these through every message.
    pub unified_finalize: Option<(String, String, Option<String>)>,
}

impl<C: Ciphersuite + Send + Sync + 'static> Default for AppState<C>
where
    <<C as Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
 {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Ciphersuite + Send + Sync + 'static> AppState<C> 
where
    <<C as Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
{
    pub fn new() -> Self {
        Self {
            device_id: String::new(),
            signal_server_url: String::new(),
            session: None,
            keystore: None,
            blockchain_addresses: Vec::new(),
            solana_public_key: None,
            etherum_public_key: None,
            pending_signatures: 0,
            log: Vec::new(),
            devices: Vec::new(),
            invites: Vec::new(),
            available_sessions: Vec::new(),
            joining_session_id: None,
            current_wallet_id: None,
            device_connections: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            data_channels: std::collections::HashMap::new(),
            device_statuses: std::collections::HashMap::new(),
            pending_ice_candidates: std::collections::HashMap::new(),
            making_offer: std::collections::HashMap::new(),
            mesh_status: MeshStatus::Incomplete,
            dkg_state: DkgState::Idle,
            received_dkg_packages: std::collections::HashMap::new(),
            received_dkg_round2_packages: std::collections::HashMap::new(),
            webrtc_initiation_in_progress: false,
            webrtc_initiation_started_at: None,
            signing_state: SigningState::Idle,
            pending_signing_requests: Vec::new(),
            pending_pre_session_commitments: Vec::new(),
            reconnection_tracker: std::collections::HashMap::new(),
            dkg_part1_public_package: None,
            dkg_part1_secret_package: None,
            dkg_part2_secret_package: None,
            dkg_round1_packages: std::collections::BTreeMap::new(),
            dkg_round2_packages: std::collections::BTreeMap::new(),
            key_package: None,
            group_public_key: None,
            public_key_package: None,
            frost_commitments: std::collections::BTreeMap::new(),
            frost_signature_shares: std::collections::BTreeMap::new(),
            frost_nonces: None,
            signing_message: None,
            identifier_map: None,
            offline_sessions: std::collections::HashMap::new(),
            offline_config: None,
            log_scroll: 0,
            round2_secret_package: None,
            reshare_in_progress: false,
            reshare_round1_secret: None,
            reshare_round2_secret: None,
            reshare_round1_packages: std::collections::BTreeMap::new(),
            reshare_round2_packages: std::collections::BTreeMap::new(),
            reshare_original_participants: Vec::new(),
            reshare_wallet_id: None,
            reshare_password: None,
            reshare_keystore_path: None,
            pending_mesh_ready_signals: std::collections::HashSet::new(),
            websocket_connected: false,
            websocket_connecting: false,
            websocket_reconnecting: false,
            websocket_listener_active: false,
            dkg_in_progress: false,
            selected_wallet: None,
            own_mesh_ready_sent: false,
            dkg_mode: None,
            offline_mode: false,
            session_start_time: None,
            webrtc_pending_participants: Vec::new(),
            websocket_error: None,
            websocket_internal_cmd_tx: None,
            websocket_msg_tx: None,
            server_msg_broadcast_tx: None,
            ice_candidate_queue: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            unified_mode: false,
            unified_dkg: None,
            unified_finalize: None,
        }
    }
    
    pub fn with_device_id(device_id: String) -> Self {
        Self::with_device_id_and_server(device_id, String::new())
    }
    
    pub fn with_device_id_and_server(device_id: String, signal_server_url: String) -> Self {
        Self {
            device_id,
            signal_server_url,
            session: None,
            keystore: None,
            blockchain_addresses: Vec::new(),
            solana_public_key: None,
            etherum_public_key: None,
            pending_signatures: 0,
            log: Vec::new(),
            devices: Vec::new(),
            invites: Vec::new(),
            available_sessions: Vec::new(),
            joining_session_id: None,
            current_wallet_id: None,
            device_connections: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            data_channels: std::collections::HashMap::new(),
            device_statuses: std::collections::HashMap::new(),
            pending_ice_candidates: std::collections::HashMap::new(),
            making_offer: std::collections::HashMap::new(),
            mesh_status: MeshStatus::Incomplete,
            dkg_state: DkgState::Idle,
            received_dkg_packages: std::collections::HashMap::new(),
            received_dkg_round2_packages: std::collections::HashMap::new(),
            webrtc_initiation_in_progress: false,
            webrtc_initiation_started_at: None,
            signing_state: SigningState::Idle,
            pending_signing_requests: Vec::new(),
            pending_pre_session_commitments: Vec::new(),
            reconnection_tracker: std::collections::HashMap::new(),
            dkg_part1_public_package: None,
            dkg_part1_secret_package: None,
            dkg_part2_secret_package: None,
            dkg_round1_packages: std::collections::BTreeMap::new(),
            dkg_round2_packages: std::collections::BTreeMap::new(),
            key_package: None,
            group_public_key: None,
            public_key_package: None,
            frost_commitments: std::collections::BTreeMap::new(),
            frost_signature_shares: std::collections::BTreeMap::new(),
            frost_nonces: None,
            signing_message: None,
            identifier_map: None,
            offline_sessions: std::collections::HashMap::new(),
            offline_config: None,
            log_scroll: 0,
            round2_secret_package: None,
            reshare_in_progress: false,
            reshare_round1_secret: None,
            reshare_round2_secret: None,
            reshare_round1_packages: std::collections::BTreeMap::new(),
            reshare_round2_packages: std::collections::BTreeMap::new(),
            reshare_original_participants: Vec::new(),
            reshare_wallet_id: None,
            reshare_password: None,
            reshare_keystore_path: None,
            pending_mesh_ready_signals: std::collections::HashSet::new(),
            websocket_connected: false,
            websocket_connecting: false,
            websocket_reconnecting: false,
            websocket_listener_active: false,
            dkg_in_progress: false,
            selected_wallet: None,
            own_mesh_ready_sent: false,
            dkg_mode: None,
            offline_mode: false,
            session_start_time: None,
            webrtc_pending_participants: Vec::new(),
            websocket_error: None,
            websocket_internal_cmd_tx: None,
            websocket_msg_tx: None,
            server_msg_broadcast_tx: None,
            ice_candidate_queue: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            unified_mode: false,
            unified_dkg: None,
            unified_finalize: None,
        }
    }
    
    /// Get DKG state from composite state
    pub async fn get_dkg_state(&self) -> DkgState {
        // Simplified implementation - just return the stored dkg_state
        self.dkg_state.clone()
    }
    
    /// Get mesh status from composite state
    pub async fn get_mesh_status(&self) -> MeshStatus {
        // Simplified implementation - just return the stored mesh_status
        self.mesh_status.clone()
    }
    
    /// Check if can start DKG
    pub async fn can_start_dkg(&self) -> bool {
        // Simple check based on mesh status
        matches!(self.mesh_status, MeshStatus::Ready)
    }
}

/// Create a Mutex-wrapped AppState for compatibility
pub fn create_legacy_appstate<C: Ciphersuite + Send + Sync + 'static>(device_id: String) -> Arc<Mutex<AppState<C>>> 
where
    <<C as Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
{
    Arc::new(Mutex::new(AppState::with_device_id(device_id)))
}