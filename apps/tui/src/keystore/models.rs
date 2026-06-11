//! Data models for the keystore module.
//!
//! This module defines the data structures used by the keystore, including
//! wallet information, device metadata, and key packages.

use std::time::{SystemTime, UNIX_EPOCH};


/// Gets the current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}


/// Information about a blockchain supported by a wallet
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlockchainInfo {
    /// Blockchain identifier (e.g., "ethereum", "bsc", "polygon", "solana")
    pub blockchain: String,
    
    /// Network type (e.g., "mainnet", "testnet", "devnet")
    pub network: String,
    
    /// Chain ID for EVM-compatible chains
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u64>,
    
    /// Address on this blockchain
    pub address: String,
    
    /// Address format/encoding (e.g., "EIP-55", "base58", "bech32")
    pub address_format: String,
    
    /// Whether this blockchain is actively used
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// Optional custom RPC endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpc_endpoint: Option<String>,
    
    /// Additional metadata specific to this blockchain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

fn default_enabled() -> bool {
    true
}

/// Information about a wallet stored in the keystore
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalletInfo {
    /// Unique identifier for this wallet (UUID)
    pub wallet_id: String,

    /// User-friendly name for the wallet
    pub name: String,

    /// Type of cryptographic curve used ("secp256k1" or "ed25519")
    pub curve_type: String,

    /// List of blockchains supported by this wallet
    pub blockchains: Vec<BlockchainInfo>,

    /// Legacy fields for backward compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockchain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_address: Option<String>,

    /// Minimum number of participants required to sign (threshold)
    pub threshold: u16,

    /// Total number of participants in the wallet
    pub total_participants: u16,

    /// Unix timestamp when the wallet was created
    pub created_at: u64,

    /// Serialized group public key for this wallet
    pub group_public_key: String,

    /// Devices that have shares for this wallet
    pub devices: Vec<DeviceInfo>,

    /// User-defined tags for organizing wallets
    pub tags: Vec<String>,

    /// Optional description for the wallet
    pub description: Option<String>,
}

impl WalletInfo {
    /// Creates a new wallet info with multiple blockchain support
    pub fn new_multi_chain(
        wallet_id: String,
        name: String,
        curve_type: String,
        blockchains: Vec<BlockchainInfo>,
        threshold: u16,
        total_participants: u16,
        group_public_key: String,
        tags: Vec<String>,
        description: Option<String>,
    ) -> Self {
        Self {
            wallet_id,
            name,
            curve_type,
            blockchains,
            blockchain: None,
            public_address: None,
            threshold,
            total_participants,
            created_at: current_timestamp(),
            group_public_key,
            devices: Vec::new(),
            tags,
            description,
        }
    }

    /// Creates a new wallet info (legacy single blockchain)
    pub fn new(
        wallet_id: String,
        name: String,
        curve_type: String,
        blockchain: String,
        public_address: String,
        threshold: u16,
        total_participants: u16,
        group_public_key: String,
        tags: Vec<String>,
        description: Option<String>,
    ) -> Self {
        // Create BlockchainInfo from legacy fields
        let blockchain_info = BlockchainInfo {
            blockchain: blockchain.clone(),
            network: "mainnet".to_string(),
            chain_id: if blockchain == "ethereum" { Some(1) } else { None },
            address: public_address,
            address_format: if blockchain == "ethereum" { "EIP-55".to_string() } else { "base58".to_string() },
            enabled: true,
            rpc_endpoint: None,
            metadata: None,
        };

        Self::new_multi_chain(
            wallet_id,
            name,
            curve_type,
            vec![blockchain_info],
            threshold,
            total_participants,
            group_public_key,
            tags,
            description,
        )
    }

    /// Gets the primary blockchain (first enabled blockchain)
    pub fn primary_blockchain(&self) -> Option<&BlockchainInfo> {
        self.blockchains.iter().find(|b| b.enabled)
    }

    /// Gets a blockchain by name
    pub fn get_blockchain(&self, blockchain: &str) -> Option<&BlockchainInfo> {
        self.blockchains.iter().find(|b| b.blockchain == blockchain)
    }

    /// Adds a device to this wallet
    pub fn add_device(&mut self, device: DeviceInfo) {
        // Replace if the device ID already exists, otherwise add
        if let Some(idx) = self
            .devices
            .iter()
            .position(|d| d.device_id == device.device_id)
        {
            self.devices[idx] = device;
        } else {
            self.devices.push(device);
        }
    }
}

/// Information about a device that can participate in signing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceInfo {
    /// Unique identifier for this device
    pub device_id: String,

    /// User-friendly name for the device
    pub name: String,

    /// Serialized FROST identifier
    pub identifier: String,

    /// Last time this device was seen/used
    pub last_seen: u64,
}

impl DeviceInfo {
    /// Creates a new device info
    pub fn new(device_id: String, name: String, identifier: String) -> Self {
        Self {
            device_id,
            name,
            identifier,
            last_seen: current_timestamp(),
        }
    }

}

/// Simplified wallet metadata - KISS and Orthogonal
/// All blockchain addresses can be derived from group_public_key + curve_type
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WalletMetadata {
    /// Wallet identifier (usually the session name from DKG)
    #[serde(alias = "wallet_id")] // For backward compatibility
    pub session_id: String,
    
    /// Device ID that owns this key share
    pub device_id: String,
    
    /// Type of cryptographic curve used ("secp256k1" or "ed25519")
    pub curve_type: String,
    
    /// Minimum number of participants required to sign (K in K-of-N)
    pub threshold: u16,
    
    /// Total number of participants (N in K-of-N)
    pub total_participants: u16,
    
    /// This device's participant index (1-based: 1, 2, 3, etc.)
    pub participant_index: u16,
    
    /// Serialized FROST group public key (source of truth for addresses)
    pub group_public_key: String,

    /// Device IDs of every participant in the DKG that produced this
    /// wallet. Used by cold-start signing to reconstruct the session
    /// (participant list + threshold + total) when `AppState.session`
    /// is empty post-restart. `#[serde(default)]` keeps wallets
    /// written by pre-field-add code deserializable — they come
    /// through with an empty `Vec`, and cold-start signing degrades
    /// to announcing with `participants=[]` exactly like before.
    #[serde(default)]
    pub participants: Vec<String>,

    /// ISO 8601 timestamp when created
    pub created_at: String,
    
    /// ISO 8601 timestamp when last modified
    pub last_modified: String,

    /// Optional user-friendly display name. Purely local — it is NOT part
    /// of the DKG wire protocol and may differ per device. The wallet's
    /// cross-device identity stays `session_id` (kept deterministic so all
    /// participants agree). UIs render `label` when set, else `session_id`.
    /// `#[serde(default)]` keeps wallets written before this field
    /// deserializable (they come through as `None`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    // === Legacy fields for backward compatibility (will be removed in v3.0) ===
    
    /// User-friendly device name (deprecated, use device_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    
    /// List of blockchains (deprecated, derive from group_public_key)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockchains: Vec<BlockchainInfo>,
    
    /// Legacy blockchain field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockchain: Option<String>,
    
    /// Legacy address field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_address: Option<String>,
    
    /// This device's identifier (deprecated, use device_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    
    /// User-defined tags (deprecated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    
    /// Optional description (deprecated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl WalletMetadata {
    /// Creates a new wallet metadata with an empty participants list
    /// (callers that know the participant list should call
    /// [`Self::with_participants`] instead, or set
    /// [`Self::participants`] directly).
    pub fn new(
        session_id: String,
        device_id: String,
        curve_type: String,
        threshold: u16,
        total_participants: u16,
        participant_index: u16,
        group_public_key: String,
    ) -> Self {
        Self::with_participants(
            session_id,
            device_id,
            curve_type,
            threshold,
            total_participants,
            participant_index,
            group_public_key,
            Vec::new(),
        )
    }

    /// Creates wallet metadata including the full participant list from
    /// the DKG ceremony. Cold-start signing reads this back from disk
    /// to reconstruct the session's `participants` / `total` /
    /// `threshold` fields.
    #[allow(clippy::too_many_arguments)]
    pub fn with_participants(
        session_id: String,
        device_id: String,
        curve_type: String,
        threshold: u16,
        total_participants: u16,
        participant_index: u16,
        group_public_key: String,
        participants: Vec<String>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            session_id,
            device_id,
            curve_type,
            threshold,
            total_participants,
            participant_index,
            group_public_key,
            participants,
            created_at: now.clone(),
            last_modified: now,
            label: None,
            // All legacy fields set to None
            device_name: None,
            blockchains: Vec::new(),
            blockchain: None,
            public_address: None,
            identifier: None,
            tags: None,
            description: None,
        }
    }

    /// User-facing name: the optional `label` if set, otherwise the
    /// deterministic `session_id`. Use this everywhere a wallet is shown.
    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.session_id)
    }

}

/// Self-contained wallet file format
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WalletFile {
    /// Format version
    pub version: String,
    
    /// Whether the data is encrypted
    pub encrypted: bool,
    
    /// Encryption algorithm used (e.g., "AES-256-GCM-Argon2id" or "AES-256-GCM-PBKDF2")
    pub algorithm: String,
    
    /// Base64-encoded encrypted data
    pub data: String,
    
    /// Embedded metadata
    pub metadata: WalletMetadata,
}

/// Master index of all wallets and devices (legacy - for migration only)
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct KeystoreIndex {
    /// Keystore format version
    pub version: u8,

    /// List of all wallets
    pub wallets: Vec<WalletInfo>,

    /// List of all devices
    pub devices: Vec<DeviceInfo>,
}

