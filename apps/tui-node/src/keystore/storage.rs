//! Storage functionality for the keystore module.
//!
//! This module provides functions for saving and loading keystore data to disk,
//! including encrypted wallet files and the keystore index.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::{
    KeystoreError, Result,
    encryption::decrypt_data,
    models::{DeviceInfo, KeystoreIndex, WalletFile, WalletMetadata},
};

/// Main keystore interface
pub struct Keystore {
    /// Base path for keystore files
    base_path: PathBuf,

    /// Unique identifier for this device
    device_id: String,
    
    /// Device name for this device
    device_name: String,

    /// Cached wallet metadata for quick access
    wallet_cache: Vec<WalletMetadata>,
}

impl Keystore {

    /// Creates a new keystore at the specified path with the given device name.
    pub fn new(base_path: impl AsRef<Path>, device_name: &str) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        let device_id = device_name.to_string();
        let device_name = device_name.to_string();

        // Create directory structure if it doesn't exist
        fs::create_dir_all(&base_path)?;

        // Create the device-specific wallet directory with curve subdirectories
        let device_wallet_dir = base_path.join(&device_id);
        fs::create_dir_all(&device_wallet_dir)?;
        fs::create_dir_all(device_wallet_dir.join("ed25519"))?;
        fs::create_dir_all(device_wallet_dir.join("secp256k1"))?;

        let mut keystore = Self {
            base_path,
            device_id,
            device_name,
            wallet_cache: Vec::new(),
        };
        
        // Load wallet metadata from existing wallet files
        keystore.reload_wallet_cache()?;
        
        // Migrate legacy files if needed
        keystore.migrate_legacy_files()?;
        
        Ok(keystore)
    }

    /// Reloads the wallet cache by scanning all wallet files
    fn reload_wallet_cache(&mut self) -> Result<()> {
        self.wallet_cache.clear();
        
        let device_dir = self.base_path.join(&self.device_id);
        
        // Scan both curve directories
        for curve_type in &["ed25519", "secp256k1"] {
            let curve_dir = device_dir.join(curve_type);
            if !curve_dir.exists() {
                continue;
            }
            
            // Read all .json files in the directory
            for entry in fs::read_dir(&curve_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    // Try to read the wallet metadata
                    if let Ok(file) = File::open(&path)
                        && let Ok(wallet_file) = serde_json::from_reader::<_, WalletFile>(file) {
                            self.wallet_cache.push(wallet_file.metadata);
                        }
                }
            }
        }
        
        Ok(())
    }

    /// Gets the device ID for this keystore
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Lists all wallets from the cache
    pub fn list_wallets(&self) -> Vec<&WalletMetadata> {
        self.wallet_cache.iter().collect()
    }

    /// Gets wallet metadata by ID
    pub fn get_wallet(&self, wallet_id: &str) -> Option<&WalletMetadata> {
        self.wallet_cache.iter().find(|w| w.session_id == wallet_id)
    }

    /// Gets this device's info (for compatibility)
    pub fn get_this_device(&self) -> Option<DeviceInfo> {
        Some(DeviceInfo::new(
            self.device_id.clone(),
            self.device_name.clone(),
            format!("device-{}", self.device_id.split('-').next().unwrap_or("unknown")),
        ))
    }

    /// Creates a new wallet in the keystore
    /// Creates a wallet with simplified metadata (KISS principle)
    /// Blockchain addresses are derived from group_public_key + curve_type
    #[allow(clippy::too_many_arguments)]
    pub fn create_wallet_multi_chain(
        &mut self,
        name: &str,
        curve_type: &str,
        _blockchains: Vec<crate::keystore::models::BlockchainInfo>, // Ignored - addresses are derived
        threshold: u16,
        total_participants: u16,
        group_public_key: &str,
        key_share_data: &[u8],
        password: &str,
        _tags: Vec<String>, // Deprecated parameter
        _description: Option<String>, // Deprecated parameter
        participant_index: u16,
        // Full device_id list from the DKG session. Read back by
        // cold-start signing to reconstruct the session metadata.
        // Pass an empty `Vec` on code paths that don't know the
        // list — cold-start signing will degrade gracefully.
        participants: Vec<String>,
        // Optional user-friendly display name. Stored as metadata.label;
        // does NOT affect the wallet_id (which stays session-derived so
        // every participant agrees). `None` → UI falls back to the id.
        label: Option<String>,
    ) -> Result<String> {
        // Use the wallet name as the wallet ID (for session name convention)
        // Sanitize the name to ensure it's a valid filename
        let wallet_id = name.replace("/", "-").replace("\\", "-").replace(":", "-");

        // Check if a wallet with this ID already exists
        if self.get_wallet(&wallet_id).is_some() {
            return Err(KeystoreError::General(format!(
                "Wallet with ID '{}' already exists", wallet_id
            )));
        }

        // Create wallet metadata including the participants list.
        let mut metadata = WalletMetadata::with_participants(
            wallet_id.clone(),
            self.device_id.clone(),
            curve_type.to_string(),
            threshold,
            total_participants,
            participant_index,
            group_public_key.to_string(),
            participants,
        );
        // Attach the optional display label (trimmed; empty → None).
        metadata.label = label
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        // Save the wallet with embedded metadata
        self.save_wallet_file_v2(&wallet_id, key_share_data, password, &metadata)?;

        // Update cache
        self.wallet_cache.push(metadata);

        Ok(wallet_id)
    }

    /// Creates a wallet (legacy single blockchain) - addresses are now derived
    pub fn create_wallet(
        &mut self,
        name: &str,
        curve_type: &str,
        _blockchain: &str, // Ignored - addresses are derived from curve_type
        _public_address: &str, // Ignored - addresses are derived from group_public_key
        threshold: u16,
        total_participants: u16,
        group_public_key: &str,
        key_share_data: &[u8],
        password: &str,
        tags: Vec<String>,
        description: Option<String>,
        participant_index: u16,
    ) -> Result<String> {
        // Just call the simplified version - blockchain info is not stored.
        // Legacy wrapper doesn't know participants; pass empty so cold-start
        // signing degrades gracefully (old behaviour preserved).
        self.create_wallet_multi_chain(
            name,
            curve_type,
            Vec::new(), // No blockchain info needed - it's derived
            threshold,
            total_participants,
            group_public_key,
            key_share_data,
            password,
            tags,
            description,
            participant_index,
            Vec::new(),
            None, // legacy wrapper has no display label
        )
    }

    /// Saves encrypted wallet data to a file with embedded metadata (v2 format)
    fn save_wallet_file_v2(&self, wallet_id: &str, data: &[u8], password: &str, metadata: &WalletMetadata) -> Result<()> {
        self.save_wallet_file_v2_with_method(wallet_id, data, password, metadata, crate::keystore::encryption::KeyDerivation::Pbkdf2)
    }


    /// Saves encrypted wallet data to a file with embedded metadata (v2 format) using specified encryption method
    fn save_wallet_file_v2_with_method(&self, wallet_id: &str, data: &[u8], password: &str, metadata: &WalletMetadata, method: crate::keystore::encryption::KeyDerivation) -> Result<()> {
        // Create device-specific wallet directory with curve type
        let wallet_dir = self.base_path.join(&self.device_id).join(&metadata.curve_type);

        // Create the directory structure if it doesn't exist
        fs::create_dir_all(&wallet_dir)?;

        // Define wallet file path
        let wallet_path = wallet_dir.join(format!("{}.json", wallet_id));

        // Encrypt the wallet data using the specified method
        let encrypted_data = crate::keystore::encryption::encrypt_data_with_method(data, password, method)?;

        // Convert encrypted data to base64 for JSON storage
        use base64::{Engine as _, engine::general_purpose};
        let base64_encrypted = general_purpose::STANDARD.encode(&encrypted_data);
        
        // Create the wallet file with embedded metadata
        let wallet_file = WalletFile {
            version: "2.0".to_string(),
            encrypted: true,
            algorithm: method.algorithm_string().to_string(),
            data: base64_encrypted,
            metadata: metadata.clone(),
        };

        // Write JSON to file with pretty formatting
        let mut file = File::create(wallet_path)?;
        serde_json::to_writer_pretty(&mut file, &wallet_file)
            .map_err(|e| KeystoreError::General(format!("Failed to write wallet JSON: {}", e)))?;

        Ok(())
    }

    /// Loads encrypted wallet data from a file
    pub fn load_wallet_file(&self, wallet_id: &str, password: &str) -> Result<Vec<u8>> {
        // Get wallet metadata to find curve type
        let wallet = self.get_wallet(wallet_id)
            .ok_or_else(|| KeystoreError::WalletNotFound(wallet_id.to_string()))?;
        
        // Device-specific wallet path with curve type
        let wallet_dir = self
            .base_path
            .join(&self.device_id)
            .join(&wallet.curve_type);
            
        let json_path = wallet_dir.join(format!("{}.json", wallet_id));
        
        if !json_path.exists() {
            return Err(KeystoreError::General(format!(
                "Wallet file not found for {}", wallet_id
            )));
        }
        
        // Read JSON format
        let file = File::open(&json_path)
            .map_err(|e| KeystoreError::General(format!("Failed to open wallet file: {}", e)))?;
        
        let wallet_file: WalletFile = serde_json::from_reader(file)
            .map_err(|e| KeystoreError::General(format!("Failed to parse wallet JSON: {}", e)))?;
        
        // Decode from base64
        use base64::{Engine as _, engine::general_purpose};
        let encrypted_data = general_purpose::STANDARD.decode(&wallet_file.data)
            .map_err(|e| KeystoreError::General(format!("Failed to decode base64 data: {}", e)))?;

        // Decrypt the data
        let decrypted_data = decrypt_data(&encrypted_data, password)?;

        Ok(decrypted_data)
    }


    
    /// Migrates legacy files to the new self-contained format
    fn migrate_legacy_files(&mut self) -> Result<()> {
        // Check if legacy index.json exists
        let index_path = self.base_path.join("index.json");
        let device_id_path = self.base_path.join("device_id");
        
        if !index_path.exists() {
            // No legacy files to migrate
            return Ok(());
        }
        
        println!("Found legacy index.json, migrating to new format...");
        
        // Load the legacy index
        let index_file = File::open(&index_path)?;
        let legacy_index: KeystoreIndex = serde_json::from_reader(index_file)
            .map_err(|e| KeystoreError::General(format!("Failed to read legacy index: {}", e)))?;
        
        // Migrate each wallet that belongs to this device
        for wallet_info in &legacy_index.wallets {
            // Check if this device has a share for this wallet
            if wallet_info.devices.iter().any(|d| d.device_id == self.device_id) {
                // Try to find the wallet file
                let wallet_dir = self.base_path.join(&self.device_id).join(&wallet_info.curve_type);
                let json_path = wallet_dir.join(format!("{}.json", wallet_info.wallet_id));
                let dat_path = wallet_dir.join(format!("{}.dat", wallet_info.wallet_id));
                
                if json_path.exists() {
                    // Check if it's already v2 format
                    if let Ok(file) = File::open(&json_path)
                        && let Ok(wallet_file) = serde_json::from_reader::<_, WalletFile>(file)
                            && wallet_file.version == "2.0" {
                                // Already migrated
                                continue;
                            }
                    
                    // Read v1 JSON file
                    let file = File::open(&json_path)?;
                    let v1_json: serde_json::Value = serde_json::from_reader(file)
                        .map_err(|e| KeystoreError::General(format!("Failed to parse v1 JSON: {}", e)))?;
                    
                    // Find participant index for this device
                    let participant_index = wallet_info.devices
                        .iter()
                        .position(|d| d.device_id == self.device_id)
                        .map(|i| i as u16 + 1) // 1-based index
                        .unwrap_or(1);
                    
                    // Create v2 metadata
                    let metadata = WalletMetadata {
                        session_id: wallet_info.wallet_id.clone(),
                        device_id: self.device_id.clone(),
                        label: None, // legacy import: no display label
                        device_name: None, // Deprecated field
                        curve_type: wallet_info.curve_type.clone(),
                        blockchain: wallet_info.blockchain.clone(),
                        public_address: wallet_info.public_address.clone(),
                        blockchains: if !wallet_info.blockchains.is_empty() {
                            wallet_info.blockchains.clone()
                        } else if let (Some(blockchain), Some(address)) = (&wallet_info.blockchain, &wallet_info.public_address) {
                            vec![crate::keystore::BlockchainInfo {
                                blockchain: blockchain.clone(),
                                network: "mainnet".to_string(),
                                chain_id: if blockchain == "ethereum" { Some(1) } else { None },
                                address: address.clone(),
                                address_format: if blockchain == "ethereum" { "EIP-55".to_string() } else { "base58".to_string() },
                                enabled: true,
                                rpc_endpoint: None,
                                metadata: None,
                            }]
                        } else {
                            Vec::new()
                        },
                        threshold: wallet_info.threshold,
                        total_participants: wallet_info.total_participants,
                        participant_index,
                        // Legacy wallet migration can't reconstruct the
                        // original DKG participant list; leave empty so
                        // cold-start signing degrades gracefully.
                        participants: Vec::new(),
                        identifier: None, // Deprecated field
                        group_public_key: wallet_info.group_public_key.clone(),
                        created_at: chrono::DateTime::from_timestamp(wallet_info.created_at as i64, 0)
                            .unwrap_or_default()
                            .to_rfc3339(),
                        last_modified: chrono::Utc::now().to_rfc3339(),
                        tags: None, // Deprecated field
                        description: None, // Deprecated field
                    };
                    
                    // Create v2 wallet file
                    let wallet_file = WalletFile {
                        version: "2.0".to_string(),
                        encrypted: v1_json.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(true),
                        algorithm: v1_json.get("algorithm").and_then(|v| v.as_str()).unwrap_or("AES-256-GCM").to_string(),
                        data: v1_json.get("data").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        metadata,
                    };
                    
                    // Write v2 file
                    let file = File::create(&json_path)?;
                    serde_json::to_writer_pretty(file, &wallet_file)
                        .map_err(|e| KeystoreError::General(format!("Failed to write v2 JSON: {}", e)))?;
                    
                    println!("Migrated wallet {} to v2 format", wallet_info.wallet_id);
                } else if dat_path.exists() {
                    // Convert .dat to v2 JSON
                    let mut file = File::open(&dat_path)?;
                    let mut encrypted_data = Vec::new();
                    file.read_to_end(&mut encrypted_data)?;
                    
                    use base64::{Engine as _, engine::general_purpose};
                    let base64_encrypted = general_purpose::STANDARD.encode(&encrypted_data);
                    
                    // Find participant index
                    let participant_index = wallet_info.devices
                        .iter()
                        .position(|d| d.device_id == self.device_id)
                        .map(|i| i as u16 + 1)
                        .unwrap_or(1);
                    
                    // Create v2 metadata
                    let metadata = WalletMetadata {
                        session_id: wallet_info.wallet_id.clone(),
                        device_id: self.device_id.clone(),
                        label: None, // legacy import: no display label
                        device_name: None, // Deprecated field
                        curve_type: wallet_info.curve_type.clone(),
                        blockchain: wallet_info.blockchain.clone(),
                        public_address: wallet_info.public_address.clone(),
                        blockchains: if !wallet_info.blockchains.is_empty() {
                            wallet_info.blockchains.clone()
                        } else if let (Some(blockchain), Some(address)) = (&wallet_info.blockchain, &wallet_info.public_address) {
                            vec![crate::keystore::BlockchainInfo {
                                blockchain: blockchain.clone(),
                                network: "mainnet".to_string(),
                                chain_id: if blockchain == "ethereum" { Some(1) } else { None },
                                address: address.clone(),
                                address_format: if blockchain == "ethereum" { "EIP-55".to_string() } else { "base58".to_string() },
                                enabled: true,
                                rpc_endpoint: None,
                                metadata: None,
                            }]
                        } else {
                            Vec::new()
                        },
                        threshold: wallet_info.threshold,
                        total_participants: wallet_info.total_participants,
                        participant_index,
                        // Legacy wallet migration can't reconstruct the
                        // original DKG participant list; leave empty so
                        // cold-start signing degrades gracefully.
                        participants: Vec::new(),
                        identifier: None, // Deprecated field
                        group_public_key: wallet_info.group_public_key.clone(),
                        created_at: chrono::DateTime::from_timestamp(wallet_info.created_at as i64, 0)
                            .unwrap_or_default()
                            .to_rfc3339(),
                        last_modified: chrono::Utc::now().to_rfc3339(),
                        tags: None, // Deprecated field
                        description: None, // Deprecated field
                    };
                    
                    // Create v2 wallet file
                    let wallet_file = WalletFile {
                        version: "2.0".to_string(),
                        encrypted: true,
                        algorithm: "AES-256-GCM".to_string(),
                        data: base64_encrypted,
                        metadata,
                    };
                    
                    // Write v2 JSON file
                    let json_file = File::create(&json_path)?;
                    serde_json::to_writer_pretty(json_file, &wallet_file)
                        .map_err(|e| KeystoreError::General(format!("Failed to write v2 JSON: {}", e)))?;
                    
                    // Delete old .dat file
                    fs::remove_file(&dat_path)?;
                    
                    println!("Converted wallet {} from .dat to v2 JSON format", wallet_info.wallet_id);
                }
            }
        }
        
        // After successful migration, rename legacy files (don't delete in case something goes wrong)
        if let Err(_e) = fs::rename(&index_path, self.base_path.join("index.json.legacy")) {
            eprintln!("Warning: Failed to rename legacy index.json: {}", _e);
        }
        
        if device_id_path.exists()
            && let Err(_e) = fs::rename(&device_id_path, self.base_path.join("device_id.legacy")) {
                eprintln!("Warning: Failed to rename legacy device_id file: {}", _e);
            }
        
        // Reload the wallet cache
        self.reload_wallet_cache()?;
        
        println!("Migration to v2 format completed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn wallet_metadata_round_trips_participants_field() {
        // Phase-D cold-start prerequisite: writing a wallet must
        // persist the participants list so a restarted node can
        // reconstruct the session's `participants` / `threshold` /
        // `total` without relying on in-memory state.
        let tmp = TempDir::new().expect("tempdir");
        let mut ks = Keystore::new(tmp.path(), "mpc-1").expect("keystore");
        let wallet_id = ks
            .create_wallet_multi_chain(
                "test-wallet",
                "secp256k1",
                Vec::new(),
                2,
                3,
                "aa".repeat(33).as_str(),
                b"ENCRYPTED-PAYLOAD",
                "password-12345",
                Vec::new(),
                None,
                1,
                vec!["mpc-1".to_string(), "mpc-2".to_string(), "mpc-3".to_string()],
                None, // no display label in this test
            )
            .expect("create_wallet_multi_chain");

        // Construct a fresh Keystore over the same directory to force
        // a reload-from-disk — exactly what cold-start does.
        let ks2 = Keystore::new(tmp.path(), "mpc-1").expect("reload");
        let wallet = ks2
            .get_wallet(&wallet_id)
            .expect("wallet visible after reload");
        assert_eq!(
            wallet.participants,
            vec!["mpc-1".to_string(), "mpc-2".to_string(), "mpc-3".to_string()],
            "participants must round-trip through disk"
        );
        assert_eq!(wallet.threshold, 2);
        assert_eq!(wallet.total_participants, 3);
    }

    #[test]
    fn legacy_wallet_without_participants_deserializes_as_empty() {
        // Pre-field-add wallets wrote JSON without a `participants`
        // field. #[serde(default)] on WalletMetadata::participants
        // must keep them loadable — we just get an empty Vec, and
        // cold-start signing degrades gracefully (warn logged).
        let legacy_json = serde_json::json!({
            "version": "2.0",
            "encrypted": true,
            "algorithm": "AES-256-GCM-PBKDF2",
            "data": "ZmFrZQ==",
            "metadata": {
                "session_id": "legacy-wallet",
                "device_id": "mpc-1",
                "curve_type": "secp256k1",
                "threshold": 2,
                "total_participants": 3,
                "participant_index": 1,
                "group_public_key": "03aa",
                "created_at": "2025-01-01T00:00:00+00:00",
                "last_modified": "2025-01-01T00:00:00+00:00"
                // No `participants` field.
            }
        });
        let file: WalletFile =
            serde_json::from_value(legacy_json).expect("legacy wallet must deserialize");
        assert!(
            file.metadata.participants.is_empty(),
            "missing participants field must default to empty Vec"
        );
    }
}
