//! L3 (process isolation): drive the REAL `mpc-wallet-cli serve` binary as
//! separate OS processes and run a full DKG between them over the JSONL
//! protocol. This is the first cross-process layer from
//! docs/cli-conformance-testing.md — nothing else exercises the actual
//! compiled binary's stdin/stdout surface, and it's the substrate the
//! cross-client (native/TUI/extension) interop tests build on.
//!
//! The signal server runs in-process (this test is itself a Tokio program);
//! the two `serve` children connect to it over loopback. Because each node is
//! a separate process, teardown is real OS process death — which is exactly
//! why this layer (not the in-process simulate) is where cold-restart signing
//! (LIFE-2) will eventually live.
//!
//! `#[ignore]` by default (spawns processes + real WebRTC over loopback):
//!   cargo test -p mpc-wallet-cli --test l3_serve_process -- --ignored --nocapture

use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::TcpListener;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::{timeout, Instant};

/// A running `serve` child with line-buffered JSONL I/O.
struct ServeProc {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
}

impl ServeProc {
    async fn spawn(device_id: &str, keystore: &str, ws_url: &str) -> anyhow::Result<Self> {
        Self::spawn_with_args(device_id, keystore, ws_url, &[]).await
    }

    async fn spawn_with_args(
        device_id: &str,
        keystore: &str,
        ws_url: &str,
        extra: &[&str],
    ) -> anyhow::Result<Self> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mpc-wallet-cli"))
            .arg("serve")
            .args(["--device-id", device_id])
            .args(["--keystore", keystore])
            .args(["--signal-server", ws_url])
            .args(extra)
            .args(["--log-level", std::env::var("L3_LOG").as_deref().unwrap_or("warn")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if std::env::var("L3_LOG").is_ok() {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .kill_on_drop(true)
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");
        Ok(Self {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
        })
    }

    async fn send(&mut self, v: Value) -> anyhow::Result<()> {
        self.stdin.write_all(v.to_string().as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Send a raw line verbatim (for malformed-input tests).
    async fn send_raw(&mut self, line: &str) -> anyhow::Result<()> {
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Read the next JSONL event (any type).
    async fn next_event(&mut self, secs: u64) -> anyhow::Result<Value> {
        let line = timeout(Duration::from_secs(secs), self.lines.next_line())
            .await
            .map_err(|_| anyhow::anyhow!("timed out reading event"))?
            .map_err(|e| anyhow::anyhow!("stdout read error: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("child stdout closed"))?;
        serde_json::from_str(&line).map_err(|e| anyhow::anyhow!("bad JSONL '{line}': {e}"))
    }

    /// Read events until one with `"event" == event` arrives (or time out).
    async fn wait_for(&mut self, event: &str, secs: u64) -> anyhow::Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                anyhow::bail!("timed out waiting for event '{event}'");
            }
            let v = self.next_event(remaining.as_secs().max(1)).await?;
            if v["event"] == event {
                return Ok(v);
            }
        }
    }

    /// Wait for a `connection` event reporting connected=true.
    async fn wait_connected(&mut self, secs: u64) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                anyhow::bail!("timed out waiting for connection");
            }
            let v = self.next_event(remaining.as_secs().max(1)).await?;
            if v["event"] == "connection" && v["connected"] == true {
                return Ok(());
            }
        }
    }

    /// Collect every event line for up to `secs`, returning them as raw
    /// strings (used to scan the whole output stream, e.g. for secret leaks).
    async fn drain_lines(&mut self, secs: u64) -> Vec<String> {
        let mut out = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match timeout(remaining, self.lines.next_line()).await {
                Ok(Ok(Some(line))) => out.push(line),
                _ => break,
            }
        }
        out
    }

    async fn quit(&mut self) {
        let _ = self.send(json!({"cmd": "quit"})).await;
        let _ = timeout(Duration::from_secs(5), self.child.wait()).await;
    }
}

/// Spawn a serve process, consume its `ready`, connect it, and wait until the
/// signal-server connection is up.
async fn spawn_connected(device_id: &str, keystore: &str, ws_url: &str) -> anyhow::Result<ServeProc> {
    let mut p = ServeProc::spawn(device_id, keystore, ws_url).await?;
    p.wait_for("ready", 10).await?;
    p.send(json!({"cmd": "connect"})).await?;
    p.wait_connected(15).await?;
    Ok(p)
}

/// Run a 2-of-2 DKG between two connected processes (a = creator, b = joiner).
/// Returns the creator's reported (wallet_id, group_public_key). Also asserts
/// the creator's `session_announced` id matches the joiner's discovered id.
async fn dkg_2of2(a: &mut ServeProc, b: &mut ServeProc) -> anyhow::Result<(String, String)> {
    a.send(json!({"id": 1, "cmd": "create_wallet", "threshold": 2, "total": 2, "password": "pw-a"}))
        .await?;
    let announced = a.wait_for("session_announced", 20).await?;
    let id_a = announced["session_id"].as_str().unwrap().to_string();
    let avail = b.wait_for("session_available", 20).await?;
    let id_b = avail["session"]["session_id"].as_str().unwrap().to_string();
    anyhow::ensure!(id_a == id_b, "creator/peer disagree on session id: {id_a} != {id_b}");
    b.send(json!({"id": 2, "cmd": "join_session", "session_id": id_b, "password": "pw-b"}))
        .await?;
    let done_a = a.wait_for("dkg_complete", 90).await?;
    let _done_b = b.wait_for("dkg_complete", 90).await?;
    Ok((
        done_a["wallet_id"].as_str().unwrap().to_string(),
        done_a["group_public_key"].as_str().unwrap().to_string(),
    ))
}

/// Verify a FROST(secp256k1) signature against the group verifying key. Inputs
/// are hex (the `signature_complete` event 0x-prefixes both; group key is bare).
fn verify_secp256k1(group_hex: &str, msg_hex: &str, sig_hex: &str) -> bool {
    use frost_secp256k1::{Signature, VerifyingKey};
    let strip = |s: &str| s.trim_start_matches("0x").to_string();
    let (Ok(vkb), Ok(msg), Ok(sigb)) = (
        hex::decode(strip(group_hex)),
        hex::decode(strip(msg_hex)),
        hex::decode(strip(sig_hex)),
    ) else {
        return false;
    };
    match (VerifyingKey::deserialize(&vkb), Signature::deserialize(&sigb)) {
        (Ok(vk), Ok(sig)) => vk.verify(&msg, &sig).is_ok(),
        _ => false,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "spawns serve processes + real WebRTC over loopback; run with --ignored"]
async fn dkg_2_of_2_across_serve_processes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(webrtc_signal_server::run(listener));
    let ws_url = format!("ws://127.0.0.1:{port}");

    let ks_a = tempfile::TempDir::new().unwrap();
    let ks_b = tempfile::TempDir::new().unwrap();

    // Connect both BEFORE the announce so b receives the live broadcast.
    let mut a = spawn_connected("proc-node-a", &ks_a.path().to_string_lossy(), &ws_url)
        .await
        .expect("a connected");
    let mut b = spawn_connected("proc-node-b", &ks_b.path().to_string_lossy(), &ws_url)
        .await
        .expect("b connected");

    let (_wallet_id, group_key) = dkg_2of2(&mut a, &mut b).await.expect("dkg");
    assert!(!group_key.is_empty(), "empty group key");
    eprintln!("✅ 2-of-2 DKG across serve processes; group={group_key}");

    a.quit().await;
    b.quit().await;
}

/// SIG-8: a co-signer running `serve --auto-approve` contributes its share to
/// an incoming signing request WITHOUT any manual approve command — gated by
/// the auto-approval policy. Exercises the security-sensitive auto-approve path
/// end to end: DKG across two processes (one in auto-approve mode), then the
/// initiator signs and the co-signer auto-joins; the signature must verify.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "spawns serve processes + real WebRTC over loopback; run with --ignored"]
async fn auto_approve_co_signer_signs_without_manual_approval() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(webrtc_signal_server::run(listener));
    let ws_url = format!("ws://127.0.0.1:{port}");

    let ks_a = tempfile::TempDir::new().unwrap();
    let ks_b = tempfile::TempDir::new().unwrap();
    // b's wallet password (set at DKG join) lives in a file for --auto-approve.
    let pw_dir = tempfile::TempDir::new().unwrap();
    let pw_file = pw_dir.path().join("approve.pw");
    std::fs::write(&pw_file, "pw-b").unwrap();
    let pw_file = pw_file.to_string_lossy().to_string();

    let mut a = spawn_connected("auto-a", &ks_a.path().to_string_lossy(), &ws_url)
        .await
        .expect("a");
    // b auto-approves any wallet (empty allowlist) using pw-b from the file.
    let mut b = ServeProc::spawn_with_args(
        "auto-b",
        &ks_b.path().to_string_lossy(),
        &ws_url,
        &["--auto-approve", "--approve-password-file", &pw_file],
    )
    .await
    .expect("spawn b");
    b.wait_for("ready", 10).await.expect("b ready");
    b.send(json!({"cmd": "connect"})).await.unwrap();
    b.wait_connected(15).await.expect("b connected");

    // DKG (b joins manually — auto-approve only governs SIGNING).
    let (wallet_id, group_key) = dkg_2of2(&mut a, &mut b).await.expect("dkg");
    assert!(!group_key.is_empty());

    // a initiates signing. b should auto-approve with NO manual command.
    a.send(json!({
        "id": 10, "cmd": "sign", "wallet_id": wallet_id,
        "message": "auto-approve me", "encoding": "utf8", "password": "pw-a"
    }))
    .await
    .unwrap();

    // a receives the aggregated signature purely via b's auto-contribution.
    let sc = a.wait_for("signature_complete", 90).await.expect("signature_complete");
    assert!(
        verify_secp256k1(&group_key, sc["message_hash"].as_str().unwrap(), sc["signature"].as_str().unwrap()),
        "auto-approved signature failed to verify"
    );
    eprintln!("✅ SIG-8: co-signer auto-approved; signature verified");

    a.quit().await;
    b.quit().await;
}

/// SEC-5: the wallet password must NEVER appear in any event emitted on
/// stdout. Passwords enter via stdin commands only; the daemon must not echo
/// them back in acks, announcements, errors, or any other event. Guards against
/// accidental secret leakage into the protocol stream (and anything tee'ing it).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "spawns a serve process; run with --ignored"]
async fn password_never_appears_in_events() {
    const SECRET: &str = "SEC5-correct-horse-battery-staple-SENTINEL";
    let ks = tempfile::TempDir::new().unwrap();
    // No real signal server; create_wallet still carries the password and the
    // daemon acks/announces, which is enough to catch any echo.
    let mut p = ServeProc::spawn("sec5-node", &ks.path().to_string_lossy(), "ws://127.0.0.1:1")
        .await
        .expect("spawn");
    p.wait_for("ready", 10).await.expect("ready");

    p.send(json!({
        "id": 1, "cmd": "create_wallet", "name": "sec5", "threshold": 2, "total": 2,
        "password": SECRET
    }))
    .await
    .unwrap();
    p.send(json!({"cmd": "status"})).await.unwrap();
    p.send(json!({"cmd": "list_wallets"})).await.unwrap();

    // Collect everything the daemon emits for a couple of seconds.
    let lines = p.drain_lines(3).await;
    assert!(!lines.is_empty(), "expected some events");
    for line in &lines {
        assert!(
            !line.contains(SECRET),
            "password leaked into an emitted event: {line}"
        );
    }
    eprintln!("✅ SEC-5: scanned {} event lines, no password leak", lines.len());
    p.quit().await;
}

/// ERR-7: a malformed JSONL line is rejected cleanly (`Error{code:"bad_request"}`)
/// and the daemon's input loop SURVIVES — a subsequent valid command still
/// works. Exercises the real serve process's stdin error handling end to end.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "spawns a serve process; run with --ignored"]
async fn malformed_jsonl_is_rejected_and_loop_survives() {
    // No signal server needed — this never connects.
    let ks = tempfile::TempDir::new().unwrap();
    let mut p = ServeProc::spawn("err7-node", &ks.path().to_string_lossy(), "ws://127.0.0.1:1")
        .await
        .expect("spawn");
    p.wait_for("ready", 10).await.expect("ready");

    // Garbage in → a bad_request error out, no crash.
    p.send_raw("this is not json {{{").await.unwrap();
    let err = p.wait_for("error", 10).await.expect("error event");
    assert_eq!(err["code"], "bad_request", "unexpected error: {err}");

    // The loop must still be alive: a valid command still gets answered.
    p.send(json!({"cmd": "status"})).await.unwrap();
    let status = p.wait_for("status", 10).await.expect("status after bad input");
    assert_eq!(status["device_id"], "err7-node");

    eprintln!("✅ ERR-7: bad_request emitted, loop survived");
    p.quit().await;
}

/// LIFE-2 (faithful, in L3): DKG across two processes, then KILL both and bring
/// fresh `serve` processes up on the SAME keystores and sign. The persisted
/// share alone — after real OS process death — must produce a signature that
/// verifies against the original group key. This is the production restart
/// condition.
///
/// Regression guard for the cold-start signing bug (now FIXED). It required two
/// fixes, both verified by this test going green: (1) an always-on WebRTC relay
/// handler so the cold-started co-signer receives the initiator's offer even
/// though no DKG driver loop is running this session, and (2) a pre-session
/// SIGN_COMMIT buffer so the initiator's commit — which races ahead of the
/// co-signer's JoinSigning session setup over the freshly-formed mesh — is
/// re-fed instead of dropped. See docs/cli-conformance-testing.md.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "spawns serve processes + real WebRTC over loopback; run with --ignored"]
async fn sign_after_process_restart_verifies() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(webrtc_signal_server::run(listener));
    let ws_url = format!("ws://127.0.0.1:{port}");

    let ks_a = tempfile::TempDir::new().unwrap();
    let ks_b = tempfile::TempDir::new().unwrap();
    let pa = ks_a.path().to_string_lossy().to_string();
    let pb = ks_b.path().to_string_lossy().to_string();

    // --- Phase 1: DKG, then kill both processes. ---
    let (wallet_id, group_key) = {
        let mut a = spawn_connected("restart-a", &pa, &ws_url).await.expect("a");
        let mut b = spawn_connected("restart-b", &pb, &ws_url).await.expect("b");
        let r = dkg_2of2(&mut a, &mut b).await.expect("dkg");
        a.quit().await;
        b.quit().await;
        r
    };
    assert!(!group_key.is_empty());
    // Give the server a moment to drop the closed connections.
    tokio::time::sleep(Duration::from_secs(1)).await;

    // --- Phase 2: fresh processes on the SAME keystores, then sign. ---
    let mut a = spawn_connected("restart-a", &pa, &ws_url).await.expect("a restart");
    let mut b = spawn_connected("restart-b", &pb, &ws_url).await.expect("b restart");

    // a initiates the signing (unlocks the persisted share, announces).
    a.send(json!({
        "id": 10, "cmd": "sign", "wallet_id": wallet_id,
        "message": "life2 across a real restart", "encoding": "utf8", "password": "pw-a"
    }))
    .await
    .unwrap();

    // b discovers the signing request and approves by joining.
    let req = b.wait_for("signing_request", 30).await.expect("b signing_request");
    let sid = req["session_id"].as_str().unwrap().to_string();
    b.send(json!({"id": 11, "cmd": "approve_signing", "session_id": sid, "password": "pw-b"}))
        .await
        .unwrap();

    // a receives the aggregated signature; it must verify against the group key.
    let sc = a.wait_for("signature_complete", 90).await.expect("a signature_complete");
    let sig = sc["signature"].as_str().unwrap();
    let msg = sc["message_hash"].as_str().unwrap();
    assert!(
        verify_secp256k1(&group_key, msg, sig),
        "post-restart signature failed to verify: sig={sig} msg={msg} group={group_key}"
    );
    eprintln!("✅ LIFE-2: signed after real process restart; signature verified");

    a.quit().await;
    b.quit().await;
}

/// RESHARE-L3 (#56): a full networked share refresh across two REAL `serve`
/// processes, then a signature with the REFRESHED shares. After DKG we KILL both
/// processes and bring fresh ones up on the SAME keystores — so the reshare runs
/// over a brand-new mesh, each node loading its OLD share from disk (exactly what
/// a separate device does). The group public key (address) must survive the
/// refresh, and a 2-of-2 signature with the new shares must verify against it.
///
/// This is the cross-process counterpart to the in-process `reshare_then_sign`
/// e2e: it exercises the actual compiled binary's `reshare` / `reshare_request`
/// / `reshare_complete` JSONL surface and the announce/join ceremony end to end.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "spawns serve processes + real WebRTC over loopback; run with --ignored"]
async fn reshare_then_sign_across_serve_processes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(webrtc_signal_server::run(listener));
    let ws_url = format!("ws://127.0.0.1:{port}");

    let ks_a = tempfile::TempDir::new().unwrap();
    let ks_b = tempfile::TempDir::new().unwrap();
    let pa = ks_a.path().to_string_lossy().to_string();
    let pb = ks_b.path().to_string_lossy().to_string();

    // --- Phase 1: DKG, then kill both processes (shares persist to disk). ---
    let (wallet_id, group_key) = {
        let mut a = spawn_connected("reshare-a", &pa, &ws_url).await.expect("a");
        let mut b = spawn_connected("reshare-b", &pb, &ws_url).await.expect("b");
        let r = dkg_2of2(&mut a, &mut b).await.expect("dkg");
        a.quit().await;
        b.quit().await;
        r
    };
    assert!(!group_key.is_empty());
    // Give the server a moment to drop the closed connections before reuse.
    tokio::time::sleep(Duration::from_secs(1)).await;

    // --- Phase 2: fresh processes on the SAME keystores, then reshare. ---
    let mut a = spawn_connected("reshare-a", &pa, &ws_url).await.expect("a restart");
    let mut b = spawn_connected("reshare-b", &pb, &ws_url).await.expect("b restart");

    // a initiates the reshare (loads its OLD share, announces a reshare session).
    a.send(json!({"id": 20, "cmd": "reshare", "wallet_id": wallet_id, "password": "pw-a"}))
        .await
        .unwrap();

    // b discovers the reshare request and approves by joining (contributing a
    // refreshed share). `join_session` is the reshare approval path.
    let req = b.wait_for("reshare_request", 30).await.expect("b reshare_request");
    let sid = req["session_id"].as_str().unwrap().to_string();
    b.send(json!({"id": 21, "cmd": "join_session", "session_id": sid, "password": "pw-b"}))
        .await
        .unwrap();

    // Both nodes complete the refresh; the group key (address) must be unchanged.
    let rc_a = a.wait_for("reshare_complete", 90).await.expect("a reshare_complete");
    let _rc_b = b.wait_for("reshare_complete", 90).await.expect("b reshare_complete");
    assert_eq!(
        rc_a["group_public_key"].as_str().unwrap(),
        group_key,
        "group key changed across reshare"
    );

    // --- Phase 3: sign with the REFRESHED shares; must verify. ---
    a.send(json!({
        "id": 30, "cmd": "sign", "wallet_id": wallet_id,
        "message": "signed after a networked reshare", "encoding": "utf8", "password": "pw-a"
    }))
    .await
    .unwrap();
    let req = b.wait_for("signing_request", 30).await.expect("b signing_request");
    let sid = req["session_id"].as_str().unwrap().to_string();
    b.send(json!({"id": 31, "cmd": "approve_signing", "session_id": sid, "password": "pw-b"}))
        .await
        .unwrap();
    let sc = a.wait_for("signature_complete", 90).await.expect("a signature_complete");
    let sig = sc["signature"].as_str().unwrap();
    let msg = sc["message_hash"].as_str().unwrap();
    assert!(
        verify_secp256k1(&group_key, msg, sig),
        "post-reshare signature failed to verify: sig={sig} msg={msg} group={group_key}"
    );
    eprintln!("✅ RESHARE-L3: reshared across processes, group preserved, refreshed shares signed");

    a.quit().await;
    b.quit().await;
}
