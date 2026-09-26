//! The account model — single source of truth for ALL clients (CLI, WASM /
//! browser extension, desktop, TUI).
//!
//! `Wallet → Account(index) → per-chain address`, exactly the BIP-44 mental
//! model: the per-chain derivation paths are PINNED here, users only ever
//! think in account indexes. Address derivation is PUBLIC-only (see
//! [`crate::hd_derivation::derive_child_verifying_key_path`]) — listing
//! accounts never touches a key share or a password.
//!
//! Every client deriving account `i` of the same wallet MUST land on the
//! same address; keeping the path table AND the per-chain address encoding
//! in one place is what guarantees it byte-for-byte.

use crate::errors::{FrostError, Result};
use crate::hd_derivation::{DerivationPath, derive_child_verifying_key_path};

/// (config key, display name) of the chains a curve's key controls.
/// EVM L2s share the Ethereum address and are deliberately not listed.
pub fn chains_for_curve(curve: &str) -> &'static [(&'static str, &'static str)] {
    if curve == "ed25519" {
        &[("solana", "Solana"), ("sui", "Sui")]
    } else {
        &[("ethereum", "Ethereum"), ("bitcoin", "Bitcoin")]
    }
}

/// The curve whose key share signs for a chain. Inverse of
/// [`chains_for_curve`]; accepts the same aliases as [`standard_path`].
pub fn curve_for_chain(chain: &str) -> Option<&'static str> {
    match chain.to_ascii_lowercase().as_str() {
        "ethereum" | "eth" | "bitcoin" | "btc" => Some("secp256k1"),
        "solana" | "sol" | "sui" => Some("ed25519"),
        _ => None,
    }
}

/// Parse an HD account child wallet id of the canonical form
/// `{parent}-{chain}-{account}` (e.g. `19caa3cf46d3-ethereum-7`) into
/// `(parent, chain, account)`.
///
/// This is THE convention every client uses to name materialized account
/// wallets, so a co-signer that receives a signing announce for a child it
/// has never derived can recognize it and materialize it deterministically.
/// Returns `None` for anything that isn't a well-formed child id — root
/// ids, arbitrary hyphenated names, unknown chains — so callers can leave
/// those on their existing path untouched.
pub fn parse_child_wallet_id(id: &str) -> Option<(&str, &str, u32)> {
    let (rest, account_s) = id.rsplit_once('-')?;
    // Strict digits only: `u32::parse` would also accept "+7", which no
    // client ever emits — reject it so odd root ids can't be misread.
    if account_s.is_empty() || !account_s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let account = account_s.parse::<u32>().ok()?;
    let (parent, chain) = rest.rsplit_once('-')?;
    if parent.is_empty() || curve_for_chain(chain).is_none() {
        return None;
    }
    Some((parent, chain, account))
}

/// The PINNED standard derivation path per chain (BIP-44/84 coin types).
pub fn standard_path(chain: &str, account: u32) -> Option<String> {
    match chain.to_ascii_lowercase().as_str() {
        "ethereum" | "eth" => Some(format!("m/44'/60'/0'/0/{account}")),
        "bitcoin" | "btc" => Some(format!("m/84'/0'/0'/0/{account}")),
        "solana" | "sol" => Some(format!("m/44'/501'/{account}'/0'")),
        "sui" => Some(format!("m/44'/784'/{account}'/0'/0'")),
        _ => None,
    }
}

/// Encode a (child) verifying key as one chain's address. Exactly the four
/// canonical chains; unknown combinations error.
pub fn address_for_chain(chain: &str, curve: &str, pubkey_bytes: &[u8]) -> Result<String> {
    match (chain.to_ascii_lowercase().as_str(), curve) {
        ("ethereum" | "eth", "secp256k1") => {
            // keccak256(uncompressed X‖Y)[12..]. FROST serializes compressed,
            // so decompress first (hashing compressed bytes gives a WRONG
            // address that doesn't correspond to the signing key).
            use k256::elliptic_curve::sec1::ToSec1Point;
            use sha3::{Digest, Keccak256};
            let pk = k256::PublicKey::from_sec1_bytes(pubkey_bytes)
                .map_err(|e| FrostError::SerializationError(format!("secp pubkey: {e}")))?;
            let point = pk.to_sec1_point(false);
            let hash = Keccak256::digest(&point.as_bytes()[1..]);
            Ok(format!("0x{}", hex::encode(&hash[12..32])))
        }
        ("bitcoin" | "btc", "secp256k1") => {
            // P2WPKH (BIP-84): bech32 segwit-v0 of hash160(compressed pubkey).
            use k256::elliptic_curve::sec1::ToSec1Point;
            use ripemd::Ripemd160;
            let pk = k256::PublicKey::from_sec1_bytes(pubkey_bytes)
                .map_err(|e| FrostError::SerializationError(format!("secp pubkey: {e}")))?;
            let compressed = pk.to_sec1_point(true);
            // sha2 0.11 and ripemd 0.1 track different `digest` majors — feed
            // bytes across, never trait objects.
            let sha = <sha2::Sha256 as sha2::Digest>::digest(compressed.as_bytes());
            let h160 = <Ripemd160 as ripemd::Digest>::digest(sha.as_slice());
            bech32::segwit::encode_v0(bech32::hrp::BC, h160.as_slice())
                .map_err(|e| FrostError::SerializationError(format!("bech32: {e}")))
        }
        ("solana" | "sol", "ed25519") => Ok(bs58::encode(pubkey_bytes).into_string()),
        ("sui", "ed25519") => {
            // sha3-256(flag(0x00 = ed25519) ‖ pubkey), 32 bytes, 0x-hex.
            use sha3::{Digest, Sha3_256};
            let mut h = Sha3_256::new();
            h.update([0x00]);
            h.update(pubkey_bytes);
            Ok(format!("0x{}", hex::encode(&h.finalize()[..32])))
        }
        (chain, curve) => Err(FrostError::SerializationError(format!(
            "no address encoding for chain {chain:?} on curve {curve:?}"
        ))),
    }
}

/// All of account `i`'s addresses for one curve's group key:
/// `(chain display name, path, address)` per chain. PUBLIC derivation only.
pub fn account_addresses(
    curve: &str,
    group_key_bytes: &[u8],
    account: u32,
) -> Result<Vec<(String, String, String)>> {
    let mut out = Vec::new();
    for (key, display) in chains_for_curve(curve) {
        let path_s = standard_path(key, account)
            .ok_or_else(|| FrostError::DerivationError(format!("no path for {key}")))?;
        let path = DerivationPath::parse(&path_s)?;
        let child = match curve {
            "ed25519" => derive_child_verifying_key_path::<frost_ed25519::Ed25519Sha512>(
                group_key_bytes,
                &path,
            )?,
            "secp256k1" => derive_child_verifying_key_path::<frost_secp256k1::Secp256K1Sha256>(
                group_key_bytes,
                &path,
            )?,
            other => {
                return Err(FrostError::DerivationError(format!(
                    "unsupported curve {other}"
                )));
            }
        };
        out.push((
            (*display).to_string(),
            path_s,
            address_for_chain(key, curve, &child)?,
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// secp256k1 generator G — its Ethereum address is a canonical test
    /// vector (privkey 1): 0x7e5f4552091a69125d5dfcb7b8c2659029395bdf.
    const G_HEX: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    #[test]
    fn ethereum_address_matches_canonical_vector() {
        let g = hex::decode(G_HEX).unwrap();
        assert_eq!(
            address_for_chain("ethereum", "secp256k1", &g).unwrap(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
    }

    #[test]
    fn bitcoin_address_is_segwit_v0() {
        let g = hex::decode(G_HEX).unwrap();
        let a = address_for_chain("bitcoin", "secp256k1", &g).unwrap();
        assert!(a.starts_with("bc1q"), "{a}");
    }

    #[test]
    fn solana_and_sui_encode_ed25519_keys() {
        let key = [7u8; 32];
        let sol = address_for_chain("solana", "ed25519", &key).unwrap();
        assert_eq!(bs58::decode(&sol).into_vec().unwrap(), key);
        let sui = address_for_chain("sui", "ed25519", &key).unwrap();
        assert!(sui.starts_with("0x") && sui.len() == 66);
    }

    #[test]
    fn wrong_curve_chain_combos_error() {
        assert!(address_for_chain("solana", "secp256k1", &[0u8; 33]).is_err());
        assert!(address_for_chain("ethereum", "ed25519", &[0u8; 32]).is_err());
    }

    #[test]
    fn standard_paths_are_pinned() {
        assert_eq!(standard_path("ethereum", 1).unwrap(), "m/44'/60'/0'/0/1");
        assert_eq!(standard_path("bitcoin", 0).unwrap(), "m/84'/0'/0'/0/0");
        assert_eq!(standard_path("solana", 2).unwrap(), "m/44'/501'/2'/0'");
        assert_eq!(standard_path("sui", 3).unwrap(), "m/44'/784'/3'/0'/0'");
        assert!(standard_path("dogecoin", 0).is_none());
    }

    #[test]
    fn parse_child_wallet_id_round_trips_the_canonical_form() {
        assert_eq!(
            parse_child_wallet_id("19caa3cf46d3-ethereum-7"),
            Some(("19caa3cf46d3", "ethereum", 7))
        );
        assert_eq!(
            parse_child_wallet_id("abc-def-solana-0"),
            Some(("abc-def", "solana", 0))
        );
    }

    #[test]
    fn parse_child_wallet_id_rejects_non_child_ids() {
        // Root ids, unknown chains, non-numeric accounts, junk.
        assert_eq!(parse_child_wallet_id("19caa3cf46d3"), None);
        assert_eq!(parse_child_wallet_id("wallet-dogecoin-1"), None);
        assert_eq!(parse_child_wallet_id("wallet-ethereum-x"), None);
        assert_eq!(parse_child_wallet_id("wallet-ethereum-+7"), None);
        assert_eq!(parse_child_wallet_id("-ethereum-1"), None);
        assert_eq!(parse_child_wallet_id("ethereum-1"), None);
        assert_eq!(parse_child_wallet_id(""), None);
    }

    #[test]
    fn curve_for_chain_matches_chains_for_curve() {
        for curve in ["secp256k1", "ed25519"] {
            for (key, _) in chains_for_curve(curve) {
                assert_eq!(curve_for_chain(key), Some(curve), "{key}");
            }
        }
        assert_eq!(curve_for_chain("dogecoin"), None);
    }

    #[test]
    fn account_addresses_are_deterministic_and_distinct_per_index() {
        use crate::resharing::dkg_keypackages;
        use frost_secp256k1::Secp256K1Sha256 as Secp;
        let (_, pp) = dkg_keypackages::<Secp>(2, 2, 61).unwrap();
        let group = pp.verifying_key().serialize().unwrap();
        let a0 = account_addresses("secp256k1", &group, 0).unwrap();
        let a0b = account_addresses("secp256k1", &group, 0).unwrap();
        let a1 = account_addresses("secp256k1", &group, 1).unwrap();
        assert_eq!(a0, a0b);
        assert_ne!(a0[0].2, a1[0].2);
        assert_eq!(a0.len(), 2); // Ethereum + Bitcoin
        assert!(a0[0].2.starts_with("0x") && a0[1].2.starts_with("bc1q"));
    }
}
