//! SLHW v1 wire protocol types (NDJSON over USB-CDC / UART).
//!
//! Every request is a single UTF-8 JSON line (`\n`-terminated) with an optional
//! correlation `id` and a `cmd`-tagged command body. Every response is a single
//! JSON line carrying `ok`, optional `id`, and either `data` (`event`-tagged)
//! or `error` (`code` + `message`).
//!
//! Security invariant: there is deliberately NO command to export secret key
//! shares or raw signing nonces.

use crate::airgap::OfflineDataPackage;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// Serde adapter for `BTreeMap<u16, String>` that works reliably even under
/// `#[serde(flatten)]` (where JSON object keys arrive buffered as strings).
mod u16_map {
    use super::*;

    pub fn serialize<S: Serializer>(
        map: &BTreeMap<u16, String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let str_map: BTreeMap<String, &String> =
            map.iter().map(|(k, v)| (k.to_string(), v)).collect();
        str_map.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<u16, String>, D::Error> {
        let str_map = BTreeMap::<String, String>::deserialize(deserializer)?;
        let mut out = BTreeMap::new();
        for (k, v) in str_map {
            let idx = k.parse::<u16>().map_err(serde::de::Error::custom)?;
            out.insert(idx, v);
        }
        Ok(out)
    }
}

/// Curves supported on-chip in SLHW v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CurveKind {
    Ed25519,
    Secp256k1,
}

impl CurveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CurveKind::Ed25519 => "ed25519",
            CurveKind::Secp256k1 => "secp256k1",
        }
    }

    pub fn from_tag(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "ed25519" => Some(CurveKind::Ed25519),
            "secp256k1" | "secp256k1-tr" => Some(CurveKind::Secp256k1),
            _ => None,
        }
    }
}

/// Top-level NDJSON request from host -> ESP32.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub command: HwCommand,
}

/// Optional human-readable Clear-Signing context displayed on the ESP32 screen
/// before the user presses the physical Approve button.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignDisplayInfo {
    /// Short summary line, e.g. `"Send 0.05 BTC"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Target recipient address, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// Formatted amount + symbol, e.g. `"0.05 BTC"` or `"12.5 SOL"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Filled in automatically by the engine (cannot be spoofed by host).
    #[serde(default)]
    pub wallet_id: String,
    #[serde(default)]
    pub chain: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub signer_address: String,
    #[serde(default)]
    pub message_digest_hex: String,
}

/// Commands accepted by the ESP32 hardware share.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum HwCommand {
    /// Health check / protocol version ping.
    Ping,
    /// Query device metadata, supported curves, lock state, wallet count.
    DeviceInfo,
    /// Initialize device PIN (first-time setup) or change PIN (when unlocked).
    SetPin {
        pin: String,
    },
    /// Unlock encrypted NVS vault using the user PIN.
    Unlock {
        pin: String,
    },
    /// Lock vault and zeroize KEK + any in-RAM signing nonces immediately.
    Lock,
    /// List stored wallet metadata (public-only; works even when locked).
    ListWallets,
    /// Public-only BIP-44/86 address derivation (works even when locked).
    DeriveAddress {
        wallet_id: String,
        chain: String,
        #[serde(default)]
        account: u32,
    },
    /// Start a FROST DKG ceremony for one curve.
    DkgInit {
        wallet_id: String,
        curve: CurveKind,
        participant_index: u16,
        threshold: u16,
        total: u16,
    },
    /// DKG Round 1: generate polynomial commitments via hardware TRNG.
    DkgRound1 {
        wallet_id: String,
    },
    /// DKG Round 2: ingest peer Round 1 packages and emit encrypted per-peer
    /// Round 2 packages (keys are 1-based participant indices).
    DkgRound2 {
        wallet_id: String,
        /// `peer_index -> hex(FROST binary or JSON round1::Package)`
        #[serde(with = "u16_map")]
        peer_round1: BTreeMap<u16, String>,
    },
    /// DKG Round 3 / Finalize: ingest peer Round 2 packages, persist encrypted
    /// share into NVS Flash, and return the group public key & Account 0 addresses.
    DkgFinalize {
        wallet_id: String,
        /// `peer_index -> hex(FROST binary or JSON round2::Package)`
        #[serde(with = "u16_map")]
        peer_round2: BTreeMap<u16, String>,
    },
    /// Signing Round 1: derive child share at `(chain, account)`, generate a
    /// single-use hardware-TRNG nonce (kept strictly in RAM), and emit the
    /// public commitment.
    SignCommit {
        wallet_id: String,
        /// `"bitcoin"`, `"solana"`, `"sui"`, or `"raw"` (root key).
        chain: String,
        #[serde(default)]
        account: u32,
    },
    /// Signing Round 2: display clear-signing info on screen, block for physical
    /// button approval, produce a FROST signature share, and immediately zeroize
    /// the single-use nonce (even if rejected or invalid).
    SignShare {
        session_id: String,
        /// Hex-encoded message (or 32-byte prehash / BIP-341 sighash).
        message_hex: String,
        /// Map of `participant_index -> hex(SigningCommitments)` for all signers
        /// in the quorum (including this device's own commitment).
        #[serde(with = "u16_map")]
        commitments: BTreeMap<u16, String>,
        #[serde(default)]
        display: SignDisplayInfo,
    },
    /// Delete a stored wallet after physical button confirmation.
    DeleteWallet {
        wallet_id: String,
    },
    /// Process an air-gap `OfflineDataPackage` imported from MicroSD
    /// (`starlab_import/`) and emit an outbound package for `starlab_export/`.
    SdImportPackage {
        package: OfflineDataPackage,
    },
}

/// Public wallet summary returned by `list_wallets`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSummary {
    pub wallet_id: String,
    pub curve: CurveKind,
    pub participant_index: u16,
    pub threshold: u16,
    pub total: u16,
    pub group_pubkey_hex: String,
}

/// Derived address record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressInfo {
    pub chain: String,
    pub path: String,
    pub address: String,
}

/// Structured machine-readable error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HwErrorCode {
    InvalidJson,
    InvalidRequest,
    PinNotSet,
    Locked,
    WrongPin,
    VaultWiped,
    WalletNotFound,
    WalletExists,
    SessionNotFound,
    UserRejected,
    CryptoError,
    StorageError,
    UnsupportedCurve,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HwErrorBody {
    pub code: HwErrorCode,
    pub message: String,
}

/// Top-level NDJSON response from ESP32 -> host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<HwEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<HwErrorBody>,
}

impl HwResponse {
    pub fn ok(id: Option<u64>, event: HwEvent) -> Self {
        Self {
            id,
            ok: true,
            data: Some(event),
            error: None,
        }
    }

    pub fn err(id: Option<u64>, code: HwErrorCode, message: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            data: None,
            error: Some(HwErrorBody {
                code,
                message: message.into(),
            }),
        }
    }

    pub fn to_ndjson_line(&self) -> String {
        let mut s = serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"ok":false,"error":{"code":"crypto_error","message":"serialize failed"}}"#.to_string());
        s.push('\n');
        s
    }
}

/// Successful response payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum HwEvent {
    Pong {
        protocol_version: u32,
    },
    DeviceInfo {
        protocol_version: u32,
        firmware_version: String,
        board: String,
        chip: String,
        curves: Vec<String>,
        pin_configured: bool,
        unlocked: bool,
        remaining_pin_attempts: u8,
        wallet_count: usize,
    },
    PinConfigured,
    Unlocked,
    Locked,
    Wallets {
        wallets: Vec<WalletSummary>,
    },
    Address {
        wallet_id: String,
        chain: String,
        account: u32,
        path: String,
        address: String,
    },
    DkgInitialized {
        wallet_id: String,
        curve: CurveKind,
        participant_index: u16,
        threshold: u16,
        total: u16,
    },
    DkgRound1 {
        wallet_id: String,
        curve: CurveKind,
        participant_index: u16,
        /// Hex-encoded FROST binary `round1::Package::serialize()`.
        package_hex: String,
    },
    DkgRound2 {
        wallet_id: String,
        curve: CurveKind,
        participant_index: u16,
        /// `recipient_index -> hex(FROST binary round2::Package::serialize())`.
        #[serde(with = "u16_map")]
        packages: BTreeMap<u16, String>,
    },
    DkgFinalized {
        wallet_id: String,
        curve: CurveKind,
        participant_index: u16,
        group_pubkey_hex: String,
        public_key_package_hex: String,
        addresses: Vec<AddressInfo>,
    },
    SignCommitment {
        session_id: String,
        wallet_id: String,
        curve: CurveKind,
        chain: String,
        account: u32,
        path: String,
        address: String,
        participant_index: u16,
        /// Hex-encoded FROST binary `SigningCommitments::serialize()`.
        commitment_hex: String,
    },
    SignShare {
        session_id: String,
        wallet_id: String,
        participant_index: u16,
        /// Hex-encoded FROST binary `SignatureShare::serialize()`.
        share_hex: String,
    },
    WalletDeleted {
        wallet_id: String,
    },
    SdExportPackage {
        package: OfflineDataPackage,
    },
}
