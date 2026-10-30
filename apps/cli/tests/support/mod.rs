//! Shared test support: the INSECURE fixture ECDSA primes.
//!
//! Safe-prime generation takes ~1.5 min per node, so the e2e tests inject
//! the three published fixture sets from `starlab-core`'s testdata (test-only
//! secrets — a wallet built on them protects nothing). Node `i` uses set
//! `i % 3`.
#![allow(dead_code)]

use starlab_core::ecdsa::Primes;

const PRIMES_JSON: &str =
    include_str!("../../../../packages/@starlab/core/src/ecdsa/testdata/primes.json");

/// The fixture prime sets (3 of them).
pub fn test_primes() -> Vec<Primes> {
    serde_json::from_str(PRIMES_JSON).expect("fixture primes parse")
}

/// Write one `--insecure-test-primes` file per set into `dir`; returns the
/// paths (node `i` takes `paths[i % paths.len()]`).
pub fn write_test_primes_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    test_primes()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let path = dir.join(format!("primes-{i}.json"));
            std::fs::write(&path, serde_json::to_vec(p).expect("serialize primes"))
                .expect("write primes file");
            path
        })
        .collect()
}
