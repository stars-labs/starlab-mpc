//! Trait-based curve identification for FROST ciphersuites
//!
//! This module provides a safe way to identify curve types at runtime
//! without using TypeId comparisons, which can be unreliable.

use frost_core::Ciphersuite;

/// Trait for identifying the curve type of a Ciphersuite
pub trait CurveIdentifier {
    /// Returns the curve type as a string
    fn curve_type() -> &'static str;
}

// Implementation for Secp256k1
impl CurveIdentifier for frost_secp256k1::Secp256K1Sha256 {
    fn curve_type() -> &'static str {
        "secp256k1"
    }
}

// Implementation for Ed25519
impl CurveIdentifier for frost_ed25519::Ed25519Sha512 {
    fn curve_type() -> &'static str {
        "ed25519"
    }
}

/// Helper function to get curve type from a generic Ciphersuite
/// This uses the trait implementation to identify the curve
pub fn get_curve_type<C: Ciphersuite + CurveIdentifier>() -> &'static str {
    C::curve_type()
}

/// Check if a curve type string matches the Ciphersuite
pub fn is_curve_type<C: Ciphersuite + CurveIdentifier>(curve_str: &str) -> bool {
    C::curve_type() == curve_str
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secp256k1_identification() {
        assert_eq!(
            frost_secp256k1::Secp256K1Sha256::curve_type(),
            "secp256k1"
        );
    }

    #[test]
    fn test_ed25519_identification() {
        assert_eq!(
            frost_ed25519::Ed25519Sha512::curve_type(),
            "ed25519"
        );
    }

    #[test]
    fn test_curve_matching() {
        assert!(is_curve_type::<frost_secp256k1::Secp256K1Sha256>("secp256k1"));
        assert!(!is_curve_type::<frost_secp256k1::Secp256K1Sha256>("ed25519"));
        
        assert!(is_curve_type::<frost_ed25519::Ed25519Sha512>("ed25519"));
        assert!(!is_curve_type::<frost_ed25519::Ed25519Sha512>("secp256k1"));
    }
}