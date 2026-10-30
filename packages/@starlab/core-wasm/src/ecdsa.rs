//! Threshold ECDSA (cggmp24, via `starlab_core::ecdsa`) for the browser:
//! the Ethereum key of a wallet.
//!
//! Same byte-level driver the native engine runs (`EcdsaCeremony`), one
//! class per protocol:
//!
//! ```text
//! const c = new EcdsaAuxInfo(sessionId, participants, index, primesJson);
//! for (;;) {
//!   for (const m of c.take_outgoing()) send(m.recipient, m.payload); // "broadcast" | keygen index
//!   const out = c.proceed();            // undefined until finished
//!   if (out !== undefined) break;
//!   const [from, bytes] = await recv(); // from = sender's keygen index
//!   c.receive(from, bytes);
//! }
//! ```
//!
//! Parties are always addressed by keygen index = position in the sorted
//! participant list (the engine's rule). Payloads are the driver's wire
//! format v1 (35-byte header + CBOR) — deliver verbatim; the data-channel
//! framing (`ECDSA:` chunks) is the caller's. `proceed()` is CPU-heavy
//! (Paillier / ZK proofs): drive ceremonies in a Web Worker, never on a UI
//! or networking thread. Ceremonies are single-use and not transferable
//! between workers.
//!
//! Primes: `ecdsa_generate_safe_prime` (minutes each in wasm — run the four
//! in parallel workers) → `ecdsa_primes_from_parts` → the `primesJson` an
//! aux-info ceremony takes.

use crate::WasmError;
use starlab_core::accounts;
use starlab_core::ecdsa::{
    self, AuxInfo, EcdsaCeremony, EcdsaKeyShare, IncompleteKeyShare, Primes, Protocol, Recipient,
    SafePrime,
};
use starlab_core::hd_derivation::DerivationPath;
use starlab_core::rng::os_rng;
use wasm_bindgen::prelude::*;

fn err(e: impl std::fmt::Display) -> WasmError {
    WasmError::new(&e.to_string())
}

fn protocol(tag: &str) -> Result<Protocol, WasmError> {
    match tag {
        "aux-info" => Ok(Protocol::AuxInfo),
        "keygen" => Ok(Protocol::Keygen),
        "signing" => Ok(Protocol::Signing),
        other => Err(WasmError::new(&format!(
            "unknown ECDSA protocol {other:?} (aux-info | keygen | signing)"
        ))),
    }
}

/// Curve tag of the ECDSA key (keystore / accounts / signing announce).
#[wasm_bindgen]
pub fn ecdsa_curve() -> String {
    ecdsa::ECDSA_CURVE.to_string()
}

/// Generate ONE Paillier safe prime (JSON `{"radix":16,"value":…}`). Slow
/// (minutes in wasm): a set needs four — run them in parallel workers and
/// join with [`ecdsa_primes_from_parts`].
#[wasm_bindgen]
pub fn ecdsa_generate_safe_prime() -> String {
    let prime = ecdsa::generate_safe_prime(&mut os_rng());
    serde_json::to_string(&prime).expect("an integer serializes")
}

/// Join four primes from [`ecdsa_generate_safe_prime`] (JSON array of
/// them) into the primes JSON an aux-info ceremony takes. Secret.
#[wasm_bindgen]
pub fn ecdsa_primes_from_parts(parts_json: &str) -> Result<String, WasmError> {
    let parts: Vec<SafePrime> = serde_json::from_str(parts_json).map_err(err)?;
    let primes = ecdsa::primes_from_parts(parts).map_err(WasmError::from)?;
    serde_json::to_string(&primes).map_err(err)
}

/// Number of safe primes per set (4).
#[wasm_bindgen]
pub fn ecdsa_prime_count() -> u32 {
    ecdsa::PRIME_COUNT as u32
}

/// Execution id (hex) of a ceremony — `protocol` is "aux-info" | "keygen" |
/// "signing"; `participants` are device ids in any order. Lets a caller
/// route a payload that arrives before its ceremony exists.
#[wasm_bindgen]
pub fn ecdsa_execution_id(
    session_id: &str,
    protocol_tag: &str,
    participants: Vec<String>,
) -> Result<String, WasmError> {
    Ok(hex::encode(ecdsa::execution_id(
        session_id,
        protocol(protocol_tag)?,
        &participants,
    )))
}

/// Execution id (hex) in a driver payload's header, or undefined.
#[wasm_bindgen]
pub fn ecdsa_payload_execution_id(payload: &[u8]) -> Option<String> {
    payload.get(3..35).map(hex::encode)
}

/// Public facts of a key share JSON: `{"participants":[…],"index":i,
/// "threshold":t,"n":n,"group_public_key":"<33-byte SEC1 hex>"}`.
#[wasm_bindgen]
pub fn ecdsa_share_info(share_json: &str) -> Result<String, WasmError> {
    let share: EcdsaKeyShare = serde_json::from_str(share_json).map_err(err)?;
    Ok(share_info(&share).to_string())
}

fn share_info(share: &EcdsaKeyShare) -> serde_json::Value {
    serde_json::json!({
        "participants": share.participants(),
        "index": share.index(),
        "threshold": share.threshold(),
        "n": share.n(),
        "group_public_key": hex::encode(share.group_public_key()),
    })
}

/// Ethereum address of `account` for an ECDSA group key (hex SEC1) —
/// public-only, the same as `derive_account_addresses("secp256k1-ecdsa", …)`.
#[wasm_bindgen]
pub fn ecdsa_ethereum_address(
    group_public_key_hex: &str,
    account: u32,
) -> Result<String, WasmError> {
    let group = hex::decode(group_public_key_hex).map_err(err)?;
    let key = accounts::account_verifying_key("ethereum", ecdsa::ECDSA_CURVE, &group, account)
        .map_err(WasmError::from)?;
    accounts::address_for_chain("ethereum", ecdsa::ECDSA_CURVE, &key).map_err(WasmError::from)
}

/// `ecrecover`: the address a 65-byte `r ‖ s ‖ v` signature (v ∈ {0,1,27,28})
/// over the 32-byte `prehash` recovers to (lowercase 0x hex).
#[wasm_bindgen]
pub fn ecdsa_recover_ethereum_address(
    prehash: &[u8],
    signature: &[u8],
) -> Result<String, WasmError> {
    accounts::recover_ethereum_address(prehash, signature).map_err(WasmError::from)
}

/// Join a keygen output and an aux-info output (both from `proceed()`) into
/// the persistable key share JSON (secret: encrypt before storing).
/// `participants` = the sorted device ids the ceremonies ran with.
#[wasm_bindgen]
pub fn ecdsa_key_share_from_parts(
    participants: Vec<String>,
    keygen_json: &str,
    aux_info_json: &str,
) -> Result<String, WasmError> {
    let incomplete: IncompleteKeyShare = serde_json::from_str(keygen_json).map_err(err)?;
    let aux: AuxInfo = serde_json::from_str(aux_info_json).map_err(err)?;
    let share =
        EcdsaKeyShare::from_parts(participants, incomplete, aux).map_err(WasmError::from)?;
    serde_json::to_string(&share).map_err(err)
}

/// Outgoing messages as a JS array of `{recipient, payload}` —
/// `recipient` is "broadcast" (every other party of the ceremony) or a
/// keygen index; `payload` a Uint8Array to deliver verbatim.
fn outgoing_to_js<O>(ceremony: &mut EcdsaCeremony<O>) -> js_sys::Array {
    let out = js_sys::Array::new();
    for m in ceremony.take_outgoing() {
        let obj = js_sys::Object::new();
        let recipient = match m.recipient {
            Recipient::Broadcast => JsValue::from_str("broadcast"),
            Recipient::P2P(i) => JsValue::from(i),
        };
        let _ = js_sys::Reflect::set(&obj, &"recipient".into(), &recipient);
        let payload = js_sys::Uint8Array::from(m.payload.as_slice());
        let _ = js_sys::Reflect::set(&obj, &"payload".into(), &payload);
        out.push(&obj);
    }
    out
}

/// The methods every ceremony class shares.
macro_rules! ceremony_common {
    ($name:ident) => {
        #[wasm_bindgen]
        impl $name {
            /// Queue a payload from party `sender` (keygen index). Throws on a
            /// payload of another execution, from a non-party, or
            /// undecodable — the ceremony's state is unchanged then, so a
            /// caller may log and drop it. Exact duplicates are ignored.
            pub fn receive(&mut self, sender: u16, payload: &[u8]) -> Result<(), WasmError> {
                self.inner.receive(sender, payload).map_err(WasmError::from)
            }

            /// Drain the messages to send (production order).
            pub fn take_outgoing(&mut self) -> js_sys::Array {
                outgoing_to_js(&mut self.inner)
            }

            /// Hex execution id (also bytes 3..35 of every payload).
            pub fn execution_id(&self) -> String {
                hex::encode(self.inner.execution_id())
            }

            /// Our keygen index.
            pub fn index(&self) -> u16 {
                self.inner.index()
            }

            /// Keygen indices of all parties (us included).
            pub fn parties(&self) -> Vec<u16> {
                self.inner.parties().to_vec()
            }

            pub fn is_finished(&self) -> bool {
                self.inner.is_finished()
            }
        }
    };
}

/// Aux-info ceremony (Paillier moduli + ZK proofs) — the slow one.
#[wasm_bindgen]
pub struct EcdsaAuxInfo {
    inner: EcdsaCeremony<AuxInfo>,
}

#[wasm_bindgen]
impl EcdsaAuxInfo {
    /// `participants`: sorted device ids (position = keygen index);
    /// `index`: ours; `primes_json`: from [`ecdsa_primes_from_parts`].
    #[wasm_bindgen(constructor)]
    pub fn new(
        session_id: &str,
        participants: Vec<String>,
        index: u16,
        primes_json: &str,
    ) -> Result<EcdsaAuxInfo, WasmError> {
        let primes: Primes = serde_json::from_str(primes_json).map_err(err)?;
        let inner = EcdsaCeremony::aux_info(session_id, &participants, index, primes, os_rng())
            .map_err(WasmError::from)?;
        Ok(Self { inner })
    }

    /// Advance; returns the aux-info JSON (secret) once finished, else
    /// undefined. Throws if the protocol aborts.
    pub fn proceed(&mut self) -> Result<Option<String>, WasmError> {
        match self.inner.proceed().map_err(WasmError::from)? {
            Some(aux) => Ok(Some(serde_json::to_string(&aux).map_err(err)?)),
            None => Ok(None),
        }
    }
}
ceremony_common!(EcdsaAuxInfo);

/// Threshold keygen ceremony.
#[wasm_bindgen]
pub struct EcdsaKeygen {
    inner: EcdsaCeremony<IncompleteKeyShare>,
}

#[wasm_bindgen]
impl EcdsaKeygen {
    #[wasm_bindgen(constructor)]
    pub fn new(
        session_id: &str,
        participants: Vec<String>,
        index: u16,
        threshold: u16,
    ) -> Result<EcdsaKeygen, WasmError> {
        let inner = EcdsaCeremony::keygen(session_id, &participants, index, threshold, os_rng())
            .map_err(WasmError::from)?;
        Ok(Self { inner })
    }

    /// Advance; returns the keygen output JSON (secret; join it with the
    /// aux info via [`ecdsa_key_share_from_parts`]) once finished.
    pub fn proceed(&mut self) -> Result<Option<String>, WasmError> {
        match self.inner.proceed().map_err(WasmError::from)? {
            Some(out) => Ok(Some(serde_json::to_string(&out).map_err(err)?)),
            None => Ok(None),
        }
    }
}
ceremony_common!(EcdsaKeygen);

/// One signing (full interactive protocol — no presignatures).
#[wasm_bindgen]
pub struct EcdsaSigning {
    inner: EcdsaCeremony<ecdsa::EcdsaSignature>,
}

#[wasm_bindgen]
impl EcdsaSigning {
    /// Sign the 32-byte `prehash` with the HD child at `path` (e.g.
    /// "m/44'/60'/0'/0/0") together with `signers` (keygen indices, exactly
    /// the share's threshold, us included). `session_id` must be fresh per
    /// signing (`sign_<uuid>`).
    #[wasm_bindgen(constructor)]
    pub fn new(
        session_id: &str,
        share_json: &str,
        signers: Vec<u16>,
        path: &str,
        prehash: &[u8],
    ) -> Result<EcdsaSigning, WasmError> {
        let share: EcdsaKeyShare = serde_json::from_str(share_json).map_err(err)?;
        let path = DerivationPath::parse(path).map_err(WasmError::from)?;
        let prehash: [u8; 32] = prehash
            .try_into()
            .map_err(|_| WasmError::new(&format!("prehash is {} bytes, need 32", prehash.len())))?;
        let inner = EcdsaCeremony::signing(session_id, &share, &signers, &path, prehash, os_rng())
            .map_err(WasmError::from)?;
        Ok(Self { inner })
    }

    /// Advance; once finished returns hex of the 65-byte `r ‖ s ‖ v`
    /// (low-s, `v = 27 + recovery_id` — personal_sign's convention; a typed
    /// transaction takes `y_parity = v - 27`), else undefined.
    pub fn proceed(&mut self) -> Result<Option<String>, WasmError> {
        Ok(self
            .inner
            .proceed()
            .map_err(WasmError::from)?
            .map(|sig| hex::encode(ethereum_signature_bytes(&sig))))
    }
}
ceremony_common!(EcdsaSigning);

/// `r ‖ s ‖ (27 + recovery_id)` — byte-identical to the engine's
/// `protocal::ecdsa::signing::ethereum_signature_bytes`.
fn ethereum_signature_bytes(sig: &ecdsa::EcdsaSignature) -> [u8; 65] {
    let mut bytes = sig.to_bytes();
    bytes[64] += 27;
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMES_JSON: &str = include_str!("../../core/src/ecdsa/testdata/primes.json");

    #[test]
    fn primes_parts_round_trip_through_json() {
        let sets: Vec<serde_json::Value> = serde_json::from_str(PRIMES_JSON).unwrap();
        let parts = sets[0]["primes"].to_string();
        let joined = ecdsa_primes_from_parts(&parts).unwrap();
        let back: serde_json::Value = serde_json::from_str(&joined).unwrap();
        assert_eq!(back["primes"], sets[0]["primes"]);
        assert!(ecdsa_primes_from_parts("[]").is_err());
    }

    #[test]
    fn execution_id_is_order_independent_and_protocol_bound() {
        let a = vec!["b".to_string(), "a".to_string()];
        let b = vec!["a".to_string(), "b".to_string()];
        let k1 = ecdsa_execution_id("s", "keygen", a).unwrap();
        let k2 = ecdsa_execution_id("s", "keygen", b.clone()).unwrap();
        assert_eq!(k1, k2);
        assert_ne!(k1, ecdsa_execution_id("s", "aux-info", b.clone()).unwrap());
        assert!(ecdsa_execution_id("s", "nope", b).is_err());
    }

    #[test]
    fn ethereum_address_is_the_accounts_one() {
        // secp256k1 generator G.
        let g = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let listed = starlab_core::accounts::account_addresses(
            ecdsa::ECDSA_CURVE,
            &hex::decode(g).unwrap(),
            3,
        )
        .unwrap();
        assert_eq!(ecdsa_ethereum_address(g, 3).unwrap(), listed[0].2);
    }

    #[test]
    fn signature_v_is_27_or_28() {
        let sig = ecdsa::EcdsaSignature {
            r: [1; 32],
            s: [2; 32],
            recovery_id: 1,
        };
        assert_eq!(ethereum_signature_bytes(&sig)[64], 28);
    }
}
