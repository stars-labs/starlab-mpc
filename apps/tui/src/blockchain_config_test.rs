//! Tests for blockchain configuration module

#[cfg(test)]
mod tests {
    use crate::blockchain_config::*;

    #[test]
    fn test_curve_compatibility() {
        // Test secp256k1 compatibility
        let secp_chains = get_compatible_chains(&CurveType::Secp256k1);
        assert!(!secp_chains.is_empty());
        assert_eq!(secp_chains.len(), 1);
        let ecdsa_chains = get_compatible_chains(&CurveType::Ecdsa);
        assert_eq!(ecdsa_chains.len(), 4);
        for chain in ["ethereum", "bsc", "polygon", "avalanche"] {
            assert!(ecdsa_chains.iter().any(|(id, _)| *id == chain));
        }
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
        assert!(
            result
                .unwrap_err()
                .contains("requires secp256k1-ecdsa curve")
        );

        // Test that secp256k1 cannot generate Solana address
        let secp256k1_key = vec![0x02; 33]; // Dummy secp256k1 key (compressed)
        let result = generate_address_for_chain(&secp256k1_key, "secp256k1", "solana");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires ed25519 curve"));
    }

    #[test]
    fn evm_addresses_use_the_ecdsa_key_and_shared_canonical_encoding() {
        let key = hex::decode("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap();
        let expected = starlab_core::accounts::address_for_chain(
            "ethereum",
            starlab_core::accounts::ECDSA_CURVE,
            &key,
        )
        .unwrap();
        assert_eq!(expected, "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf");
        for chain in ["ethereum", "bsc", "polygon", "avalanche"] {
            assert_eq!(
                generate_address_for_chain(&key, starlab_core::accounts::ECDSA_CURVE, chain)
                    .unwrap(),
                expected
            );
            assert!(generate_address_for_chain(&key, "secp256k1", chain).is_err());
        }
        assert!(
            generate_address_for_chain(&key, starlab_core::accounts::ECDSA_CURVE, "bitcoin")
                .is_err()
        );
        assert!(
            generate_address_for_chain(&key, "secp256k1", "bitcoin")
                .unwrap()
                .starts_with("bc1p")
        );
    }

    #[test]
    fn test_curve_type_parsing() {
        assert_eq!(
            CurveType::from_string("secp256k1"),
            Some(CurveType::Secp256k1)
        );
        assert_eq!(CurveType::from_string("ed25519"), Some(CurveType::Ed25519));
        assert_eq!(
            CurveType::from_string("SECP256K1"),
            Some(CurveType::Secp256k1)
        ); // Case insensitive
        assert_eq!(CurveType::from_string("ED25519"), Some(CurveType::Ed25519));
        assert_eq!(
            CurveType::from_string("SECP256K1-ECDSA"),
            Some(CurveType::Ecdsa)
        );
        assert_eq!(
            CurveType::Ecdsa.to_string(),
            starlab_core::accounts::ECDSA_CURVE
        );
        assert_eq!(CurveType::from_string("unknown"), None);
    }
}
