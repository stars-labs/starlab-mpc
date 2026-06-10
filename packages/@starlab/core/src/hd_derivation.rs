//! BIP-44 style HD key derivation for FROST threshold keys.
//!
//! After unified DKG, each participant holds key packages for ed25519 and secp256k1.
//! This module enables deriving child keys locally using additive scalar offsets,
//! preserving the threshold property without additional DKG rounds.
//!
//! The derivation follows BIP-32's HMAC-SHA512 approach:
//! ```text
//! offset = HMAC-SHA512(chaincode, parent_group_pubkey || index)[0..32]
//! child_share_i = parent_share_i + offset  (mod curve_order)
//! child_group_key = parent_group_key + offset * G
//! ```

use crate::errors::{FrostError, Result};
use frost_core::Ciphersuite;
// In `hmac 0.13`, `new_from_slice` moved out of `Mac` and into `KeyInit`;
// `update` / `finalize` still live on `Mac`. Both traits need to be in scope.
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256, Sha512};
use std::collections::BTreeMap;
use std::fmt;

type HmacSha512 = Hmac<Sha512>;

/// Chain code for HD key derivation (32 bytes).
///
/// Generated from the group public key after DKG finalization.
/// Used as the HMAC key for child key derivation.
#[derive(Clone)]
pub struct ChainCode([u8; 32]);

impl ChainCode {
    /// Derive the initial chain code from a group verifying key.
    ///
    /// Uses HMAC-SHA512 with a domain separator to produce a deterministic
    /// chain code from the group public key bytes.
    pub fn from_group_key(verifying_key_bytes: &[u8]) -> Self {
        let mut mac = HmacSha512::new_from_slice(b"frost-hd-root")
            .expect("HMAC accepts any key size");
        mac.update(verifying_key_bytes);
        let result = mac.finalize().into_bytes();
        let mut chaincode = [0u8; 32];
        chaincode.copy_from_slice(&result[32..64]);
        ChainCode(chaincode)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ChainCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChainCode({})", hex::encode(self.0))
    }
}

/// Result of child key derivation for a single curve.
pub struct DerivedKeys<C: Ciphersuite> {
    pub key_package: frost_core::keys::KeyPackage<C>,
    pub public_key_package: frost_core::keys::PublicKeyPackage<C>,
    pub chain_code: ChainCode,
}

/// BIP-44 derivation path: `m / purpose' / coin_type' / account' / change / index`.
///
/// Each segment is a u32. Hardened derivation is indicated by setting the
/// highest bit (0x80000000).
#[derive(Clone, Debug)]
pub struct DerivationPath {
    segments: Vec<u32>,
}

/// Marker for hardened derivation (BIP-32).
const HARDENED_BIT: u32 = 0x80000000;

impl DerivationPath {
    /// Create a derivation path from raw segments.
    pub fn new(segments: Vec<u32>) -> Self {
        Self { segments }
    }

    /// Parse a BIP-44 path string like `m/44'/501'/0'/0'`.
    ///
    /// The `'` or `h` suffix denotes hardened derivation.
    pub fn parse(path: &str) -> Result<Self> {
        let path = path.trim();
        let parts: Vec<&str> = path.split('/').collect();
        if parts.is_empty() {
            return Err(FrostError::DerivationError("empty derivation path".into()));
        }

        let start = if parts[0] == "m" { 1 } else { 0 };
        let mut segments = Vec::with_capacity(parts.len() - start);

        for part in &parts[start..] {
            let (num_str, hardened) = if part.ends_with('\'') || part.ends_with('h') {
                (&part[..part.len() - 1], true)
            } else {
                (*part, false)
            };

            let index: u32 = num_str
                .parse()
                .map_err(|_| FrostError::DerivationError(format!("invalid path segment: {part}")))?;

            if index >= HARDENED_BIT {
                return Err(FrostError::DerivationError(format!(
                    "index {index} too large (must be < 2^31)"
                )));
            }

            segments.push(if hardened { index | HARDENED_BIT } else { index });
        }

        Ok(Self { segments })
    }

    /// Get the path segments.
    pub fn segments(&self) -> &[u32] {
        &self.segments
    }

    /// Create a standard BIP-44 Solana path: `m/44'/501'/account'/0'`.
    pub fn solana(account: u32) -> Self {
        Self {
            segments: vec![
                44 | HARDENED_BIT,
                501 | HARDENED_BIT,
                account | HARDENED_BIT,
                HARDENED_BIT, // change = 0'
            ],
        }
    }

    /// Create a standard BIP-44 Ethereum path: `m/44'/60'/0'/0/index`.
    pub fn ethereum(index: u32) -> Self {
        Self {
            segments: vec![
                44 | HARDENED_BIT,
                60 | HARDENED_BIT,
                HARDENED_BIT, // account = 0'
                0,            // change = 0
                index,
            ],
        }
    }
}

impl fmt::Display for DerivationPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "m")?;
        for &seg in &self.segments {
            if seg >= HARDENED_BIT {
                write!(f, "/{}'", seg & !HARDENED_BIT)?;
            } else {
                write!(f, "/{seg}")?;
            }
        }
        Ok(())
    }
}

/// Compute HMAC-SHA512 for child key derivation.
///
/// Returns (scalar_seed_32_bytes, child_chaincode_32_bytes).
fn hmac_derive(chaincode: &[u8; 32], pubkey_bytes: &[u8], index: u32) -> ([u8; 32], [u8; 32]) {
    let mut mac = HmacSha512::new_from_slice(chaincode).expect("HMAC accepts any key size");
    mac.update(pubkey_bytes);
    mac.update(&index.to_be_bytes());
    let result = mac.finalize().into_bytes();

    let mut scalar_seed = [0u8; 32];
    scalar_seed.copy_from_slice(&result[..32]);
    let mut child_chaincode = [0u8; 32];
    child_chaincode.copy_from_slice(&result[32..64]);

    (scalar_seed, child_chaincode)
}

/// Convert a 32-byte seed into a valid non-zero scalar for the given ciphersuite.
///
/// If direct deserialization fails (e.g., value >= curve order for ed25519),
/// iteratively hashes with a counter until a valid scalar is found.
fn scalar_from_seed<C: Ciphersuite>(
    seed: &[u8; 32],
) -> Result<<<C::Group as frost_core::Group>::Field as frost_core::Field>::Scalar> {
    // Try direct deserialization
    if let Ok(serialization) = seed.to_vec().try_into()
        && let Ok(scalar) =
            <<C::Group as frost_core::Group>::Field as frost_core::Field>::deserialize(
                &serialization,
            )
        {
            return Ok(scalar);
        }

    // Retry with SHA-256(seed || counter) for values that exceed curve order
    for counter in 1u8..=255 {
        let mut hasher = Sha256::new();
        hasher.update(seed);
        hasher.update([counter]);
        let hash = hasher.finalize();

        if let Ok(serialization) = hash.to_vec().try_into()
            && let Ok(scalar) =
                <<C::Group as frost_core::Group>::Field as frost_core::Field>::deserialize(
                    &serialization,
                )
            {
                return Ok(scalar);
            }
    }

    Err(FrostError::DerivationError(
        "failed to derive valid scalar after 256 attempts".into(),
    ))
}

/// Deserialize bytes into a field scalar.
fn bytes_to_scalar<C: Ciphersuite>(
    bytes: &[u8],
) -> Result<<<C::Group as frost_core::Group>::Field as frost_core::Field>::Scalar> {
    let serialization = bytes
        .to_vec()
        .try_into()
        .map_err(|_| FrostError::DerivationError("scalar size mismatch".into()))?;
    <<C::Group as frost_core::Group>::Field as frost_core::Field>::deserialize(&serialization)
        .map_err(|e| FrostError::DerivationError(format!("scalar deserialization: {e}")))
}

/// Deserialize bytes into a group element.
fn bytes_to_element<C: Ciphersuite>(
    bytes: &[u8],
) -> Result<<C::Group as frost_core::Group>::Element> {
    let serialization = bytes
        .to_vec()
        .try_into()
        .map_err(|_| FrostError::DerivationError("element size mismatch".into()))?;
    <C::Group as frost_core::Group>::deserialize(&serialization)
        .map_err(|e| FrostError::DerivationError(format!("element deserialization: {e}")))
}

/// Derive a child key package from parent key packages at a given index.
///
/// This is the core HD derivation function. It shifts every participant's
/// signing share by the same scalar offset, preserving the threshold property.
///
/// # Arguments
/// * `key_package` - This participant's parent key package
/// * `public_key_package` - The parent public key package (shared by all participants)
/// * `chain_code` - The parent chain code
/// * `index` - The child index (u32, use `| 0x80000000` for hardened)
///
/// # Returns
/// Child key package, child public key package, and child chain code.
pub fn derive_child_key<C: Ciphersuite>(
    key_package: &frost_core::keys::KeyPackage<C>,
    public_key_package: &frost_core::keys::PublicKeyPackage<C>,
    chain_code: &ChainCode,
    index: u32,
) -> Result<DerivedKeys<C>> {
    // Step 1: HMAC-SHA512(chaincode, group_pubkey || index)
    let group_pubkey_bytes = public_key_package
        .verifying_key()
        .serialize()
        .map_err(|e| FrostError::DerivationError(format!("serialize verifying key: {e}")))?;

    let (scalar_seed, child_chaincode_bytes) =
        hmac_derive(chain_code.as_bytes(), group_pubkey_bytes.as_ref(), index);
    let child_chain_code = ChainCode(child_chaincode_bytes);

    // Step 2: Derive offset scalar
    let offset_scalar = scalar_from_seed::<C>(&scalar_seed)?;

    // Step 3: Compute offset point = generator * offset
    let generator = <C::Group as frost_core::Group>::generator();
    let offset_point = generator * offset_scalar;

    // Step 4: Derive child signing share
    let parent_share_bytes = key_package.signing_share().serialize();
    let parent_scalar = bytes_to_scalar::<C>(&parent_share_bytes)?;
    let child_scalar = parent_scalar + offset_scalar;
    let child_share_bytes =
        <<C::Group as frost_core::Group>::Field as frost_core::Field>::serialize(&child_scalar);
    let child_signing_share =
        frost_core::keys::SigningShare::<C>::deserialize(child_share_bytes.as_ref()).map_err(
            |e| FrostError::DerivationError(format!("child signing share: {e}")),
        )?;

    // Step 5: Derive child verifying share for this participant
    let parent_vshare_bytes = key_package
        .verifying_share()
        .serialize()
        .map_err(|e| FrostError::DerivationError(format!("serialize verifying share: {e}")))?;
    let parent_vshare_element = bytes_to_element::<C>(parent_vshare_bytes.as_ref())?;
    let child_vshare_element = parent_vshare_element + offset_point;
    let child_vshare_bytes =
        <C::Group as frost_core::Group>::serialize(&child_vshare_element)
            .map_err(|e| FrostError::DerivationError(format!("serialize child vshare: {e}")))?;
    let child_verifying_share =
        frost_core::keys::VerifyingShare::<C>::deserialize(child_vshare_bytes.as_ref()).map_err(
            |e| FrostError::DerivationError(format!("child verifying share: {e}")),
        )?;

    // Step 6: Derive child verifying key (group public key)
    let parent_vk_bytes = public_key_package
        .verifying_key()
        .serialize()
        .map_err(|e| FrostError::DerivationError(format!("serialize vk: {e}")))?;
    let parent_vk_element = bytes_to_element::<C>(parent_vk_bytes.as_ref())?;
    let child_vk_element = parent_vk_element + offset_point;
    let child_vk_bytes = <C::Group as frost_core::Group>::serialize(&child_vk_element)
        .map_err(|e| FrostError::DerivationError(format!("serialize child vk: {e}")))?;
    let child_verifying_key =
        frost_core::VerifyingKey::<C>::deserialize(child_vk_bytes.as_ref())
            .map_err(|e| FrostError::DerivationError(format!("child verifying key: {e}")))?;

    // Step 7: Derive child verifying shares for all participants
    let mut child_verifying_shares = BTreeMap::new();
    for (id, vs) in public_key_package.verifying_shares() {
        let vs_bytes = vs
            .serialize()
            .map_err(|e| FrostError::DerivationError(format!("serialize vs: {e}")))?;
        let vs_element = bytes_to_element::<C>(vs_bytes.as_ref())?;
        let child_vs_element = vs_element + offset_point;
        let child_vs_bytes = <C::Group as frost_core::Group>::serialize(&child_vs_element)
            .map_err(|e| FrostError::DerivationError(format!("serialize child vs: {e}")))?;
        let child_vs =
            frost_core::keys::VerifyingShare::<C>::deserialize(child_vs_bytes.as_ref())
                .map_err(|e| FrostError::DerivationError(format!("child vs: {e}")))?;
        child_verifying_shares.insert(*id, child_vs);
    }

    // Step 8: Construct child packages
    let child_key_package = frost_core::keys::KeyPackage::<C>::new(
        *key_package.identifier(),
        child_signing_share,
        child_verifying_share,
        child_verifying_key,
        *key_package.min_signers(),
    );

    let child_public_key_package = frost_core::keys::PublicKeyPackage::<C>::new(
        child_verifying_shares,
        child_verifying_key,
    );

    Ok(DerivedKeys {
        key_package: child_key_package,
        public_key_package: child_public_key_package,
        chain_code: child_chain_code,
    })
}

/// Derive a child key by following an entire derivation path.
///
/// Chains multiple single-index derivations to follow a BIP-44 style path.
pub fn derive_child_key_path<C: Ciphersuite>(
    key_package: &frost_core::keys::KeyPackage<C>,
    public_key_package: &frost_core::keys::PublicKeyPackage<C>,
    chain_code: &ChainCode,
    path: &DerivationPath,
) -> Result<DerivedKeys<C>> {
    let mut current_kp = key_package.clone();
    let mut current_pub = public_key_package.clone();
    let mut current_cc = chain_code.clone();

    for &index in path.segments() {
        let derived = derive_child_key::<C>(&current_kp, &current_pub, &current_cc, index)?;
        current_kp = derived.key_package;
        current_pub = derived.public_key_package;
        current_cc = derived.chain_code;
    }

    Ok(DerivedKeys {
        key_package: current_kp,
        public_key_package: current_pub,
        chain_code: current_cc,
    })
}

/// PUBLIC-ONLY path derivation: compute a child GROUP VERIFYING KEY from just
/// the parent group key — no shares, no secrets, no password.
///
/// Sound because the per-level offset comes from public material only:
/// `offset = scalar(HMAC(chain_code, parent_group_key ‖ index))` with the
/// root chain code itself derived from the group key. This is what lets a
/// wallet list receive addresses for account 0..N without unlocking
/// anything; signing for those accounts still requires the (private) share
/// derivation via [`derive_child_key_path`]. The two are consistent by
/// construction — pinned by `public_derivation_matches_full_derivation`.
pub fn derive_child_verifying_key_path<C: Ciphersuite>(
    group_verifying_key_bytes: &[u8],
    path: &DerivationPath,
) -> Result<Vec<u8>> {
    use frost_core::Group;

    let deserialize = |bytes: &[u8]| -> Result<<C::Group as Group>::Element> {
        let ser = <C::Group as Group>::Serialization::try_from(bytes.to_vec())
            .map_err(|_| FrostError::DerivationError("bad group element length".into()))?;
        <C::Group as Group>::deserialize(&ser)
            .map_err(|e| FrostError::DerivationError(format!("bad group element: {e}")))
    };
    let serialize = |elem: &<C::Group as Group>::Element| -> Result<Vec<u8>> {
        <C::Group as Group>::serialize(elem)
            .map(|s| s.as_ref().to_vec())
            .map_err(|e| FrostError::DerivationError(format!("serialize element: {e}")))
    };

    let mut current = deserialize(group_verifying_key_bytes)?;
    let mut chain_code = ChainCode::from_group_key(group_verifying_key_bytes);

    for &index in path.segments() {
        let parent_bytes = serialize(&current)?;
        let (scalar_seed, child_cc) = hmac_derive(chain_code.as_bytes(), &parent_bytes, index);
        let offset = scalar_from_seed::<C>(&scalar_seed)?;
        let generator = <C::Group as Group>::generator();
        current = current + generator * offset;
        chain_code = ChainCode(child_cc);
    }
    serialize(&current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_dkg::UnifiedDkg;

    /// Run a full 2-of-3 unified DKG, returning finalized participants.
    fn run_dkg() -> Vec<UnifiedDkg> {
        let max_signers: u16 = 3;
        let min_signers: u16 = 2;

        let mut participants: Vec<UnifiedDkg> = (1..=max_signers)
            .map(|i| {
                let mut dkg = UnifiedDkg::new();
                dkg.init_dkg(i, max_signers, min_signers);
                dkg
            })
            .collect();

        // Round 1
        let round1_packages: Vec<_> = participants
            .iter_mut()
            .map(|p| p.generate_round1().unwrap())
            .collect();

        for (sender, pkg) in round1_packages.iter().enumerate() {
            let sender_id = (sender + 1) as u16;
            for (receiver, participant) in participants.iter_mut().enumerate() {
                if sender == receiver {
                    continue;
                }
                participant.add_round1_package(sender_id, pkg).unwrap();
            }
        }

        // Round 2
        let round2_packages: Vec<_> = participants
            .iter_mut()
            .map(|p| p.generate_round2().unwrap())
            .collect();

        for (sender, sender_pkgs) in round2_packages.iter().enumerate() {
            let sender_id = (sender + 1) as u16;
            for (receiver, participant) in participants.iter_mut().enumerate() {
                let receiver_id = (receiver + 1) as u16;
                if sender_id == receiver_id {
                    continue;
                }
                let ed_hex = sender_pkgs.ed25519.get(&receiver_id).unwrap();
                let secp_hex = sender_pkgs.secp256k1.get(&receiver_id).unwrap();
                participant
                    .add_round2_package(sender_id, ed_hex, secp_hex)
                    .unwrap();
            }
        }

        // Finalize
        for p in &mut participants {
            p.finalize_dkg().unwrap();
        }

        participants
    }

    #[test]
    fn test_child_key_determinism() {
        let participants = run_dkg();
        let p = &participants[0];

        let ed_kp = p.ed25519_key_package().unwrap();
        let ed_pub = p.ed25519_public_key_package().unwrap();
        let ed_vk_bytes = ed_pub.verifying_key().serialize().unwrap();
        let cc = ChainCode::from_group_key(ed_vk_bytes.as_ref());

        let derived1 = derive_child_key::<frost_ed25519::Ed25519Sha512>(ed_kp, ed_pub, &cc, 0).unwrap();
        let derived2 = derive_child_key::<frost_ed25519::Ed25519Sha512>(ed_kp, ed_pub, &cc, 0).unwrap();

        let vk1 = derived1.public_key_package.verifying_key().serialize().unwrap();
        let vk2 = derived2.public_key_package.verifying_key().serialize().unwrap();
        assert_eq!(vk1, vk2, "same inputs must produce same child key");
    }

    #[test]
    fn test_different_indices_different_keys() {
        let participants = run_dkg();
        let p = &participants[0];

        let ed_kp = p.ed25519_key_package().unwrap();
        let ed_pub = p.ed25519_public_key_package().unwrap();
        let ed_vk_bytes = ed_pub.verifying_key().serialize().unwrap();
        let cc = ChainCode::from_group_key(ed_vk_bytes.as_ref());

        let derived0 = derive_child_key::<frost_ed25519::Ed25519Sha512>(ed_kp, ed_pub, &cc, 0).unwrap();
        let derived1 = derive_child_key::<frost_ed25519::Ed25519Sha512>(ed_kp, ed_pub, &cc, 1).unwrap();

        let vk0 = derived0.public_key_package.verifying_key().serialize().unwrap();
        let vk1 = derived1.public_key_package.verifying_key().serialize().unwrap();
        assert_ne!(vk0, vk1, "different indices must produce different keys");

        // Also verify secp256k1
        let secp_kp = p.secp256k1_key_package().unwrap();
        let secp_pub = p.secp256k1_public_key_package().unwrap();
        let secp_vk_bytes = secp_pub.verifying_key().serialize().unwrap();
        let secp_cc = ChainCode::from_group_key(secp_vk_bytes.as_ref());

        let secp_d0 = derive_child_key::<frost_secp256k1::Secp256K1Sha256>(secp_kp, secp_pub, &secp_cc, 0).unwrap();
        let secp_d1 = derive_child_key::<frost_secp256k1::Secp256K1Sha256>(secp_kp, secp_pub, &secp_cc, 1).unwrap();

        let svk0 = secp_d0.public_key_package.verifying_key().serialize().unwrap();
        let svk1 = secp_d1.public_key_package.verifying_key().serialize().unwrap();
        assert_ne!(svk0, svk1, "different indices must produce different secp256k1 keys");
    }

    #[test]
    fn test_all_participants_agree() {
        let participants = run_dkg();

        // All participants should derive the same child group public key
        let ed_pub = participants[0].ed25519_public_key_package().unwrap();
        let ed_vk_bytes = ed_pub.verifying_key().serialize().unwrap();
        let cc = ChainCode::from_group_key(ed_vk_bytes.as_ref());

        let child_vks: Vec<_> = participants
            .iter()
            .map(|p| {
                let derived = derive_child_key::<frost_ed25519::Ed25519Sha512>(
                    p.ed25519_key_package().unwrap(),
                    p.ed25519_public_key_package().unwrap(),
                    &cc,
                    42,
                )
                .unwrap();
                derived
                    .public_key_package
                    .verifying_key()
                    .serialize()
                    .unwrap()
            })
            .collect();

        for i in 1..child_vks.len() {
            assert_eq!(
                child_vks[0], child_vks[i],
                "participant {} derived different child group pubkey",
                i + 1
            );
        }
    }

    #[test]
    fn test_child_threshold_signing_ed25519() {
        let participants = run_dkg();

        // Derive child keys for all participants at index 7
        let ed_pub = participants[0].ed25519_public_key_package().unwrap();
        let ed_vk_bytes = ed_pub.verifying_key().serialize().unwrap();
        let cc = ChainCode::from_group_key(ed_vk_bytes.as_ref());

        let child_keys: Vec<_> = participants
            .iter()
            .map(|p| {
                derive_child_key::<frost_ed25519::Ed25519Sha512>(
                    p.ed25519_key_package().unwrap(),
                    p.ed25519_public_key_package().unwrap(),
                    &cc,
                    7,
                )
                .unwrap()
            })
            .collect();

        // Threshold sign with first 2 participants (2-of-3)
        let message = b"child key signing test";
        let signer_indices: Vec<usize> = vec![0, 1];

        let mut nonces = BTreeMap::new();
        let mut commitments = BTreeMap::new();

        for &idx in &signer_indices {
            let kp = &child_keys[idx].key_package;
            let (n, c) =
                frost_ed25519::round1::commit(kp.signing_share(), &mut rand_core::OsRng);
            let id = *kp.identifier();
            nonces.insert(id, n);
            commitments.insert(id, c);
        }

        let signing_pkg = frost_ed25519::SigningPackage::new(commitments, message);
        let mut sig_shares = BTreeMap::new();

        for &idx in &signer_indices {
            let kp = &child_keys[idx].key_package;
            let id = *kp.identifier();
            let share = frost_ed25519::round2::sign(&signing_pkg, &nonces[&id], kp).unwrap();
            sig_shares.insert(id, share);
        }

        let child_pub = &child_keys[0].public_key_package;
        let signature = frost_ed25519::aggregate(&signing_pkg, &sig_shares, child_pub).unwrap();

        child_pub
            .verifying_key()
            .verify(message, &signature)
            .expect("child key ed25519 signature must verify");
    }

    #[test]
    fn test_child_threshold_signing_secp256k1() {
        let participants = run_dkg();

        let secp_pub = participants[0].secp256k1_public_key_package().unwrap();
        let secp_vk_bytes = secp_pub.verifying_key().serialize().unwrap();
        let cc = ChainCode::from_group_key(secp_vk_bytes.as_ref());

        let child_keys: Vec<_> = participants
            .iter()
            .map(|p| {
                derive_child_key::<frost_secp256k1::Secp256K1Sha256>(
                    p.secp256k1_key_package().unwrap(),
                    p.secp256k1_public_key_package().unwrap(),
                    &cc,
                    3,
                )
                .unwrap()
            })
            .collect();

        let message = b"child key secp256k1 signing test";
        let signer_indices: Vec<usize> = vec![0, 2]; // participants 1 and 3

        let mut nonces = BTreeMap::new();
        let mut commitments = BTreeMap::new();

        for &idx in &signer_indices {
            let kp = &child_keys[idx].key_package;
            let (n, c) =
                frost_secp256k1::round1::commit(kp.signing_share(), &mut rand_core::OsRng);
            let id = *kp.identifier();
            nonces.insert(id, n);
            commitments.insert(id, c);
        }

        let signing_pkg = frost_secp256k1::SigningPackage::new(commitments, message);
        let mut sig_shares = BTreeMap::new();

        for &idx in &signer_indices {
            let kp = &child_keys[idx].key_package;
            let id = *kp.identifier();
            let share = frost_secp256k1::round2::sign(&signing_pkg, &nonces[&id], kp).unwrap();
            sig_shares.insert(id, share);
        }

        let child_pub = &child_keys[0].public_key_package;
        let signature =
            frost_secp256k1::aggregate(&signing_pkg, &sig_shares, child_pub).unwrap();

        child_pub
            .verifying_key()
            .verify(message, &signature)
            .expect("child key secp256k1 signature must verify");
    }

    #[test]
    fn test_derivation_path_parse() {
        let path = DerivationPath::parse("m/44'/501'/0'/0'").unwrap();
        assert_eq!(path.segments().len(), 4);
        assert_eq!(path.segments()[0], 44 | HARDENED_BIT);
        assert_eq!(path.segments()[1], 501 | HARDENED_BIT);
        assert_eq!(path.segments()[2], HARDENED_BIT); // 0'
        assert_eq!(path.segments()[3], HARDENED_BIT); // 0'
        assert_eq!(path.to_string(), "m/44'/501'/0'/0'");

        // Non-hardened
        let path2 = DerivationPath::parse("m/44'/60'/0'/0/5").unwrap();
        assert_eq!(path2.segments()[3], 0);
        assert_eq!(path2.segments()[4], 5);
    }

    #[test]
    fn test_derivation_path_chain() {
        let participants = run_dkg();
        let p = &participants[0];

        let ed_kp = p.ed25519_key_package().unwrap();
        let ed_pub = p.ed25519_public_key_package().unwrap();
        let ed_vk_bytes = ed_pub.verifying_key().serialize().unwrap();
        let cc = ChainCode::from_group_key(ed_vk_bytes.as_ref());

        let path = DerivationPath::parse("m/44'/501'/0'").unwrap();
        let derived = derive_child_key_path::<frost_ed25519::Ed25519Sha512>(ed_kp, ed_pub, &cc, &path).unwrap();

        // Verify it produces a valid key by checking we can serialize
        let _vk_bytes = derived.public_key_package.verifying_key().serialize().unwrap();
    }
    #[test]
    fn public_derivation_matches_full_derivation() {
        use crate::resharing::dkg_keypackages;
        use frost_secp256k1::Secp256K1Sha256 as Secp;
        let (kps, pp) = dkg_keypackages::<Secp>(2, 2, 51).unwrap();
        let group = pp.verifying_key().serialize().unwrap();
        let path = DerivationPath::parse("m/44'/60'/0'/0/7").unwrap();

        // Full (share-bearing) derivation
        let cc = ChainCode::from_group_key(&group);
        let full = derive_child_key_path::<Secp>(&kps[&1], &pp, &cc, &path).unwrap();
        let full_group = full.public_key_package.verifying_key().serialize().unwrap();

        // Public-only derivation from just the group key bytes
        let public_only = derive_child_verifying_key_path::<Secp>(&group, &path).unwrap();

        assert_eq!(public_only, full_group);
    }

    #[test]
    fn public_derivation_works_on_ed25519_too() {
        use crate::resharing::dkg_keypackages;
        use frost_ed25519::Ed25519Sha512 as Ed;
        let (kps, pp) = dkg_keypackages::<Ed>(2, 2, 52).unwrap();
        let group = pp.verifying_key().serialize().unwrap();
        let path = DerivationPath::parse("m/44'/501'/3'/0'").unwrap();
        let cc = ChainCode::from_group_key(&group);
        let full = derive_child_key_path::<Ed>(&kps[&1], &pp, &cc, &path).unwrap();
        let full_group = full.public_key_package.verifying_key().serialize().unwrap();
        let public_only = derive_child_verifying_key_path::<Ed>(&group, &path).unwrap();
        assert_eq!(public_only, full_group);
    }

}
