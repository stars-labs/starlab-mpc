//! In-process ECDSA ceremonies over a simulated network that shuffles
//! delivery order and duplicates messages.
//!
//! Test-time strategy: safe-prime generation (~1.5 min per party natively in
//! release, ~5 min in wasm) is NOT run here — `testdata/primes.json` holds
//! three fixed prime sets produced once by `examples/ecdsa_primes.rs`
//! (test-only secrets). The aux-info ceremony still runs for real through
//! the driver, once per test binary (`shared_aux`, ~55 s for 3 parties on
//! one thread — the long pole), and every key reuses it, which is exactly
//! the "aux info is reusable for a fixed signer group" property. Keygen is
//! milliseconds, a signing ~3 s per signer. The root `Cargo.toml` builds the
//! bigint / Paillier crates optimized in dev/test profiles too; without that
//! this module takes well over 10 minutes.

use super::*;
use crate::accounts;
use crate::rng::ChaCha20Rng;
use rand_core::{Rng as _, SeedableRng as _};
use std::sync::OnceLock;

const PRIMES_JSON: &str = include_str!("testdata/primes.json");

fn ids(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("device-{i}")).collect()
}

fn fixture_primes() -> Vec<Primes> {
    serde_json::from_str(PRIMES_JSON).expect("fixture primes parse")
}

fn rng(seed: u64) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(seed)
}

/// Emission hook: `(from, to, payload)` → payload to put on the wire.
type Tamper<'a> = &'a mut dyn FnMut(u16, u16, Vec<u8>) -> Vec<u8>;

/// Run ceremonies to completion over a shuffling, duplicating network.
/// Returns each party's result (in input order); a party that errors stops,
/// and parties left waiting when nothing is in flight report "stalled".
fn run_network<O>(
    mut parties: Vec<EcdsaCeremony<O>>,
    seed: u64,
    tamper: Option<Tamper<'_>>,
) -> Vec<std::result::Result<O, String>> {
    let mut net = rand_chacha::ChaCha20Rng::seed_from_u64(seed);
    let mut tamper = tamper;
    let mut results: Vec<Option<std::result::Result<O, String>>> =
        parties.iter().map(|_| None).collect();
    let mut in_flight: Vec<(u16, u16, Vec<u8>)> = Vec::new();

    let route = |c: &mut EcdsaCeremony<O>,
                 in_flight: &mut Vec<(u16, u16, Vec<u8>)>,
                 tamper: &mut Option<Tamper<'_>>| {
        for m in c.take_outgoing() {
            let targets: Vec<u16> = match m.recipient {
                Recipient::Broadcast => c
                    .parties()
                    .iter()
                    .copied()
                    .filter(|&p| p != c.index())
                    .collect(),
                Recipient::P2P(to) => vec![to],
            };
            for to in targets {
                let payload = match tamper {
                    Some(t) => t(c.index(), to, m.payload.clone()),
                    None => m.payload.clone(),
                };
                in_flight.push((c.index(), to, payload));
            }
        }
    };

    for (k, c) in parties.iter_mut().enumerate() {
        match c.proceed() {
            Ok(Some(o)) => results[k] = Some(Ok(o)),
            Ok(None) => {}
            Err(e) => results[k] = Some(Err(e.to_string())),
        }
        route(c, &mut in_flight, &mut tamper);
    }

    while results.iter().any(Option::is_none) && !in_flight.is_empty() {
        let pick = (net.next_u64() % in_flight.len() as u64) as usize;
        let (from, to, payload) = in_flight.swap_remove(pick);
        // Duplicate ~1 in 8 deliveries: the driver must drop exact repeats.
        if net.next_u32() % 8 == 0 {
            in_flight.push((from, to, payload.clone()));
        }
        let k = parties
            .iter()
            .position(|c| c.index() == to)
            .expect("message to a known party");
        if results[k].is_some() {
            continue;
        }
        let c = &mut parties[k];
        let step = c.receive(from, &payload).and_then(|()| c.proceed());
        match step {
            Ok(Some(o)) => results[k] = Some(Ok(o)),
            Ok(None) => {}
            Err(e) => results[k] = Some(Err(e.to_string())),
        }
        route(c, &mut in_flight, &mut tamper);
    }

    results
        .into_iter()
        .map(|r| r.unwrap_or_else(|| Err("stalled".into())))
        .collect()
}

fn unwrap_all<O>(results: Vec<std::result::Result<O, String>>) -> Vec<O> {
    results
        .into_iter()
        .enumerate()
        .map(|(k, r)| r.unwrap_or_else(|e| panic!("party {k} failed: {e}")))
        .collect()
}

/// Aux info for a 3-party group, generated once per test binary through the
/// driver (with the fixture primes) and reused by every key below.
fn shared_aux() -> &'static [AuxInfo] {
    static AUX: OnceLock<Vec<AuxInfo>> = OnceLock::new();
    AUX.get_or_init(|| {
        let participants = ids(3);
        let ceremonies = fixture_primes()
            .into_iter()
            .enumerate()
            .map(|(i, primes)| {
                EcdsaCeremony::aux_info(
                    "aux-session",
                    &participants,
                    i as u16,
                    primes,
                    rng(i as u64),
                )
                .unwrap()
            })
            .collect();
        unwrap_all(run_network(ceremonies, 1, None))
    })
}

fn keygen(session: &str, threshold: u16, seed: u64) -> Vec<EcdsaKeyShare> {
    let participants = ids(3);
    let ceremonies = (0..3u16)
        .map(|i| {
            EcdsaCeremony::keygen(
                session,
                &participants,
                i,
                threshold,
                rng(seed + u64::from(i)),
            )
            .unwrap()
        })
        .collect();
    let incomplete = unwrap_all(run_network(ceremonies, seed, None));
    incomplete
        .into_iter()
        .zip(shared_aux().iter().cloned())
        .map(|(core, aux)| EcdsaKeyShare::from_parts(participants.clone(), core, aux).unwrap())
        .collect()
}

fn sign(
    shares: &[EcdsaKeyShare],
    signers: &[u16],
    account: u32,
    prehash: [u8; 32],
    seed: u64,
) -> EcdsaSignature {
    let path =
        DerivationPath::parse(&accounts::standard_path("ethereum", account).unwrap()).unwrap();
    let session = format!("sign-{seed}");
    let ceremonies = signers
        .iter()
        .map(|&i| {
            EcdsaCeremony::signing(
                &session,
                &shares[usize::from(i)],
                signers,
                &path,
                prehash,
                rng(seed + u64::from(i)),
            )
            .unwrap()
        })
        .collect();
    let sigs = unwrap_all(run_network(ceremonies, seed, None));
    assert!(
        sigs.windows(2).all(|w| w[0] == w[1]),
        "all signers output one signature"
    );
    sigs[0]
}

fn keccak(data: &[u8]) -> [u8; 32] {
    use sha3::{Digest as _, Keccak256};
    Keccak256::digest(data).into()
}

/// ecrecover: the Ethereum address the signature recovers to, for `prehash`.
fn ecrecover(sig: &EcdsaSignature, prehash: &[u8; 32]) -> String {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    let signature = Signature::from_scalars(sig.r, sig.s).unwrap();
    let recid = RecoveryId::from_byte(sig.recovery_id).unwrap();
    let key = VerifyingKey::recover_from_prehash(prehash, &signature, recid).unwrap();
    let sec1 = key.to_sec1_point(true);
    accounts::address_for_chain("ethereum", ECDSA_CURVE, sec1.as_bytes()).unwrap()
}

/// Plain k256 verification under the key accounts.rs derives public-only.
fn verify_with_k256(group_key: &[u8], account: u32, prehash: &[u8; 32], sig: &EcdsaSignature) {
    use k256::ecdsa::signature::hazmat::PrehashVerifier;
    let child =
        accounts::account_verifying_key("ethereum", ECDSA_CURVE, group_key, account).unwrap();
    let vk = k256::ecdsa::VerifyingKey::from_sec1_bytes(&child).unwrap();
    let signature = k256::ecdsa::Signature::from_scalars(sig.r, sig.s).unwrap();
    assert!(signature.normalize_s() == signature, "low-s");
    vk.verify_prehash(prehash, &signature).unwrap();
}

#[test]
fn execution_id_is_order_independent_and_separates_everything() {
    let a = ["b".to_string(), "a".to_string()];
    let b = ["a".to_string(), "b".to_string()];
    let eid = execution_id("s1", Protocol::Keygen, &a);
    assert_eq!(eid, execution_id("s1", Protocol::Keygen, &b));
    assert_ne!(eid, execution_id("s2", Protocol::Keygen, &a));
    assert_ne!(eid, execution_id("s1", Protocol::Signing, &a));
    assert_ne!(eid, execution_id("s1", Protocol::Keygen, &a[..1]));
    // Length prefixes: moving a byte across a field boundary changes the id.
    assert_ne!(
        execution_id("s", Protocol::Keygen, &["1a".to_string()]),
        execution_id("s1", Protocol::Keygen, &["a".to_string()])
    );
}

#[test]
fn hd_child_matches_public_account_derivation() {
    // The code cggmp24 runs while signing (StarlabHd) and the public-only
    // account derivation in accounts.rs land on the same key.
    let group =
        hex::decode("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap();
    for account in 0..4 {
        let path =
            DerivationPath::parse(&accounts::standard_path("ethereum", account).unwrap()).unwrap();
        let via_cggmp_trait = hd::derive_public_key(&group, &path).unwrap();
        let via_accounts =
            accounts::account_verifying_key("ethereum", ECDSA_CURVE, &group, account).unwrap();
        assert_eq!(via_cggmp_trait.to_vec(), via_accounts, "account {account}");
    }
    // ECDSA children are untweaked: they differ from the FROST (BIP-86
    // finalized) secp256k1 child at the same path.
    let frost = accounts::account_verifying_key("ethereum", "secp256k1", &group, 0).unwrap();
    let ecdsa = accounts::account_verifying_key("ethereum", ECDSA_CURVE, &group, 0).unwrap();
    assert_ne!(frost, ecdsa);
}

#[test]
fn hd_path_encoding_keeps_the_hardened_bit() {
    let path = DerivationPath::parse("m/44'/60'/0'/0/7").unwrap();
    let encoded = hd::encode_path(&path).unwrap();
    assert!(encoded.iter().all(|&i| i < 1 << 31));
    assert_eq!(encoded, vec![44 | 1 << 30, 60 | 1 << 30, 1 << 30, 0, 7]);
    let too_big = DerivationPath::new(vec![1 << 30]);
    assert!(hd::encode_path(&too_big).is_err());
}

#[test]
fn primes_serde_round_trip() {
    let primes = fixture_primes();
    assert_eq!(primes.len(), 3);
    let json = serde_json::to_string(&primes[0]).unwrap();
    let back: Primes = serde_json::from_str(&json).unwrap();
    assert_eq!(back.primes_ref(), primes[0].primes_ref());
}

#[test]
fn two_of_three_signs_every_account_with_every_signer_subset() {
    let shares = keygen("keygen-2of3", 2, 100);
    let group = shares[0].group_public_key();
    assert!(shares.iter().all(|s| s.group_public_key() == group));
    assert!(shares.iter().all(|s| s.threshold() == 2 && s.n() == 3));
    assert_eq!(
        shares.iter().map(EcdsaKeyShare::index).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    // Persisted shares (keystore plaintext) sign exactly like fresh ones.
    let shares: Vec<EcdsaKeyShare> = shares
        .iter()
        .map(|s| serde_json::from_str(&serde_json::to_string(s).unwrap()).unwrap())
        .collect();

    let subsets: [&[u16]; 3] = [&[0, 1], &[0, 2], &[1, 2]];
    for account in 0..4u32 {
        let signers = subsets[account as usize % subsets.len()];
        let prehash = keccak(format!("starlab ecdsa account {account}").as_bytes());
        let sig = sign(&shares, signers, account, prehash, 200 + u64::from(account));

        verify_with_k256(&group, account, &prehash, &sig);
        let address = shares[0].ethereum_address(account).unwrap();
        assert_eq!(
            ecrecover(&sig, &prehash),
            address,
            "account {account} via {signers:?}"
        );
        // … and it is the address the account model lists for this key.
        let listed = accounts::account_addresses(ECDSA_CURVE, &group, account).unwrap();
        assert_eq!(
            listed,
            vec![(
                "Ethereum".to_string(),
                accounts::standard_path("ethereum", account).unwrap(),
                address
            )]
        );
    }
}

#[test]
fn three_of_three_signs() {
    let shares = keygen("keygen-3of3", 3, 300);
    let prehash = keccak(b"three of three");
    let sig = sign(&shares, &[0, 1, 2], 1, prehash, 400);
    verify_with_k256(&shares[0].group_public_key(), 1, &prehash, &sig);
    assert_eq!(
        ecrecover(&sig, &prehash),
        shares[2].ethereum_address(1).unwrap()
    );
}

#[test]
fn signing_rejects_bad_signer_sets() {
    let shares = keygen("keygen-bad-signers", 2, 500);
    let path = DerivationPath::parse("m/44'/60'/0'/0/0").unwrap();
    let try_sign = |signers: &[u16]| {
        EcdsaCeremony::signing("s", &shares[0], signers, &path, [1; 32], rng(0)).map(|_| ())
    };
    assert!(try_sign(&[0]).is_err(), "below threshold");
    assert!(try_sign(&[0, 1, 2]).is_err(), "above threshold");
    assert!(try_sign(&[1, 2]).is_err(), "we are not a signer");
    assert!(try_sign(&[0, 0]).is_err(), "duplicate");
    assert!(try_sign(&[0, 7]).is_err(), "unknown index");
    assert!(try_sign(&[1, 0]).is_ok(), "order does not matter");
}

#[test]
fn key_share_serde_round_trip() {
    let shares = keygen("keygen-serde", 2, 600);
    let json = serde_json::to_string(&shares[1]).unwrap();
    let back: EcdsaKeyShare = serde_json::from_str(&json).unwrap();
    assert_eq!(serde_json::to_string(&back).unwrap(), json);
    assert_eq!(back.participants(), shares[1].participants());
    assert_eq!(back.index(), 1);
    assert_eq!(back.threshold(), 2);
    assert_eq!(back.group_public_key(), shares[1].group_public_key());

    // A share whose participant list doesn't match n is refused on load.
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value["participants"] = serde_json::json!(["only-one"]);
    assert!(serde_json::from_value::<EcdsaKeyShare>(value).is_err());
}

#[test]
fn message_from_another_execution_is_rejected_by_the_header() {
    let participants = ids(3);
    let mut a = EcdsaCeremony::keygen("session-a", &participants, 1, 2, rng(1)).unwrap();
    let mut b = EcdsaCeremony::keygen("session-b", &participants, 0, 2, rng(2)).unwrap();
    assert!(a.proceed().unwrap().is_none());
    assert!(b.proceed().unwrap().is_none());
    let replay = a.take_outgoing().remove(0).payload;
    let err = b.receive(1, &replay).unwrap_err().to_string();
    assert!(err.contains("another execution"), "{err}");

    // Also refused: garbage, non-parties, ourselves.
    assert!(b.receive(1, &[1, 2, 3]).is_err());
    let mut own = b.take_outgoing().remove(0).payload;
    assert!(b.receive(0, &own).is_err(), "from ourselves");
    assert!(b.receive(9, &own).is_err(), "not a party");
    own.truncate(40);
    assert!(b.receive(1, &own).is_err(), "undecodable body");
}

#[test]
fn replayed_message_with_rewritten_header_aborts_the_protocol() {
    // Record everything party 1 sends to party 0 in keygen A…
    let participants = ids(3);
    let mut recorded: Vec<Vec<u8>> = Vec::new();
    let mut record = |from: u16, to: u16, payload: Vec<u8>| {
        if (from, to) == (1, 0) {
            recorded.push(payload.clone());
        }
        payload
    };
    let run_a = (0..3u16)
        .map(|i| {
            EcdsaCeremony::keygen("replay-a", &participants, i, 2, rng(10 + u64::from(i))).unwrap()
        })
        .collect();
    unwrap_all(run_network(run_a, 7, Some(&mut record)));

    // …and replay it into keygen B, header rewritten to B's execution id so
    // it passes the envelope check. The eid bound inside cggmp24's messages
    // (commitments / proofs) must still make party 0 abort.
    let eid_b = execution_id("replay-b", Protocol::Keygen, &participants);
    let mut sent = 0usize;
    let mut replay = |from: u16, to: u16, payload: Vec<u8>| {
        if (from, to) != (1, 0) {
            return payload;
        }
        let mut forged = recorded[sent].clone();
        sent += 1;
        forged[3..35].copy_from_slice(&eid_b);
        forged
    };
    let run_b = (0..3u16)
        .map(|i| {
            EcdsaCeremony::keygen("replay-b", &participants, i, 2, rng(20 + u64::from(i))).unwrap()
        })
        .collect();
    let results = run_network(run_b, 8, Some(&mut replay));
    let err = results[0]
        .as_ref()
        .err()
        .expect("party 0 must reject the replayed messages");
    assert!(err.contains("protocol aborted"), "{err}");
}

#[test]
fn primes_join_from_their_parts() {
    let set = fixture_primes().remove(0);
    let parts: Vec<SafePrime> = set.primes_ref().to_vec();
    let joined = primes_from_parts(parts.clone()).expect("four fixture primes");
    assert_eq!(joined.primes_ref(), set.primes_ref());
    // The serde form of a part is what a wasm worker hands back.
    let json = serde_json::to_string(&parts[0]).unwrap();
    assert!(json.starts_with(r#"{"radix":16,"value":""#), "{json}");

    assert!(primes_from_parts(parts[..3].to_vec()).is_err(), "3 parts");
    let mut small = parts;
    small[3] = SafePrime::from(23u32);
    assert!(primes_from_parts(small).is_err(), "undersized prime");
}
