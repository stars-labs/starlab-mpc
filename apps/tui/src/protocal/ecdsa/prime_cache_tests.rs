//! Fixture-only cache regressions: no prime generation or Paillier ceremonies.
use super::*;
pub(super) fn controlled_supply() -> PrimeSupply {
    let mut supply = PrimeSupply::background();
    Arc::get_mut(&mut supply.inner).unwrap().pause_generation = true;
    supply
}

fn make_ready(supply: &PrimeSupply) {
    supply.start();
    let epoch = supply.inner.state.lock().unwrap().generation;
    finish_generation(&supply.inner, epoch, fixture());
}

pub(super) fn fixture() -> Primes {
    let all: Vec<Primes> = serde_json::from_str(include_str!(
        "../../../../../packages/@starlab/core/src/ecdsa/testdata/primes.json"
    ))
    .expect("fixture primes");
    all.into_iter().next().expect("one set")
}

#[test]
fn encrypted_unused_primes_round_trip_with_private_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let supply = controlled_supply();
    make_ready(&supply);
    supply.configure_cache(dir.path(), "device", "wallet-password");
    let path = dir.path().join("device/unused-ecdsa-primes.cache");
    let bytes = std::fs::read(&path).unwrap();
    let plaintext = serde_json::to_vec(&fixture()).unwrap();
    assert!(!bytes.windows(plaintext.len()).any(|w| w == plaintext));
    assert!(serde_json::from_slice::<serde_json::Value>(&bytes).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let reopened = controlled_supply();
    reopened.configure_cache(dir.path(), "device", "wallet-password");
    assert!(reopened.is_ready());
    assert_eq!(
        reopened.take(Duration::ZERO).unwrap().primes_ref(),
        fixture().primes_ref()
    );
    assert!(!path.exists(), "consumption removes the lease before use");
    assert!(
        supply.take(Duration::ZERO).is_none(),
        "producer cannot reuse the consumer's primes"
    );
}

#[test]
fn concurrent_consumers_can_claim_a_published_set_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let producer = controlled_supply();
    make_ready(&producer);
    producer.configure_cache(dir.path(), "device", "wallet-password");
    let first = controlled_supply();
    let second = controlled_supply();
    first.configure_cache(dir.path(), "device", "wallet-password");
    second.configure_cache(dir.path(), "device", "wallet-password");
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let threads: Vec<_> = [first, second]
        .into_iter()
        .map(|supply| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                supply.take(Duration::ZERO).is_some()
            })
        })
        .collect();
    barrier.wait();
    assert_eq!(
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .filter(|claimed| *claimed)
            .count(),
        1
    );
    assert!(producer.take(Duration::ZERO).is_none());
}

#[test]
fn lost_or_replaced_lease_is_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let supply = controlled_supply();
    make_ready(&supply);
    supply.configure_cache(dir.path(), "device", "wallet-password");
    let path = dir.path().join("device/unused-ecdsa-primes.cache");
    std::fs::remove_file(&path).unwrap();
    let replacement = Cache::new(dir.path(), "device", "wallet-password").unwrap();
    assert!(replacement.publish(&fixture()).unwrap().is_some());
    assert!(
        supply.take(Duration::ZERO).is_none(),
        "a new publication never validates an old token"
    );
}

#[test]
fn wrong_password_and_corruption_preserve_cache_and_allow_fresh_private_primes() {
    let dir = tempfile::tempdir().unwrap();
    let producer = controlled_supply();
    make_ready(&producer);
    producer.configure_cache(dir.path(), "device", "wallet-password");
    let path = dir.path().join("device/unused-ecdsa-primes.cache");
    let original = std::fs::read(&path).unwrap();
    let wrong = controlled_supply();
    wrong.configure_cache(dir.path(), "device", "wrong-password");
    assert!(!wrong.is_ready());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    make_ready(&wrong);
    assert!(wrong.take(Duration::ZERO).is_some());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "fresh publication cannot overwrite another lease"
    );
    std::fs::write(&path, b"corrupted ciphertext").unwrap();
    let corrupt = controlled_supply();
    corrupt.configure_cache(dir.path(), "device", "wallet-password");
    assert!(!corrupt.is_ready());
    make_ready(&corrupt);
    assert!(corrupt.take(Duration::ZERO).is_some());
}

#[test]
fn cache_write_failure_keeps_never_published_fresh_primes_usable() {
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("not-a-directory");
    std::fs::write(&blocked, b"block").unwrap();
    let supply = controlled_supply();
    supply.configure_cache(&blocked, "device", "wallet-password");
    make_ready(&supply);
    assert!(supply.take(Duration::ZERO).is_some());
}

#[test]
fn restored_cache_invalidates_old_generator_and_is_never_republished() {
    let dir = tempfile::tempdir().unwrap();
    let producer = controlled_supply();
    make_ready(&producer);
    producer.configure_cache(dir.path(), "device", "wallet-password");
    let path = dir.path().join("device/unused-ecdsa-primes.cache");
    let ciphertext = std::fs::read(&path).unwrap();
    let restored = controlled_supply();
    restored.start();
    let stale_epoch = restored.inner.state.lock().unwrap().generation;
    restored.configure_cache(dir.path(), "device", "wallet-password");
    finish_generation(&restored.inner, stale_epoch, fixture());
    assert_eq!(std::fs::read(&path).unwrap(), ciphertext);
    assert!(restored.take(Duration::ZERO).is_some());
    assert!(!path.exists());
    assert!(producer.take(Duration::ZERO).is_none());
}

#[test]
fn insecure_fixed_supply_never_reads_or_writes_cache() {
    let dir = tempfile::tempdir().unwrap();
    let fixed = PrimeSupply::insecure_test_fixed(fixture());
    fixed.configure_cache(dir.path(), "device", "wallet-password");
    assert!(fixed.take(Duration::ZERO).is_some());
    assert!(!dir.path().join("device").exists());
}

#[test]
fn independent_processes_claim_a_published_set_only_once() {
    const CHILD: &str = "STARLAB_PRIME_CACHE_TEST_CHILD";
    const TEST: &str = "protocal::ecdsa::primes::cache_tests::independent_processes_claim_a_published_set_only_once";
    if let Some(directory) = std::env::var_os(CHILD) {
        let directory = std::path::PathBuf::from(directory);
        let id = std::env::var("STARLAB_PRIME_CACHE_TEST_ID").unwrap();
        let supply = controlled_supply();
        supply.configure_cache(&directory, "device", "wallet-password");
        assert!(supply.is_ready());
        std::fs::write(directory.join(format!("ready-{id}")), b"ready").unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !directory.join("claim-now").exists() {
            assert!(
                Instant::now() < deadline,
                "parent never released claim barrier"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        println!(
            "PRIME_CACHE_CLAIMED={}",
            supply.take(Duration::ZERO).is_some()
        );
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let producer = controlled_supply();
    make_ready(&producer);
    producer.configure_cache(directory.path(), "device", "wallet-password");
    let children: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|id| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, directory.path())
                .env("STARLAB_PRIME_CACHE_TEST_ID", id)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !["first", "second"]
        .iter()
        .all(|id| directory.path().join(format!("ready-{id}")).exists())
    {
        assert!(Instant::now() < deadline, "consumers did not load cache");
        std::thread::sleep(Duration::from_millis(2));
    }
    std::fs::write(directory.path().join("claim-now"), b"go").unwrap();
    let outputs: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();
    for output in &outputs {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        outputs
            .iter()
            .filter(|output| String::from_utf8_lossy(&output.stdout)
                .contains("PRIME_CACHE_CLAIMED=true"))
            .count(),
        1
    );
    assert!(
        producer.take(Duration::ZERO).is_none(),
        "producer also lost its lease"
    );
}

#[test]
fn directory_sync_failure_discards_leased_primes_and_disables_cache_until_reconfigured() {
    let dir = tempfile::tempdir().unwrap();
    let supply = controlled_supply();
    make_ready(&supply);
    supply.configure_cache(dir.path(), "device", "wallet-password");
    supply
        .inner
        .state
        .lock()
        .unwrap()
        .cache
        .as_mut()
        .unwrap()
        .fail_directory_sync = true;
    assert!(
        supply.take(Duration::ZERO).is_none(),
        "undurable lease removal cannot authorize use"
    );
    assert!(supply.inner.state.lock().unwrap().cache.is_none());
    let path = dir.path().join("device/unused-ecdsa-primes.cache");
    assert!(!path.exists());
    make_ready(&supply);
    assert!(
        supply.take(Duration::ZERO).is_some(),
        "next fresh private set must remain usable"
    );
    assert!(
        !path.exists(),
        "failed cache is not automatically republished"
    );
    supply.configure_cache(dir.path(), "device", "wallet-password");
    make_ready(&supply);
    assert!(
        path.exists(),
        "a later explicit configuration can retry caching"
    );
    assert!(supply.take(Duration::ZERO).is_some());
}

#[test]
fn ui_status_does_not_wait_for_cache_worker_lock() {
    let supply = controlled_supply();
    let _worker = supply.inner.state.lock().unwrap();
    assert_eq!(supply.status(), PrimeStatus::Generating { elapsed_secs: 0 });
}
