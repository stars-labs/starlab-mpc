//! L1 conformance matrix — Phase 1 of docs/cli-conformance-testing.md.
//!
//! Drives the §3 flow catalog entirely through CLI↔CLI runners in-process
//! (embedded signal server, real WebRTC over loopback, real FROST). This is
//! the always-on regression net for the shared Rust Elm core: every (n, t)
//! shape and the threshold-quorum signing path run here.
//!
//! Unlike the per-flow tests in `e2e_dkg.rs`, the matrix runs the WHOLE table
//! and reports EVERY failure at the end (no early abort) — a partial protocol
//! regression shows which shapes broke, not just the first.
//!
//! `#[ignore]` by default (real UDP/ICE on loopback, ~seconds per case). Run:
//!   cargo test -p mpc-wallet-cli --test conformance_matrix -- --ignored --nocapture

use mpc_wallet_cli::simulate::{
    run_late_join_discovery_simulation, run_reload_list_simulation, run_reload_unlock_simulation,
    run_signing_simulation, run_signing_simulation_enc, run_simulation, SimulateOpts,
    SIM_WALLET_LABEL,
};

fn init_logs() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("tui_node=warn,webrtc=warn")),
        )
        .with_test_writer()
        .try_init();
}

fn opts(nodes: usize, threshold: u16) -> SimulateOpts {
    opts_curve(nodes, threshold, "secp256k1")
}

fn opts_curve(nodes: usize, threshold: u16, curve: &str) -> SimulateOpts {
    SimulateOpts {
        nodes,
        threshold,
        curve: curve.into(),
        signal_url: None,
        // Larger sets need more ICE/DKG time; scale with node count.
        timeout_secs: 60 + (nodes as u64) * 20,
    }
}

/// One row's verdict, accumulated so the whole matrix runs before asserting.
struct Row {
    label: String,
    ok: bool,
    detail: String,
}

/// DKG catalog: DKG-1 (2-of-2), DKG-2 (2-of-3), DKG-3 (3-of-5), plus a small
/// t-of-n sweep (DKG-4). Each must finish with every node agreeing on one
/// non-empty group key.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
async fn dkg_matrix() {
    init_logs();
    // (nodes, threshold)
    let cases = [(2u16, 2u16), (3, 2), (3, 3), (5, 3), (4, 2)];
    let mut rows = Vec::new();

    for (n, t) in cases {
        let label = format!("DKG {t}-of-{n}");
        match run_simulation(opts(n as usize, t)).await {
            Ok(r) => {
                let ok = r.agreed
                    && r.outcomes.len() == n as usize
                    && !r.group_public_key.is_empty();
                rows.push(Row {
                    detail: format!(
                        "agreed={} outcomes={} {}ms group={}…",
                        r.agreed,
                        r.outcomes.len(),
                        r.elapsed_ms,
                        &r.group_public_key[..8.min(r.group_public_key.len())]
                    ),
                    ok,
                    label,
                });
            }
            Err(e) => rows.push(Row { label, ok: false, detail: format!("error: {e}") }),
        }
    }

    report_and_assert("DKG", &rows);
}

/// DKG-5: ed25519 (Solana) ciphersuite. Same agreement invariant as the
/// secp256k1 matrix, exercising the spawn_ed25519 runner and the curve-generic
/// DKG/keystore path end-to-end.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
async fn dkg_ed25519_matrix() {
    init_logs();
    let cases = [(2u16, 2u16), (3, 2)];
    let mut rows = Vec::new();

    for (n, t) in cases {
        let label = format!("DKG-ed25519 {t}-of-{n}");
        match run_simulation(opts_curve(n as usize, t, "ed25519")).await {
            Ok(r) => {
                let ok = r.agreed
                    && r.outcomes.len() == n as usize
                    && !r.group_public_key.is_empty();
                rows.push(Row {
                    detail: format!(
                        "agreed={} outcomes={} {}ms group={}…",
                        r.agreed,
                        r.outcomes.len(),
                        r.elapsed_ms,
                        &r.group_public_key[..8.min(r.group_public_key.len())]
                    ),
                    ok,
                    label,
                });
            }
            Err(e) => rows.push(Row { label, ok: false, detail: format!("error: {e}") }),
        }
    }

    report_and_assert("DKG-ed25519", &rows);
}

/// Signing catalog: SIG-1 (sign with exactly threshold signers) and SIG-2
/// (sign with a threshold quorum when MORE than threshold are available —
/// `run_signing_simulation` recruits exactly `threshold` of `nodes`). Every
/// produced signature must verify against the group key.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/DKG+signing over loopback; run with --ignored"]
async fn sign_matrix() {
    init_logs();
    // (nodes, threshold): (2,2) exact; (3,2)/(5,3) are quorum-subset (SIG-2).
    let cases = [(2u16, 2u16), (3, 2), (5, 3)];
    let msg = "conformance-matrix signing payload";
    let mut rows = Vec::new();

    for (n, t) in cases {
        let label = format!("SIGN {t}-of-{n}");
        match run_signing_simulation(opts(n as usize, t), msg).await {
            Ok(r) => {
                let ok = r.verified && !r.signature.is_empty();
                rows.push(Row {
                    detail: format!(
                        "verified={} {}ms sig={}…",
                        r.verified,
                        r.elapsed_ms,
                        &r.signature[..16.min(r.signature.len())]
                    ),
                    ok,
                    label,
                });
            }
            Err(e) => rows.push(Row { label, ok: false, detail: format!("error: {e}") }),
        }
    }

    report_and_assert("SIGN", &rows);
}

/// SIG-4: ed25519 sign + verify. Exercises the ed25519 signing ceremony end to
/// end (raw-bytes signing — no EIP-191 hash) and verifies against the ed25519
/// group key.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/DKG+signing over loopback; run with --ignored"]
async fn sign_ed25519_verifies() {
    init_logs();
    let mut rows = Vec::new();
    let cases = [(2u16, 2u16), (3, 2)];
    let msg = "ed25519 conformance signing payload";

    for (n, t) in cases {
        let label = format!("SIG-4 ed25519 {t}-of-{n}");
        match run_signing_simulation_enc(opts_curve(n as usize, t, "ed25519"), msg, "utf8").await {
            Ok(r) => rows.push(Row {
                ok: r.verified && !r.signature.is_empty(),
                detail: format!("verified={} {}ms", r.verified, r.elapsed_ms),
                label,
            }),
            Err(e) => rows.push(Row { label, ok: false, detail: format!("error: {e}") }),
        }
    }

    report_and_assert("SIG-4", &rows);
}

/// SIG-6: sign a HEX-encoded message. Exercises HeadlessSign's hex-decode path
/// (decode → EIP-191 hash → FROST), which the utf8 matrix never touches. The
/// produced signature must verify against the group key.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/DKG+signing over loopback; run with --ignored"]
async fn sign_hex_encoded_message_verifies() {
    init_logs();
    let mut rows = Vec::new();
    // 32 bytes of hex (0x-prefixed; HeadlessSign strips the prefix).
    let hex_msg = "0xdeadbeefcafe0123456789abcdef0011223344556677889900aabbccddeeff00";

    match run_signing_simulation_enc(opts(2, 2), hex_msg, "hex").await {
        Ok(r) => rows.push(Row {
            ok: r.verified && !r.signature.is_empty(),
            detail: format!("verified={} {}ms (hex-encoded message)", r.verified, r.elapsed_ms),
            label: "SIG-6 hex message 2-of-2".to_string(),
        }),
        Err(e) => rows.push(Row {
            label: "SIG-6 hex message 2-of-2".to_string(),
            ok: false,
            detail: format!("error: {e}"),
        }),
    }

    report_and_assert("SIG-6", &rows);
}

/// LIFE-1: cold-start persistence. Run DKG, tear node 0's runner down, bring
/// a FRESH runner up on the SAME keystore, and list wallets — the persisted
/// share must reappear with the original group key. Pure keystore round-trip
/// (no network), so deterministic. (Faithful re-signing after restart, LIFE-2,
/// needs real process death and lives in the L3 serve-subprocess harness — an
/// in-process reload leaves the old node's WebRTC ICE agents alive, which
/// corrupt a new mesh; see docs/cli-conformance-testing.md.)
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
async fn reload_lists_persisted_wallet() {
    init_logs();
    let cases = [(2u16, 2u16), (3, 2)];
    let mut rows = Vec::new();

    for (n, t) in cases {
        let label = format!("RELOAD-LIST {t}-of-{n}");
        match run_reload_list_simulation(opts(n as usize, t)).await {
            Ok(r) => rows.push(Row {
                ok: r.persisted,
                detail: format!(
                    "persisted={} {}ms group={}… ({} wallet(s) reloaded)",
                    r.persisted,
                    r.elapsed_ms,
                    &r.expected_group_public_key[..8.min(r.expected_group_public_key.len())],
                    r.reloaded_group_keys.len(),
                ),
                label,
            }),
            Err(e) => rows.push(Row { label, ok: false, detail: format!("error: {e}") }),
        }
    }

    report_and_assert("RELOAD-LIST", &rows);
}

/// LIFE-3: the user-facing wallet label round-trips through the keystore. After
/// DKG (creator labels the wallet) and a cold reload, `display_name()` must
/// still be the label — not the `session_id` fallback it degrades to when the
/// label is dropped on persist.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
async fn reload_preserves_wallet_label() {
    init_logs();
    let mut rows = Vec::new();
    let label = "LIFE-3 label round-trip";

    match run_reload_list_simulation(opts(2, 2)).await {
        Ok(r) => {
            let name = r.reloaded_wallet_names.first().cloned().unwrap_or_default();
            let ok = name == SIM_WALLET_LABEL;
            rows.push(Row {
                ok,
                detail: format!(
                    "reloaded name={name:?} (expected label {SIM_WALLET_LABEL:?})"
                ),
                label: label.to_string(),
            });
        }
        Err(e) => rows.push(Row { label: label.to_string(), ok: false, detail: format!("error: {e}") }),
    }

    report_and_assert("RELOAD-LABEL", &rows);
}

/// ERR-1: a wrong password is rejected cleanly (no panic, no partial state),
/// and the correct password unlocks (positive control proves the probe
/// actually exercises the unlock path). node 0's DKG password is
/// "sim-password-0".
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
async fn wrong_password_rejected_cleanly() {
    init_logs();
    let mut rows = Vec::new();

    // Wrong password → must be rejected.
    match run_reload_unlock_simulation(opts(2, 2), "definitely-not-the-password").await {
        Ok(r) => rows.push(Row {
            ok: r.failed && !r.unlocked,
            detail: format!("failed={} unlocked={} error={:?}", r.failed, r.unlocked, r.error),
            label: "ERR-1 wrong password rejected".to_string(),
        }),
        Err(e) => rows.push(Row {
            label: "ERR-1 wrong password rejected".to_string(),
            ok: false,
            detail: format!("error: {e}"),
        }),
    }

    // Correct password → must unlock (control).
    match run_reload_unlock_simulation(opts(2, 2), "sim-password-0").await {
        Ok(r) => rows.push(Row {
            ok: r.unlocked && !r.failed,
            detail: format!("unlocked={} failed={}", r.unlocked, r.failed),
            label: "ERR-1 correct password unlocks".to_string(),
        }),
        Err(e) => rows.push(Row {
            label: "ERR-1 correct password unlocks".to_string(),
            ok: false,
            detail: format!("error: {e}"),
        }),
    }

    report_and_assert("ERR-1", &rows);
}

/// LIFE-4: a session announced before a node connects is still discoverable.
/// node 1 connects after node 0's announce (missing the live broadcast) and
/// must find the session via the RequestActiveSessions replay. Also reports
/// whether discovery happened automatically on connect (the extension's
/// behavior) — currently the headless/CLI path needs an explicit refresh.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real WebRTC/WS over loopback; run with --ignored"]
async fn late_joiner_discovers_via_replay() {
    init_logs();
    let mut rows = Vec::new();

    match run_late_join_discovery_simulation(opts(2, 2)).await {
        Ok(r) => rows.push(Row {
            ok: r.discovered_after_refresh,
            detail: format!(
                "after_refresh={} on_connect={} {}ms",
                r.discovered_after_refresh, r.discovered_on_connect, r.elapsed_ms
            ),
            label: "LIFE-4 late joiner discovers via replay".to_string(),
        }),
        Err(e) => rows.push(Row {
            label: "LIFE-4 late joiner discovers via replay".to_string(),
            ok: false,
            detail: format!("error: {e}"),
        }),
    }

    report_and_assert("LIFE-4", &rows);
}

fn report_and_assert(group: &str, rows: &[Row]) {
    eprintln!("\n=== {group} conformance matrix ===");
    for r in rows {
        eprintln!("  [{}] {} — {}", if r.ok { "ok" } else { "FAIL" }, r.label, r.detail);
    }
    let failed: Vec<&str> = rows.iter().filter(|r| !r.ok).map(|r| r.label.as_str()).collect();
    assert!(failed.is_empty(), "{group} matrix had failures: {failed:?}");
}
