//! Bitcoin blockchain handler implementation

use super::{BlockchainHandler, ParsedTransaction, SignatureData, Result, BlockchainError};

pub struct BitcoinHandler {
    network: BitcoinNetwork,
}

#[derive(Debug, Clone, Copy)]
pub enum BitcoinNetwork {
    Mainnet,
    Testnet,
}

impl Default for BitcoinHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl BitcoinHandler {
    pub fn new() -> Self {
        Self {
            network: BitcoinNetwork::Mainnet,
        }
    }
    
    pub fn new_testnet() -> Self {
        Self {
            network: BitcoinNetwork::Testnet,
        }
    }
}

impl BlockchainHandler for BitcoinHandler {
    fn blockchain_id(&self) -> &str {
        match self.network {
            BitcoinNetwork::Mainnet => "bitcoin",
            BitcoinNetwork::Testnet => "bitcoin-testnet",
        }
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
        
        // Calculate transaction ID (double SHA256, reversed)
        use sha2::{Digest, Sha256};
        let first_hash = Sha256::digest(&raw_bytes);
        let second_hash = Sha256::digest(first_hash);
        let mut tx_id = second_hash.to_vec();
        tx_id.reverse(); // Bitcoin displays tx IDs in reverse byte order
        
        let metadata = serde_json::json!({
            "network": match self.network {
                BitcoinNetwork::Mainnet => "mainnet",
                BitcoinNetwork::Testnet => "testnet",
            },
            "size": raw_bytes.len(),
        });
        
        let summary = format!(
            "Bitcoin {} transaction (size: {} bytes)",
            match self.network {
                BitcoinNetwork::Mainnet => "mainnet",
                BitcoinNetwork::Testnet => "testnet",
            },
            raw_bytes.len()
        );
        
        Ok(ParsedTransaction {
            raw_bytes,
            hash: hex::encode(tx_id),
            summary,
            chain_id: None,
            metadata,
        })
    }
    
    fn format_for_signing(&self, tx: &ParsedTransaction) -> Result<Vec<u8>> {
        // For Bitcoin, we typically sign transaction inputs
        // This requires parsing the transaction and creating sighash
        // For now, we'll sign the double SHA256 of the transaction
        
        use sha2::{Digest, Sha256};
        let first_hash = Sha256::digest(&tx.raw_bytes);
        let second_hash = Sha256::digest(first_hash);
        
        Ok(second_hash.to_vec())
    }
    
    fn serialize_signature(&self, signature_bytes: &[u8]) -> Result<SignatureData> {
        // Bitcoin uses DER encoding for signatures
        if signature_bytes.len() < 64 {
            return Err(BlockchainError::SignatureError(
                format!("Invalid signature length: expected at least 64 bytes, got {}", signature_bytes.len())
            ));
        }
        
        // Extract r and s components (assuming 64 bytes total)
        let r = &signature_bytes[..32];
        let s = &signature_bytes[32..64];
        
        // Create DER encoding (simplified - use bitcoin crate in production).
        // Layout: SEQUENCE(0x30) len(0x44) INTEGER(0x02) len(0x20) r(32)
        //         INTEGER(0x02) len(0x20) s(32) SIGHASH_ALL(0x01)
        let mut der = vec![
            0x30, 0x44,           // SEQUENCE, total length
            0x02, 0x20,           // INTEGER, r-length
        ];
        der.extend_from_slice(r);
        der.extend_from_slice(&[0x02, 0x20]); // INTEGER, s-length
        der.extend_from_slice(s);
        der.push(0x01);                        // SIGHASH_ALL
        
        Ok(SignatureData {
            signature: hex::encode(&der),
            recovery_id: None,
            metadata: serde_json::json!({
                "format": "bitcoin-der",
                "sighash_type": "SIGHASH_ALL"
            }),
        })
    }
    
    fn get_tx_hash(&self, tx: &ParsedTransaction) -> String {
        tx.hash.clone()
    }
}