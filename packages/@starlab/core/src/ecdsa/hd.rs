//! Our HD derivation ([`crate::hd_derivation`]) plugged into cggmp24.
//!
//! cggmp24 derives child keys inside the signing protocol through the
//! `hd_wallet::HdWallet` trait (blanket-implemented for every
//! [`DeriveShift`]): it folds `derive_public_shift` over the path starting
//! from the key share's extended public key and adds the summed shift to the
//! key. [`StarlabHd`] implements that step with exactly the math of
//! [`crate::hd_derivation::derive_child_public_key_raw`] on secp256k1:
//!
//! ```text
//! (seed ‖ child_cc) = HMAC-SHA512(chain_code, SEC1_compressed(parent) ‖ index_be32)
//! shift             = scalar_from_seed(seed)       (same retry rule as FROST)
//! child             = parent + shift·G
//! root chain_code   = ChainCode::from_group_key(SEC1_compressed(group_key))
//! ```
//!
//! so a signature produced at path `m/44'/60'/0'/0/n` verifies under exactly
//! the key `accounts.rs` derives public-only. No BIP-86 finalize for ECDSA.
//!
//! **Hardened flag.** cggmp24 only passes `NonHardenedIndex` (31 bits) to
//! the trait, but our scheme has no real hardened derivation: every level is
//! the public additive offset above (that is what keeps addresses derivable
//! without a share), the hardened bit is just part of the HMAC input. We carry
//! it through cggmp24's index type in bit 30 ([`encode_path`] /
//! [`decode_index`]), which caps a path segment at `2^30 - 1` — far above
//! any account index a wallet uses.

use crate::errors::{FrostError, Result};
use crate::hd_derivation::{ChainCode, DerivationPath, hmac_derive, scalar_from_seed};
use cggmp24::generic_ec::{Point, Scalar};
use cggmp24::hd_wallet::{
    DeriveShift, DerivedShift, ExtendedKeyPair, ExtendedPublicKey, HardenedIndex, HdWallet,
    NonHardenedIndex,
};
use cggmp24::supported_curves::Secp256k1;
use frost_core::{Ciphersuite, Field, Group};
use frost_secp256k1_tr::Secp256K1Sha256TR;

/// BIP-32 hardened bit as used in [`DerivationPath`] segments.
const HARDENED: u32 = 1 << 31;
/// Where the hardened bit travels inside cggmp24's 31-bit `NonHardenedIndex`.
const HARDENED_IN_CGGMP_INDEX: u32 = 1 << 30;

/// Our derivation as a cggmp24 / `hd_wallet` HD algorithm.
pub struct StarlabHd;

/// Map a [`DerivationPath`] onto cggmp24's `NonHardenedIndex` values
/// (hardened bit moved to bit 30). Errors on a segment `>= 2^30`.
pub(crate) fn encode_path(path: &DerivationPath) -> Result<Vec<u32>> {
    path.segments()
        .iter()
        .map(|&seg| {
            let index = seg & !HARDENED;
            if index >= HARDENED_IN_CGGMP_INDEX {
                return Err(FrostError::DerivationError(format!(
                    "ECDSA path segment {index} too large (must be < 2^30)"
                )));
            }
            Ok(if seg & HARDENED != 0 {
                index | HARDENED_IN_CGGMP_INDEX
            } else {
                index
            })
        })
        .collect()
}

/// Inverse of [`encode_path`] for one segment: back to the BIP-32 `u32`.
fn decode_index(encoded: u32) -> u32 {
    if encoded & HARDENED_IN_CGGMP_INDEX != 0 {
        (encoded & !HARDENED_IN_CGGMP_INDEX) | HARDENED
    } else {
        encoded
    }
}

/// One derivation level, `index` being the full BIP-32 `u32` (hardened bit
/// included in the HMAC input).
fn derive_step(parent: &ExtendedPublicKey<Secp256k1>, index: u32) -> DerivedShift<Secp256k1> {
    let parent_bytes = parent.public_key.to_bytes(true);
    let (seed, child_chain_code) = hmac_derive(&parent.chain_code, &parent_bytes, index);
    // Same seed → scalar rule as the FROST secp256k1 derivation (the -tr
    // suite's field is plain secp256k1 Z_n; only its signing differs).
    let offset = scalar_from_seed::<Secp256K1Sha256TR>(&seed)
        .expect("a valid scalar within 256 rehashes (failure probability ~2^-32000)");
    let offset_bytes =
        <<<Secp256K1Sha256TR as Ciphersuite>::Group as Group>::Field as Field>::serialize(&offset);
    let shift = Scalar::<Secp256k1>::from_be_bytes(offset_bytes)
        .expect("a reduced secp256k1 scalar is valid in generic-ec");
    DerivedShift {
        shift,
        child_public_key: ExtendedPublicKey {
            public_key: parent.public_key + Point::generator() * shift,
            chain_code: child_chain_code,
        },
    }
}

impl DeriveShift<Secp256k1> for StarlabHd {
    fn derive_public_shift(
        parent_public_key: &ExtendedPublicKey<Secp256k1>,
        child_index: NonHardenedIndex,
    ) -> DerivedShift<Secp256k1> {
        derive_step(parent_public_key, decode_index(*child_index))
    }

    /// Our "hardened" levels are public-additive too (see module docs), so
    /// this is the same step with the hardened bit in the HMAC input.
    /// cggmp24's signing never calls it; kept consistent for completeness.
    fn derive_hardened_shift(
        parent_key: &ExtendedKeyPair<Secp256k1>,
        child_index: HardenedIndex,
    ) -> DerivedShift<Secp256k1> {
        derive_step(parent_key.public_key(), *child_index)
    }
}

/// Root extended public key of an ECDSA group key: the group key plus the
/// chain code derived from it (same rule as FROST).
pub(crate) fn root_extended_key(group_key: Point<Secp256k1>) -> ExtendedPublicKey<Secp256k1> {
    let chain_code = ChainCode::from_group_key(&group_key.to_bytes(true));
    ExtendedPublicKey {
        public_key: group_key,
        chain_code: *chain_code.as_bytes(),
    }
}

/// PUBLIC-ONLY child key (SEC1 compressed, 33 bytes) at `path`, computed
/// through [`StarlabHd`] — the exact code cggmp24 runs while signing.
pub fn derive_public_key(group_key_sec1: &[u8], path: &DerivationPath) -> Result<[u8; 33]> {
    let group_key = Point::<Secp256k1>::from_bytes(group_key_sec1)
        .map_err(|e| FrostError::DerivationError(format!("bad secp256k1 group key: {e}")))?;
    let indices = encode_path(path)?
        .into_iter()
        .map(|i| {
            NonHardenedIndex::try_from(i)
                .map_err(|e| FrostError::DerivationError(format!("index {i}: {e}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let child =
        StarlabHd::derive_child_public_key_with_path(&root_extended_key(group_key), indices);
    child
        .public_key
        .to_bytes(true)
        .as_bytes()
        .try_into()
        .map_err(|_| FrostError::DerivationError("child key is the identity".into()))
}
