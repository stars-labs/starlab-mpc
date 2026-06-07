//! Reshare (share refresh / resharing) protocol-layer driver — phase 3 of #45.
//!
//! These are the **pure, synchronous** state-transition functions that drive a
//! reshare ceremony over `AppState<C>`, mirroring `protocal::dkg`'s round
//! functions but calling frost's `refresh_dkg_*` instead of `dkg::part*`. They
//! consume the OLD share + group key already loaded on `AppState`
//! (`key_package` / `public_key_package`) and produce a refreshed share with
//! the group key **unchanged**.
//!
//! The async mesh transport (broadcast/receive the round packages over the
//! WebRTC data channel) and the command/message wiring + keystore persistence
//! land in phase 4, where the L3 multi-process test validates the whole thing.
//! Keeping the transition logic here, sync and `AppState`-only, makes it
//! unit-testable without a live mesh (see the tests at the bottom: N in-process
//! `AppState`s exchange packages and finalize to the same preserved group key).
//!
//! ## Identifier rule (RESHARE_CEREMONY_DESIGN.md §3)
//! Callers MUST pass each participant's ORIGINAL FROST identifier (from the
//! wallet's original participant list, filtered to the retained set) — never a
//! `canonical_identifier` recomputed over the reduced set. Removing a device
//! therefore yields non-contiguous ids (e.g. {1,3}); frost accepts that
//! (validated in frost-core `resharing.rs` + phase 1).

use frost_core::keys::dkg::{round1, round2};
use frost_core::keys::refresh::{refresh_dkg_part2, refresh_dkg_part_1, refresh_dkg_shares};
// frost_ed25519 / frost_secp256k1 re-export the same rand_core 0.6; OsRng is
// curve-agnostic and satisfies frost's RngCore + CryptoRng bound for any C.
use frost_ed25519::rand_core::OsRng;
use frost_core::{Ciphersuite, Identifier};

use crate::utils::appstate_compat::AppState;

/// FROST identifier for `device_id` in a reshare — derived from the wallet's
/// **ORIGINAL** participant set (design §3), NOT the retained set. Removing a
/// device must not renumber the survivors (their old key packages carry their
/// original ids), so we always canonicalise over the full original list
/// (persisted in `WalletMetadata.participants`). Using the reduced set here
/// would silently mis-key the refresh and corrupt the result.
pub fn reshare_identifier<C: Ciphersuite>(
    original_participants: &[String],
    device_id: &str,
) -> Option<Identifier<C>> {
    crate::protocal::dkg::canonical_identifier::<C>(original_participants, device_id)
}

/// Round 1: produce this node's refresh round-1 package and stash the secret.
/// `max_signers` = number of RETAINED participants; `min_signers` = the
/// (unchanged) threshold; `identifier` = this node's ORIGINAL id.
pub fn reshare_part1<C: Ciphersuite>(
    state: &mut AppState<C>,
    identifier: Identifier<C>,
    max_signers: u16,
    min_signers: u16,
) -> Result<round1::Package<C>, String> {
    let (secret, package) =
        refresh_dkg_part_1::<C, _>(identifier, max_signers, min_signers, OsRng)
            .map_err(|e| format!("reshare part1: {e}"))?;
    state.reshare_round1_secret = Some(secret);
    state.reshare_in_progress = true;
    Ok(package)
}

/// Record a peer's round-1 package (peers only — never our own id).
pub fn add_reshare_round1<C: Ciphersuite>(
    state: &mut AppState<C>,
    sender: Identifier<C>,
    package: round1::Package<C>,
) {
    state.reshare_round1_packages.insert(sender, package);
}

/// Round 2: consume our round-1 secret + the peers' round-1 packages, produce
/// the round-2 secret (stashed) and the per-recipient round-2 packages to send.
pub fn reshare_part2<C: Ciphersuite>(
    state: &mut AppState<C>,
) -> Result<std::collections::BTreeMap<Identifier<C>, round2::Package<C>>, String> {
    let secret = state
        .reshare_round1_secret
        .take()
        .ok_or("reshare part2: no round-1 secret")?;
    let (r2_secret, r2_packages) =
        refresh_dkg_part2::<C>(secret, &state.reshare_round1_packages)
            .map_err(|e| format!("reshare part2: {e}"))?;
    state.reshare_round2_secret = Some(r2_secret);
    Ok(r2_packages)
}

/// Record a peer's round-2 package addressed to us.
pub fn add_reshare_round2<C: Ciphersuite>(
    state: &mut AppState<C>,
    sender: Identifier<C>,
    package: round2::Package<C>,
) {
    state.reshare_round2_packages.insert(sender, package);
}

/// Finalize: refresh this node's share, **assert the group key is unchanged**,
/// and swap the new share + public key package onto `AppState`. Returns the new
/// `(KeyPackage, PublicKeyPackage)` for the caller to persist (phase 4: atomic
/// keystore overwrite + old-share erase). On any failure — including a
/// group-key mismatch — the OLD share on `AppState` is left intact.
#[allow(clippy::type_complexity)]
pub fn finalize_reshare<C: Ciphersuite>(
    state: &mut AppState<C>,
) -> Result<
    (
        frost_core::keys::KeyPackage<C>,
        frost_core::keys::PublicKeyPackage<C>,
    ),
    String,
> {
    let r2_secret = state
        .reshare_round2_secret
        .as_ref()
        .ok_or("reshare finalize: no round-2 secret")?;
    let old_pub = state
        .public_key_package
        .clone()
        .ok_or("reshare finalize: no existing public key package (wallet not loaded)")?;
    let old_key = state
        .key_package
        .clone()
        .ok_or("reshare finalize: no existing key package (wallet not loaded)")?;

    let (new_key, new_pub) = refresh_dkg_shares::<C>(
        r2_secret,
        &state.reshare_round1_packages,
        &state.reshare_round2_packages,
        old_pub.clone(),
        old_key,
    )
    .map_err(|e| format!("reshare finalize: {e}"))?;

    // Hard invariant: the refresh MUST preserve the group key (the address).
    let old_vk = old_pub
        .verifying_key()
        .serialize()
        .map_err(|e| format!("serialize old vk: {e}"))?;
    let new_vk = new_pub
        .verifying_key()
        .serialize()
        .map_err(|e| format!("serialize new vk: {e}"))?;
    if old_vk != new_vk {
        // Do NOT swap — keep the old, usable share.
        return Err("reshare finalize: group key changed — aborting, old share kept".into());
    }

    // Swap in the refreshed material and clear the ceremony scratch state.
    state.key_package = Some(new_key.clone());
    state.public_key_package = Some(new_pub.clone());
    clear_reshare_state(state);
    Ok((new_key, new_pub))
}

/// Drop all reshare round scratch state (on completion or abort).
pub fn clear_reshare_state<C: Ciphersuite>(state: &mut AppState<C>) {
    state.reshare_round1_secret = None;
    state.reshare_round2_secret = None;
    state.reshare_round1_packages.clear();
    state.reshare_round2_packages.clear();
    state.reshare_in_progress = false;
}

// =====================================================================
// Async mesh orchestration (#45 4b). Same-set reshare reusing the live
// post-DKG WebRTC mesh: each node triggers round 1, the round packages flow
// over the existing data channels (RESHARE_ROUND1/2: frames, routed in
// network/webrtc.rs), and each node finalizes to the unchanged group key.
//
// Scope: same participant set (every original participant stays). Reshare with
// a *reduced* set (device removal) needs a fresh announce/join to form a new
// mesh — tracked in #56. Keystore persistence of the refreshed share also needs
// password-stash plumbing (#56); finalize here swaps the in-memory share and
// emits ReshareComplete (enough to keep signing with the new shares).
// =====================================================================

use crate::elm::message::Message;
use crate::protocal::signal::WebRTCMessage;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info};

fn b64(bytes: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    BASE64.encode(bytes)
}

/// Begin a reshare on this node: refresh part 1 over the current session/mesh,
/// then broadcast our round-1 package to the retained peers. If peers' round-1
/// packages already arrived (faster transport, or they triggered first), advance
/// to round 2 immediately now that our own round-1 secret exists.
pub async fn handle_trigger_reshare_round1<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let (participants, pkg_bytes) = {
        let mut guard = state.lock().await;
        let session = match &guard.session {
            Some(s) => s.clone(),
            None => {
                error!("reshare round1: no active session");
                return;
            }
        };
        if guard.reshare_original_participants.is_empty() {
            guard.reshare_original_participants = session.participants.clone();
        }
        let original = guard.reshare_original_participants.clone();
        let my_id = match reshare_identifier::<C>(&original, &self_device_id) {
            Some(id) => id,
            None => {
                error!("reshare round1: {} not in original participants", self_device_id);
                return;
            }
        };
        let pkg = match reshare_part1(&mut guard, my_id, session.total, session.threshold) {
            Ok(p) => p,
            Err(e) => {
                error!("reshare round1: {e}");
                return;
            }
        };
        let bytes = match pkg.serialize() {
            Ok(b) => b,
            Err(e) => {
                error!("reshare round1 serialize: {e}");
                return;
            }
        };
        (session.participants.clone(), bytes)
    };

    let msg = WebRTCMessage::<C>::SimpleMessage { text: format!("RESHARE_ROUND1:{}", b64(&pkg_bytes)) };
    for peer in participants.iter().filter(|p| **p != self_device_id) {
        let _ = crate::utils::device::send_webrtc_message(peer, &msg, state.clone()).await;
    }
    info!("📡 reshare round-1 broadcast to {} peers", participants.len().saturating_sub(1));
    maybe_advance_to_reshare_round2(state, self_device_id, tx).await;
}

/// Ingest a peer's round-1 package, then try to advance to round 2.
pub async fn process_reshare_round1<C>(
    state: Arc<Mutex<AppState<C>>>,
    from_device_id: String,
    package_bytes: Vec<u8>,
    tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    {
        let mut guard = state.lock().await;
        let session = match &guard.session {
            Some(s) => s.clone(),
            None => return,
        };
        // A peer's round-1 may arrive before our own trigger ran; seed the
        // original participant set from the session so the id map works either way.
        if guard.reshare_original_participants.is_empty() {
            guard.reshare_original_participants = session.participants.clone();
        }
        let original = guard.reshare_original_participants.clone();
        let sender = match reshare_identifier::<C>(&original, &from_device_id) {
            Some(id) => id,
            None => {
                error!("reshare round1 from unknown device {from_device_id}");
                return;
            }
        };
        let pkg = match frost_core::keys::dkg::round1::Package::<C>::deserialize(&package_bytes) {
            Ok(p) => p,
            Err(e) => {
                error!("reshare round1 deserialize: {e}");
                return;
            }
        };
        add_reshare_round1(&mut guard, sender, pkg);
    }
    let self_id = state.lock().await.device_id.clone();
    maybe_advance_to_reshare_round2(state, self_id, tx).await;
}

/// Advance to round 2 iff (a) our OWN round-1 secret exists — i.e. our local
/// round-1 has run — AND (b) all retained peers' round-1 packages are in. Safe
/// to call from either the local trigger or a peer-package handler. Gating on
/// the secret is essential: a peer's round-1 can land before our own round-1 ran
/// (observed across separate processes on a fresh mesh), and `reshare_part2`
/// consumes that secret — advancing without it aborts with "no round-1 secret".
/// (DKG sidesteps this because its round-1 map includes our own package; the
/// reshare map holds peers only, so "all peers in" ≠ "we ran".)
async fn maybe_advance_to_reshare_round2<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let advance = {
        let guard = state.lock().await;
        let need = guard
            .session
            .as_ref()
            .map(|s| (s.total as usize).saturating_sub(1))
            .unwrap_or(0);
        guard.reshare_round1_secret.is_some()
            && need > 0
            && guard.reshare_round1_packages.len() >= need
    };
    if advance {
        handle_trigger_reshare_round2(state, self_device_id, tx).await;
    }
}

/// Produce round-2 packages and send each retained peer its own, then try to
/// finalize (peers' round-2 may already be buffered).
pub async fn handle_trigger_reshare_round2<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let (participants, original, per_recipient) = {
        let mut guard = state.lock().await;
        let session = match &guard.session {
            Some(s) => s.clone(),
            None => return,
        };
        let original = guard.reshare_original_participants.clone();
        let out = match reshare_part2(&mut guard) {
            Ok(o) => o,
            Err(e) => {
                error!("reshare round2: {e}");
                return;
            }
        };
        (session.participants.clone(), original, out)
    };

    for peer in participants.iter().filter(|p| **p != self_device_id) {
        let pid = match reshare_identifier::<C>(&original, peer) {
            Some(id) => id,
            None => continue,
        };
        if let Some(pkg) = per_recipient.get(&pid) {
            if let Ok(bytes) = pkg.serialize() {
                let msg = WebRTCMessage::<C>::SimpleMessage {
                    text: format!("RESHARE_ROUND2:{}", b64(&bytes)),
                };
                let _ = crate::utils::device::send_webrtc_message(peer, &msg, state.clone()).await;
            }
        }
    }
    maybe_finalize_reshare(state, tx).await;
}

/// Ingest a peer's round-2 package, then try to finalize.
pub async fn process_reshare_round2<C>(
    state: Arc<Mutex<AppState<C>>>,
    from_device_id: String,
    package_bytes: Vec<u8>,
    tx: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    {
        let mut guard = state.lock().await;
        if guard.session.is_none() {
            return;
        }
        let original = guard.reshare_original_participants.clone();
        let sender = match reshare_identifier::<C>(&original, &from_device_id) {
            Some(id) => id,
            None => {
                error!("reshare round2 from unknown device {from_device_id}");
                return;
            }
        };
        let pkg = match frost_core::keys::dkg::round2::Package::<C>::deserialize(&package_bytes) {
            Ok(p) => p,
            Err(e) => {
                error!("reshare round2 deserialize: {e}");
                return;
            }
        };
        add_reshare_round2(&mut guard, sender, pkg);
    }
    maybe_finalize_reshare(state, tx).await;
}

/// Finalize iff (a) our OWN round-2 secret exists AND (b) all peers' round-2
/// packages are in — the round-2 analogue of [`maybe_advance_to_reshare_round2`]
/// (a peer's round-2 can arrive before our own round-2 ran). Swaps the refreshed
/// share into AppState (group key asserted unchanged), persists it atomically
/// over the existing wallet, and emits `ReshareComplete`.
async fn maybe_finalize_reshare<C>(state: Arc<Mutex<AppState<C>>>, tx: UnboundedSender<Message>)
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let ready = {
        let guard = state.lock().await;
        let need = guard
            .session
            .as_ref()
            .map(|s| (s.total as usize).saturating_sub(1))
            .unwrap_or(0);
        guard.reshare_round2_secret.is_some()
            && need > 0
            && guard.reshare_round2_packages.len() >= need
    };
    if !ready {
        return;
    }
    // Finalize under lock (swaps the in-memory share + asserts the group key),
    // capturing what we need to persist; then do the keystore IO.
    let finalized = {
        let mut guard = state.lock().await;
        match finalize_reshare(&mut guard) {
            Ok((new_key, new_pub)) => {
                let group_hex = new_pub
                    .verifying_key()
                    .serialize()
                    .map(|b| hex::encode(b))
                    .unwrap_or_default();
                Some((
                    new_key,
                    new_pub,
                    group_hex,
                    guard.reshare_wallet_id.clone().unwrap_or_default(),
                    guard.reshare_password.clone(),
                    guard.reshare_keystore_path.clone(),
                    guard.device_id.clone(),
                    guard.session.clone(),
                ))
            }
            Err(e) => {
                error!("reshare finalize: {e}");
                None
            }
        }
    };
    if let Some((new_key, new_pub, group_hex, wallet_id, password, path, device_id, session)) =
        finalized
    {
        // Persist the refreshed share atomically over the existing wallet
        // (same id/curve/group-key/address). Best-effort: a failure here
        // leaves the in-memory new share usable this session and is logged.
        if let (Some(password), Some(path), Some(session)) = (password, path, &session) {
            match crate::elm::command::encode_keystore_blob(&new_key, &new_pub) {
                Ok(blob) => match crate::keystore::Keystore::new(&path, &device_id) {
                    Ok(mut ks) => {
                        let idx =
                            ks.get_wallet(&wallet_id).map(|w| w.participant_index).unwrap_or(1);
                        if let Err(e) = ks.update_wallet_share(
                            &wallet_id,
                            &blob,
                            &password,
                            session.threshold,
                            session.total,
                            session.participants.clone(),
                            idx,
                        ) {
                            error!("reshare persist: update_wallet_share: {e}");
                        } else {
                            info!("💾 refreshed share persisted for {}", wallet_id);
                        }
                    }
                    Err(e) => error!("reshare persist: keystore open: {e}"),
                },
                Err(e) => error!("reshare persist: encode blob: {e}"),
            }
        }
        info!("✅ reshare complete; group key preserved = {}", group_hex);
        let _ = tx.send(Message::ReshareComplete { wallet_id, group_public_key: group_hex });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frost_secp256k1::Secp256K1Sha256 as Secp;
    use mpc_wallet_frost_core::resharing;
    use std::collections::BTreeMap;

    fn id(i: u16) -> Identifier<Secp> {
        Identifier::<Secp>::try_from(i).unwrap()
    }

    // Drive a full reshare across `keep` (their ORIGINAL ids) using only the
    // AppState protocol functions, exchanging packages in-memory (no mesh).
    // Returns each node's new group-key hex.
    fn run_reshare(total: u16, threshold: u16, keep: &[u16]) -> Vec<String> {
        // Starting wallet: per-id KeyPackage + the shared PublicKeyPackage.
        let (kps, pp) = resharing::dkg_keypackages::<Secp>(total, threshold, 20).unwrap();

        // One AppState per retained participant, pre-loaded with its OLD share.
        let mut states: BTreeMap<u16, AppState<Secp>> = BTreeMap::new();
        for &k in keep {
            let mut st = AppState::<Secp>::with_device_id(format!("dev{k}"));
            st.key_package = Some(kps[&k].clone());
            st.public_key_package = Some(pp.clone());
            states.insert(k, st);
        }
        let max = keep.len() as u16;

        // Round 1: each produces its package.
        let mut r1: BTreeMap<u16, round1::Package<Secp>> = BTreeMap::new();
        for &k in keep {
            let pkg = reshare_part1(states.get_mut(&k).unwrap(), id(k), max, threshold).unwrap();
            r1.insert(k, pkg);
        }
        // Deliver peers' round-1 packages.
        for &k in keep {
            for &j in keep {
                if j != k {
                    add_reshare_round1(states.get_mut(&k).unwrap(), id(j), r1[&j].clone());
                }
            }
        }
        // Round 2: each produces per-recipient packages; deliver to recipients.
        let mut sent: BTreeMap<u16, BTreeMap<Identifier<Secp>, round2::Package<Secp>>> =
            BTreeMap::new();
        for &k in keep {
            let out = reshare_part2(states.get_mut(&k).unwrap()).unwrap();
            sent.insert(k, out);
        }
        for &k in keep {
            for &j in keep {
                if j != k {
                    if let Some(pkg) = sent[&j].get(&id(k)) {
                        add_reshare_round2(states.get_mut(&k).unwrap(), id(j), pkg.clone());
                    }
                }
            }
        }
        // Finalize each; collect new group keys.
        let mut keys = Vec::new();
        for &k in keep {
            let (_nk, np) = finalize_reshare(states.get_mut(&k).unwrap()).unwrap();
            keys.push(hex::encode(np.verifying_key().serialize().unwrap()));
            // AppState was swapped to the new share + ceremony state cleared.
            assert!(!states[&k].reshare_in_progress);
            assert!(states[&k].reshare_round1_packages.is_empty());
        }
        keys
    }

    #[test]
    fn reshare_identifier_uses_original_set_not_reduced() {
        // Original sorted: alice=1, bob=2, carol=3.
        let original = vec!["alice".to_string(), "bob".to_string(), "carol".to_string()];
        assert_eq!(reshare_identifier::<Secp>(&original, "alice"), Identifier::<Secp>::try_from(1).ok());
        assert_eq!(reshare_identifier::<Secp>(&original, "carol"), Identifier::<Secp>::try_from(3).ok());
        // Removing the MIDDLE device (bob): survivors must KEEP their original ids.
        // The wrong approach (canonicalise over the reduced {alice,carol}) would
        // make carol=2 — proving why we must use the original set.
        let reduced = vec!["alice".to_string(), "carol".to_string()];
        let wrong = crate::protocal::dkg::canonical_identifier::<Secp>(&reduced, "carol");
        assert_eq!(wrong, Identifier::<Secp>::try_from(2).ok(), "reduced-set id is 2 (wrong)");
        assert_ne!(
            reshare_identifier::<Secp>(&original, "carol"),
            wrong,
            "reshare must NOT renumber carol — keep original id 3"
        );
    }

    #[test]
    fn appstate_reshare_same_set_preserves_group_key() {
        let (_, pp) = resharing::dkg_keypackages::<Secp>(3, 2, 20).unwrap();
        let before = hex::encode(pp.verifying_key().serialize().unwrap());
        let keys = run_reshare(3, 2, &[1, 2, 3]);
        assert!(keys.iter().all(|k| *k == before), "group key preserved across all nodes");
    }

    #[test]
    fn appstate_reshare_remove_middle_device_noncontiguous() {
        // Remove the middle device (id 2) → survivors {1,3}; group key preserved.
        let (_, pp) = resharing::dkg_keypackages::<Secp>(3, 2, 20).unwrap();
        let before = hex::encode(pp.verifying_key().serialize().unwrap());
        let keys = run_reshare(3, 2, &[1, 3]);
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|k| *k == before), "address preserved after middle removal");
    }
}
