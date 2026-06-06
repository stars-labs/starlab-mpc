//! `reshare-simulate` — exercise the share-refresh/resharing engine end to end
//! from the CLI (#45). Runs a DKG, then refreshes the shares for a (possibly
//! reduced) participant set in-process via `mpc_wallet_frost_core::resharing`,
//! and asserts the recovery guarantees:
//!
//! - the group public key (your address) is **unchanged** by the refresh,
//! - the refreshed quorum **can sign**,
//! - an **old share can no longer sign** with the refreshed group.
//!
//! This is the CLI surface over the (tested) resharing primitive. It runs the
//! engine in-process — the networked `reshare` ceremony over the WebRTC mesh
//! (a `reshare` session type + driver, analogous to DKG) is the remaining work
//! on #45; this proves the cryptographic path and gives a CI-able / demoable
//! command in the meantime.

use frost_core::Ciphersuite;
use mpc_wallet_frost_core::resharing;
use std::collections::BTreeMap;

#[derive(serde::Serialize)]
pub struct ReshareResult {
    pub nodes: usize,
    pub threshold: u16,
    pub curve: String,
    pub kept: Vec<u16>,
    pub group_public_key: String,
    /// group key identical before and after the refresh.
    pub key_preserved: bool,
    /// the refreshed quorum produced a valid signature.
    pub refreshed_quorum_signs: bool,
    /// a stale (pre-refresh) share could NOT sign with the refreshed group.
    pub old_share_rejected: bool,
    /// all three invariants held.
    pub ok: bool,
}

impl ReshareResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

fn group_key_hex<C: Ciphersuite>(
    p: &frost_core::keys::PublicKeyPackage<C>,
) -> anyhow::Result<String> {
    Ok(hex::encode(
        p.verifying_key()
            .serialize()
            .map_err(|e| anyhow::anyhow!("serialize verifying key: {e}"))?,
    ))
}

fn run<C: Ciphersuite>(
    total: u16,
    threshold: u16,
    keep: &[u16],
) -> anyhow::Result<(String, bool, bool, bool)> {
    // 1. Initial wallet.
    let (kps, pp) = resharing::dkg_keypackages::<C>(total, threshold, 10)
        .map_err(|e| anyhow::anyhow!("dkg: {e}"))?;
    let before = group_key_hex(&pp)?;

    // 2. Refresh shares for the retained set.
    let (new_kps, new_pp) = resharing::refresh::<C>(&kps, &pp, keep, threshold, 50)
        .map_err(|e| anyhow::anyhow!("refresh: {e}"))?;
    let after = group_key_hex(&new_pp)?;
    let key_preserved = before == after;

    // 3. Refreshed quorum signs.
    let quorum: Vec<u16> = keep.iter().take(threshold as usize).copied().collect();
    let signs =
        resharing::threshold_sign_verify::<C>(&new_kps, &quorum, &new_pp, b"reshare-sim").is_ok();

    // 4. A stale share must not sign with the refreshed group: swap one quorum
    //    member's refreshed share for its OLD one.
    let mut mixed: BTreeMap<u16, _> = BTreeMap::new();
    for (idx, id) in quorum.iter().enumerate() {
        if idx == 0 {
            mixed.insert(*id, kps[id].clone()); // stale
        } else {
            mixed.insert(*id, new_kps[id].clone()); // refreshed
        }
    }
    let old_rejected =
        resharing::threshold_sign_verify::<C>(&mixed, &quorum, &new_pp, b"reshare-sim").is_err();

    Ok((after, key_preserved, signs, old_rejected))
}

/// Run the resharing simulation for `curve` ("secp256k1" | "ed25519").
pub fn run_reshare_simulation(
    nodes: usize,
    threshold: u16,
    curve: &str,
    keep: Vec<u16>,
) -> anyhow::Result<ReshareResult> {
    let total = nodes as u16;
    if threshold < 1 || threshold > total {
        anyhow::bail!("threshold {threshold} out of range for {total} nodes");
    }
    let keep = if keep.is_empty() {
        (1..=total).collect::<Vec<_>>()
    } else {
        keep
    };
    if (keep.len() as u16) < threshold {
        anyhow::bail!(
            "kept set ({}) is smaller than the threshold ({threshold})",
            keep.len()
        );
    }

    let (gpk, key_preserved, signs, old_rejected) = match curve {
        "ed25519" => run::<frost_ed25519::Ed25519Sha512>(total, threshold, &keep)?,
        "secp256k1" => run::<frost_secp256k1::Secp256K1Sha256>(total, threshold, &keep)?,
        other => anyhow::bail!("unsupported curve: {other}"),
    };

    Ok(ReshareResult {
        nodes,
        threshold,
        curve: curve.to_string(),
        kept: keep,
        group_public_key: gpk,
        key_preserved,
        refreshed_quorum_signs: signs,
        old_share_rejected: old_rejected,
        ok: key_preserved && signs && old_rejected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reshare_simulation_secp256k1_same_set() {
        let r = run_reshare_simulation(3, 2, "secp256k1", vec![]).unwrap();
        assert!(r.key_preserved && r.refreshed_quorum_signs && r.old_share_rejected && r.ok);
    }

    #[test]
    fn reshare_simulation_removes_a_participant() {
        // 2-of-3 → keep only {1,2}; address preserved, signs, old share dead.
        let r = run_reshare_simulation(3, 2, "secp256k1", vec![1, 2]).unwrap();
        assert_eq!(r.kept, vec![1, 2]);
        assert!(r.ok);
    }

    #[test]
    fn reshare_simulation_ed25519() {
        let r = run_reshare_simulation(3, 2, "ed25519", vec![]).unwrap();
        assert!(r.ok);
    }
}
