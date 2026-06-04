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
        let mut child = Command::new(env!("CARGO_BIN_EXE_mpc-wallet-cli"))
            .arg("serve")
            .args(["--device-id", device_id])
            .args(["--keystore", keystore])
            .args(["--signal-server", ws_url])
            .args(["--log-level", "warn"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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

    async fn quit(&mut self) {
        let _ = self.send(json!({"cmd": "quit"})).await;
        let _ = timeout(Duration::from_secs(5), self.child.wait()).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "spawns serve processes + real WebRTC over loopback; run with --ignored"]
async fn dkg_2_of_2_across_serve_processes() {
    // Embedded signal server on a loopback ephemeral port.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(webrtc_signal_server::run(listener));
    let ws_url = format!("ws://127.0.0.1:{port}");

    let ks_a = tempfile::TempDir::new().unwrap();
    let ks_b = tempfile::TempDir::new().unwrap();

    let mut a = ServeProc::spawn("proc-node-a", &ks_a.path().to_string_lossy(), &ws_url)
        .await
        .expect("spawn a");
    let mut b = ServeProc::spawn("proc-node-b", &ks_b.path().to_string_lossy(), &ws_url)
        .await
        .expect("spawn b");

    // Both greet.
    assert_eq!(a.wait_for("ready", 10).await.unwrap()["event"], "ready");
    assert_eq!(b.wait_for("ready", 10).await.unwrap()["event"], "ready");

    // Connect both to the signal server BEFORE the announce so b receives the
    // live broadcast.
    a.send(json!({"cmd": "connect"})).await.unwrap();
    b.send(json!({"cmd": "connect"})).await.unwrap();
    a.wait_connected(15).await.expect("a connected");
    b.wait_connected(15).await.expect("b connected");

    // a creates the wallet → announces a DKG session.
    a.send(json!({"id": 1, "cmd": "create_wallet", "threshold": 2, "total": 2, "password": "pw-a"}))
        .await
        .unwrap();

    // The creator emits session_announced; the late peer discovers the same
    // session via session_available — assert they agree on the id.
    let announced = a.wait_for("session_announced", 20).await.unwrap();
    let id_a = announced["session_id"].as_str().unwrap().to_string();
    let avail = b.wait_for("session_available", 20).await.unwrap();
    let id_b = avail["session"]["session_id"].as_str().unwrap().to_string();
    assert_eq!(id_a, id_b, "creator and peer disagree on session id");

    // b joins what it discovered.
    b.send(json!({"id": 2, "cmd": "join_session", "session_id": id_b, "password": "pw-b"}))
        .await
        .unwrap();

    // Both finish DKG with the same group key.
    let done_a = a.wait_for("dkg_complete", 90).await.expect("a dkg_complete");
    let done_b = b.wait_for("dkg_complete", 90).await.expect("b dkg_complete");
    let key_a = done_a["group_public_key"].as_str().unwrap();
    let key_b = done_b["group_public_key"].as_str().unwrap();
    assert!(!key_a.is_empty(), "empty group key");
    assert_eq!(key_a, key_b, "processes disagree on group key");

    eprintln!("✅ 2-of-2 DKG across serve processes; group={key_a}");

    a.quit().await;
    b.quit().await;
}
