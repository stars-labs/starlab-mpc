//! Tests for blockchain configuration module

#[cfg(test)]
mod tests {
    use crate::blockchain_config::*;

    #[test]
    fn test_curve_compatibility() {
        // Test secp256k1 compatibility
        let secp_chains = get_compatible_chains(&CurveType::Secp256k1);
        assert!(!secp_chains.is_empty());
        assert!(secp_chains.iter().any(|(id, _)| *id == "ethereum"));
        assert!(secp_chains.iter().any(|(id, _)| *id == "bitcoin"));
        
        // Test ed25519 compatibility
        let ed_chains = get_compatible_chains(&CurveType::Ed25519);
        assert!(!ed_chains.is_empty());
        assert!(ed_chains.iter().any(|(id, _)| *id == "solana"));
        assert!(ed_chains.iter().any(|(id, _)| *id == "sui"));
        
        // Ensure no overlap - ed25519 should not have Ethereum
        assert!(!ed_chains.iter().any(|(id, _)| *id == "ethereum"));
        
        // Ensure no overlap - secp256k1 should not have Solana
        assert!(!secp_chains.iter().any(|(id, _)| *id == "solana"));
    }
    
    #[test]
    fn test_address_generation_incompatibility() {
        // Test that ed25519 cannot generate Ethereum address
        let ed25519_key = vec![0u8; 32]; // Dummy ed25519 key
        let result = generate_address_for_chain(&ed25519_key, "ed25519", "ethereum");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires secp256k1 curve"));
        
        // Test that secp256k1 cannot generate Solana address
        let secp256k1_key = vec![0x02; 33]; // Dummy secp256k1 key (compressed)
        let result = generate_address_for_chain(&secp256k1_key, "secp256k1", "solana");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires ed25519 curve"));
    }
    
    #[test]
    fn test_signing_caveat() {
        // EVM EOAs verify with ECDSA → FROST Schnorr needs a contract account.
        for evm in ["ethereum", "bsc", "polygon", "avalanche"] {
            let c = signing_caveat(evm).expect("EVM chains must carry a caveat");
            assert!(c.contains("ECDSA"));
            assert!(c.contains("smart-contract account"));
        }
        // Chains that verify Schnorr/Ed25519 natively carry no caveat.
        for native in ["bitcoin", "solana", "sui", "aptos", "near"] {
            assert!(signing_caveat(native).is_none(), "{native} should be native");
        }
    }

    #[test]
    fn test_curve_type_parsing() {
        assert_eq!(CurveType::from_string("secp256k1"), Some(CurveType::Secp256k1));
        assert_eq!(CurveType::from_string("ed25519"), Some(CurveType::Ed25519));
        assert_eq!(CurveType::from_string("SECP256K1"), Some(CurveType::Secp256k1)); // Case insensitive
        assert_eq!(CurveType::from_string("ED25519"), Some(CurveType::Ed25519));
        assert_eq!(CurveType::from_string("unknown"), None);
    }
}