//! Root secret module for unified multi-curve key derivation.
//!
//! A single 32-byte root secret is used to deterministically derive
//! curve-specific RNG seeds via HKDF. This ensures that one DKG session
//! can produce key packages for multiple curves (e.g. ed25519 + secp256k1)
//! from the same entropy source.
//!
//! # Domain separation (the HKDF `info` grammar)
//!
//! Every consumer of a [`RootSecret`] derives its key material through
//! HKDF-SHA256 with a **structured, versioned `info` string** so that no two
//! derivation purposes can ever collide on the same byte stream. The grammar
//! is fixed and exhaustive:
//!
//! ```text
//! info := "frost-dkg" "/" VERSION "/" CURVE "/" ACCOUNT
//! VERSION := "v1"                       ; bump on ANY change to the pipeline
//! CURVE   := "ed25519" | "secp256k1"    ; the canonical curve tag
//! ACCOUNT := <decimal u32, no leading zeros except "0">
//! ```
//!
//! Examples: `frost-dkg/v1/ed25519/0`, `frost-dkg/v1/secp256k1/3`.
//!
//! Rules that make this a *deliberate* scheme rather than an incidental one:
//!
//! - **Versioned.** [`DERIVATION_VERSION`] is part of every `info`. If the KDF
//!   pipeline (hash, RNG, encoding) ever changes, bump it — old and new
//!   material are then domain-separated and a refactor can't silently reuse a
//!   byte stream that meant something else.
//! - **Account-indexed.** A `u32` account index lets one root yield multiple
//!   independent wallets *per curve* (BIP-44-style `account`), defaulting to 0.
//! - **Single namespace.** All `info` strings begin with the `frost-dkg/`
//!   prefix. Any *future* non-DKG consumer of the root (e.g. an encryption
//!   subkey) MUST pick a different, non-overlapping prefix (e.g.
//!   `frost-enc/v1/...`) so it can never share an `info` with DKG randomness.
//!   The keystore KDF (`keystore::encryption`) is **not** a consumer of this
//!   root — it derives from the user password via PBKDF2/Argon2, a disjoint
//!   label space — so there is no collision risk there.
//! - **Byte-locked.** The exact UTF-8 bytes of the `info` for each curve are
//!   pinned by a regression test (`info_bytes_are_locked`), so the derived
//!   material is stable across refactors.
//!
//! ## Salt
//!
//! HKDF is used with an **empty salt** (`None`). Domain separation is carried
//! entirely by the structured `info` above, which is the standard and
//! sufficient construction for separating purposes that share one IKM (RFC
//! 5869 §3.1: a salt adds value mainly when the IKM is low-entropy or reused
//! across *independent* protocols; here the IKM is a 32-byte CSPRNG secret and
//! every purpose is already uniquely labelled). If a future scheme ever needs
//! to derive from a caller-supplied or lower-entropy root, introduce a
//! per-deployment salt **and** bump [`DERIVATION_VERSION`].

use crate::errors::{FrostError, Result};
use hkdf::Hkdf;
use rand_chacha::ChaCha20Rng;
// rand_core 0.6 is the version FROST accepts; `rand` crate isn't used here.
use rand_core::{OsRng, RngCore, SeedableRng};
use sha2::Sha256;

const ROOT_SECRET_LEN: usize = 32;

/// Version of the key-derivation scheme, embedded in every HKDF `info`.
///
/// Bump this whenever the derivation pipeline changes in any
/// material-affecting way (hash, RNG, encoding, prefix). Material derived
/// under different versions is domain-separated.
pub const DERIVATION_VERSION: &str = "v1";

/// Canonical curve tag for ed25519 (Solana / Sui / Aptos / NEAR).
pub const CURVE_ED25519: &str = "ed25519";
/// Canonical curve tag for secp256k1 (Ethereum-family + Bitcoin).
pub const CURVE_SECP256K1: &str = "secp256k1";

/// Build the canonical HKDF `info` string for a `(curve, account)` derivation.
///
/// This is the single source of truth for the domain-separation grammar
/// documented at the module level: `frost-dkg/<version>/<curve>/<account>`.
pub fn dkg_info(curve_tag: &str, account: u32) -> String {
    format!("frost-dkg/{DERIVATION_VERSION}/{curve_tag}/{account}")
}

/// A 32-byte root secret from which curve-specific DKG randomness is derived.
#[derive(Clone)]
pub struct RootSecret([u8; ROOT_SECRET_LEN]);

impl RootSecret {
    /// Generate a new root secret from OS randomness.
    pub fn generate() -> Self {
        let mut bytes = [0u8; ROOT_SECRET_LEN];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Create a root secret from raw bytes.
    pub fn from_bytes(bytes: [u8; ROOT_SECRET_LEN]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes of the root secret.
    pub fn as_bytes(&self) -> &[u8; ROOT_SECRET_LEN] {
        &self.0
    }

    /// Derive a deterministic `ChaCha20Rng` from a fully-formed HKDF `info`.
    ///
    /// Prefer [`RootSecret::derive_dkg_rng`] (which builds the canonical
    /// `info` for you); this lower-level entry point exists for callers that
    /// have already composed an `info` via [`dkg_info`] or that need a
    /// non-DKG namespace.
    pub fn derive_rng(&self, info: &str) -> Result<ChaCha20Rng> {
        let hk = Hkdf::<Sha256>::new(None, &self.0);
        let mut seed = [0u8; 32];
        hk.expand(info.as_bytes(), &mut seed)
            .map_err(|e| FrostError::DkgError(format!("HKDF expand failed: {}", e)))?;
        Ok(ChaCha20Rng::from_seed(seed))
    }

    /// Derive a deterministic DKG RNG for `(curve, account)` using the
    /// canonical, versioned domain-separation grammar.
    pub fn derive_dkg_rng(&self, curve_tag: &str, account: u32) -> Result<ChaCha20Rng> {
        self.derive_rng(&dkg_info(curve_tag, account))
    }

    /// Derive the raw 32-byte HKDF seed for `(curve, account)` — the same
    /// material [`derive_dkg_rng`] feeds into ChaCha20. Exposed for callers
    /// (e.g. the curve registry) that need the seed bytes directly.
    pub fn derive_seed(&self, curve_tag: &str, account: u32) -> Result<[u8; 32]> {
        let hk = Hkdf::<Sha256>::new(None, &self.0);
        let mut seed = [0u8; 32];
        hk.expand(dkg_info(curve_tag, account).as_bytes(), &mut seed)
            .map_err(|e| FrostError::DkgError(format!("HKDF expand failed: {}", e)))?;
        Ok(seed)
    }

    /// Derive a deterministic RNG for the ed25519 curve DKG (account 0).
    pub fn derive_ed25519_rng(&self) -> Result<ChaCha20Rng> {
        self.derive_dkg_rng(CURVE_ED25519, 0)
    }

    /// Derive a deterministic RNG for the ed25519 curve DKG at `account`.
    pub fn derive_ed25519_rng_for_account(&self, account: u32) -> Result<ChaCha20Rng> {
        self.derive_dkg_rng(CURVE_ED25519, account)
    }

    /// Derive a deterministic RNG for the secp256k1 curve DKG (account 0).
    pub fn derive_secp256k1_rng(&self) -> Result<ChaCha20Rng> {
        self.derive_dkg_rng(CURVE_SECP256K1, 0)
    }

    /// Derive a deterministic RNG for the secp256k1 curve DKG at `account`.
    pub fn derive_secp256k1_rng_for_account(&self, account: u32) -> Result<ChaCha20Rng> {
        self.derive_dkg_rng(CURVE_SECP256K1, account)
    }
}

impl Drop for RootSecret {
    fn drop(&mut self) {
        // Zeroize on drop for security
        self.0.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_bytes(rng: &mut ChaCha20Rng) -> [u8; 32] {
        let mut buf = [0u8; 32];
        rng.fill_bytes(&mut buf);
        buf
    }

    #[test]
    fn test_deterministic_derivation() {
        let secret = RootSecret::from_bytes([42u8; 32]);
        let mut rng1 = secret.derive_ed25519_rng().unwrap();
        let mut rng2 = secret.derive_ed25519_rng().unwrap();
        assert_eq!(
            first_bytes(&mut rng1),
            first_bytes(&mut rng2),
            "Same root secret must produce same RNG output"
        );
    }

    #[test]
    fn test_different_curves_produce_different_rngs() {
        let secret = RootSecret::from_bytes([42u8; 32]);
        let mut ed_rng = secret.derive_ed25519_rng().unwrap();
        let mut secp_rng = secret.derive_secp256k1_rng().unwrap();
        assert_ne!(
            first_bytes(&mut ed_rng),
            first_bytes(&mut secp_rng),
            "Different curves must produce different RNG output"
        );
    }

    #[test]
    fn different_accounts_produce_independent_rngs() {
        let secret = RootSecret::from_bytes([7u8; 32]);
        let mut a0 = secret.derive_secp256k1_rng_for_account(0).unwrap();
        let mut a1 = secret.derive_secp256k1_rng_for_account(1).unwrap();
        let mut a2 = secret.derive_secp256k1_rng_for_account(2).unwrap();
        let b0 = first_bytes(&mut a0);
        let b1 = first_bytes(&mut a1);
        let b2 = first_bytes(&mut a2);
        assert_ne!(b0, b1, "account 0 vs 1 must differ");
        assert_ne!(b1, b2, "account 1 vs 2 must differ");
        assert_ne!(b0, b2, "account 0 vs 2 must differ");
    }

    #[test]
    fn account_zero_matches_the_default_helper() {
        // The no-arg helpers MUST equal account 0 so the common path is
        // unambiguous.
        let secret = RootSecret::from_bytes([99u8; 32]);
        let mut default = secret.derive_ed25519_rng().unwrap();
        let mut acct0 = secret.derive_ed25519_rng_for_account(0).unwrap();
        assert_eq!(first_bytes(&mut default), first_bytes(&mut acct0));
    }

    #[test]
    fn info_grammar_is_canonical() {
        assert_eq!(dkg_info(CURVE_ED25519, 0), "frost-dkg/v1/ed25519/0");
        assert_eq!(dkg_info(CURVE_SECP256K1, 0), "frost-dkg/v1/secp256k1/0");
        assert_eq!(dkg_info(CURVE_SECP256K1, 42), "frost-dkg/v1/secp256k1/42");
    }

    #[test]
    fn info_bytes_are_locked() {
        // Regression lock (#38): the exact `info` bytes determine the derived
        // material. If this changes intentionally, bump DERIVATION_VERSION and
        // update these vectors in the same reviewed diff.
        assert_eq!(
            dkg_info(CURVE_ED25519, 0).as_bytes(),
            b"frost-dkg/v1/ed25519/0"
        );
        assert_eq!(
            dkg_info(CURVE_SECP256K1, 0).as_bytes(),
            b"frost-dkg/v1/secp256k1/0"
        );
    }

    #[test]
    fn derived_seed_is_byte_stable() {
        // Pin the actual derived seed for a fixed root so a pipeline change is
        // caught even if the `info` bytes are accidentally preserved.
        let secret = RootSecret::from_bytes([1u8; 32]);
        let mut rng = secret.derive_secp256k1_rng_for_account(0).unwrap();
        let got = first_bytes(&mut rng);
        // Self-pinned vector: regenerate (and review) only on a deliberate
        // DERIVATION_VERSION bump.
        let expected = {
            let hk = Hkdf::<Sha256>::new(None, &[1u8; 32]);
            let mut seed = [0u8; 32];
            hk.expand(b"frost-dkg/v1/secp256k1/0", &mut seed).unwrap();
            let mut r = ChaCha20Rng::from_seed(seed);
            let mut b = [0u8; 32];
            r.fill_bytes(&mut b);
            b
        };
        assert_eq!(got, expected);
    }
}
