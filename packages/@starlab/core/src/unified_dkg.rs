//! Unified DKG module that runs FROST DKG for both ed25519 and secp256k1
//! simultaneously from a single root secret.
//!
//! Each participant generates one root secret, from which deterministic RNGs
//! are derived for each curve. The DKG protocol runs in parallel for both
//! curves, producing key packages for ed25519 (Solana) and secp256k1
//! (Ethereum/Bitcoin) from one shared entropy source.

use crate::ed25519::Ed25519Curve;
use crate::errors::{FrostError, Result};
use crate::hd_derivation::{ChainCode, DerivedKeys, derive_child_key};
use crate::keystore::{Keystore, MultiCurveKeystoreData};
use crate::root_secret::RootSecret;
use crate::secp256k1::Secp256k1Curve;
use crate::traits::FrostCurve;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Round 1 output containing packages for both curves.
#[derive(Serialize, Deserialize)]
pub struct UnifiedRound1Package {
    pub ed25519: String,    // hex-encoded JSON of ed25519 round1 package
    pub secp256k1: String,  // hex-encoded JSON of secp256k1 round1 package
}

/// Round 2 output containing packages for both curves, keyed by recipient.
#[derive(Serialize, Deserialize)]
pub struct UnifiedRound2Packages {
    pub ed25519: BTreeMap<u16, String>,    // participant_index -> hex-encoded package
    pub secp256k1: BTreeMap<u16, String>,  // participant_index -> hex-encoded package
}

/// Unified DKG state managing both curves from a single root secret.
pub struct UnifiedDkg {
    root_secret: RootSecret,

    // Ed25519 DKG state
    ed25519_round1_secret: Option<frost_ed25519::keys::dkg::round1::SecretPackage>,
    ed25519_round2_secret: Option<frost_ed25519::keys::dkg::round2::SecretPackage>,
    ed25519_key_package: Option<frost_ed25519::keys::KeyPackage>,
    ed25519_public_key_package: Option<frost_ed25519::keys::PublicKeyPackage>,
    ed25519_round1_packages: BTreeMap<frost_ed25519::Identifier, frost_ed25519::keys::dkg::round1::Package>,
    ed25519_round2_packages: BTreeMap<frost_ed25519::Identifier, frost_ed25519::keys::dkg::round2::Package>,

    // Secp256k1 DKG state
    secp256k1_round1_secret: Option<frost_secp256k1::keys::dkg::round1::SecretPackage>,
    secp256k1_round2_secret: Option<frost_secp256k1::keys::dkg::round2::SecretPackage>,
    secp256k1_key_package: Option<frost_secp256k1::keys::KeyPackage>,
    secp256k1_public_key_package: Option<frost_secp256k1::keys::PublicKeyPackage>,
    secp256k1_round1_packages: BTreeMap<frost_secp256k1::Identifier, frost_secp256k1::keys::dkg::round1::Package>,
    secp256k1_round2_packages: BTreeMap<frost_secp256k1::Identifier, frost_secp256k1::keys::dkg::round2::Package>,

    // HD derivation chain codes (set after finalization)
    ed25519_chain_code: Option<ChainCode>,
    secp256k1_chain_code: Option<ChainCode>,

    // Session metadata
    participant_index: u16,
    total: u16,
    threshold: u16,
    participant_indices: Vec<u16>,

    /// BIP-44-style account index: one root secret can yield multiple
    /// independent wallets per curve via the domain-separated derivation
    /// (`frost-dkg/v1/<curve>/<account>`). Defaults to 0.
    account: u32,
}

impl Default for UnifiedDkg {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedDkg {
    /// Create a new unified DKG instance with a fresh root secret.
    pub fn new() -> Self {
        Self::with_root_secret(RootSecret::generate())
    }

    /// Create a new unified DKG instance from an existing root secret.
    pub fn with_root_secret(root_secret: RootSecret) -> Self {
        Self {
            root_secret,
            ed25519_round1_secret: None,
            ed25519_round2_secret: None,
            ed25519_key_package: None,
            ed25519_public_key_package: None,
            ed25519_round1_packages: BTreeMap::new(),
            ed25519_round2_packages: BTreeMap::new(),
            secp256k1_round1_secret: None,
            secp256k1_round2_secret: None,
            secp256k1_key_package: None,
            secp256k1_public_key_package: None,
            secp256k1_round1_packages: BTreeMap::new(),
            secp256k1_round2_packages: BTreeMap::new(),
            ed25519_chain_code: None,
            secp256k1_chain_code: None,
            participant_index: 0,
            total: 0,
            threshold: 0,
            participant_indices: Vec::new(),
            account: 0,
        }
    }

    /// Initialize DKG parameters (account 0).
    pub fn init_dkg(&mut self, participant_index: u16, total: u16, threshold: u16) {
        self.init_dkg_with_account(participant_index, total, threshold, 0);
    }

    /// Initialize DKG parameters for a specific BIP-44-style account index.
    ///
    /// All participants of one ceremony MUST agree on the same `account` (it
    /// selects which domain-separated derivation each node uses); a mismatch
    /// yields incompatible round-1 packages and DKG fails.
    pub fn init_dkg_with_account(
        &mut self,
        participant_index: u16,
        total: u16,
        threshold: u16,
        account: u32,
    ) {
        self.participant_index = participant_index;
        self.total = total;
        self.threshold = threshold;
        self.participant_indices = (1..=total).collect();
        self.account = account;
    }

    /// The account index this DKG derives under.
    pub fn account(&self) -> u32 {
        self.account
    }

    /// Get reference to the root secret.
    pub fn root_secret(&self) -> &RootSecret {
        &self.root_secret
    }

    /// Generate round 1 packages for both curves using RNGs derived from the root secret.
    pub fn generate_round1(&mut self) -> Result<UnifiedRound1Package> {
        // Derive deterministic, account-scoped RNGs from the root secret.
        let mut ed_rng = self.root_secret.derive_ed25519_rng_for_account(self.account)?;
        let mut secp_rng = self.root_secret.derive_secp256k1_rng_for_account(self.account)?;

        // Ed25519 round 1
        let ed_identifier = Ed25519Curve::identifier_from_u16(self.participant_index)?;
        let (ed_r1_secret, ed_r1_package) = frost_ed25519::keys::dkg::part1(
            ed_identifier,
            self.total,
            self.threshold,
            &mut ed_rng,
        ).map_err(|e| FrostError::DkgError(e.to_string()))?;
        self.ed25519_round1_secret = Some(ed_r1_secret);

        // Secp256k1 round 1
        let secp_identifier = Secp256k1Curve::identifier_from_u16(self.participant_index)?;
        let (secp_r1_secret, secp_r1_package) = frost_secp256k1::keys::dkg::part1(
            secp_identifier,
            self.total,
            self.threshold,
            &mut secp_rng,
        ).map_err(|e| FrostError::DkgError(e.to_string()))?;
        self.secp256k1_round1_secret = Some(secp_r1_secret);

        // Serialize packages
        let ed_json = serde_json::to_string(&ed_r1_package)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let secp_json = serde_json::to_string(&secp_r1_package)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;

        Ok(UnifiedRound1Package {
            ed25519: hex::encode(ed_json),
            secp256k1: hex::encode(secp_json),
        })
    }

    /// Add a round 1 package from another participant for both curves.
    pub fn add_round1_package(&mut self, participant_index: u16, package: &UnifiedRound1Package) -> Result<()> {
        // Ed25519
        let ed_json = hex::decode(&package.ed25519)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let ed_pkg: frost_ed25519::keys::dkg::round1::Package = serde_json::from_slice(&ed_json)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let ed_id = Ed25519Curve::identifier_from_u16(participant_index)?;
        self.ed25519_round1_packages.insert(ed_id, ed_pkg);

        // Secp256k1
        let secp_json = hex::decode(&package.secp256k1)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let secp_pkg: frost_secp256k1::keys::dkg::round1::Package = serde_json::from_slice(&secp_json)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let secp_id = Secp256k1Curve::identifier_from_u16(participant_index)?;
        self.secp256k1_round1_packages.insert(secp_id, secp_pkg);

        Ok(())
    }

    /// Check if round 2 can start (all other participants' round 1 packages received).
    pub fn can_start_round2(&self) -> bool {
        let expected = (self.total - 1) as usize;
        self.ed25519_round1_packages.len() >= expected
            && self.secp256k1_round1_packages.len() >= expected
            && self.ed25519_round1_secret.is_some()
            && self.secp256k1_round1_secret.is_some()
    }

    /// Generate round 2 packages for both curves.
    pub fn generate_round2(&mut self) -> Result<UnifiedRound2Packages> {
        let self_ed_id = Ed25519Curve::identifier_from_u16(self.participant_index)?;
        let self_secp_id = Secp256k1Curve::identifier_from_u16(self.participant_index)?;

        // Filter out own round 1 packages (frost part2 expects only others')
        let ed_r1_others: BTreeMap<_, _> = self.ed25519_round1_packages.iter()
            .filter(|(id, _)| **id != self_ed_id)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();
        let secp_r1_others: BTreeMap<_, _> = self.secp256k1_round1_packages.iter()
            .filter(|(id, _)| **id != self_secp_id)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();

        // Ed25519 round 2
        let ed_r1_secret = self.ed25519_round1_secret.clone()
            .ok_or_else(|| FrostError::InvalidState("Ed25519 round 1 secret not available".into()))?;
        let (ed_r2_secret, ed_r2_packages) = frost_ed25519::keys::dkg::part2(
            ed_r1_secret,
            &ed_r1_others,
        ).map_err(|e| FrostError::DkgError(e.to_string()))?;
        self.ed25519_round2_secret = Some(ed_r2_secret);

        // Secp256k1 round 2
        let secp_r1_secret = self.secp256k1_round1_secret.clone()
            .ok_or_else(|| FrostError::InvalidState("Secp256k1 round 1 secret not available".into()))?;
        let (secp_r2_secret, secp_r2_packages) = frost_secp256k1::keys::dkg::part2(
            secp_r1_secret,
            &secp_r1_others,
        ).map_err(|e| FrostError::DkgError(e.to_string()))?;
        self.secp256k1_round2_secret = Some(secp_r2_secret);

        // Serialize ed25519 round 2 packages
        let mut ed_map = BTreeMap::new();
        for (id, package) in ed_r2_packages {
            let id_bytes = id.serialize();
            let id_value = (id_bytes[30] as u16) << 8 | id_bytes[31] as u16;
            let pkg_json = serde_json::to_string(&package)
                .map_err(|e| FrostError::SerializationError(e.to_string()))?;
            ed_map.insert(id_value, hex::encode(pkg_json));
        }

        // Serialize secp256k1 round 2 packages
        let mut secp_map = BTreeMap::new();
        for (id, package) in secp_r2_packages {
            let id_bytes = id.serialize();
            let id_value = (id_bytes[30] as u16) << 8 | id_bytes[31] as u16;
            let pkg_json = serde_json::to_string(&package)
                .map_err(|e| FrostError::SerializationError(e.to_string()))?;
            secp_map.insert(id_value, hex::encode(pkg_json));
        }

        Ok(UnifiedRound2Packages {
            ed25519: ed_map,
            secp256k1: secp_map,
        })
    }

    /// Add a round 2 package from another participant for both curves.
    pub fn add_round2_package(&mut self, sender_index: u16, ed_hex: &str, secp_hex: &str) -> Result<()> {
        // Ed25519
        let ed_json = hex::decode(ed_hex)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let ed_pkg: frost_ed25519::keys::dkg::round2::Package = serde_json::from_slice(&ed_json)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let ed_id = Ed25519Curve::identifier_from_u16(sender_index)?;
        self.ed25519_round2_packages.insert(ed_id, ed_pkg);

        // Secp256k1
        let secp_json = hex::decode(secp_hex)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let secp_pkg: frost_secp256k1::keys::dkg::round2::Package = serde_json::from_slice(&secp_json)
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        let secp_id = Secp256k1Curve::identifier_from_u16(sender_index)?;
        self.secp256k1_round2_packages.insert(secp_id, secp_pkg);

        Ok(())
    }

    /// Check if DKG can be finalized.
    pub fn can_finalize(&self) -> bool {
        self.ed25519_round2_packages.len() >= (self.threshold - 1) as usize
            && self.secp256k1_round2_packages.len() >= (self.threshold - 1) as usize
            && self.ed25519_round2_secret.is_some()
            && self.secp256k1_round2_secret.is_some()
    }

    /// Finalize DKG for both curves, producing a multi-curve keystore.
    pub fn finalize_dkg(&mut self) -> Result<MultiCurveKeystoreData> {
        let self_ed_id = Ed25519Curve::identifier_from_u16(self.participant_index)?;
        let self_secp_id = Secp256k1Curve::identifier_from_u16(self.participant_index)?;

        // Filter out own round 1 packages for part3
        let ed_r1_others: BTreeMap<_, _> = self.ed25519_round1_packages.iter()
            .filter(|(id, _)| **id != self_ed_id)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();
        let secp_r1_others: BTreeMap<_, _> = self.secp256k1_round1_packages.iter()
            .filter(|(id, _)| **id != self_secp_id)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();

        // Finalize ed25519
        let ed_r2_secret = self.ed25519_round2_secret.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Ed25519 round 2 secret not available".into()))?;
        let (ed_key_pkg, ed_pub_pkg) = frost_ed25519::keys::dkg::part3(
            ed_r2_secret,
            &ed_r1_others,
            &self.ed25519_round2_packages,
        ).map_err(|e| FrostError::DkgError(e.to_string()))?;
        // Derive chain code from ed25519 group public key
        let ed_vk_bytes = Ed25519Curve::serialize_verifying_key(&Ed25519Curve::verifying_key(&ed_pub_pkg))?;
        self.ed25519_chain_code = Some(ChainCode::from_group_key(&ed_vk_bytes));
        self.ed25519_key_package = Some(ed_key_pkg.clone());
        self.ed25519_public_key_package = Some(ed_pub_pkg.clone());

        // Finalize secp256k1
        let secp_r2_secret = self.secp256k1_round2_secret.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Secp256k1 round 2 secret not available".into()))?;
        let (secp_key_pkg, secp_pub_pkg) = frost_secp256k1::keys::dkg::part3(
            secp_r2_secret,
            &secp_r1_others,
            &self.secp256k1_round2_packages,
        ).map_err(|e| FrostError::DkgError(e.to_string()))?;
        // Derive chain code from secp256k1 group public key
        let secp_vk_bytes = Secp256k1Curve::serialize_verifying_key(&Secp256k1Curve::verifying_key(&secp_pub_pkg))?;
        self.secp256k1_chain_code = Some(ChainCode::from_group_key(&secp_vk_bytes));
        self.secp256k1_key_package = Some(secp_key_pkg.clone());
        self.secp256k1_public_key_package = Some(secp_pub_pkg.clone());

        // Build individual keystore data for each curve
        let ed_keystore = Keystore::export_keystore::<Ed25519Curve>(
            &ed_key_pkg,
            &ed_pub_pkg,
            self.threshold,
            self.total,
            self.participant_index,
            self.participant_indices.clone(),
            "ed25519",
        )?;

        let secp_keystore = Keystore::export_keystore::<Secp256k1Curve>(
            &secp_key_pkg,
            &secp_pub_pkg,
            self.threshold,
            self.total,
            self.participant_index,
            self.participant_indices.clone(),
            "secp256k1",
        )?;

        Ok(MultiCurveKeystoreData {
            ed25519: ed_keystore,
            secp256k1: secp_keystore,
        })
    }

    /// Check if DKG is complete for both curves.
    pub fn is_dkg_complete(&self) -> bool {
        self.ed25519_key_package.is_some()
            && self.ed25519_public_key_package.is_some()
            && self.secp256k1_key_package.is_some()
            && self.secp256k1_public_key_package.is_some()
    }

    /// Get the ed25519 group public key (hex).
    pub fn get_ed25519_group_public_key(&self) -> Result<String> {
        let pub_pkg = self.ed25519_public_key_package.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Ed25519 DKG not complete".into()))?;
        let vk = Ed25519Curve::verifying_key(pub_pkg);
        let bytes = Ed25519Curve::serialize_verifying_key(&vk)?;
        Ok(hex::encode(bytes))
    }

    /// Get the secp256k1 group public key (hex).
    pub fn get_secp256k1_group_public_key(&self) -> Result<String> {
        let pub_pkg = self.secp256k1_public_key_package.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Secp256k1 DKG not complete".into()))?;
        let vk = Secp256k1Curve::verifying_key(pub_pkg);
        let bytes = Secp256k1Curve::serialize_verifying_key(&vk)?;
        Ok(hex::encode(bytes))
    }

    /// Get the Solana address (base58 ed25519 public key).
    pub fn get_solana_address(&self) -> Result<String> {
        let pub_pkg = self.ed25519_public_key_package.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Ed25519 DKG not complete".into()))?;
        let vk = Ed25519Curve::verifying_key(pub_pkg);
        Ok(Ed25519Curve::get_address(&vk))
    }

    /// Get the Ethereum address (keccak256 of secp256k1 public key).
    pub fn get_eth_address(&self) -> Result<String> {
        let pub_pkg = self.secp256k1_public_key_package.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Secp256k1 DKG not complete".into()))?;
        let vk = Secp256k1Curve::verifying_key(pub_pkg);
        Secp256k1Curve::get_eth_address(&vk)
    }

    /// Get the ed25519 key package (for signing).
    pub fn ed25519_key_package(&self) -> Option<&frost_ed25519::keys::KeyPackage> {
        self.ed25519_key_package.as_ref()
    }

    /// Get the ed25519 public key package (for verification).
    pub fn ed25519_public_key_package(&self) -> Option<&frost_ed25519::keys::PublicKeyPackage> {
        self.ed25519_public_key_package.as_ref()
    }

    /// Get the secp256k1 key package (for signing).
    pub fn secp256k1_key_package(&self) -> Option<&frost_secp256k1::keys::KeyPackage> {
        self.secp256k1_key_package.as_ref()
    }

    /// Get the secp256k1 public key package (for verification).
    pub fn secp256k1_public_key_package(&self) -> Option<&frost_secp256k1::keys::PublicKeyPackage> {
        self.secp256k1_public_key_package.as_ref()
    }

    /// Get the participant index.
    pub fn participant_index(&self) -> u16 {
        self.participant_index
    }

    /// Get the ed25519 chain code (available after finalization).
    pub fn ed25519_chain_code(&self) -> Option<&ChainCode> {
        self.ed25519_chain_code.as_ref()
    }

    /// Get the secp256k1 chain code (available after finalization).
    pub fn secp256k1_chain_code(&self) -> Option<&ChainCode> {
        self.secp256k1_chain_code.as_ref()
    }

    /// Derive child key packages for both curves at the given indices.
    ///
    /// Each curve uses its own independent index, allowing different
    /// BIP-44 paths (e.g., Solana uses coin_type 501, Ethereum uses 60).
    pub fn derive_child(
        &self,
        ed_index: u32,
        secp_index: u32,
    ) -> Result<(
        DerivedKeys<frost_ed25519::Ed25519Sha512>,
        DerivedKeys<frost_secp256k1::Secp256K1Sha256>,
    )> {
        let ed_kp = self.ed25519_key_package.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Ed25519 DKG not complete".into()))?;
        let ed_pub = self.ed25519_public_key_package.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Ed25519 DKG not complete".into()))?;
        let ed_cc = self.ed25519_chain_code.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Ed25519 chain code not available".into()))?;

        let secp_kp = self.secp256k1_key_package.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Secp256k1 DKG not complete".into()))?;
        let secp_pub = self.secp256k1_public_key_package.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Secp256k1 DKG not complete".into()))?;
        let secp_cc = self.secp256k1_chain_code.as_ref()
            .ok_or_else(|| FrostError::InvalidState("Secp256k1 chain code not available".into()))?;

        let ed_derived = derive_child_key::<frost_ed25519::Ed25519Sha512>(
            ed_kp, ed_pub, ed_cc, ed_index,
        )?;
        let secp_derived = derive_child_key::<frost_secp256k1::Secp256K1Sha256>(
            secp_kp, secp_pub, secp_cc, secp_index,
        )?;

        Ok((ed_derived, secp_derived))
    }
}
