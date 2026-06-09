use ethers_core::utils::keccak256;

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
