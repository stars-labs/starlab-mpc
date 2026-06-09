//! Connection management for WebSocket and WebRTC

use super::{ConnectionInfo, ConnectionStatus, CoreError, CoreResult, CoreState, UICallback};
use std::sync::Arc;
use tracing::{info, warn};

/// Connection manager handles WebSocket and WebRTC connections
pub struct ConnectionManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
}

impl ConnectionManager {
    pub fn new(state: Arc<CoreState>, ui_callback: Arc<dyn UICallback>) -> Self {
        Self { state, ui_callback }
    }
    
    /// Connect to WebSocket server
    pub async fn connect_websocket(&self, url: String) -> CoreResult<()> {
        info!("Connecting to WebSocket server: {}", url);
        
        // Update state
        *self.state.websocket_connected.lock().await = false;
        self.ui_callback.update_connection_status(false, false).await;
        
        // Simulate connection (in real implementation, this would connect to actual WebSocket)
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Update state on success
        *self.state.websocket_connected.lock().await = true;
        
        // Update UI
        self.ui_callback.update_connection_status(true, false).await;
        self.ui_callback.show_message(
            format!("Connected to WebSocket server: {}", url),
            false
        ).await;
        
        Ok(())
    }
    
    /// Disconnect from WebSocket server
    pub async fn disconnect_websocket(&self) -> CoreResult<()> {
        info!("Disconnecting from WebSocket server");
        
        *self.state.websocket_connected.lock().await = false;
        *self.state.webrtc_connected.lock().await = false;
        
        // Clear mesh connections
        self.state.mesh_connections.lock().await.clear();
        
        // Update UI
        self.ui_callback.update_connection_status(false, false).await;
        self.ui_callback.update_mesh_connections(Vec::new()).await;
        
        self.ui_callback.show_message(
            "Disconnected from WebSocket server".to_string(),
            false
        ).await;
        
        Ok(())
    }
    
    /// Connect to WebRTC peer
    pub async fn connect_webrtc_peer(&self, peer_id: String) -> CoreResult<()> {
        info!("Connecting to WebRTC peer: {}", peer_id);
        
        // Check WebSocket connection first
        if !*self.state.websocket_connected.lock().await {
            return Err(CoreError::Network("WebSocket not connected".to_string()));
        }
        
        // Create connection info
        let connection = ConnectionInfo {
            peer_id: peer_id.clone(),
            status: ConnectionStatus::Connecting,
            latency_ms: 0,
            quality: 0.0,
        };
        
        // Add to mesh connections
        self.state.mesh_connections.lock().await.push(connection);
        
        // Update UI
        self.ui_callback.update_mesh_connections(
            self.state.mesh_connections.lock().await.clone()
        ).await;
        
        // Simulate connection establishment
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        
        // Update connection status
        let mut connections = self.state.mesh_connections.lock().await;
        if let Some(conn) = connections.iter_mut().find(|c| c.peer_id == peer_id) {
            conn.status = ConnectionStatus::Connected;
            conn.latency_ms = 25;
            conn.quality = 0.95;
        }
        let connections_clone = connections.clone();
        drop(connections);
        
        // Update WebRTC connected status if we have any connected peers
        let has_peers = connections_clone.iter()
            .any(|c| c.status == ConnectionStatus::Connected);
        *self.state.webrtc_connected.lock().await = has_peers;
        
        // Update UI
        self.ui_callback.update_connection_status(
            *self.state.websocket_connected.lock().await,
            has_peers
        ).await;
        self.ui_callback.update_mesh_connections(connections_clone).await;
        
        self.ui_callback.show_message(
            format!("Connected to peer: {}", peer_id),
            false
        ).await;
        
        Ok(())
    }
    
    /// Disconnect from WebRTC peer
    pub async fn disconnect_webrtc_peer(&self, peer_id: String) -> CoreResult<()> {
        info!("Disconnecting from WebRTC peer: {}", peer_id);
        
        // Remove from mesh connections
        self.state.mesh_connections.lock().await.retain(|c| c.peer_id != peer_id);
        
        // Update WebRTC connected status
        let connections = self.state.mesh_connections.lock().await;
        let has_peers = connections.iter()
            .any(|c| c.status == ConnectionStatus::Connected);
        drop(connections);
        
        *self.state.webrtc_connected.lock().await = has_peers;
        
        // Update UI
        self.ui_callback.update_connection_status(
            *self.state.websocket_connected.lock().await,
            has_peers
        ).await;
        self.ui_callback.update_mesh_connections(
            self.state.mesh_connections.lock().await.clone()
        ).await;
        
        self.ui_callback.show_message(
            format!("Disconnected from peer: {}", peer_id),
            false
        ).await;
        
        Ok(())
    }
    
    /// Handle peer connection failure
    pub async fn handle_peer_failure(&self, peer_id: String) -> CoreResult<()> {
        warn!("Peer connection failed: {}", peer_id);
        
        let mut connections = self.state.mesh_connections.lock().await;
        if let Some(conn) = connections.iter_mut().find(|c| c.peer_id == peer_id) {
            conn.status = ConnectionStatus::Failed;
            conn.quality = 0.0;
        }
        let connections_clone = connections.clone();
        drop(connections);
        
        // Update UI
        self.ui_callback.update_mesh_connections(connections_clone).await;
        
        self.ui_callback.show_message(
            format!("Connection to peer {} failed", peer_id),
            true
        ).await;
        
        Ok(())
    }
    
    /// Reconnect to peer
    pub async fn reconnect_peer(&self, peer_id: String) -> CoreResult<()> {
        info!("Attempting to reconnect to peer: {}", peer_id);
        
        // Update status to connecting
        let mut connections = self.state.mesh_connections.lock().await;
        if let Some(conn) = connections.iter_mut().find(|c| c.peer_id == peer_id) {
            conn.status = ConnectionStatus::Connecting;
        }
        let connections_clone = connections.clone();
        drop(connections);
        
        self.ui_callback.update_mesh_connections(connections_clone).await;
        
        // Attempt reconnection
        self.connect_webrtc_peer(peer_id).await
    }
    
    /// Update peer latency
    pub async fn update_peer_latency(&self, peer_id: String, latency_ms: u32) -> CoreResult<()> {
        let mut connections = self.state.mesh_connections.lock().await;
        if let Some(conn) = connections.iter_mut().find(|c| c.peer_id == peer_id) {
            conn.latency_ms = latency_ms;
            
            // Update quality based on latency
            conn.quality = match latency_ms {
                0..=50 => 1.0,
                51..=100 => 0.9,
                101..=200 => 0.7,
                201..=500 => 0.5,
                _ => 0.3,
            };
        }
        let connections_clone = connections.clone();
        drop(connections);
        
        self.ui_callback.update_mesh_connections(connections_clone).await;
        
        Ok(())
    }
    
    /// Get all mesh connections
    pub async fn get_mesh_connections(&self) -> Vec<ConnectionInfo> {
        self.state.mesh_connections.lock().await.clone()
    }
    
    /// Get connection status
    pub async fn get_connection_status(&self) -> (bool, bool) {
        (
            *self.state.websocket_connected.lock().await,
            *self.state.webrtc_connected.lock().await,
        )
    }
    
    /// Check if connected to specific peer
    pub async fn is_connected_to_peer(&self, peer_id: &str) -> bool {
        self.state.mesh_connections.lock().await
            .iter()
            .any(|c| c.peer_id == peer_id && c.status == ConnectionStatus::Connected)
    }
    
    /// Get connected peer count
    pub async fn get_connected_peer_count(&self) -> usize {
        self.state.mesh_connections.lock().await
            .iter()
            .filter(|c| c.status == ConnectionStatus::Connected)
            .count()
    }
}