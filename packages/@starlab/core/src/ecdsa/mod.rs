//! Threshold ECDSA on secp256k1 (CGGMP24, via `cggmp24` 0.7) for EVM
//! accounts.
//!
//! A wallet's Ethereum account signs with this key (curve tag
//! [`ECDSA_CURVE`]); ed25519 chains and Bitcoin stay on FROST. Flow:
//!
//! 1. **Primes** — [`pregenerate_primes`] (slow: safe primes; run it in the
//!    background ahead of time, [`Primes`] is serde so it can be cached
//!    encrypted).
//! 2. **Aux info** — [`EcdsaCeremony::aux_info`] (Paillier moduli + ZK
//!    proofs). The [`AuxInfo`] is reusable for every key of the same signer
//!    group (same participants in the same index order).
//! 3. **Keygen** — [`EcdsaCeremony::keygen`] (threshold `t`-of-`n`), then
//!    [`EcdsaKeyShare::from_parts`] joins keygen output + aux info into the
//!    persistable share.
//! 4. **Signing** — [`EcdsaCeremony::signing`]: exactly `t` signers, full
//!    interactive protocol, HD child at any [`DerivationPath`] (our
//!    derivation, [`hd::StarlabHd`]). Presignatures are deliberately NOT
//!    exposed (cggmp24 0.7 forbids presignature + HD / raw hash).
//!
//! All ceremonies run through the byte-level, transport-agnostic
//! [`EcdsaCeremony`] driver (see [`driver`] for the wire format), with an
//! execution id from [`execution_id`].
//!
//! Known limitation: cggmp24 has no key refresh, so an ECDSA key can't be
//! reshared (see `IMPLEMENTATION_PLAN.md`).

pub mod driver;
pub mod hd;

pub use driver::{EcdsaCeremony, OutgoingMessage, Recipient, WIRE_VERSION};

use crate::errors::{FrostError, Result};
use crate::hd_derivation::{ChainCode, DerivationPath};
use crate::rng::{CryptoRng, RngCore};
use cggmp24::generic_ec::Scalar;
use cggmp24::key_share::{AnyKeyShare, Valid};
use cggmp24::security_level::SecurityLevel128;
use cggmp24::supported_curves::Secp256k1;
use cggmp24::{ExecutionId, PrehashedDataToSign};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Curve tag of the threshold-ECDSA key (accounts / keystore / wire).
pub const ECDSA_CURVE: &str = "secp256k1-ecdsa";

/// Pre-generated Paillier safe primes for one party's aux-info run.
pub type Primes = cggmp24::PregeneratedPrimes<SecurityLevel128>;
/// Output of the aux-info ceremony (this party's Paillier key + everyone's
/// public parameters). Secret; persist encrypted.
pub type AuxInfo = cggmp24::key_share::AuxInfo<SecurityLevel128>;
/// Output of the keygen ceremony, before joining with [`AuxInfo`].
pub type IncompleteKeyShare = cggmp24::IncompleteKeyShare<Secp256k1>;
/// A completed cggmp24 key share.
pub type KeyShare = cggmp24::KeyShare<Secp256k1, SecurityLevel128>;

pub type AuxInfoCeremony = EcdsaCeremony<AuxInfo>;
pub type KeygenCeremony = EcdsaCeremony<IncompleteKeyShare>;
pub type SigningCeremony = EcdsaCeremony<EcdsaSignature>;

/// Generate one party's Paillier safe primes. Takes seconds natively and
/// much longer in wasm — run it in the background before a DKG.
pub fn pregenerate_primes<R: RngCore + CryptoRng>(rng: &mut R) -> Primes {
    Primes::generate(rng)
}

/// One Paillier safe prime (a [`Primes`] set holds [`PRIME_COUNT`]).
/// serde: `{"radix":16,"value":"<hex>"}`.
pub type SafePrime = cggmp24::backend::Integer;

/// Safe primes per [`Primes`] set.
pub const PRIME_COUNT: usize = 4;

/// Generate ONE of the [`PRIME_COUNT`] safe primes of a [`Primes`] set —
/// the unit of work behind [`pregenerate_primes`], so a caller can spread
/// the four over parallel workers and report real progress (the browser
/// extension does). Join them with [`primes_from_parts`].
pub fn generate_safe_prime<R: RngCore + CryptoRng>(rng: &mut R) -> SafePrime {
    use cggmp24::security_level::SecurityLevel as _;
    SafePrime::generate_safe_prime(rng, SecurityLevel128::RSA_PRIME_BITLEN)
}

/// Join [`PRIME_COUNT`] primes from [`generate_safe_prime`] into a set.
/// Rejects numbers below the security level's size (primality itself is
/// checked by the aux-info ZK proofs, as with any [`Primes`]).
pub fn primes_from_parts(parts: Vec<SafePrime>) -> Result<Primes> {
    let parts: [SafePrime; PRIME_COUNT] = parts.try_into().map_err(|v: Vec<SafePrime>| {
        FrostError::EcdsaError(format!("need {PRIME_COUNT} safe primes, got {}", v.len()))
    })?;
    Primes::try_from(parts)
        .map_err(|_| FrostError::EcdsaError("safe prime below the required size".into()))
}

/// The three ECDSA protocols (execution-id tag + wire tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    AuxInfo,
    Keygen,
    Signing,
}

impl Protocol {
    /// Stable name mixed into the execution id.
    pub fn tag(self) -> &'static str {
        match self {
            Protocol::AuxInfo => "aux-info",
            Protocol::Keygen => "keygen",
            Protocol::Signing => "signing",
        }
    }

    pub(crate) fn wire_tag(self) -> u8 {
        match self {
            Protocol::AuxInfo => 1,
            Protocol::Keygen => 2,
            Protocol::Signing => 3,
        }
    }
}

const EID_DOMAIN: &[u8] = b"starlab-mpc/ecdsa/execution-id/v1";

/// Execution id of one ceremony:
/// `SHA-256(domain ‖ lp(session_id) ‖ lp(protocol tag) ‖ lp(id_1) ‖ … ‖ lp(id_k))`
/// with `lp(x) = u32_be(len(x)) ‖ x` and the participant ids sorted
/// (order-independent: every party computes the same id). Session ids must
/// never repeat.
pub fn execution_id(session_id: &str, protocol: Protocol, participants: &[String]) -> [u8; 32] {
    fn lp(hasher: &mut Sha256, bytes: &[u8]) {
        let len = u32::try_from(bytes.len()).expect("execution-id component under 4 GiB");
        hasher.update(len.to_be_bytes());
        hasher.update(bytes);
    }
    let mut sorted: Vec<&str> = participants.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    let mut hasher = Sha256::new();
    hasher.update(EID_DOMAIN);
    lp(&mut hasher, session_id.as_bytes());
    lp(&mut hasher, protocol.tag().as_bytes());
    for id in sorted {
        lp(&mut hasher, id.as_bytes());
    }
    hasher.finalize().into()
}

fn validate_group(participants: &[String], index: u16) -> Result<u16> {
    let n = u16::try_from(participants.len())
        .map_err(|_| FrostError::EcdsaError("too many participants".into()))?;
    if n < 2 {
        return Err(FrostError::EcdsaError(
            "need at least 2 participants".into(),
        ));
    }
    if index >= n {
        return Err(FrostError::EcdsaError(format!(
            "index {index} out of range for {n} participants"
        )));
    }
    let mut sorted = participants.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() != participants.len() {
        return Err(FrostError::EcdsaError("duplicate participant id".into()));
    }
    Ok(n)
}

impl EcdsaCeremony<AuxInfo> {
    /// Aux-info generation for party `index` of `participants` (keygen index
    /// order — must be the same order the keygen will use).
    pub fn aux_info<R>(
        session_id: &str,
        participants: &[String],
        index: u16,
        primes: Primes,
        rng: R,
    ) -> Result<Self>
    where
        R: RngCore + CryptoRng + 'static,
    {
        let n = validate_group(participants, index)?;
        let eid = execution_id(session_id, Protocol::AuxInfo, participants);
        let sm = cggmp24::round_based::state_machine::wrap_protocol(move |party| async move {
            let mut rng = rng;
            cggmp24::aux_info_gen(ExecutionId::new(&eid), index, n, primes)
                .start(&mut rng, party)
                .await
        });
        Ok(Self::new(
            Protocol::AuxInfo,
            eid,
            index,
            (0..n).collect(),
            sm,
        ))
    }
}

impl EcdsaCeremony<IncompleteKeyShare> {
    /// Threshold (`threshold`-of-`participants.len()`) key generation for
    /// party `index`.
    pub fn keygen<R>(
        session_id: &str,
        participants: &[String],
        index: u16,
        threshold: u16,
        rng: R,
    ) -> Result<Self>
    where
        R: RngCore + CryptoRng + 'static,
    {
        let n = validate_group(participants, index)?;
        if !(2..=n).contains(&threshold) {
            return Err(FrostError::EcdsaError(format!(
                "threshold {threshold} must be in 2..={n}"
            )));
        }
        let eid = execution_id(session_id, Protocol::Keygen, participants);
        let sm = cggmp24::round_based::state_machine::wrap_protocol(move |party| async move {
            let mut rng = rng;
            // hd_wallet(false): no jointly-random chain code — ours is
            // derived from the group key (`EcdsaKeyShare::from_parts`).
            cggmp24::keygen::<Secp256k1>(ExecutionId::new(&eid), index, n)
                .set_threshold(threshold)
                .hd_wallet(false)
                .start(&mut rng, party)
                .await
        });
        Ok(Self::new(
            Protocol::Keygen,
            eid,
            index,
            (0..n).collect(),
            sm,
        ))
    }
}

impl EcdsaCeremony<EcdsaSignature> {
    /// Sign the 32-byte `prehash` (e.g. an Ethereum keccak256 sighash) with
    /// the HD child at `path`, together with `signers` (keygen indices,
    /// exactly `threshold` of them, us included).
    ///
    /// Signing a prehash is safe here because this is always the full
    /// interactive protocol — never presignatures.
    pub fn signing<R>(
        session_id: &str,
        share: &EcdsaKeyShare,
        signers: &[u16],
        path: &DerivationPath,
        prehash: [u8; 32],
        rng: R,
    ) -> Result<Self>
    where
        R: RngCore + CryptoRng + 'static,
    {
        let mut parties = signers.to_vec();
        parties.sort_unstable();
        parties.dedup();
        if parties.len() != signers.len() {
            return Err(FrostError::EcdsaError("duplicate signer index".into()));
        }
        if parties.len() != usize::from(share.threshold()) {
            return Err(FrostError::EcdsaError(format!(
                "need exactly {} signers, got {}",
                share.threshold(),
                parties.len()
            )));
        }
        if let Some(bad) = parties.iter().find(|&&p| p >= share.n()) {
            return Err(FrostError::EcdsaError(format!(
                "unknown signer index {bad}"
            )));
        }
        let me = share.index();
        let local = parties
            .iter()
            .position(|&p| p == me)
            .ok_or_else(|| FrostError::EcdsaError("we are not among the signers".into()))?;
        let local = u16::try_from(local).expect("position < threshold <= u16::MAX");
        let signer_ids: Vec<String> = parties
            .iter()
            .map(|&p| share.participants[usize::from(p)].clone())
            .collect();
        let eid = execution_id(session_id, Protocol::Signing, &signer_ids);
        let child_key = hd::derive_public_key(&share.group_public_key(), path)?;
        let cggmp_path = hd::encode_path(path)?;
        let key_share = share.key_share.clone();
        let parties_at_keygen = parties.clone();

        let sm = cggmp24::round_based::state_machine::wrap_protocol(move |party| async move {
            let mut rng = rng;
            let data = PrehashedDataToSign::from_scalar(
                Scalar::<Secp256k1>::from_be_bytes_mod_order(prehash),
            );
            let signature = cggmp24::signing(
                ExecutionId::new(&eid),
                local,
                &parties_at_keygen,
                &key_share,
            )
            .set_derivation_path_with_algo::<hd::StarlabHd, _>(cggmp_path)
            .map_err(|e| FrostError::EcdsaError(format!("derivation path: {e}")))?
            .sign(&mut rng, party, &data)
            .await
            .map_err(|e| FrostError::EcdsaError(format!("signing: {e}")))?;
            EcdsaSignature::from_cggmp(signature.normalize_s(), &child_key, &prehash)
        });
        Ok(Self::new(Protocol::Signing, eid, me, parties, sm))
    }
}

/// A recoverable secp256k1 ECDSA signature (low-s), as Ethereum wants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcdsaSignature {
    pub r: [u8; 32],
    pub s: [u8; 32],
    /// Recovery id (0 or 1): `v = 27 + recovery_id` (legacy / EIP-191) or
    /// `y_parity` in typed transactions.
    pub recovery_id: u8,
}

impl EcdsaSignature {
    /// Normalizes nothing itself: `signature` must already be low-s. Checks
    /// the signature against `public_key` (SEC1) and finds the recovery id.
    fn from_cggmp(
        signature: cggmp24::Signature<Secp256k1>,
        public_key: &[u8],
        prehash: &[u8; 32],
    ) -> Result<Self> {
        let mut rs = [0u8; 64];
        signature.write_to_slice(&mut rs);
        let r: [u8; 32] = rs[..32].try_into().expect("32 bytes");
        let s: [u8; 32] = rs[32..].try_into().expect("32 bytes");
        let sig = k256::ecdsa::Signature::from_slice(&rs)
            .map_err(|e| FrostError::EcdsaError(format!("signature encoding: {e}")))?;
        let vk = k256::ecdsa::VerifyingKey::from_sec1_bytes(public_key)
            .map_err(|e| FrostError::EcdsaError(format!("child public key: {e}")))?;
        let recovery_id = k256::ecdsa::RecoveryId::trial_recovery_from_prehash(&vk, prehash, &sig)
            .map_err(|_| {
                FrostError::EcdsaError("signature does not verify under the child key".into())
            })?;
        Ok(Self {
            r,
            s,
            recovery_id: recovery_id.to_byte(),
        })
    }

    /// `r ‖ s ‖ recovery_id` (65 bytes).
    pub fn to_bytes(&self) -> [u8; 65] {
        let mut out = [0u8; 65];
        out[..32].copy_from_slice(&self.r);
        out[32..64].copy_from_slice(&self.s);
        out[64] = self.recovery_id;
        out
    }
}

/// A party's persistable threshold-ECDSA key share: the completed cggmp24
/// share plus the ordered participant list (position = keygen index).
/// Threshold / index / n are read from the share itself. The plaintext is
/// secret (secret share + Paillier primes); the keystore encrypts its serde
/// JSON exactly like a FROST key package.
#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "EcdsaKeyShareRepr")]
pub struct EcdsaKeyShare {
    participants: Vec<String>,
    key_share: KeyShare,
}

#[derive(Deserialize)]
struct EcdsaKeyShareRepr {
    participants: Vec<String>,
    key_share: KeyShare,
}

impl TryFrom<EcdsaKeyShareRepr> for EcdsaKeyShare {
    type Error = FrostError;

    fn try_from(repr: EcdsaKeyShareRepr) -> Result<Self> {
        let share = Self {
            participants: repr.participants,
            key_share: repr.key_share,
        };
        share.validate()?;
        Ok(share)
    }
}

impl std::fmt::Debug for EcdsaKeyShare {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EcdsaKeyShare")
            .field("participants", &self.participants)
            .field("index", &self.index())
            .field("threshold", &self.threshold())
            .field("group_public_key", &hex::encode(self.group_public_key()))
            .finish_non_exhaustive()
    }
}

impl EcdsaKeyShare {
    /// Join keygen output and aux info into a signing-ready share. Sets the
    /// root chain code to the one derived from the group key (the same rule
    /// as FROST), which is what makes [`hd::StarlabHd`] children match the
    /// public-only account derivation.
    pub fn from_parts(
        participants: Vec<String>,
        incomplete: IncompleteKeyShare,
        aux: AuxInfo,
    ) -> Result<Self> {
        let mut core = incomplete.into_inner();
        core.key_info.chain_code =
            Some(hd::root_extended_key(*core.key_info.shared_public_key).chain_code);
        let core = Valid::validate(core)
            .map_err(|e| FrostError::EcdsaError(format!("invalid keygen output: {e}")))?;
        let key_share = KeyShare::from_parts((core, aux))
            .map_err(|e| FrostError::EcdsaError(format!("keygen/aux mismatch: {e}")))?;
        let share = Self {
            participants,
            key_share,
        };
        share.validate()?;
        Ok(share)
    }

    fn validate(&self) -> Result<()> {
        if self.participants.len() != usize::from(self.n()) {
            return Err(FrostError::EcdsaError(format!(
                "{} participant ids for a {}-party key",
                self.participants.len(),
                self.n()
            )));
        }
        validate_group(&self.participants, self.index())?;
        let expected = ChainCode::from_group_key(&self.group_public_key());
        if self.key_share.core.key_info.chain_code != Some(*expected.as_bytes()) {
            return Err(FrostError::EcdsaError(
                "chain code is not the one derived from the group key".into(),
            ));
        }
        Ok(())
    }

    /// Device ids; position = keygen index.
    pub fn participants(&self) -> &[String] {
        &self.participants
    }

    /// Our keygen index.
    pub fn index(&self) -> u16 {
        self.key_share.core.i
    }

    pub fn n(&self) -> u16 {
        self.key_share.n()
    }

    pub fn threshold(&self) -> u16 {
        self.key_share.min_signers()
    }

    /// Root group public key, SEC1 compressed.
    pub fn group_public_key(&self) -> [u8; 33] {
        self.key_share
            .shared_public_key()
            .to_bytes(true)
            .as_bytes()
            .try_into()
            .expect("non-zero point encodes to 33 bytes")
    }

    /// The Ethereum address of `account` — the same one
    /// [`crate::accounts::account_addresses`] lists for this key.
    pub fn ethereum_address(&self, account: u32) -> Result<String> {
        let key = crate::accounts::account_verifying_key(
            "ethereum",
            ECDSA_CURVE,
            &self.group_public_key(),
            account,
        )?;
        crate::accounts::address_for_chain("ethereum", ECDSA_CURVE, &key)
    }
}

#[cfg(test)]
mod tests;
