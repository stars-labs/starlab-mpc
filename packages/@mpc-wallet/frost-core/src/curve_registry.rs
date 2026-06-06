//! Curve registry: a tag → ciphersuite table so multi-curve DKG is a *loop
//! over registered curves* instead of hard-coded ed25519/secp256k1 arms.
//!
//! The existing [`crate::unified_dkg::UnifiedDkg`] hand-lists exactly two
//! curves with duplicated round-1/2/3 code. This module factors the curve
//! axis out:
//!
//! - [`CurveDkg`] is an **object-safe** trait that runs a FROST DKG over
//!   *hex-serialized* packages and type-erased secret state (`Box<dyn Any>`),
//!   so curves of different ciphersuites can sit behind one `dyn` pointer.
//! - [`FrostCurveDkg<C>`] is a **single generic implementation** that works
//!   for *any* `frost_core::Ciphersuite` — it calls the generic
//!   `frost_core::keys::dkg::{part1,part2,part3}`. Adding a curve is one line
//!   ([`CurveRegistry::register`]), with **zero** changes to the engine.
//! - [`run_dkg_simulation`] is the generic engine: it iterates the registry
//!   and runs a full N-party DKG per curve, returning each participant's group
//!   public key. It contains no curve-specific code, which is exactly the
//!   property the registry buys.
//!
//! Identifiers here use frost-core's canonical `Identifier::try_from(u16)`
//! (1-based), independent of `unified_dkg`'s 32-byte big-endian encoding —
//! this is a self-contained path, internally consistent across all
//! participants of a run.

use crate::errors::{FrostError, Result};
use crate::root_secret::RootSecret;
use frost_core::keys::dkg::{part1, part2, part3, round1, round2};
use frost_core::{Ciphersuite, Identifier};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use std::any::Any;
use std::collections::BTreeMap;
use std::marker::PhantomData;

/// Output of a finalized per-curve DKG for one participant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurveDkgOutput {
    /// Canonical curve tag (e.g. `"ed25519"`).
    pub tag: String,
    /// Group verifying key, hex-encoded (identical across all participants).
    pub group_public_key_hex: String,
    /// This participant's serialized key package, hex-encoded.
    pub key_package_hex: String,
}

/// Object-safe DKG surface for one curve, over hex packages + erased secrets.
///
/// Every method is curve-agnostic in signature so the engine can drive any
/// number of curves through a `&dyn CurveDkg`. Secret state crosses rounds as
/// `Box<dyn Any>` (held in-process only; never serialized).
pub trait CurveDkg: Send + Sync {
    /// Canonical, stable curve tag — also the HKDF domain-separation label.
    fn tag(&self) -> &'static str;

    /// Round 1: returns this participant's round-1 secret (erased) and the
    /// round-1 package (hex JSON) to broadcast.
    fn part1(
        &self,
        index: u16,
        total: u16,
        threshold: u16,
        seed: [u8; 32],
    ) -> Result<(Box<dyn Any>, String)>;

    /// Round 2: consumes the round-1 secret + the *other* participants'
    /// round-1 packages (keyed by 1-based index). Returns the round-2 secret
    /// (erased) and the per-recipient round-2 packages (hex JSON).
    fn part2(
        &self,
        round1_secret: Box<dyn Any>,
        others_round1: &BTreeMap<u16, String>,
    ) -> Result<(Box<dyn Any>, BTreeMap<u16, String>)>;

    /// Round 3 (finalize): consumes the round-2 secret + the other
    /// participants' round-1 and round-2 packages. Returns the group key +
    /// this participant's key package.
    fn part3(
        &self,
        round2_secret: Box<dyn Any>,
        others_round1: &BTreeMap<u16, String>,
        others_round2: &BTreeMap<u16, String>,
    ) -> Result<CurveDkgOutput>;
}

/// The single generic `CurveDkg` implementation, valid for any ciphersuite.
pub struct FrostCurveDkg<C: Ciphersuite> {
    tag: &'static str,
    _curve: PhantomData<C>,
}

impl<C: Ciphersuite> FrostCurveDkg<C> {
    /// Register a ciphersuite `C` under a canonical `tag`.
    pub const fn new(tag: &'static str) -> Self {
        Self {
            tag,
            _curve: PhantomData,
        }
    }

    /// Boxed convenience for `CurveRegistry::register`.
    pub fn boxed(tag: &'static str) -> Box<dyn CurveDkg>
    where
        C: Send + Sync + 'static,
    {
        Box::new(Self::new(tag))
    }
}

fn ident<C: Ciphersuite>(index: u16) -> Result<Identifier<C>> {
    Identifier::<C>::try_from(index)
        .map_err(|e| FrostError::InvalidIdentifier(format!("index {index}: {e}")))
}

fn to_hex<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v)
        .map(|s| hex::encode(s))
        .map_err(|e| FrostError::SerializationError(e.to_string()))
}

fn from_hex<T: for<'de> serde::Deserialize<'de>>(s: &str) -> Result<T> {
    let json = hex::decode(s).map_err(|e| FrostError::SerializationError(e.to_string()))?;
    serde_json::from_slice(&json).map_err(|e| FrostError::SerializationError(e.to_string()))
}

/// Decode an `others` map (index → hex package) into a frost identifier map.
fn decode_pkgs<C, P>(others: &BTreeMap<u16, String>) -> Result<BTreeMap<Identifier<C>, P>>
where
    C: Ciphersuite,
    P: for<'de> serde::Deserialize<'de>,
{
    let mut out = BTreeMap::new();
    for (idx, hexs) in others {
        out.insert(ident::<C>(*idx)?, from_hex::<P>(hexs)?);
    }
    Ok(out)
}

impl<C> CurveDkg for FrostCurveDkg<C>
where
    C: Ciphersuite + Send + Sync + 'static,
{
    fn tag(&self) -> &'static str {
        self.tag
    }

    fn part1(
        &self,
        index: u16,
        total: u16,
        threshold: u16,
        seed: [u8; 32],
    ) -> Result<(Box<dyn Any>, String)> {
        let mut rng = ChaCha20Rng::from_seed(seed);
        let (secret, package) = part1::<C, _>(ident::<C>(index)?, total, threshold, &mut rng)
            .map_err(|e| FrostError::DkgError(e.to_string()))?;
        Ok((Box::new(secret), to_hex(&package)?))
    }

    fn part2(
        &self,
        round1_secret: Box<dyn Any>,
        others_round1: &BTreeMap<u16, String>,
    ) -> Result<(Box<dyn Any>, BTreeMap<u16, String>)> {
        let secret = *round1_secret
            .downcast::<round1::SecretPackage<C>>()
            .map_err(|_| FrostError::InvalidState("round1 secret type mismatch".into()))?;
        let r1 = decode_pkgs::<C, round1::Package<C>>(others_round1)?;
        let (r2_secret, r2_pkgs) =
            part2::<C>(secret, &r1).map_err(|e| FrostError::DkgError(e.to_string()))?;

        // Reverse identifier → index using the known recipient set.
        let mut out = BTreeMap::new();
        for idx in others_round1.keys() {
            let id = ident::<C>(*idx)?;
            if let Some(pkg) = r2_pkgs.get(&id) {
                out.insert(*idx, to_hex(pkg)?);
            }
        }
        Ok((Box::new(r2_secret), out))
    }

    fn part3(
        &self,
        round2_secret: Box<dyn Any>,
        others_round1: &BTreeMap<u16, String>,
        others_round2: &BTreeMap<u16, String>,
    ) -> Result<CurveDkgOutput> {
        let secret = *round2_secret
            .downcast::<round2::SecretPackage<C>>()
            .map_err(|_| FrostError::InvalidState("round2 secret type mismatch".into()))?;
        let r1 = decode_pkgs::<C, round1::Package<C>>(others_round1)?;
        let r2 = decode_pkgs::<C, round2::Package<C>>(others_round2)?;
        let (key_package, pub_package) =
            part3::<C>(&secret, &r1, &r2).map_err(|e| FrostError::DkgError(e.to_string()))?;

        let vk = pub_package.verifying_key();
        let vk_bytes = vk
            .serialize()
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;

        Ok(CurveDkgOutput {
            tag: self.tag.to_string(),
            group_public_key_hex: hex::encode(vk_bytes),
            key_package_hex: to_hex(&key_package)?,
        })
    }
}

/// A registry of curves keyed by their canonical tag.
#[derive(Default)]
pub struct CurveRegistry {
    curves: Vec<Box<dyn CurveDkg>>,
}

impl CurveRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self { curves: Vec::new() }
    }

    /// The two production curves: ed25519 + secp256k1.
    pub fn with_default_curves() -> Self {
        let mut r = Self::new();
        r.register(FrostCurveDkg::<frost_ed25519::Ed25519Sha512>::boxed(
            crate::root_secret::CURVE_ED25519,
        ));
        r.register(FrostCurveDkg::<frost_secp256k1::Secp256K1Sha256>::boxed(
            crate::root_secret::CURVE_SECP256K1,
        ));
        r
    }

    /// Register a curve. Duplicate tags are rejected (last-wins would silently
    /// shadow a curve).
    pub fn register(&mut self, curve: Box<dyn CurveDkg>) -> &mut Self {
        let tag = curve.tag();
        assert!(
            !self.curves.iter().any(|c| c.tag() == tag),
            "curve tag already registered: {tag}"
        );
        self.curves.push(curve);
        self
    }

    /// Registered curve tags, in registration order.
    pub fn tags(&self) -> Vec<&'static str> {
        self.curves.iter().map(|c| c.tag()).collect()
    }

    /// Iterate the registered curves.
    pub fn iter(&self) -> impl Iterator<Item = &dyn CurveDkg> {
        self.curves.iter().map(|b| b.as_ref())
    }

    /// Number of registered curves.
    pub fn len(&self) -> usize {
        self.curves.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.curves.is_empty()
    }
}

/// Generic, curve-agnostic DKG engine.
///
/// Runs a full `threshold`-of-`total` DKG for **every** curve in the registry,
/// in-process, and returns each participant's [`CurveDkgOutput`] grouped by
/// curve tag. Per-participant entropy is the supplied `RootSecret`s expanded
/// through the domain-separated derivation (`frost-dkg/v1/<curve>/<account>`).
///
/// This function has **no curve-specific code** — adding an N-th curve to the
/// registry needs zero edits here. That is the whole point of the registry.
pub fn run_dkg_simulation(
    registry: &CurveRegistry,
    roots: &[RootSecret],
    threshold: u16,
    account: u32,
) -> Result<BTreeMap<String, Vec<CurveDkgOutput>>> {
    let total = roots.len() as u16;
    if total < 2 || threshold < 1 || threshold > total {
        return Err(FrostError::DkgError(format!(
            "invalid (threshold={threshold}, total={total})"
        )));
    }

    let mut result: BTreeMap<String, Vec<CurveDkgOutput>> = BTreeMap::new();

    for curve in registry.iter() {
        let tag = curve.tag();
        let indices: Vec<u16> = (1..=total).collect();

        // Round 1: every participant produces its secret + broadcast package.
        let mut r1_secrets: Vec<Option<Box<dyn Any>>> = Vec::with_capacity(roots.len());
        let mut r1_pkgs: BTreeMap<u16, String> = BTreeMap::new();
        for (i, root) in roots.iter().enumerate() {
            let idx = indices[i];
            let seed = root.derive_seed(tag, account)?;
            let (secret, pkg) = curve.part1(idx, total, threshold, seed)?;
            r1_secrets.push(Some(secret));
            r1_pkgs.insert(idx, pkg);
        }

        // Round 2: each participant runs part2 over the *others'* round-1
        // packages, producing per-recipient round-2 packages.
        let mut r2_secrets: Vec<Option<Box<dyn Any>>> = Vec::with_capacity(roots.len());
        // sent[sender][recipient] = hex round2 package
        let mut sent: BTreeMap<u16, BTreeMap<u16, String>> = BTreeMap::new();
        for (i, idx) in indices.iter().enumerate() {
            let others_r1: BTreeMap<u16, String> = r1_pkgs
                .iter()
                .filter(|(k, _)| *k != idx)
                .map(|(k, v)| (*k, v.clone()))
                .collect();
            let secret = r1_secrets[i]
                .take()
                .ok_or_else(|| FrostError::InvalidState("missing round1 secret".into()))?;
            let (r2_secret, r2_out) = curve.part2(secret, &others_r1)?;
            r2_secrets.push(Some(r2_secret));
            sent.insert(*idx, r2_out);
        }

        // Round 3: finalize. Each participant collects the round-2 packages
        // addressed to it (one from every other participant).
        let mut outputs = Vec::with_capacity(roots.len());
        for (i, idx) in indices.iter().enumerate() {
            let others_r1: BTreeMap<u16, String> = r1_pkgs
                .iter()
                .filter(|(k, _)| *k != idx)
                .map(|(k, v)| (*k, v.clone()))
                .collect();
            let recv_r2: BTreeMap<u16, String> = sent
                .iter()
                .filter(|(sender, _)| *sender != idx)
                .filter_map(|(sender, m)| m.get(idx).map(|hexs| (*sender, hexs.clone())))
                .collect();
            let secret = r2_secrets[i]
                .take()
                .ok_or_else(|| FrostError::InvalidState("missing round2 secret".into()))?;
            outputs.push(curve.part3(secret, &others_r1, &recv_r2)?);
        }

        result.insert(tag.to_string(), outputs);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(n: usize) -> Vec<RootSecret> {
        (0..n)
            .map(|i| RootSecret::from_bytes([i as u8 + 1; 32]))
            .collect()
    }

    fn assert_all_agree(outputs: &[CurveDkgOutput]) {
        let first = &outputs[0].group_public_key_hex;
        assert!(!first.is_empty(), "group key must be set");
        for o in outputs {
            assert_eq!(&o.group_public_key_hex, first, "all participants must agree");
        }
    }

    #[test]
    fn default_registry_has_both_curves() {
        let reg = CurveRegistry::with_default_curves();
        assert_eq!(reg.tags(), vec!["ed25519", "secp256k1"]);
    }

    #[test]
    fn two_curve_dkg_through_registry() {
        let reg = CurveRegistry::with_default_curves();
        let out = run_dkg_simulation(&reg, &roots(3), 2, 0).unwrap();
        assert_eq!(out.len(), 2);
        for tag in ["ed25519", "secp256k1"] {
            let per = &out[tag];
            assert_eq!(per.len(), 3);
            assert_all_agree(per);
        }
    }

    /// The headline acceptance for #35: registering a THIRD curve runs DKG for
    /// it with ZERO changes to `run_dkg_simulation` / the engine — only one
    /// `register(...)` line.
    #[test]
    fn third_curve_registers_without_engine_edits() {
        let mut reg = CurveRegistry::with_default_curves();
        reg.register(FrostCurveDkg::<frost_ristretto255::Ristretto255Sha512>::boxed(
            "ristretto255",
        ));
        assert_eq!(reg.tags(), vec!["ed25519", "secp256k1", "ristretto255"]);

        let out = run_dkg_simulation(&reg, &roots(3), 2, 0).unwrap();
        assert_eq!(out.len(), 3);
        for tag in ["ed25519", "secp256k1", "ristretto255"] {
            let per = &out[tag];
            assert_eq!(per.len(), 3, "{tag} should have 3 participants");
            assert_all_agree(per);
        }
    }

    #[test]
    fn different_accounts_yield_different_group_keys() {
        let reg = CurveRegistry::with_default_curves();
        let r = roots(2);
        let a0 = run_dkg_simulation(&reg, &r, 2, 0).unwrap();
        let a1 = run_dkg_simulation(&reg, &r, 2, 1).unwrap();
        assert_ne!(
            a0["secp256k1"][0].group_public_key_hex, a1["secp256k1"][0].group_public_key_hex,
            "different accounts must produce different wallets"
        );
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn duplicate_tag_panics() {
        let mut reg = CurveRegistry::new();
        reg.register(FrostCurveDkg::<frost_ed25519::Ed25519Sha512>::boxed("ed25519"));
        reg.register(FrostCurveDkg::<frost_secp256k1::Secp256K1Sha256>::boxed("ed25519"));
    }
}
