//! In-process multi-node DKG (+ optional signing) simulation (#21/#23).
//!
//! Runs a full N-node FROST ceremony inside one process against an embedded
//! signal server on an ephemeral port — real WebRTC over loopback, real
//! crypto, isolated per-node keystores. Self-contained for CI / LLM
//! smoke-testing; also the shared orchestration the e2e tests use.

use std::time::{Duration, Instant};

use serde::Serialize;
use starlab_client::elm::headless::{spawn_ed25519, spawn_secp256k1};
use starlab_client::elm::model::{WalletConfig, WalletMode};
use starlab_client::elm::{Message, Model};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// The user-facing label the simulated DKG creator gives its wallet. Chosen
/// so it can never collide with a `dkg_<uuid>` session id — that lets LIFE-3
/// assert the label genuinely round-tripped (vs `display_name()` falling back
/// to the session id when the label was dropped).
pub const SIM_WALLET_LABEL: &str = "sim-creator-wallet";

/// Simulation configuration.
pub struct SimulateOpts {
    pub nodes: usize,
    pub threshold: u16,
    pub curve: String,
    /// External signal server; `None` embeds one on an ephemeral port.
    pub signal_url: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct NodeOutcome {
    pub device_id: String,
    pub wallet_id: String,
    pub group_public_key: String,
}

#[derive(Debug, Serialize)]
pub struct SimulationResult {
    pub nodes: usize,
    pub threshold: u16,
    /// True iff every node finished DKG with the same non-empty group key.
    pub agreed: bool,
    pub group_public_key: String,
    pub outcomes: Vec<NodeOutcome>,
    pub elapsed_ms: u128,
}

impl SimulationResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

/// Result of a DKG-then-sign run.
#[derive(Debug, Serialize)]
pub struct SigningResult {
    pub nodes: usize,
    pub threshold: u16,
    pub group_public_key: String,
    /// Hex (no 0x) signature produced by the ceremony.
    pub signature: String,
    /// Hex (no 0x) bytes that were actually signed (EIP-191 hash for secp256k1).
    pub signed_message: String,
    /// True iff `signature` verifies against `group_public_key` for `signed_message`.
    pub verified: bool,
    pub elapsed_ms: u128,
}

impl SigningResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

#[derive(Debug, Clone)]
enum Evt {
    Connected,
    SessionDiscovered {
        id: String,
        signing: bool,
        reshare: bool,
    },
    DkgDone {
        wallet_id: String,
        group_key: String,
    },
    SignDone {
        signature: String,
        message: String,
    },
    ReshareDone {
        group_key: String,
    },
}

fn watcher() -> (
    Box<dyn Fn(&Model, Option<&Message>) + Send>,
    UnboundedReceiver<Evt>,
) {
    use starlab_client::protocal::signal::SessionType;
    let (tx, rx) = unbounded_channel::<Evt>();
    let closure = move |model: &Model, msg: Option<&Message>| {
        if model.network_state.connected {
            let _ = tx.send(Evt::Connected);
        }
        if let Some(m) = msg {
            match m {
                Message::SessionDiscovered { session } => {
                    let _ = tx.send(Evt::SessionDiscovered {
                        id: session.session_id.clone(),
                        signing: matches!(session.session_type, SessionType::Signing { .. }),
                        reshare: matches!(session.session_type, SessionType::Reshare { .. }),
                    });
                }
                Message::DKGFinalized {
                    wallet_id,
                    group_pubkey_hex,
                    ..
                } => {
                    let _ = tx.send(Evt::DkgDone {
                        wallet_id: wallet_id.clone(),
                        group_key: group_pubkey_hex.clone(),
                    });
                }
                Message::ReshareComplete {
                    group_public_key, ..
                } => {
                    let _ = tx.send(Evt::ReshareDone {
                        group_key: group_public_key.clone(),
                    });
                }
                Message::SigningComplete {
                    message, signature, ..
                } => {
                    let _ = tx.send(Evt::SignDone {
                        signature: hex::encode(signature),
                        message: hex::encode(message),
                    });
                }
                _ => {}
            }
        }
    };
    (Box::new(closure), rx)
}

async fn wait_for<F>(rx: &mut UnboundedReceiver<Evt>, secs: u64, pred: F) -> anyhow::Result<Evt>
where
    F: Fn(&Evt) -> bool,
{
    tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            match rx.recv().await {
                Some(e) if pred(&e) => return Ok(e),
                Some(_) => continue,
                None => anyhow::bail!("event channel closed"),
            }
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out after {secs}s waiting for event"))?
}

/// A live N-node cluster that has completed DKG. Runners stay alive (their
/// senders/receivers are retained) so callers can drive signing afterwards.
struct Cluster {
    device_ids: Vec<String>,
    senders: Vec<UnboundedSender<Message>>,
    receivers: Vec<UnboundedReceiver<Evt>>,
    // Keystores must outlive the runners.
    keystores: Vec<tempfile::TempDir>,
    outcomes: Vec<NodeOutcome>,
    group_key: String,
    agreed: bool,
    elapsed_ms: u128,
}

/// Quit every runner and give the runtime a beat to reclaim their WebRTC/ICE
/// sockets. Clusters run back to back in one process (the conformance matrix,
/// reshare, reload); runners left alive accumulate ICE agents on loopback and
/// the next cluster's mesh intermittently fails ("WebRTC connection FAILED").
/// Keystore TempDirs must outlive this call.
async fn shut_down(senders: Vec<UnboundedSender<Message>>, receivers: Vec<UnboundedReceiver<Evt>>) {
    for tx in &senders {
        let _ = tx.send(Message::Quit);
    }
    drop(senders);
    drop(receivers);
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// Spawn an embedded signal server on an ephemeral loopback port and return
/// its `ws://` URL. The server task runs until the process exits.
async fn embedded_signal_server() -> anyhow::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(starlab_signal_server::run(listener));
    Ok(format!("ws://127.0.0.1:{port}"))
}

fn validate(opts: &SimulateOpts) -> anyhow::Result<()> {
    if opts.curve != "secp256k1" && opts.curve != "ed25519" {
        anyhow::bail!("simulate supports curve=secp256k1 or ed25519");
    }
    if opts.nodes < 2 {
        anyhow::bail!("need at least 2 nodes");
    }
    if opts.threshold < 1 || opts.threshold as usize > opts.nodes {
        anyhow::bail!("threshold must be in 1..=nodes");
    }
    Ok(())
}

/// Spin up the embedded signal server (if needed) + N runners and run DKG.
async fn dkg_cluster(opts: &SimulateOpts) -> anyhow::Result<Cluster> {
    validate(opts)?;
    let started = Instant::now();

    let ws_url = match &opts.signal_url {
        Some(u) => u.clone(),
        None => embedded_signal_server().await?,
    };

    let mut keystores = Vec::new();
    let mut senders = Vec::new();
    let mut receivers = Vec::new();
    let device_ids: Vec<String> = (0..opts.nodes).map(|i| format!("sim-node-{i}")).collect();
    for device_id in &device_ids {
        let ks = tempfile::TempDir::new()?;
        let (cb, rx) = watcher();
        let ks_path = ks.path().to_string_lossy().into_owned();
        let tx = if opts.curve == "ed25519" {
            spawn_ed25519(device_id.clone(), ks_path, ws_url.clone(), cb)
        } else {
            spawn_secp256k1(device_id.clone(), ks_path, ws_url.clone(), cb)
        };
        keystores.push(ks);
        senders.push(tx);
        receivers.push(rx);
    }

    for tx in &senders {
        let _ = tx.send(Message::TriggerReconnect);
    }
    for rx in &mut receivers {
        wait_for(rx, 15, |e| matches!(e, Evt::Connected)).await?;
    }

    senders[0].send(Message::HeadlessCreateWallet {
        config: WalletConfig {
            name: SIM_WALLET_LABEL.into(),
            total_participants: opts.nodes as u16,
            threshold: opts.threshold,
            mode: WalletMode::Online,
        },
        password: "sim-password-0".into(),
        label: SIM_WALLET_LABEL.into(),
    })?;

    for (i, rx) in receivers.iter_mut().enumerate().skip(1) {
        let session_id = match wait_for(rx, 20, |e| {
            matches!(e, Evt::SessionDiscovered { signing: false, .. })
        })
        .await?
        {
            Evt::SessionDiscovered { id, .. } => id,
            _ => unreachable!(),
        };
        senders[i].send(Message::HeadlessJoinSession {
            session_id,
            password: format!("sim-password-{i}"),
            label: "sim".into(),
        })?;
    }

    let mut outcomes = Vec::new();
    for (i, rx) in receivers.iter_mut().enumerate() {
        let done = wait_for(rx, opts.timeout_secs, |e| matches!(e, Evt::DkgDone { .. })).await?;
        if let Evt::DkgDone {
            wallet_id,
            group_key,
        } = done
        {
            outcomes.push(NodeOutcome {
                device_id: device_ids[i].clone(),
                wallet_id,
                group_public_key: group_key,
            });
        }
    }

    let group_key = outcomes
        .first()
        .map(|o| o.group_public_key.clone())
        .unwrap_or_default();
    let agreed = !group_key.is_empty() && outcomes.iter().all(|o| o.group_public_key == group_key);

    Ok(Cluster {
        device_ids,
        senders,
        receivers,
        keystores,
        outcomes,
        group_key,
        agreed,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Drive a threshold signing once a cluster is connected and the wallet is
/// available: node 0 initiates, `join_count` co-signers (nodes 1..=join_count)
/// join the signing session, then we await the aggregated signature on
/// EVERY node that joined — every joiner is guaranteed a member of the
/// proposer's fixed signer set (join_count is normally threshold-1, so the
/// joined set IS the signer set), and every node that ran Round 2 must
/// independently converge on the identical signature. Shared by the
/// fresh-DKG and reload-from-disk signing paths.
///
/// Returns one `(signature, signed_message)` pair per joined node, in node
/// order (index 0 first).
async fn drive_signing(
    senders: &[UnboundedSender<Message>],
    receivers: &mut [UnboundedReceiver<Evt>],
    wallet_id: String,
    message: &str,
    encoding: &str,
    join_count: usize,
    timeout_secs: u64,
) -> anyhow::Result<Vec<(String, String)>> {
    // Initiator (node 0) announces the signing request.
    senders[0].send(Message::HeadlessSign {
        wallet_id,
        message: message.to_string(),
        encoding: encoding.to_string(),
        password: "sim-password-0".into(),
    })?;

    // Co-signers 1..=join_count approve by joining the signing session.
    for i in 1..=join_count {
        let session_id = match wait_for(&mut receivers[i], 20, |e| {
            matches!(e, Evt::SessionDiscovered { signing: true, .. })
        })
        .await?
        {
            Evt::SessionDiscovered { id, .. } => id,
            _ => unreachable!(),
        };
        senders[i].send(Message::HeadlessJoinSession {
            session_id,
            password: format!("sim-password-{i}"),
            label: String::new(),
        })?;
    }

    // Wait for the aggregated signature on every node that joined.
    let mut results = Vec::with_capacity(join_count + 1);
    for rx in receivers.iter_mut().take(join_count + 1) {
        match wait_for(rx, timeout_secs, |e| matches!(e, Evt::SignDone { .. })).await? {
            Evt::SignDone { signature, message } => results.push((signature, message)),
            _ => unreachable!(),
        }
    }
    Ok(results)
}

/// Run DKG only and return a summary.
pub async fn run_simulation(opts: SimulateOpts) -> anyhow::Result<SimulationResult> {
    let nodes = opts.nodes;
    let threshold = opts.threshold;
    let c = dkg_cluster(&opts).await?;
    shut_down(c.senders, c.receivers).await;
    Ok(SimulationResult {
        nodes,
        threshold,
        agreed: c.agreed,
        group_public_key: c.group_key,
        outcomes: c.outcomes,
        elapsed_ms: c.elapsed_ms,
    })
}

/// Run DKG, then sign `message` with a quorum, and verify the signature.
pub async fn run_signing_simulation(
    opts: SimulateOpts,
    message: &str,
) -> anyhow::Result<SigningResult> {
    run_signing_simulation_enc(opts, message, "utf8").await
}

/// As [`run_signing_simulation`], but with an explicit message `encoding`
/// ("utf8" or "hex") — exercises `HeadlessSign`'s hex-decode path (SIG-6).
pub async fn run_signing_simulation_enc(
    opts: SimulateOpts,
    message: &str,
    encoding: &str,
) -> anyhow::Result<SigningResult> {
    let nodes = opts.nodes;
    let threshold = opts.threshold;
    let started = Instant::now();
    let mut c = dkg_cluster(&opts).await?;
    if !c.agreed {
        anyhow::bail!("DKG did not agree; aborting signing");
    }
    let wallet_id = c.outcomes[0].wallet_id.clone();

    let (signature, signed_message) = drive_signing(
        &c.senders,
        &mut c.receivers,
        wallet_id,
        message,
        encoding,
        (threshold as usize).saturating_sub(1),
        opts.timeout_secs,
    )
    .await?
    .remove(0);
    shut_down(c.senders, c.receivers).await;

    let verified =
        verify_signature(&opts.curve, &c.group_key, &signed_message, &signature).unwrap_or(false);

    Ok(SigningResult {
        nodes,
        threshold,
        group_public_key: c.group_key,
        signature,
        signed_message,
        verified,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Result of a signing run where EVERY node joined the ceremony — not just
/// `threshold`-many. Regression coverage for the bug where every node ran
/// Round 2 the moment it individually saw threshold-many commitments: with
/// MORE than threshold signers online, different nodes picked different
/// signer sets and `aggregate` failed with `UnknownIdentifier`. The fix has
/// the proposer fix ONE signer set and broadcast it before anyone runs
/// Round 2, so every joined node — set member or not — must converge on the
/// identical aggregated signature.
#[derive(Debug, Serialize)]
pub struct AllSignersOnlineResult {
    pub nodes: usize,
    pub threshold: u16,
    pub group_public_key: String,
    /// One aggregated signature (hex) per joined node, in node order.
    pub signatures: Vec<String>,
    /// True iff every joined node reported the exact same signature bytes.
    pub all_agreed: bool,
    /// True iff `signatures[0]` verifies against `group_public_key`.
    pub verified: bool,
    pub elapsed_ms: u128,
}

impl AllSignersOnlineResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

/// DKG, then sign `message` with EVERY node online and joined (not just a
/// threshold-many quorum) — see [`AllSignersOnlineResult`].
pub async fn run_signing_simulation_all_signers_online(
    opts: SimulateOpts,
    message: &str,
) -> anyhow::Result<AllSignersOnlineResult> {
    let nodes = opts.nodes;
    let threshold = opts.threshold;
    let started = Instant::now();
    let mut c = dkg_cluster(&opts).await?;
    if !c.agreed {
        anyhow::bail!("DKG did not agree; aborting signing");
    }
    let wallet_id = c.outcomes[0].wallet_id.clone();

    // Every node besides the initiator joins — this is the "more than
    // threshold signers online" scenario the old per-node first-come rule
    // got wrong.
    let results = drive_signing(
        &c.senders,
        &mut c.receivers,
        wallet_id,
        message,
        "utf8",
        nodes - 1,
        opts.timeout_secs,
    )
    .await?;
    shut_down(c.senders, c.receivers).await;

    let signatures: Vec<String> = results.iter().map(|(sig, _)| sig.clone()).collect();
    let all_agreed = signatures.len() == nodes && signatures.windows(2).all(|w| w[0] == w[1]);
    let verified = match results.first() {
        Some((sig, msg)) => verify_signature(&opts.curve, &c.group_key, msg, sig).unwrap_or(false),
        None => false,
    };

    Ok(AllSignersOnlineResult {
        nodes,
        threshold,
        group_public_key: c.group_key,
        signatures,
        all_agreed,
        verified,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Outcome of a Bitcoin (P2TR key-path) signing run.
#[derive(Debug, Serialize)]
pub struct BitcoinSigningResult {
    /// Account-0 P2TR address the shares sign for.
    pub address: String,
    pub sighash: String,
    pub signature: String,
    /// BIP-340 verification of `signature` over `sighash` against `address`.
    pub verified: bool,
}

/// DKG, then sign a 32-byte BIP-341 sighash with the wallet's Bitcoin
/// account 0 and verify it the way a Bitcoin node would: BIP-340 against the
/// output key its P2TR address commits to.
pub async fn run_bitcoin_signing_simulation(
    opts: SimulateOpts,
    sighash_hex: &str,
) -> anyhow::Result<BitcoinSigningResult> {
    let threshold = opts.threshold;
    let mut c = dkg_cluster(&opts).await?;
    if !c.agreed {
        anyhow::bail!("DKG did not agree; aborting signing");
    }
    let wallet_id = format!("{}-bitcoin-0", c.outcomes[0].wallet_id);
    let (signature, signed_message) = drive_signing(
        &c.senders,
        &mut c.receivers,
        wallet_id,
        sighash_hex,
        "hex",
        (threshold as usize).saturating_sub(1),
        opts.timeout_secs,
    )
    .await?
    .remove(0);
    shut_down(c.senders, c.receivers).await;

    let group_key = hex::decode(&c.group_key)?;
    let address = starlab_core::accounts::account_addresses("secp256k1", &group_key, 0)?
        .into_iter()
        .find(|(chain, _, _)| chain == "Bitcoin")
        .map(|(_, _, address)| address)
        .ok_or_else(|| anyhow::anyhow!("no Bitcoin account address"))?;
    let verified = starlab_core::accounts::verify_taproot_signature(
        &address,
        &hex::decode(signed_message.trim_start_matches("0x"))?,
        &hex::decode(signature.trim_start_matches("0x"))?,
    )?;
    Ok(BitcoinSigningResult {
        address,
        sighash: signed_message,
        signature,
        verified,
    })
}

/// Outcome of a networked reshare end-to-end run (#45 4b).
#[derive(Debug, Serialize)]
pub struct ReshareE2eResult {
    pub nodes: usize,
    pub threshold: u16,
    pub curve: String,
    pub dkg_group_public_key: String,
    /// Group key reported by every node after the reshare.
    pub reshare_group_public_key: String,
    /// True iff every node's post-reshare group key == the DKG group key.
    pub key_preserved: bool,
    /// True iff a threshold signature with the REFRESHED shares verifies.
    pub signed_after_reshare: bool,
    /// True iff node 0's refreshed share is on disk with the unchanged group key.
    pub share_persisted: bool,
    pub elapsed_ms: u128,
}

impl ReshareE2eResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
    pub fn ok(&self) -> bool {
        self.key_preserved && self.signed_after_reshare && self.share_persisted
    }
}

/// Which runners perform the reshare after the DKG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReshareRunners {
    /// Fresh processes against the same keystores (a device that restarted).
    Fresh,
    /// The runners that just ran the DKG (a long-lived `serve` node).
    Reused,
}

/// Networked reshare end to end over the real WebRTC mesh: run DKG, then a
/// same-set reshare (announced by node 0, joined by the rest) on `runners`,
/// confirm all nodes preserve the group key, and finally sign with the
/// REFRESHED shares + verify.
pub async fn run_reshare_e2e(
    opts: SimulateOpts,
    message: &str,
    runners: ReshareRunners,
) -> anyhow::Result<ReshareE2eResult> {
    let nodes = opts.nodes;
    let threshold = opts.threshold;
    let started = Instant::now();
    let c = dkg_cluster(&opts).await?;
    if !c.agreed {
        anyhow::bail!("DKG did not agree; aborting reshare");
    }
    let dkg_group_key = c.group_key.clone();
    let wallet_id = c.outcomes[0].wallet_id.clone();

    let device_ids = c.device_ids.clone();
    let keystores = c.keystores; // move: must outlive the runners
    let (senders, mut receivers) = match runners {
        ReshareRunners::Reused => (c.senders, c.receivers),
        ReshareRunners::Fresh => {
            // True cross-process reshare (#56): tear down the DKG runners and spawn FRESH
            // ones against the SAME keystores. Each fresh node loads its OLD share from
            // disk, the initiator announces a reshare session, the retained signers
            // discover + join, a NEW mesh forms, and every node refreshes. This
            // exercises the disk-load + announce/join path a separate device would take
            // — not the in-memory post-DKG mesh shortcut (4b).
            //
            // The fresh runners reuse the ORIGINAL device_ids (reshare identifiers are
            // canonical over the original set, design §3) so they connect to a FRESH
            // embedded signal server — the old runners still hold those ids on the old
            // server (the server rejects duplicate live registrations, exactly as it
            // would for a real still-connected device).
            let ws_url = match &opts.signal_url {
                Some(u) => u.clone(),
                None => embedded_signal_server().await?,
            };
            // Tear the DKG cluster down before bringing the reshare cluster up.
            shut_down(c.senders, c.receivers).await;

            let mut senders = Vec::new();
            let mut receivers = Vec::new();
            for (i, device_id) in device_ids.iter().enumerate() {
                let (cb, rx) = watcher();
                let ks_path = keystores[i].path().to_string_lossy().into_owned();
                let tx = if opts.curve == "ed25519" {
                    spawn_ed25519(device_id.clone(), ks_path, ws_url.clone(), cb)
                } else {
                    spawn_secp256k1(device_id.clone(), ks_path, ws_url.clone(), cb)
                };
                senders.push(tx);
                receivers.push(rx);
            }
            for tx in &senders {
                let _ = tx.send(Message::TriggerReconnect);
            }
            for rx in &mut receivers {
                wait_for(rx, 15, |e| matches!(e, Evt::Connected)).await?;
            }
            (senders, receivers)
        }
    };

    // Initiator (node 0) announces the reshare; retained signers 1.. join it.
    senders[0].send(Message::HeadlessReshare {
        wallet_id: wallet_id.clone(),
        password: "sim-password-0".into(),
        keystore_path: keystores[0].path().to_string_lossy().to_string(),
    })?;
    for (i, rx) in receivers.iter_mut().enumerate().skip(1) {
        let session_id = match wait_for(rx, 20, |e| {
            matches!(e, Evt::SessionDiscovered { reshare: true, .. })
        })
        .await?
        {
            Evt::SessionDiscovered { id, .. } => id,
            _ => unreachable!(),
        };
        senders[i].send(Message::HeadlessJoinSession {
            session_id,
            password: format!("sim-password-{i}"),
            label: "reshare".into(),
        })?;
    }

    // Every node must report reshare completion with the unchanged group key.
    let mut reshare_keys = Vec::new();
    for rx in receivers.iter_mut() {
        let done = wait_for(rx, opts.timeout_secs, |e| {
            matches!(e, Evt::ReshareDone { .. })
        })
        .await?;
        if let Evt::ReshareDone { group_key } = done {
            reshare_keys.push(group_key);
        }
    }
    let key_preserved =
        reshare_keys.len() == nodes && reshare_keys.iter().all(|k| *k == dkg_group_key);
    let reshare_group_key = reshare_keys.first().cloned().unwrap_or_default();

    // Persistence check: node 0's refreshed share is on disk with the unchanged
    // group key (a fresh Keystore reads from the file we just rewrote).
    let persisted_group_key = {
        use starlab_client::keystore::Keystore;
        Keystore::new(keystores[0].path(), &device_ids[0])
            .ok()
            .and_then(|ks| {
                ks.get_wallet(&wallet_id)
                    .map(|w| w.group_public_key.clone())
            })
            .unwrap_or_default()
    };
    let share_persisted = persisted_group_key == dkg_group_key;

    // Sign with the REFRESHED shares and verify against the (unchanged) group
    // key's account-0 child.
    let (signature, signed_message) = drive_signing(
        &senders,
        &mut receivers,
        wallet_id,
        message,
        "utf8",
        (threshold as usize).saturating_sub(1),
        opts.timeout_secs,
    )
    .await?
    .remove(0);
    shut_down(senders, receivers).await;
    let signed_after_reshare =
        verify_signature(&opts.curve, &dkg_group_key, &signed_message, &signature).unwrap_or(false);

    Ok(ReshareE2eResult {
        nodes,
        threshold,
        curve: opts.curve.clone(),
        dkg_group_public_key: dkg_group_key,
        reshare_group_public_key: reshare_group_key,
        key_preserved,
        signed_after_reshare,
        share_persisted,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Outcome of a reload-and-list (LIFE-1) run.
#[derive(Debug, Serialize)]
pub struct ReloadListResult {
    /// Group key the DKG produced (what the reload must rediscover).
    pub expected_group_public_key: String,
    /// Group keys the fresh runner loaded from the persisted keystore.
    pub reloaded_group_keys: Vec<String>,
    /// User-facing wallet names (`display_name()`) after the cold reload —
    /// LIFE-3 checks the creation label survives the keystore round-trip.
    pub reloaded_wallet_names: Vec<String>,
    /// True iff the expected key reappeared after the cold reload.
    pub persisted: bool,
    pub elapsed_ms: u128,
}

impl ReloadListResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

/// LIFE-1: run DKG, tear the original node 0 runner down, then bring a FRESH
/// runner up on node 0's SAME keystore directory and list its wallets — the
/// persisted share must reappear with the original group key.
///
/// This is a pure keystore round-trip: the fresh runner is never connected to
/// the network (no `TriggerReconnect`), so it's fully deterministic and free
/// of the in-process "ghost task" interference that makes a faithful
/// cold-restart *re-signing* (LIFE-2) impossible in one process — `Quit` only
/// breaks the Elm loop, leaving the old node's WebRTC/WS tasks (and their ICE
/// agents) alive to corrupt a new mesh. Faithful LIFE-2 needs real process
/// death and belongs in the L3 `serve`-subprocess harness (see
/// docs/cli-conformance-testing.md).
pub async fn run_reload_list_simulation(opts: SimulateOpts) -> anyhow::Result<ReloadListResult> {
    let started = Instant::now();

    let c = dkg_cluster(&opts).await?;
    if !c.agreed {
        anyhow::bail!("DKG did not agree; aborting reload-list");
    }
    let expected_group_public_key = c.group_key.clone();
    let device_id = c.device_ids[0].clone();
    let keystore_path = c.keystores[0].path().to_string_lossy().into_owned();

    // Tear down the original runners; keep the TempDirs (in `c`) alive so the
    // on-disk keystores persist for the fresh runner to read.
    shut_down(c.senders, c.receivers).await;

    // Fresh runner on node 0's keystore. The runner auto-sends ListWallets on
    // startup (see HeadlessRunner::run), so the persisted wallets land in the
    // model without any network. Capture their group keys via the sync hook.
    // Capture (group_key, display_name) pairs from the reloaded model.
    let (wtx, mut wrx) = unbounded_channel::<Vec<(String, String)>>();
    let cb = move |model: &Model, _msg: Option<&Message>| {
        let wallets: Vec<(String, String)> = model
            .wallet_state
            .wallets
            .iter()
            .map(|w| (w.group_public_key.clone(), w.display_name().to_string()))
            .collect();
        if !wallets.is_empty() {
            let _ = wtx.send(wallets);
        }
    };
    // Signal URL is irrelevant — we never connect.
    let _tx = spawn_secp256k1(device_id, keystore_path, String::new(), cb);

    let reloaded = tokio::time::timeout(Duration::from_secs(10), wrx.recv())
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for reloaded wallet list"))?
        .ok_or_else(|| anyhow::anyhow!("reloaded runner produced no wallet list"))?;

    let reloaded_group_keys: Vec<String> = reloaded.iter().map(|(k, _)| k.clone()).collect();
    let reloaded_wallet_names: Vec<String> = reloaded.iter().map(|(_, n)| n.clone()).collect();
    let persisted = reloaded_group_keys.contains(&expected_group_public_key);

    drop(c.keystores);

    Ok(ReloadListResult {
        expected_group_public_key,
        reloaded_group_keys,
        reloaded_wallet_names,
        persisted,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Outcome of a reload-and-unlock attempt (ERR-1).
#[derive(Debug, Serialize)]
pub struct UnlockAttemptResult {
    /// True iff the keystore unlocked (correct password).
    pub unlocked: bool,
    /// True iff the unlock was cleanly rejected (`WalletUnlockFailed`).
    pub failed: bool,
    /// The rejection message, if any.
    pub error: Option<String>,
    pub elapsed_ms: u128,
}

impl UnlockAttemptResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

/// ERR-1: run DKG, tear node 0 down, then on a FRESH runner over node 0's
/// keystore attempt to unlock-and-sign with `password`. A wrong password must
/// be rejected cleanly (`WalletUnlockFailed`) — no panic, no partial state —
/// while the correct password unlocks. The unlock reads the keystore directly
/// and fires before any mesh is needed, so this is fast and deterministic (no
/// network, immune to the ghost-task issue that blocks in-process LIFE-2).
pub async fn run_reload_unlock_simulation(
    opts: SimulateOpts,
    password: &str,
) -> anyhow::Result<UnlockAttemptResult> {
    let started = Instant::now();

    let c = dkg_cluster(&opts).await?;
    if !c.agreed {
        anyhow::bail!("DKG did not agree; aborting reload-unlock");
    }
    let wallet_id = c.outcomes[0].wallet_id.clone();
    let device_id = c.device_ids[0].clone();
    let keystore_path = c.keystores[0].path().to_string_lossy().into_owned();

    shut_down(c.senders, c.receivers).await;

    // (unlocked, error) — exactly one of WalletUnlocked / WalletUnlockFailed.
    let (utx, mut urx) = unbounded_channel::<(bool, Option<String>)>();
    let cb = move |_model: &Model, msg: Option<&Message>| match msg {
        Some(Message::WalletUnlocked { .. }) => {
            let _ = utx.send((true, None));
        }
        Some(Message::WalletUnlockFailed { error }) => {
            let _ = utx.send((false, Some(error.clone())));
        }
        _ => {}
    };
    // No network: unlock reads the keystore directly.
    let tx = spawn_secp256k1(device_id, keystore_path, String::new(), cb);
    tx.send(Message::HeadlessSign {
        wallet_id,
        message: "err1-unlock-probe".into(),
        encoding: "utf8".into(),
        password: password.to_string(),
    })?;

    let (unlocked, error) = tokio::time::timeout(Duration::from_secs(15), urx.recv())
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for unlock outcome"))?
        .ok_or_else(|| anyhow::anyhow!("runner produced no unlock outcome"))?;

    // Stop the runner (a correct unlock would otherwise proceed to a mesh-
    // dependent signing attempt we don't drive here).
    let _ = tx.send(Message::Quit);
    drop(c.keystores);

    Ok(UnlockAttemptResult {
        unlocked,
        failed: !unlocked,
        error,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Outcome of a late-join discovery run (LIFE-4).
#[derive(Debug, Serialize)]
pub struct LateJoinResult {
    /// True iff the late node discovered the session WITHOUT an explicit
    /// replay request (i.e. an automatic cold-start replay on connect).
    pub discovered_on_connect: bool,
    /// True iff the late node discovered the session after an explicit
    /// `RequestActiveSessions` replay.
    pub discovered_after_refresh: bool,
    pub elapsed_ms: u128,
}

impl LateJoinResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

/// LIFE-4: a session announced BEFORE a node connects must still be
/// discoverable. node 0 connects and announces a DKG session; node 1 then
/// connects *late* (missing the live broadcast) and must find the session via
/// the `RequestActiveSessions` replay.
///
/// Also records whether the late node discovered it WITHOUT an explicit
/// refresh — i.e. whether the headless runner auto-replays on connect the way
/// the browser extension does. (It currently does not; the headless/CLI path
/// needs an explicit refresh — `discovered_on_connect` captures that parity
/// gap rather than asserting it.)
pub async fn run_late_join_discovery_simulation(
    opts: SimulateOpts,
) -> anyhow::Result<LateJoinResult> {
    validate(&opts)?;
    let started = Instant::now();

    let ws_url = match &opts.signal_url {
        Some(u) => u.clone(),
        None => embedded_signal_server().await?,
    };

    // --- node 0: connect, announce a DKG session, leave it pending ---
    let ks0 = tempfile::TempDir::new()?;
    let (cb0, mut rx0) = watcher();
    let tx0 = spawn_secp256k1(
        "late-node-0".into(),
        ks0.path().to_string_lossy().into_owned(),
        ws_url.clone(),
        cb0,
    );
    let _ = tx0.send(Message::TriggerReconnect);
    wait_for(&mut rx0, 15, |e| matches!(e, Evt::Connected)).await?;
    tx0.send(Message::HeadlessCreateWallet {
        config: WalletConfig {
            name: SIM_WALLET_LABEL.into(),
            total_participants: opts.nodes as u16,
            threshold: opts.threshold,
            mode: WalletMode::Online,
        },
        password: "late-password-0".into(),
        label: SIM_WALLET_LABEL.into(),
    })?;
    // Let the announcement reach the server before node 1 connects, so node 1
    // genuinely misses the live broadcast and must rely on the replay.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // --- node 1: connect LATE (after the announce) ---
    let ks1 = tempfile::TempDir::new()?;
    let (cb1, mut rx1) = watcher();
    let tx1 = spawn_secp256k1(
        "late-node-1".into(),
        ks1.path().to_string_lossy().into_owned(),
        ws_url.clone(),
        cb1,
    );
    let _ = tx1.send(Message::TriggerReconnect);
    wait_for(&mut rx1, 15, |e| matches!(e, Evt::Connected)).await?;

    // Did it auto-discover on connect (no explicit refresh)?
    let discovered_on_connect = wait_for(&mut rx1, 3, |e| {
        matches!(e, Evt::SessionDiscovered { signing: false, .. })
    })
    .await
    .is_ok();

    // Now request the replay explicitly and require discovery.
    let discovered_after_refresh = if discovered_on_connect {
        true
    } else {
        tx1.send(Message::HeadlessRefreshSessions)?;
        wait_for(&mut rx1, 15, |e| {
            matches!(e, Evt::SessionDiscovered { signing: false, .. })
        })
        .await
        .is_ok()
    };

    let _ = tx0.send(Message::Quit);
    let _ = tx1.send(Message::Quit);
    drop(ks0);
    drop(ks1);

    Ok(LateJoinResult {
        discovered_on_connect,
        discovered_after_refresh,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// ERR-4: attempt to sign a wallet that doesn't exist. A fresh runner over an
/// empty keystore (no DKG, no network) is asked to sign a bogus wallet_id; the
/// unlock must fail cleanly (`WalletUnlockFailed`) rather than hang or panic.
/// Fast + deterministic — no signal server, no WebRTC.
pub async fn run_unknown_wallet_sign_simulation() -> anyhow::Result<UnlockAttemptResult> {
    let started = Instant::now();
    let ks = tempfile::TempDir::new()?;
    let (utx, mut urx) = unbounded_channel::<(bool, Option<String>)>();
    let cb = move |_model: &Model, msg: Option<&Message>| match msg {
        Some(Message::WalletUnlocked { .. }) => {
            let _ = utx.send((true, None));
        }
        Some(Message::WalletUnlockFailed { error }) => {
            let _ = utx.send((false, Some(error.clone())));
        }
        _ => {}
    };
    let tx = spawn_secp256k1(
        "err4-node".into(),
        ks.path().to_string_lossy().into_owned(),
        String::new(),
        cb,
    );
    tx.send(Message::HeadlessSign {
        wallet_id: "wallet-does-not-exist".into(),
        message: "err4".into(),
        encoding: "utf8".into(),
        password: "pw".into(),
    })?;

    let (unlocked, error) = tokio::time::timeout(Duration::from_secs(10), urx.recv())
        .await
        .map_err(|_| anyhow::anyhow!("timed out — sign of unknown wallet hung (no clean error)"))?
        .ok_or_else(|| anyhow::anyhow!("runner produced no unlock outcome"))?;

    let _ = tx.send(Message::Quit);
    drop(ks);

    Ok(UnlockAttemptResult {
        unlocked,
        failed: !unlocked,
        error,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Verify a produced FROST signature for a root wallet. `HeadlessSign` never
/// signs with the root: a root id is mapped to account 0 of the curve's
/// primary chain, so verify against that child key, derived publicly from
/// the root group key.
fn verify_signature(
    curve: &str,
    group_key_hex: &str,
    message_hex: &str,
    sig_hex: &str,
) -> anyhow::Result<bool> {
    let chain = if curve == "ed25519" {
        "solana"
    } else {
        "ethereum"
    };
    let path = starlab_core::accounts::standard_path(chain, 0)
        .ok_or_else(|| anyhow::anyhow!("no standard path for {chain}"))?;
    let path = starlab_core::DerivationPath::parse(&path)?;
    let group_key = hex::decode(group_key_hex)?;
    if curve == "ed25519" {
        let child = starlab_core::derive_child_verifying_key_path::<frost_ed25519::Ed25519Sha512>(
            &group_key, &path,
        )?;
        verify_ed25519(&hex::encode(child), message_hex, sig_hex)
    } else {
        let child = starlab_core::derive_child_verifying_key_path::<
            frost_secp256k1_tr::Secp256K1Sha256TR,
        >(&group_key, &path)?;
        verify_secp256k1(&hex::encode(child), message_hex, sig_hex)
    }
}

/// Verify a FROST(secp256k1) signature against the group verifying key.
fn verify_secp256k1(group_key_hex: &str, message_hex: &str, sig_hex: &str) -> anyhow::Result<bool> {
    use frost_secp256k1_tr::{Signature, VerifyingKey};
    let vk_bytes = hex::decode(group_key_hex)?;
    let msg = hex::decode(message_hex)?;
    let sig_bytes = hex::decode(sig_hex)?;
    let vk = VerifyingKey::deserialize(&vk_bytes)?;
    let sig = Signature::deserialize(&sig_bytes)?;
    Ok(vk.verify(&msg, &sig).is_ok())
}

/// Verify a FROST(ed25519) signature against the group verifying key.
fn verify_ed25519(group_key_hex: &str, message_hex: &str, sig_hex: &str) -> anyhow::Result<bool> {
    use frost_ed25519::{Signature, VerifyingKey};
    let vk_bytes = hex::decode(group_key_hex)?;
    let msg = hex::decode(message_hex)?;
    let sig_bytes = hex::decode(sig_hex)?;
    let vk = VerifyingKey::deserialize(&vk_bytes)?;
    let sig = Signature::deserialize(&sig_bytes)?;
    Ok(vk.verify(&msg, &sig).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "real WebRTC/DKG over loopback; run with --ignored"]
    async fn simulate_2_of_2() {
        let result = run_simulation(SimulateOpts {
            nodes: 2,
            threshold: 2,
            curve: "secp256k1".into(),
            signal_url: None,
            timeout_secs: 90,
        })
        .await
        .expect("simulation ran");
        assert!(result.agreed, "nodes disagreed: {:?}", result.outcomes);
        assert_eq!(result.outcomes.len(), 2);
        assert!(!result.group_public_key.is_empty());
    }

    /// ERR-4: signing a wallet that doesn't exist fails cleanly (no DKG, no
    /// network) — fast lane.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unknown_wallet_sign_fails_cleanly() {
        let r = run_unknown_wallet_sign_simulation()
            .await
            .expect("did not hang");
        assert!(r.failed && !r.unlocked, "expected clean failure, got {r:?}");
        assert!(r.error.is_some(), "expected an error message");
    }
}
