//! Ethereum blockchain handler implementation

use super::{BlockchainHandler, ParsedTransaction, SignatureData, Result, BlockchainError};

pub struct EthereumHandler {
    // Can add configuration here if needed
}

impl Default for EthereumHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl EthereumHandler {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Parse Ethereum transaction and extract key fields
    fn parse_eth_transaction(tx_bytes: &[u8]) -> Result<(String, u64, serde_json::Value)> {
        // Basic validation
        if tx_bytes.is_empty() {
            return Err(BlockchainError::InvalidTransaction(
                "Empty transaction data".to_string()
            ));
        }

        // Calculate transaction hash (keccak256)
        use sha3::{Digest, Keccak256};
        let tx_hash = hex::encode(Keccak256::digest(tx_bytes));

        // chain_id + envelope type. Typed envelopes (EIP-2718) put the chain
        // id as the FIRST list item right after the type byte; legacy txs
        // don't carry one pre-signing (EIP-155 encodes it in `v` only after
        // signing), so legacy falls back to mainnet with the type recorded.
        let (chain_id, tx_type) = match tx_bytes[0] {
            0x01 => (Self::rlp_first_u64(&tx_bytes[1..])?, "eip2930"),
            0x02 => (Self::rlp_first_u64(&tx_bytes[1..])?, "eip1559"),
            0x03 => (Self::rlp_first_u64(&tx_bytes[1..])?, "eip4844"),
            b if b >= 0xc0 => (1u64, "legacy"), // RLP list ⇒ legacy envelope
            b => {
                return Err(BlockchainError::InvalidTransaction(format!(
                    "unknown transaction envelope type 0x{b:02x}"
                )))
            }
        };

        let metadata = serde_json::json!({
            "type": tx_type,
            "size": tx_bytes.len(),
        });

        Ok((tx_hash, chain_id, metadata))
    }

    /// Decode the first item of an RLP list payload as a u64 — for typed tx
    /// envelopes this is the chain id. Handles the two encodings a u64 can
    /// have: single byte < 0x80, or 0x80+len prefix followed by big-endian
    /// bytes.
    fn rlp_first_u64(rlp: &[u8]) -> Result<u64> {
        let err = |m: &str| BlockchainError::InvalidTransaction(m.to_string());
        if rlp.is_empty() {
            return Err(err("empty RLP payload"));
        }
        // Skip the outer list header.
        let payload_start = match rlp[0] {
            b if (0xc0..=0xf7).contains(&b) => 1,
            b if b >= 0xf8 => {
                let len_of_len = (b - 0xf7) as usize;
                1 + len_of_len
            }
            _ => return Err(err("typed tx body is not an RLP list")),
        };
        let item = rlp.get(payload_start..).ok_or_else(|| err("truncated RLP list"))?;
        if item.is_empty() {
            return Err(err("empty RLP list"));
        }
        match item[0] {
            // single-byte value (0x00–0x7f encodes itself)
            b if b < 0x80 => Ok(b as u64),
            // 0x80 = zero-length string ⇒ value 0
            0x80 => Ok(0),
            // short string: 0x80+len, then big-endian bytes
            b if b <= 0x88 => {
                let len = (b - 0x80) as usize;
                let bytes = item.get(1..1 + len).ok_or_else(|| err("truncated chain id"))?;
                let mut v = 0u64;
                for &x in bytes {
                    v = (v << 8) | x as u64;
                }
                Ok(v)
            }
            _ => Err(err("chain id too large for u64")),
        }
    }
}

impl BlockchainHandler for EthereumHandler {
    fn blockchain_id(&self) -> &str {
        "ethereum"
    }
    
    fn curve_type(&self) -> &str {
        "secp256k1"
    }
    
    fn parse_transaction(&self, tx_hex: &str) -> Result<ParsedTransaction> {
        // Remove 0x prefix if present
        let tx_hex = tx_hex.strip_prefix("0x").unwrap_or(tx_hex);
        
        // Decode hex to bytes
        let raw_bytes = hex::decode(tx_hex)
            .map_err(|e| BlockchainError::ParseError(
                format!("Invalid hex transaction: {}", e)
            ))?;
        
        // Parse transaction
        let (hash, chain_id, metadata) = Self::parse_eth_transaction(&raw_bytes)?;
        
        // Create summary
        let summary = format!(
            "Ethereum transaction on chain {} (size: {} bytes)",
            chain_id,
            raw_bytes.len()
        );
        
        Ok(ParsedTransaction {
            raw_bytes,
            hash: format!("0x{}", hash),
            summary,
            chain_id: Some(chain_id),
            metadata,
        })
    }
    
    fn format_for_signing(&self, tx: &ParsedTransaction) -> Result<Vec<u8>> {
        // For Ethereum, we sign the transaction hash (keccak256)
        use sha3::{Digest, Keccak256};
        let hash = Keccak256::digest(&tx.raw_bytes);
        Ok(hash.to_vec())
    }
    
    fn serialize_signature(&self, signature_bytes: &[u8]) -> Result<SignatureData> {
        // FROST signatures are typically 64 bytes (r,s)
        if signature_bytes.len() < 64 {
            return Err(BlockchainError::SignatureError(
                format!("Invalid signature length: expected at least 64 bytes, got {}", signature_bytes.len())
            ));
        }
        
        // For Ethereum, we need to format as r,s,v
        // FROST gives us the signature, but we need to calculate v (recovery id)
        // This is complex and requires the public key and message
        
        // For now, return the raw signature
        // In production, calculate proper recovery ID
        let r = &signature_bytes[..32];
        let s = &signature_bytes[32..64];
        
        // Format as 0x-prefixed hex
        let signature_hex = format!("0x{}{}", hex::encode(r), hex::encode(s));
        
        Ok(SignatureData {
            signature: signature_hex,
            recovery_id: Some(27), // Placeholder - need proper calculation
            metadata: serde_json::json!({
                "format": "ethereum",
                "note": "Recovery ID needs proper calculation"
            }),
        })
    }
    
    fn get_tx_hash(&self, tx: &ParsedTransaction) -> String {
        tx.hash.clone()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8]) -> (u64, String) {
        let (_, chain_id, meta) = EthereumHandler::parse_eth_transaction(bytes).unwrap();
        (chain_id, meta["type"].as_str().unwrap().to_string())
    }

    #[test]
    fn eip1559_mainnet_chain_id() {
        // 0x02 || rlp([0x01, …]) — single-byte chain id 1
        let (id, ty) = parse(&[0x02, 0xc1, 0x01]);
        assert_eq!(id, 1);
        assert_eq!(ty, "eip1559");
    }

    #[test]
    fn eip1559_polygon_chain_id() {
        // chain id 137 = 0x89 encodes as 0x81 0x89
        let (id, _) = parse(&[0x02, 0xc2, 0x81, 0x89]);
        assert_eq!(id, 137);
    }

    #[test]
    fn eip1559_arbitrum_multibyte_chain_id() {
        // 42161 = 0xa4b1 → 0x82 0xa4 0xb1
        let (id, _) = parse(&[0x02, 0xc3, 0x82, 0xa4, 0xb1]);
        assert_eq!(id, 42161);
    }

    #[test]
    fn eip2930_and_long_list_header() {
        let (id, ty) = parse(&[0x01, 0xc1, 0x05]);
        assert_eq!((id, ty.as_str()), (5, "eip2930"));
        // long-form list header (0xf8 + 1-byte length), chain id 5 first
        let mut long = vec![0x02, 0xf8, 0x3c, 0x05];
        long.extend(std::iter::repeat(0u8).take(59));
        let (id2, _) = parse(&long);
        assert_eq!(id2, 5);
    }

    #[test]
    fn legacy_falls_back_to_mainnet() {
        let (id, ty) = parse(&[0xc1, 0x01]);
        assert_eq!((id, ty.as_str()), (1, "legacy"));
    }

    #[test]
    fn unknown_envelope_rejected() {
        assert!(EthereumHandler::parse_eth_transaction(&[0x7f, 0x00]).is_err());
        assert!(EthereumHandler::parse_eth_transaction(&[]).is_err());
    }
}
