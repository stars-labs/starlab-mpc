use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use rand_core::OsRng;

/// Serialize a u16 participant index into a 32-byte big-endian identifier.
///
/// Both ed25519 and secp256k1 use the same identifier encoding:
/// 30 zero bytes followed by the u16 in big-endian.
pub fn identifier_bytes_from_u16(value: u16) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let be = value.to_be_bytes();
    bytes[30] = be[0];
    bytes[31] = be[1];
    bytes
}

/// Generic trait for FROST curve operations
/// This abstracts over Ed25519 and Secp256k1 curves
pub trait FrostCurve {
    type Identifier: Copy + Clone + Serialize + for<'de> Deserialize<'de> + Ord;
    type KeyPackage: Clone + Serialize + for<'de> Deserialize<'de>;
    type PublicKeyPackage: Clone + Serialize + for<'de> Deserialize<'de>;
    type Round1SecretPackage: Clone;
    type Round2SecretPackage: Clone;
    type Round1Package: Clone + Serialize + for<'de> Deserialize<'de>;
    type Round2Package: Clone + Serialize + for<'de> Deserialize<'de>;
    type VerifyingKey;
    
    // FROST signing types
    type SigningNonces: Clone;
    type SigningCommitments: Clone + Serialize + for<'de> Deserialize<'de>;
    type SignatureShare: Clone + Serialize + for<'de> Deserialize<'de>;
    type Signature: Clone + Serialize + for<'de> Deserialize<'de>;
    type SigningPackage;

    // DKG operations
    fn identifier_from_u16(value: u16) -> Result<Self::Identifier>;
    
    fn dkg_part1(
        identifier: Self::Identifier,
        total: u16,
        threshold: u16,
        rng: &mut OsRng,
    ) -> Result<(Self::Round1SecretPackage, Self::Round1Package)>;
    
    // Return type can't be type-aliased from the trait itself (alias would
    // need to be generic over `C: FrostCurve` at the module level, which is
    // more noise than this one call site). Left as-is.
    #[allow(clippy::type_complexity)]
    fn dkg_part2(
        round1_secret: Self::Round1SecretPackage,
        round1_packages: &BTreeMap<Self::Identifier, Self::Round1Package>,
    ) -> Result<(Self::Round2SecretPackage, BTreeMap<Self::Identifier, Self::Round2Package>)>;
    
    fn dkg_part3(
        round2_secret: &Self::Round2SecretPackage,
        round1_packages: &BTreeMap<Self::Identifier, Self::Round1Package>,
        round2_packages: &BTreeMap<Self::Identifier, Self::Round2Package>,
    ) -> Result<(Self::KeyPackage, Self::PublicKeyPackage)>;
    
    // Key operations
    fn verifying_key(public_key_package: &Self::PublicKeyPackage) -> Self::VerifyingKey;
    fn serialize_verifying_key(key: &Self::VerifyingKey) -> Result<Vec<u8>>;
    fn get_address(key: &Self::VerifyingKey) -> String;
    
    // Signing operations
    fn generate_signing_commitment(
        key_package: &Self::KeyPackage,
    ) -> Result<(Self::SigningNonces, Self::SigningCommitments)>;
    
    fn generate_signature_share(
        signing_package: &Self::SigningPackage,
        nonces: &Self::SigningNonces,
        key_package: &Self::KeyPackage,
    ) -> Result<Self::SignatureShare>;
    
    fn aggregate_signature(
        signing_package: &Self::SigningPackage,
        signature_shares: &BTreeMap<Self::Identifier, Self::SignatureShare>,
        public_key_package: &Self::PublicKeyPackage,
    ) -> Result<Self::Signature>;
    
    fn create_signing_package(
        commitments: &BTreeMap<Self::Identifier, Self::SigningCommitments>,
        message: &[u8],
    ) -> Result<Self::SigningPackage>;
    
    fn serialize_signature(signature: &Self::Signature) -> Result<Vec<u8>>;
}