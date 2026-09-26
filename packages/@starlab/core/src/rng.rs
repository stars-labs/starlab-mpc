//! RNG bridge at the FROST boundary.
//!
//! frost-core 3.0 (latest) still takes `rand_core 0.6` `RngCore + CryptoRng`,
//! while the rest of the tree is on `rand_core 0.10` / `rand_chacha 0.10` /
//! `getrandom 0.4`. The two trait families are unrelated types, so every RNG
//! handed to a frost function goes through [`FrostRng`], which forwards each
//! call 1:1 (`next_u32` → `next_u32`, …) — the byte stream is unchanged, so
//! seeded ChaCha20 derivations stay bit-for-bit deterministic.
//!
//! The 0.6 traits come from frost's own re-export (`frost_ed25519::rand_core`),
//! so this crate never depends on the old `rand_core` directly.

use frost_ed25519::rand_core as frost_rand_core;
use rand_core::UnwrapErr;

/// The `rand_core 0.6` traits frost's APIs are bounded on.
pub use frost_rand_core::{CryptoRng, RngCore};

/// Adapts a `rand_core 0.10` RNG to the `rand_core 0.6` traits frost takes.
#[derive(Clone, Debug)]
pub struct FrostRng<R>(pub R);

impl<R: rand_core::Rng> frost_rand_core::RngCore for FrostRng<R> {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), frost_rand_core::Error> {
        self.0.fill_bytes(dest);
        Ok(())
    }
}

impl<R: rand_core::CryptoRng> frost_rand_core::CryptoRng for FrostRng<R> {}

impl<R: rand_core::SeedableRng> rand_core::SeedableRng for FrostRng<R> {
    type Seed = R::Seed;

    fn from_seed(seed: Self::Seed) -> Self {
        FrostRng(R::from_seed(seed))
    }
}

/// Seeded ChaCha20 (`rand_chacha 0.10`), frost-compatible. Every deterministic
/// DKG / reshare RNG in the engine is this type.
pub type ChaCha20Rng = FrostRng<rand_chacha::ChaCha20Rng>;

/// The OS CSPRNG (`getrandom::SysRng`) in frost-compatible form. Panics on an
/// OS RNG failure, exactly like `rand_core 0.6`'s `OsRng` did.
pub type OsRng = FrostRng<UnwrapErr<getrandom::SysRng>>;

/// Construct an [`OsRng`].
pub fn os_rng() -> OsRng {
    FrostRng(UnwrapErr(getrandom::SysRng))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::{Rng as _, SeedableRng as _};

    #[test]
    fn bridge_forwards_the_exact_chacha_stream() {
        let mut direct = rand_chacha::ChaCha20Rng::from_seed([9u8; 32]);
        let mut bridged = FrostRng(rand_chacha::ChaCha20Rng::from_seed([9u8; 32]));
        assert_eq!(direct.next_u32(), bridged.next_u32());
        assert_eq!(direct.next_u64(), bridged.next_u64());
        let (mut a, mut b) = ([0u8; 45], [0u8; 45]);
        direct.fill_bytes(&mut a);
        bridged.fill_bytes(&mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn os_rng_produces_distinct_output() {
        let mut rng = os_rng();
        assert_ne!(rng.next_u64(), rng.next_u64());
    }
}
