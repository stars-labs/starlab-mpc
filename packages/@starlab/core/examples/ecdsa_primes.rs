//! Time Paillier safe-prime pre-generation for the threshold-ECDSA aux info,
//! and optionally write the sets as JSON (how the committed test fixture
//! `src/ecdsa/testdata/primes.json` was produced).
//!
//! ```text
//! cargo run --release -p starlab-core --example ecdsa_primes -- [count] [out.json]
//! ```

use starlab_core::ecdsa::{Primes, pregenerate_primes};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let count: usize = args
        .next()
        .map(|s| s.parse().expect("count must be a number"))
        .unwrap_or(1);
    let out = args.next();

    let mut rng = starlab_core::rng::os_rng();
    let mut sets: Vec<Primes> = Vec::with_capacity(count);
    let total = Instant::now();
    for i in 0..count {
        let started = Instant::now();
        sets.push(pregenerate_primes(&mut rng));
        println!("set {i}: 4 safe primes in {:.2?}", started.elapsed());
    }
    println!(
        "{count} set(s) in {:.2?} (avg {:.2?})",
        total.elapsed(),
        total.elapsed() / count.max(1) as u32
    );

    if let Some(path) = out {
        let json = serde_json::to_string(&sets).expect("primes serialize");
        std::fs::write(&path, json).expect("write primes");
        println!("wrote {path}");
    }
}
