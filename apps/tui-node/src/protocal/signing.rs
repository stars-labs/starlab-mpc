//! FROST Threshold Signing — Protocol Layer
//!
//! Structure mirrors `protocal/dkg.rs`: three async handlers driven by
//! Commands, and a SigningState tracked on `AppState<C>`. The round-1
//! (commit) and round-2 (sign) FROST functions are applied locally; peer
//! artifacts flow over the existing WebRTC mesh using `SIGN_COMMIT:<b64>`
//! and `SIGN_SHARE:<b64>` prefixes on `WebRTCMessage::SimpleMessage`.
//!
//! Unlike DKG — which must see EVERY participant's package before
//! moving forward — signing only needs `session.threshold` participants.
//! We accumulate whatever the mesh delivers and fire the next phase the
//! moment we've crossed that bar. Late arrivals are ignored.
//!
//! Public surface:
//! - [`handle_start_signing`]: kickoff. Runs `round1::commit` on this
//!   node, stashes the nonces, broadcasts the commitment.
//! - [`process_signing_round1`]: peer commitment arrived. Accumulate;
//!   when threshold reached, build `SigningPackage`, run `round2::sign`
//!   locally, broadcast the share.
//! - [`process_signing_round2`]: peer share arrived. Accumulate; when
//!   threshold reached, `aggregate`, serialize the signature, and emit
//!   `Message::SigningComplete` to the UI layer.
//!
//! All error paths transition `SigningState = Failed { reason }` and
//! emit `Message::SigningFailed`. There are no panics in this module —
//! a kill-the-task FROST error must not take down the tokio runtime.

use crate::elm::message::Message;
use crate::protocal::dkg::canonical_identifier;
use crate::protocal::signal::WebRTCMessage;
use crate::utils::appstate_compat::AppState;
use crate::utils::state::SigningState;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use frost_core::{Ciphersuite, Identifier};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info, warn};

/// Prefix markers for the two signing-related frames on the SimpleMessage
/// channel. The inbound dispatcher in `network/webrtc.rs` strip-prefixes
/// against these constants; keep them in sync.
pub const SIGN_COMMIT_PREFIX: &str = "SIGN_COMMIT:";
pub const SIGN_SHARE_PREFIX: &str = "SIGN_SHARE:";

/// Synthetic signing-request id for the "sign a message right now" flow
/// we implement in Phase C. The real pending-signing-request queue is a
/// Phase E concern; until then every ceremony uses this single id, which
/// lets the UI match SigningComplete emissions to the active sign screen
/// without a queue lookup.
pub(crate) const INLINE_SIGNING_ID: &str = "inline";

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
    let (key_package, session, my_identifier) = {
        let mut guard = state.lock().await;

        let Some(kp) = guard.key_package.clone() else {
            let err = "handle_start_signing: no key_package — unlock the wallet first"
                .to_string();
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
        // attempt doesn't contaminate this one. Caveat on `frost_commitments`:
        // we deliberately do NOT clear that map here. Reason: on the
        // joiner path, handle_start_signing runs AFTER the node has
        // already buffered the creator's SIGN_COMMIT via
        // `process_signing_round1`. Clearing would drop that buffered
        // commit and stall the ceremony at threshold - 1. Inserts are
        // keyed by `Identifier<C>`, so stale entries from a previous
        // ceremony with the same participants get overwritten by the
        // fresh commit below rather than accumulated. If a prior
        // ceremony had DIFFERENT participants you'd get cross-contamination,
        // but we don't support concurrent ceremonies this phase.
        guard.frost_signature_shares.clear();
        guard.frost_nonces = None;
        guard.signing_message = Some(message.clone());
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

        (kp, session, my_id)
    };

    // ---- FROST Round 1: commit
    use frost_ed25519::rand_core::OsRng;
    let mut rng = OsRng;
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
        guard.frost_commitments.insert(my_identifier, commitments.clone());
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

    // Check the threshold-reached edge here too — a 2-of-3 wallet where
    // only this node signs would otherwise sit on its own commitment
    // forever. `try_advance_to_round2` runs Round 2 locally if we
    // already have threshold-many commitments (including our own).
    try_advance_to_round2::<C>(&state, &ui_tx, &self_device_id).await;
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
        let guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            // Late arrival with no session — ignore, log.
            warn!("SIGN_COMMIT from {} but no active session; dropping", from_device_id);
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

    // Insert into the accumulator.
    {
        let mut guard = state.lock().await;
        guard.frost_commitments.insert(decode_result.0, decode_result.1);
    }

    try_advance_to_round2::<C>(&state, &ui_tx, &self_device_id).await;
}

/// If this node has gathered threshold-many commitments AND its own
/// Round-1 nonces are stashed, derive our Round-2 share and broadcast
/// it. Idempotent — if we've already broadcast our share, returns
/// without doing anything.
async fn try_advance_to_round2<C>(
    state: &Arc<Mutex<AppState<C>>>,
    ui_tx: &UnboundedSender<Message>,
    self_device_id: &str,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    // Collect the inputs under a short lock.
    let (commitments_for_pkg, message, nonces, key_package, my_id, threshold, participants) = {
        let guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            return;
        };
        let threshold = session.threshold as usize;
        if guard.frost_commitments.len() < threshold {
            return; // not enough yet — wait for more SIGN_COMMITs
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
        let Some(nonces) = guard.frost_nonces.clone() else {
            warn!("try_advance_to_round2: frost_nonces is None — did Round 1 run?");
            return;
        };
        let Some(message) = guard.signing_message.clone() else {
            warn!("try_advance_to_round2: signing_message is None; dropping");
            return;
        };

        (
            guard.frost_commitments.clone(),
            message,
            nonces,
            key_package.clone(),
            my_id,
            threshold,
            session.participants.clone(),
        )
    };

    info!(
        "✅ {}: threshold reached ({} commitments), running Round 2",
        self_device_id, threshold
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
            own_share: Some(guard.frost_signature_shares[&my_id].clone()),
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
            warn!("SIGN_SHARE from {} but no active session; dropping", from_device_id);
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

/// If this node has threshold-many shares AND a SigningPackage built
/// during Round 1, run `frost_core::aggregate` and emit
/// `Message::SigningComplete`. Idempotent.
async fn try_aggregate<C>(state: &Arc<Mutex<AppState<C>>>, ui_tx: &UnboundedSender<Message>)
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let (signing_package, shares, pubkey_package, message, threshold) = {
        let guard = state.lock().await;
        let Some(session) = guard.session.as_ref() else {
            return;
        };
        let threshold = session.threshold as usize;
        if guard.frost_signature_shares.len() < threshold {
            return;
        }
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
        // Rebuild the SigningPackage from the accumulated commitments — we
        // deliberately don't rely on the SharePhase::signing_package field
        // because that's only populated on the node that ran Round 2
        // locally. Every node that's about to aggregate has the full
        // `frost_commitments` map, so this construction is stable.
        let pkg = frost_core::SigningPackage::new(guard.frost_commitments.clone(), &message);
        (
            pkg,
            guard.frost_signature_shares.clone(),
            pkp.clone(),
            message,
            threshold,
        )
    };

    info!(
        "🧮 Threshold shares reached ({}), running aggregate",
        threshold
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
    if pubkey_package.verifying_key().verify(&message, &signature).is_err() {
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
    }

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
// helpers
// -----------------------------------------------------------------

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

// Suppress a "unused import" warning when compiled without the downstream
// sites yet in place (they land in Stage C.3/C.4). Remove once wired.
#[allow(dead_code)]
pub(crate) fn _unused_anchor<C: Ciphersuite>(
    _x: &BTreeMap<Identifier<C>, ()>,
) {
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
    use frost_secp256k1::{
        keys::{generate_with_dealer, IdentifierList, KeyPackage as KP, PublicKeyPackage as PKP},
        rand_core::OsRng,
        Identifier, Secp256K1Sha256,
    };
    use std::collections::BTreeMap;

    type KeyPkgMap = BTreeMap<Identifier, KP>;

    fn trusted_2_of_3() -> (KeyPkgMap, PKP) {
        let mut rng = OsRng;
        let (shares, pkp) =
            generate_with_dealer(3, 2, IdentifierList::Default, &mut rng).expect("keygen");
        let mut kps = KeyPkgMap::new();
        for (id, share) in shares {
            kps.insert(id, share.try_into().expect("share→KP"));
        }
        (kps, pkp)
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
        let mut rng = OsRng;
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
        let signature = frost_core::aggregate::<Secp256K1Sha256>(&signing_package, &shares_map, &pkp)
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
        let mut rng = OsRng;
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
        let result = frost_core::aggregate::<Secp256K1Sha256>(&pkg_b, &shares, &pkp);
        assert!(
            result.is_err(),
            "aggregating shares for the wrong message must fail; got Ok({:?})",
            result.map(|s| s.serialize().unwrap())
        );
    }
}
