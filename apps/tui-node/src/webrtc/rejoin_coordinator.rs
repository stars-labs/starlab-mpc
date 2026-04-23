//! Coordinator for handling participant rejoin and state recovery

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use serde::{Serialize, Deserialize};

use super::mesh_manager::PeerId;

/// Session state for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Session ID
    pub session_id: String,
    /// Current round/phase
    pub current_round: u8,
    /// Participants in session
    pub participants: Vec<PeerId>,
    /// Threshold required
    pub threshold: usize,
    /// Messages exchanged
    pub message_count: u64,
    /// Session start time
    pub started_at: u64,
}

/// Rejoin request from a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejoinRequest {
    /// Requesting peer
    pub peer_id: PeerId,
    /// Session they want to rejoin
    pub session_id: String,
    /// Last known round
    pub last_round: u8,
    /// Authentication token
    pub auth_token: String,
    /// Timestamp
    pub timestamp: u64,
}

/// Rejoin response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejoinResponse {
    /// Whether rejoin was accepted
    pub accepted: bool,
    /// Current session state
    pub session_state: Option<SessionState>,
    /// Missed messages to catch up
    pub missed_messages: Vec<MissedMessage>,
    /// Reason if rejected
    pub rejection_reason: Option<String>,
}

/// Missed message during disconnection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissedMessage {
    /// Message sender
    pub from: PeerId,
    /// Message round
    pub round: u8,
    /// Message type
    pub msg_type: String,
    /// Message data
    pub data: Vec<u8>,
    /// Timestamp
    pub timestamp: u64,
}

/// Message buffer for recovery
#[derive(Debug, Clone)]
pub struct MessageBuffer {
    /// Maximum messages to keep
    max_size: usize,
    /// Buffered messages
    messages: VecDeque<MissedMessage>,
}

impl MessageBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            messages: VecDeque::with_capacity(max_size),
        }
    }

    pub fn add_message(&mut self, message: MissedMessage) {
        if self.messages.len() >= self.max_size {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }

    pub fn get_messages_since(&self, round: u8) -> Vec<MissedMessage> {
        self.messages.iter()
            .filter(|m| m.round >= round)
            .cloned()
            .collect()
    }
}

/// Rejoin coordinator for managing participant recovery
pub struct RejoinCoordinator {
    /// Pending rejoin requests
    pub pending_rejoins: Arc<Mutex<HashMap<PeerId, RejoinRequest>>>,
    /// Current session state
    pub session_state: Arc<Mutex<SessionState>>,
    /// Message buffers per peer
    pub message_buffers: Arc<Mutex<HashMap<PeerId, MessageBuffer>>>,
    /// Authenticated peers
    pub authenticated_peers: Arc<Mutex<HashMap<PeerId, String>>>,
    /// Rejoin history
    pub rejoin_history: Arc<Mutex<Vec<RejoinEvent>>>,
}

/// Rejoin event for history tracking
#[derive(Debug, Clone)]
pub struct RejoinEvent {
    pub peer_id: PeerId,
    pub timestamp: Instant,
    pub success: bool,
    pub reason: String,
}

impl RejoinCoordinator {
    /// Creates a new rejoin coordinator
    pub fn new(session_id: String, participants: Vec<PeerId>, threshold: usize) -> Self {
        let session_state = SessionState {
            session_id,
            current_round: 0,
            participants,
            threshold,
            message_count: 0,
            started_at: Instant::now().elapsed().as_secs(),
        };

        Self {
            pending_rejoins: Arc::new(Mutex::new(HashMap::new())),
            session_state: Arc::new(Mutex::new(session_state)),
            message_buffers: Arc::new(Mutex::new(HashMap::new())),
            authenticated_peers: Arc::new(Mutex::new(HashMap::new())),
            rejoin_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Handles a rejoin request
    pub async fn handle_rejoin_request(&self, request: RejoinRequest) -> RejoinResponse {
        println!("  🔄 Processing rejoin request from peer {}", request.peer_id);
        
        // Validate the request
        if !self.validate_rejoin(&request).await {
            self.record_rejoin_event(request.peer_id, false, "Validation failed");
            return RejoinResponse {
                accepted: false,
                session_state: None,
                missed_messages: Vec::new(),
                rejection_reason: Some("Invalid rejoin request".to_string()),
            };
        }

        // Authenticate the peer
        if !self.authenticate_peer(request.peer_id, &request.auth_token) {
            self.record_rejoin_event(request.peer_id, false, "Authentication failed");
            return RejoinResponse {
                accepted: false,
                session_state: None,
                missed_messages: Vec::new(),
                rejection_reason: Some("Authentication failed".to_string()),
            };
        }

        // Store the rejoin request
        self.pending_rejoins.lock().unwrap().insert(request.peer_id, request.clone());

        // Get current session state
        let session_state = self.session_state.lock().unwrap().clone();

        // Get missed messages
        let missed_messages = self.get_missed_messages(request.peer_id, request.last_round);

        println!("  ✅ Rejoin accepted for peer {}", request.peer_id);
        println!("    • Current round: {}", session_state.current_round);
        println!("    • Missed messages: {}", missed_messages.len());

        self.record_rejoin_event(request.peer_id, true, "Rejoin successful");

        RejoinResponse {
            accepted: true,
            session_state: Some(session_state),
            missed_messages,
            rejection_reason: None,
        }
    }

    /// Validates a rejoin request
    pub async fn validate_rejoin(&self, request: &RejoinRequest) -> bool {
        let session = self.session_state.lock().unwrap();
        
        // Check session ID matches
        if request.session_id != session.session_id {
            println!("  ❌ Invalid session ID");
            return false;
        }

        // Check if peer was originally in session
        if !session.participants.contains(&request.peer_id) {
            println!("  ❌ Peer not in original participant list");
            return false;
        }

        // Check if rejoin is within reasonable time
        let elapsed = Instant::now().elapsed().as_secs() - session.started_at;
        if elapsed > 3600 { // 1 hour limit
            println!("  ❌ Session too old for rejoin");
            return false;
        }

        true
    }

    /// Authenticates a rejoining peer
    fn authenticate_peer(&self, peer_id: PeerId, auth_token: &str) -> bool {
        // In real implementation, would verify cryptographic signature
        // For simulation, just check token format
        if auth_token.len() < 10 {
            return false;
        }

        // Store authentication
        self.authenticated_peers.lock().unwrap().insert(peer_id, auth_token.to_string());
        true
    }

    /// Syncs a participant with current state
    pub async fn sync_participant(&self, peer_id: PeerId) {
        println!("  📥 Syncing participant {} with current state", peer_id);
        
        let session = self.session_state.lock().unwrap();
        println!("    • Session: {}", session.session_id);
        println!("    • Round: {}", session.current_round);
        println!("    • Messages: {}", session.message_count);
        
        // Remove from pending
        self.pending_rejoins.lock().unwrap().remove(&peer_id);
    }

    /// Records a message for recovery
    pub fn record_message(&self, from: PeerId, round: u8, msg_type: &str, data: Vec<u8>) {
        let message = MissedMessage {
            from,
            round,
            msg_type: msg_type.to_string(),
            data,
            timestamp: Instant::now().elapsed().as_secs(),
        };

        let mut buffers = self.message_buffers.lock().unwrap();
        
        // Add to all peer buffers except sender
        let session = self.session_state.lock().unwrap();
        for peer in &session.participants {
            if *peer != from {
                buffers.entry(*peer)
                    .or_insert_with(|| MessageBuffer::new(100))
                    .add_message(message.clone());
            }
        }

        // Increment message count
        let mut session = self.session_state.lock().unwrap();
        session.message_count += 1;
    }

    /// Gets missed messages for a peer
    fn get_missed_messages(&self, peer_id: PeerId, since_round: u8) -> Vec<MissedMessage> {
        let buffers = self.message_buffers.lock().unwrap();
        
        if let Some(buffer) = buffers.get(&peer_id) {
            buffer.get_messages_since(since_round)
        } else {
            Vec::new()
        }
    }

    /// Advances to next round
    pub fn advance_round(&self) {
        let mut session = self.session_state.lock().unwrap();
        session.current_round += 1;
        println!("  📝 Advanced to round {}", session.current_round);
    }

    /// Records a rejoin event
    fn record_rejoin_event(&self, peer_id: PeerId, success: bool, reason: &str) {
        let event = RejoinEvent {
            peer_id,
            timestamp: Instant::now(),
            success,
            reason: reason.to_string(),
        };

        self.rejoin_history.lock().unwrap().push(event);
    }

    /// Gets rejoin statistics
    pub fn get_rejoin_stats(&self) -> RejoinStats {
        let history = self.rejoin_history.lock().unwrap();
        
        let total_attempts = history.len();
        let successful = history.iter().filter(|e| e.success).count();
        let failed = total_attempts - successful;
        
        let peer_attempts: HashMap<PeerId, usize> = history.iter()
            .fold(HashMap::new(), |mut acc, event| {
                *acc.entry(event.peer_id).or_insert(0) += 1;
                acc
            });

        RejoinStats {
            total_attempts,
            successful_rejoins: successful,
            failed_rejoins: failed,
            peer_attempts,
        }
    }
}

/// Rejoin statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejoinStats {
    pub total_attempts: usize,
    pub successful_rejoins: usize,
    pub failed_rejoins: usize,
    pub peer_attempts: HashMap<PeerId, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rejoin_request_handling() {
        let coordinator = RejoinCoordinator::new(
            "test-session".to_string(),
            vec![1, 2, 3],
            2,
        );

        let request = RejoinRequest {
            peer_id: 2,
            session_id: "test-session".to_string(),
            last_round: 1,
            auth_token: "valid_token_123".to_string(),
            timestamp: 0,
        };

        let response = coordinator.handle_rejoin_request(request).await;
        assert!(response.accepted);
        assert!(response.session_state.is_some());
    }

    #[tokio::test]
    async fn test_invalid_rejoin() {
        let coordinator = RejoinCoordinator::new(
            "test-session".to_string(),
            vec![1, 2, 3],
            2,
        );

        let request = RejoinRequest {
            peer_id: 4, // Not in participant list
            session_id: "test-session".to_string(),
            last_round: 1,
            auth_token: "valid_token".to_string(),
            timestamp: 0,
        };

        let response = coordinator.handle_rejoin_request(request).await;
        assert!(!response.accepted);
        assert!(response.rejection_reason.is_some());
    }

}