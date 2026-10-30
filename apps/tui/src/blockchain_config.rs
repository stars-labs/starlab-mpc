//! Signing-key compatibility and blockchain address configuration.
//!
//! EVM chains require the cggmp24 ECDSA key. The secp256k1 FROST
//! BIP-340 key controls Bitcoin Taproot; ed25519 FROST controls ed25519 chains.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum CurveType {
    Secp256k1,
    Ecdsa,
    Ed25519,
}

impl CurveType {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "secp256k1" => Some(CurveType::Secp256k1),
            starlab_core::accounts::ECDSA_CURVE => Some(CurveType::Ecdsa),
            "ed25519" => Some(CurveType::Ed25519),
            _ => None,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            CurveType::Secp256k1 => "secp256k1",
            CurveType::Ed25519 => "ed25519",
            CurveType::Ecdsa => starlab_core::accounts::ECDSA_CURVE,
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

    // ECDSA chains (Ethereum-compatible EOAs)
    config.insert(
        "ethereum",
        BlockchainInfo {
            name: "Ethereum",
            curve: CurveType::Ecdsa,
            symbol: "ETH",
            address_prefix: Some("0x"),
        },
    );

    config.insert(
        "bitcoin",
        BlockchainInfo {
            name: "Bitcoin",
            curve: CurveType::Secp256k1,
            symbol: "BTC",
            address_prefix: None, // Bitcoin uses different encoding
        },
    );

    config.insert(
        "bsc",
        BlockchainInfo {
            name: "Binance Smart Chain",
            curve: CurveType::Ecdsa,
            symbol: "BNB",
            address_prefix: Some("0x"),
        },
    );

    config.insert(
        "polygon",
        BlockchainInfo {
            name: "Polygon",
            curve: CurveType::Ecdsa,
            symbol: "MATIC",
            address_prefix: Some("0x"),
        },
    );

    config.insert(
        "avalanche",
        BlockchainInfo {
            name: "Avalanche C-Chain",
            curve: CurveType::Ecdsa,
            symbol: "AVAX",
            address_prefix: Some("0x"),
        },
    );

    // ed25519 chains
    config.insert(
        "solana",
        BlockchainInfo {
            name: "Solana",
            curve: CurveType::Ed25519,
            symbol: "SOL",
            address_prefix: None,
        },
    );

    config.insert(
        "sui",
        BlockchainInfo {
            name: "Sui",
            curve: CurveType::Ed25519,
            symbol: "SUI",
            address_prefix: Some("0x"),
        },
    );

    config.insert(
        "aptos",
        BlockchainInfo {
            name: "Aptos",
            curve: CurveType::Ed25519,
            symbol: "APT",
            address_prefix: Some("0x"),
        },
    );

    config.insert(
        "near",
        BlockchainInfo {
            name: "Near",
            curve: CurveType::Ed25519,
            symbol: "NEAR",
            address_prefix: None,
        },
    );

    config
}

/// Get compatible blockchains for a given curve
pub fn get_compatible_chains(curve: &CurveType) -> Vec<(&'static str, BlockchainInfo)> {
    let config = get_blockchain_config();
    config
        .into_iter()
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
    let chain_info = config
        .get(chain)
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
            curve.to_string(),
            group_public_key,
        )
        .map_err(|e| e.to_string());
    }

    // Generate address based on chain type. The canonical chains never reach
    // this match (they early-return above), so only the non-canonical
    // encodings live here.
    match (chain, &curve) {
        // EVM chains share the canonical Ethereum ECDSA address encoding.
        ("bsc" | "polygon" | "avalanche", CurveType::Ecdsa) => {
            starlab_core::accounts::address_for_chain(
                "ethereum",
                curve.to_string(),
                group_public_key,
            )
            .map_err(|e| e.to_string())
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

        _ => Err(format!(
            "Address generation not implemented for {} with {}",
            chain,
            curve.to_string()
        )),
    }
}
