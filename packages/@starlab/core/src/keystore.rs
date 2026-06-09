use crate::errors::{FrostError, Result};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Keystore data structure that's compatible between CLI and browser extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreData {
    // Core data for FROST protocol
    pub key_package: String,  // Base64 encoded
    pub public_key_package: String,  // Base64 encoded
    pub min_signers: u16,
    pub max_signers: u16,
    pub participant_index: u16,
    pub participant_indices: Vec<u16>,
    pub curve: String,  // "secp256k1" or "ed25519"
    
    // Additional fields for UI/management
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// Multi-curve keystore holding key packages for both ed25519 and secp256k1,
/// derived from a single root secret during unified DKG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCurveKeystoreData {
    pub ed25519: KeystoreData,
    pub secp256k1: KeystoreData,
}

/// High-level keystore abstraction
pub struct Keystore;

impl Keystore {
    /// Export keystore data in a format compatible with both CLI and browser
    pub fn export_keystore<C: crate::traits::FrostCurve>(
        key_package: &C::KeyPackage,
        public_key_package: &C::PublicKeyPackage,
        min_signers: u16,
        max_signers: u16,
        participant_index: u16,
        participant_indices: Vec<u16>,
        curve: &str,
    ) -> Result<KeystoreData> {
        let key_package_bytes = serde_json::to_vec(key_package)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let public_key_package_bytes = serde_json::to_vec(public_key_package)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        
        Ok(KeystoreData {
            key_package: BASE64.encode(&key_package_bytes),
            public_key_package: BASE64.encode(&public_key_package_bytes),
            min_signers,
            max_signers,
            participant_index,
            participant_indices,
            curve: curve.to_string(),
            wallet_id: None,
            device_id: None,
            device_name: None,
            session_id: None,
            timestamp: None,
        })
    }
    
    /// Import keystore data and deserialize the packages
    pub fn import_keystore<C: crate::traits::FrostCurve>(
        keystore_data: &KeystoreData,
    ) -> Result<(C::KeyPackage, C::PublicKeyPackage)> {
        let key_package_bytes = BASE64.decode(&keystore_data.key_package)
            .map_err(|e| FrostError::SerializationError(format!("Failed to decode key package: {}", e)))?;
        let public_key_package_bytes = BASE64.decode(&keystore_data.public_key_package)
            .map_err(|e| FrostError::SerializationError(format!("Failed to decode public key package: {}", e)))?;
        
        let key_package: C::KeyPackage = serde_json::from_slice(&key_package_bytes)
            .map_err(|e| FrostError::SerializationError(format!("Failed to deserialize key package: {}", e)))?;
        let public_key_package: C::PublicKeyPackage = serde_json::from_slice(&public_key_package_bytes)
            .map_err(|e| FrostError::SerializationError(format!("Failed to deserialize public key package: {}", e)))?;
        
        Ok((key_package, public_key_package))
    }
}

/// Encryption module for keystore files
pub mod encryption {
    use super::*;
    use aes_gcm::{
        aead::{Aead, KeyInit, OsRng},
        Aes256Gcm, Key, Nonce,
    };
    use argon2::{
        password_hash::{rand_core::RngCore, PasswordHasher, SaltString},
        Argon2,
    };
    use pbkdf2::{Pbkdf2, Params};
    
    /// Encrypt data using Argon2id (CLI compatible)
    pub fn encrypt_argon2(data: &[u8], password: &str) -> Result<Vec<u8>> {
        // Generate salt
        let salt = SaltString::generate(&mut OsRng);
        
        // Derive key using Argon2id
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        let hash_bytes = password_hash.hash.ok_or_else(|| 
            FrostError::EncryptionError("Failed to get hash bytes".to_string()))?;
        let key = Key::<Aes256Gcm>::from_slice(&hash_bytes.as_bytes()[0..32]);
        
        // Generate nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Encrypt
        let cipher = Aes256Gcm::new(key);
        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        // Combine salt + nonce + ciphertext
        let mut result = Vec::new();
        result.extend_from_slice(salt.as_str().as_bytes());
        result.push(0); // null terminator for salt
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
    
    /// Decrypt data using Argon2id (CLI compatible)
    pub fn decrypt_argon2(encrypted_data: &[u8], password: &str) -> Result<Vec<u8>> {
        // Extract salt (find null terminator)
        let salt_end = encrypted_data.iter().position(|&b| b == 0)
            .ok_or_else(|| FrostError::EncryptionError("Invalid encrypted data format".to_string()))?;
        let salt_str = std::str::from_utf8(&encrypted_data[..salt_end])
            .map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        let salt = SaltString::from_b64(salt_str)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        // Extract nonce and ciphertext
        let nonce_start = salt_end + 1;
        let nonce_end = nonce_start + 12;
        let nonce = Nonce::from_slice(&encrypted_data[nonce_start..nonce_end]);
        let ciphertext = &encrypted_data[nonce_end..];
        
        // Derive key
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        let hash_bytes = password_hash.hash.ok_or_else(|| 
            FrostError::EncryptionError("Failed to get hash bytes".to_string()))?;
        let key = Key::<Aes256Gcm>::from_slice(&hash_bytes.as_bytes()[0..32]);
        
        // Decrypt
        let cipher = Aes256Gcm::new(key);
        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))
    }
    
    /// Encrypt data using PBKDF2 (browser compatible)
    pub fn encrypt_pbkdf2(data: &[u8], password: &str) -> Result<Vec<u8>> {
        // Generate salt (16 bytes)
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        
        // Derive key using PBKDF2
        let params = Params {
            rounds: 100_000,
            output_length: 32,
        };
        let pbkdf2 = Pbkdf2;
        let salt_string = SaltString::encode_b64(&salt)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        let password_hash = pbkdf2.hash_password_customized(
            password.as_bytes(),
            None,
            None,
            params,
            &salt_string,
        ).map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        let hash_bytes = password_hash.hash.ok_or_else(|| 
            FrostError::EncryptionError("Failed to get hash bytes".to_string()))?;
        let key = Key::<Aes256Gcm>::from_slice(hash_bytes.as_bytes());
        
        // Generate nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Encrypt
        let cipher = Aes256Gcm::new(key);
        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        // Combine salt + nonce + ciphertext
        let mut result = Vec::new();
        result.extend_from_slice(&salt);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
    
    /// Decrypt data using PBKDF2 (browser compatible)
    pub fn decrypt_pbkdf2(encrypted_data: &[u8], password: &str) -> Result<Vec<u8>> {
        if encrypted_data.len() < 28 { // 16 (salt) + 12 (nonce) + at least some ciphertext
            return Err(FrostError::EncryptionError("Invalid encrypted data length".to_string()));
        }
        
        // Extract components
        let salt = &encrypted_data[..16];
        let nonce = Nonce::from_slice(&encrypted_data[16..28]);
        let ciphertext = &encrypted_data[28..];
        
        // Derive key
        let params = Params {
            rounds: 100_000,
            output_length: 32,
        };
        let pbkdf2 = Pbkdf2;
        let salt_string = SaltString::encode_b64(salt)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        let password_hash = pbkdf2.hash_password_customized(
            password.as_bytes(),
            None,
            None,
            params,
            &salt_string,
        ).map_err(|e| FrostError::EncryptionError(e.to_string()))?;
        
        let hash_bytes = password_hash.hash.ok_or_else(|| 
            FrostError::EncryptionError("Failed to get hash bytes".to_string()))?;
        let key = Key::<Aes256Gcm>::from_slice(hash_bytes.as_bytes());
        
        // Decrypt
        let cipher = Aes256Gcm::new(key);
        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| FrostError::EncryptionError(e.to_string()))
    }
}