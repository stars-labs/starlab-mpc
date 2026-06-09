//! Encryption utilities for the keystore module.
//!
//! This module provides functions for encrypting and decrypting keystore data
//! using AES-256-GCM with either Argon2id (CLI default) or PBKDF2 (browser compatible).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2, Params,
};
use pbkdf2::{pbkdf2_hmac_array};
use sha2::Sha256;

use crate::keystore::KeystoreError;

// Constants for encryption parameters
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32; // 256 bits

// PBKDF2 constants (browser compatible)
const PBKDF2_ITERATIONS: u32 = 100_000; // Standard for PBKDF2-SHA256

/// Key derivation method used for encryption
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDerivation {
    /// Argon2id - secure but not browser compatible
    Argon2id,
    /// PBKDF2-SHA256 - browser compatible
    Pbkdf2,
}

impl KeyDerivation {
    /// Returns the algorithm identifier string for metadata
    pub fn algorithm_string(&self) -> &'static str {
        match self {
            KeyDerivation::Argon2id => "AES-256-GCM-Argon2id",
            KeyDerivation::Pbkdf2 => "AES-256-GCM-PBKDF2",
        }
    }
}


/// Encrypts data with a password using AES-256-GCM with the specified key derivation method.
///
/// The output format is: `salt (16 bytes) + nonce (12 bytes) + ciphertext`
pub fn encrypt_data_with_method(data: &[u8], password: &str, method: KeyDerivation) -> crate::keystore::Result<Vec<u8>> {
    // Generate a random salt (direct system CSPRNG; stable across `rand` version churn).
    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt)
        .map_err(|e| KeystoreError::General(format!("getrandom failed for salt: {}", e)))?;

    // Derive key using the specified method
    let key = match method {
        KeyDerivation::Argon2id => {
            let salt_string = SaltString::encode_b64(&salt)
                .map_err(|e| KeystoreError::General(format!("Salt encoding error: {}", e)))?;

            let argon2 = Argon2::new(
                argon2::Algorithm::Argon2id,
                argon2::Version::V0x13,
                Params::new(4096, 3, 1, Some(KEY_LEN)).unwrap(),
            );

            let password_hash = argon2
                .hash_password(password.as_bytes(), &salt_string)
                .map_err(|e| KeystoreError::EncryptionError(format!("Password hashing error: {}", e)))?;
            
            let binding = password_hash.hash.unwrap();
            let hash_bytes = binding.as_bytes();
            *Key::<Aes256Gcm>::from_slice(hash_bytes)
        }
        KeyDerivation::Pbkdf2 => {
            let key_bytes: [u8; KEY_LEN] = pbkdf2_hmac_array::<Sha256, KEY_LEN>(
                password.as_bytes(),
                &salt,
                PBKDF2_ITERATIONS,
            );
            *Key::<Aes256Gcm>::from_slice(&key_bytes)
        }
    };

    // Generate a random nonce (fresh for every encryption; critical for AES-GCM safety).
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes)
        .map_err(|e| KeystoreError::General(format!("getrandom failed for nonce: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt the data
    let cipher = Aes256Gcm::new(&key);
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| KeystoreError::EncryptionError(format!("Encryption error: {}", e)))?;

    // Combine salt, nonce, and ciphertext
    let mut result = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypts data that was encrypted with `encrypt_data`.
///
/// The input format is expected to be: `salt (16 bytes) + nonce (12 bytes) + ciphertext`
/// This function tries both PBKDF2 and Argon2id for backward compatibility.
pub fn decrypt_data(encrypted_data: &[u8], password: &str) -> crate::keystore::Result<Vec<u8>> {
    // Try PBKDF2 first (current default, browser compatible)
    match decrypt_data_with_method(encrypted_data, password, KeyDerivation::Pbkdf2) {
        Ok(data) => Ok(data),
        Err(_) => {
            // If PBKDF2 fails, try Argon2id (legacy method for backward compatibility)
            decrypt_data_with_method(encrypted_data, password, KeyDerivation::Argon2id)
        }
    }
}

/// Decrypts data that was encrypted with the specified key derivation method.
///
/// The input format is expected to be: `salt (16 bytes) + nonce (12 bytes) + ciphertext`
pub fn decrypt_data_with_method(encrypted_data: &[u8], password: &str, method: KeyDerivation) -> crate::keystore::Result<Vec<u8>> {
    // Check if the data is long enough to contain the salt and nonce
    if encrypted_data.len() < SALT_LEN + NONCE_LEN {
        return Err(KeystoreError::DecryptionError("Invalid encrypted data format".to_string()));
    }

    // Extract salt and nonce
    let salt = &encrypted_data[0..SALT_LEN];
    let nonce_bytes = &encrypted_data[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &encrypted_data[SALT_LEN + NONCE_LEN..];

    // Derive key using the specified method
    let key = match method {
        KeyDerivation::Argon2id => {
            let salt_string = SaltString::encode_b64(salt)
                .map_err(|e| KeystoreError::DecryptionError(format!("Salt decoding error: {}", e)))?;

            let argon2 = Argon2::new(
                argon2::Algorithm::Argon2id,
                argon2::Version::V0x13,
                Params::new(4096, 3, 1, Some(KEY_LEN)).unwrap(),
            );

            let password_hash = argon2
                .hash_password(password.as_bytes(), &salt_string)
                .map_err(|e| KeystoreError::DecryptionError(format!("Password hashing error: {}", e)))?;
            
            let binding = password_hash.hash.unwrap();
            let hash_bytes = binding.as_bytes();
            *Key::<Aes256Gcm>::from_slice(hash_bytes)
        }
        KeyDerivation::Pbkdf2 => {
            let key_bytes: [u8; KEY_LEN] = pbkdf2_hmac_array::<Sha256, KEY_LEN>(
                password.as_bytes(),
                salt,
                PBKDF2_ITERATIONS,
            );
            *Key::<Aes256Gcm>::from_slice(&key_bytes)
        }
    };

    // Decrypt the data
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(&key);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| KeystoreError::InvalidPassword)?;

    Ok(plaintext)
}

