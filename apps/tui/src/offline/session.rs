//! Offline signing session management

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use super::{
    types::*,
    OfflineError, Result,
};

/// State of an offline signing session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineSession {
    /// Unique session identifier
    pub session_id: String,
    
    /// Current state of the session
    pub state: SessionState,
    
    /// Wallet being used
    pub wallet_id: String,
    
    /// Devices involved in signing
    pub participants: Vec<String>,
    
    /// Minimum signatures required
    pub threshold: u16,
    
    /// Session creation time
    pub created_at: DateTime<Utc>,
    
    /// Session expiration time
    pub expires_at: DateTime<Utc>,
    
    /// Original signing request
    pub signing_request: Option<SigningRequest>,
    
    /// Collected commitments
    pub commitments: HashMap<String, CommitmentsData>,
    
    /// Signing package (once created)
    pub signing_package: Option<SigningPackage>,
    
    /// Collected signature shares
    pub signature_shares: HashMap<String, SignatureShareData>,
    
    /// Final aggregated signature
    pub aggregated_signature: Option<AggregatedSignature>,
}

/// State machine for offline sessions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionState {
    /// Initial state - waiting for signing request
    Created,
    
    /// Signing request received, waiting for commitments
    AwaitingCommitments,
    
    /// Have enough commitments, package can be created
    CommitmentsReady,
    
    /// Signing package created, waiting for shares
    AwaitingShares,
    
    /// Have enough shares, can aggregate
    SharesReady,
    
    /// Signature has been aggregated
    Complete,
    
    /// Session failed or was cancelled
    Failed(String),
}

impl OfflineSession {
    /// Create a new offline signing session
    pub fn new(
        session_id: String,
        wallet_id: String,
        participants: Vec<String>,
        threshold: u16,
        expiration_minutes: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            state: SessionState::Created,
            wallet_id,
            participants,
            threshold,
            created_at: now,
            expires_at: now + chrono::Duration::minutes(expiration_minutes as i64),
            signing_request: None,
            commitments: HashMap::new(),
            signing_package: None,
            signature_shares: HashMap::new(),
            aggregated_signature: None,
        }
    }
    
    /// Check if session has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
    
    /// Validate session is in correct state for an operation
    pub fn validate_state(&self, expected: &SessionState) -> Result<()> {
        if self.is_expired() {
            return Err(OfflineError::SessionExpired(self.expires_at));
        }
        
        if &self.state != expected {
            return Err(OfflineError::InvalidState(format!(
                "Expected state {:?}, but session is in {:?}",
                expected, self.state
            )));
        }
        
        Ok(())
    }
    
    /// Add signing request to session
    pub fn add_signing_request(&mut self, request: SigningRequest) -> Result<()> {
        self.validate_state(&SessionState::Created)?;
        
        // Validate request matches session
        if request.wallet_id != self.wallet_id {
            return Err(OfflineError::InvalidFormat(format!(
                "Wallet mismatch: expected {}, got {}",
                self.wallet_id, request.wallet_id
            )));
        }
        
        self.signing_request = Some(request);
        self.state = SessionState::AwaitingCommitments;
        Ok(())
    }
    
    /// Add commitments from a participant
    pub fn add_commitments(&mut self, commitments: CommitmentsData) -> Result<()> {
        // Allow adding commitments in both states
        match &self.state {
            SessionState::AwaitingCommitments | SessionState::CommitmentsReady => {},
            _ => return Err(OfflineError::InvalidState(format!(
                "Cannot add commitments in state {:?}", self.state
            ))),
        }
        
        // Validate participant
        if !self.participants.contains(&commitments.device_id) {
            return Err(OfflineError::UnauthorizedDevice(commitments.device_id));
        }
        
        self.commitments.insert(commitments.device_id.clone(), commitments);
        
        // Check if we have enough commitments
        if self.commitments.len() >= self.threshold as usize {
            self.state = SessionState::CommitmentsReady;
        }
        
        Ok(())
    }
    
    /// Create signing package from collected commitments
    pub fn create_signing_package(&mut self, message: String) -> Result<SigningPackage> {
        self.validate_state(&SessionState::CommitmentsReady)?;
        
        if self.commitments.len() < self.threshold as usize {
            return Err(OfflineError::ThresholdNotMet(
                self.commitments.len(),
                self.threshold as usize,
            ));
        }
        
        let mut package_commitments = HashMap::new();
        
        for (device_id, commitment) in &self.commitments {
            package_commitments.insert(
                device_id.clone(),
                ParticipantCommitments {
                    identifier: commitment.identifier.clone(),
                    hiding: commitment.hiding_nonce_commitment.clone(),
                    binding: commitment.binding_nonce_commitment.clone(),
                },
            );
        }
        
        let package = SigningPackage {
            session_id: self.session_id.clone(),
            message,
            commitments: package_commitments,
        };
        
        self.signing_package = Some(package.clone());
        self.state = SessionState::AwaitingShares;
        
        Ok(package)
    }
    
    /// Add signature share from a participant
    pub fn add_signature_share(&mut self, share: SignatureShareData) -> Result<()> {
        // Allow adding shares in both states
        match &self.state {
            SessionState::AwaitingShares | SessionState::SharesReady => {},
            _ => return Err(OfflineError::InvalidState(format!(
                "Cannot add signature share in state {:?}", self.state
            ))),
        }
        
        // Validate participant
        if !self.participants.contains(&share.device_id) {
            return Err(OfflineError::UnauthorizedDevice(share.device_id));
        }
        
        // Ensure they provided commitments
        if !self.commitments.contains_key(&share.device_id) {
            return Err(OfflineError::InvalidState(format!(
                "Device {} didn't provide commitments", share.device_id
            )));
        }
        
        self.signature_shares.insert(share.device_id.clone(), share);
        
        // Check if we have enough shares
        if self.signature_shares.len() >= self.threshold as usize {
            self.state = SessionState::SharesReady;
        }
        
        Ok(())
    }
    
    /// Mark session as complete with aggregated signature
    pub fn complete_with_signature(&mut self, signature: AggregatedSignature) -> Result<()> {
        self.validate_state(&SessionState::SharesReady)?;
        
        self.aggregated_signature = Some(signature);
        self.state = SessionState::Complete;
        
        Ok(())
    }
    
    /// Get a summary of session progress
    pub fn get_progress(&self) -> SessionProgress {
        SessionProgress {
            session_id: self.session_id.clone(),
            state: self.state.clone(),
            wallet_id: self.wallet_id.clone(),
            commitments_received: self.commitments.len(),
            commitments_needed: self.threshold as usize,
            shares_received: self.signature_shares.len(),
            shares_needed: self.threshold as usize,
            expires_in: self.expires_at.signed_duration_since(Utc::now()),
        }
    }
}

/// Progress information for an offline session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionProgress {
    pub session_id: String,
    pub state: SessionState,
    pub wallet_id: String,
    pub commitments_received: usize,
    pub commitments_needed: usize,
    pub shares_received: usize,
    pub shares_needed: usize,
    pub expires_in: chrono::Duration,
}