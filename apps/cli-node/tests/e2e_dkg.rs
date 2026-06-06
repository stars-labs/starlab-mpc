//! End-to-end DKG test (issue #16), built on the shared `simulate`
//! orchestrator (#21): real FROST DKG across N `HeadlessRunner`s in one
//! process, against an embedded signal server, WebRTC over loopback.
//!
//! `#[ignore]` by default (real UDP/ICE on loopback, ~seconds). Run with:
//!   cargo test -p mpc-wallet-cli --test e2e_dkg -- --ignored --nocapture

use mpc_wallet_cli::simulate::{run_signing_simulation, run_simulation, SimulateOpts};

fn init_logs() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("tui_node=warn,webrtc=warn")),
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
    })
    .await
    .expect("simulation ran");

    assert!(result.agreed, "nodes disagreed on group key: {:?}", result.outcomes);
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
    })
    .await
    .expect("simulation ran");

    assert!(result.agreed, "nodes disagreed on group key: {:?}", result.outcomes);
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
