//! Blockchain abstraction layer for multi-chain support
//!
//! This module provides traits and implementations for blockchain-specific
//! operations like transaction parsing, message formatting, and signature serialization.

use thiserror::Error;

/// Blockchain error type
#[derive(Debug, Error)]
pub enum BlockchainError {
    #[error("General error: {0}")]
    General(String),
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    #[error("Signature error: {0}")]
    SignatureError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, BlockchainError>;

pub mod ethereum;
pub mod solana;
pub mod bitcoin;

/// Trait for blockchain-specific operations
pub trait BlockchainHandler: Send + Sync {
    /// Get the blockchain identifier
    fn blockchain_id(&self) -> &str;
    
    /// Get the curve type required for this blockchain
    fn curve_type(&self) -> &str;
    
    /// Parse and validate a transaction
    fn parse_transaction(&self, tx_hex: &str) -> Result<ParsedTransaction>;
    
    /// Format a message for signing according to blockchain requirements
    fn format_for_signing(&self, tx: &ParsedTransaction) -> Result<Vec<u8>>;
    
    /// Serialize a signature to blockchain-specific format
    /// The signature is provided as raw bytes
    fn serialize_signature(&self, signature_bytes: &[u8]) -> Result<SignatureData>;
    
    /// Get transaction hash for display/logging
    fn get_tx_hash(&self, tx: &ParsedTransaction) -> String;
}

/// Parsed transaction data
#[derive(Debug, Clone)]
pub struct ParsedTransaction {
    /// Raw transaction bytes
    pub raw_bytes: Vec<u8>,
    /// Transaction hash
    pub hash: String,
    /// Human-readable transaction summary
    pub summary: String,
    /// Chain ID (for EVM chains)
    pub chain_id: Option<u64>,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Serialized signature data
#[derive(Debug, Clone)]
pub struct SignatureData {
    /// The signature in blockchain-specific format
    pub signature: String,
    /// Recovery ID for ECDSA (if applicable)
    pub recovery_id: Option<u8>,
    /// Additional signature data
    pub metadata: serde_json::Value,
}

/// Registry of blockchain handlers
pub struct BlockchainRegistry {
    handlers: std::collections::HashMap<String, Box<dyn BlockchainHandler>>,
}

impl Default for BlockchainRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockchainRegistry {
    /// Create a new registry with default handlers
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: std::collections::HashMap::new(),
        };
        
        // Register default handlers
        registry.register(Box::new(ethereum::EthereumHandler::new()));
        registry.register(Box::new(solana::SolanaHandler::new()));
        registry.register(Box::new(bitcoin::BitcoinHandler::new()));
        
        registry
    }
    
    /// Register a blockchain handler
    pub fn register(&mut self, handler: Box<dyn BlockchainHandler>) {
        self.handlers.insert(handler.blockchain_id().to_string(), handler);
    }
    
    /// Get a handler by blockchain ID
    pub fn get(&self, blockchain: &str) -> Option<&dyn BlockchainHandler> {
        self.handlers.get(blockchain).map(|h| h.as_ref())
    }
    
    /// Get handler for a chain ID (for EVM chains)
    pub fn get_by_chain_id(&self, chain_id: u64) -> Option<&dyn BlockchainHandler> {
        // Map chain IDs to blockchain names
        let blockchain = match chain_id {
            1 => "ethereum",
            56 => "bsc",
            137 => "polygon",
            42161 => "arbitrum",
            10 => "optimism",
            43114 => "avalanche",
            _ => return None,
        };
        self.get(blockchain)
    }
}