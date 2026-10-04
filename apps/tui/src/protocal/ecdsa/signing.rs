//! Ethereum signing through the threshold-ECDSA protocol.
//!
//! A cggmp24 signing needs its exactly-`threshold` signer set up front, so
//! the proposer fixes it, with the same rule FROST uses (`SIGN_SET`, #121):
//!
//! 1. Every joined node (having unlocked its share) sends
//!    `ECDSA_SIGN_READY` to the proposer.
//! 2. The proposer takes itself + the first `threshold - 1` READY senders,
//!    broadcasts `ECDSA_SIGN_SET` to every participant, and starts signing.
//! 3. A node listed in the set starts signing on receipt; one left out waits.
//! 4. Each signer outputs the signature; the proposer then sends it to the
//!    participants outside the set (`ECDSA_SIGN_DONE`), who check it
//!    recovers to the account's address — so every joined node reports the
//!    same signature.
//!
//! Every control frame names the signing session (`sign_<uuid>`, fresh per
//! signing), and protocol frames carry the ceremony's execution id, so
//! frames of an earlier signing can never be mistaken for this one's.

use super::wire::{self, Control};
use super::worker::{self, Job, Outcome};
use crate::elm::message::Message;
use crate::protocal::signing::INLINE_SIGNING_ID;
use crate::utils::appstate_compat::AppState;
use frost_core::Ciphersuite;
use starlab_core::ecdsa::{ECDSA_CURVE, EcdsaKeyShare, EcdsaSignature};
use starlab_core::hd_derivation::DerivationPath;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tracing::{error, info, warn};

/// Control frames kept for a signing that hasn't started here yet.
const MAX_PENDING_CONTROL: usize = 64;

/// Whether `wallet_id` is an account child that signs with the ECDSA key
/// (`{root}-ethereum-{n}`).
pub fn is_ecdsa_wallet(wallet_id: &str) -> bool {
    starlab_core::accounts::parse_child_wallet_id(wallet_id).is_some_and(|(_, chain, _)| {
        starlab_core::accounts::curve_for_chain(chain) == Some(ECDSA_CURVE)
    })
}

/// `r ‖ s ‖ v` with `v = 27 + recovery_id` — what `personal_sign` /
/// `eth_sign` return; a typed transaction takes `y_parity = v - 27`.
pub fn ethereum_signature_bytes(sig: &EcdsaSignature) -> Vec<u8> {
    let mut bytes = sig.to_bytes().to_vec();
    bytes[64] += 27;
    bytes
}

/// One running signing on this node.
pub(crate) struct SigningCtx {
    session_id: String,
    proposer: String,
    self_id: String,
    /// Device ids in keygen order (the share's participant list).
    participants: Vec<String>,
    threshold: usize,
    prehash: [u8; 32],
    path: DerivationPath,
    share: EcdsaKeyShare,
    /// Account address the signature must recover to.
    address: String,
    /// Proposer only: READY senders in arrival order, self first.
    ready: Vec<String>,
    signer_set: Option<Vec<String>>,
    worker_started: bool,
    epoch: u64,
    ui_tx: UnboundedSender<Message>,
}

/// Start an ECDSA signing of the 32-byte `message` (EIP-191 hash or a raw
/// prehash) for the Ethereum account child `wallet_id`, in the signing
/// session recorded on `AppState.session` (fresh `sign_<uuid>` id; the
/// proposer is `session.proposer_id`). The root's ECDSA share must be
/// unlocked (`AppState.ecdsa.key_share`).
pub async fn start<C>(
    state: &Arc<Mutex<AppState<C>>>,
    ui_tx: &UnboundedSender<Message>,
    wallet_id: &str,
    message: Vec<u8>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let fail_now = |reason: String| {
        error!("{reason}");
        let _ = ui_tx.send(Message::SigningFailed {
            request_id: INLINE_SIGNING_ID.to_string(),
            error: reason,
        });
    };
    let Some((root, _, account)) = starlab_core::accounts::parse_child_wallet_id(wallet_id) else {
        fail_now(format!(
            "ECDSA signing: {wallet_id} is not an account wallet"
        ));
        return;
    };
    let Ok(prehash) = <[u8; 32]>::try_from(message.as_slice()) else {
        fail_now(format!(
            "ECDSA signing takes a 32-byte hash, got {} bytes",
            message.len()
        ));
        return;
    };
    let path = match starlab_core::accounts::standard_path("ethereum", account)
        .ok_or_else(|| "no Ethereum path".to_string())
        .and_then(|p| DerivationPath::parse(&p).map_err(|e| e.to_string()))
    {
        Ok(p) => p,
        Err(e) => {
            fail_now(format!("ECDSA signing: {e}"));
            return;
        }
    };

    let (epoch, timeout, proposer, self_id, session_id) = {
        let mut guard = state.lock().await;
        let Some(session) = guard.session.clone() else {
            drop(guard);
            fail_now("ECDSA signing: no active session".into());
            return;
        };
        let share = match &guard.ecdsa.key_share {
            Some((id, share)) if id == root => share.clone(),
            _ => {
                drop(guard);
                fail_now(format!(
                    "ECDSA signing: the ECDSA share of wallet {root} is not unlocked"
                ));
                return;
            }
        };
        let address = match share.ethereum_address(account) {
            Ok(a) => a,
            Err(e) => {
                drop(guard);
                fail_now(format!("ECDSA signing: {e}"));
                return;
            }
        };
        let self_id = guard.device_id.clone();
        guard.ecdsa.signing_epoch += 1;
        let epoch = guard.ecdsa.signing_epoch;
        // A new ceremony: cancel whatever ran before, forget its frames.
        guard.ecdsa.remove_worker();
        guard.ceremony_outbox.clear();
        let is_proposer = session.proposer_id == self_id;
        guard.ecdsa.signing = Some(SigningCtx {
            session_id: session.session_id.clone(),
            proposer: session.proposer_id.clone(),
            self_id: self_id.clone(),
            participants: share.participants().to_vec(),
            threshold: usize::from(share.threshold()),
            prehash,
            path,
            share,
            address,
            ready: if is_proposer {
                vec![self_id.clone()]
            } else {
                Vec::new()
            },
            signer_set: None,
            worker_started: false,
            epoch,
            ui_tx: ui_tx.clone(),
        });
        (
            epoch,
            guard.signing_timeout,
            session.proposer_id,
            self_id,
            session.session_id,
        )
    };
    info!(
        "🖊️  ECDSA signing {} for {} (proposer {})",
        session_id, wallet_id, proposer
    );
    spawn_timeout(state.clone(), epoch, timeout);

    if proposer != self_id {
        let ready = Control {
            session_id: session_id.clone(),
            signers: Vec::new(),
            signature: String::new(),
        }
        .encode(wire::SIGN_READY_PREFIX);
        super::send_texts(state, &proposer, &[ready]).await;
    }

    // Control frames that beat this node's session.
    let replay: Vec<_> = {
        let mut guard = state.lock().await;
        let (mine, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut guard.ecdsa.pending_control)
            .into_iter()
            .partition(|(_, _, c)| c.session_id == session_id);
        guard.ecdsa.pending_control = keep;
        mine
    };
    for (from, prefix, control) in replay {
        on_control(state, &from, prefix, control, None).await;
    }
    // A proposer with threshold 1 needs no READY.
    try_fix_set(state, epoch).await;
}

/// A READY / SET / DONE frame from `from`.
pub(crate) async fn on_control<C>(
    state: &Arc<Mutex<AppState<C>>>,
    from: &str,
    prefix: &'static str,
    control: Control,
    _ui_tx: Option<&UnboundedSender<Message>>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let mut guard = state.lock().await;
    let Some(ctx) = guard
        .ecdsa
        .signing
        .as_mut()
        .filter(|c| c.session_id == control.session_id)
    else {
        info!(
            "{prefix} from {from} for {} before that signing started here; buffering",
            control.session_id
        );
        let pending = &mut guard.ecdsa.pending_control;
        if pending.len() >= MAX_PENDING_CONTROL {
            pending.remove(0);
        }
        pending.push((from.to_string(), prefix, control));
        return;
    };
    let epoch = ctx.epoch;
    match prefix {
        wire::SIGN_READY_PREFIX => {
            if ctx.self_id != ctx.proposer {
                warn!("ECDSA_SIGN_READY from {from}, but we are not the proposer; ignored");
                return;
            }
            if !ctx.participants.iter().any(|p| p == from) {
                warn!("ECDSA_SIGN_READY from non-participant {from}; ignored");
                return;
            }
            if ctx.signer_set.is_none() && !ctx.ready.iter().any(|p| p == from) {
                ctx.ready.push(from.to_string());
            }
            drop(guard);
            try_fix_set(state, epoch).await;
        }
        wire::SIGN_SET_PREFIX => {
            if from != ctx.proposer {
                warn!(
                    "ECDSA_SIGN_SET from non-proposer {from} (proposer is {}); ignored",
                    ctx.proposer
                );
                return;
            }
            if let Err(e) = validate_set(ctx, &control.signers) {
                fail(&mut guard, format!("ECDSA_SIGN_SET from {from}: {e}"));
                return;
            }
            match &ctx.signer_set {
                Some(existing) if same_set(existing, &control.signers) => return,
                Some(existing) => {
                    let e = format!(
                        "ECDSA_SIGN_SET {:?} conflicts with the fixed set {existing:?}",
                        control.signers
                    );
                    fail(&mut guard, e);
                    return;
                }
                None => {}
            }
            info!("🔒 ECDSA signer set (from {from}): {:?}", control.signers);
            let member = control.signers.contains(&ctx.self_id);
            ctx.signer_set = Some(control.signers);
            drop(guard);
            if member {
                start_worker(state, epoch).await;
            } else {
                info!("not in the ECDSA signer set; waiting for the proposer's signature");
            }
        }
        wire::SIGN_DONE_PREFIX => {
            if from != ctx.proposer {
                warn!("ECDSA_SIGN_DONE from non-proposer {from}; ignored");
                return;
            }
            let signature = match hex::decode(&control.signature) {
                Ok(s) => s,
                Err(e) => {
                    fail(&mut guard, format!("ECDSA_SIGN_DONE: bad hex: {e}"));
                    return;
                }
            };
            match starlab_core::accounts::recover_ethereum_address(&ctx.prehash, &signature) {
                Ok(addr) if addr == ctx.address => {}
                Ok(addr) => {
                    let e = format!(
                        "ECDSA_SIGN_DONE: signature recovers to {addr}, expected {}",
                        ctx.address
                    );
                    fail(&mut guard, e);
                    return;
                }
                Err(e) => {
                    fail(&mut guard, format!("ECDSA_SIGN_DONE: {e}"));
                    return;
                }
            }
            let ctx = guard.ecdsa.signing.take().expect("checked above");
            guard.ecdsa.remove_worker();
            guard.ceremony_outbox.clear();
            drop(guard);
            info!("🎉 ECDSA signature received from the proposer");
            let _ = ctx.ui_tx.send(Message::SigningComplete {
                request_id: INLINE_SIGNING_ID.to_string(),
                message: ctx.prehash.to_vec(),
                signature,
            });
        }
        _ => {}
    }
}

fn same_set(a: &[String], b: &[String]) -> bool {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort();
    b.sort();
    a == b
}

fn validate_set(ctx: &SigningCtx, set: &[String]) -> Result<(), String> {
    let mut unique = set.to_vec();
    unique.sort();
    unique.dedup();
    if unique.len() != set.len() {
        return Err(format!("duplicate signer in {set:?}"));
    }
    if set.len() != ctx.threshold {
        return Err(format!(
            "{} signers, the wallet's threshold is {}",
            set.len(),
            ctx.threshold
        ));
    }
    if !set.contains(&ctx.proposer) {
        return Err(format!("set {set:?} does not include the proposer"));
    }
    if let Some(unknown) = set.iter().find(|d| !ctx.participants.contains(d)) {
        return Err(format!("unknown device {unknown}"));
    }
    Ok(())
}

/// Proposer: once `threshold` nodes are ready, fix the set, announce it and
/// start signing. Idempotent.
async fn try_fix_set<C>(state: &Arc<Mutex<AppState<C>>>, epoch: u64)
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let (set_frame, peers) = {
        let mut guard = state.lock().await;
        let Some(ctx) = guard.ecdsa.signing.as_mut().filter(|c| c.epoch == epoch) else {
            return;
        };
        if ctx.self_id != ctx.proposer
            || ctx.signer_set.is_some()
            || ctx.ready.len() < ctx.threshold
        {
            return;
        }
        let set: Vec<String> = ctx.ready.iter().take(ctx.threshold).cloned().collect();
        info!("🔒 ECDSA signer set fixed (proposer): {:?}", set);
        ctx.signer_set = Some(set.clone());
        let frame = Control {
            session_id: ctx.session_id.clone(),
            signers: set,
            signature: String::new(),
        }
        .encode(wire::SIGN_SET_PREFIX);
        let peers: Vec<String> = ctx
            .participants
            .iter()
            .filter(|p| **p != ctx.self_id)
            .cloned()
            .collect();
        let peers: Vec<String> = peers.into_iter().filter(|p| guard.is_online(p)).collect();
        (frame, peers)
    };
    for peer in &peers {
        super::send_texts(state, peer, std::slice::from_ref(&set_frame)).await;
    }
    start_worker(state, epoch).await;
}

async fn start_worker<C>(state: &Arc<Mutex<AppState<C>>>, epoch: u64)
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let (out_rx, done_rx) = {
        let mut guard = state.lock().await;
        let Some(ctx) = guard.ecdsa.signing.as_mut().filter(|c| c.epoch == epoch) else {
            return;
        };
        if ctx.worker_started {
            return;
        }
        ctx.worker_started = true;
        let set = ctx.signer_set.clone().unwrap_or_default();
        let signers: Vec<u16> = set
            .iter()
            .filter_map(|d| ctx.participants.iter().position(|p| p == d))
            .filter_map(|i| u16::try_from(i).ok())
            .collect();
        let job = Job::Sign {
            session_id: ctx.session_id.clone(),
            share: Box::new(ctx.share.clone()),
            signers,
            path: ctx.path.clone(),
            prehash: ctx.prehash,
        };
        let (out_tx, out_rx) = unbounded_channel();
        let (status_tx, _status_rx) = unbounded_channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let timeout = guard.signing_timeout;
        match worker::spawn(job, timeout, out_tx, status_tx, done_tx) {
            Ok(inbox) => guard.ecdsa.install_worker(inbox),
            Err(e) => {
                fail(&mut guard, format!("ECDSA signing: worker thread: {e}"));
                return;
            }
        }
        (out_rx, done_rx)
    };
    super::spawn_transport(state.clone(), out_rx);
    let state = state.clone();
    tokio::spawn(async move {
        let result = done_rx
            .await
            .unwrap_or_else(|_| Err("ECDSA worker vanished".into()));
        finish(&state, epoch, result).await;
    });
}

async fn finish<C>(state: &Arc<Mutex<AppState<C>>>, epoch: u64, result: Result<Outcome, String>)
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let mut guard = state.lock().await;
    if guard.ecdsa.signing.as_ref().map(|c| c.epoch) != Some(epoch) {
        return; // timed out / superseded already
    }
    let sig = match result {
        Ok(Outcome::Sign(sig)) => sig,
        Ok(Outcome::Dkg(_)) => unreachable!("a signing job yields a signature"),
        Err(e) => {
            fail(&mut guard, format!("ECDSA signing failed: {e}"));
            return;
        }
    };
    let ctx = guard.ecdsa.signing.take().expect("checked above");
    guard.ecdsa.remove_worker();
    guard.ceremony_outbox.clear();
    let signature = ethereum_signature_bytes(&sig);
    // The proposer tells the joined nodes it left out.
    let outsiders: Vec<String> = if ctx.self_id == ctx.proposer {
        let set = ctx.signer_set.clone().unwrap_or_default();
        ctx.participants
            .iter()
            .filter(|p| !set.contains(p) && guard.is_online(p))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    drop(guard);
    info!(
        "🎉 ECDSA signature for {}: {}",
        ctx.address,
        hex::encode(&signature)
    );
    if !outsiders.is_empty() {
        let done = Control {
            session_id: ctx.session_id.clone(),
            signers: Vec::new(),
            signature: hex::encode(&signature),
        }
        .encode(wire::SIGN_DONE_PREFIX);
        for peer in &outsiders {
            super::send_texts(state, peer, std::slice::from_ref(&done)).await;
        }
    }
    crate::protocal::signing::withdraw_completed_invite(state).await;
    let _ = ctx.ui_tx.send(Message::SigningComplete {
        request_id: INLINE_SIGNING_ID.to_string(),
        message: ctx.prehash.to_vec(),
        signature,
    });
}

fn spawn_timeout<C>(state: Arc<Mutex<AppState<C>>>, epoch: u64, timeout: std::time::Duration)
where
    C: Ciphersuite + Send + Sync + 'static,
{
    tokio::spawn(async move {
        tokio::time::sleep(timeout).await;
        let mut guard = state.lock().await;
        let Some(ctx) = guard.ecdsa.signing.as_ref().filter(|c| c.epoch == epoch) else {
            return;
        };
        let secs = timeout.as_secs();
        let reason = match &ctx.signer_set {
            None => format!(
                "ECDSA signing timed out after {secs}s: only {} of threshold {} signers ready",
                ctx.ready.len().max(1),
                ctx.threshold
            ),
            Some(set) => {
                format!("ECDSA signing timed out after {secs}s: the signers {set:?} did not finish")
            }
        };
        fail(&mut guard, reason);
    });
}

fn fail<C: Ciphersuite>(guard: &mut AppState<C>, reason: String) {
    error!("{reason}");
    guard.ecdsa.remove_worker();
    guard.ceremony_outbox.clear();
    if let Some(ctx) = guard.ecdsa.signing.take() {
        let _ = ctx.ui_tx.send(Message::SigningFailed {
            request_id: INLINE_SIGNING_ID.to_string(),
            error: reason,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ethereum_children_sign_with_ecdsa() {
        assert!(is_ecdsa_wallet("19caa3cf46d3-ethereum-0"));
        assert!(is_ecdsa_wallet("19caa3cf46d3-eth-3"));
        assert!(!is_ecdsa_wallet("19caa3cf46d3-bitcoin-0"));
        assert!(!is_ecdsa_wallet("19caa3cf46d3-solana-0"));
        assert!(!is_ecdsa_wallet("19caa3cf46d3"));
    }

    #[test]
    fn signature_bytes_use_v_27_28() {
        let sig = EcdsaSignature {
            r: [1; 32],
            s: [2; 32],
            recovery_id: 1,
        };
        let bytes = ethereum_signature_bytes(&sig);
        assert_eq!(bytes.len(), 65);
        assert_eq!(&bytes[..32], &[1; 32]);
        assert_eq!(&bytes[32..64], &[2; 32]);
        assert_eq!(bytes[64], 28);
    }
}
