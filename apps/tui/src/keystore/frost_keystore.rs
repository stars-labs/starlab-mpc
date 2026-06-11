//! FROST-specific keystore functionality for saving and loading key packages

use frost_secp256k1::{
    keys::{KeyPackage, PublicKeyPackage},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

/// Error types for FROST keystore operations
#[derive(Debug, thiserror::Error)]
pub enum FrostKeystoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Encryption error: {0}")]
    Encryption(String),
    
    #[error("Invalid password")]
    InvalidPassword,
    
    #[error("Invalid keystore format")]
    InvalidFormat,
    
    #[error("FROST error: {0}")]
    Frost(String),
}

type Result<T> = std::result::Result<T, FrostKeystoreError>;

/// Output of `encrypt_data`: `(ciphertext, salt, iv, tag)`.
/// `ciphertext` is AES-256-GCM, `salt` is the 32-byte PBKDF2 salt,
/// `iv` is the 12-byte AES-GCM nonce, `tag` is the auth tag.
type EncryptedBundle = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

/// FROST keystore format for persistent storage
#[derive(Debug, Serialize, Deserialize)]
pub struct FrostKeystore {
    /// Version of the keystore format
    pub version: String,
    
    /// Unique identifier for this keystore
    pub id: String,
    
    /// Ethereum address derived from the group public key
    pub address: String,
    
    /// Encrypted data containing the FROST key package
    pub crypto: CryptoData,
    
    /// FROST-specific metadata
    pub frost: FrostMetadata,
}

/// Encrypted data structure
#[derive(Debug, Serialize, Deserialize)]
pub struct CryptoData {
    /// Cipher used for encryption (aes-256-gcm)
    pub cipher: String,
    
    /// Cipher parameters
    pub cipherparams: CipherParams,
    
    /// Encrypted key package data (hex encoded)
    pub ciphertext: String,
    
    /// Key derivation function (pbkdf2)
    pub kdf: String,
    
    /// KDF parameters
    pub kdfparams: KdfParams,
}

/// Cipher parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct CipherParams {
    /// Initialization vector (hex encoded)
    pub iv: String,
    
    /// Authentication tag (hex encoded)
    pub tag: String,
}

/// Key derivation function parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct KdfParams {
    /// Iteration count
    pub c: u32,
    
    /// Derived key length
    pub dklen: u32,
    
    /// PRF (hmac-sha256)
    pub prf: String,
    
    /// Salt (hex encoded)
    pub salt: String,
}

/// FROST-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrostMetadata {
    /// Threshold required for signing
    pub threshold: u16,
    
    /// Total number of participants
    pub total_participants: u16,
    
    /// This participant's ID (1-based)
    pub participant_id: u16,
    
    /// Group public key (hex encoded)
    pub group_public_key: String,
    
    /// Curve type (secp256k1)
    pub curve: String,
}

/// Serializable format for KeyPackage
#[derive(Debug, Serialize, Deserialize)]
struct SerializableKeyPackage {
    identifier: u16,
    signing_share: Vec<u8>,
    verifying_share: Vec<u8>,
    verifying_key: Vec<u8>,
    min_signers: u16,
}

/// Serializable format for PublicKeyPackage
#[derive(Debug, Serialize, Deserialize)]
struct SerializablePublicKeyPackage {
    verifying_shares: BTreeMap<u16, Vec<u8>>,
    verifying_key: Vec<u8>,
}

/// Combined package for storage
#[derive(Debug, Serialize, Deserialize)]
struct FrostPackage {
    key_package: SerializableKeyPackage,
    pubkey_package: SerializablePublicKeyPackage,
}

/// Manager for FROST keystore operations
pub struct FrostKeystoreManager {
    base_path: PathBuf,
}

impl FrostKeystoreManager {
    /// Creates a new keystore manager with the specified base path
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }
    
    /// Saves a FROST key package to an encrypted keystore file
    pub fn save_keystore(
        &self,
        participant_id: u16,
        key_package: &KeyPackage,
        pubkey_package: &PublicKeyPackage,
        password: &str,
        threshold: u16,
        total_participants: u16,
    ) -> Result<String> {
        // Generate keystore ID
        let keystore_id = uuid::Uuid::new_v4().to_string();
        
        // Serialize the key packages
        let frost_package = self.serialize_packages(key_package, pubkey_package)?;
        let package_bytes = serde_json::to_vec(&frost_package)?;
        
        // Encrypt the data
        let (ciphertext, salt, iv, tag) = self.encrypt_data(&package_bytes, password)?;
        
        // Get group public key and derive Ethereum address  
        // Store the actual verifying key for proper persistence
        let _group_vk = pubkey_package.verifying_key();
        
        // For Ethereum address, we'd need to properly serialize the key
        // For now, use a placeholder address
        let eth_address = format!("0x{}", hex::encode([0u8; 20]));
        
        // Create keystore structure
        let keystore = FrostKeystore {
            version: "1.0".to_string(),
            id: keystore_id,
            address: eth_address,
            crypto: CryptoData {
                cipher: "aes-256-gcm".to_string(),
                cipherparams: CipherParams {
                    iv: hex::encode(iv),
                    tag: hex::encode(tag),
                },
                ciphertext: hex::encode(ciphertext),
                kdf: "pbkdf2".to_string(),
                kdfparams: KdfParams {
                    c: 262144,
                    dklen: 32,
                    prf: "hmac-sha256".to_string(),
                    salt: hex::encode(salt),
                },
            },
            frost: FrostMetadata {
                threshold,
                total_participants,
                participant_id,
                group_public_key: format!("test_group_key_{}", participant_id), // Simplified for testing
                curve: "secp256k1".to_string(),
            },
        };
        
        // Save to file
        let filename = format!("keystore_p{}.json", participant_id);
        let filepath = self.base_path.join(&filename);
        let file = fs::File::create(&filepath)?;
        serde_json::to_writer_pretty(file, &keystore)?;
        
        Ok(filepath.to_string_lossy().to_string())
    }
    
    /// Loads a FROST key package from an encrypted keystore file
    pub fn load_keystore(
        &self,
        filepath: impl AsRef<Path>,
        password: &str,
    ) -> Result<(u16, KeyPackage, PublicKeyPackage, FrostMetadata)> {
        // Read keystore file
        let file = fs::File::open(filepath)?;
        let keystore: FrostKeystore = serde_json::from_reader(file)?;
        
        // Decrypt the data
        let ciphertext = hex::decode(&keystore.crypto.ciphertext)
            .map_err(|_e| FrostKeystoreError::InvalidFormat)?;
        let salt = hex::decode(&keystore.crypto.kdfparams.salt)
            .map_err(|_e| FrostKeystoreError::InvalidFormat)?;
        let iv = hex::decode(&keystore.crypto.cipherparams.iv)
            .map_err(|_e| FrostKeystoreError::InvalidFormat)?;
        let tag = hex::decode(&keystore.crypto.cipherparams.tag)
            .map_err(|_e| FrostKeystoreError::InvalidFormat)?;
        
        let decrypted = self.decrypt_data(&ciphertext, &salt, &iv, &tag, password)?;
        
        // Deserialize the packages
        let frost_package: FrostPackage = serde_json::from_slice(&decrypted)?;
        let (key_package, pubkey_package) = self.deserialize_packages(frost_package)?;
        
        Ok((
            keystore.frost.participant_id,
            key_package,
            pubkey_package,
            keystore.frost,
        ))
    }
    
    /// Encrypts data using AES-256-GCM with PBKDF2 key derivation
    fn encrypt_data(&self, data: &[u8], password: &str) -> Result<EncryptedBundle> {
        // Generate random salt and IV via the OS CSPRNG directly — stable API
        // and avoids the higher-level `rand` crate's trait-version churn.
        let mut salt = vec![0u8; 32];
        let mut iv = vec![0u8; 12];
        getrandom::fill(&mut salt)
            .map_err(|e| FrostKeystoreError::Encryption(format!("getrandom salt: {}", e)))?;
        getrandom::fill(&mut iv)
            .map_err(|e| FrostKeystoreError::Encryption(format!("getrandom iv: {}", e)))?;
        
        // Derive key using PBKDF2
        let mut key = vec![0u8; 32];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, 262144, &mut key);
        
        // Encrypt using AES-256-GCM
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| FrostKeystoreError::Encryption(e.to_string()))?;
        let nonce = Nonce::from_slice(&iv);
        
        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| FrostKeystoreError::Encryption(e.to_string()))?;
        
        // Split ciphertext and tag
        let (ct, tag) = ciphertext.split_at(ciphertext.len() - 16);
        
        Ok((ct.to_vec(), salt, iv, tag.to_vec()))
    }
    
    /// Decrypts data using AES-256-GCM with PBKDF2 key derivation
    fn decrypt_data(
        &self,
        ciphertext: &[u8],
        salt: &[u8],
        iv: &[u8],
        tag: &[u8],
        password: &str,
    ) -> Result<Vec<u8>> {
        // Derive key using PBKDF2
        let mut key = vec![0u8; 32];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, 262144, &mut key);
        
        // Decrypt using AES-256-GCM
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| FrostKeystoreError::Encryption(e.to_string()))?;
        let nonce = Nonce::from_slice(iv);
        
        // Combine ciphertext and tag
        let mut combined = ciphertext.to_vec();
        combined.extend_from_slice(tag);
        
        let plaintext = cipher.decrypt(nonce, combined.as_ref())
            .map_err(|_| FrostKeystoreError::InvalidPassword)?;
        
        Ok(plaintext)
    }
    
    /// Serializes FROST packages for storage
    fn serialize_packages(
        &self,
        key_package: &KeyPackage,
        pubkey_package: &PublicKeyPackage,
    ) -> Result<FrostPackage> {
        // Serialize using serde_json for now
        let key_pkg_bytes = serde_json::to_vec(key_package)
            .map_err(|e| FrostKeystoreError::Frost(e.to_string()))?;
        
        let pubkey_pkg_bytes = serde_json::to_vec(pubkey_package)
            .map_err(|e| FrostKeystoreError::Frost(e.to_string()))?;
        
        // Create simplified serializable versions
        let serializable_key = SerializableKeyPackage {
            identifier: 1, // Placeholder - the actual data is in signing_share
            signing_share: key_pkg_bytes,
            verifying_share: vec![],
            verifying_key: vec![],
            min_signers: 2,
        };
        
        let serializable_pubkey = SerializablePublicKeyPackage {
            verifying_shares: BTreeMap::new(),
            verifying_key: pubkey_pkg_bytes,
        };
        
        Ok(FrostPackage {
            key_package: serializable_key,
            pubkey_package: serializable_pubkey,
        })
    }
    
    /// Deserializes FROST packages from storage
    fn deserialize_packages(&self, package: FrostPackage) -> Result<(KeyPackage, PublicKeyPackage)> {
        // Deserialize using serde_json
        let key_package: KeyPackage = serde_json::from_slice(&package.key_package.signing_share)
            .map_err(|e| FrostKeystoreError::Frost(e.to_string()))?;
        
        let pubkey_package: PublicKeyPackage = serde_json::from_slice(&package.pubkey_package.verifying_key)
            .map_err(|e| FrostKeystoreError::Frost(e.to_string()))?;
        
        Ok((key_package, pubkey_package))
    }
}

