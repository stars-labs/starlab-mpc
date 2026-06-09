//! Hybrid mode coordinator for managing mixed online/offline participants

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

/// Participant operational mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParticipantMode {
    /// Online via WebSocket/WebRTC
    Online,
    /// Offline via SD card exchange
    Offline,
}

/// Participant information
#[derive(Debug, Clone)]
pub struct ParticipantInfo {
    pub id: u16,
    pub name: String,
    pub mode: ParticipantMode,
    pub identifier: frost_secp256k1::Identifier,
}

/// Message type for hybrid coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HybridMessage {
    /// DKG Round 1 commitment
    DkgRound1(Vec<u8>),
    /// DKG Round 2 shares
    DkgRound2(Vec<u8>),
    /// Signing commitment
    SigningCommitment(Vec<u8>),
    /// Signature share
    SignatureShare(Vec<u8>),
    /// Transaction to sign
    Transaction(Vec<u8>),
}

/// Hybrid coordinator for managing mixed online/offline participants
pub struct HybridCoordinator {
    /// Participant information
    participants: HashMap<u16, ParticipantInfo>,
    
    /// Online message queue (simulated WebSocket/WebRTC)
    online_queue: Arc<Mutex<HashMap<u16, Vec<HybridMessage>>>>,
    
    /// Offline message storage (simulated SD card)
    offline_storage: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    
    /// Current round for coordination
    current_round: u8,
}

impl Default for HybridCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridCoordinator {
    /// Creates a new hybrid coordinator
    pub fn new() -> Self {
        Self {
            participants: HashMap::new(),
            online_queue: Arc::new(Mutex::new(HashMap::new())),
            offline_storage: Arc::new(Mutex::new(HashMap::new())),
            current_round: 0,
        }
    }
    
    /// Registers a participant
    pub fn register_participant(
        &mut self,
        id: u16,
        name: &str,
        mode: ParticipantMode,
    ) {
        let identifier = frost_secp256k1::Identifier::try_from(id)
            .expect("Invalid identifier");
        
        let info = ParticipantInfo {
            id,
            name: name.to_string(),
            mode: mode.clone(),
            identifier,
        };
        
        self.participants.insert(id, info);
        
        // Initialize online queue if online
        if mode == ParticipantMode::Online {
            self.online_queue.lock().unwrap().insert(id, Vec::new());
        }
    }
    
    /// Sends a message to a participant
    pub fn send_message(
        &self,
        from: u16,
        to: u16,
        message: HybridMessage,
    ) -> Result<(), String> {
        let participant = self.participants.get(&to)
            .ok_or_else(|| format!("Participant {} not found", to))?;
        
        match participant.mode {
            ParticipantMode::Online => {
                // Simulate WebSocket/WebRTC delivery
                println!("  📡 Sending message from P{} to P{} via WebSocket", from, to);
                let mut queue = self.online_queue.lock().unwrap();
                queue.entry(to).or_default().push(message);
                Ok(())
            }
            ParticipantMode::Offline => {
                // Simulate SD card export
                println!("  💾 Exporting message from P{} for P{} to SD card", from, to);
                let key = format!("msg_from_{}_to_{}_round_{}", from, to, self.current_round);
                let data = serde_json::to_vec(&message)
                    .map_err(|e| format!("Failed to serialize: {}", e))?;
                self.offline_storage.lock().unwrap().insert(key, data);
                Ok(())
            }
        }
    }
    
    /// Broadcasts a message to all participants
    pub fn broadcast_message(
        &self,
        from: u16,
        message: HybridMessage,
    ) -> Result<(), String> {
        let participants: Vec<u16> = self.participants.keys().copied().collect();
        
        for to in participants {
            if to != from {
                self.send_message(from, to, message.clone())?;
            }
        }
        
        Ok(())
    }
    
    /// Receives messages for a participant
    pub fn receive_messages(&self, participant_id: u16) -> Result<Vec<HybridMessage>, String> {
        let participant = self.participants.get(&participant_id)
            .ok_or_else(|| format!("Participant {} not found", participant_id))?;
        
        match participant.mode {
            ParticipantMode::Online => {
                // Get from online queue
                let mut queue = self.online_queue.lock().unwrap();
                let messages = queue.get_mut(&participant_id)
                    .map(|q| {
                        let msgs = q.clone();
                        q.clear(); // Clear after reading
                        msgs
                    })
                    .unwrap_or_default();
                
                if !messages.is_empty() {
                    println!("  📨 P{} received {} messages via WebSocket", 
                             participant_id, messages.len());
                }
                Ok(messages)
            }
            ParticipantMode::Offline => {
                // Import from SD card
                let storage = self.offline_storage.lock().unwrap();
                let mut messages = Vec::new();
                
                for (key, data) in storage.iter() {
                    if key.contains(&format!("to_{}_", participant_id)) {
                        let message: HybridMessage = serde_json::from_slice(data)
                            .map_err(|e| format!("Failed to deserialize: {}", e))?;
                        messages.push(message);
                    }
                }
                
                if !messages.is_empty() {
                    println!("  📥 P{} imported {} messages from SD card", 
                             participant_id, messages.len());
                }
                Ok(messages)
            }
        }
    }
    
    /// Simulates SD card exchange for offline participants
    pub fn perform_sd_card_exchange(&self) {
        println!("\n💾 Performing SD card exchange for offline participants...");
        
        // In real implementation, this would involve physical SD card handling
        // Here we just log the operation
        let offline_participants: Vec<_> = self.participants
            .values()
            .filter(|p| p.mode == ParticipantMode::Offline)
            .collect();
        
        for participant in offline_participants {
            println!("  📤 SD card prepared for {}", participant.name);
        }
        
        println!("  ✅ SD card exchange complete");
    }
    
    /// Advances to the next round
    pub fn advance_round(&mut self) {
        self.current_round += 1;
        println!("\n🔄 Advanced to round {}", self.current_round);
    }
    
    /// Gets participant information
    pub fn get_participant(&self, id: u16) -> Option<&ParticipantInfo> {
        self.participants.get(&id)
    }
    
    /// Gets all online participants
    pub fn get_online_participants(&self) -> Vec<&ParticipantInfo> {
        self.participants
            .values()
            .filter(|p| p.mode == ParticipantMode::Online)
            .collect()
    }
    
    /// Gets all offline participants
    pub fn get_offline_participants(&self) -> Vec<&ParticipantInfo> {
        self.participants
            .values()
            .filter(|p| p.mode == ParticipantMode::Offline)
            .collect()
    }
    
    /// Simulates network failure for online nodes
    pub fn simulate_network_failure(&mut self) {
        println!("\n⚠️ NETWORK FAILURE DETECTED!");
        
        // Convert all online nodes to offline
        for participant in self.participants.values_mut() {
            if participant.mode == ParticipantMode::Online {
                println!("  🔌 {} switching to offline mode", participant.name);
                participant.mode = ParticipantMode::Offline;
            }
        }
        
        println!("  ✅ All nodes now operating in offline mode");
    }
    
    /// Restores network connectivity
    pub fn restore_network(&mut self, participant_ids: Vec<u16>) {
        println!("\n🌐 Restoring network connectivity...");
        
        for id in participant_ids {
            if let Some(participant) = self.participants.get_mut(&id)
                && participant.mode == ParticipantMode::Offline {
                    println!("  ✅ {} back online", participant.name);
                    participant.mode = ParticipantMode::Online;
                    self.online_queue.lock().unwrap().insert(id, Vec::new());
                }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hybrid_coordinator() {
        let mut coordinator = HybridCoordinator::new();
        
        // Register participants
        coordinator.register_participant(1, "Alice", ParticipantMode::Online);
        coordinator.register_participant(2, "Bob", ParticipantMode::Online);
        coordinator.register_participant(3, "Charlie", ParticipantMode::Offline);
        
        // Test message sending
        let message = HybridMessage::DkgRound1(vec![1, 2, 3]);
        coordinator.send_message(1, 2, message.clone()).unwrap();
        coordinator.send_message(1, 3, message).unwrap();
        
        // Test message receiving
        let messages = coordinator.receive_messages(2).unwrap();
        assert_eq!(messages.len(), 1);
        
        let messages = coordinator.receive_messages(3).unwrap();
        assert_eq!(messages.len(), 1);
    }
    
    #[test]
    fn test_network_failure() {
        let mut coordinator = HybridCoordinator::new();
        
        coordinator.register_participant(1, "Alice", ParticipantMode::Online);
        coordinator.register_participant(2, "Bob", ParticipantMode::Online);
        
        // Simulate network failure
        coordinator.simulate_network_failure();
        
        // Check all are offline
        assert!(coordinator.get_online_participants().is_empty());
        assert_eq!(coordinator.get_offline_participants().len(), 2);
        
        // Restore network
        coordinator.restore_network(vec![1, 2]);
        assert_eq!(coordinator.get_online_participants().len(), 2);
    }
}