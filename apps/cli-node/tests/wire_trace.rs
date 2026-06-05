//! L2 wire-frame trace (Phase 2, point 1 of docs/cli-conformance-testing.md).
//!
//! Captures the actual on-the-wire signal-server protocol — the
//! `announce_session` / `session_available` / `relay` frames the browser
//! extension must parse and produce — WITHOUT instrumenting tui-node or the
//! signal server. A test-only recording WebSocket proxy sits between the
//! clients and the embedded signal server, logging every text frame in both
//! directions; the clients are simply pointed at the proxy via
//! `SimulateOpts.signal_url`. The captured frames are normalized (volatile
//! crypto/id fields → placeholders) and pinned as a golden — the protocol
//! spec the L4 differential oracle will diff the extension against.
//!
//! `#[ignore]` by default (real WebRTC/DKG over loopback).
//! Regenerate the golden after a reviewed protocol change:
//!   BLESS=1 cargo test -p mpc-wallet-cli --test wire_trace -- --ignored

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use mpc_wallet_cli::simulate::{run_simulation, SimulateOpts};

/// A captured frame: direction (`C>` client→server, `S>` server→client) + the
/// raw JSON text.
type FrameLog = Arc<Mutex<Vec<String>>>;

/// Spawn a recording WS proxy in front of `upstream_url`. Returns the proxy's
/// `ws://` URL and the shared frame log. Each accepted connection is bridged to
/// a fresh upstream connection; every text frame is recorded then forwarded.
async fn spawn_record_proxy(upstream_url: String) -> (String, FrameLog) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind proxy");
    let port = listener.local_addr().unwrap().port();
    let log: FrameLog = Arc::new(Mutex::new(Vec::new()));
    let log_accept = log.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(bridge_conn(stream, upstream_url.clone(), log_accept.clone()));
        }
    });
    (format!("ws://127.0.0.1:{port}"), log)
}

async fn bridge_conn(incoming: TcpStream, upstream_url: String, log: FrameLog) {
    let client_ws = match tokio_tungstenite::accept_async(incoming).await {
        Ok(w) => w,
        Err(_) => return,
    };
    let (upstream_ws, _) = match tokio_tungstenite::connect_async(&upstream_url).await {
        Ok(x) => x,
        Err(_) => return,
    };
    let (mut client_tx, mut client_rx) = client_ws.split();
    let (mut up_tx, mut up_rx) = upstream_ws.split();

    let log_c = log.clone();
    let c2s = tokio::spawn(async move {
        while let Some(Ok(msg)) = client_rx.next().await {
            if let Message::Text(t) = &msg {
                log_c.lock().unwrap().push(format!("C>{}", t.as_str()));
            }
            if up_tx.send(msg).await.is_err() {
                break;
            }
        }
    });
    let s2c = tokio::spawn(async move {
        while let Some(Ok(msg)) = up_rx.next().await {
            if let Message::Text(t) = &msg {
                log.lock().unwrap().push(format!("S>{}", t.as_str()));
            }
            if client_tx.send(msg).await.is_err() {
                break;
            }
        }
    });
    let _ = tokio::join!(c2s, s2c);
}

/// Run a DKG with all client traffic routed through the recording proxy.
/// Returns the captured frames.
async fn capture_dkg_frames(nodes: usize, threshold: u16) -> Vec<String> {
    // Embedded signal server.
    let server = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
    let s_port = server.local_addr().unwrap().port();
    tokio::spawn(webrtc_signal_server::run(server));
    // Recording proxy in front of it.
    let (proxy_url, log) = spawn_record_proxy(format!("ws://127.0.0.1:{s_port}")).await;

    let r = run_simulation(SimulateOpts {
        nodes,
        threshold,
        curve: "secp256k1".into(),
        signal_url: Some(proxy_url),
        timeout_secs: 90,
    })
    .await
    .expect("dkg through proxy");
    assert!(r.agreed, "DKG did not agree through the proxy: {:?}", r.outcomes);

    let frames = log.lock().unwrap().clone();
    frames
}

/// Recursively redact volatile wire-frame fields to stable placeholders so the
/// golden pins shape, not per-run crypto/transport values.
fn redact(v: &mut serde_json::Value) {
    use serde_json::Value;
    const VOLATILE: &[&str] = &[
        "session_id",
        "proposer_id",
        "group_public_key",
        "signing_message_hex",
        "device_id",
        "from",
        "to",
        // WebRTC transport blob (SDP / ICE candidates) — pure volatile.
        "data",
    ];
    match v {
        Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                if k == "participants" {
                    *val = Value::Array(vec![Value::String("<device>".into())]);
                } else if VOLATILE.contains(&k.as_str()) {
                    *val = Value::String(format!("<{k}>"));
                } else {
                    redact(val);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact),
        _ => {}
    }
}

/// First frame of a given direction+type, with its body normalized.
fn first_shape(frames: &[String], dir: char, ty: &str) -> Option<String> {
    for f in frames {
        let (d, body) = f.split_at(2);
        if !d.starts_with(dir) {
            continue;
        }
        let mut v: serde_json::Value = serde_json::from_str(body).ok()?;
        if v.get("type").and_then(|t| t.as_str()) == Some(ty) {
            redact(&mut v);
            return Some(serde_json::to_string(&v).unwrap());
        }
    }
    None
}

fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dkg_wire_protocol.golden.txt")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
async fn dkg_wire_protocol_matches_golden() {
    let frames = capture_dkg_frames(2, 2).await;
    assert!(!frames.is_empty(), "no wire frames captured");

    // 1. Directional type vocabulary (stable: the set of frame types the DKG
    //    protocol uses, regardless of count/order).
    let mut vocab: BTreeSet<String> = BTreeSet::new();
    for f in &frames {
        let (dir, body) = f.split_at(2);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
            if let Some(t) = v.get("type").and_then(|t| t.as_str()) {
                vocab.insert(format!("{dir}{t}"));
            }
        }
    }

    // 2. Normalized shape of the two session-discovery frames the extension
    //    must produce/parse. (relay bodies are WebRTC transport — volatile —
    //    so only their presence is pinned, via the vocabulary.)
    let announce = first_shape(&frames, 'C', "announce_session")
        .expect("an announce_session frame");
    let available = first_shape(&frames, 'S', "session_available")
        .expect("a session_available frame");

    let actual = format!(
        "# DKG wire-protocol contract (normalized)\n\n# type vocabulary\n{}\n\n# announce_session (client→server)\n{}\n\n# session_available (server→client)\n{}\n",
        vocab.into_iter().collect::<Vec<_>>().join("\n"),
        announce,
        available,
    );

    let path = golden_path();
    if std::env::var("BLESS").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        eprintln!("blessed wire golden at {}", path.display());
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing wire golden {} — generate with BLESS=1 cargo test -p mpc-wallet-cli --test wire_trace -- --ignored",
            path.display()
        )
    });
    assert_eq!(
        actual, expected,
        "DKG wire protocol drifted from golden. If intended, regenerate with \
         BLESS=1 and review the diff (this is the contract the extension must match)."
    );
}
