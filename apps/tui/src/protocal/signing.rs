//! FROST Threshold Signing — Protocol Layer
//!
//! Structure mirrors `protocal/dkg.rs`: async handlers driven by Commands,
//! and a SigningState tracked on `AppState<C>`. The round-1 (commit) and
//! round-2 (sign) FROST functions are applied locally; peer artifacts flow
//! over the existing WebRTC mesh using `SIGN_COMMIT:<b64>`, `SIGN_SHARE:<b64>`
//! and `SIGN_SET:<b64>` prefixes on `WebRTCMessage::SimpleMessage`.
//!
//! With MORE than `threshold` signers online, every node accumulating
//! whatever threshold-many commitments it happened to see first (the old
//! rule) let different nodes pick different signer sets, so aggregation
//! failed with `UnknownIdentifier`. The signer set is now fixed by exactly
//! one node — the proposer (`session.proposer_id`) — and broadcast to
//! everyone else before Round 2 runs anywhere:
//!
//! 1. The **proposer** fixes the set (itself + the first threshold-1
//!    commitments it received — today's first-come rule, but confined to
//!    the proposer) the moment it has threshold-many commitments, THEN
//!    broadcasts `SIGN_SET` to every other participant, THEN runs Round 2.
//! 2. A **non-proposer** never picks a set. It runs Round 2 only once it
//!    has received `SIGN_SET` from the proposer, is listed in it, and holds
//!    every set member's commitment (own included). A node NOT in the set
//!    never signs.
//! 3. **Every** node — set member or not — aggregates once it holds the
//!    `SIGN_SHARE` of every set member, built from exactly the set's
//!    commitments + shares, so every node (including one that never
//!    signed) converges on the identical signature.
//!
//! Public surface:
//! - [`handle_start_signing`]: kickoff. Runs `round1::commit` on this
//!   node, stashes the nonces, broadcasts the commitment.
//! - [`process_signing_round1`]: peer commitment arrived. Accumulate; the
//!   proposer may now have enough to fix the signer set (see above).
//! - [`process_signing_set`]: the proposer's `SIGN_SET` arrived (or a
//!   buffered SIGN_COMMIT/SIGN_SET unblocked Round 2 once applied).
//!   Validate it, stash it, and try to advance.
//! - [`process_signing_round2`]: peer share arrived. Accumulate; try to
//!   aggregate.
//!
//! All error paths transition `SigningState = Failed { reason }` and
//! emit `Message::SigningFailed`. There are no panics in this module —
//! a kill-the-task FROST error must not take down the tokio runtime.

use crate::elm::message::Message;
use crate::protocal::dkg::canonical_identifier;
use crate::protocal::signal::WebRTCMessage;
use crate::utils::appstate_compat::AppState;
use crate::utils::state::SigningState;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use frost_core::Ciphersuite;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info, warn};

/// Prefix markers for the signing-related frames on the SimpleMessage
/// channel. The inbound dispatcher in `network/webrtc.rs` strip-prefixes
/// against these constants; keep them in sync.
pub const SIGN_COMMIT_PREFIX: &str = "SIGN_COMMIT:";
pub const SIGN_SHARE_PREFIX: &str = "SIGN_SHARE:";
/// The proposer-fixed signer set: `SIGN_SET:` + standard base64 (with
/// padding) of the UTF-8 JSON array of device-id strings, e.g.
/// `SIGN_SET:` + base64(`["ext-1","cli-2"]`). The browser extension
/// implements the exact same wire format — order doesn't matter (receivers
/// treat it as a set), but the encoding must match byte-for-byte.
pub const SIGN_SET_PREFIX: &str = "SIGN_SET:";

/// Synthetic signing-request id for the "sign a message right now" flow
/// we implement in Phase C. The real pending-signing-request queue is a
/// Phase E concern; until then every ceremony uses this single id, which
/// lets the UI match SigningComplete emissions to the active sign screen
/// without a queue lookup.
pub(crate) const INLINE_SIGNING_ID: &str = "inline";

/// Default per-ceremony signing timeout. If a signer the proposer picked
/// (SIGN_SET) goes offline before its SIGN_SHARE — or too few shareholders
/// ever commit — every node would otherwise sit in
/// `CommitmentPhase`/`SharePhase` forever: automatic re-picking inside the
/// SAME ceremony is unsafe (a signer that already produced a share must
/// never sign a different package with the same nonces — FROST nonce reuse
/// leaks the key). So instead we fail cleanly via `fail_and_notify` once this
/// much time has passed and let the caller retry with a fresh ceremony.
/// Overridable per-node via `AppState::signing_timeout` (e.g. for tests).
pub const SIGNING_TIMEOUT: Duration = Duration::from_secs(120);

/// `SIGNING_TIMEOUT`, overridable via the `STARLAB_SIGNING_TIMEOUT_MS`
/// environment variable — test/e2e use only, so a stuck-ceremony test isn't
/// stuck itself for the real 120s. Read once per `AppState` construction
/// (`AppState::new` / `with_device_id_and_server`), mirroring the existing
/// `PERF_MONITORING` env-flag pattern in `utils::performance`.
pub fn default_signing_timeout() -> Duration {
    std::env::var("STARLAB_SIGNING_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(SIGNING_TIMEOUT)
}

// -----------------------------------------------------------------
// handle_start_signing — entry point, runs Round 1 + broadcasts
// -----------------------------------------------------------------

/// Begin a new signing ceremony on this node. Must be called exactly
/// once per ceremony per node — downstream `process_signing_round1` /
/// `process_signing_round2` invocations rely on the Round-1 nonces stashed
/// here.
///
/// Preconditions:
/// - `AppState.key_package` is `Some(_)` (Stage C.1's `UnlockWallet`
///   populates this; callers must sequence properly)
/// - `AppState.public_key_package` is `Some(_)`
/// - `AppState.session` is `Some(_)` with the signing quorum
///
/// Any precondition miss transitions `signing_state = Failed { reason }`
/// and emits `Message::SigningFailed` so the UI can show a modal.
pub async fn handle_start_signing<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    message: Vec<u8>,
    ui_tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    info!(
        "🖊️  Starting signing ceremony on {} for a {}-byte message",
        self_device_id,
        message.len()
    );

    // ---- Preconditions in one short lock
    let (key_package, session, my_identifier, epoch, timeout) = {
        let mut guard = state.lock().await;

        let Some(kp) = guard.key_package.clone() else {
            let err = "handle_start_signing: no key_package — unlock the wallet first".to_string();
            fail_and_notify(&mut guard, &ui_tx, err);
            return;
        };

        let Some(session) = guard.session.clone() else {
            let err = "handle_start_signing: no active session on AppState".to_string();
            fail_and_notify(&mut guard, &ui_tx, err);
            return;
        };

        let my_id = match canonical_identifier::<C>(&session.participants, &self_device_id) {
            Some(id) => id,
            None => {
                let err = format!(
                    "handle_start_signing: device_id {} not in session.participants {:?}",
                    self_device_id, session.participants
                );
                fail_and_notify(&mut guard, &ui_tx, err);
                return;
            }
        };

        // Reset transient state from a prior ceremony so an aborted
        // attempt doesn't contaminate this one. Caveat on `frost_commitments`,
        // `frost_signature_shares` AND `signer_set`: we deliberately do NOT
        // clear these here. Reason: `AppState.session` survives from the
        // prior DKG (or a prior signing) with the SAME participants/threshold,
        // so a peer's SIGN_COMMIT / SIGN_SET / SIGN_SHARE for THIS ceremony
        // can be accepted and applied (by `process_signing_round1` /
        // `process_signing_set` / `process_signing_round2`) the moment it
        // arrives — `session.is_none()` is false, so nothing buffers it —
        // even if that's BEFORE this node has locally reached
        // `handle_start_signing`. Clearing here would drop whatever already
        // landed and stall the ceremony (a real interop race: a fast
        // proposer + fast co-signer can finish Round 2 and broadcast SIGN_SET
        // / SIGN_SHARE before a slower third participant even starts).
        // `frost_commitments`/`frost_signature_shares` inserts are keyed by
        // `Identifier<C>`, so stale entries from a previous ceremony with the
        // same participants get overwritten by the fresh ones rather than
        // accumulated, and `signer_set` is `Option`-guarded (only ever
        // written once, by the proposer or by a validated SIGN_SET) with the
        // post-aggregate signature verification as the safety net against a
        // truly stale value slipping through. If a prior ceremony had
        // DIFFERENT participants you'd get cross-contamination, but we don't
        // support concurrent ceremonies this phase.
        // Identify THIS ceremony so a timer armed for a prior one (aborted or
        // already complete) can never fail this one — see `signing_epoch` /
        // the timeout task spawned below.
        guard.signing_epoch = guard.signing_epoch.wrapping_add(1);
        let epoch = guard.signing_epoch;
        let timeout = guard.signing_timeout;

        guard.frost_nonces = None;
        guard.signing_message = Some(message.clone());
        guard.ceremony_outbox.clear();
        // Arrival order always restarts with self (inserted into
        // `frost_commitments` right below) — only the proposer (who always
        // calls this BEFORE fixing any set) reads it, so no early-arrival
        // race applies here.
        guard.commitment_arrival_order = vec![self_device_id.clone()];
        guard.signing_state = SigningState::CommitmentPhase {
            signing_id: INLINE_SIGNING_ID.to_string(),
            transaction_data: format!("{} bytes", message.len()),
            selected_signers: Vec::new(),
            commitments: BTreeMap::new(),
            own_commitment: None,
            nonces: None,
            blockchain: String::new(),
            chain_id: None,
        };

        (kp, session, my_id, epoch, timeout)
    };

    // Arm the per-ceremony timeout now that the ceremony has actually
    // started (every early-return above bails before this point, so a
    // failed precondition never arms a timer for a ceremony that never ran).
    spawn_signing_timeout(state.clone(), ui_tx.clone(), epoch, timeout);

    // ---- FROST Round 1: commit
    let mut rng = starlab_core::rng::os_rng();
    let (nonces, commitments) = frost_core::round1::commit(key_package.signing_share(), &mut rng);

    let commitment_bytes = match commitments.serialize() {
        Ok(b) => b,
        Err(e) => {
            let err = format!("SigningCommitments::serialize: {:?}", e);
            let mut guard = state.lock().await;
            fail_and_notify(&mut guard, &ui_tx, err);
            return;
        }
    };

    // Stash our own nonces+commitment. Insert our commitment into the
    // accumulator so `process_signing_round1` can count toward threshold
    // without a special "did I already include myself" check.
    {
        let mut guard = state.lock().await;
        guard.frost_nonces = Some(nonces);
        guard.frost_commitments.insert(my_identifier, commitments);
    }

    // ---- Broadcast SIGN_COMMIT:<b64>
    broadcast_signing_frame(
        &state,
        &session.participants,
        &self_device_id,
        SIGN_COMMIT_PREFIX,
        &commitment_bytes,
    )
    .await;

    // Re-feed any peer SIGN_COMMITs that arrived before our session existed
    // (the cold-start race): now that the session AND our own nonces are in
    // place, they can be decoded, counted toward threshold, and may complete
    // Round 1. No-op (empty buffer) on the warm path.
    drain_pre_session_commitments::<C>(state.clone(), self_device_id.clone(), ui_tx.clone()).await;
    // Same for a `SIGN_SET` that beat this node's session into existence
    // (a cold-started co-signer, the same race `pending_pre_session_commitments`
    // guards against).
    drain_pre_session_signer_set::<C>(state.clone(), self_device_id.clone(), ui_tx.clone()).await;

    // Check the threshold-reached edge here too — a 2-of-3 wallet where
    // only this node signs would otherwise sit on its own commitment
    // forever. `try_advance_to_round2` runs Round 2 locally if we
    // already have threshold-many commitments (including our own).
    try_advance_to_round2::<C>(&state, &ui_tx, &self_device_id).await;
    // A node outside the fixed signer set never runs Round 2, so the only
    // thing that can trigger ITS aggregation is a peer's SIGN_COMMIT / SIGN_SET
    // / SIGN_SHARE arriving — which, on a warm `AppState.session` (reused from
    // a prior DKG/signing), can happen well BEFORE this node calls
    // `handle_start_signing` (nothing buffers it; `session.is_none()` is
    // false). Every such early arrival's `try_aggregate` call silently no-ops
    // because `signing_message` — set only by THIS function, right above —
    // wasn't there yet. Retry once now that it is: if every set member's
    // commitment + share already arrived, this is what actually completes
    // the ceremony for a late-starting non-member.
    try_aggregate::<C>(&state, &ui_tx).await;
}

/// Re-feed `SIGN_COMMIT`s that arrived before this node had a signing session
/// (see `AppState::pending_pre_session_commitments`). Called once the session
/// is established. Drains the buffer and runs each commit through the normal
/// `process_signing_round1` path (which now resolves the sender Identifier).
pub async fn drain_pre_session_commitments<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    ui_tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let buffered: Vec<(String, Vec<u8>)> = {
        let mut guard = state.lock().await;
        std::mem::take(&mut guard.pending_pre_session_commitments)
    };
    if buffered.is_empty() {
        return;
    }
    info!(
        "signing: re-feeding {} buffered pre-session SIGN_COMMIT(s)",
        buffered.len()
    );
    for (from, bytes) in buffered {
        process_signing_round1(
            state.clone(),
            self_device_id.clone(),
            from,
            bytes,
            ui_tx.clone(),
        )
        .await;
    }
}

/// Re-feed a `SIGN_SET` that arrived before this node had a signing session
/// (see `AppState::pending_pre_session_signer_set`). Called once the session
/// is established. Mirrors `drain_pre_session_commitments`.
pub async fn drain_pre_session_signer_set<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    ui_tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let buffered: Vec<(String, Vec<u8>)> = {
        let mut guard = state.lock().await;
        std::mem::take(&mut guard.pending_pre_session_signer_set)
    };
    if buffered.is_empty() {
        return;
    }
    info!(
        "signing: re-feeding {} buffered pre-session SIGN_SET(s)",
        buffered.len()
    );
    for (from, bytes) in buffered {
        process_signing_set(
            state.clone(),
            self_device_id.clone(),
            from,
            bytes,
            ui_tx.clone(),
        )
        .await;
    }
}

// -----------------------------------------------------------------
// process_signing_round1 — peer commitments arrive here
// -----------------------------------------------------------------

pub async fn process_signing_round1<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    from_device_id: String,
    commitment_bytes: Vec<u8>,
    ui_tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    info!(
        "📥 Received SIGN_COMMIT from {} ({} bytes)",
        from_device_id,
        commitment_bytes.len()
    );

    // Resolve the sender's canonical identifier and decode the commitment
    // in one short lock.
    let decode_result = {
        let mut guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            // The commit beat our own session setup (a cold-started co-signer
            // whose JoinSigning hasn't run yet). Buffer the raw bytes — we
            // can't resolve the sender's Identifier without a participants
            // list — and re-feed via `drain_pre_session_commitments` once the
            // session exists. Dropping here is what stalled cold-start signing.
            info!(
                "SIGN_COMMIT from {} before session ready; buffering",
                from_device_id
            );
            guard
                .pending_pre_session_commitments
                .push((from_device_id.clone(), commitment_bytes.clone()));
            return;
        };
        let sender_id = match canonical_identifier::<C>(&session.participants, &from_device_id) {
            Some(id) => id,
            None => {
                warn!(
                    "SIGN_COMMIT from unknown device {} (session.participants={:?}); dropping",
                    from_device_id, session.participants
                );
                return;
            }
        };
        let decoded =
            match frost_core::round1::SigningCommitments::<C>::deserialize(&commitment_bytes) {
                Ok(c) => c,
                Err(e) => {
                    let err = format!(
                        "process_signing_round1: SigningCommitments::deserialize from {}: {:?}",
                        from_device_id, e
                    );
                    drop(guard);
                    let mut g = state.lock().await;
                    fail_and_notify(&mut g, &ui_tx, err);
                    return;
                }
            };
        (sender_id, decoded)
    };

    // Insert into the accumulator. Also record arrival order (dedup: a
    // resend of the exact same frame never reaches here twice — the
    // `SeenFrames` de-dup in `network::webrtc` catches that — but a
    // never-fixed proposer might still see the sender again after a
    // buffered replay, so guard anyway).
    {
        let mut guard = state.lock().await;
        guard
            .frost_commitments
            .insert(decode_result.0, decode_result.1);
        if !guard.commitment_arrival_order.contains(&from_device_id) {
            guard.commitment_arrival_order.push(from_device_id.clone());
        }
    }

    try_advance_to_round2::<C>(&state, &ui_tx, &self_device_id).await;
    // A commitment can be the last piece a non-member node needed to build
    // the SIGN_SET signer set's SigningPackage for aggregation (shares may
    // have already arrived).
    try_aggregate::<C>(&state, &ui_tx).await;
}

/// Two jobs, run in order:
///
/// 1. **Proposer only, once**: the moment `frost_commitments` crosses
///    `threshold`, fix the signer set to itself + the first threshold-1
///    arrivals (`commitment_arrival_order` — today's first-come rule,
///    but confined to the proposer) and broadcast `SIGN_SET` to every
///    other participant BEFORE doing anything else this call.
/// 2. **Every set member**: once `signer_set` is populated (fixed just
///    above if we're the proposer, or received via `process_signing_set`
///    otherwise) and we hold the commitment of every member (own
///    included), derive our Round-2 share and broadcast it.
///
/// A node not listed in the fixed set returns after step 1 without ever
/// running Round 2 — it doesn't sign, but it still aggregates once every
/// member's share arrives (`try_aggregate`). Idempotent — if we've already
/// broadcast our share, returns without doing anything.
async fn try_advance_to_round2<C>(
    state: &Arc<Mutex<AppState<C>>>,
    ui_tx: &UnboundedSender<Message>,
    self_device_id: &str,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    // Step 1: proposer fixes the signer set, exactly once.
    let just_fixed: Option<(Vec<String>, Vec<String>)> = {
        let mut guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            return;
        };
        if session.proposer_id != self_device_id || guard.signer_set.is_some() {
            None
        } else {
            let threshold = session.threshold as usize;
            if guard.frost_commitments.len() < threshold {
                return; // not enough yet — wait for more SIGN_COMMITs
            }
            let set: Vec<String> = guard
                .commitment_arrival_order
                .iter()
                .take(threshold)
                .cloned()
                .collect();
            let participants = session.participants.clone();
            info!(
                "🔒 {} (proposer): fixed signer set {:?}",
                self_device_id, set
            );
            guard.signer_set = Some(set.iter().cloned().collect());
            Some((set, participants))
        }
    };
    if let Some((set, participants)) = just_fixed {
        let payload = match serde_json::to_vec(&set) {
            Ok(b) => b,
            Err(e) => {
                let mut g = state.lock().await;
                fail_and_notify(&mut g, ui_tx, format!("SIGN_SET serialize: {e}"));
                return;
            }
        };
        broadcast_signing_frame(
            state,
            &participants,
            self_device_id,
            SIGN_SET_PREFIX,
            &payload,
        )
        .await;
    }

    // Step 2: run Round 2 once we hold every signer-set member's commitment.
    let (commitments_for_pkg, message, nonces, key_package, my_id, participants) = {
        let guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            return;
        };
        let Some(signer_set) = guard.signer_set.as_ref() else {
            return; // non-proposer still waiting on SIGN_SET
        };
        if !signer_set.contains(self_device_id) {
            return; // not a member of the fixed set — never signs
        }
        let Some(key_package) = guard.key_package.as_ref() else {
            warn!("try_advance_to_round2: no key_package — cannot sign, bailing");
            return;
        };
        let my_id = match canonical_identifier::<C>(&session.participants, self_device_id) {
            Some(id) => id,
            None => return,
        };
        // If we've already produced a share, don't re-enter Round 2.
        if guard.frost_signature_shares.contains_key(&my_id) {
            return;
        }
        // The SigningPackage must use EXACTLY the set's commitments — never
        // whatever else `frost_commitments` accumulated from signers outside
        // the set.
        let mut commitments_for_pkg = BTreeMap::new();
        for device in signer_set {
            let Some(id) = canonical_identifier::<C>(&session.participants, device) else {
                // SIGN_SET is validated against session.participants before
                // being stored (see `process_signing_set`) — unreachable.
                warn!("signer_set member {} not in session.participants", device);
                return;
            };
            let Some(commitment) = guard.frost_commitments.get(&id) else {
                return; // still waiting on this member's SIGN_COMMIT
            };
            commitments_for_pkg.insert(id, *commitment);
        }
        let Some(nonces) = guard.frost_nonces.clone() else {
            warn!("try_advance_to_round2: frost_nonces is None — did Round 1 run?");
            return;
        };
        let Some(message) = guard.signing_message.clone() else {
            warn!("try_advance_to_round2: signing_message is None; dropping");
            return;
        };

        (
            commitments_for_pkg,
            message,
            nonces,
            key_package.clone(),
            my_id,
            session.participants.clone(),
        )
    };

    info!(
        "✅ {}: have every signer-set member's commitment, running Round 2",
        self_device_id
    );

    let signing_package = frost_core::SigningPackage::new(commitments_for_pkg, &message);

    let share = match frost_core::round2::sign(&signing_package, &nonces, &key_package) {
        Ok(s) => s,
        Err(e) => {
            let err = format!("frost_core::round2::sign: {:?}", e);
            let mut g = state.lock().await;
            fail_and_notify(&mut g, ui_tx, err);
            return;
        }
    };

    // `SignatureShare::serialize()` returns `Vec<u8>` directly (infallible,
    // unlike `Signature::serialize()` which returns Result). No match.
    let share_bytes = share.serialize();

    {
        let mut guard = state.lock().await;
        guard.frost_signature_shares.insert(my_id, share);
    }

    broadcast_signing_frame(
        state,
        &participants,
        self_device_id,
        SIGN_SHARE_PREFIX,
        &share_bytes,
    )
    .await;

    // Advance the FSM for the UI.
    {
        let mut guard = state.lock().await;
        guard.signing_state = SigningState::SharePhase {
            signing_id: INLINE_SIGNING_ID.to_string(),
            transaction_data: format!("{} bytes", message.len()),
            selected_signers: Vec::new(),
            signing_package: Some(signing_package),
            shares: guard.frost_signature_shares.clone(),
            own_share: Some(guard.frost_signature_shares[&my_id]),
            blockchain: String::new(),
            chain_id: None,
        };
    }

    // In 1-of-N degenerate quorums we're already done.
    try_aggregate::<C>(state, ui_tx).await;
}

// -----------------------------------------------------------------
// process_signing_round2 — peer signature shares arrive here
// -----------------------------------------------------------------

pub async fn process_signing_round2<C>(
    state: Arc<Mutex<AppState<C>>>,
    from_device_id: String,
    share_bytes: Vec<u8>,
    ui_tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    info!(
        "📥 Received SIGN_SHARE from {} ({} bytes)",
        from_device_id,
        share_bytes.len()
    );

    let (sender_id, share) = {
        let guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            warn!(
                "SIGN_SHARE from {} but no active session; dropping",
                from_device_id
            );
            return;
        };
        let sender_id = match canonical_identifier::<C>(&session.participants, &from_device_id) {
            Some(id) => id,
            None => {
                warn!(
                    "SIGN_SHARE from unknown device {}; dropping",
                    from_device_id
                );
                return;
            }
        };
        let share = match frost_core::round2::SignatureShare::<C>::deserialize(&share_bytes) {
            Ok(s) => s,
            Err(e) => {
                let err = format!(
                    "process_signing_round2: SignatureShare::deserialize from {}: {:?}",
                    from_device_id, e
                );
                drop(guard);
                let mut g = state.lock().await;
                fail_and_notify(&mut g, &ui_tx, err);
                return;
            }
        };
        (sender_id, share)
    };

    {
        let mut guard = state.lock().await;
        guard.frost_signature_shares.insert(sender_id, share);
    }

    try_aggregate::<C>(&state, &ui_tx).await;
}

// -----------------------------------------------------------------
// process_signing_set — the proposer's fixed signer set arrives here
// -----------------------------------------------------------------

/// Decode a `SIGN_SET` payload: standard base64 (with padding, already
/// stripped by the caller) of the UTF-8 JSON array of device-id strings.
/// Exposed for the wire-format unit tests below.
pub fn decode_signer_set(payload: &[u8]) -> Result<Vec<String>, String> {
    serde_json::from_slice::<Vec<String>>(payload)
        .map_err(|e| format!("SIGN_SET payload is not a JSON string array: {e}"))
}

/// Encode the full `SIGN_SET:<b64>` wire frame — order as given (receivers
/// treat it as a set). Production code never needs the full frame string
/// (`broadcast_signing_frame` does the equivalent base64(payload) step
/// itself given just the JSON bytes); this exists for the wire-format test.
#[cfg(test)]
fn encode_signer_set_frame(device_ids: &[String]) -> String {
    let json = serde_json::to_vec(device_ids).expect("Vec<String> always serializes");
    format!("{SIGN_SET_PREFIX}{}", BASE64.encode(json))
}

/// Received the proposer's fixed signer set (device ids, order doesn't
/// matter). Only `session.proposer_id` may send this frame — anyone else's
/// is ignored with a warning, never treated as an error (a stale co-signer
/// guessing at the protocol isn't a ceremony-ending event). An invalid set
/// from the real proposer (unknown device, missing the proposer itself, or
/// the wrong size) DOES fail the ceremony: every other node validates the
/// exact same way, so a mismatch here is a real protocol bug, not a
/// transient race.
pub async fn process_signing_set<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    from_device_id: String,
    signer_set_bytes: Vec<u8>,
    ui_tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    info!(
        "📥 Received SIGN_SET from {} ({} bytes)",
        from_device_id,
        signer_set_bytes.len()
    );

    // Cold-start race: the session doesn't exist yet, so we can't check
    // `from_device_id` against `session.proposer_id` yet. Buffer the raw
    // bytes and re-feed via `drain_pre_session_signer_set` once the session
    // exists — mirrors `pending_pre_session_commitments`.
    let (proposer_id, participants, threshold) = {
        let mut guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            info!(
                "SIGN_SET from {} before session ready; buffering",
                from_device_id
            );
            guard
                .pending_pre_session_signer_set
                .push((from_device_id.clone(), signer_set_bytes.clone()));
            return;
        };
        (
            session.proposer_id.clone(),
            session.participants.clone(),
            session.threshold as usize,
        )
    };

    if from_device_id != proposer_id {
        warn!(
            "SIGN_SET from non-proposer {} (proposer is {}); ignoring",
            from_device_id, proposer_id
        );
        return;
    }

    let device_ids = match decode_signer_set(&signer_set_bytes) {
        Ok(ids) => ids,
        Err(e) => {
            let mut g = state.lock().await;
            fail_and_notify(&mut g, &ui_tx, format!("process_signing_set: {e}"));
            return;
        }
    };
    let set: std::collections::BTreeSet<String> = device_ids.into_iter().collect();

    if set.len() != threshold {
        let err = format!(
            "SIGN_SET from proposer {} has {} member(s), expected threshold {}: {:?}",
            proposer_id,
            set.len(),
            threshold,
            set
        );
        let mut g = state.lock().await;
        fail_and_notify(&mut g, &ui_tx, err);
        return;
    }
    if !set.contains(&proposer_id) {
        let err = format!(
            "SIGN_SET from proposer {proposer_id} does not include the proposer itself: {set:?}"
        );
        let mut g = state.lock().await;
        fail_and_notify(&mut g, &ui_tx, err);
        return;
    }
    if let Some(unknown) = set.iter().find(|d| !participants.contains(d)) {
        let err = format!("SIGN_SET from proposer {proposer_id} lists unknown device {unknown}");
        let mut g = state.lock().await;
        fail_and_notify(&mut g, &ui_tx, err);
        return;
    }

    {
        let mut guard = state.lock().await;
        match &guard.signer_set {
            Some(existing) if existing == &set => return, // resend; already applied
            Some(existing) => {
                let err = format!(
                    "SIGN_SET from proposer {proposer_id} ({set:?}) conflicts with the \
                     already-fixed signer set ({existing:?})"
                );
                fail_and_notify(&mut guard, &ui_tx, err);
                return;
            }
            None => {
                info!(
                    "🔒 Fixed signer set (from proposer {}): {:?}",
                    proposer_id, set
                );
                guard.signer_set = Some(set);
            }
        }
    }

    try_advance_to_round2::<C>(&state, &ui_tx, &self_device_id).await;
    try_aggregate::<C>(&state, &ui_tx).await;
}

/// If this node holds the `SIGN_SHARE` of every fixed signer-set member
/// (own included, when this node is a member) AND the signer set is known,
/// run `frost_core::aggregate` and emit `Message::SigningComplete`.
/// Idempotent. Runs on EVERY node, member or not — a node outside the set
/// never calls `round2::sign` but still collects the set's shares off the
/// mesh and aggregates them, so it converges on the identical signature.
async fn try_aggregate<C>(state: &Arc<Mutex<AppState<C>>>, ui_tx: &UnboundedSender<Message>)
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let (signing_package, shares, pubkey_package, message, signer_set_len) = {
        let guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            return;
        };
        let Some(signer_set) = guard.signer_set.as_ref() else {
            return; // no fixed set yet — nothing to aggregate against
        };
        // Guard against double-aggregate: Complete state means we already
        // emitted a signature — ignore further SIGN_SHAREs.
        if matches!(guard.signing_state, SigningState::Complete { .. }) {
            return;
        }
        let Some(pkp) = guard.public_key_package.as_ref() else {
            warn!("try_aggregate: no public_key_package; cannot aggregate");
            return;
        };
        let Some(message) = guard.signing_message.clone() else {
            return;
        };
        // Exactly the signer set's commitments + shares — never whatever
        // else `frost_commitments`/`frost_signature_shares` accumulated
        // from signers outside the fixed set.
        let mut commitments = BTreeMap::new();
        let mut shares = BTreeMap::new();
        for device in signer_set {
            let Some(id) = canonical_identifier::<C>(&session.participants, device) else {
                warn!("signer_set member {} not in session.participants", device);
                return;
            };
            let Some(commitment) = guard.frost_commitments.get(&id) else {
                return; // still waiting on this member's SIGN_COMMIT
            };
            let Some(share) = guard.frost_signature_shares.get(&id) else {
                return; // still waiting on this member's SIGN_SHARE
            };
            commitments.insert(id, *commitment);
            shares.insert(id, *share);
        }
        let pkg = frost_core::SigningPackage::new(commitments, &message);
        (pkg, shares, pkp.clone(), message, signer_set.len())
    };

    info!(
        "🧮 Have every signer-set member's share ({}), running aggregate",
        signer_set_len
    );

    let signature = match frost_core::aggregate(&signing_package, &shares, &pubkey_package) {
        Ok(sig) => sig,
        Err(e) => {
            let err = format!("frost_core::aggregate: {:?}", e);
            let mut g = state.lock().await;
            fail_and_notify(&mut g, ui_tx, err);
            return;
        }
    };

    // Sanity check: the signature should verify under the group key.
    // A failure here means some share was malformed and the aggregate
    // silently combined garbage — fail loudly before the UI claims
    // success.
    if pubkey_package
        .verifying_key()
        .verify(&message, &signature)
        .is_err()
    {
        let err = "aggregated signature failed verification under group_verifying_key".to_string();
        let mut g = state.lock().await;
        fail_and_notify(&mut g, ui_tx, err);
        return;
    }

    let signature_bytes = match signature.serialize() {
        Ok(b) => b,
        Err(e) => {
            let err = format!("Signature::serialize: {:?}", e);
            let mut g = state.lock().await;
            fail_and_notify(&mut g, ui_tx, err);
            return;
        }
    };

    {
        let mut guard = state.lock().await;
        guard.signing_state = SigningState::Complete {
            signing_id: INLINE_SIGNING_ID.to_string(),
            signature: signature_bytes.clone(),
        };
        // Free the live buffers — the SigningState::Complete variant
        // carries the only thing downstream needs (the signature bytes).
        guard.frost_commitments.clear();
        guard.frost_signature_shares.clear();
        guard.frost_nonces = None;
        guard.signing_message = None;
        guard.ceremony_outbox.clear();
        guard.signer_set = None;
        guard.commitment_arrival_order.clear();
    }

    withdraw_completed_invite(state).await;

    info!(
        "🎉 Signing complete: {}",
        hex::encode(&signature_bytes[..16.min(signature_bytes.len())])
    );
    let _ = ui_tx.send(Message::SigningComplete {
        request_id: INLINE_SIGNING_ID.to_string(),
        message: message.clone(),
        signature: signature_bytes,
    });
}

// -----------------------------------------------------------------
// signing timeout — a stuck ceremony fails cleanly instead of hanging
// -----------------------------------------------------------------

/// Arm the per-ceremony timeout armed by `handle_start_signing`. `epoch` is
/// the value `AppState.signing_epoch` was just bumped to for THIS ceremony;
/// when the timer fires it only acts if the epoch is still current AND the
/// ceremony is still stuck in `CommitmentPhase`/`SharePhase` — so a timer
/// left over from an aborted or already-completed ceremony can never fail a
/// newer one, and a ceremony that finished (or already failed) on its own is
/// left alone.
fn spawn_signing_timeout<C>(
    state: Arc<Mutex<AppState<C>>>,
    ui_tx: UnboundedSender<Message>,
    epoch: u64,
    timeout: Duration,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    tokio::spawn(async move {
        tokio::time::sleep(timeout).await;

        let mut guard = state.lock().await;
        if guard.signing_epoch != epoch {
            return; // a newer ceremony has since started; this timer is stale
        }
        if !matches!(
            guard.signing_state,
            SigningState::CommitmentPhase { .. } | SigningState::SharePhase { .. }
        ) {
            return; // already completed or failed on its own
        }

        let reason = describe_timeout_reason(&guard, timeout);
        fail_and_notify(&mut guard, &ui_tx, reason);
    });
}

/// Build a precise "what's missing" reason for a timed-out ceremony:
/// - the signer set was fixed → name the members whose `SIGN_SHARE` never
///   arrived (the proposer itself always has one the moment it fixes the
///   set, so this always names at least one peer).
/// - still waiting on Round 1 → report how many commitments arrived vs the
///   threshold (no signer set to name members from yet).
fn describe_timeout_reason<C: Ciphersuite>(guard: &AppState<C>, timeout: Duration) -> String {
    let secs = timeout.as_secs();
    let Some(session) = guard.session.as_ref() else {
        return format!("signing timed out after {secs}s: no active session");
    };
    match guard.signer_set.as_ref() {
        Some(signer_set) => {
            let missing: Vec<&str> = signer_set
                .iter()
                .filter(|device| {
                    !canonical_identifier::<C>(&session.participants, device)
                        .is_some_and(|id| guard.frost_signature_shares.contains_key(&id))
                })
                .map(String::as_str)
                .collect();
            format!(
                "signing timed out after {secs}s: no SIGN_SHARE from {}",
                missing.join(", ")
            )
        }
        None => format!(
            "signing timed out after {secs}s: only {} of threshold {} commitments arrived",
            guard.frost_commitments.len(),
            session.threshold
        ),
    }
}

// -----------------------------------------------------------------
// helpers
// -----------------------------------------------------------------

/// Retire discovery only. Keep the session and mesh alive so slower peers
/// can finish receiving their shares / final signature, including outsiders
/// in a more-than-threshold ceremony. Joiners must never alter the roster.
pub(crate) async fn withdraw_completed_invite<C: Ciphersuite>(state: &Arc<Mutex<AppState<C>>>) {
    let mut guard = state.lock().await;
    let Some(session) = guard.session.as_ref() else {
        return;
    };
    if session.proposer_id != guard.device_id {
        return;
    }
    let session_id = session.session_id.clone();
    guard.retired_signing_session_id = Some(session_id.clone());
    let Some(tx) = guard.websocket_msg_tx.as_ref() else {
        return;
    };
    let leave = starlab_signal_server::ClientMsg::LeaveSession { session_id };
    match serde_json::to_string(&leave) {
        Ok(json) => {
            if let Err(e) = tx.send(json) {
                warn!("completed signing invite withdrawal failed: {e}");
            }
        }
        Err(e) => warn!("completed signing invite serialization failed: {e}"),
    }
}

fn fail_and_notify<C: Ciphersuite>(
    guard: &mut AppState<C>,
    ui_tx: &UnboundedSender<Message>,
    reason: String,
) {
    error!("{}", reason);
    guard.signing_state = SigningState::Failed {
        signing_id: INLINE_SIGNING_ID.to_string(),
        reason: reason.clone(),
    };
    // Wipe transient buffers so a retry starts fresh.
    guard.frost_commitments.clear();
    guard.frost_signature_shares.clear();
    guard.frost_nonces = None;
    guard.signing_message = None;
    guard.ceremony_outbox.clear();
    guard.signer_set = None;
    guard.commitment_arrival_order.clear();
    let _ = ui_tx.send(Message::SigningFailed {
        request_id: INLINE_SIGNING_ID.to_string(),
        error: reason,
    });
}

/// Base64-encode `payload`, wrap as `"{prefix}{b64}"` inside a
/// `WebRTCMessage::SimpleMessage`, and push it to every participant
/// except ourselves. Reuses the same retry-heavy data-channel sender
/// as the DKG layer (`utils::device::send_webrtc_message`).
async fn broadcast_signing_frame<C>(
    state: &Arc<Mutex<AppState<C>>>,
    participants: &[String],
    self_device_id: &str,
    prefix: &str,
    payload: &[u8],
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let message = WebRTCMessage::<C>::SimpleMessage {
        text: format!("{}{}", prefix, BASE64.encode(payload)),
    };

    for device_id in participants {
        if device_id == self_device_id {
            continue;
        }
        // Signing needs only threshold-many of the participants; retrying an
        // offline one here held up the frame to every peer after it (by up to
        // 5s per round), and a one-shot signer could finish and exit first.
        if !state.lock().await.is_online(device_id) {
            info!(
                "⏭ {} is not on the signal server; skipping {}",
                device_id,
                prefix.trim_end_matches(':')
            );
            continue;
        }
        let mut retry = 0;
        const MAX_RETRIES: u32 = 10;
        const RETRY_DELAY_MS: u64 = 500;
        loop {
            match crate::utils::device::send_webrtc_message(device_id, &message, state.clone())
                .await
            {
                Ok(()) => {
                    info!("✅ Sent {} to {}", prefix.trim_end_matches(':'), device_id);
                    break;
                }
                Err(e)
                    if (e.contains("Data channel not found")
                        || e.contains("Data channel for")
                        || e.contains("is not open"))
                        && retry < MAX_RETRIES - 1 =>
                {
                    retry += 1;
                    info!(
                        "⏳ Data channel not ready for {} ({}); retry {}/{}",
                        device_id, prefix, retry, MAX_RETRIES
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                }
                Err(e) => {
                    warn!(
                        "❌ Failed to send {} to {}: {}",
                        prefix.trim_end_matches(':'),
                        device_id,
                        e
                    );
                    break;
                }
            }
        }
    }
}

// -----------------------------------------------------------------
// Integration test — 3-of-3 signing, all in-memory
// -----------------------------------------------------------------
//
// Can't exercise the full async-broadcast path here (no WebRTC, no
// tokio mesh) but we CAN verify the FROST-facing math by calling
// `round1::commit`, constructing `SigningPackage`, `round2::sign`,
// and `aggregate` exactly the way this module does. That gives us a
// regression guard against a future refactor that swaps the
// aggregation inputs.

#[cfg(test)]
mod tests {
    use frost_secp256k1_tr::{
        Identifier, Secp256K1Sha256TR,
        keys::{IdentifierList, KeyPackage as KP, PublicKeyPackage as PKP, generate_with_dealer},
    };
    use std::collections::BTreeMap;

    type KeyPkgMap = BTreeMap<Identifier, KP>;

    fn trusted_2_of_3() -> (KeyPkgMap, PKP) {
        let rng = starlab_core::rng::os_rng();
        let (shares, pkp) =
            generate_with_dealer(3, 2, IdentifierList::Default, rng).expect("keygen");
        let mut kps = KeyPkgMap::new();
        for (id, share) in shares {
            kps.insert(id, share.try_into().expect("share→KP"));
        }
        (kps, pkp)
    }

    /// An offline participant must not hold up the frame to the others.
    #[tokio::test]
    async fn broadcast_skips_participants_that_are_offline() {
        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.online_devices = Some(["me".to_string()].into_iter().collect());
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state));
        let participants = ["me".to_string(), "gone".to_string()];
        let sent = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            super::broadcast_signing_frame(&state, &participants, "me", "SIGN_COMMIT:", b"x"),
        )
        .await;
        assert!(sent.is_ok(), "broadcast waited on an offline participant");
    }

    /// The happy path this module drives: round1::commit × threshold,
    /// round2::sign × threshold, aggregate, verify.
    #[test]
    fn round_trip_signs_a_message_that_verifies() {
        let (kps, pkp) = trusted_2_of_3();
        let message = b"Phase C test vector";

        // Pick threshold-many signers (2 of 3) arbitrarily — sorted iter
        // gives us a stable pick.
        let signers: Vec<Identifier> = kps.keys().take(2).copied().collect();

        // Round 1
        let mut rng = starlab_core::rng::os_rng();
        let mut nonces_map = BTreeMap::new();
        let mut commitments_map = BTreeMap::new();
        for id in &signers {
            let (nonces, commitments) =
                frost_core::round1::commit(kps[id].signing_share(), &mut rng);
            nonces_map.insert(*id, nonces);
            commitments_map.insert(*id, commitments);
        }

        let signing_package = frost_core::SigningPackage::new(commitments_map.clone(), message);

        // Round 2
        let mut shares_map = BTreeMap::new();
        for id in &signers {
            let share = frost_core::round2::sign(&signing_package, &nonces_map[id], &kps[id])
                .expect("round2::sign");
            shares_map.insert(*id, share);
        }

        // Aggregate
        let signature =
            frost_core::aggregate::<Secp256K1Sha256TR>(&signing_package, &shares_map, &pkp)
                .expect("aggregate");

        pkp.verifying_key()
            .verify(message, &signature)
            .expect("signature must verify under group vk");
    }

    /// If one share is swapped for a different (valid-looking) one,
    /// `aggregate` must reject — we lean on this to keep the
    /// double-aggregate guard in `try_aggregate` honest.
    #[test]
    fn aggregate_rejects_wrong_share_under_group_key() {
        let (kps, pkp) = trusted_2_of_3();
        let message_a = b"message A";
        let message_b = b"message B";

        // Signers produce valid shares for message_a
        let signers: Vec<Identifier> = kps.keys().take(2).copied().collect();
        let mut rng = starlab_core::rng::os_rng();
        let mut nonces_map = BTreeMap::new();
        let mut commitments_map = BTreeMap::new();
        for id in &signers {
            let (n, c) = frost_core::round1::commit(kps[id].signing_share(), &mut rng);
            nonces_map.insert(*id, n);
            commitments_map.insert(*id, c);
        }
        let pkg_a = frost_core::SigningPackage::new(commitments_map.clone(), message_a);
        let mut shares = BTreeMap::new();
        for id in &signers {
            shares.insert(
                *id,
                frost_core::round2::sign(&pkg_a, &nonces_map[id], &kps[id]).unwrap(),
            );
        }
        // Build the wrong signing package (different message) and attempt to
        // aggregate the shares that belong to `message_a` against it.
        let pkg_b = frost_core::SigningPackage::new(commitments_map, message_b);
        let result = frost_core::aggregate::<Secp256K1Sha256TR>(&pkg_b, &shares, &pkp);
        assert!(
            result.is_err(),
            "aggregating shares for the wrong message must fail; got Ok({:?})",
            result.map(|s| s.serialize().unwrap())
        );
    }

    // -------------------------------------------------------------
    // SIGN_SET — wire format + decision-logic tests
    // -------------------------------------------------------------

    use super::{BASE64, Message, SIGN_SET_PREFIX, decode_signer_set, encode_signer_set_frame};
    use crate::protocal::dkg::canonical_identifier;
    use crate::protocal::signal::{SessionInfo, SessionType};
    use crate::utils::state::SigningState;
    use base64::Engine as _;

    fn signing_session(proposer_id: &str, participants: &[&str], threshold: u16) -> SessionInfo {
        SessionInfo {
            session_id: "sim-sign".to_string(),
            proposer_id: proposer_id.to_string(),
            total: participants.len() as u16,
            threshold,
            participants: participants.iter().map(|s| s.to_string()).collect(),
            session_type: SessionType::Signing {
                wallet_name: "w".to_string(),
                curve_type: "secp256k1".to_string(),
                blockchain: "secp256k1".to_string(),
                group_public_key: String::new(),
            },
            curve_type: "secp256k1".to_string(),
            coordination_type: "Network".to_string(),
            signing_message_hex: None,
        }
    }

    #[tokio::test]
    async fn reconnect_reannounces_live_signing_with_its_original_wire_contract() {
        use futures_util::StreamExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let received = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let mut messages = Vec::new();
            for _ in 0..2 {
                let message = ws.next().await.unwrap().unwrap();
                messages.push(
                    serde_json::from_str::<serde_json::Value>(message.to_text().unwrap()).unwrap(),
                );
            }
            messages
        });
        let (mut sink, _) = crate::elm::ws_runtime::dial(&format!("ws://{address}"))
            .await
            .unwrap();
        let mut session = signing_session("a", &["a", "b", "c"], 2);
        session.signing_message_hex = Some("4242".into());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        crate::elm::ws_runtime::send_retired_session(&mut sink, "completed-sign").await;
        crate::elm::ws_runtime::send_reannounce(&mut sink, &session, &tx).await;
        let messages = received.await.unwrap();
        assert_eq!(
            messages[0],
            serde_json::json!({"type": "leave_session", "session_id": "completed-sign"})
        );
        let info = &messages[1]["session_info"];
        assert_eq!(info["session_type"], "signing");
        assert_eq!(info["wallet_name"], "w");
        assert_eq!(info["blockchain"], "secp256k1");
        assert_eq!(info["signing_message_hex"], "4242");
    }

    #[tokio::test]
    async fn completed_invite_stays_retired_when_reconnecting_without_a_live_socket() {
        let mut guard = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        guard.device_id = "a".into();
        guard.session = Some(signing_session("a", &["a", "b", "c"], 2));
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(guard));
        // Finishing while disconnected must still retire the invitation locally.
        super::withdraw_completed_invite(&state).await;
        let params = crate::elm::ws_runtime::read_connect_params(&state).await;
        assert!(params.existing_session.is_none());
        assert_eq!(
            params.retired_signing_session_id.as_deref(),
            Some("sim-sign")
        );
        assert!(
            state.lock().await.session.is_some(),
            "late mesh traffic retains its context"
        );
        state.lock().await.session.as_mut().unwrap().session_id = "next-sign".into();
        let params = crate::elm::ws_runtime::read_connect_params(&state).await;
        assert_eq!(params.existing_session.unwrap().session_id, "next-sign");
        state.lock().await.device_id = "b".into();
        let params = crate::elm::ws_runtime::read_connect_params(&state).await;
        assert!(
            params.existing_session.is_none(),
            "joiners never re-announce the proposer session"
        );
    }

    #[tokio::test]
    async fn completed_proposer_withdraws_discovery_without_tearing_down_session() {
        let mut guard = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        guard.device_id = "a".into();
        guard.session = Some(signing_session("a", &["a", "b", "c"], 2));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        guard.websocket_msg_tx = Some(tx);
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(guard));
        super::withdraw_completed_invite(&state).await;
        let message: serde_json::Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(
            message,
            serde_json::json!({"type": "leave_session", "session_id": "sim-sign"})
        );
        assert_eq!(
            state
                .lock()
                .await
                .session
                .as_ref()
                .unwrap()
                .participants
                .len(),
            3
        );
        // A joiner completing later must not mutate the surviving signer roster.
        state.lock().await.device_id = "b".into();
        super::withdraw_completed_invite(&state).await;
        assert!(rx.try_recv().is_err());
        // The next ceremony retires its own invite, never the previous one.
        let mut guard = state.lock().await;
        guard.device_id = "a".into();
        guard.session.as_mut().unwrap().session_id = "next-sign".into();
        drop(guard);
        super::withdraw_completed_invite(&state).await;
        let message: serde_json::Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(message["session_id"], "next-sign");
    }

    #[test]
    fn signer_set_wire_format_matches_the_spec_vector_and_round_trips() {
        let ids = vec!["a".to_string(), "b".to_string()];
        let frame = encode_signer_set_frame(&ids);
        assert_eq!(frame, "SIGN_SET:WyJhIiwiYiJd");

        let payload = frame.strip_prefix(SIGN_SET_PREFIX).expect("has the prefix");
        let decoded_bytes = BASE64.decode(payload).expect("valid base64");
        let decoded = decode_signer_set(&decoded_bytes).expect("valid JSON string array");
        assert_eq!(decoded, ids);
    }

    #[test]
    fn decode_signer_set_rejects_non_json() {
        assert!(decode_signer_set(b"not json").is_err());
    }

    /// The proposer fixes itself + the first threshold-1 arrivals the
    /// moment it has threshold-many commitments, THEN runs Round 2 (it is
    /// always a member of its own fixed set).
    #[tokio::test]
    async fn proposer_fixes_signer_set_and_signs_once_threshold_commitments_arrive() {
        let (kps, pkp) = trusted_2_of_3();
        let session = signing_session("a", &["a", "b", "c"], 2);
        let message = b"proposer fixes the set".to_vec();

        let id_a = canonical_identifier::<Secp256K1Sha256TR>(&session.participants, "a").unwrap();
        let id_b = canonical_identifier::<Secp256K1Sha256TR>(&session.participants, "b").unwrap();

        let mut rng = starlab_core::rng::os_rng();
        let (nonces_a, commit_a) = frost_core::round1::commit(kps[&id_a].signing_share(), &mut rng);
        let (_nonces_b, commit_b) =
            frost_core::round1::commit(kps[&id_b].signing_share(), &mut rng);

        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.device_id = "a".into();
        state.session = Some(session);
        state.key_package = Some(kps[&id_a].clone());
        state.public_key_package = Some(pkp);
        state.signing_message = Some(message);
        state.frost_nonces = Some(nonces_a);
        state.frost_commitments.insert(id_a, commit_a);
        state.frost_commitments.insert(id_b, commit_b);
        state.commitment_arrival_order = vec!["a".to_string(), "b".to_string()];
        // Only self online — sends to "b"/"c" are skipped outright rather
        // than retried against a data channel that doesn't exist in this test.
        state.online_devices = Some(["a".to_string()].into_iter().collect());
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        super::try_advance_to_round2::<Secp256K1Sha256TR>(&state, &tx, "a").await;

        let guard = state.lock().await;
        assert_eq!(
            guard.signer_set,
            Some(["a".to_string(), "b".to_string()].into_iter().collect()),
            "proposer must fix itself + the first threshold-1 arrivals"
        );
        assert!(
            guard.frost_signature_shares.contains_key(&id_a),
            "proposer is a member of its own fixed set — must have signed"
        );
    }

    /// A non-proposer never picks a set on its own, even once it has
    /// threshold-many commitments — it must wait for SIGN_SET.
    #[tokio::test]
    async fn non_proposer_does_not_run_round2_without_sign_set() {
        let (kps, pkp) = trusted_2_of_3();
        let session = signing_session("a", &["a", "b", "c"], 2);
        let message = b"non-proposer waits".to_vec();

        let id_a = canonical_identifier::<Secp256K1Sha256TR>(&session.participants, "a").unwrap();
        let id_b = canonical_identifier::<Secp256K1Sha256TR>(&session.participants, "b").unwrap();

        let mut rng = starlab_core::rng::os_rng();
        let (nonces_b, commit_b) = frost_core::round1::commit(kps[&id_b].signing_share(), &mut rng);
        let (_nonces_a, commit_a) =
            frost_core::round1::commit(kps[&id_a].signing_share(), &mut rng);

        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.device_id = "b".into();
        state.session = Some(session);
        state.key_package = Some(kps[&id_b].clone());
        state.public_key_package = Some(pkp);
        state.signing_message = Some(message);
        state.frost_nonces = Some(nonces_b);
        // Threshold-many commitments (b's own + a's) — the OLD rule would
        // have signed right here. "b" is not the proposer, so it must not.
        state.frost_commitments.insert(id_b, commit_b);
        state.frost_commitments.insert(id_a, commit_a);
        state.online_devices = Some(["b".to_string()].into_iter().collect());
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        super::try_advance_to_round2::<Secp256K1Sha256TR>(&state, &tx, "b").await;

        let guard = state.lock().await;
        assert!(
            guard.signer_set.is_none(),
            "non-proposer must never fix a signer set"
        );
        assert!(
            guard.frost_signature_shares.is_empty(),
            "must not sign before SIGN_SET arrives"
        );
    }

    /// A node outside the fixed signer set never signs, but still
    /// aggregates once it holds every member's commitment + share — and
    /// converges on the identical signature.
    #[tokio::test]
    async fn non_member_never_signs_but_still_aggregates() {
        let (kps, pkp) = trusted_2_of_3();
        let session = signing_session("a", &["a", "b", "c"], 2);
        let message = b"non-member aggregates".to_vec();

        let id_a = canonical_identifier::<Secp256K1Sha256TR>(&session.participants, "a").unwrap();
        let id_b = canonical_identifier::<Secp256K1Sha256TR>(&session.participants, "b").unwrap();

        // Real Round 1 + Round 2 for the fixed set {a, b}; "c" plays no
        // part in producing these, only in aggregating them.
        let mut rng = starlab_core::rng::os_rng();
        let (nonces_a, commit_a) = frost_core::round1::commit(kps[&id_a].signing_share(), &mut rng);
        let (nonces_b, commit_b) = frost_core::round1::commit(kps[&id_b].signing_share(), &mut rng);
        let mut commitments = BTreeMap::new();
        commitments.insert(id_a, commit_a);
        commitments.insert(id_b, commit_b);
        let pkg = frost_core::SigningPackage::new(commitments, &message);
        let share_a = frost_core::round2::sign(&pkg, &nonces_a, &kps[&id_a]).unwrap();
        let share_b = frost_core::round2::sign(&pkg, &nonces_b, &kps[&id_b]).unwrap();

        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.device_id = "c".into();
        state.session = Some(session);
        // "c" has no key_package/nonces of its own — it never signs.
        state.public_key_package = Some(pkp);
        state.signing_message = Some(message);
        state.signer_set = Some(["a".to_string(), "b".to_string()].into_iter().collect());
        state.frost_commitments.insert(id_a, commit_a);
        state.frost_commitments.insert(id_b, commit_b);
        state.frost_signature_shares.insert(id_a, share_a);
        state.frost_signature_shares.insert(id_b, share_b);
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        // "c" is not in the signer set: must not attempt Round 2.
        super::try_advance_to_round2::<Secp256K1Sha256TR>(&state, &tx, "c").await;
        assert_eq!(
            state.lock().await.frost_signature_shares.len(),
            2,
            "non-member must not add its own share"
        );

        super::try_aggregate::<Secp256K1Sha256TR>(&state, &tx).await;

        let guard = state.lock().await;
        assert!(
            matches!(guard.signing_state, SigningState::Complete { .. }),
            "non-member must still aggregate to completion: {:?}",
            guard.signing_state
        );
        drop(guard);

        let msg = rx.try_recv().expect("SigningComplete must be emitted");
        assert!(matches!(msg, Message::SigningComplete { .. }));
    }

    fn state_with_session(
        self_id: &str,
        proposer: &str,
    ) -> crate::utils::appstate_compat::AppState<Secp256K1Sha256TR> {
        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.device_id = self_id.to_string();
        state.session = Some(signing_session(proposer, &["a", "b", "c"], 2));
        state
    }

    #[tokio::test]
    async fn signer_set_with_wrong_size_fails_the_ceremony() {
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state_with_session("b", "a")));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let payload = serde_json::to_vec(&["a".to_string()]).unwrap(); // size 1, threshold 2

        super::process_signing_set::<Secp256K1Sha256TR>(
            state.clone(),
            "b".into(),
            "a".into(),
            payload,
            tx,
        )
        .await;

        let guard = state.lock().await;
        assert!(
            matches!(guard.signing_state, SigningState::Failed { .. }),
            "wrong-size SIGN_SET must fail the ceremony: {:?}",
            guard.signing_state
        );
    }

    #[tokio::test]
    async fn signer_set_missing_the_proposer_fails_the_ceremony() {
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state_with_session("c", "a")));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let payload = serde_json::to_vec(&["b".to_string(), "c".to_string()]).unwrap();

        super::process_signing_set::<Secp256K1Sha256TR>(
            state.clone(),
            "c".into(),
            "a".into(),
            payload,
            tx,
        )
        .await;

        let guard = state.lock().await;
        assert!(
            matches!(guard.signing_state, SigningState::Failed { .. }),
            "SIGN_SET without the proposer must fail the ceremony: {:?}",
            guard.signing_state
        );
    }

    #[tokio::test]
    async fn signer_set_with_unknown_device_fails_the_ceremony() {
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state_with_session("b", "a")));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let payload = serde_json::to_vec(&["a".to_string(), "ghost".to_string()]).unwrap();

        super::process_signing_set::<Secp256K1Sha256TR>(
            state.clone(),
            "b".into(),
            "a".into(),
            payload,
            tx,
        )
        .await;

        let guard = state.lock().await;
        assert!(
            matches!(guard.signing_state, SigningState::Failed { .. }),
            "SIGN_SET listing an unknown device must fail the ceremony: {:?}",
            guard.signing_state
        );
    }

    #[tokio::test]
    async fn signer_set_from_a_non_proposer_is_ignored_not_fatal() {
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state_with_session("c", "a")));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let payload = serde_json::to_vec(&["a".to_string(), "b".to_string()]).unwrap();

        // "b" is not the proposer ("a" is) — must be ignored, not fatal.
        super::process_signing_set::<Secp256K1Sha256TR>(
            state.clone(),
            "c".into(),
            "b".into(),
            payload,
            tx,
        )
        .await;

        let guard = state.lock().await;
        assert!(guard.signer_set.is_none());
        assert!(!matches!(guard.signing_state, SigningState::Failed { .. }));
    }

    /// A `SIGN_SET` that beats this node's own session into existence (a
    /// cold-started co-signer) is buffered, then applied once the session
    /// exists — mirrors the existing `pending_pre_session_commitments` test
    /// coverage in spirit.
    #[tokio::test]
    async fn sign_set_before_session_exists_is_buffered_and_applied_later() {
        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.device_id = "b".into();
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let payload = serde_json::to_vec(&["a".to_string(), "b".to_string()]).unwrap();

        super::process_signing_set::<Secp256K1Sha256TR>(
            state.clone(),
            "b".into(),
            "a".into(),
            payload,
            tx.clone(),
        )
        .await;

        {
            let guard = state.lock().await;
            assert!(
                guard.signer_set.is_none(),
                "must not apply before a session exists"
            );
            assert_eq!(guard.pending_pre_session_signer_set.len(), 1);
        }

        {
            let mut guard = state.lock().await;
            guard.session = Some(signing_session("a", &["a", "b", "c"], 2));
        }

        super::drain_pre_session_signer_set::<Secp256K1Sha256TR>(state.clone(), "b".into(), tx)
            .await;

        let guard = state.lock().await;
        assert_eq!(
            guard.signer_set,
            Some(["a".to_string(), "b".to_string()].into_iter().collect())
        );
        assert!(guard.pending_pre_session_signer_set.is_empty());
    }

    // -------------------------------------------------------------
    // signing timeout — a stuck ceremony fails cleanly instead of hanging
    // -------------------------------------------------------------

    /// A ceremony stuck in `SharePhase` (fixed signer set, one member's
    /// `SIGN_SHARE` never arrived) fails once its timeout elapses, naming the
    /// missing signer, wiping the transient ceremony state, and emitting
    /// `Message::SigningFailed` — exactly `fail_and_notify`'s contract.
    #[tokio::test]
    async fn stuck_share_phase_times_out_naming_the_missing_signer() {
        let (kps, pkp) = trusted_2_of_3();
        let session = signing_session("a", &["a", "b", "c"], 2);
        let message = b"stuck ceremony".to_vec();

        let id_a = canonical_identifier::<Secp256K1Sha256TR>(&session.participants, "a").unwrap();
        let id_b = canonical_identifier::<Secp256K1Sha256TR>(&session.participants, "b").unwrap();

        let mut rng = starlab_core::rng::os_rng();
        let (nonces_a, commit_a) = frost_core::round1::commit(kps[&id_a].signing_share(), &mut rng);
        let (_nonces_b, commit_b) =
            frost_core::round1::commit(kps[&id_b].signing_share(), &mut rng);
        let mut commitments = BTreeMap::new();
        commitments.insert(id_a, commit_a);
        commitments.insert(id_b, commit_b);
        let pkg = frost_core::SigningPackage::new(commitments, &message);
        let share_a = frost_core::round2::sign(&pkg, &nonces_a, &kps[&id_a]).unwrap();

        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.device_id = "a".into();
        state.session = Some(session);
        state.public_key_package = Some(pkp);
        state.signing_message = Some(message);
        // Signer set is {a, b}; "b" is the shareholder that never sends its
        // SIGN_SHARE (e.g. shut down right after DKG).
        state.signer_set = Some(["a".to_string(), "b".to_string()].into_iter().collect());
        state.frost_commitments.insert(id_a, commit_a);
        state.frost_commitments.insert(id_b, commit_b);
        state.frost_signature_shares.insert(id_a, share_a);
        state.signing_state = SigningState::SharePhase {
            signing_id: super::INLINE_SIGNING_ID.to_string(),
            transaction_data: "14 bytes".to_string(),
            selected_signers: Vec::new(),
            signing_package: Some(pkg),
            shares: state.frost_signature_shares.clone(),
            own_share: Some(share_a),
            blockchain: String::new(),
            chain_id: None,
        };
        state.signing_epoch = 7;
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        super::spawn_signing_timeout::<Secp256K1Sha256TR>(
            state.clone(),
            tx,
            7,
            std::time::Duration::from_millis(20),
        );

        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("the timer must fire")
            .expect("channel must still be open");
        match msg {
            Message::SigningFailed { error, .. } => {
                assert!(
                    error.contains("timed out") && error.contains("from b"),
                    "must name the missing signer: {error}"
                );
            }
            other => panic!("expected SigningFailed, got {other:?}"),
        }

        let guard = state.lock().await;
        assert!(
            matches!(guard.signing_state, SigningState::Failed { .. }),
            "signing_state must be Failed: {:?}",
            guard.signing_state
        );
        assert!(guard.signer_set.is_none(), "signer_set must be wiped");
        assert!(
            guard.frost_signature_shares.is_empty(),
            "frost_signature_shares must be wiped"
        );
        assert!(
            guard.frost_commitments.is_empty(),
            "frost_commitments must be wiped"
        );
    }

    /// A ceremony that already reached `Complete` before its timer fires must
    /// be left alone — no spurious `SigningFailed` clobbering a real result.
    #[tokio::test]
    async fn timer_is_a_noop_once_the_ceremony_already_completed() {
        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.signing_epoch = 3;
        state.signing_state = SigningState::Complete {
            signing_id: super::INLINE_SIGNING_ID.to_string(),
            signature: vec![0xaa; 64],
        };
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        // Same epoch as the (already-finished) ceremony — the state check,
        // not the epoch check, is what must save it here.
        super::spawn_signing_timeout::<Secp256K1Sha256TR>(
            state.clone(),
            tx,
            3,
            std::time::Duration::from_millis(20),
        );

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        assert!(
            rx.try_recv().is_err(),
            "a completed ceremony must not emit SigningFailed"
        );
        let guard = state.lock().await;
        assert!(matches!(guard.signing_state, SigningState::Complete { .. }));
    }

    /// A timer armed for an earlier ceremony (lower epoch) must not touch a
    /// newer ceremony that has since started on the same `AppState` — the
    /// epoch check, not the state check, is what must save it here (the
    /// newer ceremony is ALSO stuck in `CommitmentPhase`).
    #[tokio::test]
    async fn stale_timer_from_a_previous_ceremony_does_not_fail_the_next_one() {
        let mut state = crate::utils::appstate_compat::AppState::<Secp256K1Sha256TR>::new();
        state.session = Some(signing_session("a", &["a", "b", "c"], 2));
        state.signing_epoch = 9; // ceremony 9 has already superseded ceremony 8
        state.signing_state = SigningState::CommitmentPhase {
            signing_id: super::INLINE_SIGNING_ID.to_string(),
            transaction_data: "14 bytes".to_string(),
            selected_signers: Vec::new(),
            commitments: BTreeMap::new(),
            own_commitment: None,
            nonces: None,
            blockchain: String::new(),
            chain_id: None,
        };
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(state));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        // Arm a timer for the OLD ceremony (epoch 8); the live one is 9.
        super::spawn_signing_timeout::<Secp256K1Sha256TR>(
            state.clone(),
            tx,
            8,
            std::time::Duration::from_millis(20),
        );

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        assert!(
            rx.try_recv().is_err(),
            "a stale timer must not fail the newer ceremony"
        );
        let guard = state.lock().await;
        assert!(
            matches!(guard.signing_state, SigningState::CommitmentPhase { .. }),
            "the newer ceremony's state must be untouched: {:?}",
            guard.signing_state
        );
    }
}
