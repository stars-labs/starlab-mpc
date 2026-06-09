//! UIProvider trait for abstracting UI operations
//! 
//! This module provides the UIProvider trait that was previously in the ui module,
//! now integrated into the Elm architecture.

use async_trait::async_trait;
use crate::protocal::signal::SessionInfo;
use crate::utils::state::PendingSigningRequest;

/// Wallet display information for UI compatibility
#[derive(Debug, Clone)]
pub struct WalletDisplayInfo {
    pub session_id: String,
    pub device_id: String,
    pub curve_type: String,
    pub threshold: u16,
    pub total_participants: u16,
    pub created_at: String,
}

/// Trait for abstracting UI operations across different frontends (TUI, GUI, Web, etc.)
/// This allows the core application logic to update any UI without knowing the implementation details
#[async_trait]
pub trait UIProvider: Send + Sync {
    // Connection management
    async fn set_connection_status(&self, connected: bool);
    async fn set_device_id(&self, device_id: String);
    
    // Device list management
    async fn update_device_list(&self, devices: Vec<String>);
    async fn update_device_status(&self, device_id: String, status: String);
    
    // Session management
    async fn update_session_status(&self, status: String);
    async fn add_session_invite(&self, invite: SessionInfo);
    async fn remove_session_invite(&self, session_id: String);
    async fn set_active_session(&self, session: Option<SessionInfo>);
    
    // DKG updates
    async fn update_dkg_status(&self, status: String);
    async fn set_generated_address(&self, address: Option<String>);
    async fn set_group_public_key(&self, key: Option<String>);
    
    // Signing operations
    async fn add_signing_request(&self, request: PendingSigningRequest);
    async fn remove_signing_request(&self, signing_id: String);
    async fn update_signing_status(&self, status: String);
    async fn set_signature_result(&self, signing_id: String, signature: Vec<u8>);
    
    // Wallet management
    async fn update_wallet_list(&self, wallets: Vec<WalletDisplayInfo>);
    async fn set_selected_wallet(&self, wallet_id: Option<String>);
    
    // Logging
    async fn add_log(&self, message: String);
    async fn set_logs(&self, logs: Vec<String>);
    
    // Mesh network status
    async fn update_mesh_status(&self, ready_devices: usize, total_devices: usize);
    
    // Error handling
    async fn show_error(&self, error: String);
    async fn show_success(&self, message: String);
    
    // General status
    async fn set_busy(&self, busy: bool);
    async fn set_progress(&self, progress: Option<f32>);
}

/// NoOp implementation for testing or headless operation
pub struct NoOpUIProvider;

#[async_trait]
impl UIProvider for NoOpUIProvider {
    async fn set_connection_status(&self, _connected: bool) {}
    async fn set_device_id(&self, _device_id: String) {}
    async fn update_device_list(&self, _devices: Vec<String>) {}
    async fn update_device_status(&self, _device_id: String, _status: String) {}
    async fn update_session_status(&self, _status: String) {}
    async fn add_session_invite(&self, _invite: SessionInfo) {}
    async fn remove_session_invite(&self, _session_id: String) {}
    async fn set_active_session(&self, _session: Option<SessionInfo>) {}
    async fn update_dkg_status(&self, _status: String) {}
    async fn set_generated_address(&self, _address: Option<String>) {}
    async fn set_group_public_key(&self, _key: Option<String>) {}
    async fn add_signing_request(&self, _request: PendingSigningRequest) {}
    async fn remove_signing_request(&self, _signing_id: String) {}
    async fn update_signing_status(&self, _status: String) {}
    async fn set_signature_result(&self, _signing_id: String, _signature: Vec<u8>) {}
    async fn update_wallet_list(&self, _wallets: Vec<WalletDisplayInfo>) {}
    async fn set_selected_wallet(&self, _wallet_id: Option<String>) {}
    async fn add_log(&self, _message: String) {}
    async fn set_logs(&self, _logs: Vec<String>) {}
    async fn update_mesh_status(&self, _ready_devices: usize, _total_devices: usize) {}
    async fn show_error(&self, _error: String) {}
    async fn show_success(&self, _message: String) {}
    async fn set_busy(&self, _busy: bool) {}
    async fn set_progress(&self, _progress: Option<f32>) {}
}