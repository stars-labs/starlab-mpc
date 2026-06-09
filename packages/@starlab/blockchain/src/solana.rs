//! Solana blockchain handler implementation

use super::{BlockchainHandler, ParsedTransaction, SignatureData, Result, BlockchainError};
use solana_sdk::bs58;

pub struct SolanaHandler {
    // Can add configuration here if needed
}

impl Default for SolanaHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SolanaHandler {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Parse Solana transaction
    fn parse_solana_transaction(tx_bytes: &[u8]) -> Result<(String, serde_json::Value)> {
        // Basic validation
        if tx_bytes.is_empty() {
            return Err(BlockchainError::InvalidTransaction(
                "Empty transaction data".to_string()
            ));
        }
        
        // For Solana, we typically sign the message directly
        // Transaction format is more complex (includes recent blockhash, instructions, etc.)
        
        // Calculate transaction ID (first signature will be the ID)
        // For unsigned tx, we'll use a hash
        use sha2::{Digest, Sha256};
        let tx_hash = hex::encode(Sha256::digest(tx_bytes));
        
        let metadata = serde_json::json!({
            "type": "transaction",
            "size": tx_bytes.len(),
            "version": "legacy", // or "v0"
        });
        
        Ok((tx_hash, metadata))
    }
}

impl BlockchainHandler for SolanaHandler {
    fn blockchain_id(&self) -> &str {
        "solana"
    }
    
    fn curve_type(&self) -> &str {
        "ed25519"
    }
    
    fn parse_transaction(&self, tx_hex: &str) -> Result<ParsedTransaction> {
        // Remove 0x prefix if present (though Solana typically uses base58)
        let tx_hex = tx_hex.strip_prefix("0x").unwrap_or(tx_hex);
        
        // Try to decode as hex first
        let raw_bytes = if let Ok(bytes) = hex::decode(tx_hex) {
            bytes
        } else {
            // Try base58 decode
            bs58::decode(tx_hex)
                .into_vec()
                .map_err(|e| BlockchainError::ParseError(
                    format!("Invalid transaction encoding: {}", e)
                ))?
        };
        
        // Parse transaction
        let (hash, metadata) = Self::parse_solana_transaction(&raw_bytes)?;
        
        // Create summary
        let summary = format!(
            "Solana transaction (size: {} bytes)",
            raw_bytes.len()
        );
        
        Ok(ParsedTransaction {
            raw_bytes,
            hash,
            summary,
            chain_id: None, // Solana doesn't use chain IDs
            metadata,
        })
    }
    
    fn format_for_signing(&self, tx: &ParsedTransaction) -> Result<Vec<u8>> {
        // For Solana, we sign the serialized transaction bytes directly
        // No additional hashing needed (Solana does this internally)
        Ok(tx.raw_bytes.clone())
    }
    
    fn serialize_signature(&self, signature_bytes: &[u8]) -> Result<SignatureData> {
        // Solana expects 64-byte signatures
        if signature_bytes.len() != 64 {
            return Err(BlockchainError::SignatureError(
                format!("Invalid signature length for Solana: {} bytes", signature_bytes.len())
            ));
        }
        
        // Solana uses base58 encoding for signatures
        let signature_b58 = bs58::encode(signature_bytes).into_string();
        
        Ok(SignatureData {
            signature: signature_b58,
            recovery_id: None, // Not used for Ed25519
            metadata: serde_json::json!({
                "format": "solana",
                "encoding": "base58"
            }),
        })
    }
    
    fn get_tx_hash(&self, tx: &ParsedTransaction) -> String {
        tx.hash.clone()
    }
}