//! Chrome extension compatibility module.
//!
//! Import/export between the TUI's on-disk keystore format and the Chrome
//! extension's keystore JSON, enabling seamless interoperability.

use super::{KeystoreError, Result, DeviceInfo};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;

/// Key share data format compatible with Chrome extension
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionKeyShareData {
    // Core FROST key material
    pub key_package: String,           // Serialized FROST KeyPackage (base64)
    pub public_key_package: String,    // Serialized PublicKeyPackage (base64)
    pub group_public_key: String,      // The group's public key (hex)
    
    // Session information
    pub session_id: String,
    pub device_id: String,
    pub participant_index: u16,
    
    // Threshold configuration
    pub threshold: u16,
    pub total_participants: u16,
    pub participants: Vec<String>,
    
    // Blockchain specific
    pub curve: String,  // "secp256k1" or "ed25519"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethereum_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solana_address: Option<String>,
    
    // Metadata
    pub created_at: i64,  // Unix timestamp in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_date: Option<i64>,
}

/// Encrypted key share format for Chrome extension
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionEncryptedKeyShare {
    pub wallet_id: String,
    pub algorithm: String,  // "AES-256-GCM-PBKDF2" for browser compatibility
    pub salt: String,       // base64
    pub iv: String,         // base64
    pub ciphertext: String, // base64
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_tag: Option<String>, // base64
}

/// Keystore backup format for Chrome extension
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionKeystoreBackup {
    pub version: String,
    pub device_id: String,
    pub exported_at: i64,
    pub wallets: Vec<ExtensionBackupWallet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionBackupWallet {
    pub metadata: ExtensionWalletMetadata,
    pub encrypted_share: ExtensionEncryptedKeyShare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionWalletMetadata {
    pub id: String,
    pub name: String,
    pub blockchain: String,
    pub address: String,
    pub session_id: String,
    pub is_active: bool,
    pub has_backup: bool,
}

// Use shared frost-core library for keystore functionality
use mpc_wallet_frost_core::{
    keystore::KeystoreData as FrostKeystoreData,
};
use frost_secp256k1::keys::{KeyPackage as Secp256k1KeyPackage, PublicKeyPackage as Secp256k1PublicKeyPackage};
use frost_ed25519::keys::{KeyPackage as Ed25519KeyPackage, PublicKeyPackage as Ed25519PublicKeyPackage};

/// CLI wallet data structure for storing FROST key material
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletData {
    /// FROST key package for secp256k1 (Ethereum)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secp256k1_key_package: Option<Secp256k1KeyPackage>,
    
    /// Public key package for secp256k1 (Ethereum)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secp256k1_public_key: Option<Secp256k1PublicKeyPackage>,
    
    /// FROST key package for ed25519 (Solana)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ed25519_key_package: Option<Ed25519KeyPackage>,
    
    /// Public key package for ed25519 (Solana)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ed25519_public_key: Option<Ed25519PublicKeyPackage>,
    
    /// Session ID for this wallet
    pub session_id: String,
    
    /// Device ID that owns this key share
    pub device_id: String,
}

impl ExtensionKeyShareData {
    /// Convert from shared frost-core keystore format
    pub fn from_frost_keystore(
        keystore_data: &FrostKeystoreData,
        wallet_info: &super::models::WalletInfo,
        device_info: &DeviceInfo,
        _blockchain: &str,
    ) -> Result<Self> {
        // Get participant info from keystore data
        let participant_index = keystore_data.participant_index;
        let participants = wallet_info.devices.iter()
            .map(|d| d.device_id.clone())
            .collect();
        
        // Get blockchain-specific addresses
        let (ethereum_address, solana_address) = match keystore_data.curve.as_str() {
            "secp256k1" => (
                Some(wallet_info.get_blockchain("ethereum")
                    .map(|b| b.address.clone())
                    .unwrap_or_else(|| "N/A".to_string())),
                None
            ),
            "ed25519" => (
                None,
                Some(wallet_info.get_blockchain("solana")
                    .map(|b| b.address.clone())
                    .unwrap_or_else(|| "N/A".to_string()))
            ),
            _ => (None, None),
        };
        
        Ok(Self {
            key_package: keystore_data.key_package.clone(),
            public_key_package: keystore_data.public_key_package.clone(),
            group_public_key: wallet_info.group_public_key.clone(),
            session_id: keystore_data.session_id.clone()
                .unwrap_or_else(|| wallet_info.wallet_id.clone()),
            device_id: keystore_data.device_id.clone()
                .unwrap_or_else(|| device_info.device_id.clone()),
            participant_index,
            threshold: keystore_data.min_signers,
            total_participants: keystore_data.max_signers,
            participants,
            curve: keystore_data.curve.clone(),
            ethereum_address,
            solana_address,
            created_at: Utc::now().timestamp_millis(),
            last_used: None,
            backup_date: None,
        })
    }
    
    /// Convert CLI wallet data to extension format (legacy)
    pub fn from_cli_wallet(
        wallet_data: &WalletData,
        wallet_info: &super::models::WalletInfo,
        device_info: &DeviceInfo,
        blockchain: &str,
    ) -> Result<Self> {
        // Find participant index
        let participant_index = wallet_info.devices
            .iter()
            .position(|d| d.device_id == device_info.device_id)
            .ok_or_else(|| KeystoreError::General("Device not found in participants".into()))?;
        
        // Serialize key packages based on blockchain type
        let (key_package_base64, public_key_base64, curve, ethereum_address, solana_address) = 
            match blockchain {
                "ethereum" => {
                    let key_package = wallet_data.secp256k1_key_package.as_ref()
                        .ok_or_else(|| KeystoreError::General("Missing secp256k1 key package".into()))?;
                    let public_key = wallet_data.secp256k1_public_key.as_ref()
                        .ok_or_else(|| KeystoreError::General("Missing secp256k1 public key".into()))?;
                    
                    let key_bytes = serde_json::to_vec(key_package)
                        .map_err(|e| KeystoreError::SerializationError(e.to_string()))?;
                    let pub_bytes = serde_json::to_vec(public_key)
                        .map_err(|e| KeystoreError::SerializationError(e.to_string()))?;
                    
                    (
                        general_purpose::STANDARD.encode(&key_bytes),
                        general_purpose::STANDARD.encode(&pub_bytes),
                        "secp256k1",
                        Some(wallet_info.get_blockchain("ethereum")
                            .map(|b| b.address.clone())
                            .unwrap_or_else(|| "N/A".to_string())),
                        None
                    )
                },
                "solana" => {
                    let key_package = wallet_data.ed25519_key_package.as_ref()
                        .ok_or_else(|| KeystoreError::General("Missing ed25519 key package".into()))?;
                    let public_key = wallet_data.ed25519_public_key.as_ref()
                        .ok_or_else(|| KeystoreError::General("Missing ed25519 public key".into()))?;
                    
                    let key_bytes = serde_json::to_vec(key_package)
                        .map_err(|e| KeystoreError::SerializationError(e.to_string()))?;
                    let pub_bytes = serde_json::to_vec(public_key)
                        .map_err(|e| KeystoreError::SerializationError(e.to_string()))?;
                    
                    (
                        general_purpose::STANDARD.encode(&key_bytes),
                        general_purpose::STANDARD.encode(&pub_bytes),
                        "ed25519",
                        None,
                        Some(wallet_info.get_blockchain("solana")
                            .map(|b| b.address.clone())
                            .unwrap_or_else(|| "N/A".to_string()))
                    )
                },
                _ => return Err(KeystoreError::General("Unknown blockchain".into())),
            };
        
        // Get group public key hex - use the provided wallet_info address
        let primary_blockchain = wallet_info.primary_blockchain()
            .ok_or_else(|| KeystoreError::General("No enabled blockchain found".into()))?;
        let group_public_key = primary_blockchain.address.clone();
        
        Ok(ExtensionKeyShareData {
            key_package: key_package_base64,
            public_key_package: public_key_base64,
            group_public_key,
            session_id: wallet_data.session_id.clone(),
            device_id: wallet_data.device_id.clone(),
            participant_index: (participant_index + 1) as u16, // 1-based in extension
            threshold: wallet_info.threshold,
            total_participants: wallet_info.total_participants,
            participants: wallet_info.devices.iter().map(|d| d.device_id.clone()).collect(),
            curve: curve.to_string(),
            ethereum_address,
            solana_address,
            created_at: (wallet_info.created_at * 1000) as i64,
            last_used: None, // TODO: Add last_used tracking
            backup_date: Some(Utc::now().timestamp_millis()),
        })
    }
    
    /// Convert CLI wallet data to extension format using metadata
    pub fn from_cli_wallet_metadata(
        wallet_data: &WalletData,
        wallet_metadata: &super::models::WalletMetadata,
        _device_info: &DeviceInfo,
    ) -> Result<Self> {
        // Use participant index from metadata
        let _participant_index = (wallet_metadata.participant_index - 1) as usize; // Convert to 0-based
        
        // Serialize key packages based on blockchain type
        // Get the primary blockchain or first enabled one
        let primary_blockchain = if !wallet_metadata.blockchains.is_empty() {
            wallet_metadata.blockchains.iter()
                .find(|b| b.enabled)
                .or_else(|| wallet_metadata.blockchains.first())
                .map(|b| b.blockchain.as_str())
        } else {
            wallet_metadata.blockchain.as_deref()
        }.ok_or_else(|| KeystoreError::General("No blockchain specified".into()))?;
        
        let (key_package_base64, public_key_base64, curve, ethereum_address, solana_address) = 
            match primary_blockchain {
                "ethereum" => {
                    let key_package = wallet_data.secp256k1_key_package.as_ref()
                        .ok_or_else(|| KeystoreError::General("Missing secp256k1 key package".into()))?;
                    let public_key = wallet_data.secp256k1_public_key.as_ref()
                        .ok_or_else(|| KeystoreError::General("Missing secp256k1 public key".into()))?;
                    
                    (
                        general_purpose::STANDARD.encode(serde_json::to_vec(key_package)
                            .map_err(|e| KeystoreError::SerializationError(e.to_string()))?),
                        general_purpose::STANDARD.encode(serde_json::to_vec(public_key)
                            .map_err(|e| KeystoreError::SerializationError(e.to_string()))?),
                        "secp256k1",
                        Some(wallet_metadata.blockchains.iter()
                            .find(|b| b.blockchain == "ethereum")
                            .or_else(|| wallet_metadata.blockchains.first())
                            .map(|b| b.address.clone())
                            .or_else(|| wallet_metadata.public_address.clone())
                            .unwrap_or_default()),
                        None,
                    )
                }
                "solana" => {
                    let key_package = wallet_data.ed25519_key_package.as_ref()
                        .ok_or_else(|| KeystoreError::General("Missing ed25519 key package".into()))?;
                    let public_key = wallet_data.ed25519_public_key.as_ref()
                        .ok_or_else(|| KeystoreError::General("Missing ed25519 public key".into()))?;
                    
                    (
                        general_purpose::STANDARD.encode(serde_json::to_vec(key_package)
                            .map_err(|e| KeystoreError::SerializationError(e.to_string()))?),
                        general_purpose::STANDARD.encode(serde_json::to_vec(public_key)
                            .map_err(|e| KeystoreError::SerializationError(e.to_string()))?),
                        "ed25519",
                        None,
                        Some(wallet_metadata.blockchains.iter()
                            .find(|b| b.blockchain == "solana")
                            .or_else(|| wallet_metadata.blockchains.first())
                            .map(|b| b.address.clone())
                            .or_else(|| wallet_metadata.public_address.clone())
                            .unwrap_or_default()),
                    )
                }
                _ => return Err(KeystoreError::UnsupportedBlockchain(primary_blockchain.to_string())),
            };
        
        // Group public key is already serialized as a string
        let group_public_key = wallet_metadata.group_public_key.clone();
        
        Ok(ExtensionKeyShareData {
            key_package: key_package_base64,
            public_key_package: public_key_base64,
            group_public_key,
            session_id: wallet_data.session_id.clone(),
            device_id: wallet_data.device_id.clone(),
            participant_index: wallet_metadata.participant_index,
            threshold: wallet_metadata.threshold,
            total_participants: wallet_metadata.total_participants,
            participants: vec![wallet_metadata.device_id.clone()], // Only this device for now
            curve: curve.to_string(),
            ethereum_address,
            solana_address,
            created_at: chrono::DateTime::parse_from_rfc3339(&wallet_metadata.created_at)
                .map(|dt| dt.timestamp_millis())
                .unwrap_or_else(|_| Utc::now().timestamp_millis()),
            last_used: None,
            backup_date: Some(Utc::now().timestamp_millis()),
        })
    }
    
    /// Convert extension format to CLI wallet data
    pub fn to_cli_wallet(&self) -> Result<(WalletData, super::models::WalletInfo)> {
        // Decode key packages
        let key_package_bytes = general_purpose::STANDARD
            .decode(&self.key_package)
            .map_err(|e| KeystoreError::DecryptionError(format!("Base64 decode error: {}", e)))?;
        
        let public_key_bytes = general_purpose::STANDARD
            .decode(&self.public_key_package)
            .map_err(|e| KeystoreError::DecryptionError(format!("Base64 decode error: {}", e)))?;
        
        // Create wallet data based on curve type
        let wallet_data = match self.curve.as_str() {
            "secp256k1" => {
                let key_package: Secp256k1KeyPackage = serde_json::from_slice(&key_package_bytes)
                    .map_err(|e| KeystoreError::DecryptionError(e.to_string()))?;
                let public_key: Secp256k1PublicKeyPackage = serde_json::from_slice(&public_key_bytes)
                    .map_err(|e| KeystoreError::DecryptionError(e.to_string()))?;
                
                WalletData {
                    secp256k1_key_package: Some(key_package),
                    secp256k1_public_key: Some(public_key),
                    ed25519_key_package: None,
                    ed25519_public_key: None,
                    session_id: self.session_id.clone(),
                    device_id: self.device_id.clone(),
                }
            },
            "ed25519" => {
                let key_package: Ed25519KeyPackage = serde_json::from_slice(&key_package_bytes)
                    .map_err(|e| KeystoreError::DecryptionError(e.to_string()))?;
                let public_key: Ed25519PublicKeyPackage = serde_json::from_slice(&public_key_bytes)
                    .map_err(|e| KeystoreError::DecryptionError(e.to_string()))?;
                
                WalletData {
                    secp256k1_key_package: None,
                    secp256k1_public_key: None,
                    ed25519_key_package: Some(key_package),
                    ed25519_public_key: Some(public_key),
                    session_id: self.session_id.clone(),
                    device_id: self.device_id.clone(),
                }
            },
            _ => return Err(KeystoreError::General("Unknown curve type".into())),
        };
        
        // Create device info
        let devices: Vec<super::models::DeviceInfo> = self.participants
            .iter()
            .enumerate()
            .map(|(idx, device_id)| {
                super::DeviceInfo {
                    device_id: device_id.clone(),
                    name: format!("Device {}", idx + 1),
                    identifier: format!("{}", idx + 1), // Use numeric identifier as string
                    last_seen: chrono::Utc::now().timestamp_millis() as u64,
                }
            })
            .collect();
        
        // Determine blockchain and address
        let (blockchain, curve_type, address) = match self.curve.as_str() {
            "secp256k1" => ("ethereum", "secp256k1", self.ethereum_address.as_ref()
                .ok_or_else(|| KeystoreError::General("Missing Ethereum address".into()))?),
            "ed25519" => ("solana", "ed25519", self.solana_address.as_ref()
                .ok_or_else(|| KeystoreError::General("Missing Solana address".into()))?),
            _ => return Err(KeystoreError::General("Unknown curve".into())),
        };
        
        let wallet_info = super::models::WalletInfo::new(
            self.session_id.clone(),
            format!("Imported Wallet {}", &self.session_id[..8]),
            curve_type.to_string(),
            blockchain.to_string(),
            address.clone(),
            self.threshold,
            self.total_participants,
            self.group_public_key.clone(),
            vec![], // tags
            Some(format!("Imported from Chrome extension on {}", Utc::now().format("%Y-%m-%d"))),
        );
        
        // Add devices to wallet info
        let mut wallet_info_with_devices = wallet_info;
        for device in devices {
            wallet_info_with_devices.add_device(device);
        }
        
        Ok((wallet_data, wallet_info_with_devices))
    }
}

/// Encrypt data using PBKDF2 (Chrome extension compatible)
pub fn encrypt_for_extension(
    data: &ExtensionKeyShareData,
    password: &str,
    wallet_id: &str,
) -> Result<ExtensionEncryptedKeyShare> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce
    };
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    // Generate salt and IV via the OS CSPRNG directly — stable across `rand` API churn.
    let mut salt = [0u8; 16];
    let mut iv = [0u8; 12];
    getrandom::fill(&mut salt).expect("getrandom failed for salt");
    getrandom::fill(&mut iv).expect("getrandom failed for iv");
    
    // Derive key using PBKDF2 (100k iterations for Chrome extension compatibility)
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        &salt,
        100_000,
        &mut key,
    );
    
    // Serialize and encrypt
    let plaintext = serde_json::to_vec(data)
        .map_err(|e| KeystoreError::SerializationError(e.to_string()))?;
    
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&iv);
    
    let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())
        .map_err(|e| KeystoreError::EncryptionError(e.to_string()))?;
    
    Ok(ExtensionEncryptedKeyShare {
        wallet_id: wallet_id.to_string(),
        algorithm: "AES-GCM".to_string(),
        salt: general_purpose::STANDARD.encode(salt),
        iv: general_purpose::STANDARD.encode(iv),
        ciphertext: general_purpose::STANDARD.encode(&ciphertext),
        auth_tag: None, // Included in ciphertext for AES-GCM
    })
}

/// Decrypt data using PBKDF2 (Chrome extension compatible)
pub fn decrypt_from_extension(
    encrypted: &ExtensionEncryptedKeyShare,
    password: &str,
) -> Result<ExtensionKeyShareData> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce
    };
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;
    
    // Decode base64
    let salt = general_purpose::STANDARD.decode(&encrypted.salt)
        .map_err(|e| KeystoreError::DecryptionError(format!("Salt decode: {}", e)))?;
    let iv = general_purpose::STANDARD.decode(&encrypted.iv)
        .map_err(|e| KeystoreError::DecryptionError(format!("IV decode: {}", e)))?;
    let ciphertext = general_purpose::STANDARD.decode(&encrypted.ciphertext)
        .map_err(|e| KeystoreError::DecryptionError(format!("Ciphertext decode: {}", e)))?;
    
    // Derive key using PBKDF2
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        &salt,
        100_000,
        &mut key,
    );
    
    // Decrypt
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&iv);
    
    let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| KeystoreError::DecryptionError(e.to_string()))?;
    
    let data: ExtensionKeyShareData = serde_json::from_slice(&plaintext)
        .map_err(|e| KeystoreError::DecryptionError(e.to_string()))?;
    
    Ok(data)
}


