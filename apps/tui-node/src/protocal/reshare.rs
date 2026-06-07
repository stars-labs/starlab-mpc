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
