//! Transport layer for hybrid online/offline communication

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use super::coordinator::HybridMessage;

/// Online transport simulating WebSocket/WebRTC
pub struct OnlineTransport {
    /// Simulated WebSocket connections
    connections: Arc<Mutex<HashMap<u16, bool>>>,
    
    /// Message buffer
    buffer: Arc<Mutex<HashMap<u16, Vec<Vec<u8>>>>>,
    
    /// Network latency simulation (ms)
    latency: u64,
}

impl OnlineTransport {
    /// Creates a new online transport
    pub fn new(latency: u64) -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            buffer: Arc::new(Mutex::new(HashMap::new())),
            latency,
        }
    }
    
    /// Establishes a connection
    pub fn connect(&self, participant_id: u16) {
        self.connections.lock().unwrap().insert(participant_id, true);
        println!("  🔗 P{} connected via WebSocket", participant_id);
    }
    
    /// Disconnects a participant
    pub fn disconnect(&self, participant_id: u16) {
        self.connections.lock().unwrap().insert(participant_id, false);
        println!("  ❌ P{} disconnected", participant_id);
    }
    
    /// Checks if participant is connected
    pub fn is_connected(&self, participant_id: u16) -> bool {
        self.connections
            .lock()
            .unwrap()
            .get(&participant_id)
            .copied()
            .unwrap_or(false)
    }
    
    /// Sends data to a participant
    pub fn send(&self, to: u16, data: Vec<u8>) -> Result<(), String> {
        if !self.is_connected(to) {
            return Err(format!("P{} is not connected", to));
        }
        
        // Simulate network latency
        if self.latency > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.latency));
        }
        
        self.buffer
            .lock()
            .unwrap()
            .entry(to)
            .or_default()
            .push(data);
        
        Ok(())
    }
    
    /// Receives data for a participant
    pub fn receive(&self, participant_id: u16) -> Vec<Vec<u8>> {
        self.buffer
            .lock()
            .unwrap()
            .remove(&participant_id)
            .unwrap_or_default()
    }
}

/// Offline transport simulating SD card exchange
pub struct OfflineTransport {
    /// Simulated SD card storage
    sd_card: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    
    /// Current SD card location (which participant has it)
    sd_card_holder: Arc<Mutex<Option<u16>>>,
}

impl Default for OfflineTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl OfflineTransport {
    /// Creates a new offline transport
    pub fn new() -> Self {
        Self {
            sd_card: Arc::new(Mutex::new(HashMap::new())),
            sd_card_holder: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Exports data to SD card
    pub fn export_to_sd(
        &self,
        from: u16,
        filename: &str,
        data: Vec<u8>,
    ) -> Result<(), String> {
        println!("  💾 P{} exporting to SD card: {}", from, filename);
        
        self.sd_card
            .lock()
            .unwrap()
            .insert(filename.to_string(), data);
        
        *self.sd_card_holder.lock().unwrap() = Some(from);
        
        Ok(())
    }
    
    /// Imports data from SD card
    pub fn import_from_sd(
        &self,
        to: u16,
        filename: &str,
    ) -> Result<Vec<u8>, String> {
        println!("  📥 P{} importing from SD card: {}", to, filename);
        
        let data = self.sd_card
            .lock()
            .unwrap()
            .get(filename)
            .cloned()
            .ok_or_else(|| format!("File {} not found on SD card", filename))?;
        
        *self.sd_card_holder.lock().unwrap() = Some(to);
        
        Ok(data)
    }
    
    /// Transfers SD card to another participant
    pub fn transfer_sd_card(&self, from: u16, to: u16) {
        println!("  🤝 Transferring SD card from P{} to P{}", from, to);
        *self.sd_card_holder.lock().unwrap() = Some(to);
        
        // Simulate physical transfer time
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    
    /// Lists files on SD card
    pub fn list_sd_files(&self) -> Vec<String> {
        self.sd_card
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }
    
    /// Clears SD card
    pub fn clear_sd_card(&self) {
        println!("  🗑️ Clearing SD card");
        self.sd_card.lock().unwrap().clear();
    }
    
    /// Gets current SD card holder
    pub fn get_sd_holder(&self) -> Option<u16> {
        *self.sd_card_holder.lock().unwrap()
    }
}

/// Combined transport for hybrid operations
pub struct HybridTransport {
    pub online: OnlineTransport,
    pub offline: OfflineTransport,
}

impl HybridTransport {
    /// Creates a new hybrid transport
    pub fn new(network_latency: u64) -> Self {
        Self {
            online: OnlineTransport::new(network_latency),
            offline: OfflineTransport::new(),
        }
    }
    
    /// Bridges a message from online to offline
    pub fn bridge_to_offline(
        &self,
        message: &HybridMessage,
        filename: &str,
    ) -> Result<(), String> {
        println!("  🌉 Bridging message from online to offline");
        
        let data = serde_json::to_vec(message)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        
        // Export to SD card for offline participant
        self.offline.export_to_sd(0, filename, data)?;
        
        Ok(())
    }
    
    /// Bridges a message from offline to online
    pub fn bridge_to_online(
        &self,
        filename: &str,
        to: u16,
    ) -> Result<(), String> {
        println!("  🌉 Bridging message from offline to online");
        
        // Import from SD card
        let data = self.offline.import_from_sd(0, filename)?;
        
        // Send via online transport
        self.online.send(to, data)?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_online_transport() {
        let transport = OnlineTransport::new(0);
        
        transport.connect(1);
        transport.connect(2);
        
        assert!(transport.is_connected(1));
        assert!(transport.is_connected(2));
        
        transport.send(1, vec![1, 2, 3]).unwrap();
        let received = transport.receive(1);
        assert_eq!(received.len(), 1);
        assert_eq!(received[0], vec![1, 2, 3]);
    }
    
    #[test]
    fn test_offline_transport() {
        let transport = OfflineTransport::new();
        
        transport.export_to_sd(1, "test.dat", vec![4, 5, 6]).unwrap();
        assert_eq!(transport.get_sd_holder(), Some(1));
        
        transport.transfer_sd_card(1, 2);
        assert_eq!(transport.get_sd_holder(), Some(2));
        
        let data = transport.import_from_sd(2, "test.dat").unwrap();
        assert_eq!(data, vec![4, 5, 6]);
    }
    
    #[test]
    fn test_hybrid_transport() {
        let transport = HybridTransport::new(0);
        
        let message = HybridMessage::DkgRound1(vec![7, 8, 9]);
        
        // Bridge from online to offline
        transport.bridge_to_offline(&message, "bridge.dat").unwrap();
        
        // Bridge back from offline to online
        transport.online.connect(3);
        transport.bridge_to_online("bridge.dat", 3).unwrap();
        
        let received = transport.online.receive(3);
        assert_eq!(received.len(), 1);
    }
}