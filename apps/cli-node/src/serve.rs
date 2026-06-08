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
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tui_node::elm::headless::{spawn_ed25519, spawn_secp256k1};
use tui_node::elm::model::{WalletConfig, WalletMode};
use tui_node::elm::{Message, Model};

use crate::bridge::{Bridge, Snapshot};
use crate::policy::AutoApprovePolicy;
use crate::protocol::{CliCommand, CliEvent, CliRequest, PROTOCOL_VERSION};

/// Runtime configuration for `serve`.
pub struct ServeOpts {
    pub device_id: String,
    pub keystore_path: String,
    pub signal_url: String,
    /// "secp256k1" (default) or "ed25519" — the runner ciphersuite, fixed at
    /// spawn. ed25519 produces standard RFC-8032 signatures.
    pub curve: String,
    /// Auto-approval policy for incoming signing requests (disabled unless
    /// the operator opts in). Shared so the runner callback can consume it.
    pub auto_approve: std::sync::Arc<AutoApprovePolicy>,
    /// Password used to unlock the wallet when auto-approving. Ignored unless
    /// `auto_approve` is enabled.
    pub approve_password: String,
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
    let pending_reshare: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));

    // --- runner sync closure: model/message → events + snapshot cache ---
    let out_for_sync = out_tx.clone();
    let bridge_for_sync = bridge.clone();
    let snapshot_for_sync = snapshot.clone();
    let pending_for_sync = pending_create.clone();
    let pending_sign_for_sync = pending_sign.clone();
    let pending_reshare_for_sync = pending_reshare.clone();
    // Auto-approve plumbing: the callback can't capture the runner sender
    // (chicken-and-egg with spawn), so route it through a OnceLock set right
    // after spawn returns.
    let approve_sender: Arc<std::sync::OnceLock<UnboundedSender<Message>>> =
        Arc::new(std::sync::OnceLock::new());
    let approve_sender_cb = approve_sender.clone();
    let policy = opts.auto_approve.clone();
    let approve_pw = opts.approve_password.clone();
    let cb = move |model: &Model, msg: Option<&Message>| {
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
                    // Announcement is mid-ceremony: correlate with the create
                    // command but DON'T consume the id — DkgComplete still
                    // needs it to close the loop.
                    CliEvent::SessionAnnounced { correlates, .. } if correlates.is_none() => {
                        *correlates = *pending_for_sync.lock().unwrap();
                    }
                    CliEvent::SignatureComplete { correlates, .. } if correlates.is_none() => {
                        *correlates = pending_sign_for_sync.lock().unwrap().take();
                    }
                    CliEvent::ReshareComplete { correlates, .. } if correlates.is_none() => {
                        *correlates = pending_reshare_for_sync.lock().unwrap().take();
                    }
                    // Policy-gated auto-approval: join the signing session to
                    // contribute our share, only if the operator opted in AND
                    // the request passes the policy (allowlist + budget).
                    CliEvent::SigningRequest { session_id, wallet, .. } => {
                        if policy.try_approve(wallet) {
                            if let Some(tx) = approve_sender_cb.get() {
                                let _ = tx.send(Message::HeadlessJoinSession {
                                    session_id: session_id.clone(),
                                    password: approve_pw.clone(),
                                    label: String::new(),
                                });
                                let _ = out_for_sync.send(CliEvent::Error {
                                    correlates: None,
                                    code: "auto_approved".into(),
                                    message: format!(
                                        "auto-approved signing request for {wallet} ({session_id})"
                                    ),
                                });
                            }
                        }
                    }
                    // Reshare is approved the same way a co-signer approves
                    // signing — by joining the session to contribute a refreshed
                    // share. Gate on the same policy (allowlist + budget).
                    CliEvent::ReshareRequest { session_id, wallet, .. } => {
                        if policy.try_approve(wallet) {
                            if let Some(tx) = approve_sender_cb.get() {
                                let _ = tx.send(Message::HeadlessJoinSession {
                                    session_id: session_id.clone(),
                                    password: approve_pw.clone(),
                                    label: String::new(),
                                });
                                let _ = out_for_sync.send(CliEvent::Error {
                                    correlates: None,
                                    code: "auto_approved".into(),
                                    message: format!(
                                        "auto-approved reshare request for {wallet} ({session_id})"
                                    ),
                                });
                            }
                        }
                    }
                    _ => {}
                }
                let _ = out_for_sync.send(ev);
            }
        }
    ;
    let runner_tx = if opts.curve == "ed25519" {
        spawn_ed25519(
            opts.device_id.clone(),
            opts.keystore_path.clone(),
            opts.signal_url.clone(),
            cb,
        )
    } else {
        spawn_secp256k1(
            opts.device_id.clone(),
            opts.keystore_path.clone(),
            opts.signal_url.clone(),
            cb,
        )
    };
    let _ = approve_sender.set(runner_tx.clone());

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
                    message: format!(
                        "invalid JSONL command: {e}. Expected one JSON object per line, e.g. \
                         {{\"cmd\":\"create_wallet\",\"threshold\":2,\"total\":3,\"password\":\"…\"}}. \
                         Run `mpc-wallet-cli schema` for the full command list."
                    ),
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
                // A roomless connection to the hosted (wss://) server is rejected
                // by the multi-tenant signal server, so the connection will never
                // come up — tell the operator why, in-band, instead of letting
                // every later command silently hang.
                if opts.signal_url.starts_with("wss://") && !opts.signal_url.contains("room=") {
                    let _ = out_tx.send(CliEvent::Error {
                        correlates: id,
                        code: "room_required".into(),
                        message: format!(
                            "no room set for the hosted server ({}). It requires a strong room \
                             (≥16 chars) — a roomless connection is rejected. Restart `serve` with \
                             --room <id> (the SAME value on every node).",
                            opts.signal_url
                        ),
                    });
                }
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
                // Fire a server replay so sessions announced before we
                // connected are (re)discovered; they stream back as
                // `session_available` events as the replies arrive. Also
                // answer immediately from the cache for what we already know.
                let _ = runner_tx.send(Message::HeadlessRefreshSessions);
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
            CliCommand::Reshare {
                wallet_id,
                password,
            } => {
                // Initiator: load the OLD share + announce a reshare session.
                // Retained signers approve with `join_session`. Correlate the
                // eventual `reshare_complete` with this command id.
                *pending_reshare.lock().unwrap() = id;
                let _ = runner_tx.send(Message::HeadlessReshare {
                    wallet_id,
                    password,
                    keystore_path: opts.keystore_path.clone(),
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
