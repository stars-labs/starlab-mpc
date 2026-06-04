//! The `serve` daemon: stdin JSONL commands ↔ `HeadlessRunner` ↔ stdout
//! JSONL events. This is the LLM/agent + test-harness control surface.
//!
//! Threading:
//!  - a dedicated **writer task** owns stdout; everyone sends `CliEvent`s
//!    to it over a channel so lines never interleave;
//!  - the **runner sync closure** turns model/message updates into events
//!    (via `Bridge`) and caches a `Snapshot`;
//!  - the **input task** (this fn) reads stdin, injects `Message`s into the
//!    runner, and answers snapshot queries from the cached state.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::unbounded_channel;
use tui_node::elm::headless::spawn_secp256k1;
use tui_node::elm::model::{WalletConfig, WalletMode};
use tui_node::elm::{Message, Model};

use crate::bridge::{Bridge, Snapshot};
use crate::protocol::{CliCommand, CliEvent, CliRequest, PROTOCOL_VERSION};

/// Runtime configuration for `serve`.
pub struct ServeOpts {
    pub device_id: String,
    pub keystore_path: String,
    pub signal_url: String,
    /// P1 supports secp256k1 only (the runner ciphersuite is fixed at
    /// spawn). The field is surfaced now so the contract is stable.
    pub curve: String,
}

pub async fn serve(opts: ServeOpts) -> anyhow::Result<()> {
    // --- stdout writer task: single owner of stdout ---
    let (out_tx, mut out_rx) = unbounded_channel::<CliEvent>();
    let writer = tokio::spawn(async move {
        use std::io::Write;
        let stdout = std::io::stdout();
        while let Some(ev) = out_rx.recv().await {
            let mut lock = stdout.lock();
            let _ = writeln!(lock, "{}", ev.to_line());
            let _ = lock.flush();
        }
    });

    // Greeting line first so clients can sync on it.
    let _ = out_tx.send(CliEvent::Ready {
        protocol: PROTOCOL_VERSION,
        device_id: opts.device_id.clone(),
        curve: opts.curve.clone(),
    });

    // --- shared state between the runner closure and the input loop ---
    let bridge = Arc::new(Mutex::new(Bridge::new()));
    let snapshot = Arc::new(Mutex::new(Snapshot {
        device_id: opts.device_id.clone(),
        ..Snapshot::default()
    }));
    // Correlate the next terminal event with the command that started it.
    let pending_create: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));
    let pending_sign: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));

    // --- runner sync closure: model/message → events + snapshot cache ---
    let out_for_sync = out_tx.clone();
    let bridge_for_sync = bridge.clone();
    let snapshot_for_sync = snapshot.clone();
    let pending_for_sync = pending_create.clone();
    let pending_sign_for_sync = pending_sign.clone();
    let runner_tx = spawn_secp256k1(
        opts.device_id.clone(),
        opts.keystore_path.clone(),
        opts.signal_url.clone(),
        move |model: &Model, msg: Option<&Message>| {
            let mut b = bridge_for_sync.lock().unwrap();
            let events = b.on_sync(model, msg);
            *snapshot_for_sync.lock().unwrap() = b.snapshot(model);
            drop(b);
            for mut ev in events {
                // Stamp the originating command id onto terminal events.
                match &mut ev {
                    CliEvent::DkgComplete { correlates, .. } if correlates.is_none() => {
                        *correlates = pending_for_sync.lock().unwrap().take();
                    }
                    CliEvent::SignatureComplete { correlates, .. } if correlates.is_none() => {
                        *correlates = pending_sign_for_sync.lock().unwrap().take();
                    }
                    _ => {}
                }
                let _ = out_for_sync.send(ev);
            }
        },
    );

    // --- input loop: stdin JSONL → runner / snapshot replies ---
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let req: CliRequest = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                let _ = out_tx.send(CliEvent::Error {
                    correlates: None,
                    code: "bad_request".into(),
                    message: e.to_string(),
                });
                continue;
            }
        };
        let id = req.id;
        let ack = |id: Option<u64>| {
            if let Some(id) = id {
                let _ = out_tx.send(CliEvent::Ack { correlates: id });
            }
        };

        match req.command {
            CliCommand::Connect => {
                let _ = runner_tx.send(Message::TriggerReconnect);
                ack(id);
            }
            CliCommand::Disconnect => {
                // No direct disconnect message in the core yet; surface it
                // as a no-op ack rather than silently dropping.
                ack(id);
            }
            CliCommand::Status => {
                let s = snapshot.lock().unwrap().clone();
                let _ = out_tx.send(CliEvent::Status {
                    connected: s.connected,
                    device_id: s.device_id,
                    wallets: s.wallets,
                });
            }
            CliCommand::ListWallets => {
                // Refresh from disk, then answer from the cache.
                let _ = runner_tx.send(Message::ListWallets);
                let s = snapshot.lock().unwrap().clone();
                let _ = out_tx.send(CliEvent::Wallets { wallets: s.wallets });
            }
            CliCommand::ListSessions => {
                let s = snapshot.lock().unwrap().clone();
                let _ = out_tx.send(CliEvent::Sessions { sessions: s.sessions });
            }
            CliCommand::CreateWallet {
                name,
                threshold,
                total,
                curve: _curve,
                password,
            } => {
                // P1: ciphersuite is secp256k1 (runner-fixed); `curve` is
                // accepted but not yet used to pick ed25519.
                *pending_create.lock().unwrap() = id;
                let label = name.trim().to_string();
                let config = WalletConfig {
                    name: if label.is_empty() {
                        "Wallet".to_string()
                    } else {
                        label.clone()
                    },
                    total_participants: total,
                    threshold,
                    mode: WalletMode::Online,
                };
                let _ = runner_tx.send(Message::HeadlessCreateWallet {
                    config,
                    password,
                    label,
                });
                ack(id);
            }
            CliCommand::JoinSession {
                session_id,
                password,
                label,
            } => {
                let _ = runner_tx.send(Message::HeadlessJoinSession {
                    session_id,
                    password,
                    label,
                });
                ack(id);
            }
            CliCommand::Sign {
                wallet_id,
                message,
                encoding,
                password,
            } => {
                *pending_sign.lock().unwrap() = id;
                let _ = runner_tx.send(Message::HeadlessSign {
                    wallet_id,
                    message,
                    encoding,
                    password,
                });
                ack(id);
            }
            CliCommand::ApproveSigning {
                session_id,
                password,
            } => {
                // A co-signer approves by joining the signing session.
                let _ = runner_tx.send(Message::HeadlessJoinSession {
                    session_id,
                    password,
                    label: String::new(),
                });
                ack(id);
            }
            CliCommand::Quit => {
                let _ = runner_tx.send(Message::Quit);
                break;
            }
        }
    }

    // stdin closed or `quit` — stop the runner and drain the writer.
    let _ = runner_tx.send(Message::Quit);
    drop(out_tx);
    let _ = writer.await;
    Ok(())
}
