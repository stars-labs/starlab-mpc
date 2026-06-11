//! Blockchain configuration and curve compatibility mapping
//! 
//! Different blockchains require different cryptographic curves:
//! - Ethereum, Bitcoin, BSC, Polygon: secp256k1
//! - Solana, Sui, Aptos: ed25519
//! 
//! This module ensures we only generate addresses for compatible chains.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum CurveType {
    Secp256k1,
    Ed25519,
}

impl CurveType {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "secp256k1" => Some(CurveType::Secp256k1),
            "ed25519" => Some(CurveType::Ed25519),
            _ => None,
        }
    }
    
    pub fn to_string(&self) -> &'static str {
        match self {
            CurveType::Secp256k1 => "secp256k1",
            CurveType::Ed25519 => "ed25519",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockchainInfo {
    pub name: &'static str,
    pub curve: CurveType,
    pub symbol: &'static str,
    pub address_prefix: Option<&'static str>,
}

/// Get blockchain configuration
pub fn get_blockchain_config() -> HashMap<&'static str, BlockchainInfo> {
    let mut config = HashMap::new();
    
    // secp256k1 chains (Ethereum compatible)
    config.insert("ethereum", BlockchainInfo {
        name: "Ethereum",
        curve: CurveType::Secp256k1,
        symbol: "ETH",
        address_prefix: Some("0x"),
    });
    
    config.insert("bitcoin", BlockchainInfo {
        name: "Bitcoin",
        curve: CurveType::Secp256k1,
        symbol: "BTC",
        address_prefix: None, // Bitcoin uses different encoding
    });
    
    config.insert("bsc", BlockchainInfo {
        name: "Binance Smart Chain",
        curve: CurveType::Secp256k1,
        symbol: "BNB",
        address_prefix: Some("0x"),
    });
    
    config.insert("polygon", BlockchainInfo {
        name: "Polygon",
        curve: CurveType::Secp256k1,
        symbol: "MATIC",
        address_prefix: Some("0x"),
    });
    
    config.insert("avalanche", BlockchainInfo {
        name: "Avalanche C-Chain",
        curve: CurveType::Secp256k1,
        symbol: "AVAX",
        address_prefix: Some("0x"),
    });
    
    // ed25519 chains
    config.insert("solana", BlockchainInfo {
        name: "Solana",
        curve: CurveType::Ed25519,
        symbol: "SOL",
        address_prefix: None,
    });
    
    config.insert("sui", BlockchainInfo {
        name: "Sui",
        curve: CurveType::Ed25519,
        symbol: "SUI",
        address_prefix: Some("0x"),
    });
    
    config.insert("aptos", BlockchainInfo {
        name: "Aptos",
        curve: CurveType::Ed25519,
        symbol: "APT",
        address_prefix: Some("0x"),
    });
    
    config.insert("near", BlockchainInfo {
        name: "Near",
        curve: CurveType::Ed25519,
        symbol: "NEAR",
        address_prefix: None,
    });
    
    config
}

/// Signing-scheme compatibility caveat for a chain, if one applies.
///
/// FROST produces **Schnorr** signatures. Bitcoin Taproot and the ed25519
/// chains verify those natively, but a standard Ethereum-family **EOA**
/// transaction verifies with **ECDSA** and will reject a Schnorr signature —
/// EVM use needs a smart-contract account (ERC-4337 / a Schnorr verifier).
///
/// This is the single source of truth for that warning so the TUI / native /
/// CLI surface one consistent message (the extension mirrors it in TS). See
/// `docs/SIGNATURE_CHAIN_COMPATIBILITY.md`.
///
/// Returns `None` when the chain verifies FROST signatures natively.
pub fn signing_caveat(chain: &str) -> Option<&'static str> {
    match chain {
        "ethereum" | "bsc" | "polygon" | "avalanche" => Some(
            "Threshold-Schnorr wallet: a standard Ethereum-family EOA \
             transaction verifies with ECDSA and will not accept this \
             signature. EVM use requires a smart-contract account (ERC-4337 / \
             a Schnorr-verifier contract). Receiving is fine; spending via a \
             normal EOA transaction is not supported.",
        ),
        _ => None,
    }
}

/// Get compatible blockchains for a given curve
pub fn get_compatible_chains(curve: &CurveType) -> Vec<(&'static str, BlockchainInfo)> {
    let config = get_blockchain_config();
    config.into_iter()
        .filter(|(_, info)| info.curve == *curve)
        .collect()
}

/// Generate appropriate address based on curve type and chain
pub fn generate_address_for_chain(
    group_public_key: &[u8],
    curve_str: &str,
    chain: &str,
) -> Result<String, String> {
    let curve = CurveType::from_string(curve_str)
        .ok_or_else(|| format!("Unknown curve type: {}", curve_str))?;
    
    let config = get_blockchain_config();
    let chain_info = config.get(chain)
        .ok_or_else(|| format!("Unknown blockchain: {}", chain))?;
    
    // Check curve compatibility
    if chain_info.curve != curve {
        return Err(format!(
            "{} requires {} curve, but wallet uses {}",
            chain_info.name,
            chain_info.curve.to_string(),
            curve.to_string()
        ));
    }
    
    // The four canonical chains delegate to starlab_core::accounts — the
    // single shared implementation (CLI/WASM/desktop must agree byte-for-
    // byte). Remaining chains keep their local encodings below.
    if matches!(chain, "ethereum" | "bitcoin" | "solana" | "sui") {
        return starlab_core::accounts::address_for_chain(
            chain,
            &curve.to_string(),
            group_public_key,
        )
        .map_err(|e| e.to_string());
    }

    // Generate address based on chain type. The canonical chains never reach
    // this match (they early-return above), so only the non-canonical
    // encodings live here.
    match (chain, &curve) {
        // Ethereum-compatible chains with secp256k1
        ("bsc" | "polygon" | "avalanche", CurveType::Secp256k1) => {
            // EVM addresses are keccak256(uncompressed pubkey X‖Y)[12..].
            // FROST serializes the group verifying key COMPRESSED (33 bytes,
            // 0x02/0x03 ‖ X), so we MUST decompress first: hashing the
            // compressed bytes yields an address that does NOT correspond to
            // the signing key. `from_sec1_bytes` accepts both compressed (33)
            // and uncompressed (65) encodings, so this is robust to either
            // input. Must stay byte-for-byte identical to
            // `starlab_core::accounts::address_for_chain("ethereum", …)`.
            use k256::elliptic_curve::sec1::ToEncodedPoint;
            use sha3::{Digest, Keccak256};
            let pk = k256::PublicKey::from_sec1_bytes(group_public_key)
                .map_err(|e| format!("invalid secp256k1 public key: {e}"))?;
            let point = pk.to_encoded_point(false); // 0x04 ‖ X ‖ Y
            let xy = &point.as_bytes()[1..]; // X ‖ Y, 64 bytes
            let mut hasher = Keccak256::new();
            hasher.update(xy);
            let hash = hasher.finalize();
            Ok(format!("0x{}", hex::encode(&hash[12..32]))) // Last 20 bytes
        }

        // Aptos with ed25519
        ("aptos", CurveType::Ed25519) => {
            // Aptos addresses are derived from auth key
            use sha3::{Digest, Sha3_256};
            let mut hasher = Sha3_256::new();
            hasher.update(group_public_key);
            hasher.update([0x00]); // Single signature scheme
            let hash = hasher.finalize();
            Ok(format!("0x{}", hex::encode(&hash[..32])))
        }
        
        _ => {
            Err(format!("Address generation not implemented for {} with {}", chain, curve.to_string()))
        }
    }
}