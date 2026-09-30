//! Storage functionality for the keystore module.
//!
//! This module provides functions for saving and loading keystore data to disk,
//! including encrypted wallet files and the keystore index.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use starlab_core::ecdsa::{ECDSA_CURVE, EcdsaKeyShare};

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
        fs::create_dir_all(device_wallet_dir.join(ECDSA_CURVE))?;

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

        // Scan every curve directory. The ECDSA share (Ethereum) comes last
        // so `get_wallet(id)` keeps returning a wallet's FROST entry first.
        for curve_type in &["ed25519", "secp256k1", ECDSA_CURVE] {
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
                        && let Ok(wallet_file) = serde_json::from_reader::<_, WalletFile>(file)
                    {
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
            format!(
                "device-{}",
                self.device_id.split('-').next().unwrap_or("unknown")
            ),
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
        _tags: Vec<String>,           // Deprecated parameter
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
                "Wallet with ID '{}' already exists",
                wallet_id
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

    /// Persist a UNIFIED wallet: the SAME `wallet_id` under BOTH `ed25519/` and
    /// `secp256k1/` curve dirs, from one dual-curve DKG ceremony. Unlike
    /// `create_wallet_multi_chain` (which rejects a duplicate wallet_id), this
    /// deliberately writes two curve-scoped files sharing the id — they live in
    /// different curve directories and never collide on disk. The wallet thus
    /// yields Ethereum/Bitcoin (secp256k1) AND Solana/Sui (ed25519) addresses.
    #[allow(clippy::too_many_arguments)]
    pub fn create_wallet_unified(
        &mut self,
        wallet_id: &str,
        threshold: u16,
        total_participants: u16,
        participant_index: u16,
        participants: Vec<String>,
        label: Option<String>,
        password: &str,
        // (curve, group_public_key_hex, key_share_blob) for each curve.
        ed25519_group_public_key: &str,
        ed25519_key_share: &[u8],
        secp256k1_group_public_key: &str,
        secp256k1_key_share: &[u8],
    ) -> Result<String> {
        let wallet_id = wallet_id.replace(['/', '\\', ':'], "-");

        for (curve, group_pk, share) in [
            ("ed25519", ed25519_group_public_key, ed25519_key_share),
            ("secp256k1", secp256k1_group_public_key, secp256k1_key_share),
        ] {
            let mut metadata = WalletMetadata::with_participants(
                wallet_id.clone(),
                self.device_id.clone(),
                curve.to_string(),
                threshold,
                total_participants,
                participant_index,
                group_pk.to_string(),
                participants.clone(),
            );
            metadata.label = label
                .clone()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            // Atomic write — never leaves a curve dir with a half-written share.
            self.save_wallet_file_atomic(&wallet_id, share, password, &metadata)?;
            self.wallet_cache.push(metadata);
        }

        Ok(wallet_id)
    }

    /// Persist a wallet's threshold-ECDSA share (its Ethereum key) as the
    /// curve file `secp256k1-ecdsa/<wallet_id>.json` next to the wallet's
    /// FROST files: same encrypted v2 format, same password. Threshold,
    /// participants and our index come from the share itself.
    pub fn save_ecdsa_share(
        &mut self,
        wallet_id: &str,
        share: &EcdsaKeyShare,
        password: &str,
        label: Option<String>,
    ) -> Result<()> {
        check_wallet_id(wallet_id)?;
        let mut metadata = WalletMetadata::with_participants(
            wallet_id.to_string(),
            self.device_id.clone(),
            ECDSA_CURVE.to_string(),
            share.threshold(),
            share.n(),
            share.index() + 1,
            hex::encode(share.group_public_key()),
            share.participants().to_vec(),
        );
        metadata.label = label
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let plaintext = serde_json::to_vec(share)
            .map_err(|e| KeystoreError::SerializationError(e.to_string()))?;
        self.save_wallet_file_atomic(wallet_id, &plaintext, password, &metadata)?;
        self.wallet_cache
            .retain(|w| !(w.session_id == wallet_id && w.curve_type == ECDSA_CURVE));
        self.wallet_cache.push(metadata);
        Ok(())
    }

    /// Decrypt a wallet's ECDSA share (see [`Self::save_ecdsa_share`]).
    pub fn load_ecdsa_share(&self, wallet_id: &str, password: &str) -> Result<EcdsaKeyShare> {
        let blob = self.load_wallet_file_for_curve(wallet_id, ECDSA_CURVE, password)?;
        serde_json::from_slice(&blob)
            .map_err(|e| KeystoreError::General(format!("Not a valid ECDSA key share: {e}")))
    }

    /// Overwrite an existing wallet's key share + the refresh-affected metadata
    /// (threshold / total / participants / participant_index) **in place** for a
    /// reshare (#45): same `wallet_id`, same `curve_type`, same
    /// `group_public_key` (address) and `label` — only the share and the signer
    /// set change. Written **atomically** (temp file + fsync + rename) so a
    /// crash can never leave the wallet without a valid share.
    #[allow(clippy::too_many_arguments)]
    pub fn update_wallet_share(
        &mut self,
        wallet_id: &str,
        key_share_data: &[u8],
        password: &str,
        threshold: u16,
        total_participants: u16,
        participants: Vec<String>,
        participant_index: u16,
    ) -> Result<()> {
        let existing = self
            .get_wallet(wallet_id)
            .cloned()
            .ok_or_else(|| KeystoreError::WalletNotFound(wallet_id.to_string()))?;

        let mut metadata = WalletMetadata::with_participants(
            wallet_id.to_string(),
            self.device_id.clone(),
            existing.curve_type.clone(),
            threshold,
            total_participants,
            participant_index,
            existing.group_public_key.clone(), // address is preserved by the refresh
            participants,
        );
        metadata.label = existing.label.clone();

        self.save_wallet_file_atomic(wallet_id, key_share_data, password, &metadata)?;

        // Replace the cache entry in place (or insert if somehow absent).
        if let Some(slot) = self
            .wallet_cache
            .iter_mut()
            .find(|w| w.session_id == wallet_id)
        {
            *slot = metadata;
        } else {
            self.wallet_cache.push(metadata);
        }
        Ok(())
    }

    /// Atomic variant of `save_wallet_file_v2`: write to a temp file, fsync,
    /// then rename over the target so the on-disk wallet is always a complete,
    /// valid file (used by the reshare share-swap; never a window with no share).
    fn save_wallet_file_atomic(
        &self,
        wallet_id: &str,
        data: &[u8],
        password: &str,
        metadata: &WalletMetadata,
    ) -> Result<()> {
        let wallet_dir = self
            .base_path
            .join(&self.device_id)
            .join(&metadata.curve_type);
        fs::create_dir_all(&wallet_dir)?;
        let wallet_path = wallet_dir.join(format!("{}.json", wallet_id));
        let tmp_path = wallet_dir.join(format!(".{}.json.tmp", wallet_id));

        let method = crate::keystore::encryption::KeyDerivation::Pbkdf2;
        let encrypted_data =
            crate::keystore::encryption::encrypt_data_with_method(data, password, method)?;
        use base64::{Engine as _, engine::general_purpose};
        let base64_encrypted = general_purpose::STANDARD.encode(&encrypted_data);
        let wallet_file = WalletFile {
            version: "2.0".to_string(),
            encrypted: true,
            algorithm: method.algorithm_string().to_string(),
            data: base64_encrypted,
            metadata: metadata.clone(),
        };

        {
            let mut file = File::create(&tmp_path)?;
            serde_json::to_writer_pretty(&mut file, &wallet_file).map_err(|e| {
                KeystoreError::General(format!("Failed to write wallet JSON: {}", e))
            })?;
            file.sync_all()?; // durable before the rename
        }
        fs::rename(&tmp_path, &wallet_path)?; // atomic on the same filesystem
        Ok(())
    }

    /// Creates a wallet (legacy single blockchain) - addresses are now derived
    pub fn create_wallet(
        &mut self,
        name: &str,
        curve_type: &str,
        _blockchain: &str,     // Ignored - addresses are derived from curve_type
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
    fn save_wallet_file_v2(
        &self,
        wallet_id: &str,
        data: &[u8],
        password: &str,
        metadata: &WalletMetadata,
    ) -> Result<()> {
        self.save_wallet_file_v2_with_method(
            wallet_id,
            data,
            password,
            metadata,
            crate::keystore::encryption::KeyDerivation::Pbkdf2,
        )
    }

    /// Saves encrypted wallet data to a file with embedded metadata (v2 format) using specified encryption method
    fn save_wallet_file_v2_with_method(
        &self,
        wallet_id: &str,
        data: &[u8],
        password: &str,
        metadata: &WalletMetadata,
        method: crate::keystore::encryption::KeyDerivation,
    ) -> Result<()> {
        // Create device-specific wallet directory with curve type
        let wallet_dir = self
            .base_path
            .join(&self.device_id)
            .join(&metadata.curve_type);

        // Create the directory structure if it doesn't exist
        fs::create_dir_all(&wallet_dir)?;

        // Define wallet file path
        let wallet_path = wallet_dir.join(format!("{}.json", wallet_id));

        // Encrypt the wallet data using the specified method
        let encrypted_data =
            crate::keystore::encryption::encrypt_data_with_method(data, password, method)?;

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

    /// Loads encrypted wallet data from a file (first-match curve — see
    /// [`Self::load_wallet_file_for_curve`] for unified wallets that store
    /// the same id under BOTH curve directories).
    pub fn load_wallet_file(&self, wallet_id: &str, password: &str) -> Result<Vec<u8>> {
        let wallet = self
            .get_wallet(wallet_id)
            .ok_or_else(|| KeystoreError::WalletNotFound(wallet_id.to_string()))?;
        let curve = wallet.curve_type.clone();
        self.load_wallet_file_for_curve(wallet_id, &curve, password)
    }

    /// Curve-explicit variant of [`Self::load_wallet_file`]: a unified wallet
    /// has one encrypted share per curve under the same id; callers that care
    /// which curve they get (HD derivation, reshare) must say so.
    pub fn load_wallet_file_for_curve(
        &self,
        wallet_id: &str,
        curve_type: &str,
        password: &str,
    ) -> Result<Vec<u8>> {
        // Device-specific wallet path with curve type
        let wallet_dir = self.base_path.join(&self.device_id).join(curve_type);

        let json_path = wallet_dir.join(format!("{}.json", wallet_id));

        if !json_path.exists() {
            return Err(KeystoreError::General(format!(
                "Wallet file not found for {}",
                wallet_id
            )));
        }

        // Read JSON format
        let file = File::open(&json_path)
            .map_err(|e| KeystoreError::General(format!("Failed to open wallet file: {}", e)))?;

        let wallet_file: WalletFile = serde_json::from_reader(file)
            .map_err(|e| KeystoreError::General(format!("Failed to parse wallet JSON: {}", e)))?;

        // Decode from base64
        use base64::{Engine as _, engine::general_purpose};
        let encrypted_data = general_purpose::STANDARD
            .decode(&wallet_file.data)
            .map_err(|e| KeystoreError::General(format!("Failed to decode base64 data: {}", e)))?;

        // Decrypt the data
        let decrypted_data = decrypt_data(&encrypted_data, password)?;

        Ok(decrypted_data)
    }

    /// The directory this keystore reads and writes.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Export one key share as a portable file: the same v2 `WalletFile`
    /// JSON that lives on disk (still encrypted with the wallet password),
    /// so a share file copied by hand imports too. `password` must unlock
    /// it and the payload must be a valid FROST share for `curve_type` —
    /// we never hand out a file that would fail to import.
    pub fn export_share(
        &self,
        wallet_id: &str,
        curve_type: &str,
        password: &str,
    ) -> Result<String> {
        check_wallet_id(wallet_id)?;
        let path = self
            .base_path
            .join(&self.device_id)
            .join(curve_type)
            .join(format!("{wallet_id}.json"));
        let file = File::open(&path)
            .map_err(|_| KeystoreError::WalletNotFound(format!("{wallet_id} ({curve_type})")))?;
        let wallet_file: WalletFile = serde_json::from_reader(file)
            .map_err(|e| KeystoreError::General(format!("Failed to parse wallet JSON: {e}")))?;
        let blob = decrypt_wallet_file(&wallet_file, password)?;
        validate_share(&wallet_file.metadata, &blob)?;
        serde_json::to_string_pretty(&wallet_file)
            .map_err(|e| KeystoreError::SerializationError(e.to_string()))
    }

    /// Import a share file produced by [`Self::export_share`] (or copied from
    /// another keystore). The password must unlock it and the payload must
    /// be a FROST share for its curve whose group key matches the metadata.
    /// The file is stored as-is (same encryption) under this keystore's
    /// device directory. An existing wallet with the same id + curve is
    /// never overwritten.
    pub fn import_share(&mut self, file_json: &[u8], password: &str) -> Result<WalletMetadata> {
        let wallet_file: WalletFile = serde_json::from_slice(file_json)
            .map_err(|e| KeystoreError::General(format!("Not a Starlab key share file: {e}")))?;
        if wallet_file.version != "2.0" || !wallet_file.encrypted {
            return Err(KeystoreError::General(format!(
                "Unsupported key share file (version {}, encrypted {}); expected an encrypted v2.0 file",
                wallet_file.version, wallet_file.encrypted
            )));
        }
        let metadata = wallet_file.metadata.clone();
        check_wallet_id(&metadata.session_id)?;
        let blob = decrypt_wallet_file(&wallet_file, password)?;
        validate_share(&metadata, &blob)?;

        let wallet_dir = self
            .base_path
            .join(&self.device_id)
            .join(&metadata.curve_type);
        let target = wallet_dir.join(format!("{}.json", metadata.session_id));
        if target.exists() {
            return Err(KeystoreError::General(format!(
                "Wallet {} ({}) is already in keystore",
                metadata.session_id, metadata.curve_type
            )));
        }
        fs::create_dir_all(&wallet_dir)?;
        let mut file = File::create(&target)?;
        serde_json::to_writer_pretty(&mut file, &wallet_file)
            .map_err(|e| KeystoreError::General(format!("Failed to write wallet JSON: {e}")))?;
        self.reload_wallet_cache()?;
        Ok(metadata)
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
            if wallet_info
                .devices
                .iter()
                .any(|d| d.device_id == self.device_id)
            {
                // Try to find the wallet file
                let wallet_dir = self
                    .base_path
                    .join(&self.device_id)
                    .join(&wallet_info.curve_type);
                let json_path = wallet_dir.join(format!("{}.json", wallet_info.wallet_id));
                let dat_path = wallet_dir.join(format!("{}.dat", wallet_info.wallet_id));

                if json_path.exists() {
                    // Check if it's already v2 format
                    if let Ok(file) = File::open(&json_path)
                        && let Ok(wallet_file) = serde_json::from_reader::<_, WalletFile>(file)
                        && wallet_file.version == "2.0"
                    {
                        // Already migrated
                        continue;
                    }

                    // Read v1 JSON file
                    let file = File::open(&json_path)?;
                    let v1_json: serde_json::Value =
                        serde_json::from_reader(file).map_err(|e| {
                            KeystoreError::General(format!("Failed to parse v1 JSON: {}", e))
                        })?;

                    // Find participant index for this device
                    let participant_index = wallet_info
                        .devices
                        .iter()
                        .position(|d| d.device_id == self.device_id)
                        .map(|i| i as u16 + 1) // 1-based index
                        .unwrap_or(1);

                    // Create v2 metadata
                    let metadata = WalletMetadata {
                        session_id: wallet_info.wallet_id.clone(),
                        device_id: self.device_id.clone(),
                        label: None,       // legacy import: no display label
                        device_name: None, // Deprecated field
                        curve_type: wallet_info.curve_type.clone(),
                        blockchain: wallet_info.blockchain.clone(),
                        public_address: wallet_info.public_address.clone(),
                        blockchains: if !wallet_info.blockchains.is_empty() {
                            wallet_info.blockchains.clone()
                        } else if let (Some(blockchain), Some(address)) =
                            (&wallet_info.blockchain, &wallet_info.public_address)
                        {
                            vec![crate::keystore::BlockchainInfo {
                                blockchain: blockchain.clone(),
                                network: "mainnet".to_string(),
                                chain_id: if blockchain == "ethereum" {
                                    Some(1)
                                } else {
                                    None
                                },
                                address: address.clone(),
                                address_format: if blockchain == "ethereum" {
                                    "EIP-55".to_string()
                                } else {
                                    "base58".to_string()
                                },
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
                        created_at: chrono::DateTime::from_timestamp(
                            wallet_info.created_at as i64,
                            0,
                        )
                        .unwrap_or_default()
                        .to_rfc3339(),
                        last_modified: chrono::Utc::now().to_rfc3339(),
                        tags: None,        // Deprecated field
                        description: None, // Deprecated field
                    };

                    // Create v2 wallet file
                    let wallet_file = WalletFile {
                        version: "2.0".to_string(),
                        encrypted: v1_json
                            .get("encrypted")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        algorithm: v1_json
                            .get("algorithm")
                            .and_then(|v| v.as_str())
                            .unwrap_or("AES-256-GCM")
                            .to_string(),
                        data: v1_json
                            .get("data")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        metadata,
                    };

                    // Write v2 file
                    let file = File::create(&json_path)?;
                    serde_json::to_writer_pretty(file, &wallet_file).map_err(|e| {
                        KeystoreError::General(format!("Failed to write v2 JSON: {}", e))
                    })?;

                    println!("Migrated wallet {} to v2 format", wallet_info.wallet_id);
                } else if dat_path.exists() {
                    // Convert .dat to v2 JSON
                    let mut file = File::open(&dat_path)?;
                    let mut encrypted_data = Vec::new();
                    file.read_to_end(&mut encrypted_data)?;

                    use base64::{Engine as _, engine::general_purpose};
                    let base64_encrypted = general_purpose::STANDARD.encode(&encrypted_data);

                    // Find participant index
                    let participant_index = wallet_info
                        .devices
                        .iter()
                        .position(|d| d.device_id == self.device_id)
                        .map(|i| i as u16 + 1)
                        .unwrap_or(1);

                    // Create v2 metadata
                    let metadata = WalletMetadata {
                        session_id: wallet_info.wallet_id.clone(),
                        device_id: self.device_id.clone(),
                        label: None,       // legacy import: no display label
                        device_name: None, // Deprecated field
                        curve_type: wallet_info.curve_type.clone(),
                        blockchain: wallet_info.blockchain.clone(),
                        public_address: wallet_info.public_address.clone(),
                        blockchains: if !wallet_info.blockchains.is_empty() {
                            wallet_info.blockchains.clone()
                        } else if let (Some(blockchain), Some(address)) =
                            (&wallet_info.blockchain, &wallet_info.public_address)
                        {
                            vec![crate::keystore::BlockchainInfo {
                                blockchain: blockchain.clone(),
                                network: "mainnet".to_string(),
                                chain_id: if blockchain == "ethereum" {
                                    Some(1)
                                } else {
                                    None
                                },
                                address: address.clone(),
                                address_format: if blockchain == "ethereum" {
                                    "EIP-55".to_string()
                                } else {
                                    "base58".to_string()
                                },
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
                        created_at: chrono::DateTime::from_timestamp(
                            wallet_info.created_at as i64,
                            0,
                        )
                        .unwrap_or_default()
                        .to_rfc3339(),
                        last_modified: chrono::Utc::now().to_rfc3339(),
                        tags: None,        // Deprecated field
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
                    serde_json::to_writer_pretty(json_file, &wallet_file).map_err(|e| {
                        KeystoreError::General(format!("Failed to write v2 JSON: {}", e))
                    })?;

                    // Delete old .dat file
                    fs::remove_file(&dat_path)?;

                    println!(
                        "Converted wallet {} from .dat to v2 JSON format",
                        wallet_info.wallet_id
                    );
                }
            }
        }

        // After successful migration, rename legacy files (don't delete in case something goes wrong)
        if let Err(_e) = fs::rename(&index_path, self.base_path.join("index.json.legacy")) {
            eprintln!("Warning: Failed to rename legacy index.json: {}", _e);
        }

        if device_id_path.exists()
            && let Err(_e) = fs::rename(&device_id_path, self.base_path.join("device_id.legacy"))
        {
            eprintln!("Warning: Failed to rename legacy device_id file: {}", _e);
        }

        // Reload the wallet cache
        self.reload_wallet_cache()?;

        println!("Migration to v2 format completed successfully");
        Ok(())
    }
}

/// Wallet ids become file names — refuse anything that could escape the
/// curve directory.
fn check_wallet_id(wallet_id: &str) -> Result<()> {
    if wallet_id.is_empty()
        || wallet_id.contains('/')
        || wallet_id.contains('\\')
        || wallet_id.contains("..")
    {
        return Err(KeystoreError::General(format!(
            "Invalid wallet id {wallet_id:?}"
        )));
    }
    Ok(())
}

/// Decrypt a `WalletFile` payload. A wrong password surfaces as
/// [`KeystoreError::InvalidPassword`].
fn decrypt_wallet_file(wallet_file: &WalletFile, password: &str) -> Result<Vec<u8>> {
    use base64::{Engine as _, engine::general_purpose};
    let encrypted = general_purpose::STANDARD
        .decode(&wallet_file.data)
        .map_err(|e| KeystoreError::General(format!("Failed to decode base64 data: {e}")))?;
    decrypt_data(&encrypted, password).map_err(|e| match e {
        KeystoreError::DecryptionError(_) => KeystoreError::InvalidPassword,
        other => other,
    })
}

/// frost's binary package header is `[version: u8][crc32(ciphersuite id): 4
/// bytes BE]`; this is the short id of the pre-Taproot (vanilla)
/// `FROST-secp256k1-SHA256-v1` suite. A keystore blob is
/// `[kp_len: u32][KeyPackage][pkp_len: u32][PublicKeyPackage]`, so the
/// KeyPackage's suite id sits at bytes 5..9.
const VANILLA_SECP256K1_SHORT_ID: [u8; 4] = [0xee, 0xd6, 0xb1, 0xb1];
const BLOB_SUITE_ID: std::ops::Range<usize> = 5..9;

/// The decrypted payload must be a FROST share for the metadata's curve,
/// internally consistent, and carry the metadata's group key.
fn validate_share(metadata: &WalletMetadata, blob: &[u8]) -> Result<()> {
    let group_key = match metadata.curve_type.as_str() {
        "secp256k1" => {
            if blob.get(BLOB_SUITE_ID) == Some(VANILLA_SECP256K1_SHORT_ID.as_slice()) {
                return Err(KeystoreError::General(
                    "Not a BIP-340/Taproot secp256k1 FROST share (pre-Taproot vanilla \
                     secp256k1 shares are not supported)"
                        .to_string(),
                ));
            }
            share_group_key::<frost_secp256k1_tr::Secp256K1Sha256TR>(blob)?
        }
        "ed25519" => share_group_key::<frost_ed25519::Ed25519Sha512>(blob)?,
        ECDSA_CURVE => {
            let share: EcdsaKeyShare = serde_json::from_slice(blob)
                .map_err(|e| KeystoreError::General(format!("Not a valid ECDSA key share: {e}")))?;
            hex::encode(share.group_public_key())
        }
        other => {
            return Err(KeystoreError::General(format!(
                "Unsupported curve {other:?} (expected secp256k1, ed25519 or {ECDSA_CURVE})"
            )));
        }
    };
    if !group_key.eq_ignore_ascii_case(&metadata.group_public_key) {
        return Err(KeystoreError::General(
            "Key share does not match the wallet's group public key".to_string(),
        ));
    }
    Ok(())
}

/// Decode a `(KeyPackage, PublicKeyPackage)` blob and return its group key
/// (hex), checking both halves agree.
fn share_group_key<C: frost_core::Ciphersuite>(blob: &[u8]) -> Result<String> {
    let (key_package, public_key_package) = crate::elm::command::decode_keystore_blob::<C>(blob)
        .map_err(|e| {
            KeystoreError::General(format!("Not a valid {} FROST key share: {e}", C::ID))
        })?;
    if key_package.verifying_key() != public_key_package.verifying_key() {
        return Err(KeystoreError::General(
            "Key share is inconsistent: KeyPackage and PublicKeyPackage disagree on the group key"
                .to_string(),
        ));
    }
    let bytes = public_key_package
        .verifying_key()
        .serialize()
        .map_err(|e| KeystoreError::SerializationError(format!("{e:?}")))?;
    Ok(hex::encode(bytes))
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
                vec![
                    "mpc-1".to_string(),
                    "mpc-2".to_string(),
                    "mpc-3".to_string(),
                ],
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
            vec![
                "mpc-1".to_string(),
                "mpc-2".to_string(),
                "mpc-3".to_string()
            ],
            "participants must round-trip through disk"
        );
        assert_eq!(wallet.threshold, 2);
        assert_eq!(wallet.total_participants, 3);
    }

    #[test]
    fn update_wallet_share_swaps_share_and_metadata_preserving_address() {
        // Reshare (#45): overwrite the share + signer set in place, keep the
        // group key (address). 2-of-3 → remove a device → 2-of-2, new share.
        let tmp = TempDir::new().expect("tempdir");
        let mut ks = Keystore::new(tmp.path(), "mpc-1").expect("keystore");
        let group_key = "bb".repeat(33);
        let wallet_id = ks
            .create_wallet_multi_chain(
                "resh-wallet",
                "secp256k1",
                Vec::new(),
                2,
                3,
                group_key.as_str(),
                b"OLD-SHARE",
                "pw-123456789012",
                Vec::new(),
                None,
                1,
                vec!["mpc-1".into(), "mpc-2".into(), "mpc-3".into()],
                Some("My Wallet".into()),
            )
            .expect("create");

        ks.update_wallet_share(
            &wallet_id,
            b"NEW-REFRESHED-SHARE",
            "pw-123456789012",
            2,
            2,
            vec!["mpc-1".into(), "mpc-3".into()],
            1,
        )
        .expect("update_wallet_share");

        // Reload from disk → fresh Keystore.
        let ks2 = Keystore::new(tmp.path(), "mpc-1").expect("reload");
        let w = ks2.get_wallet(&wallet_id).expect("wallet after reshare");
        assert_eq!(
            w.group_public_key, group_key,
            "address (group key) preserved"
        );
        assert_eq!(w.total_participants, 2, "signer set shrank");
        assert_eq!(
            w.participants,
            vec!["mpc-1".to_string(), "mpc-3".to_string()]
        );
        assert_eq!(w.label.as_deref(), Some("My Wallet"), "label preserved");
        // The refreshed share bytes are what load now.
        let loaded = ks2
            .load_wallet_file(&wallet_id, "pw-123456789012")
            .expect("decrypt");
        assert_eq!(loaded, b"NEW-REFRESHED-SHARE");
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

/// Export → import of real FROST shares between keystores.
#[cfg(test)]
mod share_transfer_tests {
    use super::*;
    use crate::elm::command::{decode_keystore_blob, encode_keystore_blob};
    use frost_ed25519::Ed25519Sha512 as Ed;
    use frost_secp256k1_tr::Secp256K1Sha256TR as Secp;
    use starlab_core::resharing::dkg_keypackages;
    use tempfile::TempDir;

    const PW: &str = "correct-horse-battery";

    /// Keystore A (device-a) holding participant 1's share of a fresh
    /// 2-of-3 wallet. Returns the dir guard, the keystore, the wallet id,
    /// all key packages and the public key package.
    fn keystore_with_share<C: frost_core::Ciphersuite>(
        curve: &str,
    ) -> (
        TempDir,
        Keystore,
        String,
        std::collections::BTreeMap<u16, frost_core::keys::KeyPackage<C>>,
        frost_core::keys::PublicKeyPackage<C>,
    ) {
        let dir = TempDir::new().unwrap();
        let mut ks = Keystore::new(dir.path(), "device-a").unwrap();
        let (kps, pp) = dkg_keypackages::<C>(3, 2, 42).unwrap();
        let blob = encode_keystore_blob::<C>(&kps[&1], &pp).unwrap();
        let group_hex = hex::encode(pp.verifying_key().serialize().unwrap());
        let id = ks
            .create_wallet_multi_chain(
                "w-transfer",
                curve,
                vec![],
                2,
                3,
                &group_hex,
                &blob,
                PW,
                vec![],
                None,
                1,
                vec!["device-a".into(), "device-b".into(), "device-c".into()],
                Some("Savings".into()),
            )
            .unwrap();
        (dir, ks, id, kps, pp)
    }

    fn account0(curve: &str, group_hex: &str) -> Vec<(String, String, String)> {
        starlab_core::accounts::account_addresses(curve, &hex::decode(group_hex).unwrap(), 0)
            .unwrap()
    }

    #[test]
    fn secp256k1_share_round_trips_and_still_signs() {
        use frost_secp256k1_tr as frost;
        let (_a, ks_a, id, kps, pp) = keystore_with_share::<Secp>("secp256k1");
        let exported = ks_a.export_share(&id, "secp256k1", PW).unwrap();

        let dir_b = TempDir::new().unwrap();
        let mut ks_b = Keystore::new(dir_b.path(), "device-restore").unwrap();
        let meta = ks_b.import_share(exported.as_bytes(), PW).unwrap();

        let original = ks_a.get_wallet(&id).unwrap();
        assert_eq!(meta.session_id, id);
        assert_eq!(meta.group_public_key, original.group_public_key);
        assert_eq!((meta.threshold, meta.total_participants), (2, 3));
        assert_eq!(meta.display_name(), "Savings");
        assert_eq!(
            account0("secp256k1", &meta.group_public_key),
            account0("secp256k1", &original.group_public_key)
        );
        assert!(ks_b.get_wallet(&id).is_some(), "cache reloaded");

        // The imported share + another participant's share sign 2-of-3.
        let blob = ks_b.load_wallet_file(&id, PW).unwrap();
        let (imported, imported_pp) = decode_keystore_blob::<Secp>(&blob).unwrap();
        assert_eq!(imported_pp.verifying_key(), pp.verifying_key());
        let signers = [imported, kps[&2].clone()];
        let mut rng = starlab_core::rng::os_rng();
        let msg = b"import/export round trip";
        let mut nonces = std::collections::BTreeMap::new();
        let mut commitments = std::collections::BTreeMap::new();
        for kp in &signers {
            let (n, c) = frost::round1::commit(kp.signing_share(), &mut rng);
            nonces.insert(*kp.identifier(), n);
            commitments.insert(*kp.identifier(), c);
        }
        let package = frost::SigningPackage::new(commitments, msg);
        let mut shares = std::collections::BTreeMap::new();
        for kp in &signers {
            let share = frost::round2::sign(&package, &nonces[kp.identifier()], kp).unwrap();
            shares.insert(*kp.identifier(), share);
        }
        let signature = frost::aggregate(&package, &shares, &imported_pp).unwrap();
        imported_pp
            .verifying_key()
            .verify(msg, &signature)
            .expect("signature from the imported share verifies");
    }

    #[test]
    fn ed25519_share_round_trips() {
        let (_a, ks_a, id, _kps, pp) = keystore_with_share::<Ed>("ed25519");
        let exported = ks_a.export_share(&id, "ed25519", PW).unwrap();
        let dir_b = TempDir::new().unwrap();
        let mut ks_b = Keystore::new(dir_b.path(), "device-restore").unwrap();
        let meta = ks_b.import_share(exported.as_bytes(), PW).unwrap();
        assert_eq!(
            meta.group_public_key,
            hex::encode(pp.verifying_key().serialize().unwrap())
        );
        let blob = ks_b.load_wallet_file(&id, PW).unwrap();
        let (_, pp_b) = decode_keystore_blob::<Ed>(&blob).unwrap();
        assert_eq!(pp_b.verifying_key(), pp.verifying_key());
    }

    #[test]
    fn wrong_password_is_rejected_on_export_and_import() {
        let (_a, ks_a, id, _, _) = keystore_with_share::<Secp>("secp256k1");
        assert!(matches!(
            ks_a.export_share(&id, "secp256k1", "wrong-password"),
            Err(KeystoreError::InvalidPassword)
        ));
        let exported = ks_a.export_share(&id, "secp256k1", PW).unwrap();
        let dir_b = TempDir::new().unwrap();
        let mut ks_b = Keystore::new(dir_b.path(), "device-restore").unwrap();
        let err = ks_b
            .import_share(exported.as_bytes(), "wrong-password")
            .unwrap_err();
        assert!(matches!(err, KeystoreError::InvalidPassword));
        assert_eq!(err.to_string(), "Invalid password");
        assert!(ks_b.list_wallets().is_empty());
    }

    #[test]
    fn duplicate_import_is_refused() {
        let (_a, mut ks_a, id, _, _) = keystore_with_share::<Secp>("secp256k1");
        let exported = ks_a.export_share(&id, "secp256k1", PW).unwrap();
        let err = ks_a.import_share(exported.as_bytes(), PW).unwrap_err();
        assert!(err.to_string().contains("already in keystore"), "{err}");
    }

    /// Replace the frost header suite id (bytes 1..5 of every package) in a
    /// `[len][kp][len][pkp]` blob.
    fn rewrite_suite_id(blob: &[u8], from: [u8; 4], to: [u8; 4]) -> Vec<u8> {
        let mut out = blob.to_vec();
        let mut pos = 0;
        while pos < out.len() {
            let len = u32::from_le_bytes(out[pos..pos + 4].try_into().unwrap()) as usize;
            let id = pos + 5..pos + 9;
            assert_eq!(out[id.clone()], from, "suite id at the package header");
            out[id].copy_from_slice(&to);
            pos += 4 + len;
        }
        out
    }

    #[test]
    fn vanilla_secp256k1_share_is_rejected_clearly() {
        let (_a, ks_a, id, kps, pp) = keystore_with_share::<Secp>("secp256k1");
        let tr_blob = encode_keystore_blob::<Secp>(&kps[&1], &pp).unwrap();
        // crc32("FROST-secp256k1-SHA256-TR-v1") — the header layout this
        // detection relies on.
        assert_eq!(tr_blob[BLOB_SUITE_ID], [0x23, 0x0f, 0x8a, 0xb3]);
        let vanilla_blob = rewrite_suite_id(
            &tr_blob,
            [0x23, 0x0f, 0x8a, 0xb3],
            VANILLA_SECP256K1_SHORT_ID,
        );
        let exported = ks_a.export_share(&id, "secp256k1", PW).unwrap();
        let mut file: WalletFile = serde_json::from_str(&exported).unwrap();
        use base64::{Engine as _, engine::general_purpose};
        file.data = general_purpose::STANDARD.encode(
            crate::keystore::encryption::encrypt_data_with_method(
                &vanilla_blob,
                PW,
                crate::keystore::encryption::KeyDerivation::Pbkdf2,
            )
            .unwrap(),
        );

        let dir_b = TempDir::new().unwrap();
        let mut ks_b = Keystore::new(dir_b.path(), "device-restore").unwrap();
        let err = ks_b
            .import_share(serde_json::to_string(&file).unwrap().as_bytes(), PW)
            .unwrap_err();
        assert!(err.to_string().contains("BIP-340/Taproot"), "{err}");
        assert!(ks_b.list_wallets().is_empty());
    }

    #[test]
    fn non_share_files_are_rejected_clearly() {
        let dir = TempDir::new().unwrap();
        let mut ks = Keystore::new(dir.path(), "d").unwrap();
        let err = ks.import_share(b"{\"hello\":1}", PW).unwrap_err();
        assert!(
            err.to_string().contains("Not a Starlab key share file"),
            "{err}"
        );
    }
}
