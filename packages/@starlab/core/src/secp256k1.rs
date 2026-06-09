use crate::{traits::FrostCurve, errors::{FrostError, Result}};
use frost_secp256k1::{
    self,
    Identifier, Signature,
    keys::{
        KeyPackage, PublicKeyPackage,
        dkg,
    },
    round1::{SigningCommitments, SigningNonces},
    round2::SignatureShare,
    SigningPackage,
};
use rand_core::OsRng;
use std::collections::BTreeMap;
use sha3::{Digest, Keccak256};
use k256::ecdsa::VerifyingKey as K256VerifyingKey;

pub struct Secp256k1Curve;

impl FrostCurve for Secp256k1Curve {
    type Identifier = Identifier;
    type KeyPackage = KeyPackage;
    type PublicKeyPackage = PublicKeyPackage;
    type Round1SecretPackage = frost_secp256k1::keys::dkg::round1::SecretPackage;
    type Round2SecretPackage = frost_secp256k1::keys::dkg::round2::SecretPackage;
    type Round1Package = frost_secp256k1::keys::dkg::round1::Package;
    type Round2Package = frost_secp256k1::keys::dkg::round2::Package;
    type VerifyingKey = frost_secp256k1::VerifyingKey;
    type SigningNonces = SigningNonces;
    type SigningCommitments = SigningCommitments;
    type SignatureShare = SignatureShare;
    type Signature = Signature;
    type SigningPackage = SigningPackage;

    fn identifier_from_u16(value: u16) -> Result<Self::Identifier> {
        let bytes = crate::traits::identifier_bytes_from_u16(value);
        Identifier::deserialize(&bytes)
            .map_err(|_| FrostError::InvalidIdentifier("Invalid identifier bytes".to_string()))
    }

    fn dkg_part1(
        identifier: Self::Identifier,
        total: u16,
        threshold: u16,
        rng: &mut OsRng,
    ) -> Result<(Self::Round1SecretPackage, Self::Round1Package)> {
        dkg::part1(identifier, total, threshold, rng)
            .map_err(|e| FrostError::DkgError(e.to_string()))
    }

    fn dkg_part2(
        round1_secret: Self::Round1SecretPackage,
        round1_packages: &BTreeMap<Self::Identifier, Self::Round1Package>,
    ) -> Result<(Self::Round2SecretPackage, BTreeMap<Self::Identifier, Self::Round2Package>)> {
        dkg::part2(round1_secret, round1_packages)
            .map_err(|e| FrostError::DkgError(e.to_string()))
    }

    fn dkg_part3(
        round2_secret: &Self::Round2SecretPackage,
        round1_packages: &BTreeMap<Self::Identifier, Self::Round1Package>,
        round2_packages: &BTreeMap<Self::Identifier, Self::Round2Package>,
    ) -> Result<(Self::KeyPackage, Self::PublicKeyPackage)> {
        dkg::part3(round2_secret, round1_packages, round2_packages)
            .map_err(|e| FrostError::DkgError(e.to_string()))
    }

    fn verifying_key(public_key_package: &Self::PublicKeyPackage) -> Self::VerifyingKey {
        *public_key_package.verifying_key()
    }

    fn serialize_verifying_key(key: &Self::VerifyingKey) -> Result<Vec<u8>> {
        key.serialize()
            .map_err(|e| FrostError::SerializationError(e.to_string()))
    }

    fn get_address(key: &Self::VerifyingKey) -> String {
        // For Secp256k1, this returns a generic hex representation
        // The Ethereum address calculation is done separately
        let pubkey_bytes = key.serialize().unwrap_or_default();
        hex::encode(&pubkey_bytes)
    }

    fn generate_signing_commitment(
        key_package: &Self::KeyPackage,
    ) -> Result<(Self::SigningNonces, Self::SigningCommitments)> {
        let mut rng = OsRng;
        let (nonces, commitments) = frost_secp256k1::round1::commit(key_package.signing_share(), &mut rng);
        Ok((nonces, commitments))
    }

    fn generate_signature_share(
        signing_package: &Self::SigningPackage,
        nonces: &Self::SigningNonces,
        key_package: &Self::KeyPackage,
    ) -> Result<Self::SignatureShare> {
        frost_secp256k1::round2::sign(signing_package, nonces, key_package)
            .map_err(|e| FrostError::SigningError(format!("Failed to generate signature share: {:?}", e)))
    }

    fn aggregate_signature(
        signing_package: &Self::SigningPackage,
        signature_shares: &BTreeMap<Self::Identifier, Self::SignatureShare>,
        public_key_package: &Self::PublicKeyPackage,
    ) -> Result<Self::Signature> {
        frost_secp256k1::aggregate(signing_package, signature_shares, public_key_package)
            .map_err(|e| FrostError::SigningError(e.to_string()))
    }

    fn create_signing_package(
        commitments: &BTreeMap<Self::Identifier, Self::SigningCommitments>,
        message: &[u8],
    ) -> Result<Self::SigningPackage> {
        Ok(frost_secp256k1::SigningPackage::new(
            commitments.clone(),
            message,
        ))
    }

    fn serialize_signature(signature: &Self::Signature) -> Result<Vec<u8>> {
        signature
            .serialize()
            .map(|bytes| bytes.to_vec())
            .map_err(|e| FrostError::SerializationError(e.to_string()))
    }
}

// Additional Ethereum-specific functions
impl Secp256k1Curve {
    pub fn get_eth_address(verifying_key: &frost_secp256k1::VerifyingKey) -> Result<String> {
        let pubkey_bytes = verifying_key.serialize()
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;
        
        // Try to interpret as SEC1 uncompressed key
        if let Ok(k256_key) = K256VerifyingKey::from_sec1_bytes(&pubkey_bytes) {
            let uncompressed = k256_key.to_encoded_point(false);
            let uncompressed_bytes = uncompressed.as_bytes();
            
            // Skip the 0x04 prefix for uncompressed keys
            let public_key_bytes = &uncompressed_bytes[1..];
            
            // Compute Keccak256 hash
            let hash = Keccak256::digest(public_key_bytes);
            
            // Take the last 20 bytes as the address
            let address_bytes = &hash[12..];
            Ok(format!("0x{}", hex::encode(address_bytes)))
        } else {
            Err(FrostError::SerializationError("Failed to parse verifying key".to_string()))
        }
    }
}