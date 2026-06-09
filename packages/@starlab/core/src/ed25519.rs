use crate::{traits::FrostCurve, errors::{FrostError, Result}};
use frost_ed25519::{
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

pub struct Ed25519Curve;

impl FrostCurve for Ed25519Curve {
    type Identifier = Identifier;
    type KeyPackage = KeyPackage;
    type PublicKeyPackage = PublicKeyPackage;
    type Round1SecretPackage = frost_ed25519::keys::dkg::round1::SecretPackage;
    type Round2SecretPackage = frost_ed25519::keys::dkg::round2::SecretPackage;
    type Round1Package = frost_ed25519::keys::dkg::round1::Package;
    type Round2Package = frost_ed25519::keys::dkg::round2::Package;
    type VerifyingKey = frost_ed25519::VerifyingKey;
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
        let pubkey_bytes = key.serialize().unwrap_or_default();
        bs58::encode(pubkey_bytes).into_string()
    }

    fn generate_signing_commitment(
        key_package: &Self::KeyPackage,
    ) -> Result<(Self::SigningNonces, Self::SigningCommitments)> {
        let mut rng = OsRng;
        let (nonces, commitments) = frost_ed25519::round1::commit(key_package.signing_share(), &mut rng);
        Ok((nonces, commitments))
    }

    fn generate_signature_share(
        signing_package: &Self::SigningPackage,
        nonces: &Self::SigningNonces,
        key_package: &Self::KeyPackage,
    ) -> Result<Self::SignatureShare> {
        frost_ed25519::round2::sign(signing_package, nonces, key_package)
            .map_err(|e| FrostError::SigningError(format!("Failed to generate signature share: {:?}", e)))
    }

    fn aggregate_signature(
        signing_package: &Self::SigningPackage,
        signature_shares: &BTreeMap<Self::Identifier, Self::SignatureShare>,
        public_key_package: &Self::PublicKeyPackage,
    ) -> Result<Self::Signature> {
        frost_ed25519::aggregate(signing_package, signature_shares, public_key_package)
            .map_err(|e| FrostError::SigningError(e.to_string()))
    }

    fn create_signing_package(
        commitments: &BTreeMap<Self::Identifier, Self::SigningCommitments>,
        message: &[u8],
    ) -> Result<Self::SigningPackage> {
        Ok(frost_ed25519::SigningPackage::new(
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