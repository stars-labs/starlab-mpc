//! Core hardware-share state machine ([`HardwareShareEngine`]).
//!
//! Runs identically on real ESP32 firmware (`esp_hal.rs`) and host simulators
//! (`SimHal`). All FROST randomness is sourced from [`HardwareHal::fill_random`]
//! via [`HalFrostRng`], secret shares are persisted only as AES-256-GCM
//! ciphertext in [`NvsVault`], and Round 2 signing strictly requires physical
//! confirmation via [`HardwareHal::confirm_signing`].

use crate::SLHW_PROTOCOL_VERSION;
use crate::airgap::{AirGapEnvelope, OfflineDataPackage};
use crate::hal::{HalFrostRng, HardwareHal};
use crate::protocol::{
    AddressInfo, CurveKind, HwCommand, HwErrorCode, HwEvent, HwRequest, HwResponse,
    SignDisplayInfo,
};
use crate::vault::{NvsVault, StoredWallet, VaultError};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use frost_ed25519 as ed;
use frost_secp256k1_tr as secp;
use sha2::{Digest, Sha256};
use starlab_core::accounts::{account_addresses, address_for_chain, standard_path};
use starlab_core::hd_derivation::{ChainCode, DerivationPath, derive_child_key_path};
use std::collections::BTreeMap;

/// Maximum concurrent in-RAM signing sessions (guards MCU SRAM).
const MAX_PENDING_SIGN_SESSIONS: usize = 4;

enum DkgCurveState {
    Ed25519 {
        r1_secret: Option<ed::keys::dkg::round1::SecretPackage>,
        r2_secret: Option<ed::keys::dkg::round2::SecretPackage>,
        r1_peers: BTreeMap<ed::Identifier, ed::keys::dkg::round1::Package>,
    },
    Secp256k1 {
        r1_secret: Option<secp::keys::dkg::round1::SecretPackage>,
        r2_secret: Option<secp::keys::dkg::round2::SecretPackage>,
        r1_peers: BTreeMap<secp::Identifier, secp::keys::dkg::round1::Package>,
    },
}

struct ActiveDkg {
    wallet_id: String,
    curve: CurveKind,
    participant_index: u16,
    threshold: u16,
    total: u16,
    state: DkgCurveState,
}

enum PendingSignCurve {
    Ed25519 {
        nonces: ed::round1::SigningNonces,
        key_package: ed::keys::KeyPackage,
    },
    Secp256k1 {
        nonces: secp::round1::SigningNonces,
        key_package: secp::keys::KeyPackage,
    },
}

type EngineResult<T> = Result<T, (HwErrorCode, String)>;

struct CommitOutcome {
    path: String,
    address: String,
    commitment_bytes: Vec<u8>,
    curve_state: PendingSignCurve,
}

struct PendingSignSession {
    wallet_id: String,
    participant_index: u16,
    chain: String,
    path: String,
    address: String,
    curve_state: PendingSignCurve,
}

/// Transport-agnostic ESP32 hardware MPC share engine.
pub struct HardwareShareEngine<H: HardwareHal> {
    hal: H,
    vault: NvsVault,
    active_dkg: Option<ActiveDkg>,
    pending_sign: BTreeMap<String, PendingSignSession>,
    session_counter: u64,
}

impl<H: HardwareHal> HardwareShareEngine<H> {
    pub fn new(mut hal: H) -> Self {
        hal.show_status("Starlab Key", "Ready (Locked)");
        Self {
            hal,
            vault: NvsVault::new(),
            active_dkg: None,
            pending_sign: BTreeMap::new(),
            session_counter: 0,
        }
    }

    pub fn hal(&self) -> &H {
        &self.hal
    }

    pub fn hal_mut(&mut self) -> &mut H {
        &mut self.hal
    }

    /// Process a single NDJSON request line and return an NDJSON response line.
    pub fn handle_line(&mut self, line: &str) -> String {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return HwResponse::err(None, HwErrorCode::InvalidJson, "empty request line")
                .to_ndjson_line();
        }
        match serde_json::from_str::<HwRequest>(trimmed) {
            Ok(req) => self.handle_request(req).to_ndjson_line(),
            Err(e) => HwResponse::err(None, HwErrorCode::InvalidJson, format!("invalid JSON: {e}"))
                .to_ndjson_line(),
        }
    }

    /// Execute one structured [`HwRequest`].
    pub fn handle_request(&mut self, req: HwRequest) -> HwResponse {
        let id = req.id;
        match self.dispatch(req.command) {
            Ok(ev) => HwResponse::ok(id, ev),
            Err((code, msg)) => HwResponse::err(id, code, msg),
        }
    }

    fn dispatch(&mut self, cmd: HwCommand) -> Result<HwEvent, (HwErrorCode, String)> {
        match cmd {
            HwCommand::Ping => Ok(HwEvent::Pong {
                protocol_version: SLHW_PROTOCOL_VERSION,
            }),
            HwCommand::DeviceInfo => self.cmd_device_info(),
            HwCommand::SetPin { pin } => self.cmd_set_pin(&pin),
            HwCommand::Unlock { pin } => self.cmd_unlock(&pin),
            HwCommand::Lock => self.cmd_lock(),
            HwCommand::ListWallets => self.cmd_list_wallets(),
            HwCommand::DeriveAddress {
                wallet_id,
                chain,
                account,
            } => self.cmd_derive_address(&wallet_id, &chain, account),
            HwCommand::DkgInit {
                wallet_id,
                curve,
                participant_index,
                threshold,
                total,
            } => self.cmd_dkg_init(wallet_id, curve, participant_index, threshold, total),
            HwCommand::DkgRound1 { wallet_id } => self.cmd_dkg_round1(&wallet_id),
            HwCommand::DkgRound2 {
                wallet_id,
                peer_round1,
            } => self.cmd_dkg_round2(&wallet_id, peer_round1),
            HwCommand::DkgFinalize {
                wallet_id,
                peer_round2,
            } => self.cmd_dkg_finalize(&wallet_id, peer_round2),
            HwCommand::SignCommit {
                wallet_id,
                chain,
                account,
            } => self.cmd_sign_commit(&wallet_id, &chain, account),
            HwCommand::SignShare {
                session_id,
                message_hex,
                commitments,
                display,
            } => self.cmd_sign_share(&session_id, &message_hex, commitments, display),
            HwCommand::DeleteWallet { wallet_id } => self.cmd_delete_wallet(&wallet_id),
            HwCommand::SdImportPackage { package } => self.cmd_sd_import(package),
        }
    }

    fn require_unlocked(&self) -> Result<(), (HwErrorCode, String)> {
        if !self.vault.is_unlocked() {
            return Err((HwErrorCode::Locked, "device is locked — send unlock first".into()));
        }
        Ok(())
    }

    fn cmd_device_info(&self) -> Result<HwEvent, (HwErrorCode, String)> {
        let pin_configured = self.vault.is_pin_configured(&self.hal).map_err(map_vault_err)?;
        let remaining_pin_attempts = self
            .vault
            .remaining_attempts(&self.hal)
            .map_err(map_vault_err)?;
        let wallet_count = self
            .vault
            .list_stored_wallets(&self.hal)
            .map_err(map_vault_err)?
            .len();

        Ok(HwEvent::DeviceInfo {
            protocol_version: SLHW_PROTOCOL_VERSION,
            firmware_version: self.hal.firmware_version().to_string(),
            board: self.hal.board_name().to_string(),
            chip: self.hal.chip_name().to_string(),
            curves: vec!["ed25519".into(), "secp256k1".into()],
            pin_configured,
            unlocked: self.vault.is_unlocked(),
            remaining_pin_attempts,
            wallet_count,
        })
    }

    fn cmd_set_pin(&mut self, pin: &str) -> Result<HwEvent, (HwErrorCode, String)> {
        self.vault.set_pin(&mut self.hal, pin).map_err(map_vault_err)?;
        self.hal.show_status("Starlab Key", "PIN Set (Unlocked)");
        Ok(HwEvent::PinConfigured)
    }

    fn cmd_unlock(&mut self, pin: &str) -> Result<HwEvent, (HwErrorCode, String)> {
        self.vault.unlock(&mut self.hal, pin).map_err(map_vault_err)?;
        self.hal.show_status("Starlab Key", "Unlocked");
        Ok(HwEvent::Unlocked)
    }

    fn cmd_lock(&mut self) -> Result<HwEvent, (HwErrorCode, String)> {
        self.vault.lock();
        // Drop all sensitive ephemeral state (DKG secrets + single-use signing nonces).
        self.active_dkg = None;
        self.pending_sign.clear();
        self.hal.show_status("Starlab Key", "Locked");
        Ok(HwEvent::Locked)
    }

    fn cmd_list_wallets(&self) -> Result<HwEvent, (HwErrorCode, String)> {
        let wallets = self
            .vault
            .list_stored_wallets(&self.hal)
            .map_err(map_vault_err)?
            .into_iter()
            .map(|w| w.summary())
            .collect();
        Ok(HwEvent::Wallets { wallets })
    }

    fn cmd_derive_address(
        &self,
        wallet_id: &str,
        chain: &str,
        account: u32,
    ) -> Result<HwEvent, (HwErrorCode, String)> {
        let meta = self
            .vault
            .load_wallet_meta(&self.hal, wallet_id)
            .map_err(map_vault_err)?;
        let group_bytes = hex::decode(&meta.group_pubkey_hex)
            .map_err(|e| (HwErrorCode::CryptoError, format!("bad stored pubkey: {e}")))?;

        let path = standard_path(chain, account).ok_or_else(|| {
            (
                HwErrorCode::InvalidRequest,
                format!("unsupported chain '{chain}'"),
            )
        })?;
        let child_vk =
            starlab_core::accounts::account_verifying_key(chain, meta.curve.as_str(), &group_bytes, account)
                .map_err(|e| (HwErrorCode::UnsupportedCurve, e.to_string()))?;
        let address = address_for_chain(chain, meta.curve.as_str(), &child_vk)
            .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;

        Ok(HwEvent::Address {
            wallet_id: wallet_id.to_string(),
            chain: chain.to_ascii_lowercase(),
            account,
            path,
            address,
        })
    }

    fn cmd_dkg_init(
        &mut self,
        wallet_id: String,
        curve: CurveKind,
        participant_index: u16,
        threshold: u16,
        total: u16,
    ) -> Result<HwEvent, (HwErrorCode, String)> {
        self.require_unlocked()?;
        if wallet_id.is_empty() || wallet_id.len() > 64 {
            return Err((HwErrorCode::InvalidRequest, "wallet_id must be 1..=64 chars".into()));
        }
        if threshold < 2 || threshold > total || participant_index == 0 || participant_index > total {
            return Err((
                HwErrorCode::InvalidRequest,
                format!(
                    "invalid threshold params: index={participant_index}, t={threshold}, n={total}"
                ),
            ));
        }
        if self.vault.load_wallet_meta(&self.hal, &wallet_id).is_ok() {
            return Err((
                HwErrorCode::WalletExists,
                format!("wallet '{wallet_id}' already exists"),
            ));
        }

        let state = match curve {
            CurveKind::Ed25519 => DkgCurveState::Ed25519 {
                r1_secret: None,
                r2_secret: None,
                r1_peers: BTreeMap::new(),
            },
            CurveKind::Secp256k1 => DkgCurveState::Secp256k1 {
                r1_secret: None,
                r2_secret: None,
                r1_peers: BTreeMap::new(),
            },
        };

        self.hal
            .show_status("DKG Init", &format!("{wallet_id} ({threshold}/{total})"));
        self.active_dkg = Some(ActiveDkg {
            wallet_id: wallet_id.clone(),
            curve,
            participant_index,
            threshold,
            total,
            state,
        });

        Ok(HwEvent::DkgInitialized {
            wallet_id,
            curve,
            participant_index,
            threshold,
            total,
        })
    }

    fn cmd_dkg_round1(&mut self, wallet_id: &str) -> Result<HwEvent, (HwErrorCode, String)> {
        self.require_unlocked()?;
        let dkg = self.active_dkg.as_mut().ok_or_else(|| {
            (HwErrorCode::SessionNotFound, "no active DKG session".into())
        })?;
        if dkg.wallet_id != wallet_id {
            return Err((
                HwErrorCode::SessionNotFound,
                format!("active DKG is '{}', not '{wallet_id}'", dkg.wallet_id),
            ));
        }

        let mut rng = HalFrostRng::new(&mut self.hal);
        let package_bytes = match &mut dkg.state {
            DkgCurveState::Ed25519 { r1_secret, .. } => {
                let id = ed::Identifier::try_from(dkg.participant_index)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let (secret, pkg) =
                    ed::keys::dkg::part1(id, dkg.total, dkg.threshold, &mut rng)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                *r1_secret = Some(secret);
                pkg.serialize()
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?
            }
            DkgCurveState::Secp256k1 { r1_secret, .. } => {
                let id = secp::Identifier::try_from(dkg.participant_index)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let (secret, pkg) =
                    secp::keys::dkg::part1(id, dkg.total, dkg.threshold, &mut rng)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                *r1_secret = Some(secret);
                pkg.serialize()
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?
            }
        };

        let curve = dkg.curve;
        let participant_index = dkg.participant_index;
        self.hal.show_status("DKG Round 1", "Sent commitment");

        Ok(HwEvent::DkgRound1 {
            wallet_id: wallet_id.to_string(),
            curve,
            participant_index,
            package_hex: hex::encode(package_bytes),
        })
    }

    fn cmd_dkg_round2(
        &mut self,
        wallet_id: &str,
        peer_round1: BTreeMap<u16, String>,
    ) -> Result<HwEvent, (HwErrorCode, String)> {
        self.require_unlocked()?;
        let dkg = self.active_dkg.as_mut().ok_or_else(|| {
            (HwErrorCode::SessionNotFound, "no active DKG session".into())
        })?;
        if dkg.wallet_id != wallet_id {
            return Err((
                HwErrorCode::SessionNotFound,
                format!("active DKG is '{}', not '{wallet_id}'", dkg.wallet_id),
            ));
        }
        let expected_peers = usize::from(dkg.total - 1);
        if peer_round1.len() != expected_peers || peer_round1.contains_key(&dkg.participant_index) {
            return Err((
                HwErrorCode::InvalidRequest,
                format!(
                    "expected {expected_peers} peer round1 packages (excluding self #{}), got {}",
                    dkg.participant_index,
                    peer_round1.len()
                ),
            ));
        }

        let mut out_packages = BTreeMap::new();
        match &mut dkg.state {
            DkgCurveState::Ed25519 {
                r1_secret,
                r2_secret,
                r1_peers,
            } => {
                let mut peers = BTreeMap::new();
                for (idx, pkg_hex) in &peer_round1 {
                    let id = ed::Identifier::try_from(*idx)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let pkg = decode_frost_hex(pkg_hex, ed::keys::dkg::round1::Package::deserialize)?;
                    peers.insert(id, pkg);
                }
                let secret = r1_secret.take().ok_or_else(|| {
                    (HwErrorCode::InvalidRequest, "dkg_round1 must run before dkg_round2".into())
                })?;
                let (secret2, r2_map) = ed::keys::dkg::part2(secret, &peers)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                *r1_peers = peers;
                *r2_secret = Some(secret2);

                for (recipient_id, pkg) in r2_map {
                    let idx = (1..=dkg.total)
                        .find(|i| ed::Identifier::try_from(*i).ok() == Some(recipient_id))
                        .ok_or_else(|| {
                            (HwErrorCode::CryptoError, "unknown ed25519 recipient".into())
                        })?;
                    let bytes = pkg
                        .serialize()
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    out_packages.insert(idx, hex::encode(bytes));
                }
            }
            DkgCurveState::Secp256k1 {
                r1_secret,
                r2_secret,
                r1_peers,
            } => {
                let mut peers = BTreeMap::new();
                for (idx, pkg_hex) in &peer_round1 {
                    let id = secp::Identifier::try_from(*idx)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let pkg = decode_frost_hex(pkg_hex, secp::keys::dkg::round1::Package::deserialize)?;
                    peers.insert(id, pkg);
                }
                let secret = r1_secret.take().ok_or_else(|| {
                    (HwErrorCode::InvalidRequest, "dkg_round1 must run before dkg_round2".into())
                })?;
                let (secret2, r2_map) = secp::keys::dkg::part2(secret, &peers)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                *r1_peers = peers;
                *r2_secret = Some(secret2);

                for (recipient_id, pkg) in r2_map {
                    let idx = (1..=dkg.total)
                        .find(|i| secp::Identifier::try_from(*i).ok() == Some(recipient_id))
                        .ok_or_else(|| {
                            (HwErrorCode::CryptoError, "unknown secp256k1 recipient".into())
                        })?;
                    let bytes = pkg
                        .serialize()
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    out_packages.insert(idx, hex::encode(bytes));
                }
            }
        }

        let curve = dkg.curve;
        let participant_index = dkg.participant_index;
        self.hal.show_status("DKG Round 2", "Sent peer shares");

        Ok(HwEvent::DkgRound2 {
            wallet_id: wallet_id.to_string(),
            curve,
            participant_index,
            packages: out_packages,
        })
    }

    fn cmd_dkg_finalize(
        &mut self,
        wallet_id: &str,
        peer_round2: BTreeMap<u16, String>,
    ) -> Result<HwEvent, (HwErrorCode, String)> {
        self.require_unlocked()?;
        let Some(dkg) = self.active_dkg.take() else {
            return Err((HwErrorCode::SessionNotFound, "no active DKG session".into()));
        };
        if dkg.wallet_id != wallet_id {
            self.active_dkg = Some(dkg);
            return Err((
                HwErrorCode::SessionNotFound,
                format!("active DKG is not '{wallet_id}'"),
            ));
        }

        let expected_peers = usize::from(dkg.total - 1);
        if peer_round2.len() != expected_peers || peer_round2.contains_key(&dkg.participant_index) {
            return Err((
                HwErrorCode::InvalidRequest,
                format!("expected {expected_peers} peer round2 packages"),
            ));
        }

        let (group_vk_bytes, pub_pkg_json, key_pkg_json) = match dkg.state {
            DkgCurveState::Ed25519 {
                r2_secret,
                r1_peers,
                ..
            } => {
                let secret2 = r2_secret.ok_or_else(|| {
                    (HwErrorCode::InvalidRequest, "dkg_round2 must run before dkg_finalize".into())
                })?;
                let mut r2_peers = BTreeMap::new();
                for (idx, pkg_hex) in &peer_round2 {
                    let id = ed::Identifier::try_from(*idx)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let pkg = decode_frost_hex(pkg_hex, ed::keys::dkg::round2::Package::deserialize)?;
                    r2_peers.insert(id, pkg);
                }
                let (key_pkg, pub_pkg) = ed::keys::dkg::part3(&secret2, &r1_peers, &r2_peers)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let vk = pub_pkg
                    .verifying_key()
                    .serialize()
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let pub_json = serde_json::to_vec(&pub_pkg)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let key_json = serde_json::to_vec(&key_pkg)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                (vk, pub_json, key_json)
            }
            DkgCurveState::Secp256k1 {
                r2_secret,
                r1_peers,
                ..
            } => {
                let secret2 = r2_secret.ok_or_else(|| {
                    (HwErrorCode::InvalidRequest, "dkg_round2 must run before dkg_finalize".into())
                })?;
                let mut r2_peers = BTreeMap::new();
                for (idx, pkg_hex) in &peer_round2 {
                    let id = secp::Identifier::try_from(*idx)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let pkg =
                        decode_frost_hex(pkg_hex, secp::keys::dkg::round2::Package::deserialize)?;
                    r2_peers.insert(id, pkg);
                }
                let (key_pkg, pub_pkg) = secp::keys::dkg::part3(&secret2, &r1_peers, &r2_peers)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let vk = pub_pkg
                    .verifying_key()
                    .serialize()
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let pub_json = serde_json::to_vec(&pub_pkg)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let key_json = serde_json::to_vec(&key_pkg)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                (vk, pub_json, key_json)
            }
        };

        let group_pubkey_hex = hex::encode(&group_vk_bytes);
        self.vault
            .save_wallet(
                &mut self.hal,
                &dkg.wallet_id,
                dkg.curve,
                dkg.participant_index,
                dkg.threshold,
                dkg.total,
                group_pubkey_hex.clone(),
                &pub_pkg_json,
                &key_pkg_json,
            )
            .map_err(map_vault_err)?;

        let addresses = account_addresses(dkg.curve.as_str(), &group_vk_bytes, 0)
            .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?
            .into_iter()
            .map(|(chain, path, address)| AddressInfo {
                chain,
                path,
                address,
            })
            .collect();

        self.hal
            .show_status("DKG Saved", &format!("{} (t={}/{})", dkg.wallet_id, dkg.threshold, dkg.total));

        Ok(HwEvent::DkgFinalized {
            wallet_id: dkg.wallet_id,
            curve: dkg.curve,
            participant_index: dkg.participant_index,
            group_pubkey_hex,
            public_key_package_hex: hex::encode(&pub_pkg_json),
            addresses,
        })
    }

    fn cmd_sign_commit(
        &mut self,
        wallet_id: &str,
        chain: &str,
        account: u32,
    ) -> Result<HwEvent, (HwErrorCode, String)> {
        self.require_unlocked()?;
        if self.pending_sign.len() >= MAX_PENDING_SIGN_SESSIONS {
            return Err((
                HwErrorCode::InvalidRequest,
                "too many pending signing sessions — complete or lock device first".into(),
            ));
        }

        let record = self
            .vault
            .load_wallet_meta(&self.hal, wallet_id)
            .map_err(map_vault_err)?;
        let key_json = self.vault.decrypt_key_package(&record).map_err(map_vault_err)?;
        let pub_json = BASE64
            .decode(&record.public_key_package_b64)
            .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;

        let chain_norm = chain.to_ascii_lowercase();
        let CommitOutcome {
            path: path_str,
            address,
            commitment_bytes,
            curve_state,
        } = self.derive_and_commit(&record, &chain_norm, account, &key_json, &pub_json)?;

        self.session_counter = self.session_counter.wrapping_add(1);
        let session_id = format!("s-{:04}", self.session_counter);

        self.pending_sign.insert(
            session_id.clone(),
            PendingSignSession {
                wallet_id: record.wallet_id.clone(),
                participant_index: record.participant_index,
                chain: chain_norm.clone(),
                path: path_str.clone(),
                address: address.clone(),
                curve_state,
            },
        );
        self.hal
            .show_status("Sign Round 1", &format!("{} #{account}", chain_norm));

        Ok(HwEvent::SignCommitment {
            session_id,
            wallet_id: record.wallet_id,
            curve: record.curve,
            chain: chain_norm,
            account,
            path: path_str,
            address,
            participant_index: record.participant_index,
            commitment_hex: hex::encode(commitment_bytes),
        })
    }

    fn derive_and_commit(
        &mut self,
        record: &StoredWallet,
        chain_norm: &str,
        account: u32,
        key_json: &[u8],
        pub_json: &[u8],
    ) -> EngineResult<CommitOutcome> {
        let mut rng = HalFrostRng::new(&mut self.hal);
        let use_raw = chain_norm == "raw";

        match record.curve {
            CurveKind::Ed25519 => {
                let root_kp: ed::keys::KeyPackage = serde_json::from_slice(key_json)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let root_pub: ed::keys::PublicKeyPackage = serde_json::from_slice(pub_json)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let (kp, pub_pkg, path_str, addr) = if use_raw {
                    let vk = root_pub
                        .verifying_key()
                        .serialize()
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    (root_kp, root_pub, "m".to_string(), hex::encode(vk))
                } else {
                    let path_s = standard_path(chain_norm, account).ok_or_else(|| {
                        (
                            HwErrorCode::InvalidRequest,
                            format!("unsupported chain '{chain_norm}'"),
                        )
                    })?;
                    let path = DerivationPath::parse(&path_s)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let root_vk = root_pub
                        .verifying_key()
                        .serialize()
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let cc = ChainCode::from_group_key(&root_vk);
                    let derived = derive_child_key_path::<ed::Ed25519Sha512>(
                        &root_kp, &root_pub, &cc, &path,
                    )
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let child_vk = derived
                        .public_key_package
                        .verifying_key()
                        .serialize()
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let addr = address_for_chain(chain_norm, "ed25519", &child_vk)
                        .map_err(|e| (HwErrorCode::UnsupportedCurve, e.to_string()))?;
                    (derived.key_package, derived.public_key_package, path_s, addr)
                };
                let _ = pub_pkg;
                let (nonces, commitments) = ed::round1::commit(kp.signing_share(), &mut rng);
                let c_bytes = commitments
                    .serialize()
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                Ok(CommitOutcome {
                    path: path_str,
                    address: addr,
                    commitment_bytes: c_bytes,
                    curve_state: PendingSignCurve::Ed25519 {
                        nonces,
                        key_package: kp,
                    },
                })
            }
            CurveKind::Secp256k1 => {
                let root_kp: secp::keys::KeyPackage = serde_json::from_slice(key_json)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let root_pub: secp::keys::PublicKeyPackage = serde_json::from_slice(pub_json)
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                let (kp, pub_pkg, path_str, addr) = if use_raw {
                    let vk = root_pub
                        .verifying_key()
                        .serialize()
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    (root_kp, root_pub, "m".to_string(), hex::encode(vk))
                } else {
                    let path_s = standard_path(chain_norm, account).ok_or_else(|| {
                        (
                            HwErrorCode::InvalidRequest,
                            format!("unsupported chain '{chain_norm}'"),
                        )
                    })?;
                    let path = DerivationPath::parse(&path_s)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let root_vk = root_pub
                        .verifying_key()
                        .serialize()
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let cc = ChainCode::from_group_key(&root_vk);
                    let derived = derive_child_key_path::<secp::Secp256K1Sha256TR>(
                        &root_kp, &root_pub, &cc, &path,
                    )
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let child_vk = derived
                        .public_key_package
                        .verifying_key()
                        .serialize()
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let addr = address_for_chain(chain_norm, "secp256k1", &child_vk)
                        .map_err(|e| (HwErrorCode::UnsupportedCurve, e.to_string()))?;
                    (derived.key_package, derived.public_key_package, path_s, addr)
                };
                let _ = pub_pkg;
                let (nonces, commitments) = secp::round1::commit(kp.signing_share(), &mut rng);
                let c_bytes = commitments
                    .serialize()
                    .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                Ok(CommitOutcome {
                    path: path_str,
                    address: addr,
                    commitment_bytes: c_bytes,
                    curve_state: PendingSignCurve::Secp256k1 {
                        nonces,
                        key_package: kp,
                    },
                })
            }
        }
    }

    fn cmd_sign_share(
        &mut self,
        session_id: &str,
        message_hex: &str,
        commitments: BTreeMap<u16, String>,
        mut display: SignDisplayInfo,
    ) -> Result<HwEvent, (HwErrorCode, String)> {
        self.require_unlocked()?;

        // CRITICAL SECURITY INVARIANT:
        // Remove the pending session FIRST so the single-use FROST SigningNonces
        // are ALWAYS consumed and dropped/zeroized upon leaving this scope —
        // even if validation fails or the user presses the physical REJECT button.
        let session = self.pending_sign.remove(session_id).ok_or_else(|| {
            (
                HwErrorCode::SessionNotFound,
                format!("signing session '{session_id}' not found or already consumed"),
            )
        })?;

        let message_bytes = hex::decode(message_hex.trim_start_matches("0x"))
            .map_err(|e| (HwErrorCode::InvalidRequest, format!("bad message_hex: {e}")))?;

        // Populate trusted, device-computed display metadata (host cannot spoof
        // which wallet, chain, derivation path, or message digest is being signed).
        display.wallet_id = session.wallet_id.clone();
        display.chain = session.chain.clone();
        display.path = session.path.clone();
        display.signer_address = session.address.clone();
        display.message_digest_hex = if message_bytes.len() == 32 {
            hex::encode(&message_bytes)
        } else {
            hex::encode(Sha256::digest(&message_bytes))
        };

        // Physical Clear-Signing confirmation gate.
        let approved = self
            .hal
            .confirm_signing(&display)
            .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
        if !approved {
            self.hal.show_status("Sign Rejected", "Nonce destroyed");
            return Err((
                HwErrorCode::UserRejected,
                "signing rejected via physical button (nonce destroyed)".into(),
            ));
        }

        let share_bytes = match session.curve_state {
            PendingSignCurve::Ed25519 {
                nonces,
                key_package,
            } => {
                let mut map = BTreeMap::new();
                for (idx, c_hex) in &commitments {
                    let id = ed::Identifier::try_from(*idx)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let c = decode_frost_hex(c_hex, ed::round1::SigningCommitments::deserialize)?;
                    map.insert(id, c);
                }
                let pkg = ed::SigningPackage::new(map, &message_bytes);
                let share = ed::round2::sign(&pkg, &nonces, &key_package)
                    .map_err(|e| (HwErrorCode::CryptoError, format!("frost ed25519 sign: {e:?}")))?;
                share.serialize()
            }
            PendingSignCurve::Secp256k1 {
                nonces,
                key_package,
            } => {
                let mut map = BTreeMap::new();
                for (idx, c_hex) in &commitments {
                    let id = secp::Identifier::try_from(*idx)
                        .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
                    let c =
                        decode_frost_hex(c_hex, secp::round1::SigningCommitments::deserialize)?;
                    map.insert(id, c);
                }
                let pkg = secp::SigningPackage::new(map, &message_bytes);
                let share = secp::round2::sign(&pkg, &nonces, &key_package).map_err(|e| {
                    (
                        HwErrorCode::CryptoError,
                        format!("frost secp256k1 sign: {e:?}"),
                    )
                })?;
                share.serialize()
            }
        };

        self.hal.show_status("Signed OK", &session.chain);
        Ok(HwEvent::SignShare {
            session_id: session_id.to_string(),
            wallet_id: session.wallet_id,
            participant_index: session.participant_index,
            share_hex: hex::encode(share_bytes),
        })
    }

    fn cmd_delete_wallet(&mut self, wallet_id: &str) -> Result<HwEvent, (HwErrorCode, String)> {
        self.require_unlocked()?;
        // Verify wallet exists before asking the user to press confirm.
        let _ = self
            .vault
            .load_wallet_meta(&self.hal, wallet_id)
            .map_err(map_vault_err)?;

        let confirmed = self
            .hal
            .confirm_destructive("DELETE SHARE?", wallet_id)
            .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;
        if !confirmed {
            return Err((
                HwErrorCode::UserRejected,
                "wallet deletion rejected via physical button".into(),
            ));
        }

        self.vault
            .delete_wallet(&mut self.hal, wallet_id)
            .map_err(map_vault_err)?;
        self.hal.show_status("Share Deleted", wallet_id);
        Ok(HwEvent::WalletDeleted {
            wallet_id: wallet_id.to_string(),
        })
    }

    fn cmd_sd_import(
        &mut self,
        package: OfflineDataPackage,
    ) -> Result<HwEvent, (HwErrorCode, String)> {
        if !package.verify_checksum() {
            return Err((
                HwErrorCode::InvalidRequest,
                "OfflineDataPackage SHA-256 checksum mismatch".into(),
            ));
        }
        let envelope: AirGapEnvelope = serde_json::from_slice(&package.data).map_err(|e| {
            (
                HwErrorCode::InvalidRequest,
                format!("invalid air-gap envelope JSON: {e}"),
            )
        })?;
        let AirGapEnvelope::Request { command } = envelope else {
            return Err((
                HwErrorCode::InvalidRequest,
                "expected AirGapEnvelope::Request".into(),
            ));
        };

        let event = self.dispatch(command)?;
        let out_bytes = serde_json::to_vec(&AirGapEnvelope::Response { event })
            .map_err(|e| (HwErrorCode::CryptoError, e.to_string()))?;

        let out_pkg = OfflineDataPackage::new(
            format!("{}-resp", package.package_id),
            format!("{}_response", package.operation_type),
            package.round,
            package.participant_id,
            package.timestamp,
            out_bytes,
        );
        Ok(HwEvent::SdExportPackage { package: out_pkg })
    }
}

/// Accept BOTH FROST's canonical binary `serialize()` encoding AND JSON encoding
/// (matching `core-wasm`'s `decode_wire` so CLI, TUI, Desktop, and Extension
/// peers interoperate seamlessly with ESP32 hardware shares).
fn decode_frost_hex<T, E>(
    hex_str: &str,
    frost_deserialize: impl FnOnce(&[u8]) -> Result<T, E>,
) -> Result<T, (HwErrorCode, String)>
where
    T: serde::de::DeserializeOwned,
    E: std::fmt::Display,
{
    let bytes = hex::decode(hex_str.trim_start_matches("0x"))
        .map_err(|e| (HwErrorCode::InvalidRequest, format!("invalid hex: {e}")))?;
    frost_deserialize(&bytes).or_else(|bin_err| {
        serde_json::from_slice(&bytes).map_err(|json_err| {
            (
                HwErrorCode::InvalidRequest,
                format!("package decode failed (binary: {bin_err}; json: {json_err})"),
            )
        })
    })
}

fn map_vault_err(e: VaultError) -> (HwErrorCode, String) {
    let code = match &e {
        VaultError::PinNotSet => HwErrorCode::PinNotSet,
        VaultError::Locked => HwErrorCode::Locked,
        VaultError::WrongPin { .. } => HwErrorCode::WrongPin,
        VaultError::Wiped => HwErrorCode::VaultWiped,
        VaultError::InvalidPinFormat => HwErrorCode::InvalidRequest,
        VaultError::WalletExists(_) => HwErrorCode::WalletExists,
        VaultError::WalletNotFound(_) => HwErrorCode::WalletNotFound,
        VaultError::Crypto(_) => HwErrorCode::CryptoError,
        VaultError::Storage(_) => HwErrorCode::StorageError,
    };
    (code, e.to_string())
}
