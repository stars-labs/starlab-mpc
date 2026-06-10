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
        CliEvent::Accounts { wallet_id, accounts } => {
            let mut rows: Vec<Vec<String>> = Vec::new();
            for a in accounts {
                for (i, addr) in a.addresses.iter().enumerate() {
                    rows.push(vec![
                        if i == 0 { a.account.to_string() } else { String::new() },
                        addr.chain.clone(),
                        addr.address.clone(),
                    ]);
                }
            }
            format!(
                "Accounts for {wallet_id} (standard paths; derive --account <i> --save to sign)\n\n{}",
                render_table(&["ACCOUNT", "CHAIN", "ADDRESS"], &rows)
            )
        }
        CliEvent::DerivedAddresses {
            wallet_id,
            path,
            child_id,
            addresses,
            saved,
        } => {
            let rows: Vec<Vec<String>> = addresses
                .iter()
                .map(|a| vec![a.chain.clone(), a.address.clone()])
                .collect();
            let mut out = format!(
                "✔ Derived {path} from {wallet_id}\n  Child wallet:  {child_id}{}\n\n",
                if *saved { "  (saved — co-signers must run the same derive --save)" } else { "  (preview — re-run with --save to persist)" }
            );
            out.push_str(&render_table(&["CHAIN", "ADDRESS"], &rows));
            out
        }
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

/// The PINNED standard derivation path per chain (BIP-44/84 coin types).
/// These are fixed so users only ever think in account indexes — exactly the
/// BIP-44 contract. Returns None for unknown chains.
pub fn standard_path(chain: &str, account: u32) -> Option<String> {
    match chain.to_ascii_lowercase().as_str() {
        "ethereum" | "eth" => Some(format!("m/44'/60'/0'/0/{account}")),
        "bitcoin" | "btc" => Some(format!("m/84'/0'/0'/0/{account}")),
        "solana" | "sol" => Some(format!("m/44'/501'/{account}'/0'")),
        "sui" => Some(format!("m/44'/784'/{account}'/0'/0'")),
        _ => None,
    }
}

/// `wallet accounts` — list account 0..count with per-chain addresses from
/// the pinned standard paths. PUBLIC derivation only (the chain code comes
/// from the group key): no password, no network, safe to run anywhere.
pub async fn wallet_accounts(
    opts: OneShotOpts,
    wallet_id: String,
    count: u32,
) -> anyhow::Result<bool> {
    use starlab_client::keystore::Keystore;

    let ks = Keystore::new(&opts.keystore_path, &opts.device_id)
        .map_err(|e| anyhow::anyhow!("open keystore {}: {e}", opts.keystore_path))?;
    let metas: Vec<_> = ks
        .list_wallets()
        .into_iter()
        .filter(|w| w.session_id == wallet_id)
        .cloned()
        .collect();
    if metas.is_empty() {
        anyhow::bail!(
            "wallet '{wallet_id}' not found in {} (device {}).",
            opts.keystore_path,
            opts.device_id
        );
    }

    // (chain display, chain key, curve, group key bytes) for each chain the
    // wallet's curves control.
    let mut chains: Vec<(String, String, String, Vec<u8>)> = Vec::new();
    for meta in &metas {
        let group = hex::decode(&meta.group_public_key)
            .map_err(|e| anyhow::anyhow!("bad group key hex: {e}"))?;
        for (key, display) in crate::bridge::chains_for_curve(&meta.curve_type) {
            chains.push((
                (*display).to_string(),
                (*key).to_string(),
                meta.curve_type.clone(),
                group.clone(),
            ));
        }
    }

    let mut accounts: Vec<crate::protocol::AccountEntry> = Vec::new();
    for i in 0..count {
        let mut addresses = Vec::new();
        for (display, key, curve, group) in &chains {
            let path_s = standard_path(key, i)
                .ok_or_else(|| anyhow::anyhow!("no standard path for {key}"))?;
            let path = starlab_core::DerivationPath::parse(&path_s)
                .map_err(|e| anyhow::anyhow!("parse {path_s}: {e}"))?;
            let child_pub = match curve.as_str() {
                "ed25519" => starlab_core::derive_child_verifying_key_path::<
                    frost_ed25519::Ed25519Sha512,
                >(group, &path),
                _ => starlab_core::derive_child_verifying_key_path::<
                    frost_secp256k1::Secp256K1Sha256,
                >(group, &path),
            }
            .map_err(|e| anyhow::anyhow!("derive {path_s}: {e}"))?;
            let address = crate::bridge::derive_address(&hex::encode(&child_pub), curve, key);
            if !address.is_empty() {
                addresses.push(crate::protocol::ChainAddress {
                    chain: display.clone(),
                    address,
                });
            }
        }
        accounts.push(crate::protocol::AccountEntry { account: i, addresses });
    }

    print(
        &CliEvent::Accounts { wallet_id, accounts },
        opts.json,
    );
    Ok(true)
}

/// Deterministic child wallet id: every participant deriving the same
/// (parent, path) must land on the same id so co-signing "just works".
fn child_wallet_id(parent: &str, path: &str) -> String {
    let sanitized: String = path
        .trim_start_matches("m/")
        .replace('\'', "h")
        .replace('/', "-");
    format!("{parent}-{sanitized}")
}

/// Derive a child (key share + group key) for one curve from a decrypted
/// keystore blob. Returns (child blob, child group public key hex).
fn derive_child_for_curve<C: frost_core::Ciphersuite>(
    blob: &[u8],
    path: &starlab_core::DerivationPath,
) -> anyhow::Result<(Vec<u8>, String)> {
    use starlab_client::elm::command::{decode_keystore_blob, encode_keystore_blob};
    let (kp, pp) = decode_keystore_blob::<C>(blob)
        .map_err(|e| anyhow::anyhow!("decode key share: {e}"))?;
    let group = pp
        .verifying_key()
        .serialize()
        .map_err(|e| anyhow::anyhow!("serialize group key: {e}"))?;
    let chain_code = starlab_core::ChainCode::from_group_key(group.as_ref());
    let derived = starlab_core::derive_child_key_path::<C>(&kp, &pp, &chain_code, path)
        .map_err(|e| anyhow::anyhow!("derive: {e}"))?;
    let child_group = derived
        .public_key_package
        .verifying_key()
        .serialize()
        .map_err(|e| anyhow::anyhow!("serialize child group key: {e}"))?;
    let child_blob = encode_keystore_blob::<C>(&derived.key_package, &derived.public_key_package)
        .map_err(|e| anyhow::anyhow!("encode child share: {e}"))?;
    Ok((child_blob, hex::encode(child_group)))
}

/// `wallet derive` — BIP-44-style HD child derivation. Fully offline: reads
/// the encrypted share(s), derives per curve, optionally persists the child.
/// Deterministic across participants (the chain code comes from the group
/// key), so a threshold of co-signers deriving the SAME path can sign for
/// the child address.
pub async fn wallet_derive(
    opts: OneShotOpts,
    wallet_id: String,
    path: String,
    // `Some("ethereum-1")` when the path came from --account/--chain — gives
    // friendlier child ids than the sanitized raw path.
    child_suffix: Option<String>,
    password: String,
    save: bool,
) -> anyhow::Result<bool> {
    use crate::bridge::{chains_for_curve, derive_address};
    use starlab_client::keystore::Keystore;

    let parsed = starlab_core::DerivationPath::parse(&path)
        .map_err(|e| anyhow::anyhow!("bad --path {path:?}: {e}"))?;

    let mut ks = Keystore::new(&opts.keystore_path, &opts.device_id)
        .map_err(|e| anyhow::anyhow!("open keystore {}: {e}", opts.keystore_path))?;
    let metas: Vec<_> = ks
        .list_wallets()
        .into_iter()
        .filter(|w| w.session_id == wallet_id)
        .cloned()
        .collect();
    if metas.is_empty() {
        anyhow::bail!(
            "wallet '{wallet_id}' not found in {} (device {}). Run `starlab-cli wallet list`.",
            opts.keystore_path,
            opts.device_id
        );
    }

    let child_id = match &child_suffix {
        Some(sfx) => format!("{wallet_id}-{sfx}"),
        None => child_wallet_id(&wallet_id, &path),
    };
    let mut addresses: Vec<crate::protocol::ChainAddress> = Vec::new();
    // (curve, child_group_hex, child_blob) per curve, for the optional save.
    let mut children: Vec<(String, String, Vec<u8>)> = Vec::new();

    for meta in &metas {
        let blob = ks
            .load_wallet_file_for_curve(&wallet_id, &meta.curve_type, &password)
            .map_err(|e| anyhow::anyhow!("unlock '{wallet_id}' ({}): {e}", meta.curve_type))?;
        let (child_blob, child_group_hex) = match meta.curve_type.as_str() {
            "ed25519" => derive_child_for_curve::<frost_ed25519::Ed25519Sha512>(&blob, &parsed)?,
            "secp256k1" => {
                derive_child_for_curve::<frost_secp256k1::Secp256K1Sha256>(&blob, &parsed)?
            }
            other => anyhow::bail!("unsupported curve in keystore: {other}"),
        };
        for (key, display) in chains_for_curve(&meta.curve_type) {
            let address = derive_address(&child_group_hex, &meta.curve_type, key);
            if !address.is_empty() {
                addresses.push(crate::protocol::ChainAddress {
                    chain: (*display).to_string(),
                    address,
                });
            }
        }
        children.push((meta.curve_type.clone(), child_group_hex, child_blob));
    }

    if save {
        let m0 = &metas[0];
        let ed = children.iter().find(|(c, _, _)| c == "ed25519");
        let secp = children.iter().find(|(c, _, _)| c == "secp256k1");
        match (ed, secp) {
            (Some((_, ed_g, ed_b)), Some((_, sp_g, sp_b))) => {
                ks.create_wallet_unified(
                    &child_id,
                    m0.threshold,
                    m0.total_participants,
                    m0.participant_index,
                    m0.participants.clone(),
                    Some(path.clone()),
                    &password,
                    ed_g,
                    ed_b,
                    sp_g,
                    sp_b,
                )
                .map_err(|e| anyhow::anyhow!("save child wallet: {e}"))?;
            }
            _ => {
                let (curve, group_hex, blob) = &children[0];
                ks.create_wallet_multi_chain(
                    &child_id,
                    curve,
                    vec![],
                    m0.threshold,
                    m0.total_participants,
                    group_hex,
                    blob,
                    &password,
                    vec![],
                    None,
                    m0.participant_index,
                    m0.participants.clone(),
                    Some(path.clone()),
                )
                .map_err(|e| anyhow::anyhow!("save child wallet: {e}"))?;
            }
        }
    }

    print(
        &CliEvent::DerivedAddresses {
            wallet_id,
            path,
            child_id,
            addresses,
            saved: save,
        },
        opts.json,
    );
    Ok(true)
}

#[cfg(test)]
mod derive_tests {
    use super::*;
    use starlab_client::elm::command::encode_keystore_blob;
    use starlab_core::resharing::{dkg_keypackages, threshold_sign_verify};
    use frost_secp256k1::Secp256K1Sha256 as Secp;
    use std::collections::BTreeMap;

    #[test]
    fn child_id_is_deterministic_and_filename_safe() {
        let id = child_wallet_id("6ced1766e7ff", "m/44'/60'/0'/0/1");
        assert_eq!(id, "6ced1766e7ff-44h-60h-0h-0-1");
        assert!(!id.contains('/') && !id.contains('\''));
    }

    /// THE money test: two participants independently derive the same path
    /// from their own shares → identical child group key, and a threshold of
    /// the DERIVED shares produces a verifying signature. HD children are
    /// first-class signing wallets, not just display addresses.
    #[test]
    fn derived_children_share_a_group_key_and_can_threshold_sign() {
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 41).unwrap();
        let path = starlab_core::DerivationPath::parse("m/44'/60'/0'/0/1").unwrap();

        let mut child_kps = BTreeMap::new();
        let mut child_groups = Vec::new();
        for i in [1u16, 2, 3] {
            let blob = encode_keystore_blob::<Secp>(&kps[&i], &pp).unwrap();
            let (child_blob, group_hex) =
                derive_child_for_curve::<Secp>(&blob, &path).unwrap();
            let (ckp, cpp) =
                starlab_client::elm::command::decode_keystore_blob::<Secp>(&child_blob).unwrap();
            child_kps.insert(i, ckp);
            child_groups.push(group_hex);
            if i == 3 {
                // quorum {1,3} of the DERIVED shares signs under the child key
                let mut quorum = BTreeMap::new();
                quorum.insert(1u16, child_kps[&1].clone());
                quorum.insert(3u16, child_kps[&3].clone());
                threshold_sign_verify::<Secp>(&quorum, &[1, 3], &cpp, b"hd-child-sign").unwrap();
            }
        }
        // all participants agree on the child group key
        assert_eq!(child_groups[0], child_groups[1]);
        assert_eq!(child_groups[1], child_groups[2]);
        // and the child key differs from the parent
        let parent_hex = hex::encode(pp.verifying_key().serialize().unwrap());
        assert_ne!(child_groups[0], parent_hex);
    }

    #[test]
    fn different_paths_yield_different_children() {
        let (kps, pp) = dkg_keypackages::<Secp>(2, 2, 42).unwrap();
        let blob = encode_keystore_blob::<Secp>(&kps[&1], &pp).unwrap();
        let p1 = starlab_core::DerivationPath::parse("m/44'/60'/0'/0/1").unwrap();
        let p2 = starlab_core::DerivationPath::parse("m/44'/60'/0'/0/2").unwrap();
        let (_, g1) = derive_child_for_curve::<Secp>(&blob, &p1).unwrap();
        let (_, g2) = derive_child_for_curve::<Secp>(&blob, &p2).unwrap();
        assert_ne!(g1, g2);
    }
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
