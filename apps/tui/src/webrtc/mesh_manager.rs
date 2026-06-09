//! WebRTC mesh network manager for establishing and maintaining P2P connections

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

/// Peer identifier
pub type PeerId = u16;

/// Connection state for a peer
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Connecting (exchanging SDP/ICE)
    Connecting,
    /// Connected and ready
    Connected,
    /// Connection failed
    Failed(String),
    /// Reconnecting after failure
    Reconnecting,
}

/// WebRTC data channel
#[derive(Debug, Clone)]
pub struct RTCDataChannel {
    pub id: String,
    pub label: String,
    pub ordered: bool,
    pub reliable: bool,
    pub state: ConnectionState,
}

/// Simulated WebRTC peer connection
#[derive(Debug, Clone)]
pub struct RTCPeerConnection {
    pub remote_peer: PeerId,
    pub state: ConnectionState,
    pub ice_connection_state: String,
    pub signaling_state: String,
    pub data_channels: Vec<RTCDataChannel>,
    pub created_at: Instant,
    pub last_activity: Instant,
}

impl RTCPeerConnection {
    pub fn new(remote_peer: PeerId) -> Self {
        Self {
            remote_peer,
            state: ConnectionState::Disconnected,
            ice_connection_state: "new".to_string(),
            signaling_state: "stable".to_string(),
            data_channels: Vec::new(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
        }
    }

    pub fn create_data_channel(&mut self, label: &str, ordered: bool, reliable: bool) -> RTCDataChannel {
        let channel = RTCDataChannel {
            id: format!("{}_{}", self.remote_peer, label),
            label: label.to_string(),
            ordered,
            reliable,
            state: ConnectionState::Connecting,
        };
        self.data_channels.push(channel.clone());
        channel
    }
}

/// Mesh topology representation
#[derive(Debug, Clone)]
pub struct MeshTopology {
    /// Number of participants
    pub total_peers: usize,
    /// Minimum peers for threshold
    pub threshold: usize,
    /// Current connections (adjacency list)
    pub connections: HashMap<PeerId, HashSet<PeerId>>,
}

impl MeshTopology {
    pub fn new(total_peers: usize, threshold: usize) -> Self {
        Self {
            total_peers,
            threshold,
            connections: HashMap::new(),
        }
    }

    pub fn add_connection(&mut self, peer1: PeerId, peer2: PeerId) {
        self.connections.entry(peer1).or_default().insert(peer2);
        self.connections.entry(peer2).or_default().insert(peer1);
    }

    pub fn remove_connection(&mut self, peer1: PeerId, peer2: PeerId) {
        if let Some(connections) = self.connections.get_mut(&peer1) {
            connections.remove(&peer2);
        }
        if let Some(connections) = self.connections.get_mut(&peer2) {
            connections.remove(&peer1);
        }
    }

    pub fn is_fully_connected(&self) -> bool {
        if self.connections.len() != self.total_peers {
            return false;
        }
        
        for connections in self.connections.values() {
            if connections.len() != self.total_peers - 1 {
                return false;
            }
        }
        true
    }

    pub fn get_connected_peers(&self, peer: PeerId) -> Vec<PeerId> {
        self.connections.get(&peer)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn meets_threshold(&self) -> bool {
        let connected_count = self.connections.iter()
            .filter(|(_, conns)| !conns.is_empty())
            .count();
        connected_count >= self.threshold
    }
}

/// WebRTC mesh network manager
pub struct WebRTCMeshManager {
    /// Local peer ID
    pub local_peer: PeerId,
    /// Active connections
    pub connections: Arc<Mutex<HashMap<PeerId, RTCPeerConnection>>>,
    /// Data channels
    pub data_channels: Arc<Mutex<HashMap<PeerId, RTCDataChannel>>>,
    /// Connection states
    pub connection_states: Arc<Mutex<HashMap<PeerId, ConnectionState>>>,
    /// Mesh topology
    pub mesh_topology: Arc<Mutex<MeshTopology>>,
    /// Message buffer for offline peers
    pub message_buffer: Arc<Mutex<HashMap<PeerId, Vec<Vec<u8>>>>>,
}

impl WebRTCMeshManager {
    /// Creates a new mesh manager
    pub fn new(local_peer: PeerId, total_peers: usize, threshold: usize) -> Self {
        Self {
            local_peer,
            connections: Arc::new(Mutex::new(HashMap::new())),
            data_channels: Arc::new(Mutex::new(HashMap::new())),
            connection_states: Arc::new(Mutex::new(HashMap::new())),
            mesh_topology: Arc::new(Mutex::new(MeshTopology::new(total_peers, threshold))),
            message_buffer: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Establishes the mesh network
    pub async fn establish_mesh(&mut self, peers: Vec<PeerId>) -> Result<(), String> {
        println!("🌐 Establishing WebRTC mesh for peer {}", self.local_peer);
        
        for peer in peers {
            if peer != self.local_peer {
                self.connect_to_peer(peer).await?;
            }
        }
        
        // Wait for all connections to establish
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let topology = self.mesh_topology.lock().unwrap();
        if topology.is_fully_connected() {
            println!("✅ Full mesh established!");
            Ok(())
        } else {
            println!("⚠️ Partial mesh established");
            Ok(())
        }
    }

    /// Connects to a specific peer
    async fn connect_to_peer(&mut self, peer: PeerId) -> Result<(), String> {
        println!("  📡 Connecting {} → {}", self.local_peer, peer);
        
        // Create peer connection
        let mut connection = RTCPeerConnection::new(peer);
        connection.state = ConnectionState::Connecting;
        
        // Simulate SDP exchange
        connection.signaling_state = "have-local-offer".to_string();
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        connection.signaling_state = "have-remote-answer".to_string();
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Simulate ICE gathering
        connection.ice_connection_state = "checking".to_string();
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        connection.ice_connection_state = "connected".to_string();
        connection.state = ConnectionState::Connected;
        
        // Create data channels
        let _reliable = connection.create_data_channel("reliable", true, true);
        let _unreliable = connection.create_data_channel("unreliable", false, false);
        
        // Update connection state
        connection.last_activity = Instant::now();
        
        // Store connection
        self.connections.lock().unwrap().insert(peer, connection.clone());
        self.connection_states.lock().unwrap().insert(peer, ConnectionState::Connected);
        
        // Update topology
        self.mesh_topology.lock().unwrap().add_connection(self.local_peer, peer);
        
        println!("  ✅ Connected {} ↔ {}", self.local_peer, peer);
        Ok(())
    }

    /// Handles peer disconnection
    pub async fn handle_peer_disconnect(&mut self, peer: PeerId) {
        println!("  🔌 Peer {} disconnected from {}", peer, self.local_peer);
        
        // Update connection state
        self.connection_states.lock().unwrap().insert(peer, ConnectionState::Disconnected);
        
        // Remove from topology
        self.mesh_topology.lock().unwrap().remove_connection(self.local_peer, peer);
        
        // Buffer messages for this peer
        self.message_buffer.lock().unwrap().entry(peer).or_default();
    }

    /// Handles peer rejoin
    pub async fn handle_peer_rejoin(&mut self, peer: PeerId) -> Result<(), String> {
        println!("  🔄 Peer {} rejoining mesh with {}", peer, self.local_peer);
        
        // Mark as reconnecting
        self.connection_states.lock().unwrap().insert(peer, ConnectionState::Reconnecting);
        
        // Re-establish connection
        self.connect_to_peer(peer).await?;
        
        // Send buffered messages
        let mut buffer = self.message_buffer.lock().unwrap();
        if let Some(messages) = buffer.remove(&peer) {
            println!("  📤 Sending {} buffered messages to {}", messages.len(), peer);
            // In real implementation, would send these messages
        }
        
        Ok(())
    }

    /// Gets list of connected peers
    pub fn get_connected_peers(&self) -> Vec<PeerId> {
        self.connection_states.lock().unwrap()
            .iter()
            .filter(|(_, state)| **state == ConnectionState::Connected)
            .map(|(peer, _)| *peer)
            .collect()
    }

    /// Checks if threshold is met
    pub fn is_threshold_met(&self) -> bool {
        self.mesh_topology.lock().unwrap().meets_threshold()
    }

    /// Sends a message to a peer
    pub fn send_message(&self, to: PeerId, message: Vec<u8>) -> Result<(), String> {
        let states = self.connection_states.lock().unwrap();
        
        match states.get(&to) {
            Some(ConnectionState::Connected) => {
                println!("  📨 Sending message from {} to {}", self.local_peer, to);
                Ok(())
            }
            Some(ConnectionState::Disconnected) | Some(ConnectionState::Failed(_)) => {
                // Buffer the message
                let mut buffer = self.message_buffer.lock().unwrap();
                buffer.entry(to).or_default().push(message);
                println!("  💾 Buffered message for offline peer {}", to);
                Ok(())
            }
            _ => Err(format!("Peer {} not found or connecting", to))
        }
    }

    /// Broadcasts a message to all connected peers
    pub fn broadcast_message(&self, message: Vec<u8>) -> Result<(), String> {
        let peers = self.get_connected_peers();
        println!("  📢 Broadcasting from {} to {} peers", self.local_peer, peers.len());
        
        for peer in peers {
            self.send_message(peer, message.clone())?;
        }
        Ok(())
    }

    /// Simulates network failure for this peer
    pub fn simulate_network_failure(&mut self) {
        println!("  ⚠️ Network failure for peer {}", self.local_peer);
        
        let peers: Vec<PeerId> = self.connections.lock().unwrap().keys().copied().collect();
        
        for peer in peers {
            self.connection_states.lock().unwrap().insert(peer, ConnectionState::Failed("Network failure".to_string()));
            self.mesh_topology.lock().unwrap().remove_connection(self.local_peer, peer);
        }
    }

    /// Gets mesh statistics
    pub fn get_mesh_stats(&self) -> MeshStats {
        let states = self.connection_states.lock().unwrap();
        let topology = self.mesh_topology.lock().unwrap();
        
        MeshStats {
            total_peers: topology.total_peers,
            connected_peers: states.iter().filter(|(_, s)| **s == ConnectionState::Connected).count(),
            disconnected_peers: states.iter().filter(|(_, s)| **s == ConnectionState::Disconnected).count(),
            failed_peers: states.iter().filter(|(_, s)| matches!(s, ConnectionState::Failed(_))).count(),
            is_fully_connected: topology.is_fully_connected(),
            meets_threshold: topology.meets_threshold(),
        }
    }
}

/// Mesh network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshStats {
    pub total_peers: usize,
    pub connected_peers: usize,
    pub disconnected_peers: usize,
    pub failed_peers: usize,
    pub is_fully_connected: bool,
    pub meets_threshold: bool,
}

// Use tokio for async runtime in tests
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mesh_establishment() {
        let mut manager = WebRTCMeshManager::new(1, 3, 2);
        let result = manager.establish_mesh(vec![1, 2, 3]).await;
        assert!(result.is_ok());
        
        let peers = manager.get_connected_peers();
        assert_eq!(peers.len(), 2);
    }

    #[tokio::test]
    async fn test_peer_disconnect_and_rejoin() {
        let mut manager = WebRTCMeshManager::new(1, 3, 2);
        manager.establish_mesh(vec![1, 2, 3]).await.unwrap();
        
        // Disconnect peer 2
        manager.handle_peer_disconnect(2).await;
        assert_eq!(manager.get_connected_peers().len(), 1);
        
        // Rejoin peer 2
        manager.handle_peer_rejoin(2).await.unwrap();
        assert_eq!(manager.get_connected_peers().len(), 2);
    }
}