//! Paillier safe primes for the ECDSA aux-info ceremony.
//!
//! Generating them takes ~1.5 min natively, so a node starts on them in the
//! background when its runner starts ([`PrimeSupply::start`]) — normally
//! long before a DKG needs them. A DKG takes the ready set (waiting if the
//! generator is still running) and the generator immediately starts on the
//! next set, so a second wallet in the same process doesn't wait again.
//! Once a wallet password is available, unused sets are encrypted on disk.
//! A published set is usable only after its unique disk lease is claimed.

use super::prime_cache::{Cache, Lease};
use starlab_core::ecdsa::Primes;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, warn};

enum Slot {
    Idle,
    /// Generating since the instant.
    Generating(Instant),
    Ready {
        primes: Box<Primes>,
        lease: Option<Lease>,
    },
}

/// What a UI shows about the primes (the "ECDSA setup" of the next wallet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrimeStatus {
    /// Not started yet (it starts with the runner, or with the first DKG).
    #[default]
    Idle,
    /// Being generated for `elapsed_secs` so far.
    Generating { elapsed_secs: u64 },
    /// A set is ready: the next DKG won't wait.
    Ready,
    /// Test-only fixed primes ([`PrimeSupply::insecure_test_fixed`]).
    InsecureTestFixed,
}

impl PrimeStatus {
    /// One English status line, e.g. "ECDSA setup: preparing safe primes
    /// (1m 12s so far, usually 1-5 min)".
    pub fn describe(&self) -> String {
        match self {
            Self::Idle => "ECDSA setup: not started".to_string(),
            Self::Generating { .. } => format!(
                "ECDSA setup: preparing safe primes ({})",
                self.progress().unwrap_or_default()
            ),
            Self::Ready => "ECDSA setup: ready".to_string(),
            Self::InsecureTestFixed => "ECDSA setup: ready (INSECURE test primes)".to_string(),
        }
    }

    /// While generating: "1m 12s so far, usually 1-5 min".
    pub fn progress(&self) -> Option<String> {
        match self {
            Self::Generating { elapsed_secs } => Some(format!(
                "{} so far, usually 1-5 min",
                format_elapsed(*elapsed_secs)
            )),
            _ => None,
        }
    }

    /// Whether a DKG would start its ECDSA part without waiting.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready | Self::InsecureTestFixed)
    }
}

/// "42s" / "1m 12s".
pub fn format_elapsed(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

struct State {
    slot: Slot,
    generation: u64,
    cache: Option<Cache>,
}

struct Inner {
    /// Test-only fixed primes (see [`PrimeSupply::insecure_test_fixed`]).
    fixed: Option<Box<Primes>>,
    state: Mutex<State>,
    #[cfg(test)]
    pause_generation: bool,
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
                state: Mutex::new(State {
                    slot: Slot::Idle,
                    generation: 0,
                    cache: None,
                }),
                #[cfg(test)]
                pause_generation: false,
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
                state: Mutex::new(State {
                    slot: Slot::Idle,
                    generation: 0,
                    cache: None,
                }),
                #[cfg(test)]
                pause_generation: false,
                ready: Condvar::new(),
            }),
        }
    }

    /// Configure encrypted unused-prime persistence after a wallet password is
    /// available. Blocking KDF and filesystem work: call via spawn_blocking.
    pub fn configure_cache(&self, base: &Path, device: &str, password: &str) {
        if self.inner.fixed.is_some() {
            return;
        }
        let cache = match Cache::new(base, device, password) {
            Ok(cache) => cache,
            Err(e) => {
                warn!("ECDSA prime cache disabled: {e}");
                return;
            }
        };
        let mut state = self.inner.state.lock().expect("prime state lock");
        if state.cache.as_ref() == Some(&cache) {
            return;
        }
        let loaded = cache.load();
        let restored = loaded.is_some();
        // Existing leased memory belongs to the previous configuration. Never
        // turn it into a private set or republish it under another password.
        if loaded.is_some() || matches!(state.slot, Slot::Ready { lease: Some(_), .. }) {
            state.generation += 1;
            state.slot = Slot::Idle;
        }
        state.cache = Some(cache);
        if let Some((primes, lease)) = loaded {
            state.slot = Slot::Ready {
                primes: Box::new(primes),
                lease: Some(lease),
            };
            self.inner.ready.notify_all();
        }
        let configured = state.cache.as_ref().expect("configured").clone();
        if !restored && let Slot::Ready { primes, lease } = &mut state.slot {
            // These primes were generated before configuration and never
            // published, so they can safely become a new leased set.
            match configured.publish(primes) {
                Ok(published) => *lease = published,
                Err(e) => warn!("ECDSA prime cache write failed; keeping fresh private set: {e}"),
            }
        }
        drop(state);
        self.start();
    }

    /// Start the launch-time pre-generator, preserving an already ready set.
    pub fn start(&self) {
        if self.inner.fixed.is_some() {
            return;
        }
        let mut state = self.inner.state.lock().expect("prime state lock");
        if !matches!(state.slot, Slot::Idle) {
            return;
        }
        state.generation += 1;
        let epoch = state.generation;
        state.slot = Slot::Generating(Instant::now());
        drop(state);
        #[cfg(test)]
        if self.inner.pause_generation {
            return;
        }
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
                finish_generation(&inner, epoch, primes);
            });
        if let Err(e) = spawned {
            warn!("ECDSA: could not spawn the prime generator: {e}");
            let mut state = self.inner.state.lock().expect("prime state lock");
            if state.generation == epoch {
                state.slot = Slot::Idle;
            }
        }
    }

    /// Whether a DKG could take primes right now without waiting.
    pub fn is_ready(&self) -> bool {
        self.status().is_ready()
    }

    /// The generator's state, for the UI.
    pub fn status(&self) -> PrimeStatus {
        if self.inner.fixed.is_some() {
            return PrimeStatus::InsecureTestFixed;
        }
        // UI polling must not wait behind a blocking cache KDF/fsync.
        let Ok(state) = self.inner.state.try_lock() else {
            return PrimeStatus::Generating { elapsed_secs: 0 };
        };
        match &state.slot {
            Slot::Idle => PrimeStatus::Idle,
            Slot::Generating(since) => PrimeStatus::Generating {
                elapsed_secs: since.elapsed().as_secs(),
            },
            Slot::Ready { .. } => PrimeStatus::Ready,
        }
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
        let mut state = self.inner.state.lock().expect("prime state lock");
        loop {
            if matches!(state.slot, Slot::Ready { .. }) {
                let Slot::Ready { primes, lease } = std::mem::replace(&mut state.slot, Slot::Idle)
                else {
                    unreachable!()
                };
                let usable = lease.as_ref().is_none_or(|lease| {
                    state.cache.as_ref().is_some_and(|cache| cache.claim(lease))
                });
                if !usable {
                    // A failed claim can mean unsupported directory sync or
                    // persistent contention. Do not endlessly publish and lose
                    // the replacement: keep subsequent fresh sets private until
                    // a later password handoff explicitly configures again.
                    state.cache = None;
                }
                drop(state);
                self.start();
                if usable {
                    return Some(*primes);
                }
                warn!(
                    "ECDSA prime-cache lease was lost; discarding primes and generating a fresh set"
                );
                state = self.inner.state.lock().expect("prime state lock");
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            state = self
                .inner
                .ready
                .wait_timeout(state, left)
                .expect("prime state lock")
                .0;
        }
    }
}

fn finish_generation(inner: &Inner, epoch: u64, primes: Primes) {
    let mut state = inner.state.lock().expect("prime state lock");
    if state.generation != epoch || !matches!(state.slot, Slot::Generating(_)) {
        return;
    }
    let lease = state
        .cache
        .as_ref()
        .and_then(|cache| match cache.publish(&primes) {
            Ok(lease) => lease,
            Err(e) => {
                warn!("ECDSA prime cache write failed; keeping fresh private set: {e}");
                None
            }
        });
    state.slot = Slot::Ready {
        primes: Box::new(primes),
        lease,
    };
    inner.ready.notify_all();
}

#[cfg(test)]
#[path = "prime_cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
mod tests {
    use super::*;

    use super::cache_tests::{controlled_supply, fixture};

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
        let supply = controlled_supply();
        assert!(!supply.is_ready(), "nothing is generated before start()");
        // take() starts the generator; it can't finish within 1 ms.
        assert!(supply.take(Duration::from_millis(1)).is_none());
    }

    #[test]
    fn status_reports_idle_then_generating() {
        let supply = controlled_supply();
        assert_eq!(supply.status(), PrimeStatus::Idle);
        supply.start();
        assert!(matches!(
            supply.status(),
            PrimeStatus::Generating { elapsed_secs: 0 }
        ));
        assert!(!supply.status().is_ready());
    }

    #[test]
    fn fixed_supply_status_names_the_insecure_primes() {
        let status = PrimeSupply::insecure_test_fixed(fixture()).status();
        assert!(status.is_ready());
        assert_eq!(
            status.describe(),
            "ECDSA setup: ready (INSECURE test primes)"
        );
    }

    #[test]
    fn status_lines_are_plain_english() {
        assert_eq!(PrimeStatus::Ready.describe(), "ECDSA setup: ready");
        assert_eq!(
            PrimeStatus::Generating { elapsed_secs: 72 }.describe(),
            "ECDSA setup: preparing safe primes (1m 12s so far, usually 1-5 min)"
        );
        assert_eq!(format_elapsed(9), "9s");
    }
}
