use wasm_bindgen::prelude::*;
use starlab_core::{
    FrostCurve, FrostError,
    ed25519::Ed25519Curve,
    secp256k1::Secp256k1Curve,
    keystore::{Keystore, KeystoreData},
    root_secret::RootSecret,
    unified_dkg::{UnifiedDkg, UnifiedRound1Package},
};
// In rand 0.9+ `rngs::OsRng` moved to `rand_core` (it's the same type;
// the `rand` crate's re-export was dropped). Match frost-core's usage
// for consistency.
use rand_core::OsRng;
use std::collections::BTreeMap;

// Re-export specific FROST types needed by WASM
use frost_ed25519::{
    Identifier as Ed25519Identifier,
    keys::{KeyPackage as Ed25519KeyPackage, PublicKeyPackage as Ed25519PublicKeyPackage},
    round1::{SigningCommitments as Ed25519SigningCommitments, SigningNonces as Ed25519SigningNonces},
    round2::SignatureShare as Ed25519SignatureShare,
};

use frost_secp256k1::{
    Identifier as Secp256k1Identifier,
    keys::{KeyPackage as Secp256k1KeyPackage, PublicKeyPackage as Secp256k1PublicKeyPackage},
    round1::{SigningCommitments as Secp256k1SigningCommitments, SigningNonces as Secp256k1SigningNonces},
    round2::SignatureShare as Secp256k1SignatureShare,
};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// Error type for WASM
#[wasm_bindgen]
#[derive(Debug)]
pub struct WasmError {
    message: String,
}

#[wasm_bindgen]
impl WasmError {
    #[wasm_bindgen(constructor)]
    pub fn new(message: &str) -> Self {
        WasmError {
            message: message.to_string(),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

impl From<FrostError> for WasmError {
    fn from(error: FrostError) -> Self {
        WasmError {
            message: error.to_string(),
        }
    }
}

// Per-curve WASM wrappers (FrostDkgEd25519 / FrostDkgSecp256k1) —
// single-curve DKG + threshold signing used by the browser extension.
// The TUI node uses the curve-agnostic UnifiedDkg binding further
// down (line ~625); keep both sides in lock-step when touching DKG
// round bookkeeping.
//
// Contract with frost-core (as of 2.2):
//   - dkg::part2 requires round1_packages.len() == max_signers - 1
//     (keys/dkg.rs:505). `can_start_round2` below matches.
//   - dkg::part2 expects peer packages only; self is NOT added to
//     round1_packages here (generate_round1 stores only the secret).
//   - round2::sign requires the signer's own commitment to appear
//     in signing_package (round2.rs:135). `signing_commit` inserts
//     the own commitment into self.signing_commitments to satisfy
//     this without exposing the requirement to the caller.
//   - aggregate requires signature_shares identifiers to match
//     signing_commitments. `sign` inserts the own share into
//     self.signature_shares for the same reason.
//
// Regression tests covering all four contracts live in
// apps/browser-extension/tests/wasm-frost-contracts.test.ts.

// Ed25519 WASM wrapper
#[wasm_bindgen]
pub struct FrostDkgEd25519 {
    round1_secret: Option<frost_ed25519::keys::dkg::round1::SecretPackage>,
    round2_secret: Option<frost_ed25519::keys::dkg::round2::SecretPackage>,
    key_package: Option<Ed25519KeyPackage>,
    public_key_package: Option<Ed25519PublicKeyPackage>,
    round1_packages: BTreeMap<Ed25519Identifier, frost_ed25519::keys::dkg::round1::Package>,
    round2_packages: BTreeMap<Ed25519Identifier, frost_ed25519::keys::dkg::round2::Package>,
    signing_nonces: Option<Ed25519SigningNonces>,
    signing_commitments: BTreeMap<Ed25519Identifier, Ed25519SigningCommitments>,
    signature_shares: BTreeMap<Ed25519Identifier, Ed25519SignatureShare>,
    participant_indices: Vec<u16>,
    threshold: u16,
    total: u16,
    participant_index: u16,
}

impl Default for FrostDkgEd25519 {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl FrostDkgEd25519 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            round1_secret: None,
            round2_secret: None,
            key_package: None,
            public_key_package: None,
            round1_packages: BTreeMap::new(),
            round2_packages: BTreeMap::new(),
            signing_nonces: None,
            signing_commitments: BTreeMap::new(),
            signature_shares: BTreeMap::new(),
            participant_indices: Vec::new(),
            threshold: 0,
            total: 0,
            participant_index: 0,
        }
    }

    pub fn init_dkg(&mut self, participant_index: u16, total: u16, threshold: u16) -> Result<(), WasmError> {
        self.participant_index = participant_index;
        self.total = total;
        self.threshold = threshold;
        self.participant_indices = (1..=total).collect();
        Ok(())
    }

    pub fn generate_round1(&mut self) -> Result<String, WasmError> {
        let identifier = Ed25519Curve::identifier_from_u16(self.participant_index)?;
        let mut rng = OsRng;
        
        let (round1_secret, round1_package) = Ed25519Curve::dkg_part1(
            identifier,
            self.total,
            self.threshold,
            &mut rng,
        )?;
        
        self.round1_secret = Some(round1_secret);
        let package_json = serde_json::to_string(&round1_package)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        Ok(hex::encode(package_json))
    }

    pub fn add_round1_package(&mut self, participant_index: u16, package_hex: &str) -> Result<(), WasmError> {
        let package_json = hex::decode(package_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let package: frost_ed25519::keys::dkg::round1::Package = serde_json::from_slice(&package_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let identifier = Ed25519Curve::identifier_from_u16(participant_index)?;
        self.round1_packages.insert(identifier, package);
        Ok(())
    }

    pub fn can_start_round2(&self) -> bool {
        // frost-core's dkg::part2 expects round1_packages to contain
        // entries for every OTHER participant — so (total - 1). Our
        // generate_round1() only stores the local round1_secret and
        // never self-inserts the public package. Matching both sides
        // means checking for n-1 here.
        self.round1_packages.len() == (self.total.saturating_sub(1)) as usize
            && self.round1_secret.is_some()
    }

    pub fn generate_round2(&mut self) -> Result<String, WasmError> {
        let round1_secret = self.round1_secret.clone()
            .ok_or_else(|| WasmError::new("Round 1 secret not available"))?;

        let (round2_secret, round2_packages) = Ed25519Curve::dkg_part2(
            round1_secret,
            &self.round1_packages,
        )?;

        self.round2_secret = Some(round2_secret);

        let mut packages_map = BTreeMap::new();
        for (id, package) in round2_packages {
            // Both Ed25519 and Secp256k1 identifiers are produced by
            // identifier_bytes_from_u16 (traits.rs): 30 zero bytes
            // followed by the u16 big-endian at [30..=31]. Reading
            // from [31] (low) | [30]<<8 (high) recovers the index.
            let ser = id.serialize();
            let id_value = ser[31] as u16 | ((ser[30] as u16) << 8);
            packages_map.insert(id_value, hex::encode(serde_json::to_string(&package).unwrap()));
        }

        // Wrap the outer JSON in hex::encode so the wire format matches
        // generate_round1 (hex-encoded JSON). The JS consumer in
        // webrtc.ts _generateAndBroadcastRound2 hex-decodes before
        // JSON-parsing — without this the outer JSON contains raw
        // braces that fail to hex-decode.
        Ok(hex::encode(serde_json::to_string(&packages_map).unwrap()))
    }

    pub fn add_round2_package(&mut self, sender_index: u16, package_hex: &str) -> Result<(), WasmError> {
        let package_json = hex::decode(package_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let package: frost_ed25519::keys::dkg::round2::Package = serde_json::from_slice(&package_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;

        let identifier = Ed25519Curve::identifier_from_u16(sender_index)?;
        self.round2_packages.insert(identifier, package);
        Ok(())
    }

    pub fn can_finalize(&self) -> bool {
        self.round2_packages.len() >= (self.threshold - 1) as usize && self.round2_secret.is_some()
    }

    pub fn finalize_dkg(&mut self) -> Result<String, WasmError> {
        let round2_secret = self.round2_secret.as_ref()
            .ok_or_else(|| WasmError::new("Round 2 secret not available"))?;
        
        let (key_package, public_key_package) = Ed25519Curve::dkg_part3(
            round2_secret,
            &self.round1_packages,
            &self.round2_packages,
        )?;
        
        self.key_package = Some(key_package.clone());
        self.public_key_package = Some(public_key_package.clone());
        
        let keystore_data = Keystore::export_keystore::<Ed25519Curve>(
            &key_package,
            &public_key_package,
            self.threshold,
            self.total,
            self.participant_index,
            self.participant_indices.clone(),
            "ed25519",
        )?;
        
        Ok(serde_json::to_string(&keystore_data).unwrap())
    }

    pub fn get_group_public_key(&self) -> Result<String, WasmError> {
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("DKG not complete"))?;
        
        let verifying_key = Ed25519Curve::verifying_key(public_key_package);
        let key_bytes = Ed25519Curve::serialize_verifying_key(&verifying_key)?;
        Ok(hex::encode(key_bytes))
    }

    pub fn get_address(&self) -> Result<String, WasmError> {
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("DKG not complete"))?;
        
        let verifying_key = Ed25519Curve::verifying_key(public_key_package);
        Ok(Ed25519Curve::get_address(&verifying_key))
    }

    pub fn is_dkg_complete(&self) -> bool {
        self.key_package.is_some() && self.public_key_package.is_some()
    }

    pub fn signing_commit(&mut self) -> Result<String, WasmError> {
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;

        let (nonces, commitments) = Ed25519Curve::generate_signing_commitment(key_package)?;
        self.signing_nonces = Some(nonces);

        // frost-core's round2::sign() (see round2.rs:135) requires the
        // signer's OWN commitment to be present in the signing_package.
        // Register ours here so callers can treat add_signing_commitment
        // as a peer-only operation. Keyed by our identifier, matching
        // the layout frost-core expects.
        let own_identifier = Ed25519Curve::identifier_from_u16(self.participant_index)?;
        self.signing_commitments.insert(own_identifier, commitments);

        let commitment_hex = hex::encode(serde_json::to_string(&commitments).unwrap());
        Ok(commitment_hex)
    }

    pub fn add_signing_commitment(&mut self, participant_index: u16, commitment_hex: &str) -> Result<(), WasmError> {
        let commitment_json = hex::decode(commitment_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let commitment: Ed25519SigningCommitments = serde_json::from_slice(&commitment_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;

        let identifier = Ed25519Curve::identifier_from_u16(participant_index)?;
        self.signing_commitments.insert(identifier, commitment);
        Ok(())
    }

    pub fn sign(&mut self, message_hex: &str) -> Result<String, WasmError> {
        let message = hex::decode(message_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;

        let signing_package = Ed25519Curve::create_signing_package(&self.signing_commitments, &message)?;

        let nonces = self.signing_nonces.as_ref()
            .ok_or_else(|| WasmError::new("Signing nonces not available"))?;
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;

        let signature_share = Ed25519Curve::generate_signature_share(&signing_package, nonces, key_package)?;

        // Register our own share so aggregate_signature() sees it in
        // self.signature_shares alongside peers'. frost-core's aggregate
        // (see frost-core/src/lib.rs) requires signature_shares to cover
        // every identifier present in signing_commitments, so omitting
        // self would immediately fail with UnknownIdentifier.
        let own_identifier = Ed25519Curve::identifier_from_u16(self.participant_index)?;
        self.signature_shares.insert(own_identifier, signature_share);

        Ok(hex::encode(serde_json::to_string(&signature_share).unwrap()))
    }

    pub fn add_signature_share(&mut self, participant_index: u16, share_hex: &str) -> Result<(), WasmError> {
        let share_json = hex::decode(share_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let share: Ed25519SignatureShare = serde_json::from_slice(&share_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let identifier = Ed25519Curve::identifier_from_u16(participant_index)?;
        self.signature_shares.insert(identifier, share);
        Ok(())
    }

    pub fn aggregate_signature(&self, message_hex: &str) -> Result<String, WasmError> {
        let message = hex::decode(message_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let signing_package = Ed25519Curve::create_signing_package(&self.signing_commitments, &message)?;
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("Public key package not available"))?;
        
        let signature = Ed25519Curve::aggregate_signature(&signing_package, &self.signature_shares, public_key_package)?;
        let sig_bytes = Ed25519Curve::serialize_signature(&signature)?;
        
        Ok(hex::encode(sig_bytes))
    }

    pub fn clear_signing_state(&mut self) {
        self.signing_nonces = None;
        self.signing_commitments.clear();
        self.signature_shares.clear();
    }

    pub fn has_signing_nonces(&self) -> bool {
        self.signing_nonces.is_some()
    }

    pub fn import_keystore(&mut self, keystore_json: &str) -> Result<(), WasmError> {
        let keystore_data: KeystoreData = serde_json::from_str(keystore_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let (key_package, public_key_package) = Keystore::import_keystore::<Ed25519Curve>(&keystore_data)?;
        
        self.key_package = Some(key_package);
        self.public_key_package = Some(public_key_package);
        self.threshold = keystore_data.min_signers;
        self.total = keystore_data.max_signers;
        self.participant_index = keystore_data.participant_index;
        self.participant_indices = keystore_data.participant_indices;
        
        Ok(())
    }

    pub fn export_keystore(&self) -> Result<String, WasmError> {
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("Public key package not available"))?;
        
        let keystore_data = Keystore::export_keystore::<Ed25519Curve>(
            key_package,
            public_key_package,
            self.threshold,
            self.total,
            self.participant_index,
            self.participant_indices.clone(),
            "ed25519",
        )?;
        
        Ok(serde_json::to_string(&keystore_data).unwrap())
    }
}

// Secp256k1 WASM wrapper
#[wasm_bindgen]
pub struct FrostDkgSecp256k1 {
    round1_secret: Option<frost_secp256k1::keys::dkg::round1::SecretPackage>,
    round2_secret: Option<frost_secp256k1::keys::dkg::round2::SecretPackage>,
    key_package: Option<Secp256k1KeyPackage>,
    public_key_package: Option<Secp256k1PublicKeyPackage>,
    round1_packages: BTreeMap<Secp256k1Identifier, frost_secp256k1::keys::dkg::round1::Package>,
    round2_packages: BTreeMap<Secp256k1Identifier, frost_secp256k1::keys::dkg::round2::Package>,
    signing_nonces: Option<Secp256k1SigningNonces>,
    signing_commitments: BTreeMap<Secp256k1Identifier, Secp256k1SigningCommitments>,
    signature_shares: BTreeMap<Secp256k1Identifier, Secp256k1SignatureShare>,
    participant_indices: Vec<u16>,
    threshold: u16,
    total: u16,
    participant_index: u16,
}

impl Default for FrostDkgSecp256k1 {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl FrostDkgSecp256k1 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            round1_secret: None,
            round2_secret: None,
            key_package: None,
            public_key_package: None,
            round1_packages: BTreeMap::new(),
            round2_packages: BTreeMap::new(),
            signing_nonces: None,
            signing_commitments: BTreeMap::new(),
            signature_shares: BTreeMap::new(),
            participant_indices: Vec::new(),
            threshold: 0,
            total: 0,
            participant_index: 0,
        }
    }

    pub fn init_dkg(&mut self, participant_index: u16, total: u16, threshold: u16) -> Result<(), WasmError> {
        self.participant_index = participant_index;
        self.total = total;
        self.threshold = threshold;
        self.participant_indices = (1..=total).collect();
        Ok(())
    }

    pub fn generate_round1(&mut self) -> Result<String, WasmError> {
        let identifier = Secp256k1Curve::identifier_from_u16(self.participant_index)?;
        let mut rng = OsRng;
        
        let (round1_secret, round1_package) = Secp256k1Curve::dkg_part1(
            identifier,
            self.total,
            self.threshold,
            &mut rng,
        )?;
        
        self.round1_secret = Some(round1_secret);
        let package_json = serde_json::to_string(&round1_package)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        Ok(hex::encode(package_json))
    }

    pub fn add_round1_package(&mut self, participant_index: u16, package_hex: &str) -> Result<(), WasmError> {
        let package_json = hex::decode(package_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let package: frost_secp256k1::keys::dkg::round1::Package = serde_json::from_slice(&package_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let identifier = Secp256k1Curve::identifier_from_u16(participant_index)?;
        self.round1_packages.insert(identifier, package);
        Ok(())
    }

    pub fn can_start_round2(&self) -> bool {
        // frost-core's dkg::part2 expects round1_packages to contain
        // entries for every OTHER participant — so (total - 1). Our
        // generate_round1() only stores the local round1_secret and
        // never self-inserts the public package. Matching both sides
        // means checking for n-1 here.
        self.round1_packages.len() == (self.total.saturating_sub(1)) as usize
            && self.round1_secret.is_some()
    }

    pub fn generate_round2(&mut self) -> Result<String, WasmError> {
        let round1_secret = self.round1_secret.clone()
            .ok_or_else(|| WasmError::new("Round 1 secret not available"))?;

        let (round2_secret, round2_packages) = Secp256k1Curve::dkg_part2(
            round1_secret,
            &self.round1_packages,
        )?;

        self.round2_secret = Some(round2_secret);

        let mut packages_map = BTreeMap::new();
        for (id, package) in round2_packages {
            // Secp256k1 identifiers serialize as u32 big-endian in the
            // last four bytes (bytes [28..=31]) — matches the test
            // helper's writeUInt32BE(index, 28).
            let ser = id.serialize();
            let id_value = ser[31] as u16 | ((ser[30] as u16) << 8);
            packages_map.insert(id_value, hex::encode(serde_json::to_string(&package).unwrap()));
        }

        // Wrap outer JSON in hex::encode — see Ed25519 note above.
        Ok(hex::encode(serde_json::to_string(&packages_map).unwrap()))
    }

    pub fn add_round2_package(&mut self, sender_index: u16, package_hex: &str) -> Result<(), WasmError> {
        let package_json = hex::decode(package_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let package: frost_secp256k1::keys::dkg::round2::Package = serde_json::from_slice(&package_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let identifier = Secp256k1Curve::identifier_from_u16(sender_index)?;
        self.round2_packages.insert(identifier, package);
        Ok(())
    }

    pub fn can_finalize(&self) -> bool {
        self.round2_packages.len() >= (self.threshold - 1) as usize && self.round2_secret.is_some()
    }

    pub fn finalize_dkg(&mut self) -> Result<String, WasmError> {
        let round2_secret = self.round2_secret.as_ref()
            .ok_or_else(|| WasmError::new("Round 2 secret not available"))?;
        
        let (key_package, public_key_package) = Secp256k1Curve::dkg_part3(
            round2_secret,
            &self.round1_packages,
            &self.round2_packages,
        )?;
        
        self.key_package = Some(key_package.clone());
        self.public_key_package = Some(public_key_package.clone());
        
        let keystore_data = Keystore::export_keystore::<Secp256k1Curve>(
            &key_package,
            &public_key_package,
            self.threshold,
            self.total,
            self.participant_index,
            self.participant_indices.clone(),
            "secp256k1",
        )?;
        
        Ok(serde_json::to_string(&keystore_data).unwrap())
    }

    pub fn get_group_public_key(&self) -> Result<String, WasmError> {
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("DKG not complete"))?;
        
        let verifying_key = Secp256k1Curve::verifying_key(public_key_package);
        let key_bytes = Secp256k1Curve::serialize_verifying_key(&verifying_key)?;
        Ok(hex::encode(key_bytes))
    }

    pub fn get_address(&self) -> Result<String, WasmError> {
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("DKG not complete"))?;
        
        let verifying_key = Secp256k1Curve::verifying_key(public_key_package);
        Ok(Secp256k1Curve::get_address(&verifying_key))
    }

    pub fn get_eth_address(&self) -> Result<String, WasmError> {
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("DKG not complete"))?;
        
        let verifying_key = Secp256k1Curve::verifying_key(public_key_package);
        Ok(Secp256k1Curve::get_eth_address(&verifying_key)?)
    }

    pub fn is_dkg_complete(&self) -> bool {
        self.key_package.is_some() && self.public_key_package.is_some()
    }

    pub fn signing_commit(&mut self) -> Result<String, WasmError> {
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;

        let (nonces, commitments) = Secp256k1Curve::generate_signing_commitment(key_package)?;
        self.signing_nonces = Some(nonces);

        // See Ed25519 signing_commit for context: frost-core requires
        // the signer's own commitment to be in the signing_package.
        let own_identifier = Secp256k1Curve::identifier_from_u16(self.participant_index)?;
        self.signing_commitments.insert(own_identifier, commitments);

        let commitment_hex = hex::encode(serde_json::to_string(&commitments).unwrap());
        Ok(commitment_hex)
    }

    pub fn add_signing_commitment(&mut self, participant_index: u16, commitment_hex: &str) -> Result<(), WasmError> {
        let commitment_json = hex::decode(commitment_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let commitment: Secp256k1SigningCommitments = serde_json::from_slice(&commitment_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let identifier = Secp256k1Curve::identifier_from_u16(participant_index)?;
        self.signing_commitments.insert(identifier, commitment);
        Ok(())
    }

    pub fn sign(&mut self, message_hex: &str) -> Result<String, WasmError> {
        let message = hex::decode(message_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;

        let signing_package = Secp256k1Curve::create_signing_package(&self.signing_commitments, &message)?;

        let nonces = self.signing_nonces.as_ref()
            .ok_or_else(|| WasmError::new("Signing nonces not available"))?;
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;

        let signature_share = Secp256k1Curve::generate_signature_share(&signing_package, nonces, key_package)?;

        // See Ed25519 sign() for context — register own share so
        // aggregate_signature() covers all identifiers.
        let own_identifier = Secp256k1Curve::identifier_from_u16(self.participant_index)?;
        self.signature_shares.insert(own_identifier, signature_share);

        Ok(hex::encode(serde_json::to_string(&signature_share).unwrap()))
    }

    pub fn add_signature_share(&mut self, participant_index: u16, share_hex: &str) -> Result<(), WasmError> {
        let share_json = hex::decode(share_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let share: Secp256k1SignatureShare = serde_json::from_slice(&share_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;

        let identifier = Secp256k1Curve::identifier_from_u16(participant_index)?;
        self.signature_shares.insert(identifier, share);
        Ok(())
    }

    pub fn aggregate_signature(&self, message_hex: &str) -> Result<String, WasmError> {
        let message = hex::decode(message_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let signing_package = Secp256k1Curve::create_signing_package(&self.signing_commitments, &message)?;
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("Public key package not available"))?;
        
        let signature = Secp256k1Curve::aggregate_signature(&signing_package, &self.signature_shares, public_key_package)?;
        let sig_bytes = Secp256k1Curve::serialize_signature(&signature)?;
        
        Ok(hex::encode(sig_bytes))
    }

    pub fn clear_signing_state(&mut self) {
        self.signing_nonces = None;
        self.signing_commitments.clear();
        self.signature_shares.clear();
    }

    pub fn has_signing_nonces(&self) -> bool {
        self.signing_nonces.is_some()
    }

    pub fn import_keystore(&mut self, keystore_json: &str) -> Result<(), WasmError> {
        let keystore_data: KeystoreData = serde_json::from_str(keystore_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let (key_package, public_key_package) = Keystore::import_keystore::<Secp256k1Curve>(&keystore_data)?;
        
        self.key_package = Some(key_package);
        self.public_key_package = Some(public_key_package);
        self.threshold = keystore_data.min_signers;
        self.total = keystore_data.max_signers;
        self.participant_index = keystore_data.participant_index;
        self.participant_indices = keystore_data.participant_indices;
        
        Ok(())
    }

    pub fn export_keystore(&self) -> Result<String, WasmError> {
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("Public key package not available"))?;
        
        let keystore_data = Keystore::export_keystore::<Secp256k1Curve>(
            key_package,
            public_key_package,
            self.threshold,
            self.total,
            self.participant_index,
            self.participant_indices.clone(),
            "secp256k1",
        )?;
        
        Ok(serde_json::to_string(&keystore_data).unwrap())
    }
}

#[wasm_bindgen]
pub fn main() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    
    console_log!("MPC Wallet WASM initialized");
}

// Called when the WASM module is instantiated
#[wasm_bindgen(start)]
pub fn start() {
    main();
}

// ============================================================================
// Unified DKG: single root secret → both ed25519 + secp256k1 key packages
// ============================================================================

#[wasm_bindgen]
pub struct FrostDkgUnified {
    dkg: UnifiedDkg,
}

impl Default for FrostDkgUnified {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl FrostDkgUnified {
    /// Create a new unified DKG with a fresh root secret.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            dkg: UnifiedDkg::new(),
        }
    }

    /// Create a unified DKG from an existing root secret (hex-encoded 32 bytes).
    pub fn from_root_secret(root_secret_hex: &str) -> Result<FrostDkgUnified, WasmError> {
        let bytes = hex::decode(root_secret_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        if bytes.len() != 32 {
            return Err(WasmError::new("Root secret must be exactly 32 bytes"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self {
            dkg: UnifiedDkg::with_root_secret(RootSecret::from_bytes(arr)),
        })
    }

    /// Get the root secret as hex (for backup/storage).
    pub fn get_root_secret_hex(&self) -> String {
        hex::encode(self.dkg.root_secret().as_bytes())
    }

    /// Initialize DKG parameters.
    pub fn init_dkg(&mut self, participant_index: u16, total: u16, threshold: u16) {
        self.dkg.init_dkg(participant_index, total, threshold);
    }

    /// Generate round 1 packages for both curves.
    /// Returns JSON: `{ "ed25519": "<hex>", "secp256k1": "<hex>" }`
    pub fn generate_round1(&mut self) -> Result<String, WasmError> {
        let package = self.dkg.generate_round1()?;
        serde_json::to_string(&package)
            .map_err(|e| WasmError::new(&e.to_string()))
    }

    /// Add a round 1 package from another participant.
    /// package_json is the JSON output from another participant's generate_round1().
    pub fn add_round1_package(&mut self, participant_index: u16, package_json: &str) -> Result<(), WasmError> {
        let package: UnifiedRound1Package = serde_json::from_str(package_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        self.dkg.add_round1_package(participant_index, &package)?;
        Ok(())
    }

    /// Check if round 2 can start.
    pub fn can_start_round2(&self) -> bool {
        self.dkg.can_start_round2()
    }

    /// Generate round 2 packages for both curves.
    /// Returns JSON: `{ "ed25519": { <participant_index>: "<hex>", ... }, "secp256k1": { ... } }`
    pub fn generate_round2(&mut self) -> Result<String, WasmError> {
        let packages = self.dkg.generate_round2()?;
        serde_json::to_string(&packages)
            .map_err(|e| WasmError::new(&e.to_string()))
    }

    /// Add round 2 packages from another participant for both curves.
    pub fn add_round2_package(&mut self, sender_index: u16, ed_hex: &str, secp_hex: &str) -> Result<(), WasmError> {
        self.dkg.add_round2_package(sender_index, ed_hex, secp_hex)?;
        Ok(())
    }

    /// Check if DKG can be finalized.
    pub fn can_finalize(&self) -> bool {
        self.dkg.can_finalize()
    }

    /// Finalize DKG, producing a multi-curve keystore.
    /// Returns JSON with both ed25519 and secp256k1 keystore data.
    pub fn finalize_dkg(&mut self) -> Result<String, WasmError> {
        let keystore = self.dkg.finalize_dkg()?;
        serde_json::to_string(&keystore)
            .map_err(|e| WasmError::new(&e.to_string()))
    }

    /// Check if DKG is complete for both curves.
    pub fn is_dkg_complete(&self) -> bool {
        self.dkg.is_dkg_complete()
    }

    /// Get Solana address (ed25519 base58).
    pub fn get_solana_address(&self) -> Result<String, WasmError> {
        self.dkg.get_solana_address().map_err(|e| e.into())
    }

    /// Get Ethereum address (secp256k1 keccak256).
    pub fn get_eth_address(&self) -> Result<String, WasmError> {
        self.dkg.get_eth_address().map_err(|e| e.into())
    }

    /// Get ed25519 group public key (hex).
    pub fn get_ed25519_public_key(&self) -> Result<String, WasmError> {
        self.dkg.get_ed25519_group_public_key().map_err(|e| e.into())
    }

    /// Get secp256k1 group public key (hex).
    pub fn get_secp256k1_public_key(&self) -> Result<String, WasmError> {
        self.dkg.get_secp256k1_group_public_key().map_err(|e| e.into())
    }

    /// Export the ed25519 keystore data (for use with FrostDkgEd25519 signing).
    /// Must be called after finalize_dkg().
    pub fn export_ed25519_keystore(&mut self) -> Result<String, WasmError> {
        let keystore = self.dkg.finalize_dkg()?;
        serde_json::to_string(&keystore.ed25519)
            .map_err(|e| WasmError::new(&e.to_string()))
    }

    /// Export the secp256k1 keystore data (for use with FrostDkgSecp256k1 signing).
    /// Must be called after finalize_dkg().
    pub fn export_secp256k1_keystore(&mut self) -> Result<String, WasmError> {
        let keystore = self.dkg.finalize_dkg()?;
        serde_json::to_string(&keystore.secp256k1)
            .map_err(|e| WasmError::new(&e.to_string()))
    }
}
// ===========================================================================
// Reshare (#23 downstream): distributed share refresh in the browser.
//
// Symmetric with the keystore flow the extension already uses:
//   keystore JSON (export_keystore format) → init_reshare → round1/round2
//   over the existing WebRTC mesh → finalize_reshare → NEW keystore JSON.
// The group public key is preserved (address stable); old shares go dead.
// Wire format matches the DKG classes: hex(serde_json(Package)).
//
// Same constraint as the core engine: every participant in the new set must
// already hold a share — refresh rotates or REMOVES devices, it cannot mint
// a share for a brand-new device.
// ===========================================================================

macro_rules! frost_reshare_impl {
    ($name:ident, $curve:ty, $suite:ty, $curve_str:literal) => {
        #[wasm_bindgen]
        pub struct $name {
            session: Option<starlab_core::ReshareSession<$suite>>,
            new_ids: Vec<u16>,
            threshold: u16,
            my_id: u16,
            result: Option<(
                frost_core::keys::KeyPackage<$suite>,
                frost_core::keys::PublicKeyPackage<$suite>,
            )>,
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        #[wasm_bindgen]
        impl $name {
            #[wasm_bindgen(constructor)]
            pub fn new() -> Self {
                Self {
                    session: None,
                    new_ids: Vec::new(),
                    threshold: 0,
                    my_id: 0,
                    result: None,
                }
            }

            /// Start a reshare from this device's existing keystore JSON (the
            /// exact format export_keystore produces). `new_ids` is the
            /// comma-separated new participant set (must include this device;
            /// every id must already hold a share).
            pub fn init_reshare(
                &mut self,
                keystore_json: &str,
                new_ids_csv: &str,
                new_threshold: u16,
            ) -> Result<(), WasmError> {
                let keystore_data: KeystoreData = serde_json::from_str(keystore_json)
                    .map_err(|e| WasmError::new(&e.to_string()))?;
                let (kp, pp) = Keystore::import_keystore::<$curve>(&keystore_data)?;
                let new_ids: Vec<u16> = new_ids_csv
                    .split(',')
                    .map(|s| s.trim().parse::<u16>())
                    .collect::<std::result::Result<_, _>>()
                    .map_err(|e| WasmError::new(&format!("bad new_ids: {e}")))?;
                let my_id = keystore_data.participant_index;
                self.session = Some(
                    starlab_core::ReshareSession::new(
                        my_id,
                        new_threshold,
                        new_ids.clone(),
                        kp,
                        pp,
                    )
                    .map_err(WasmError::from)?,
                );
                self.new_ids = new_ids;
                self.threshold = new_threshold;
                self.my_id = my_id;
                self.result = None;
                Ok(())
            }

            /// Generate this device's round-1 package (broadcast to all peers).
            pub fn reshare_round1(&mut self) -> Result<String, WasmError> {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| WasmError::new("init_reshare not called"))?;
                let pkg = session.round1(&mut OsRng).map_err(WasmError::from)?;
                let json = serde_json::to_string(&pkg)
                    .map_err(|e| WasmError::new(&e.to_string()))?;
                Ok(hex::encode(json))
            }

            pub fn add_reshare_round1(
                &mut self,
                from: u16,
                package_hex: &str,
            ) -> Result<(), WasmError> {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| WasmError::new("init_reshare not called"))?;
                let bytes = hex::decode(package_hex)
                    .map_err(|e| WasmError::new(&e.to_string()))?;
                let pkg: frost_core::keys::dkg::round1::Package<$suite> =
                    serde_json::from_slice(&bytes)
                        .map_err(|e| WasmError::new(&e.to_string()))?;
                session.add_round1(from, pkg).map_err(WasmError::from)
            }

            pub fn can_reshare_round2(&self) -> bool {
                self.session.as_ref().map(|s| s.can_round2()).unwrap_or(false)
            }

            /// Generate the per-recipient round-2 packages as a JSON object
            /// mapping recipient id → hex package. Send each value ONLY to its
            /// recipient.
            pub fn reshare_round2(&mut self) -> Result<String, WasmError> {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| WasmError::new("init_reshare not called"))?;
                let sent = session.round2().map_err(WasmError::from)?;
                let mut out: BTreeMap<String, String> = BTreeMap::new();
                for (rcpt, pkg) in sent {
                    let json = serde_json::to_string(&pkg)
                        .map_err(|e| WasmError::new(&e.to_string()))?;
                    out.insert(rcpt.to_string(), hex::encode(json));
                }
                serde_json::to_string(&out).map_err(|e| WasmError::new(&e.to_string()))
            }

            pub fn add_reshare_round2(
                &mut self,
                from: u16,
                package_hex: &str,
            ) -> Result<(), WasmError> {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| WasmError::new("init_reshare not called"))?;
                let bytes = hex::decode(package_hex)
                    .map_err(|e| WasmError::new(&e.to_string()))?;
                let pkg: frost_core::keys::dkg::round2::Package<$suite> =
                    serde_json::from_slice(&bytes)
                        .map_err(|e| WasmError::new(&e.to_string()))?;
                session.add_round2(from, pkg).map_err(WasmError::from)
            }

            pub fn can_finalize_reshare(&self) -> bool {
                self.session.as_ref().map(|s| s.can_finalize()).unwrap_or(false)
            }

            /// Finalize and return the NEW keystore JSON (same format as
            /// export_keystore — drop-in replacement for the stored share).
            pub fn finalize_reshare(&mut self) -> Result<String, WasmError> {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| WasmError::new("init_reshare not called"))?;
                let (kp, pp) = session.finalize().map_err(WasmError::from)?;
                let keystore_data = Keystore::export_keystore::<$curve>(
                    &kp,
                    &pp,
                    self.threshold,
                    self.new_ids.len() as u16,
                    self.my_id,
                    self.new_ids.clone(),
                    $curve_str,
                )?;
                self.result = Some((kp, pp));
                serde_json::to_string(&keystore_data)
                    .map_err(|e| WasmError::new(&e.to_string()))
            }

            /// Group public key hex after finalize — callers verify it equals
            /// the pre-reshare key (address unchanged).
            pub fn get_group_public_key(&self) -> Result<String, WasmError> {
                let (_, pp) = self
                    .result
                    .as_ref()
                    .ok_or_else(|| WasmError::new("reshare not finalized"))?;
                pp.verifying_key()
                    .serialize()
                    .map(hex::encode)
                    .map_err(|e| WasmError::new(&e.to_string()))
            }
        }
    };
}

frost_reshare_impl!(FrostReshareEd25519, Ed25519Curve, frost_ed25519::Ed25519Sha512, "ed25519");
frost_reshare_impl!(
    FrostReshareSecp256k1,
    Secp256k1Curve,
    frost_secp256k1::Secp256K1Sha256,
    "secp256k1"
);

// ===========================================================================
// HD derivation (BIP-44-style child keys from the group key).
//
// Deterministic: every participant derives the same child from the same
// path, because the chain code comes from the group verifying key. The
// output is a full keystore JSON for the CHILD key — import it into a
// FrostDkg* instance and sign with the derived account like any other.
// ===========================================================================

macro_rules! derive_keystore_impl {
    ($fn_name:ident, $curve:ty, $suite:ty, $curve_str:literal) => {
        /// Derive a child keystore at a BIP-44 path (e.g. "m/44'/60'/0'/0/1").
        /// Deterministic across participants (chain code comes from the group
        /// key), so every device derives the SAME child account. The output is
        /// a full keystore JSON — import it into a FrostDkg instance and sign
        /// with the derived account like any other.
        #[wasm_bindgen]
        pub fn $fn_name(keystore_json: &str, path: &str) -> Result<String, WasmError> {
            let keystore_data: KeystoreData = serde_json::from_str(keystore_json)
                .map_err(|e| WasmError::new(&e.to_string()))?;
            let (kp, pp) = Keystore::import_keystore::<$curve>(&keystore_data)?;

            let parsed = starlab_core::DerivationPath::parse(path).map_err(WasmError::from)?;
            let group_key = pp
                .verifying_key()
                .serialize()
                .map_err(|e| WasmError::new(&e.to_string()))?;
            let chain_code = starlab_core::ChainCode::from_group_key(group_key.as_ref());
            let derived =
                starlab_core::derive_child_key_path::<$suite>(&kp, &pp, &chain_code, &parsed)
                    .map_err(WasmError::from)?;

            let out = Keystore::export_keystore::<$curve>(
                &derived.key_package,
                &derived.public_key_package,
                keystore_data.min_signers,
                keystore_data.max_signers,
                keystore_data.participant_index,
                keystore_data.participant_indices.clone(),
                $curve_str,
            )?;
            serde_json::to_string(&out).map_err(|e| WasmError::new(&e.to_string()))
        }
    };
}

derive_keystore_impl!(
    derive_child_keystore_ed25519,
    Ed25519Curve,
    frost_ed25519::Ed25519Sha512,
    "ed25519"
);
derive_keystore_impl!(
    derive_child_keystore_secp256k1,
    Secp256k1Curve,
    frost_secp256k1::Secp256K1Sha256,
    "secp256k1"
);


use frost_secp256k1_tr::{
    Identifier as Secp256k1TrIdentifier,
    keys::{KeyPackage as Secp256k1TrKeyPackage, PublicKeyPackage as Secp256k1TrPublicKeyPackage},
    round1::{SigningCommitments as Secp256k1TrSigningCommitments, SigningNonces as Secp256k1TrSigningNonces},
    round2::SignatureShare as Secp256k1TrSignatureShare,
};
use starlab_core::secp256k1_tr::Secp256k1TrCurve;

// Secp256k1-Taproot (BIP-340) WASM wrapper — same surface as
// FrostDkgSecp256k1Tr but over the frost-secp256k1-tr ciphersuite, producing
// BIP-340-verifiable Schnorr signatures for Bitcoin P2TR. Keep using
// FrostDkgSecp256k1Tr for EVM (#93 4337 path); keystores are NOT
// interchangeable between the two suites.
#[wasm_bindgen]
pub struct FrostDkgSecp256k1Tr {
    round1_secret: Option<frost_secp256k1_tr::keys::dkg::round1::SecretPackage>,
    round2_secret: Option<frost_secp256k1_tr::keys::dkg::round2::SecretPackage>,
    key_package: Option<Secp256k1TrKeyPackage>,
    public_key_package: Option<Secp256k1TrPublicKeyPackage>,
    round1_packages: BTreeMap<Secp256k1TrIdentifier, frost_secp256k1_tr::keys::dkg::round1::Package>,
    round2_packages: BTreeMap<Secp256k1TrIdentifier, frost_secp256k1_tr::keys::dkg::round2::Package>,
    signing_nonces: Option<Secp256k1TrSigningNonces>,
    signing_commitments: BTreeMap<Secp256k1TrIdentifier, Secp256k1TrSigningCommitments>,
    signature_shares: BTreeMap<Secp256k1TrIdentifier, Secp256k1TrSignatureShare>,
    participant_indices: Vec<u16>,
    threshold: u16,
    total: u16,
    participant_index: u16,
}

impl Default for FrostDkgSecp256k1Tr {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl FrostDkgSecp256k1Tr {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            round1_secret: None,
            round2_secret: None,
            key_package: None,
            public_key_package: None,
            round1_packages: BTreeMap::new(),
            round2_packages: BTreeMap::new(),
            signing_nonces: None,
            signing_commitments: BTreeMap::new(),
            signature_shares: BTreeMap::new(),
            participant_indices: Vec::new(),
            threshold: 0,
            total: 0,
            participant_index: 0,
        }
    }

    pub fn init_dkg(&mut self, participant_index: u16, total: u16, threshold: u16) -> Result<(), WasmError> {
        self.participant_index = participant_index;
        self.total = total;
        self.threshold = threshold;
        self.participant_indices = (1..=total).collect();
        Ok(())
    }

    pub fn generate_round1(&mut self) -> Result<String, WasmError> {
        let identifier = Secp256k1TrCurve::identifier_from_u16(self.participant_index)?;
        let mut rng = OsRng;
        
        let (round1_secret, round1_package) = Secp256k1TrCurve::dkg_part1(
            identifier,
            self.total,
            self.threshold,
            &mut rng,
        )?;
        
        self.round1_secret = Some(round1_secret);
        let package_json = serde_json::to_string(&round1_package)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        Ok(hex::encode(package_json))
    }

    pub fn add_round1_package(&mut self, participant_index: u16, package_hex: &str) -> Result<(), WasmError> {
        let package_json = hex::decode(package_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let package: frost_secp256k1_tr::keys::dkg::round1::Package = serde_json::from_slice(&package_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let identifier = Secp256k1TrCurve::identifier_from_u16(participant_index)?;
        self.round1_packages.insert(identifier, package);
        Ok(())
    }

    pub fn can_start_round2(&self) -> bool {
        // frost-core's dkg::part2 expects round1_packages to contain
        // entries for every OTHER participant — so (total - 1). Our
        // generate_round1() only stores the local round1_secret and
        // never self-inserts the public package. Matching both sides
        // means checking for n-1 here.
        self.round1_packages.len() == (self.total.saturating_sub(1)) as usize
            && self.round1_secret.is_some()
    }

    pub fn generate_round2(&mut self) -> Result<String, WasmError> {
        let round1_secret = self.round1_secret.clone()
            .ok_or_else(|| WasmError::new("Round 1 secret not available"))?;

        let (round2_secret, round2_packages) = Secp256k1TrCurve::dkg_part2(
            round1_secret,
            &self.round1_packages,
        )?;

        self.round2_secret = Some(round2_secret);

        let mut packages_map = BTreeMap::new();
        for (id, package) in round2_packages {
            // Secp256k1 identifiers serialize as u32 big-endian in the
            // last four bytes (bytes [28..=31]) — matches the test
            // helper's writeUInt32BE(index, 28).
            let ser = id.serialize();
            let id_value = ser[31] as u16 | ((ser[30] as u16) << 8);
            packages_map.insert(id_value, hex::encode(serde_json::to_string(&package).unwrap()));
        }

        // Wrap outer JSON in hex::encode — see Ed25519 note above.
        Ok(hex::encode(serde_json::to_string(&packages_map).unwrap()))
    }

    pub fn add_round2_package(&mut self, sender_index: u16, package_hex: &str) -> Result<(), WasmError> {
        let package_json = hex::decode(package_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let package: frost_secp256k1_tr::keys::dkg::round2::Package = serde_json::from_slice(&package_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let identifier = Secp256k1TrCurve::identifier_from_u16(sender_index)?;
        self.round2_packages.insert(identifier, package);
        Ok(())
    }

    pub fn can_finalize(&self) -> bool {
        self.round2_packages.len() >= (self.threshold - 1) as usize && self.round2_secret.is_some()
    }

    pub fn finalize_dkg(&mut self) -> Result<String, WasmError> {
        let round2_secret = self.round2_secret.as_ref()
            .ok_or_else(|| WasmError::new("Round 2 secret not available"))?;
        
        let (key_package, public_key_package) = Secp256k1TrCurve::dkg_part3(
            round2_secret,
            &self.round1_packages,
            &self.round2_packages,
        )?;
        
        self.key_package = Some(key_package.clone());
        self.public_key_package = Some(public_key_package.clone());
        
        let keystore_data = Keystore::export_keystore::<Secp256k1TrCurve>(
            &key_package,
            &public_key_package,
            self.threshold,
            self.total,
            self.participant_index,
            self.participant_indices.clone(),
            "secp256k1-tr",
        )?;
        
        Ok(serde_json::to_string(&keystore_data).unwrap())
    }

    pub fn get_group_public_key(&self) -> Result<String, WasmError> {
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("DKG not complete"))?;
        
        let verifying_key = Secp256k1TrCurve::verifying_key(public_key_package);
        let key_bytes = Secp256k1TrCurve::serialize_verifying_key(&verifying_key)?;
        Ok(hex::encode(key_bytes))
    }

    pub fn get_address(&self) -> Result<String, WasmError> {
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("DKG not complete"))?;
        
        let verifying_key = Secp256k1TrCurve::verifying_key(public_key_package);
        Ok(Secp256k1TrCurve::get_address(&verifying_key))
    }


    /// BIP-340 x-only Taproot output key (32 bytes hex): the SEC1 key minus
    /// its parity prefix. This is what P2TR addresses / verifiers consume.
    pub fn get_taproot_xonly_key(&self) -> Result<String, WasmError> {
        let pp = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("DKG not complete"))?;
        let sec1 = pp.verifying_key().serialize()
            .map_err(|e| WasmError::new(&e.to_string()))?;
        if sec1.len() != 33 {
            return Err(WasmError::new("unexpected verifying key length"));
        }
        Ok(hex::encode(&sec1[1..]))
    }

    pub fn is_dkg_complete(&self) -> bool {
        self.key_package.is_some() && self.public_key_package.is_some()
    }

    pub fn signing_commit(&mut self) -> Result<String, WasmError> {
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;

        let (nonces, commitments) = Secp256k1TrCurve::generate_signing_commitment(key_package)?;
        self.signing_nonces = Some(nonces);

        // See Ed25519 signing_commit for context: frost-core requires
        // the signer's own commitment to be in the signing_package.
        let own_identifier = Secp256k1TrCurve::identifier_from_u16(self.participant_index)?;
        self.signing_commitments.insert(own_identifier, commitments);

        let commitment_hex = hex::encode(serde_json::to_string(&commitments).unwrap());
        Ok(commitment_hex)
    }

    pub fn add_signing_commitment(&mut self, participant_index: u16, commitment_hex: &str) -> Result<(), WasmError> {
        let commitment_json = hex::decode(commitment_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let commitment: Secp256k1TrSigningCommitments = serde_json::from_slice(&commitment_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let identifier = Secp256k1TrCurve::identifier_from_u16(participant_index)?;
        self.signing_commitments.insert(identifier, commitment);
        Ok(())
    }

    pub fn sign(&mut self, message_hex: &str) -> Result<String, WasmError> {
        let message = hex::decode(message_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;

        let signing_package = Secp256k1TrCurve::create_signing_package(&self.signing_commitments, &message)?;

        let nonces = self.signing_nonces.as_ref()
            .ok_or_else(|| WasmError::new("Signing nonces not available"))?;
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;

        let signature_share = Secp256k1TrCurve::generate_signature_share(&signing_package, nonces, key_package)?;

        // See Ed25519 sign() for context — register own share so
        // aggregate_signature() covers all identifiers.
        let own_identifier = Secp256k1TrCurve::identifier_from_u16(self.participant_index)?;
        self.signature_shares.insert(own_identifier, signature_share);

        Ok(hex::encode(serde_json::to_string(&signature_share).unwrap()))
    }

    pub fn add_signature_share(&mut self, participant_index: u16, share_hex: &str) -> Result<(), WasmError> {
        let share_json = hex::decode(share_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        let share: Secp256k1TrSignatureShare = serde_json::from_slice(&share_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;

        let identifier = Secp256k1TrCurve::identifier_from_u16(participant_index)?;
        self.signature_shares.insert(identifier, share);
        Ok(())
    }

    pub fn aggregate_signature(&self, message_hex: &str) -> Result<String, WasmError> {
        let message = hex::decode(message_hex)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let signing_package = Secp256k1TrCurve::create_signing_package(&self.signing_commitments, &message)?;
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("Public key package not available"))?;
        
        let signature = Secp256k1TrCurve::aggregate_signature(&signing_package, &self.signature_shares, public_key_package)?;
        let sig_bytes = Secp256k1TrCurve::serialize_signature(&signature)?;
        
        Ok(hex::encode(sig_bytes))
    }

    pub fn clear_signing_state(&mut self) {
        self.signing_nonces = None;
        self.signing_commitments.clear();
        self.signature_shares.clear();
    }

    pub fn has_signing_nonces(&self) -> bool {
        self.signing_nonces.is_some()
    }

    pub fn import_keystore(&mut self, keystore_json: &str) -> Result<(), WasmError> {
        let keystore_data: KeystoreData = serde_json::from_str(keystore_json)
            .map_err(|e| WasmError::new(&e.to_string()))?;
        
        let (key_package, public_key_package) = Keystore::import_keystore::<Secp256k1TrCurve>(&keystore_data)?;
        
        self.key_package = Some(key_package);
        self.public_key_package = Some(public_key_package);
        self.threshold = keystore_data.min_signers;
        self.total = keystore_data.max_signers;
        self.participant_index = keystore_data.participant_index;
        self.participant_indices = keystore_data.participant_indices;
        
        Ok(())
    }

    pub fn export_keystore(&self) -> Result<String, WasmError> {
        let key_package = self.key_package.as_ref()
            .ok_or_else(|| WasmError::new("Key package not available"))?;
        let public_key_package = self.public_key_package.as_ref()
            .ok_or_else(|| WasmError::new("Public key package not available"))?;
        
        let keystore_data = Keystore::export_keystore::<Secp256k1TrCurve>(
            key_package,
            public_key_package,
            self.threshold,
            self.total,
            self.participant_index,
            self.participant_indices.clone(),
            "secp256k1-tr",
        )?;
        
        Ok(serde_json::to_string(&keystore_data).unwrap())
    }
}


// ============================================================================
