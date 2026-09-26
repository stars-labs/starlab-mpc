use crate::errors::{FrostError, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};

/// Keystore data structure that's compatible between CLI and browser extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreData {
    // Core data for FROST protocol
    pub key_package: String,        // Base64 encoded
    pub public_key_package: String, // Base64 encoded
    pub min_signers: u16,
    pub max_signers: u16,
    pub participant_index: u16,
    pub participant_indices: Vec<u16>,
    pub curve: String, // "secp256k1" or "ed25519"

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
        let key_package_bytes = BASE64.decode(&keystore_data.key_package).map_err(|e| {
            FrostError::SerializationError(format!("Failed to decode key package: {}", e))
        })?;
        let public_key_package_bytes =
            BASE64
                .decode(&keystore_data.public_key_package)
                .map_err(|e| {
                    FrostError::SerializationError(format!(
                        "Failed to decode public key package: {}",
                        e
                    ))
                })?;

        let key_package: C::KeyPackage =
            serde_json::from_slice(&key_package_bytes).map_err(|e| {
                FrostError::SerializationError(format!("Failed to deserialize key package: {}", e))
            })?;
        let public_key_package: C::PublicKeyPackage =
            serde_json::from_slice(&public_key_package_bytes).map_err(|e| {
                FrostError::SerializationError(format!(
                    "Failed to deserialize public key package: {}",
                    e
                ))
            })?;

        Ok((key_package, public_key_package))
    }
}

/// Encryption module for keystore files
pub mod encryption {
    use super::*;
    use aes_gcm::{
        Aes256Gcm,
        aead::{Aead, KeyInit, Nonce},
    };
    use argon2::{Algorithm, Argon2, Params, Version};
    use base64::engine::general_purpose::STANDARD_NO_PAD as B64;
    use sha2::Sha256;

    const SALT_LEN: usize = 16;
    const NONCE_LEN: usize = 12;
    const KEY_LEN: usize = 32;
    const PBKDF2_ROUNDS: u32 = 100_000;

    fn enc_err(e: impl std::fmt::Display) -> FrostError {
        FrostError::EncryptionError(e.to_string())
    }

    fn random<const N: usize>() -> Result<[u8; N]> {
        let mut buf = [0u8; N];
        getrandom::fill(&mut buf).map_err(enc_err)?;
        Ok(buf)
    }

    /// Argon2id v0x13, m=19456 KiB, t=2, p=1, 32-byte output. Pinned
    /// explicitly (these were argon2 0.5's defaults) so existing ciphertexts
    /// keep decrypting even if the crate's defaults ever move.
    fn argon2_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN]> {
        let params = Params::new(19 * 1024, 2, 1, Some(KEY_LEN)).map_err(enc_err)?;
        let mut key = [0u8; KEY_LEN];
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(enc_err)?;
        Ok(key)
    }

    fn pbkdf2_key(password: &str, salt: &[u8]) -> [u8; KEY_LEN] {
        let mut key = [0u8; KEY_LEN];
        pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
        key
    }

    fn seal(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], data: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key).map_err(enc_err)?;
        cipher
            .encrypt(&Nonce::<Aes256Gcm>::from(*nonce), data)
            .map_err(enc_err)
    }

    fn open(key: &[u8; KEY_LEN], nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::<Aes256Gcm>::try_from(nonce).map_err(enc_err)?;
        let cipher = Aes256Gcm::new_from_slice(key).map_err(enc_err)?;
        cipher.decrypt(&nonce, ciphertext).map_err(enc_err)
    }

    /// Encrypt data using Argon2id (CLI compatible).
    ///
    /// Format: `b64(salt)` (PHC-style unpadded base64) `‖ 0x00 ‖ nonce(12) ‖ ciphertext`.
    /// The KDF runs over the *decoded* salt bytes.
    pub fn encrypt_argon2(data: &[u8], password: &str) -> Result<Vec<u8>> {
        let salt = random::<SALT_LEN>()?;
        let nonce = random::<NONCE_LEN>()?;
        let ciphertext = seal(&argon2_key(password, &salt)?, &nonce, data)?;

        let mut result = B64.encode(salt).into_bytes();
        result.push(0); // null terminator for salt
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt data using Argon2id (CLI compatible)
    pub fn decrypt_argon2(encrypted_data: &[u8], password: &str) -> Result<Vec<u8>> {
        let salt_end = encrypted_data
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| enc_err("Invalid encrypted data format"))?;
        let salt = B64.decode(&encrypted_data[..salt_end]).map_err(enc_err)?;
        let nonce_start = salt_end + 1;
        let nonce_end = nonce_start + NONCE_LEN;
        if encrypted_data.len() < nonce_end {
            return Err(enc_err("Invalid encrypted data length"));
        }
        open(
            &argon2_key(password, &salt)?,
            &encrypted_data[nonce_start..nonce_end],
            &encrypted_data[nonce_end..],
        )
    }

    /// Encrypt data using PBKDF2-SHA256 (browser compatible).
    ///
    /// Format: `salt(16) ‖ nonce(12) ‖ ciphertext`.
    pub fn encrypt_pbkdf2(data: &[u8], password: &str) -> Result<Vec<u8>> {
        let salt = random::<SALT_LEN>()?;
        let nonce = random::<NONCE_LEN>()?;
        let ciphertext = seal(&pbkdf2_key(password, &salt), &nonce, data)?;

        let mut result = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
        result.extend_from_slice(&salt);
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt data using PBKDF2 (browser compatible)
    pub fn decrypt_pbkdf2(encrypted_data: &[u8], password: &str) -> Result<Vec<u8>> {
        if encrypted_data.len() < SALT_LEN + NONCE_LEN {
            return Err(enc_err("Invalid encrypted data length"));
        }
        let (salt, rest) = encrypted_data.split_at(SALT_LEN);
        let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
        open(&pbkdf2_key(password, salt), nonce, ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::encryption::*;

    // Ciphertexts produced by the pre-upgrade stack (argon2 0.5 / pbkdf2 0.12
    // password-hash API / aes-gcm 0.10). Existing keystores must keep opening.
    const LEGACY_ARGON2: &str = "796d70362b363462596e5062426473676c4f37546251000c6a30f2e70b9533f34a00ae48e51d1193a8bf5cb44b8ea0b678950795ea2c99b774577278349b4288a0a8ca7f68ae1da819";
    const LEGACY_PBKDF2: &str = "9df35b2f6e3c2903c517c36b28e1ec643b25a433a06b523db9bacf8457618fa08e49b1f8ecb665ec927376e13d68bf02fce8367aca6315ab34cf6d9bbff46321e411";
    const PLAINTEXT: &[u8] = b"starlab-compat-fixture";
    const PASSWORD: &str = "hunter2";

    #[test]
    fn decrypts_legacy_argon2_ciphertext() {
        let blob = hex::decode(LEGACY_ARGON2).unwrap();
        assert_eq!(decrypt_argon2(&blob, PASSWORD).unwrap(), PLAINTEXT);
    }

    #[test]
    fn decrypts_legacy_pbkdf2_ciphertext() {
        let blob = hex::decode(LEGACY_PBKDF2).unwrap();
        assert_eq!(decrypt_pbkdf2(&blob, PASSWORD).unwrap(), PLAINTEXT);
    }

    #[test]
    fn argon2_round_trips() {
        let blob = encrypt_argon2(PLAINTEXT, PASSWORD).unwrap();
        assert_eq!(decrypt_argon2(&blob, PASSWORD).unwrap(), PLAINTEXT);
    }

    #[test]
    fn pbkdf2_round_trips() {
        let blob = encrypt_pbkdf2(PLAINTEXT, PASSWORD).unwrap();
        assert_eq!(decrypt_pbkdf2(&blob, PASSWORD).unwrap(), PLAINTEXT);
    }

    #[test]
    fn wrong_password_is_rejected() {
        let blob = hex::decode(LEGACY_PBKDF2).unwrap();
        assert!(decrypt_pbkdf2(&blob, "wrong").is_err());
    }
}
