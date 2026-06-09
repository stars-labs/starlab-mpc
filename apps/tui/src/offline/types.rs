//! Type definitions for offline mode data structures

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Version of the offline data format
pub const OFFLINE_DATA_VERSION: &str = "1.0";

/// Wrapper for all offline data transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineData {
    /// Format version
    pub version: String,
    
    /// Type of data
    #[serde(rename = "type")]
    pub data_type: OfflineDataType,
    
    /// Unique session identifier
    pub session_id: String,
    
    /// When this data was created
    pub created_at: DateTime<Utc>,
    
    /// When this data expires and should not be used
    pub expires_at: DateTime<Utc>,
    
    /// The actual data payload
    pub data: serde_json::Value,
}

/// Types of offline data that can be transferred
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OfflineDataType {
    /// Initial signing request from coordinator
    SigningRequest,
    
    /// Nonce commitments from a signer
    Commitments,
    
    /// Aggregated commitments package from coordinator
    SigningPackage,
    
    /// Signature share from a signer
    SignatureShare,
    
    /// Final aggregated signature
    AggregatedSignature,
}

/// Signing request sent from coordinator to signers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningRequest {
    /// Wallet identifier
    pub wallet_id: String,
    
    /// Transaction to be signed
    pub transaction: TransactionData,
    
    /// Human-readable description
    pub message: String,
    
    /// Required signing devices
    pub required_signers: Vec<String>,
    
    /// Minimum number of signers needed
    pub threshold: u16,
    
    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

/// Transaction data to be signed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    /// Blockchain type
    #[serde(rename = "type")]
    pub chain_type: String, // "ethereum" or "solana"
    
    /// Encoded transaction payload
    pub payload: String, // Base64 encoded
    
    /// Transaction hash
    pub hash: String, // Hex encoded
    
    /// Chain-specific data
    pub chain_data: Option<serde_json::Value>,
}

/// Nonce commitments from a signing participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentsData {
    /// Original session ID this is responding to
    pub session_id: String,
    
    /// Device that generated these commitments
    pub device_id: String,
    
    /// FROST identifier (hex)
    pub identifier: String,
    
    /// Hiding nonce commitment (hex)
    pub hiding_nonce_commitment: String,
    
    /// Binding nonce commitment (hex)
    pub binding_nonce_commitment: String,
}

/// Signing package with aggregated commitments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningPackage {
    /// Session identifier
    pub session_id: String,
    
    /// Message to sign (hex encoded)
    pub message: String,
    
    /// Commitments from all participants
    pub commitments: HashMap<String, ParticipantCommitments>,
}

/// Individual participant's commitments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantCommitments {
    /// FROST identifier (hex)
    pub identifier: String,
    
    /// Hiding commitment (hex)
    pub hiding: String,
    
    /// Binding commitment (hex)
    pub binding: String,
}

/// Signature share from a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureShareData {
    /// Session identifier
    pub session_id: String,
    
    /// Device that generated this share
    pub device_id: String,
    
    /// FROST identifier (hex)
    pub identifier: String,
    
    /// The signature share (hex)
    pub signature_share: String,
}

/// Final aggregated signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedSignature {
    /// Session identifier
    pub session_id: String,
    
    /// The complete signature
    pub signature: SignatureData,
    
    /// Devices that contributed
    pub signers: Vec<String>,
    
    /// Original transaction data
    pub transaction: TransactionData,
}

/// Signature data format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureData {
    /// Signature algorithm
    pub algorithm: String, // "ecdsa" or "eddsa"
    
    /// The signature value
    pub value: SignatureValue,
}

/// Signature value representation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SignatureValue {
    /// ECDSA signature (r, s)
    Ecdsa { r: String, s: String },
    
    /// EdDSA signature
    Eddsa { signature: String },
}

impl OfflineData {
    /// Create a new offline data wrapper
    pub fn new(
        data_type: OfflineDataType,
        session_id: String,
        data: impl Serialize,
        expiration_minutes: u64,
    ) -> Result<Self, super::OfflineError> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::minutes(expiration_minutes as i64);
        
        let data_value = serde_json::to_value(data)
            .map_err(|e| super::OfflineError::SerializationError(e.to_string()))?;
        
        Ok(Self {
            version: OFFLINE_DATA_VERSION.to_string(),
            data_type,
            session_id,
            created_at: now,
            expires_at,
            data: data_value,
        })
    }
    
    /// Check if this data has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
    
    /// Validate the data
    pub fn validate(&self) -> Result<(), super::OfflineError> {
        // Check version
        if self.version != OFFLINE_DATA_VERSION {
            return Err(super::OfflineError::InvalidFormat(format!(
                "Unsupported version: {}, expected: {}",
                self.version, OFFLINE_DATA_VERSION
            )));
        }
        
        // Check expiration
        if self.is_expired() {
            return Err(super::OfflineError::SessionExpired(self.expires_at));
        }
        
        Ok(())
    }
    
    /// Extract typed data
    pub fn extract<T: for<'de> Deserialize<'de>>(&self) -> Result<T, super::OfflineError> {
        serde_json::from_value(self.data.clone())
            .map_err(|e| super::OfflineError::InvalidFormat(e.to_string()))
    }
}