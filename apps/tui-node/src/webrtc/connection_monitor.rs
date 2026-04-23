//! Connection monitoring with heartbeat and quality metrics

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

use super::mesh_manager::PeerId;

/// Connection quality metrics
#[derive(Debug, Clone)]
pub struct ConnectionQuality {
    /// Round-trip latency in milliseconds
    pub latency_ms: u32,
    /// Packet loss rate (0.0 to 1.0)
    pub packet_loss_rate: f32,
    /// Available bandwidth in kbps
    pub bandwidth_kbps: u32,
    /// Last heartbeat received
    pub last_heartbeat: Instant,
    /// Connection score (0-100)
    pub score: u8,
}

impl Default for ConnectionQuality {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionQuality {
    pub fn new() -> Self {
        Self {
            latency_ms: 0,
            packet_loss_rate: 0.0,
            bandwidth_kbps: 10000,
            last_heartbeat: Instant::now(),
            score: 100,
        }
    }

    pub fn calculate_score(&mut self) {
        // Calculate score based on metrics
        let latency_score = if self.latency_ms < 50 { 100 }
            else if self.latency_ms < 150 { 80 }
            else if self.latency_ms < 300 { 60 }
            else if self.latency_ms < 500 { 40 }
            else { 20 };
        
        let loss_score = ((1.0 - self.packet_loss_rate) * 100.0) as u8;
        
        let bandwidth_score = if self.bandwidth_kbps > 5000 { 100 }
            else if self.bandwidth_kbps > 1000 { 80 }
            else if self.bandwidth_kbps > 500 { 60 }
            else if self.bandwidth_kbps > 100 { 40 }
            else { 20 };
        
        // Weighted average
        self.score = ((latency_score as u32 * 40 + loss_score as u32 * 40 + bandwidth_score as u32 * 20) / 100) as u8;
    }

    pub fn is_healthy(&self) -> bool {
        self.score >= 60 && 
        self.last_heartbeat.elapsed() < Duration::from_secs(10)
    }
}

/// Heartbeat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub from: PeerId,
    pub to: PeerId,
    pub sequence: u64,
    pub timestamp: u64,
}

/// Connection monitor for tracking peer health
pub struct ConnectionMonitor {
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Timeout threshold for considering peer dead
    pub timeout_threshold: Duration,
    /// Connection quality metrics per peer
    pub quality_metrics: Arc<Mutex<HashMap<PeerId, ConnectionQuality>>>,
    /// Heartbeat sequence numbers
    pub heartbeat_sequences: Arc<Mutex<HashMap<PeerId, u64>>>,
    /// Pending heartbeats (for RTT calculation)
    pub pending_heartbeats: Arc<Mutex<HashMap<(PeerId, u64), Instant>>>,
}

impl Default for ConnectionMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionMonitor {
    /// Creates a new connection monitor
    pub fn new() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(5),
            timeout_threshold: Duration::from_secs(15),
            quality_metrics: Arc::new(Mutex::new(HashMap::new())),
            heartbeat_sequences: Arc::new(Mutex::new(HashMap::new())),
            pending_heartbeats: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Starts monitoring a peer
    pub fn start_monitoring(&self, peer: PeerId) {
        let mut metrics = self.quality_metrics.lock().unwrap();
        metrics.insert(peer, ConnectionQuality::new());
        
        let mut sequences = self.heartbeat_sequences.lock().unwrap();
        sequences.insert(peer, 0);
        
        println!("  🔍 Started monitoring peer {}", peer);
    }

    /// Sends a heartbeat to a peer
    pub fn send_heartbeat(&self, from: PeerId, to: PeerId) -> Heartbeat {
        let mut sequences = self.heartbeat_sequences.lock().unwrap();
        let seq = sequences.entry(to).and_modify(|s| *s += 1).or_insert(1);
        
        let heartbeat = Heartbeat {
            from,
            to,
            sequence: *seq,
            timestamp: Instant::now().elapsed().as_millis() as u64,
        };
        
        // Track pending heartbeat for RTT calculation
        let mut pending = self.pending_heartbeats.lock().unwrap();
        pending.insert((to, heartbeat.sequence), Instant::now());
        
        heartbeat
    }

    /// Handles received heartbeat response
    pub fn handle_heartbeat_response(&self, from: PeerId, sequence: u64) {
        let mut pending = self.pending_heartbeats.lock().unwrap();
        
        if let Some(sent_time) = pending.remove(&(from, sequence)) {
            let rtt = sent_time.elapsed().as_millis() as u32;
            
            let mut metrics = self.quality_metrics.lock().unwrap();
            if let Some(quality) = metrics.get_mut(&from) {
                // Update latency (moving average)
                quality.latency_ms = (quality.latency_ms * 7 + rtt * 3) / 10;
                quality.last_heartbeat = Instant::now();
                quality.calculate_score();
            }
        }
    }

    /// Updates packet loss rate for a peer
    pub fn update_packet_loss(&self, peer: PeerId, loss_rate: f32) {
        let mut metrics = self.quality_metrics.lock().unwrap();
        if let Some(quality) = metrics.get_mut(&peer) {
            quality.packet_loss_rate = loss_rate;
            quality.calculate_score();
        }
    }

    /// Updates bandwidth estimate for a peer
    pub fn update_bandwidth(&self, peer: PeerId, bandwidth_kbps: u32) {
        let mut metrics = self.quality_metrics.lock().unwrap();
        if let Some(quality) = metrics.get_mut(&peer) {
            quality.bandwidth_kbps = bandwidth_kbps;
            quality.calculate_score();
        }
    }

    /// Simulates degraded connection quality
    pub fn degrade_connection(&self, peer: PeerId, latency: u32, loss: f32) {
        let mut metrics = self.quality_metrics.lock().unwrap();
        if let Some(quality) = metrics.get_mut(&peer) {
            quality.latency_ms = latency;
            quality.packet_loss_rate = loss;
            quality.calculate_score();
            
            println!("  ⚠️ Degraded connection to {}: {}ms latency, {:.1}% loss", 
                     peer, latency, loss * 100.0);
        }
    }

    /// Restores connection quality
    pub fn restore_connection(&self, peer: PeerId) {
        let mut metrics = self.quality_metrics.lock().unwrap();
        if let Some(quality) = metrics.get_mut(&peer) {
            quality.latency_ms = 30;
            quality.packet_loss_rate = 0.0;
            quality.bandwidth_kbps = 10000;
            quality.calculate_score();
            
            println!("  ✅ Restored connection to {}", peer);
        }
    }

    /// Checks for dead peers
    pub fn check_dead_peers(&self) -> Vec<PeerId> {
        let metrics = self.quality_metrics.lock().unwrap();
        let mut dead_peers = Vec::new();
        
        for (peer, quality) in metrics.iter() {
            if quality.last_heartbeat.elapsed() > self.timeout_threshold {
                dead_peers.push(*peer);
            }
        }
        
        dead_peers
    }

    /// Gets connection quality for a peer
    pub fn get_quality(&self, peer: PeerId) -> Option<ConnectionQuality> {
        self.quality_metrics.lock().unwrap().get(&peer).cloned()
    }

    /// Gets all healthy peers
    pub fn get_healthy_peers(&self) -> Vec<PeerId> {
        let metrics = self.quality_metrics.lock().unwrap();
        metrics.iter()
            .filter(|(_, quality)| quality.is_healthy())
            .map(|(peer, _)| *peer)
            .collect()
    }

    /// Gets connection statistics
    pub fn get_stats(&self) -> ConnectionStats {
        let metrics = self.quality_metrics.lock().unwrap();
        
        let healthy_count = metrics.values().filter(|q| q.is_healthy()).count();
        let degraded_count = metrics.values().filter(|q| q.score < 60 && q.score > 0).count();
        let dead_count = metrics.values()
            .filter(|q| q.last_heartbeat.elapsed() > self.timeout_threshold)
            .count();
        
        let avg_latency = if !metrics.is_empty() {
            metrics.values().map(|q| q.latency_ms).sum::<u32>() / metrics.len() as u32
        } else { 0 };
        
        let avg_loss = if !metrics.is_empty() {
            metrics.values().map(|q| q.packet_loss_rate).sum::<f32>() / metrics.len() as f32
        } else { 0.0 };
        
        ConnectionStats {
            total_peers: metrics.len(),
            healthy_peers: healthy_count,
            degraded_peers: degraded_count,
            dead_peers: dead_count,
            average_latency_ms: avg_latency,
            average_packet_loss: avg_loss,
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub total_peers: usize,
    pub healthy_peers: usize,
    pub degraded_peers: usize,
    pub dead_peers: usize,
    pub average_latency_ms: u32,
    pub average_packet_loss: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_monitoring() {
        let monitor = ConnectionMonitor::new();
        
        monitor.start_monitoring(2);
        
        // Send heartbeat
        let hb = monitor.send_heartbeat(1, 2);
        assert_eq!(hb.sequence, 1);
        
        // Simulate response
        monitor.handle_heartbeat_response(2, 1);
        
        let quality = monitor.get_quality(2).unwrap();
        assert!(quality.is_healthy());
    }

}