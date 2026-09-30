//! Paillier safe primes for the ECDSA aux-info ceremony.
//!
//! Generating them takes ~1.5 min natively, so a node starts on them in the
//! background when its runner starts ([`PrimeSupply::start`]) — normally
//! long before a DKG needs them. A DKG takes the ready set (waiting if the
//! generator is still running) and the generator immediately starts on the
//! next set, so a second wallet in the same process doesn't wait again.
//! Primes only ever live in memory; nothing is written to disk.

use starlab_core::ecdsa::Primes;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, warn};

enum Slot {
    Idle,
    Generating,
    Ready(Box<Primes>),
}

struct Inner {
    /// Test-only fixed primes (see [`PrimeSupply::insecure_test_fixed`]).
    fixed: Option<Box<Primes>>,
    slot: Mutex<Slot>,
    ready: Condvar,
}

/// A node's source of safe primes. Cheap to clone (shared handle).
#[derive(Clone)]
pub struct PrimeSupply {
    inner: Arc<Inner>,
}

impl Default for PrimeSupply {
    fn default() -> Self {
        Self::background()
    }
}

impl std::fmt::Debug for PrimeSupply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimeSupply")
            .field("insecure_test_fixed", &self.inner.fixed.is_some())
            .field("ready", &self.is_ready())
            .finish()
    }
}

impl PrimeSupply {
    /// Real primes, generated on a background thread once [`Self::start`]
    /// is called (or on first use).
    pub fn background() -> Self {
        Self {
            inner: Arc::new(Inner {
                fixed: None,
                slot: Mutex::new(Slot::Idle),
                ready: Condvar::new(),
            }),
        }
    }

    /// **INSECURE — tests only.** Hand out the same precomputed primes to
    /// every ceremony instead of generating fresh ones, so an end-to-end
    /// test doesn't spend minutes on safe-prime generation. A Paillier key
    /// from published primes protects nothing: never use this for a wallet
    /// that holds value. Only reachable through explicitly named test
    /// entry points (the hidden `--insecure-test-primes` CLI flag, the
    /// simulator's `insecure_test_primes` option).
    pub fn insecure_test_fixed(primes: Primes) -> Self {
        warn!("⚠️  ECDSA: using INSECURE fixed test primes — never use this wallet for real funds");
        Self {
            inner: Arc::new(Inner {
                fixed: Some(Box::new(primes)),
                slot: Mutex::new(Slot::Idle),
                ready: Condvar::new(),
            }),
        }
    }

    /// Start generating in the background (no-op when a set is ready, being
    /// generated, or the supply is fixed).
    pub fn start(&self) {
        if self.inner.fixed.is_some() {
            return;
        }
        let mut slot = self.inner.slot.lock().expect("prime slot lock");
        if !matches!(*slot, Slot::Idle) {
            return;
        }
        *slot = Slot::Generating;
        drop(slot);
        let inner = self.inner.clone();
        let spawned = std::thread::Builder::new()
            .name("ecdsa-primes".into())
            .spawn(move || {
                let started = Instant::now();
                info!("ECDSA: generating Paillier safe primes in the background");
                let mut rng = starlab_core::rng::os_rng();
                let primes = starlab_core::ecdsa::pregenerate_primes(&mut rng);
                info!(
                    "ECDSA: safe primes ready after {:.0}s",
                    started.elapsed().as_secs_f64()
                );
                *inner.slot.lock().expect("prime slot lock") = Slot::Ready(Box::new(primes));
                inner.ready.notify_all();
            });
        if let Err(e) = spawned {
            warn!("ECDSA: could not spawn the prime generator: {e}");
            *self.inner.slot.lock().expect("prime slot lock") = Slot::Idle;
        }
    }

    /// Whether a DKG could take primes right now without waiting.
    pub fn is_ready(&self) -> bool {
        self.inner.fixed.is_some()
            || matches!(
                *self.inner.slot.lock().expect("prime slot lock"),
                Slot::Ready(_)
            )
    }

    /// Take one set, blocking up to `timeout` for the generator. Starts the
    /// generator if it isn't running, and starts on the next set after
    /// taking this one. Blocking: call from a worker thread, never async.
    pub fn take(&self, timeout: Duration) -> Option<Primes> {
        if let Some(fixed) = &self.inner.fixed {
            return Some((**fixed).clone());
        }
        self.start();
        let deadline = Instant::now() + timeout;
        let mut slot = self.inner.slot.lock().expect("prime slot lock");
        loop {
            if matches!(*slot, Slot::Ready(_)) {
                let Slot::Ready(primes) = std::mem::replace(&mut *slot, Slot::Idle) else {
                    unreachable!()
                };
                drop(slot);
                self.start(); // the next wallet's set
                return Some(*primes);
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            slot = self
                .inner
                .ready
                .wait_timeout(slot, left)
                .expect("prime slot lock")
                .0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Primes {
        let all: Vec<Primes> = serde_json::from_str(include_str!(
            "../../../../../packages/@starlab/core/src/ecdsa/testdata/primes.json"
        ))
        .expect("fixture primes");
        all.into_iter().next().expect("one set")
    }

    #[test]
    fn fixed_supply_hands_out_the_same_primes_repeatedly() {
        let supply = PrimeSupply::insecure_test_fixed(fixture());
        assert!(supply.is_ready());
        let a = supply.take(Duration::ZERO).unwrap();
        let b = supply.take(Duration::ZERO).unwrap();
        assert_eq!(a.primes_ref(), b.primes_ref());
    }

    #[test]
    fn background_supply_is_lazy_and_times_out_while_generating() {
        let supply = PrimeSupply::background();
        assert!(!supply.is_ready(), "nothing is generated before start()");
        // take() starts the generator; it can't finish within 1 ms.
        assert!(supply.take(Duration::from_millis(1)).is_none());
    }
}
