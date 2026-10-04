//! End-to-end DKG test (issue #16), built on the shared `simulate`
//! orchestrator (#21): real FROST DKG across N `HeadlessRunner`s in one
//! process, against an embedded signal server, WebRTC over loopback.
//!
//! `#[ignore]` by default (real UDP/ICE on loopback, ~seconds). Run with:
//!   cargo test -p starlab-cli --test e2e_dkg -- --ignored --nocapture

mod support;

use starlab_cli::simulate::{
    SimulateOpts, run_signing_simulation, run_signing_simulation_all_signers_online,
    run_signing_timeout_then_retry_simulation, run_simulation,
};

fn init_logs() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("starlab_client=warn,webrtc=warn")
            }),
        )
        .with_test_writer()
        .try_init();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
async fn dkg_2_of_2_completes_and_persists() {
    init_logs();
    let result = run_simulation(SimulateOpts {
        nodes: 2,
        threshold: 2,
        curve: "secp256k1".into(),
        signal_url: None,
        timeout_secs: 90,
        insecure_test_primes: support::test_primes(),
    })
    .await
    .expect("simulation ran");

    assert!(
        result.agreed,
        "nodes disagreed on group key: {:?}",
        result.outcomes
    );
    assert_eq!(result.outcomes.len(), 2);
    assert!(!result.group_public_key.is_empty());
    eprintln!(
        "✅ 2-of-2 DKG ok in {}ms, group={}",
        result.elapsed_ms, result.group_public_key
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
async fn dkg_2_of_3_completes() {
    init_logs();
    let result = run_simulation(SimulateOpts {
        nodes: 3,
        threshold: 2,
        curve: "secp256k1".into(),
        signal_url: None,
        timeout_secs: 120,
        insecure_test_primes: support::test_primes(),
    })
    .await
    .expect("simulation ran");

    assert!(
        result.agreed,
        "nodes disagreed on group key: {:?}",
        result.outcomes
    );
    assert_eq!(result.outcomes.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real WebRTC/DKG+signing over loopback; run with --ignored"]
async fn dkg_then_sign_2_of_2_verifies() {
    init_logs();
    let result = run_signing_simulation(
        SimulateOpts {
            nodes: 2,
            threshold: 2,
            curve: "secp256k1".into(),
            signal_url: None,
            timeout_secs: 120,
            insecure_test_primes: support::test_primes(),
        },
        "hello from the e2e signing test",
    )
    .await
    .expect("signing simulation ran");

    assert!(!result.signature.is_empty(), "empty signature");
    assert!(
        result.verified,
        "signature did not verify against the group key: {result:?}"
    );
    eprintln!(
        "✅ 2-of-2 sign ok in {}ms, verified={}, sig={}…",
        result.elapsed_ms,
        result.verified,
        &result.signature[..16.min(result.signature.len())]
    );
}

/// Regression test for the signer-set bug: with MORE than `threshold`
/// signers online, the OLD rule ("run Round 2 the moment I personally see
/// threshold-many commitments") let different nodes pick different signer
/// sets — `aggregate` then failed with `UnknownIdentifier`. Reproduces the
/// browser-extension interop scenario ("ext + 2 CLI", 2-of-3, all 3 online)
/// entirely inside the CLI simulator: a 2-of-3 wallet where ALL 3 nodes join
/// the signing ceremony. Every node — including the one the proposer's fixed
/// set leaves out — must report the identical aggregated signature.
///
/// Before the SIGN_SET fix this test failed (nondeterministically — timeout
/// waiting for `SignDone` on at least one node, or an `UnknownIdentifier`
/// surfaced as a `SigningFailed` that `drive_signing` doesn't currently
/// distinguish from a hang) because no prior e2e test drove a 2-of-3 signing
/// with all 3 nodes participating: `dkg_then_sign_2_of_2_verifies` is 2-of-2
/// (threshold == nodes, so "more than threshold online" can't happen), and
/// `bitcoin_account_signs_a_sighash_valid_for_its_p2tr_address` /
/// `reshare_then_sign_2_of_3_*` are 2-of-3 but only join `threshold`-many
/// (2) co-signers — the third node never signs. `conformance_matrix` and
/// `l3_serve_process` don't exercise 3-online signing either.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC/DKG+signing over loopback; run with --ignored"]
async fn frost_sign_2_of_3_all_three_online_agree_on_one_signature() {
    init_logs();
    let result = run_signing_simulation_all_signers_online(
        SimulateOpts {
            nodes: 3,
            threshold: 2,
            curve: "secp256k1".into(),
            signal_url: None,
            timeout_secs: 120,
            insecure_test_primes: support::test_primes(),
        },
        // Bitcoin account: the FROST BIP-340 signer set (#121).
        &"5b".repeat(32),
        true,
    )
    .await
    .expect("all-signers-online signing simulation ran");

    assert_eq!(
        result.signatures.len(),
        3,
        "all 3 nodes must report a signature"
    );
    assert!(
        result.all_agreed,
        "nodes disagreed on the aggregated signature: {:?}",
        result.signatures
    );
    assert!(
        result.verified,
        "signature did not verify against the group key: {result:?}"
    );
    eprintln!(
        "✅ 2-of-3 sign with all 3 online agreed in {}ms, verified={}, sig={}…",
        result.elapsed_ms,
        result.verified,
        &result.signatures[0][..16.min(result.signatures[0].len())]
    );
}

/// Regression test for the signing-timeout feature: a picked signer that
/// never responds (down, or never approves the request) must not wedge the
/// ceremony forever — node 0 announces, nobody joins, so it never gathers
/// `threshold`-many commitments and must fail cleanly on its own once the
/// shortened per-node timeout elapses. A retry —
/// a fresh ceremony with a real co-signer — must then succeed and verify,
/// proving the timeout wiped the ceremony state cleanly enough to start
/// over (automatic re-picking of a different signer INSIDE the same
/// ceremony is unsafe: FROST nonce reuse would leak the key).
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC/DKG+signing over loopback; run with --ignored"]
async fn signing_times_out_when_nobody_joins_then_retry_succeeds() {
    init_logs();

    let result = run_signing_timeout_then_retry_simulation(
        SimulateOpts {
            nodes: 3,
            threshold: 2,
            // This regression exercises FROST nonce/state cleanup.
            // The secp256k1 simulator also provisions ECDSA and selects
            // Ethereum; its Paillier worker cannot use this 3s FROST budget.
            curve: "ed25519".into(),
            signal_url: None,
            timeout_secs: 60,
            insecure_test_primes: support::test_primes(),
        },
        "picked signer never responds; ceremony must time out, then retry",
    )
    .await
    .expect("signing-timeout-then-retry simulation ran");

    assert!(
        result.timeout_reason.contains("timed out"),
        "first ceremony must fail via the timeout: {}",
        result.timeout_reason
    );
    assert!(
        result.timeout_reason.contains("threshold"),
        "with nobody joined, the reason must cite the commitment shortfall: {}",
        result.timeout_reason
    );
    assert!(!result.retried_signature.is_empty(), "empty signature");
    assert!(
        result.verified,
        "retried signature did not verify against the group key: {result:?}"
    );
    eprintln!(
        "✅ stuck ceremony timed out ({}), retry ok in {}ms, verified={}",
        result.timeout_reason, result.elapsed_ms, result.verified
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC signing over loopback; run with --ignored"]
async fn bitcoin_account_signs_a_sighash_valid_for_its_p2tr_address() {
    init_logs();
    let sighash = "5a".repeat(32); // a BIP-341 sighash is 32 bytes
    let result = starlab_cli::simulate::run_bitcoin_signing_simulation(
        SimulateOpts {
            nodes: 3,
            threshold: 2,
            curve: "secp256k1".into(),
            signal_url: None,
            timeout_secs: 120,
            insecure_test_primes: support::test_primes(),
        },
        &sighash,
    )
    .await
    .expect("bitcoin signing simulation ran");

    assert!(result.address.starts_with("bc1p"), "{result:?}");
    assert_eq!(
        result.sighash.trim_start_matches("0x"),
        sighash,
        "signed the raw sighash"
    );
    assert!(
        result.verified,
        "BIP-340 signature must verify for the P2TR address: {result:?}"
    );
    eprintln!(
        "✅ 2-of-3 Taproot key-path signature valid for {}",
        result.address
    );
}

/// DKG, then a same-set reshare on `runners`; the group key must survive, the
/// refreshed shares must sign, and node 0's share must be persisted.
async fn reshare_then_sign_2_of_3(runners: starlab_cli::simulate::ReshareRunners) {
    init_logs();
    let r = starlab_cli::simulate::run_reshare_e2e(
        starlab_cli::simulate::SimulateOpts {
            nodes: 3,
            threshold: 2,
            curve: "secp256k1".into(),
            signal_url: None,
            timeout_secs: 120,
            insecure_test_primes: support::test_primes(),
        },
        "reshared then signed",
        runners,
    )
    .await
    .expect("reshare e2e ran");

    assert!(r.key_preserved, "group key changed across reshare: {r:?}");
    assert_eq!(r.dkg_group_public_key, r.reshare_group_public_key);
    assert!(
        r.signed_after_reshare,
        "refreshed shares failed to sign: {r:?}"
    );
    assert!(
        r.share_persisted,
        "refreshed share not persisted with same group key: {r:?}"
    );
    eprintln!(
        "✅ reshare e2e ({runners:?}) ok in {}ms, group preserved = {}",
        r.elapsed_ms, r.dkg_group_public_key
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC reshare over loopback; run with --ignored"]
async fn reshare_then_sign_2_of_3_preserves_group_key() {
    // #45 4b / #56: fresh processes load their shares from disk and reshare.
    reshare_then_sign_2_of_3(starlab_cli::simulate::ReshareRunners::Fresh).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC reshare over loopback; run with --ignored"]
async fn reshare_on_the_nodes_that_ran_the_dkg() {
    // A long-lived `serve` node reshares after its own DKG: the finished DKG
    // must not block the reshare's round 1 as "FROST already running".
    reshare_then_sign_2_of_3(starlab_cli::simulate::ReshareRunners::Reused).await;
}

fn ecdsa_opts(threshold: u16) -> SimulateOpts {
    SimulateOpts {
        nodes: 3,
        threshold,
        curve: "secp256k1".into(),
        signal_url: None,
        timeout_secs: 180,
        insecure_test_primes: support::test_primes(),
    }
}

/// ECDSA counterpart of the all-online test: 2-of-3 Ethereum signing with
/// all 3 joined. The proposer fixes the set (`ECDSA_SIGN_SET`), the two
/// signers run cggmp24, and the node left out gets the signature from the
/// proposer (`ECDSA_SIGN_DONE`) — all 3 report the same one.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC/DKG+ECDSA signing over loopback; run with --ignored"]
async fn ecdsa_sign_2_of_3_all_three_online_agree_on_one_signature() {
    init_logs();
    let result =
        run_signing_simulation_all_signers_online(ecdsa_opts(2), "ecdsa, all 3 online", false)
            .await
            .expect("all-online ECDSA signing ran");
    assert_eq!(result.signatures.len(), 3);
    assert!(result.all_agreed, "{:?}", result.signatures);
    assert!(result.verified, "must ecrecover to the account: {result:?}");
    eprintln!(
        "✅ ECDSA 2-of-3 all online agreed in {}ms",
        result.elapsed_ms
    );
}

/// Threshold ECDSA over the real network: 2-of-3 DKG (FROST + ECDSA aux
/// info + keygen), then EIP-191 signings by every signer pair — consecutive
/// signings on one cluster, each pair with a different initiator — plus a
/// raw 32-byte prehash signing (a transaction-sighash stand-in). Every
/// signature must `ecrecover` to account 0's Ethereum address from
/// accounts.rs, identically on both signers.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC/DKG+ECDSA over loopback; run with --ignored"]
async fn ecdsa_2_of_3_every_signer_pair_recovers_the_account_address() {
    init_logs();
    let prehash = "a1".repeat(32);
    let r = starlab_cli::simulate::run_ecdsa_e2e(
        ecdsa_opts(2),
        &[
            (0, vec![1], "hello from signers 0+1", "utf8"),
            (0, vec![2], "hello from signers 0+2", "utf8"),
            (1, vec![2], "hello from signers 1+2", "utf8"),
            (2, vec![0], &prehash, "prehash"),
        ],
    )
    .await
    .expect("ECDSA e2e ran");
    eprintln!("{}", serde_json::to_string_pretty(&r).unwrap());
    assert!(r.ok(), "{r:?}");
    assert_eq!(r.signings[3].signed_hash, prehash, "prehash signed as-is");
    eprintln!(
        "✅ ECDSA 2-of-3: DKG {}ms, signings {:?}ms",
        r.dkg_ms,
        r.signings.iter().map(|s| s.elapsed_ms).collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC/DKG+ECDSA over loopback; run with --ignored"]
async fn ecdsa_3_of_3_signs_twice() {
    init_logs();
    let r = starlab_cli::simulate::run_ecdsa_e2e(
        ecdsa_opts(3),
        &[
            (0, vec![1, 2], "3-of-3 first", "utf8"),
            (2, vec![0, 1], "3-of-3 second", "utf8"),
        ],
    )
    .await
    .expect("ECDSA 3-of-3 e2e ran");
    assert!(r.ok(), "{r:?}");
    eprintln!(
        "✅ ECDSA 3-of-3: DKG {}ms, signings {:?}ms",
        r.dkg_ms,
        r.signings.iter().map(|s| s.elapsed_ms).collect::<Vec<_>>()
    );
}
