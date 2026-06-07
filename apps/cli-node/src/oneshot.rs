//! One-shot subcommands (#24/#25): start a headless runner inline, send a
//! command, block until the correlated terminal event (or timeout), print
//! the result as JSON, and map it to an exit code. Thin wrappers over the
//! same runner + bridge that `serve` uses — for humans/scripts that want a
//! single blocking command instead of the JSONL daemon.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tui_node::elm::headless::{spawn_ed25519, spawn_secp256k1};
use tui_node::elm::model::{WalletConfig, WalletMode};
use tui_node::elm::{Message, Model};

use crate::bridge::Bridge;
use crate::protocol::CliEvent;

/// Shared configuration for every one-shot command.
pub struct OneShotOpts {
    pub device_id: String,
    pub keystore_path: String,
    pub signal_url: String,
    pub timeout_secs: u64,
    /// Ciphersuite: "secp256k1" (default) or "ed25519". ed25519 produces a
    /// standard RFC-8032 signature that any off-the-shelf verifier (and Solana)
    /// can check — the runner ciphersuite is fixed at spawn.
    pub curve: String,
}

/// Spawn a runner whose events stream to the returned receiver.
fn start(opts: &OneShotOpts) -> (UnboundedSender<Message>, UnboundedReceiver<CliEvent>) {
    let bridge = Arc::new(Mutex::new(Bridge::new()));
    let (ev_tx, ev_rx) = unbounded_channel::<CliEvent>();
    let cb = move |model: &Model, msg: Option<&Message>| {
        let events = bridge.lock().unwrap().on_sync(model, msg);
        for e in events {
            let _ = ev_tx.send(e);
        }
    };
    let tx = if opts.curve == "ed25519" {
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
    (tx, ev_rx)
}

async fn wait_event<P>(
    rx: &mut UnboundedReceiver<CliEvent>,
    secs: u64,
    pred: P,
) -> anyhow::Result<CliEvent>
where
    P: Fn(&CliEvent) -> bool,
{
    tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            match rx.recv().await {
                Some(e) if pred(&e) => return Ok(e),
                // Surface a server/runtime error promptly instead of waiting.
                Some(CliEvent::Error { code, message, .. }) => {
                    anyhow::bail!("{code}: {message}")
                }
                Some(_) => continue,
                None => anyhow::bail!("runner stopped before the expected event"),
            }
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out after {secs}s"))?
}

fn print(ev: &CliEvent) {
    println!(
        "{}",
        serde_json::to_string_pretty(ev).unwrap_or_else(|_| ev.to_line())
    );
}

/// `wallet list` — read the keystore (no network).
pub async fn wallet_list(opts: OneShotOpts) -> anyhow::Result<bool> {
    let (tx, mut rx) = start(&opts);
    tx.send(Message::ListWallets)?;
    let ev = wait_event(&mut rx, 5, |e| matches!(e, CliEvent::Wallets { .. })).await?;
    print(&ev);
    Ok(true)
}

/// `wallet create` — announce a DKG and block until it completes.
pub async fn wallet_create(
    opts: OneShotOpts,
    name: String,
    threshold: u16,
    total: u16,
    password: String,
) -> anyhow::Result<bool> {
    let (tx, mut rx) = start(&opts);
    tx.send(Message::TriggerReconnect)?;
    wait_event(&mut rx, 15, |e| {
        matches!(e, CliEvent::Connection { connected: true })
    })
    .await?;
    tx.send(Message::HeadlessCreateWallet {
        config: WalletConfig {
            name: name.clone(),
            total_participants: total,
            threshold,
            mode: WalletMode::Online,
        },
        password,
        label: name,
    })?;
    let ev = wait_event(&mut rx, opts.timeout_secs, |e| {
        matches!(e, CliEvent::DkgComplete { .. })
    })
    .await?;
    print(&ev);
    Ok(true)
}

/// `session join` — join a discovered DKG/signing session by id.
pub async fn session_join(
    opts: OneShotOpts,
    session_id: String,
    password: String,
) -> anyhow::Result<bool> {
    let (tx, mut rx) = start(&opts);
    tx.send(Message::TriggerReconnect)?;
    wait_event(&mut rx, 15, |e| {
        matches!(e, CliEvent::Connection { connected: true })
    })
    .await?;
    // Give the server a moment to replay the session, then join.
    tokio::time::sleep(Duration::from_secs(3)).await;
    tx.send(Message::HeadlessJoinSession {
        session_id,
        password,
        label: String::new(),
    })?;
    let ev = wait_event(&mut rx, opts.timeout_secs, |e| {
        matches!(
            e,
            CliEvent::DkgComplete { .. }
                | CliEvent::SignatureComplete { .. }
                | CliEvent::ReshareComplete { .. }
        )
    })
    .await?;
    print(&ev);
    Ok(true)
}

/// `reshare` — initiate a share refresh/resharing of an existing wallet and
/// block until it completes. The group public key (address) is preserved; the
/// refreshed share replaces the old one on disk. Retained co-signers approve by
/// running `session join` (or `serve`) on the announced reshare session.
pub async fn reshare(
    opts: OneShotOpts,
    wallet_id: String,
    password: String,
) -> anyhow::Result<bool> {
    let (tx, mut rx) = start(&opts);
    tx.send(Message::TriggerReconnect)?;
    wait_event(&mut rx, 15, |e| {
        matches!(e, CliEvent::Connection { connected: true })
    })
    .await?;
    tx.send(Message::HeadlessReshare {
        wallet_id,
        password,
        keystore_path: opts.keystore_path.clone(),
    })?;
    let ev = wait_event(&mut rx, opts.timeout_secs, |e| {
        matches!(e, CliEvent::ReshareComplete { .. })
    })
    .await?;
    print(&ev);
    Ok(true)
}

/// `sign` — initiate a threshold signing and block until it completes.
pub async fn sign(
    opts: OneShotOpts,
    wallet_id: String,
    message: String,
    encoding: String,
    password: String,
) -> anyhow::Result<bool> {
    let (tx, mut rx) = start(&opts);
    tx.send(Message::TriggerReconnect)?;
    wait_event(&mut rx, 15, |e| {
        matches!(e, CliEvent::Connection { connected: true })
    })
    .await?;
    tx.send(Message::HeadlessSign {
        wallet_id,
        message,
        encoding,
        password,
    })?;
    let ev = wait_event(&mut rx, opts.timeout_secs, |e| {
        matches!(e, CliEvent::SignatureComplete { .. })
    })
    .await?;
    print(&ev);
    Ok(true)
}
