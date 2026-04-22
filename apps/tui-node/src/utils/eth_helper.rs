use ethers_core::types::Address;
use ethers_core::utils::keccak256;
use frost_core::Ciphersuite;
use frost_core::keys::PublicKeyPackage;
use frost_secp256k1::Secp256K1Sha256; // Specific ciphersuite for Ethereum
use k256::elliptic_curve::sec1::ToEncodedPoint; // For public key manipulation
use std::error::Error;

/// Ethereum's `personal_sign` / EIP-191 message hash. Used for
/// signing human-readable messages in a way that's safe from
/// confusion with raw transaction hashes (the 0x19 prefix is
/// invalid RLP, so an EIP-191 message can never be mistaken for a
/// tx). The resulting 32-byte hash is what an off-chain signer
/// passes to ECDSA — FROST is a drop-in replacement for that
/// step.
///
/// Formula:
///     keccak256(b"\x19Ethereum Signed Message:\n" + len(msg) + msg)
///
/// where `len(msg)` is the ASCII-decimal length as bytes.
/// Ecrecover on the produced signature will succeed against the
/// wallet's Ethereum address, making this signature directly
/// usable as a `personal_sign` in dapps.
pub fn eip191_hash(message: &[u8]) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(28 + 16 + message.len());
    preimage.extend_from_slice(b"\x19Ethereum Signed Message:\n");
    preimage.extend_from_slice(message.len().to_string().as_bytes());
    preimage.extend_from_slice(message);
    keccak256(&preimage)
}

/// Derives an Ethereum address from a FROST group verifying key (PublicKeyPackage).
/// This function is specific to the Secp256K1Sha256 ciphersuite used with Ethereum.
pub fn derive_eth_address(
    pubkey_package: &PublicKeyPackage<Secp256K1Sha256>,
) -> Result<Address, Box<dyn Error + Send + Sync>> {
    let group_public_key = pubkey_package.verifying_key();

    // Serialize the key in compressed format first, as per frost_secp256k1 default
    let compressed_bytes = group_public_key.serialize()?;

    // Decompress the public key using k256
    let compressed_point = k256::PublicKey::from_sec1_bytes(&compressed_bytes)
        .map_err(|e| format!("Failed to parse compressed public key: {}", e))?;
    let uncompressed_point = compressed_point.to_encoded_point(false); // false for uncompressed
    let uncompressed_bytes_slice = uncompressed_point.as_bytes();

    // Ensure it's uncompressed (starts with 0x04) and is 65 bytes long
    if uncompressed_bytes_slice.len() != 65 || uncompressed_bytes_slice[0] != 0x04 {
        return Err(format!(
            "Unexpected uncompressed public key format (len={}, prefix={})",
            uncompressed_bytes_slice.len(),
            uncompressed_bytes_slice[0]
        )
        .into());
    }

    // Hash the uncompressed key (excluding the 0x04 prefix)
    let hash = keccak256(&uncompressed_bytes_slice[1..]);

    // Take the last 20 bytes of the hash
    let address_bytes = &hash[12..];
    Ok(Address::from_slice(address_bytes))
}

/// Signs an Ethereum transaction using the FROST protocol.
///
/// NOTE: This is a placeholder function. Actual Ethereum transaction signing
/// with FROST requires constructing the correct transaction payload,
/// signing its hash, and then assembling the final signed transaction.
/// The `KeyPackage` would be of type `KeyPackage<Secp256K1Sha256>`.
#[allow(dead_code)]
pub fn sign_eth_transaction<C: Ciphersuite>(
    transaction_bytes: &[u8], // Typically the EIP-155 hash of the transaction
    _signing_key: &frost_core::keys::KeyPackage<C>, // Should be KeyPackage<Secp256K1Sha256>
) -> Result<Vec<u8>, String> {
    // This is a stub implementation.
    // Actual implementation would involve:
    // 1. Using FROST to sign `transaction_bytes` (which should be a 32-byte hash).
    // 2. Recovering the v, r, s components of the Ethereum signature.
    // 3. Potentially RLP encoding the transaction with the signature.
    Ok(transaction_bytes.to_vec())
}

/// Verifies an Ethereum transaction signature.
///
/// NOTE: This is a placeholder function. Actual verification involves
/// recovering the public key from the signature and message hash,
/// then deriving the Ethereum address from that public key and comparing it.
#[allow(dead_code)]
pub fn verify_eth_signature(
    message_hash: &[u8],    // Typically the EIP-155 hash of the transaction
    signature_bytes: &[u8], // RLP encoded signature or v,r,s components
    _expected_address: &Address,
) -> Result<bool, String> {
    // This is a stub implementation.
    // Actual implementation would involve:
    // 1. Parsing v, r, s from `signature_bytes`.
    // 2. Using ecrecover to get the public key from `message_hash` and v,r,s.
    // 3. Deriving the Ethereum address from the recovered public key.
    // 4. Comparing with `expected_address`.
    if message_hash.is_empty() || signature_bytes.is_empty() {
        return Err("Missing required parameters".to_string());
    }
    // Placeholder for actual verification against expected_address
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known EIP-191 test vector. "hello" (5 bytes) → keccak256 of
    /// `0x19Ethereum Signed Message:\n5hello`.
    /// Matches the hash an MetaMask `personal_sign("hello")` would
    /// produce and what `eth_sign.recoverAddress` expects. The
    /// canonical value is widely published; we hardcode it as a
    /// regression guard so an accidental formatter change (e.g.
    /// using `format!("{:?}")` on the length, or stripping the
    /// 0x19 prefix) would be caught immediately.
    #[test]
    fn eip191_hash_matches_known_vector_for_hello() {
        let hash = eip191_hash(b"hello");
        let expected =
            hex::decode("50b2c43fd39106bafbba0da34fc430e1f91e3c96ea2acee2bc34119f92b37750")
                .expect("decode");
        assert_eq!(hash.as_slice(), expected.as_slice());
    }

    /// Empty message still hashes — the length prefix becomes "0".
    #[test]
    fn eip191_hash_handles_empty_message() {
        // Should be keccak256(b"\x19Ethereum Signed Message:\n0").
        // Easier than hardcoding the result: compute expected
        // independently and assert the two match.
        let preimage = b"\x19Ethereum Signed Message:\n0";
        let expected = ethers_core::utils::keccak256(preimage);
        let hash = eip191_hash(b"");
        assert_eq!(hash, expected);
    }

    /// Length prefix is ASCII-decimal, not binary. A 100-byte
    /// message should hash with "100" in the preimage, not byte
    /// `0x64`. Protects against an accidental `u8::to_le_bytes()`
    /// style mistake.
    #[test]
    fn eip191_hash_length_prefix_is_decimal_ascii() {
        let msg = vec![0x41u8; 100]; // 100 ASCII 'A's
        let preimage_decimal = {
            let mut v = b"\x19Ethereum Signed Message:\n".to_vec();
            v.extend_from_slice(b"100");
            v.extend_from_slice(&msg);
            v
        };
        let preimage_binary = {
            let mut v = b"\x19Ethereum Signed Message:\n".to_vec();
            v.push(100u8); // <-- wrong
            v.extend_from_slice(&msg);
            v
        };
        assert_eq!(
            eip191_hash(&msg),
            ethers_core::utils::keccak256(&preimage_decimal),
            "hash must use decimal-ASCII length prefix"
        );
        assert_ne!(
            eip191_hash(&msg),
            ethers_core::utils::keccak256(&preimage_binary),
            "hash must NOT use binary length — ecrecover would fail"
        );
    }
}
