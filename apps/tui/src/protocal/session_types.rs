//! Enhanced session types and structures for the wallet creation flow
//!
//! This module defines the data structures needed to support the documented
//! wallet creation flow with proper session discovery and announcement.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Session announcement for wallet creation discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAnnouncement {
    pub session_id: String,
    pub wallet_name: String,
    pub creator_device: String,
    pub curve_type: String, // "secp256k1" or "ed25519"
    pub total: u16,
    pub threshold: u16,
    pub participants_joined: u16,
    pub mode: String, // "Online", "Offline", "Hybrid"
    pub blockchain_support: Vec<String>, // ["ethereum", "bitcoin", etc.]
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub description: Option<String>,
    pub requires_approval: bool,
    pub tags: Vec<String>,
}

impl SessionAnnouncement {
    /// Check if the session is still valid (not expired)
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }

    /// Check if there's still room for more participants
    pub fn has_space(&self) -> bool {
        self.participants_joined < self.total
    }

    /// Get a human-readable status string
    pub fn status_string(&self) -> String {
        if !self.is_valid() {
            "Expired".to_string()
        } else if !self.has_space() {
            "Full".to_string()
        } else {
            format!("Open ({}/{})", self.participants_joined, self.total)
        }
    }

    /// Get time remaining until expiration
    pub fn time_remaining(&self) -> String {
        let now = Utc::now();
        if now >= self.expires_at {
            "Expired".to_string()
        } else {
            let duration = self.expires_at - now;
            let hours = duration.num_hours();
            let minutes = duration.num_minutes() % 60;
            
            if hours > 0 {
                format!("{}h {}m", hours, minutes)
            } else {
                format!("{}m", minutes)
            }
        }
    }
}

/// Session update notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUpdate {
    pub session_id: String,
    pub participants: Vec<String>,
    pub update_type: SessionUpdateType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionUpdateType {
    ParticipantJoined { device_id: String },
    ParticipantLeft { device_id: String },
    SessionStarted,
    SessionCompleted,
    SessionFailed { reason: String },
}

/// Enhanced session discovery filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFilter {
    pub session_type: Option<String>, // "DKG", "Signing"
    pub curve_type: Option<String>,   // "secp256k1", "ed25519"
    pub blockchain: Option<String>,   // "ethereum", "solana", etc.
    pub mode: Option<String>,         // "Online", "Offline", "Hybrid"
    pub min_threshold: Option<u16>,
    pub max_threshold: Option<u16>,
    pub only_valid: bool,            // Only non-expired sessions
    pub only_available: bool,        // Only sessions with space
    pub tags: Vec<String>,           // Filter by tags
}

impl Default for SessionFilter {
    fn default() -> Self {
        Self {
            session_type: Some("DKG".to_string()),
            curve_type: None,
            blockchain: None,
            mode: None,
            min_threshold: None,
            max_threshold: None,
            only_valid: true,
            only_available: true,
            tags: vec![],
        }
    }
}

/// Wallet creation session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSessionMetadata {
    pub wallet_name: String,
    pub creation_mode: String, // "Quick", "Custom", "MultiChain", "Offline"
    pub expected_participants: Vec<String>,
    pub blockchain_config: Vec<BlockchainConfig>,
    pub security_level: SecurityLevel,
    pub auto_save_enabled: bool,
    pub backup_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    pub blockchain: String,
    pub network: String,
    pub enabled: bool,
    pub chain_id: Option<u64>,
    pub derivation_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityLevel {
    Standard,  // Default security settings
    High,      // Enhanced verification and longer timeouts
    Maximum,   // Maximum security with additional checks
}

/// Progress tracking for wallet creation sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletCreationSessionProgress {
    pub session_id: String,
    pub stage: String, // "Configuration", "Discovery", "MeshFormation", etc.
    pub current_step: u8,
    pub total_steps: u8,
    pub participants_ready: Vec<String>,
    pub participants_pending: Vec<String>,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub last_update: DateTime<Utc>,
    pub messages: Vec<ProgressMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMessage {
    pub timestamp: DateTime<Utc>,
    pub level: MessageLevel,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// Participant readiness status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantStatus {
    pub device_id: String,
    pub display_name: Option<String>,
    pub connection_state: ConnectionState,
    pub dkg_ready: bool,
    pub last_seen: DateTime<Utc>,
    pub capabilities: ParticipantCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionState {
    Discovering,
    Connecting,
    Connected,
    Ready,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantCapabilities {
    pub supported_curves: Vec<String>,
    pub supported_blockchains: Vec<String>,
    pub offline_mode: bool,
    pub keystore_available: bool,
    pub version: String,
}

/// WebSocket message types for enhanced session management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnhancedWebSocketMessage {
    // Session Management
    SessionAnnouncement(SessionAnnouncement),
    SessionUpdate(SessionUpdate),
    SessionDiscovery { filter: Option<SessionFilter> },
    SessionDiscoveryResponse { sessions: Vec<SessionAnnouncement> },
    
    // Participant Management
    ParticipantStatus(ParticipantStatus),
    ParticipantReady { session_id: String, device_id: String },
    ParticipantCapabilities { device_id: String, capabilities: ParticipantCapabilities },
    
    // Progress Tracking
    ProgressUpdate(WalletCreationSessionProgress),
    
    // Error Handling
    SessionError { session_id: String, error: String },
    
    // Heartbeat and Connection Management
    Heartbeat { device_id: String, timestamp: DateTime<Utc> },
    ConnectionTest { from: String, to: String, test_id: String },
    ConnectionTestResponse { test_id: String, success: bool, latency_ms: Option<u64> },
}

impl EnhancedWebSocketMessage {
    /// Get the session ID if the message is session-related
    pub fn session_id(&self) -> Option<&str> {
        match self {
            EnhancedWebSocketMessage::SessionAnnouncement(ann) => Some(&ann.session_id),
            EnhancedWebSocketMessage::SessionUpdate(update) => Some(&update.session_id),
            EnhancedWebSocketMessage::ParticipantReady { session_id, .. } => Some(session_id),
            EnhancedWebSocketMessage::ProgressUpdate(progress) => Some(&progress.session_id),
            EnhancedWebSocketMessage::SessionError { session_id, .. } => Some(session_id),
            _ => None,
        }
    }

    /// Check if this message requires immediate handling
    pub fn is_urgent(&self) -> bool {
        matches!(self, 
            EnhancedWebSocketMessage::SessionError { .. } |
            EnhancedWebSocketMessage::ConnectionTest { .. }
        )
    }
}

/// Configuration for offline session coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineSessionConfig {
    pub export_format: OfflineFormat,
    pub encryption_enabled: bool,
    pub qr_code_support: bool,
    pub file_exchange: bool,
    pub sd_card_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OfflineFormat {
    Json,
    Binary,
    QrCode,
}

impl Default for OfflineSessionConfig {
    fn default() -> Self {
        Self {
            export_format: OfflineFormat::Json,
            encryption_enabled: true,
            qr_code_support: true,
            file_exchange: true,
            sd_card_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_announcement_validity() {
        let now = Utc::now();
        let announcement = SessionAnnouncement {
            session_id: "test-session".to_string(),
            wallet_name: "test-wallet".to_string(),
            creator_device: "device-1".to_string(),
            curve_type: "secp256k1".to_string(),
            total: 3,
            threshold: 2,
            participants_joined: 1,
            mode: "Online".to_string(),
            blockchain_support: vec!["ethereum".to_string()],
            created_at: now,
            expires_at: now + chrono::Duration::hours(24),
            description: None,
            requires_approval: true,
            tags: vec!["dkg".to_string()],
        };

        assert!(announcement.is_valid());
        assert!(announcement.has_space());
        assert_eq!(announcement.status_string(), "Open (1/3)");
    }

    #[test]
    fn test_session_filter_default() {
        let filter = SessionFilter::default();
        assert_eq!(filter.session_type, Some("DKG".to_string()));
        assert!(filter.only_valid);
        assert!(filter.only_available);
    }
}