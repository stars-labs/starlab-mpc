//! One-shot subcommands (#24/#25): start a headless runner inline, send a
//! command, block until the correlated terminal event (or timeout), print
//! the result as JSON, and map it to an exit code. Thin wrappers over the
//! same runner + bridge that `serve` uses — for humans/scripts that want a
//! single blocking command instead of the JSONL daemon.
//!
//! Error UX: these commands are the surface investors poke at by hand, so every
//! failure path returns an **actionable** message (what failed + the most likely
//! cause + the next thing to try) rather than a bare "timed out". See
//! `connect_help` and the per-command `wait_outcome` hints.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use starlab_client::elm::headless::{spawn_ed25519, spawn_secp256k1};
use starlab_client::elm::model::{WalletConfig, WalletMode};
use starlab_client::elm::{Message, Model};

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
    /// `--json`: emit machine-readable JSON. Default is the human table/summary.
    pub json: bool,
}

/// Latest participant roster the runner has observed (from the live session),
/// captured on every model sync. Lets a timeout report "joined so far: [...]" —
/// the killer diagnostic when a device is missing or two used the same
/// `--device-id` (the roster then never reaches `total`).
#[derive(Default, Clone)]
struct RosterPeek {
    participants: Vec<String>,
    total: u16,
}

/// Actionable suffix built from the roster we saw, for timeout messages.
fn roster_hint(roster: &Arc<Mutex<RosterPeek>>) -> String {
    let r = roster.lock().unwrap().clone();
    let seen = if r.participants.is_empty() {
        "none".to_string()
    } else {
        r.participants.join(", ")
    };
    let total = if r.total > 0 { r.total.to_string() } else { "?".to_string() };
    format!(
        "\n  → joined so far: [{seen}] ({}/{total}) — if that's short, a device isn't connected \
         or two used the SAME --device-id (each needs a unique one).",
        r.participants.len()
    )
}

/// Spawn a runner whose events stream to the returned receiver; also returns a
/// live view of the participant roster for timeout diagnostics.
fn start(
    opts: &OneShotOpts,
) -> (
    UnboundedSender<Message>,
    UnboundedReceiver<CliEvent>,
    Arc<Mutex<RosterPeek>>,
) {
    let bridge = Arc::new(Mutex::new(Bridge::new()));
    let (ev_tx, ev_rx) = unbounded_channel::<CliEvent>();
    let roster = Arc::new(Mutex::new(RosterPeek::default()));
    let roster_cb = roster.clone();
    let cb = move |model: &Model, msg: Option<&Message>| {
        if let Some(s) = model.active_session.as_ref() {
            let mut r = roster_cb.lock().unwrap();
            r.participants = s.participants.clone();
            r.total = s.total;
        }
        let events = bridge.lock().unwrap().on_sync(model, msg);
        for e in events {
            let _ = ev_tx.send(e);
        }
    };
    // "unified" runs the dual-curve ceremony — spawn on secp256k1 (the generic
    // `C` DKG fields are unused on the unified path; the UnifiedDkg lives in
    // app_state.unified_dkg) and flip the model into unified mode below.
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
    if opts.curve == "unified" {
        let _ = tx.send(Message::SetUnifiedMode { unified: true });
    }
    (tx, ev_rx, roster)
}

/// Actionable message when the signal-server connection never establishes —
/// the #1 thing a hands-on user trips over (a roomless hosted connection is
/// silently rejected, so it just "hangs" until this 15s timeout).
fn connect_help(opts: &OneShotOpts, secs: u64) -> String {
    let hosted = opts.signal_url.starts_with("wss://");
    let has_room = opts.signal_url.contains("room=");
    let mut s = format!(
        "could not connect to the signal server within {secs}s ({}).",
        opts.signal_url
    );
    if hosted && !has_room {
        s.push_str(
            "\n  → No --room set. The hosted server requires a strong room (≥16 chars); a \
             roomless connection is rejected.\
             \n    Add the SAME value on every device, e.g.  --room \"$(uuidgen | tr -d -)\".",
        );
    } else {
        s.push_str(
            "\n  → Check the server is reachable and you're online. For a LAN/offline demo, run a \
             local server and use  --signal-server ws://<host-ip>:9000  (no room needed).",
        );
    }
    s.push_str(
        "\n  → To prove the whole stack with no external server:  \
         scripts/demo/ceremony.sh --nodes 3 --threshold 2 --sign hello",
    );
    s
}

/// Wait for the signal-server connection; on timeout return [`connect_help`].
/// A server-sent error frame (e.g. weak/missing room) surfaces immediately.
async fn wait_connected(rx: &mut UnboundedReceiver<CliEvent>, opts: &OneShotOpts) -> anyhow::Result<()> {
    const SECS: u64 = 15;
    if opts.signal_url.starts_with("wss://") && !opts.signal_url.contains("room=") {
        eprintln!(
            "note: no --room set — the hosted server requires a strong --room (≥16 chars); \
             if this hangs, that's why."
        );
    }
    let res = tokio::time::timeout(Duration::from_secs(SECS), async {
        loop {
            match rx.recv().await {
                Some(CliEvent::Connection { connected: true }) => return Ok(()),
                Some(CliEvent::Error { code, message, .. }) => anyhow::bail!("{code}: {message}"),
                Some(_) => continue,
                None => anyhow::bail!("the runner stopped before it could connect"),
            }
        }
    })
    .await;
    match res {
        Ok(inner) => inner,
        Err(_) => Err(anyhow::anyhow!(connect_help(opts, SECS))),
    }
}

/// Wait for a terminal outcome. On a runtime error, surface it verbatim; on
/// timeout, append `hint` (what to check) to a clear "timed out waiting for X".
async fn wait_outcome<P>(
    rx: &mut UnboundedReceiver<CliEvent>,
    secs: u64,
    waiting_for: &str,
    hint: &str,
    roster: Option<&Arc<Mutex<RosterPeek>>>,
    pred: P,
) -> anyhow::Result<CliEvent>
where
    P: Fn(&CliEvent) -> bool,
{
    let res = tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            match rx.recv().await {
                Some(e) if pred(&e) => return Ok(e),
                Some(CliEvent::Error { code, message, .. }) => anyhow::bail!("{code}: {message}"),
                Some(_) => continue,
                None => anyhow::bail!("the runner stopped before {waiting_for}"),
            }
        }
    })
    .await;
    match res {
        Ok(inner) => inner,
        Err(_) => {
            let extra = roster.map(roster_hint).unwrap_or_default();
            Err(anyhow::anyhow!(
                "timed out after {secs}s waiting for {waiting_for}.{hint}{extra}"
            ))
        }
    }
}

fn print(ev: &CliEvent, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(ev).unwrap_or_else(|_| ev.to_line())
        );
    } else {
        println!("{}", render_human(ev));
    }
}

/// kubectl-style column table: header + rows, two-space gutters, widths from
/// the longest cell per column.
fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for c in 0..cols {
            if let Some(cell) = row.get(c) {
                widths[c] = widths[c].max(cell.len());
            }
        }
    }
    let fmt_row = |cells: Vec<&str>| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if i + 1 == cols {
                    c.to_string() // last column: no trailing padding
                } else {
                    format!("{:width$}", c, width = widths[i] + 2)
                }
            })
            .collect::<String>()
    };
    let mut out = fmt_row(headers.to_vec());
    for row in rows {
        out.push('\n');
        out.push_str(&fmt_row(row.iter().map(|s| s.as_str()).collect()));
    }
    out
}

/// Human rendering for one-shot outcomes (`--json` bypasses this).
fn render_human(ev: &CliEvent) -> String {
    match ev {
        CliEvent::Wallets { wallets } => {
            if wallets.is_empty() {
                return "No wallets. Create one with `starlab-cli wallet create`.".to_string();
            }
            // One WALLET per group: first row carries id/name/quorum, the
            // wallet's remaining chain addresses continue on blank-prefixed
            // rows (one key, many chains — a unified wallet shows all four).
            let mut rows: Vec<Vec<String>> = Vec::new();
            for w in wallets {
                let name = if w.name == w.id { "-".to_string() } else { w.name.clone() };
                if w.addresses.is_empty() {
                    rows.push(vec![
                        w.id.clone(),
                        name,
                        w.threshold.clone(),
                        w.chain.clone(),
                        w.address.clone(),
                    ]);
                    continue;
                }
                for (i, a) in w.addresses.iter().enumerate() {
                    if i == 0 {
                        rows.push(vec![
                            w.id.clone(),
                            name.clone(),
                            w.threshold.clone(),
                            a.chain.clone(),
                            a.address.clone(),
                        ]);
                    } else {
                        rows.push(vec![
                            String::new(),
                            String::new(),
                            String::new(),
                            a.chain.clone(),
                            a.address.clone(),
                        ]);
                    }
                }
            }
            render_table(&["ID", "NAME", "QUORUM", "CHAIN", "ADDRESS"], &rows)
        }
        CliEvent::Sessions { sessions } => {
            if sessions.is_empty() {
                return "No active sessions.".to_string();
            }
            let rows: Vec<Vec<String>> = sessions
                .iter()
                .map(|s| {
                    vec![
                        s.session_id.clone(),
                        s.session_type.clone(),
                        format!("{}-of-{}", s.threshold, s.total),
                        s.proposer.clone(),
                        s.participants.join(","),
                    ]
                })
                .collect();
            render_table(
                &["SESSION", "TYPE", "QUORUM", "PROPOSER", "PARTICIPANTS"],
                &rows,
            )
        }
        CliEvent::DkgComplete {
            wallet_id,
            address,
            group_public_key,
            ..
        } => format!(
            "✔ Wallet created\n  Wallet ID:  {wallet_id}\n  Address:    {address}\n  Group key:  {group_public_key}"
        ),
        CliEvent::SignatureComplete {
            signature,
            message_hash,
            ..
        } => format!(
            "✔ Signature complete\n  Message hash:  {message_hash}\n  Signature:     {signature}"
        ),
        CliEvent::ReshareComplete {
            wallet_id,
            group_public_key,
            ..
        } => format!(
            "✔ Reshare complete — group key (and address) unchanged\n  Wallet ID:  {wallet_id}\n  Group key:  {group_public_key}"
        ),
        CliEvent::Error { code, message, .. } => format!("✖ {code}: {message}"),
        CliEvent::Ready {
            protocol,
            device_id,
            curve,
        } => format!("ready (protocol v{protocol}, device {device_id}, curve {curve})"),
        CliEvent::Ack { correlates } => format!("ack #{correlates}"),
        CliEvent::Connection { connected } => if *connected {
            "✔ connected to the signal server".to_string()
        } else {
            "✖ disconnected from the signal server".to_string()
        },
        CliEvent::Status {
            connected,
            device_id,
            wallets,
        } => {
            let mut out = format!(
                "Device:     {device_id}\nConnection: {}",
                if *connected { "connected" } else { "disconnected" }
            );
            out.push_str("\n\n");
            out.push_str(&render_human(&CliEvent::Wallets {
                wallets: wallets.clone(),
            }));
            out
        }
        CliEvent::SessionAnnounced { session_id, .. } => {
            format!("✔ Session announced\n  Session ID:  {session_id}")
        }
        CliEvent::SessionAvailable { session } => render_table(
            &["SESSION", "TYPE", "QUORUM", "PROPOSER", "PARTICIPANTS"],
            &[vec![
                session.session_id.clone(),
                session.session_type.clone(),
                format!("{}-of-{}", session.threshold, session.total),
                session.proposer.clone(),
                session.participants.join(","),
            ]],
        ),
        CliEvent::DkgProgress {
            session_id,
            round,
            received,
            need,
        } => format!("… DKG round {round}: {received}/{need} packages (session {session_id})"),
        CliEvent::SigningRequest {
            session_id,
            wallet,
            threshold,
            total,
            proposer,
        } => format!(
            "⧖ Signing request from {proposer}\n  Wallet:    {wallet}\n  Quorum:    {threshold}-of-{total}\n  Session:   {session_id}\n  → approve with: starlab-cli session join --session-id {session_id} …"
        ),
        CliEvent::ReshareRequest {
            session_id,
            wallet,
            threshold,
            total,
            proposer,
        } => format!(
            "⧖ Reshare request from {proposer}\n  Wallet:    {wallet}\n  Quorum:    {threshold}-of-{total}\n  Session:   {session_id}\n  → approve with: starlab-cli session join --session-id {session_id} …"
        ),
    }
}

/// `wallet list` — read the keystore (no network).
pub async fn wallet_list(opts: OneShotOpts) -> anyhow::Result<bool> {
    let (tx, mut rx, _roster) = start(&opts);
    tx.send(Message::ListWallets)?;
    let ev = wait_outcome(
        &mut rx,
        5,
        "the wallet list",
        &format!("\n  → Check the --keystore path is readable ({}).", opts.keystore_path),
        None,
        |e| matches!(e, CliEvent::Wallets { .. }),
    )
    .await?;
    print(&ev, opts.json);
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
    if total < 2 {
        anyhow::bail!(
            "--total must be ≥ 2 (got {total}). A shared wallet needs at least two devices; \
             the classic demo is --total 3 --threshold 2 (2-of-3)."
        );
    }
    if threshold < 1 || threshold > total {
        anyhow::bail!(
            "--threshold must be between 1 and --total ({total}); got {threshold}. \
             Tip: 2-of-3 = --threshold 2 --total 3."
        );
    }
    let (tx, mut rx, roster) = start(&opts);
    tx.send(Message::TriggerReconnect)?;
    wait_connected(&mut rx, &opts).await?;
    eprintln!(
        "note: announcing a {threshold}-of-{total} DKG as device '{}'. Waiting up to {}s for the \
         other {} participant(s) to join (same --room, a UNIQUE --device-id each)…",
        opts.device_id,
        opts.timeout_secs,
        total - 1
    );
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
    // Surface the announced session id + the exact command the OTHER devices run
    // to join. DKG only completes once all `total` participants join, so a lone
    // `wallet create` sits and waits — this tells the operator precisely what to
    // do next instead of leaving them guessing (the reported "how to start").
    if let Ok(CliEvent::SessionAnnounced { session_id, .. }) = wait_outcome(
        &mut rx,
        20,
        "the session announcement",
        "",
        None,
        |e| matches!(e, CliEvent::SessionAnnounced { .. }),
    )
    .await
    {
        let room = opts.signal_url.split("room=").nth(1).unwrap_or("");
        let room_flag = if room.is_empty() {
            String::new()
        } else {
            format!(" --room {room}")
        };
        eprintln!(
            "note: session id = {session_id}\n  → on each of the other {} device(s), run (unique \
             --device-id, SAME password):\n      starlab-cli session join --session-id \
             {session_id}{room_flag} --device-id <unique>",
            total - 1
        );
    }
    let ev = wait_outcome(
        &mut rx,
        opts.timeout_secs,
        "the DKG to complete",
        "\n  → DKG needs ALL participants online together. On each OTHER device run \
         `starlab-cli session join --session-id <id shown above>` with the SAME --room and a \
         unique --device-id.",
        Some(&roster),
        |e| matches!(e, CliEvent::DkgComplete { .. }),
    )
    .await?;
    print(&ev, opts.json);
    Ok(true)
}

/// `session join` — join a discovered DKG/signing session by id.
pub async fn session_join(
    opts: OneShotOpts,
    session_id: String,
    password: String,
) -> anyhow::Result<bool> {
    let (tx, mut rx, roster) = start(&opts);
    tx.send(Message::TriggerReconnect)?;
    wait_connected(&mut rx, &opts).await?;
    // Cold-start discovery + join. The creator almost always announces the
    // session BEFORE we connect, and `announce_session` is a one-shot broadcast,
    // so we must ask the server to replay active sessions
    // (`HeadlessRefreshSessions` → `RequestActiveSessions`) — otherwise we never
    // discover the session and silently time out (the bug investors hit). We
    // retry refresh→join on a short cadence to beat the announce/connect race in
    // EITHER order; `HeadlessJoinSession` is idempotent once joined, so the
    // repeats are harmless no-ops after the mesh forms.
    {
        let tx_join = tx.clone();
        let sid = session_id.clone();
        let pw = password.clone();
        let attempts = (opts.timeout_secs / 6).clamp(5, 12);
        tokio::spawn(async move {
            for _ in 0..attempts {
                let _ = tx_join.send(Message::HeadlessRefreshSessions);
                tokio::time::sleep(Duration::from_millis(700)).await;
                let _ = tx_join.send(Message::HeadlessJoinSession {
                    session_id: sid.clone(),
                    password: pw.clone(),
                    label: String::new(),
                });
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    }
    let ev = wait_outcome(
        &mut rx,
        opts.timeout_secs,
        "the session to complete",
        "\n  → Is the session id correct and the creator still online in the SAME --room? \
         Everyone must share --room and --signal-server; the password must match this wallet.",
        Some(&roster),
        |e| {
            matches!(
                e,
                CliEvent::DkgComplete { .. }
                    | CliEvent::SignatureComplete { .. }
                    | CliEvent::ReshareComplete { .. }
            )
        },
    )
    .await?;
    print(&ev, opts.json);
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
    let (tx, mut rx, roster) = start(&opts);
    tx.send(Message::TriggerReconnect)?;
    wait_connected(&mut rx, &opts).await?;
    tx.send(Message::HeadlessReshare {
        wallet_id,
        password,
        keystore_path: opts.keystore_path.clone(),
    })?;
    let ev = wait_outcome(
        &mut rx,
        opts.timeout_secs,
        "the reshare to complete",
        "\n  → Reshare needs the retained signers to join the announced session in the SAME \
         --room (via `session join`, or `serve --auto-approve`). Check --wallet-id exists \
         (`wallet list`) and the password matches.",
        Some(&roster),
        |e| matches!(e, CliEvent::ReshareComplete { .. }),
    )
    .await?;
    print(&ev, opts.json);
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
    let (tx, mut rx, roster) = start(&opts);
    tx.send(Message::TriggerReconnect)?;
    wait_connected(&mut rx, &opts).await?;
    tx.send(Message::HeadlessSign {
        wallet_id,
        message,
        encoding,
        password,
    })?;
    let ev = wait_outcome(
        &mut rx,
        opts.timeout_secs,
        "the signature",
        "\n  → Signing needs a quorum to approve. Did a co-signer run `session join` (or \
         `serve --auto-approve`) on the announced session in the SAME --room? Check --wallet-id \
         exists (`wallet list`) and the password matches.",
        Some(&roster),
        |e| matches!(e, CliEvent::SignatureComplete { .. }),
    )
    .await?;
    print(&ev, opts.json);
    Ok(true)
}

#[cfg(test)]
mod output_tests {
    use super::*;
    use crate::protocol::WalletEntry;

    #[test]
    fn table_aligns_columns_kubectl_style() {
        let t = render_table(
            &["ID", "CHAIN"],
            &[
                vec!["short".into(), "Ethereum".into()],
                vec!["a-much-longer-id".into(), "Solana".into()],
            ],
        );
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines.len(), 3);
        // CHAIN starts at the same byte offset on every line.
        let off = lines[0].find("CHAIN").unwrap();
        assert_eq!(&lines[1][off..off + 8], "Ethereum");
        assert_eq!(&lines[2][off..off + 6], "Solana");
        // last column has no trailing padding
        assert!(!lines[1].ends_with(' '));
    }

    #[test]
    fn wallets_render_as_table_and_empty_has_a_hint() {
        let ev = CliEvent::Wallets {
            wallets: vec![WalletEntry {
                id: "abc".into(),
                name: "W".into(),
                address: "0x1".into(),
                chain: "Ethereum".into(),
                threshold: "2/3".into(),
                curves: vec![],
                addresses: vec![],
            }],
        };
        let out = render_human(&ev);
        assert!(out.starts_with("ID  "));
        assert!(out.contains("Ethereum"));

        let empty = render_human(&CliEvent::Wallets { wallets: vec![] });
        assert!(empty.contains("wallet create"));
    }

    #[test]
    fn outcome_summaries_are_human() {
        let dkg = render_human(&CliEvent::DkgComplete {
            correlates: None,
            wallet_id: "w1".into(),
            address: "0xabc".into(),
            group_public_key: "02ff".into(),
        });
        assert!(dkg.contains("✔ Wallet created") && dkg.contains("0xabc"));

        let err = render_human(&CliEvent::Error {
            correlates: None,
            code: "timeout".into(),
            message: "no quorum".into(),
        });
        assert_eq!(err, "✖ timeout: no quorum");
    }

    #[test]
    fn status_renders_device_connection_and_wallet_table() {
        let out = render_human(&CliEvent::Status {
            connected: true,
            device_id: "mpc-1".into(),
            wallets: vec![WalletEntry {
                id: "abc".into(),
                name: "W".into(),
                address: "0x1".into(),
                chain: "Ethereum".into(),
                threshold: "2/3".into(),
                curves: vec![],
                addresses: vec![],
            }],
        });
        assert!(out.contains("Device:     mpc-1"));
        assert!(out.contains("Connection: connected"));
        assert!(out.contains("ID  ")); // embedded wallet table
    }

    #[test]
    fn requests_point_at_the_join_command() {
        let out = render_human(&CliEvent::SigningRequest {
            session_id: "s-1".into(),
            wallet: "w".into(),
            threshold: 2,
            total: 3,
            proposer: "mpc-2".into(),
        });
        assert!(out.contains("session join --session-id s-1"));
        assert!(out.contains("2-of-3"));
    }
}
