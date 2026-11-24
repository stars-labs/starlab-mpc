//! Encrypted NVS Flash keystore for the ESP32 hardware share.
//!
//! Layout in NVS:
//! - `meta:pin`     -> JSON [`PinHeader`] (salt, verifier MAC, failed_attempts)
//! - `w:<wallet_id>` -> JSON [`StoredWallet`] (public metadata + AES-256-GCM
//!   encrypted secret `KeyPackage` bytes)
//!
//! Key schedule:
//! 1. `pin_key = PBKDF2-HMAC-SHA256(pin, pin_salt, 60_000)` (32 bytes)
//! 2. `master = HKDF-SHA256(salt = device_hw_secret, ikm = pin_key)`
//!    - `kek      = Expand(master, "starlab-hw/v1/kek")` (AES-256-GCM key)
//!    - `verifier = Expand(master, "starlab-hw/v1/pin-verify")` (constant-time check)
//!
//! Mixing the chip's eFuse HMAC secret (`device_hw_secret`) ensures that an
//! attacker who reads external Flash without eFuse access cannot run an
//! offline dictionary attack on a 6-digit PIN.

use crate::MAX_PIN_ATTEMPTS;
use crate::hal::{HalError, HardwareHal};
use crate::protocol::{CurveKind, WalletSummary};
use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit, Nonce},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::Zeroizing;

const PIN_META_KEY: &str = "meta:pin";
const WALLET_KEY_PREFIX: &str = "w:";
const PBKDF2_ROUNDS: u32 = 60_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("PIN not initialized")]
    PinNotSet,
    #[error("vault is locked")]
    Locked,
    #[error("wrong PIN ({remaining} attempts remaining)")]
    WrongPin { remaining: u8 },
    #[error("maximum PIN attempts exceeded — all hardware shares wiped")]
    Wiped,
    #[error("PIN must be 4..=64 printable characters")]
    InvalidPinFormat,
    #[error("wallet already exists: {0}")]
    WalletExists(String),
    #[error("wallet not found: {0}")]
    WalletNotFound(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("storage error: {0}")]
    Storage(#[from] HalError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PinHeader {
    salt_b64: String,
    verifier_b64: String,
    failed_attempts: u8,
    max_attempts: u8,
}

/// On-flash wallet record: public metadata is readable while locked (for
/// `list_wallets` and public-only `derive_address`); secret `KeyPackage` is
/// always stored as AES-256-GCM ciphertext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredWallet {
    pub wallet_id: String,
    pub curve: CurveKind,
    pub participant_index: u16,
    pub threshold: u16,
    pub total: u16,
    /// Hex-encoded group verifying key bytes.
    pub group_pubkey_hex: String,
    /// Base64-encoded JSON of FROST `PublicKeyPackage` (public).
    pub public_key_package_b64: String,
    /// 12-byte AES-GCM nonce (base64).
    pub nonce_b64: String,
    /// AES-256-GCM ciphertext of JSON-encoded FROST `KeyPackage` (base64).
    pub ciphertext_b64: String,
}

impl StoredWallet {
    pub fn summary(&self) -> WalletSummary {
        WalletSummary {
            wallet_id: self.wallet_id.clone(),
            curve: self.curve,
            participant_index: self.participant_index,
            threshold: self.threshold,
            total: self.total,
            group_pubkey_hex: self.group_pubkey_hex.clone(),
        }
    }
}

/// Manages PIN authentication, KEK lifecycle in RAM, and AES-256-GCM wallet
/// persistence on top of [`HardwareHal`].
pub struct NvsVault {
    kek: Option<Zeroizing<[u8; 32]>>,
}

impl Default for NvsVault {
    fn default() -> Self {
        Self::new()
    }
}

impl NvsVault {
    pub fn new() -> Self {
        Self { kek: None }
    }

    pub fn is_unlocked(&self) -> bool {
        self.kek.is_some()
    }

    pub fn lock(&mut self) {
        self.kek = None;
    }

    pub fn is_pin_configured<H: HardwareHal>(&self, hal: &H) -> Result<bool, VaultError> {
        Ok(hal.nvs_get(PIN_META_KEY)?.is_some())
    }

    pub fn remaining_attempts<H: HardwareHal>(&self, hal: &H) -> Result<u8, VaultError> {
        let Some(raw) = hal.nvs_get(PIN_META_KEY)? else {
            return Ok(MAX_PIN_ATTEMPTS);
        };
        let header: PinHeader =
            serde_json::from_slice(&raw).map_err(|e| VaultError::Crypto(e.to_string()))?;
        Ok(header.max_attempts.saturating_sub(header.failed_attempts))
    }

    /// Initialize a new PIN (if unconfigured) or re-wrap all stored shares
    /// under a new PIN (if already unlocked).
    pub fn set_pin<H: HardwareHal>(&mut self, hal: &mut H, new_pin: &str) -> Result<(), VaultError> {
        validate_pin(new_pin)?;
        let already_configured = self.is_pin_configured(hal)?;
        if already_configured && !self.is_unlocked() {
            return Err(VaultError::Locked);
        }

        // If rotating PIN, decrypt all existing shares first while old KEK is live.
        let existing_plaintexts: Vec<(StoredWallet, Zeroizing<Vec<u8>>)> = if already_configured {
            let wallets = self.list_stored_wallets(hal)?;
            let mut out = Vec::with_capacity(wallets.len());
            for w in wallets {
                let pt = self.decrypt_key_package(&w)?;
                out.push((w, pt));
            }
            out
        } else {
            Vec::new()
        };

        let mut salt = [0u8; SALT_LEN];
        hal.fill_random(&mut salt)?;
        let hw_secret = hal.device_hw_secret();
        let (new_kek, verifier) = derive_keys(new_pin, &salt, &hw_secret)?;

        let header = PinHeader {
            salt_b64: BASE64.encode(salt),
            verifier_b64: BASE64.encode(*verifier),
            failed_attempts: 0,
            max_attempts: MAX_PIN_ATTEMPTS,
        };
        let header_bytes =
            serde_json::to_vec(&header).map_err(|e| VaultError::Crypto(e.to_string()))?;
        hal.nvs_set(PIN_META_KEY, &header_bytes)?;
        self.kek = Some(new_kek);

        // Re-encrypt every existing share with fresh nonces under the new KEK.
        for (mut w, pt) in existing_plaintexts {
            let (nonce_b64, ciphertext_b64) = self.encrypt_bytes(hal, &pt)?;
            w.nonce_b64 = nonce_b64;
            w.ciphertext_b64 = ciphertext_b64;
            let bytes = serde_json::to_vec(&w).map_err(|e| VaultError::Crypto(e.to_string()))?;
            hal.nvs_set(&wallet_nvs_key(&w.wallet_id), &bytes)?;
        }

        Ok(())
    }

    /// Authenticate with PIN. Increments persistent retry counter on failure
    /// and irreversibly wipes all shares after `MAX_PIN_ATTEMPTS` consecutive
    /// failures.
    pub fn unlock<H: HardwareHal>(&mut self, hal: &mut H, pin: &str) -> Result<(), VaultError> {
        let Some(raw) = hal.nvs_get(PIN_META_KEY)? else {
            return Err(VaultError::PinNotSet);
        };
        let mut header: PinHeader =
            serde_json::from_slice(&raw).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let salt = BASE64
            .decode(&header.salt_b64)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        let expected_verifier = BASE64
            .decode(&header.verifier_b64)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;

        let hw_secret = hal.device_hw_secret();
        let (candidate_kek, candidate_verifier) = derive_keys(pin, &salt, &hw_secret)?;

        let matches = expected_verifier.len() == candidate_verifier.len()
            && bool::from(expected_verifier.as_slice().ct_eq(candidate_verifier.as_slice()));

        if matches {
            if header.failed_attempts != 0 {
                header.failed_attempts = 0;
                let updated =
                    serde_json::to_vec(&header).map_err(|e| VaultError::Crypto(e.to_string()))?;
                hal.nvs_set(PIN_META_KEY, &updated)?;
            }
            self.kek = Some(candidate_kek);
            return Ok(());
        }

        // Wrong PIN: persist incremented counter BEFORE returning.
        header.failed_attempts = header.failed_attempts.saturating_add(1);
        if header.failed_attempts >= header.max_attempts {
            self.wipe_all(hal)?;
            return Err(VaultError::Wiped);
        }

        let remaining = header.max_attempts.saturating_sub(header.failed_attempts);
        let updated = serde_json::to_vec(&header).map_err(|e| VaultError::Crypto(e.to_string()))?;
        hal.nvs_set(PIN_META_KEY, &updated)?;
        Err(VaultError::WrongPin { remaining })
    }

    /// Irreversibly wipe all stored wallet shares and PIN metadata from NVS.
    pub fn wipe_all<H: HardwareHal>(&mut self, hal: &mut H) -> Result<(), VaultError> {
        self.lock();
        let keys = hal.nvs_list_prefix(WALLET_KEY_PREFIX)?;
        for k in keys {
            hal.nvs_remove(&k)?;
        }
        hal.nvs_remove(PIN_META_KEY)?;
        hal.show_status("VAULT WIPED", "Max PIN failures");
        Ok(())
    }

    /// Persist a newly generated wallet share (ciphertext only for secret share).
    #[allow(clippy::too_many_arguments)]
    pub fn save_wallet<H: HardwareHal>(
        &self,
        hal: &mut H,
        wallet_id: &str,
        curve: CurveKind,
        participant_index: u16,
        threshold: u16,
        total: u16,
        group_pubkey_hex: String,
        public_key_package_json: &[u8],
        key_package_json: &[u8],
    ) -> Result<StoredWallet, VaultError> {
        if !self.is_unlocked() {
            return Err(VaultError::Locked);
        }
        let key = wallet_nvs_key(wallet_id);
        if hal.nvs_get(&key)?.is_some() {
            return Err(VaultError::WalletExists(wallet_id.to_string()));
        }

        let (nonce_b64, ciphertext_b64) = self.encrypt_bytes(hal, key_package_json)?;
        let record = StoredWallet {
            wallet_id: wallet_id.to_string(),
            curve,
            participant_index,
            threshold,
            total,
            group_pubkey_hex,
            public_key_package_b64: BASE64.encode(public_key_package_json),
            nonce_b64,
            ciphertext_b64,
        };
        let bytes = serde_json::to_vec(&record).map_err(|e| VaultError::Crypto(e.to_string()))?;
        hal.nvs_set(&key, &bytes)?;
        Ok(record)
    }

    pub fn load_wallet_meta<H: HardwareHal>(
        &self,
        hal: &H,
        wallet_id: &str,
    ) -> Result<StoredWallet, VaultError> {
        let Some(raw) = hal.nvs_get(&wallet_nvs_key(wallet_id))? else {
            return Err(VaultError::WalletNotFound(wallet_id.to_string()));
        };
        serde_json::from_slice(&raw).map_err(|e| VaultError::Crypto(e.to_string()))
    }

    pub fn list_stored_wallets<H: HardwareHal>(&self, hal: &H) -> Result<Vec<StoredWallet>, VaultError> {
        let keys = hal.nvs_list_prefix(WALLET_KEY_PREFIX)?;
        let mut out = Vec::with_capacity(keys.len());
        for k in keys {
            if let Some(raw) = hal.nvs_get(&k)? {
                let w: StoredWallet =
                    serde_json::from_slice(&raw).map_err(|e| VaultError::Crypto(e.to_string()))?;
                out.push(w);
            }
        }
        out.sort_by(|a, b| a.wallet_id.cmp(&b.wallet_id));
        Ok(out)
    }

    pub fn delete_wallet<H: HardwareHal>(
        &self,
        hal: &mut H,
        wallet_id: &str,
    ) -> Result<(), VaultError> {
        if !self.is_unlocked() {
            return Err(VaultError::Locked);
        }
        let key = wallet_nvs_key(wallet_id);
        if hal.nvs_get(&key)?.is_none() {
            return Err(VaultError::WalletNotFound(wallet_id.to_string()));
        }
        hal.nvs_remove(&key)?;
        Ok(())
    }

    /// Decrypt the stored FROST `KeyPackage` JSON bytes into a zeroizing buffer.
    pub fn decrypt_key_package(&self, record: &StoredWallet) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let kek = self.kek.as_ref().ok_or(VaultError::Locked)?;
        let nonce_bytes = BASE64
            .decode(&record.nonce_b64)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        let ciphertext = BASE64
            .decode(&record.ciphertext_b64)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        let nonce = Nonce::<Aes256Gcm>::try_from(nonce_bytes.as_slice())
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        let cipher =
            Aes256Gcm::new_from_slice(&**kek).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let plaintext = cipher
            .decrypt(&nonce, ciphertext.as_slice())
            .map_err(|e| VaultError::Crypto(format!("AEAD decrypt failed: {e}")))?;
        Ok(Zeroizing::new(plaintext))
    }

    fn encrypt_bytes<H: HardwareHal>(
        &self,
        hal: &mut H,
        plaintext: &[u8],
    ) -> Result<(String, String), VaultError> {
        let kek = self.kek.as_ref().ok_or(VaultError::Locked)?;
        let mut nonce_raw = [0u8; NONCE_LEN];
        hal.fill_random(&mut nonce_raw)?;
        let cipher =
            Aes256Gcm::new_from_slice(&**kek).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let ciphertext = cipher
            .encrypt(&Nonce::<Aes256Gcm>::from(nonce_raw), plaintext)
            .map_err(|e| VaultError::Crypto(format!("AEAD encrypt failed: {e}")))?;
        Ok((BASE64.encode(nonce_raw), BASE64.encode(ciphertext)))
    }
}

fn wallet_nvs_key(wallet_id: &str) -> String {
    format!("{WALLET_KEY_PREFIX}{wallet_id}")
}

fn validate_pin(pin: &str) -> Result<(), VaultError> {
    let len = pin.len();
    if !(4..=64).contains(&len) || !pin.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err(VaultError::InvalidPinFormat);
    }
    Ok(())
}

type DerivedKeyMaterial = (Zeroizing<[u8; 32]>, Zeroizing<[u8; 32]>);

fn derive_keys(
    pin: &str,
    salt: &[u8],
    hw_secret: &[u8; 32],
) -> Result<DerivedKeyMaterial, VaultError> {
    let mut pbkdf_out = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2_hmac::<Sha256>(pin.as_bytes(), salt, PBKDF2_ROUNDS, &mut *pbkdf_out);

    // Mix chip-unique eFuse HMAC secret into HKDF salt.
    let hk = Hkdf::<Sha256>::new(Some(hw_secret), &*pbkdf_out);
    let mut kek = Zeroizing::new([0u8; 32]);
    let mut verifier = Zeroizing::new([0u8; 32]);
    hk.expand(b"starlab-hw/v1/kek", &mut *kek)
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    hk.expand(b"starlab-hw/v1/pin-verify", &mut *verifier)
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    Ok((kek, verifier))
}
