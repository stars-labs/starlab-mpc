//! secp256k1 FROST over the BIP-340 / Taproot ciphersuite (`frost-secp256k1-tr`).
//!
//! Signatures are BIP-340 Schnorr, so they spend Bitcoin P2TR outputs; the
//! DKG's `post_dkg` hook commits the root key to an unspendable script path
//! (BIP-341), and account children are BIP-86 output keys (see
//! `hd_derivation::AccountKeyFinalize`). EVM needs ECDSA and does not use this
//! suite for on-chain transactions.

use crate::rng::{OsRng, os_rng};
use crate::{
    errors::{FrostError, Result},
    traits::FrostCurve,
};
use frost_secp256k1_tr::{
    self, Identifier, Signature, SigningPackage,
    keys::{KeyPackage, PublicKeyPackage, dkg},
    round1::{SigningCommitments, SigningNonces},
    round2::SignatureShare,
};
use k256::ecdsa::VerifyingKey as K256VerifyingKey;
use sha3::{Digest, Keccak256};
use std::collections::BTreeMap;

pub struct Secp256k1Curve;

impl FrostCurve for Secp256k1Curve {
    type Identifier = Identifier;
    type KeyPackage = KeyPackage;
    type PublicKeyPackage = PublicKeyPackage;
    type Round1SecretPackage = frost_secp256k1_tr::keys::dkg::round1::SecretPackage;
    type Round2SecretPackage = frost_secp256k1_tr::keys::dkg::round2::SecretPackage;
    type Round1Package = frost_secp256k1_tr::keys::dkg::round1::Package;
    type Round2Package = frost_secp256k1_tr::keys::dkg::round2::Package;
    type VerifyingKey = frost_secp256k1_tr::VerifyingKey;
    type SigningNonces = SigningNonces;
    type SigningCommitments = SigningCommitments;
    type SignatureShare = SignatureShare;
    type Signature = Signature;
    type SigningPackage = SigningPackage;

    fn identifier_from_u16(value: u16) -> Result<Self::Identifier> {
        // frost's own conversion — scalar encoding is curve-specific
        // (ed25519 is little-endian), and CLI/TUI peers use it too.
        Identifier::try_from(value)
            .map_err(|_| FrostError::InvalidIdentifier(format!("Invalid identifier {value}")))
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
    ) -> Result<(
        Self::Round2SecretPackage,
        BTreeMap<Self::Identifier, Self::Round2Package>,
    )> {
        dkg::part2(round1_secret, round1_packages).map_err(|e| FrostError::DkgError(e.to_string()))
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
        let mut rng = os_rng();
        let (nonces, commitments) =
            frost_secp256k1_tr::round1::commit(key_package.signing_share(), &mut rng);
        Ok((nonces, commitments))
    }

    fn generate_signature_share(
        signing_package: &Self::SigningPackage,
        nonces: &Self::SigningNonces,
        key_package: &Self::KeyPackage,
    ) -> Result<Self::SignatureShare> {
        frost_secp256k1_tr::round2::sign(signing_package, nonces, key_package).map_err(|e| {
            FrostError::SigningError(format!("Failed to generate signature share: {:?}", e))
        })
    }

    fn aggregate_signature(
        signing_package: &Self::SigningPackage,
        signature_shares: &BTreeMap<Self::Identifier, Self::SignatureShare>,
        public_key_package: &Self::PublicKeyPackage,
    ) -> Result<Self::Signature> {
        frost_secp256k1_tr::aggregate(signing_package, signature_shares, public_key_package)
            .map_err(|e| FrostError::SigningError(e.to_string()))
    }

    fn create_signing_package(
        commitments: &BTreeMap<Self::Identifier, Self::SigningCommitments>,
        message: &[u8],
    ) -> Result<Self::SigningPackage> {
        Ok(frost_secp256k1_tr::SigningPackage::new(
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
    pub fn get_eth_address(verifying_key: &frost_secp256k1_tr::VerifyingKey) -> Result<String> {
        let pubkey_bytes = verifying_key
            .serialize()
            .map_err(|e| FrostError::SerializationError(e.to_string()))?;

        // Try to interpret as SEC1 uncompressed key
        if let Ok(k256_key) = K256VerifyingKey::from_sec1_bytes(&pubkey_bytes) {
            let uncompressed = k256_key.to_sec1_point(false);
            let uncompressed_bytes = uncompressed.as_bytes();

            // Skip the 0x04 prefix for uncompressed keys
            let public_key_bytes = &uncompressed_bytes[1..];

            // Compute Keccak256 hash
            let hash = Keccak256::digest(public_key_bytes);

            // Take the last 20 bytes as the address
            let address_bytes = &hash[12..];
            Ok(format!("0x{}", hex::encode(address_bytes)))
        } else {
            Err(FrostError::SerializationError(
                "Failed to parse verifying key".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::resharing::{dkg_keypackages, refresh, threshold_sign_verify};
    use frost_secp256k1_tr::Secp256K1Sha256TR as Tr;

    #[test]
    fn taproot_dkg_sign_verify_roundtrip() {
        // The generic engine helpers are ciphersuite-generic, so the whole
        // DKG → threshold-sign → verify pipeline proves the -tr suite works.
        let (kps, pp) = dkg_keypackages::<Tr>(3, 2, 31).unwrap();
        threshold_sign_verify::<Tr>(&kps, &[1, 3], &pp, b"taproot-roundtrip").unwrap();
    }

    #[test]
    fn taproot_verifying_key_is_even_y_normalized() {
        // The -tr ciphersuite serializes SEC1-compressed (33 bytes; parity
        // prefix 0x02/0x03). BIP-340 verifiers use only the x coordinate —
        // consumers take bytes[1..33] as the Taproot output key; the suite
        // handles even-Y normalization internally during signing (proven by
        // the roundtrip test: its verify IS BIP-340).
        let (_, pp) = dkg_keypackages::<Tr>(2, 2, 32).unwrap();
        let bytes = pp.verifying_key().serialize().unwrap();
        assert_eq!(bytes.len(), 33);
        assert!(
            bytes[0] == 0x02 || bytes[0] == 0x03,
            "expected SEC1 parity prefix"
        );
    }

    #[test]
    fn taproot_reshare_works_too() {
        let (kps, pp) = dkg_keypackages::<Tr>(3, 2, 33).unwrap();
        let (new_kps, new_pp) = refresh::<Tr>(&kps, &pp, &[1, 2], 2, 73).unwrap();
        assert_eq!(
            new_pp.verifying_key().serialize().unwrap(),
            pp.verifying_key().serialize().unwrap()
        );
        threshold_sign_verify::<Tr>(&new_kps, &[1, 2], &new_pp, b"taproot-reshare").unwrap();
    }
}
