//! Share refresh / resharing on top of `frost_core::keys::refresh` (#45).
//!
//! Refreshing re-randomises every participant's secret share **without changing
//! the group public key** (your address is stable). It serves two ends:
//!
//! - **Proactive security:** rotate all shares on a schedule so shares stolen in
//!   different epochs never combine to reach the threshold.
//! - **Remove a device:** a quorum re-shares among a *subset*, dropping a
//!   lost/stolen participant; that participant's old share can no longer sign.
//!
//! After a refresh, **old shares are useless** — an old `KeyPackage` cannot be
//! combined with refreshed ones to produce a valid signature. That is the
//! security property the recovery story relies on (see
//! `docs/RECOVERY_AND_RESHARING.md`).
//!
//! ## What this does NOT do
//!
//! frost-core's refresh (both the dealerless DKG variant used here and the
//! trusted-dealer `refresh_share`) requires **every refreshing participant to
//! already hold a share** (`refresh_dkg_shares` takes the old `KeyPackage`). So
//! you can rotate the existing holders or *remove* one, but you **cannot add a
//! brand-new device that never had a share** through refresh. Bringing a fresh
//! replacement device online is a keystore-restore (copy an existing encrypted
//! share) or a fuller enrollment protocol — not this primitive.
//!
//! This module is the in-process, ciphersuite-generic engine that proves the
//! mechanism end to end; the networked client ceremony (a `reshare` session +
//! CLI) layers on top, exactly as the DKG client does over `keys::dkg`.

use crate::errors::{FrostError, Result};
use frost_core::keys::dkg::{part1 as dkg_part1, part2 as dkg_part2, part3 as dkg_part3};
use frost_core::keys::refresh::{refresh_dkg_part2, refresh_dkg_part_1, refresh_dkg_shares};
use frost_core::keys::{KeyPackage, PublicKeyPackage};
use frost_core::{Ciphersuite, Identifier};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use std::collections::BTreeMap;

fn ident<C: Ciphersuite>(i: u16) -> Result<Identifier<C>> {
    Identifier::<C>::try_from(i).map_err(|e| FrostError::InvalidIdentifier(format!("{i}: {e}")))
}

#[cfg(test)]
fn group_key_hex<C: Ciphersuite>(p: &PublicKeyPackage<C>) -> Result<String> {
    p.verifying_key()
        .serialize()
        .map(hex::encode)
        .map_err(|e| FrostError::SerializationError(e.to_string()))
}

/// Run a fresh dealerless DKG for `total`/`threshold` (deterministic per seed).
/// Returns each participant's key package + the shared public key package.
/// Used as the *starting state* a refresh operates on.
pub fn dkg_keypackages<C: Ciphersuite>(
    total: u16,
    threshold: u16,
    seed_base: u8,
) -> Result<(BTreeMap<u16, KeyPackage<C>>, PublicKeyPackage<C>)> {
    let ids: Vec<u16> = (1..=total).collect();

    // round 1
    let mut r1_secret = BTreeMap::new();
    let mut r1_pkgs: BTreeMap<Identifier<C>, _> = BTreeMap::new();
    for &i in &ids {
        let mut rng = ChaCha20Rng::from_seed([seed_base.wrapping_add(i as u8); 32]);
        let (s, p) = dkg_part1::<C, _>(ident::<C>(i)?, total, threshold, &mut rng)
            .map_err(|e| FrostError::DkgError(e.to_string()))?;
        r1_secret.insert(i, s);
        r1_pkgs.insert(ident::<C>(i)?, p);
    }
    // round 2
    let mut r2_secret = BTreeMap::new();
    let mut r2_for: BTreeMap<u16, BTreeMap<Identifier<C>, _>> = BTreeMap::new();
    for &i in &ids {
        let others: BTreeMap<_, _> = r1_pkgs
            .iter()
            .filter(|(k, _)| **k != ident::<C>(i).unwrap())
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let (s, sent) = dkg_part2::<C>(r1_secret.remove(&i).unwrap(), &others)
            .map_err(|e| FrostError::DkgError(e.to_string()))?;
        r2_secret.insert(i, s);
        for (rcpt, pkg) in sent {
            r2_for.entry_by_recipient(rcpt, i, pkg, &ids);
        }
    }
    // round 3
    let mut kps = BTreeMap::new();
    let mut pubpkg = None;
    for &i in &ids {
        let others_r1: BTreeMap<_, _> = r1_pkgs
            .iter()
            .filter(|(k, _)| **k != ident::<C>(i).unwrap())
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let recv_r2 = r2_for.received_for(i)?;
        let (kp, pp) = dkg_part3::<C>(&r2_secret[&i], &others_r1, &recv_r2)
            .map_err(|e| FrostError::DkgError(e.to_string()))?;
        kps.insert(i, kp);
        pubpkg = Some(pp);
    }
    Ok((kps, pubpkg.unwrap()))
}

/// Refresh shares for the participant set `new_ids` (a subset of, or equal to,
/// the current holders — every id in `new_ids` MUST already hold a share in
/// `old_kps`). The group public key is preserved; fresh key packages are
/// returned. Removing an id (omit it from `new_ids`) drops that participant.
pub fn refresh<C: Ciphersuite>(
    old_kps: &BTreeMap<u16, KeyPackage<C>>,
    old_pub: &PublicKeyPackage<C>,
    new_ids: &[u16],
    threshold: u16,
    seed_base: u8,
) -> Result<(BTreeMap<u16, KeyPackage<C>>, PublicKeyPackage<C>)> {
    let total = new_ids.len() as u16;
    for id in new_ids {
        if !old_kps.contains_key(id) {
            return Err(FrostError::InvalidState(format!(
                "participant {id} has no existing share to refresh"
            )));
        }
    }

    // refresh round 1
    let mut r1_secret = BTreeMap::new();
    let mut r1_pkgs: BTreeMap<Identifier<C>, _> = BTreeMap::new();
    for &i in new_ids {
        let mut rng = ChaCha20Rng::from_seed([seed_base.wrapping_add(i as u8); 32]);
        let (s, p) = refresh_dkg_part_1::<C, _>(ident::<C>(i)?, total, threshold, &mut rng)
            .map_err(|e| FrostError::DkgError(e.to_string()))?;
        r1_secret.insert(i, s);
        r1_pkgs.insert(ident::<C>(i)?, p);
    }
    // refresh round 2
    let mut r2_secret = BTreeMap::new();
    let mut r2_for: BTreeMap<u16, BTreeMap<Identifier<C>, _>> = BTreeMap::new();
    for &i in new_ids {
        let others: BTreeMap<_, _> = r1_pkgs
            .iter()
            .filter(|(k, _)| **k != ident::<C>(i).unwrap())
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let (s, sent) = refresh_dkg_part2::<C>(r1_secret.remove(&i).unwrap(), &others)
            .map_err(|e| FrostError::DkgError(e.to_string()))?;
        r2_secret.insert(i, s);
        for (rcpt, pkg) in sent {
            r2_for.entry_by_recipient(rcpt, i, pkg, new_ids);
        }
    }
    // refresh finalize — needs each participant's OLD key package + old pub key
    let mut new_kps = BTreeMap::new();
    let mut new_pub = None;
    for &i in new_ids {
        let others_r1: BTreeMap<_, _> = r1_pkgs
            .iter()
            .filter(|(k, _)| **k != ident::<C>(i).unwrap())
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let recv_r2 = r2_for.received_for(i)?;
        let (kp, pp) = refresh_dkg_shares::<C>(
            &r2_secret[&i],
            &others_r1,
            &recv_r2,
            old_pub.clone(),
            old_kps[&i].clone(),
        )
        .map_err(|e| FrostError::DkgError(e.to_string()))?;
        new_kps.insert(i, kp);
        new_pub = Some(pp);
    }
    Ok((new_kps, new_pub.unwrap()))
}

/// Sign `msg` with the given quorum of key packages and verify against the
/// public key package. Returns Ok(()) iff a valid threshold signature is
/// produced — used to prove refreshed shares work and old ones don't.
pub fn threshold_sign_verify<C: Ciphersuite>(
    kps: &BTreeMap<u16, KeyPackage<C>>,
    signer_ids: &[u16],
    pubpkg: &PublicKeyPackage<C>,
    msg: &[u8],
) -> Result<()> {
    use frost_core::{aggregate, round1, round2, SigningPackage};
    let mut nonces = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    for &i in signer_ids {
        let kp = kps
            .get(&i)
            .ok_or_else(|| FrostError::InvalidState(format!("no share for signer {i}")))?;
        let mut rng = ChaCha20Rng::from_seed([0x5a_u8.wrapping_add(i as u8); 32]);
        let (n, c) = round1::commit(kp.signing_share(), &mut rng);
        nonces.insert(i, n);
        commitments.insert(ident::<C>(i)?, c);
    }
    let signing_package = SigningPackage::new(commitments, msg);
    let mut shares = BTreeMap::new();
    for &i in signer_ids {
        let share = round2::sign(&signing_package, &nonces[&i], &kps[&i])
            .map_err(|e| FrostError::DkgError(e.to_string()))?;
        shares.insert(ident::<C>(i)?, share);
    }
    let sig = aggregate(&signing_package, &shares, pubpkg)
        .map_err(|e| FrostError::DkgError(e.to_string()))?;
    pubpkg
        .verifying_key()
        .verify(msg, &sig)
        .map_err(|e| FrostError::DkgError(format!("verify failed: {e}")))
}

// --- small helpers to route round-2 packages by recipient ------------------

trait Round2Routing<C: Ciphersuite, P> {
    fn entry_by_recipient(&mut self, recipient: Identifier<C>, sender: u16, pkg: P, ids: &[u16]);
    fn received_for(&self, me: u16) -> Result<BTreeMap<Identifier<C>, P>>;
}

impl<C: Ciphersuite, P: Clone> Round2Routing<C, P> for BTreeMap<u16, BTreeMap<Identifier<C>, P>> {
    fn entry_by_recipient(&mut self, recipient: Identifier<C>, sender: u16, pkg: P, ids: &[u16]) {
        // Map the recipient Identifier back to its u16 index.
        for &cand in ids {
            if let Ok(id) = ident::<C>(cand) {
                if id == recipient {
                    self.entry(cand).or_default().insert(ident::<C>(sender).unwrap(), pkg);
                    return;
                }
            }
        }
    }
    fn received_for(&self, me: u16) -> Result<BTreeMap<Identifier<C>, P>> {
        Ok(self.get(&me).cloned().unwrap_or_default())
    }
}

// --- distributed per-participant reshare session ---------------------------

/// Per-participant, transport-agnostic reshare state machine.
///
/// `refresh()` above runs every participant in one process — fine for tests
/// and demos, useless for real deployments where each device holds only its
/// own share. `ReshareSession` is the distributed counterpart: every device
/// constructs one with **its own** old `KeyPackage`, exchanges the round
/// packages over whatever transport it has (WebRTC mesh, WebSocket relay,
/// SD card), and finalizes into its **new** `KeyPackage` with the group
/// public key preserved.
///
/// Round flow (mirrors the DKG session shape consumers already know):
/// `round1()` → broadcast pkg → `add_round1(from, pkg)` ×(n−1) →
/// `round2()` → send each entry to its recipient → `add_round2(from, pkg)`
/// ×(n−1) → `finalize()`.
///
/// Same constraint as `refresh`: every id in `new_ids` must already hold a
/// share (refresh can rotate or REMOVE devices, never mint a share for a
/// brand-new one).
pub struct ReshareSession<C: Ciphersuite> {
    my_id: u16,
    threshold: u16,
    new_ids: Vec<u16>,
    old_kp: KeyPackage<C>,
    old_pub: PublicKeyPackage<C>,
    r1_secret: Option<frost_core::keys::dkg::round1::SecretPackage<C>>,
    r1_received: BTreeMap<Identifier<C>, frost_core::keys::dkg::round1::Package<C>>,
    r2_secret: Option<frost_core::keys::dkg::round2::SecretPackage<C>>,
    r2_received: BTreeMap<Identifier<C>, frost_core::keys::dkg::round2::Package<C>>,
}

impl<C: Ciphersuite> ReshareSession<C> {
    pub fn new(
        my_id: u16,
        threshold: u16,
        new_ids: Vec<u16>,
        old_kp: KeyPackage<C>,
        old_pub: PublicKeyPackage<C>,
    ) -> Result<Self> {
        if !new_ids.contains(&my_id) {
            return Err(FrostError::InvalidState(format!(
                "my id {my_id} is not in the new participant set"
            )));
        }
        if (new_ids.len() as u16) < threshold {
            return Err(FrostError::InvalidState(format!(
                "{} participants cannot meet threshold {threshold}",
                new_ids.len()
            )));
        }
        Ok(Self {
            my_id,
            threshold,
            new_ids,
            old_kp,
            old_pub,
            r1_secret: None,
            r1_received: BTreeMap::new(),
            r2_secret: None,
            r2_received: BTreeMap::new(),
        })
    }

    /// Generate this participant's round-1 package (broadcast it to all peers).
    pub fn round1<R: rand_core::RngCore + rand_core::CryptoRng>(
        &mut self,
        rng: &mut R,
    ) -> Result<frost_core::keys::dkg::round1::Package<C>> {
        let (secret, pkg) = refresh_dkg_part_1::<C, _>(
            ident::<C>(self.my_id)?,
            self.new_ids.len() as u16,
            self.threshold,
            rng,
        )
        .map_err(|e| FrostError::DkgError(e.to_string()))?;
        self.r1_secret = Some(secret);
        Ok(pkg)
    }

    /// Record a peer's round-1 package.
    pub fn add_round1(
        &mut self,
        from: u16,
        pkg: frost_core::keys::dkg::round1::Package<C>,
    ) -> Result<()> {
        if from == self.my_id || !self.new_ids.contains(&from) {
            return Err(FrostError::InvalidState(format!(
                "unexpected round1 sender {from}"
            )));
        }
        self.r1_received.insert(ident::<C>(from)?, pkg);
        Ok(())
    }

    /// True once round-1 packages from all n−1 peers have arrived.
    pub fn can_round2(&self) -> bool {
        self.r1_secret.is_some() && self.r1_received.len() == self.new_ids.len() - 1
    }

    /// Generate the per-recipient round-2 packages (send each to its peer).
    pub fn round2(&mut self) -> Result<BTreeMap<u16, frost_core::keys::dkg::round2::Package<C>>> {
        if !self.can_round2() {
            return Err(FrostError::InvalidState("round1 not complete".into()));
        }
        let secret = self
            .r1_secret
            .take()
            .ok_or_else(|| FrostError::InvalidState("round2 already generated".into()))?;
        let (r2_secret, sent) = refresh_dkg_part2::<C>(secret, &self.r1_received)
            .map_err(|e| FrostError::DkgError(e.to_string()))?;
        self.r2_secret = Some(r2_secret);
        // Map recipient Identifier back to u16 for the transport layer.
        let mut out = BTreeMap::new();
        for (rcpt, pkg) in sent {
            for &cand in &self.new_ids {
                if ident::<C>(cand)? == rcpt {
                    out.insert(cand, pkg.clone());
                }
            }
        }
        Ok(out)
    }

    /// Record the round-2 package a peer sent specifically to us.
    pub fn add_round2(
        &mut self,
        from: u16,
        pkg: frost_core::keys::dkg::round2::Package<C>,
    ) -> Result<()> {
        if from == self.my_id || !self.new_ids.contains(&from) {
            return Err(FrostError::InvalidState(format!(
                "unexpected round2 sender {from}"
            )));
        }
        self.r2_received.insert(ident::<C>(from)?, pkg);
        Ok(())
    }

    /// True once round-2 packages from all n−1 peers have arrived.
    pub fn can_finalize(&self) -> bool {
        self.r2_secret.is_some() && self.r2_received.len() == self.new_ids.len() - 1
    }

    /// Produce this participant's refreshed key package. The group public key
    /// in the returned `PublicKeyPackage` equals the old one (address stable).
    pub fn finalize(&mut self) -> Result<(KeyPackage<C>, PublicKeyPackage<C>)> {
        if !self.can_finalize() {
            return Err(FrostError::InvalidState("round2 not complete".into()));
        }
        let r2_secret = self
            .r2_secret
            .take()
            .ok_or_else(|| FrostError::InvalidState("already finalized".into()))?;
        refresh_dkg_shares::<C>(
            &r2_secret,
            &self.r1_received,
            &self.r2_received,
            self.old_pub.clone(),
            self.old_kp.clone(),
        )
        .map_err(|e| FrostError::DkgError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frost_secp256k1::Secp256K1Sha256 as Secp;

    #[test]
    fn refresh_preserves_group_key_same_set() {
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 10).unwrap();
        let before = group_key_hex(&pp).unwrap();
        let (new_kps, new_pp) = refresh::<Secp>(&kps, &pp, &[1, 2, 3], 2, 50).unwrap();
        assert_eq!(group_key_hex(&new_pp).unwrap(), before, "group key must not change");
        // refreshed quorum can sign
        threshold_sign_verify::<Secp>(&new_kps, &[1, 2], &new_pp, b"after refresh").unwrap();
    }

    #[test]
    fn old_share_cannot_mix_with_refreshed_shares() {
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 11).unwrap();
        let (new_kps, new_pp) = refresh::<Secp>(&kps, &pp, &[1, 2, 3], 2, 51).unwrap();
        // Mix an OLD share (id 1) with a NEW share (id 2) → must NOT verify.
        let mut mixed = BTreeMap::new();
        mixed.insert(1u16, kps[&1].clone()); // stale
        mixed.insert(2u16, new_kps[&2].clone()); // refreshed
        let r = threshold_sign_verify::<Secp>(&mixed, &[1, 2], &new_pp, b"mix");
        assert!(r.is_err(), "stale + refreshed shares must not produce a valid signature");
    }

    #[test]
    fn refresh_can_remove_a_participant() {
        // 2-of-3 → drop participant 3, refresh among {1,2} as 2-of-2.
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 12).unwrap();
        let before = group_key_hex(&pp).unwrap();
        let (new_kps, new_pp) = refresh::<Secp>(&kps, &pp, &[1, 2], 2, 52).unwrap();
        assert_eq!(group_key_hex(&new_pp).unwrap(), before, "address preserved after removal");
        assert_eq!(new_kps.len(), 2, "removed participant has no new share");
        threshold_sign_verify::<Secp>(&new_kps, &[1, 2], &new_pp, b"after removal").unwrap();
        // The removed participant's old share is now useless against the new set.
        let mut with_removed = new_kps.clone();
        with_removed.insert(3u16, kps[&3].clone());
        assert!(
            threshold_sign_verify::<Secp>(&with_removed, &[1, 3], &new_pp, b"x").is_err(),
            "removed participant's old share must not sign with the refreshed group"
        );
    }

    #[test]
    fn cannot_refresh_a_participant_without_an_existing_share() {
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 13).unwrap();
        // id 9 never had a share.
        let err = refresh::<Secp>(&kps, &pp, &[1, 2, 9], 2, 53);
        assert!(err.is_err(), "refresh must reject a participant with no prior share");
    }

    // --- Phase 1 gate (#45): NON-CONTIGUOUS identifiers ---------------------
    // Removing a *middle* device leaves the survivors holding their original
    // (now non-contiguous) FROST ids, e.g. {1,3}. The networked ceremony MUST
    // keep those original ids (not recompute canonical_identifier over the
    // reduced set). These tests prove frost-core's refresh accepts that: a
    // refresh over ids {1,3} with max_signers=2 preserves the group key and the
    // surviving pair can sign. If this ever fails, the removal feature needs
    // the trusted-dealer fallback (see RESHARE_CEREMONY_DESIGN.md §3).
    #[test]
    fn refresh_with_noncontiguous_ids_removing_the_middle_device() {
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 14).unwrap();
        let before = group_key_hex(&pp).unwrap();
        // Remove the MIDDLE device (id 2) → survivors keep ids {1,3}.
        let (new_kps, new_pp) = refresh::<Secp>(&kps, &pp, &[1, 3], 2, 54).unwrap();
        assert_eq!(
            group_key_hex(&new_pp).unwrap(),
            before,
            "group key (address) must survive a non-contiguous-id refresh"
        );
        assert_eq!(new_kps.keys().copied().collect::<Vec<_>>(), vec![1, 3]);
        // The surviving {1,3} pair can sign with the refreshed shares.
        threshold_sign_verify::<Secp>(&new_kps, &[1, 3], &new_pp, b"noncontig").unwrap();
        // The removed device (id 2) and any stale share are dead against the
        // refreshed group.
        let mut with_removed = new_kps.clone();
        with_removed.insert(2u16, kps[&2].clone());
        assert!(
            threshold_sign_verify::<Secp>(&with_removed, &[1, 2], &new_pp, b"x").is_err(),
            "removed middle device's old share must not sign the refreshed group"
        );
    }

    #[test]
    fn refresh_noncontiguous_ids_ed25519() {
        // Same as above on ed25519, to be sure it's curve-agnostic.
        use frost_ed25519::Ed25519Sha512 as Ed;
        let (kps, pp) = dkg_keypackages::<Ed>(3, 2, 15).unwrap();
        let before = group_key_hex(&pp).unwrap();
        let (new_kps, new_pp) = refresh::<Ed>(&kps, &pp, &[1, 3], 2, 55).unwrap();
        assert_eq!(group_key_hex(&new_pp).unwrap(), before);
        threshold_sign_verify::<Ed>(&new_kps, &[1, 3], &new_pp, b"noncontig-ed").unwrap();
    }

    #[test]
    fn refresh_5_of_5_down_to_3_of_5_arbitrary_subset() {
        // Larger, more adversarial: 3-of-5, keep a scattered subset {2,4,5}
        // (drop 1 and 3). Group key preserved; the 3 survivors sign.
        let (kps, pp) = dkg_keypackages::<Secp>(5, 3, 16).unwrap();
        let before = group_key_hex(&pp).unwrap();
        let (new_kps, new_pp) = refresh::<Secp>(&kps, &pp, &[2, 4, 5], 3, 56).unwrap();
        assert_eq!(group_key_hex(&new_pp).unwrap(), before);
        assert_eq!(new_kps.keys().copied().collect::<Vec<_>>(), vec![2, 4, 5]);
        threshold_sign_verify::<Secp>(&new_kps, &[2, 4, 5], &new_pp, b"scattered").unwrap();
    }

    /// Drive N independent ReshareSession instances exactly the way real
    /// devices would over a transport: round1 broadcast → round2 directed →
    /// finalize. Returns each participant's new key package + the public pkg.
    fn drive_sessions<C: Ciphersuite>(
        kps: &BTreeMap<u16, KeyPackage<C>>,
        pp: &PublicKeyPackage<C>,
        new_ids: &[u16],
        threshold: u16,
        seed: u8,
    ) -> Result<(BTreeMap<u16, KeyPackage<C>>, PublicKeyPackage<C>)> {
        let mut sessions: BTreeMap<u16, ReshareSession<C>> = BTreeMap::new();
        for &i in new_ids {
            sessions.insert(
                i,
                ReshareSession::new(i, threshold, new_ids.to_vec(), kps[&i].clone(), pp.clone())?,
            );
        }
        // round 1: everyone broadcasts
        let mut r1 = BTreeMap::new();
        for &i in new_ids {
            let mut rng = ChaCha20Rng::from_seed([seed.wrapping_add(i as u8); 32]);
            r1.insert(i, sessions.get_mut(&i).unwrap().round1(&mut rng)?);
        }
        for &i in new_ids {
            for &j in new_ids {
                if i != j {
                    sessions.get_mut(&i).unwrap().add_round1(j, r1[&j].clone())?;
                }
            }
        }
        // round 2: directed sends
        let mut r2: BTreeMap<u16, BTreeMap<u16, _>> = BTreeMap::new();
        for &i in new_ids {
            assert!(sessions[&i].can_round2());
            r2.insert(i, sessions.get_mut(&i).unwrap().round2()?);
        }
        for &sender in new_ids {
            for (&rcpt, pkg) in &r2[&sender] {
                sessions.get_mut(&rcpt).unwrap().add_round2(sender, pkg.clone())?;
            }
        }
        // finalize
        let mut new_kps = BTreeMap::new();
        let mut new_pp = None;
        for &i in new_ids {
            assert!(sessions[&i].can_finalize());
            let (kp, p) = sessions.get_mut(&i).unwrap().finalize()?;
            new_kps.insert(i, kp);
            new_pp = Some(p);
        }
        Ok((new_kps, new_pp.unwrap()))
    }

    #[test]
    fn distributed_session_rotates_and_preserves_group_key() {
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 21).unwrap();
        let before = group_key_hex(&pp).unwrap();
        let (new_kps, new_pp) = drive_sessions::<Secp>(&kps, &pp, &[1, 2, 3], 2, 61).unwrap();
        assert_eq!(group_key_hex(&new_pp).unwrap(), before);
        threshold_sign_verify::<Secp>(&new_kps, &[1, 3], &new_pp, b"session-rotate").unwrap();
        // Old shares must NOT combine with new ones: mix old kp 1 + new kp 2.
        let mut mixed = BTreeMap::new();
        mixed.insert(1u16, kps[&1].clone());
        mixed.insert(2u16, new_kps[&2].clone());
        assert!(threshold_sign_verify::<Secp>(&mixed, &[1, 2], &new_pp, b"mixed").is_err());
    }

    #[test]
    fn distributed_session_removes_a_device() {
        // 2-of-3 → reshare among {1,2}: device 3's old share goes dead.
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 22).unwrap();
        let before = group_key_hex(&pp).unwrap();
        let (new_kps, new_pp) = drive_sessions::<Secp>(&kps, &pp, &[1, 2], 2, 62).unwrap();
        assert_eq!(group_key_hex(&new_pp).unwrap(), before);
        threshold_sign_verify::<Secp>(&new_kps, &[1, 2], &new_pp, b"session-remove").unwrap();
    }

    #[test]
    fn distributed_session_works_on_ed25519_too() {
        use frost_ed25519::Ed25519Sha512 as Ed;
        let (kps, pp) = dkg_keypackages::<Ed>(3, 2, 23).unwrap();
        let before = group_key_hex(&pp).unwrap();
        let (new_kps, new_pp) = drive_sessions::<Ed>(&kps, &pp, &[1, 2, 3], 2, 63).unwrap();
        assert_eq!(group_key_hex(&new_pp).unwrap(), before);
        threshold_sign_verify::<Ed>(&new_kps, &[2, 3], &new_pp, b"session-ed").unwrap();
    }

    #[test]
    fn session_rejects_outsider_and_bad_config() {
        let (kps, pp) = dkg_keypackages::<Secp>(3, 2, 24).unwrap();
        // my_id not in the new set
        assert!(ReshareSession::<Secp>::new(3, 2, vec![1, 2], kps[&3].clone(), pp.clone()).is_err());
        // too few participants for the threshold
        assert!(ReshareSession::<Secp>::new(1, 3, vec![1, 2], kps[&1].clone(), pp.clone()).is_err());
        // unexpected sender
        let mut s = ReshareSession::<Secp>::new(1, 2, vec![1, 2], kps[&1].clone(), pp).unwrap();
        let mut rng = ChaCha20Rng::from_seed([7u8; 32]);
        let pkg = s.round1(&mut rng).unwrap();
        assert!(s.add_round1(9, pkg).is_err());
    }
}
