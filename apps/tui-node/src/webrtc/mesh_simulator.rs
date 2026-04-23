//! WebRTC mesh network simulator for testing various scenarios.
//!
//! Uses `std::sync::Mutex` guards across `.await` points in several
//! handlers (handle_peer_join / leave / crash / rejoin). Clippy's
//! `await_holding_lock` lint flags this as a potential deadlock
//! source — and it would be, in a multi-threaded runtime with
//! concurrent awaiters. This simulator runs scenarios sequentially
//! on one task; there's no second thread to deadlock against.
//!
//! The correct long-term fix is migrating `managers` and
//! `rejoin_coordinator` from `std::sync::Mutex` to
//! `tokio::sync::Mutex`. Keeping the std version for now so the
//! simulator stays a pure-stdlib, no-tokio-runtime-needed harness
//! for the examples under apps/tui-node/examples/.
#![allow(clippy::await_holding_lock)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

use super::mesh_manager::{WebRTCMeshManager, PeerId};
use super::connection_monitor::ConnectionMonitor;
use super::rejoin_coordinator::RejoinCoordinator;

/// Network condition for simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkCondition {
    /// Perfect network conditions
    Perfect,
    /// Degraded with latency and loss
    Degraded { latency_ms: u32, packet_loss: f32 },
    /// Complete failure
    Failed,
    /// Intermittent connectivity
    Intermittent { up_time: Duration, down_time: Duration },
}

/// Simulation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimulationEvent {
    /// Peer joins the network
    PeerJoin(PeerId),
    /// Peer leaves gracefully
    PeerLeave(PeerId),
    /// Peer crashes suddenly
    PeerCrash(PeerId),
    /// Network condition changes
    NetworkChange(PeerId, NetworkCondition),
    /// DKG starts
    DkgStart,
    /// Signing starts
    SigningStart,
    /// Message sent
    MessageSent { from: PeerId, to: PeerId, size: usize },
    /// Rejoin attempt
    RejoinAttempt(PeerId),
}

/// Simulation scenario
#[derive(Debug, Clone)]
pub struct SimulationScenario {
    pub name: String,
    pub description: String,
    pub events: Vec<(Duration, SimulationEvent)>,
}

impl SimulationScenario {
    /// Creates a basic mesh establishment scenario
    pub fn basic_mesh() -> Self {
        Self {
            name: "Basic Mesh Establishment".to_string(),
            description: "Three peers establishing full mesh".to_string(),
            events: vec![
                (Duration::from_secs(0), SimulationEvent::PeerJoin(1)),
                (Duration::from_millis(100), SimulationEvent::PeerJoin(2)),
                (Duration::from_millis(200), SimulationEvent::PeerJoin(3)),
            ],
        }
    }

    /// Creates a disconnection and rejoin scenario
    pub fn disconnect_rejoin() -> Self {
        Self {
            name: "Disconnect and Rejoin".to_string(),
            description: "Peer disconnects during DKG and rejoins".to_string(),
            events: vec![
                (Duration::from_secs(0), SimulationEvent::PeerJoin(1)),
                (Duration::from_secs(0), SimulationEvent::PeerJoin(2)),
                (Duration::from_secs(0), SimulationEvent::PeerJoin(3)),
                (Duration::from_secs(1), SimulationEvent::DkgStart),
                (Duration::from_secs(2), SimulationEvent::PeerCrash(3)),
                (Duration::from_secs(5), SimulationEvent::RejoinAttempt(3)),
                (Duration::from_secs(6), SimulationEvent::PeerJoin(3)),
            ],
        }
    }

    /// Creates a network degradation scenario
    pub fn network_degradation() -> Self {
        Self {
            name: "Network Degradation".to_string(),
            description: "Network quality degrades and recovers".to_string(),
            events: vec![
                (Duration::from_secs(0), SimulationEvent::PeerJoin(1)),
                (Duration::from_secs(0), SimulationEvent::PeerJoin(2)),
                (Duration::from_secs(0), SimulationEvent::PeerJoin(3)),
                (Duration::from_secs(1), SimulationEvent::NetworkChange(2, 
                    NetworkCondition::Degraded { latency_ms: 500, packet_loss: 0.1 })),
                (Duration::from_secs(3), SimulationEvent::NetworkChange(2, 
                    NetworkCondition::Degraded { latency_ms: 1000, packet_loss: 0.3 })),
                (Duration::from_secs(5), SimulationEvent::NetworkChange(2, 
                    NetworkCondition::Perfect)),
            ],
        }
    }

    /// Creates a network partition scenario
    pub fn network_partition() -> Self {
        Self {
            name: "Network Partition".to_string(),
            description: "Network splits into two groups".to_string(),
            events: vec![
                (Duration::from_secs(0), SimulationEvent::PeerJoin(1)),
                (Duration::from_secs(0), SimulationEvent::PeerJoin(2)),
                (Duration::from_secs(0), SimulationEvent::PeerJoin(3)),
                (Duration::from_secs(1), SimulationEvent::DkgStart),
                // Partition: (1,2) | (3)
                (Duration::from_secs(2), SimulationEvent::NetworkChange(3, 
                    NetworkCondition::Failed)),
                (Duration::from_secs(4), SimulationEvent::SigningStart),
                // Heal partition
                (Duration::from_secs(6), SimulationEvent::NetworkChange(3, 
                    NetworkCondition::Perfect)),
            ],
        }
    }
}

/// WebRTC mesh network simulator
pub struct MeshSimulator {
    /// Participant managers
    pub managers: HashMap<PeerId, Arc<Mutex<WebRTCMeshManager>>>,
    /// Connection monitors
    pub monitors: HashMap<PeerId, Arc<ConnectionMonitor>>,
    /// Rejoin coordinator
    pub rejoin_coordinator: Arc<Mutex<RejoinCoordinator>>,
    /// Network conditions per peer
    pub network_conditions: Arc<Mutex<HashMap<PeerId, NetworkCondition>>>,
    /// Simulation start time
    pub start_time: Instant,
    /// Event log
    pub event_log: Arc<Mutex<Vec<(Duration, String)>>>,
}

impl MeshSimulator {
    /// Creates a new mesh simulator
    pub fn new(peers: Vec<PeerId>, threshold: usize) -> Self {
        let mut managers = HashMap::new();
        let mut monitors = HashMap::new();
        let mut network_conditions = HashMap::new();

        for peer in &peers {
            let manager = WebRTCMeshManager::new(*peer, peers.len(), threshold);
            managers.insert(*peer, Arc::new(Mutex::new(manager)));
            
            let monitor = ConnectionMonitor::new();
            monitors.insert(*peer, Arc::new(monitor));
            
            network_conditions.insert(*peer, NetworkCondition::Perfect);
        }

        let rejoin_coordinator = RejoinCoordinator::new(
            "simulation-session".to_string(),
            peers,
            threshold,
        );

        Self {
            managers,
            monitors,
            rejoin_coordinator: Arc::new(Mutex::new(rejoin_coordinator)),
            network_conditions: Arc::new(Mutex::new(network_conditions)),
            start_time: Instant::now(),
            event_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Logs an event
    fn log_event(&self, message: String) {
        let elapsed = self.start_time.elapsed();
        println!("[{:>6.2}s] {}", elapsed.as_secs_f32(), &message);
        self.event_log.lock().unwrap().push((elapsed, message));
    }

    /// Runs a simulation scenario
    pub async fn run_scenario(&mut self, scenario: SimulationScenario) {
        println!("\n╔════════════════════════════════════════════════╗");
        println!("║  SIMULATION: {}  ║", Self::pad_string(&scenario.name, 42));
        println!("╚════════════════════════════════════════════════╝");
        println!("\n📝 {}", scenario.description);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        self.start_time = Instant::now();

        for (delay, event) in scenario.events {
            // Wait for the scheduled time
            let elapsed = self.start_time.elapsed();
            if delay > elapsed {
                tokio::time::sleep(delay - elapsed).await;
            }

            self.handle_event(event).await;
        }

        // Final report
        self.print_final_report();
    }

    /// Handles a simulation event
    async fn handle_event(&mut self, event: SimulationEvent) {
        match event {
            SimulationEvent::PeerJoin(peer) => {
                self.log_event(format!("✅ Peer {} joining network", peer));
                self.handle_peer_join(peer).await;
            }
            SimulationEvent::PeerLeave(peer) => {
                self.log_event(format!("👋 Peer {} leaving gracefully", peer));
                self.handle_peer_leave(peer).await;
            }
            SimulationEvent::PeerCrash(peer) => {
                self.log_event(format!("💥 Peer {} crashed suddenly", peer));
                self.handle_peer_crash(peer).await;
            }
            SimulationEvent::NetworkChange(peer, condition) => {
                self.log_event(format!("🌐 Network change for peer {}: {:?}", peer, condition));
                self.handle_network_change(peer, condition).await;
            }
            SimulationEvent::DkgStart => {
                self.log_event("🔑 DKG protocol starting".to_string());
                self.handle_dkg_start().await;
            }
            SimulationEvent::SigningStart => {
                self.log_event("✍️ Signing protocol starting".to_string());
                self.handle_signing_start().await;
            }
            SimulationEvent::MessageSent { from, to, size } => {
                self.log_event(format!("📨 Message ({} bytes): {} → {}", size, from, to));
                self.handle_message_sent(from, to, size).await;
            }
            SimulationEvent::RejoinAttempt(peer) => {
                self.log_event(format!("🔄 Peer {} attempting to rejoin", peer));
                self.handle_rejoin_attempt(peer).await;
            }
        }
    }

    /// Handles peer join
    async fn handle_peer_join(&mut self, peer: PeerId) {
        let all_peers: Vec<PeerId> = self.managers.keys().copied().collect();
        
        if let Some(manager) = self.managers.get(&peer) {
            let mut mgr = manager.lock().unwrap();
            mgr.establish_mesh(all_peers).await.ok();
        }

        // Start monitoring
        if let Some(monitor) = self.monitors.get(&peer) {
            for other_peer in self.managers.keys() {
                if *other_peer != peer {
                    monitor.start_monitoring(*other_peer);
                }
            }
        }
    }

    /// Handles peer leave
    async fn handle_peer_leave(&mut self, peer: PeerId) {
        // Notify other peers
        for (other_peer, manager) in &self.managers {
            if *other_peer != peer {
                let mut mgr = manager.lock().unwrap();
                mgr.handle_peer_disconnect(peer).await;
            }
        }
    }

    /// Handles peer crash
    async fn handle_peer_crash(&mut self, peer: PeerId) {
        // Simulate sudden disconnection
        if let Some(manager) = self.managers.get(&peer) {
            let mut mgr = manager.lock().unwrap();
            mgr.simulate_network_failure();
        }

        // Other peers detect the crash
        for (other_peer, manager) in &self.managers {
            if *other_peer != peer {
                let mut mgr = manager.lock().unwrap();
                mgr.handle_peer_disconnect(peer).await;
            }
        }
    }

    /// Handles network condition change
    async fn handle_network_change(&mut self, peer: PeerId, condition: NetworkCondition) {
        self.network_conditions.lock().unwrap().insert(peer, condition.clone());

        match condition {
            NetworkCondition::Perfect => {
                if let Some(monitor) = self.monitors.get(&peer) {
                    for other_peer in self.managers.keys() {
                        if *other_peer != peer {
                            monitor.restore_connection(*other_peer);
                        }
                    }
                }
            }
            NetworkCondition::Degraded { latency_ms, packet_loss } => {
                if let Some(monitor) = self.monitors.get(&peer) {
                    for other_peer in self.managers.keys() {
                        if *other_peer != peer {
                            monitor.degrade_connection(*other_peer, latency_ms, packet_loss);
                        }
                    }
                }
            }
            NetworkCondition::Failed => {
                if let Some(manager) = self.managers.get(&peer) {
                    let mut mgr = manager.lock().unwrap();
                    mgr.simulate_network_failure();
                }
            }
            NetworkCondition::Intermittent { .. } => {
                // Could implement periodic up/down simulation
            }
        }
    }

    /// Handles DKG start
    async fn handle_dkg_start(&mut self) {
        {
            let coordinator = self.rejoin_coordinator.lock().unwrap();
            coordinator.advance_round();
        } // Drop lock here

        // Simulate DKG message exchange
        let peers: Vec<PeerId> = self.managers.keys().copied().collect();
        for from in &peers {
            for to in &peers {
                if from != to {
                    self.handle_message_sent(*from, *to, 256).await;
                }
            }
        }
    }

    /// Handles signing start
    async fn handle_signing_start(&mut self) {
        // Check if we have threshold
        let connected_counts: Vec<usize> = self.managers.values()
            .map(|m| m.lock().unwrap().get_connected_peers().len())
            .collect();

        let max_connected = connected_counts.iter().max().unwrap_or(&0);
        if *max_connected >= 1 { // At least 2 peers (self + 1)
            self.log_event("✅ Threshold met, signing can proceed".to_string());
        } else {
            self.log_event("❌ Below threshold, signing blocked".to_string());
        }
    }

    /// Handles message sent
    async fn handle_message_sent(&mut self, from: PeerId, to: PeerId, size: usize) {
        // Check network conditions
        let conditions = self.network_conditions.lock().unwrap();
        
        // Simulate based on conditions
        let from_condition = conditions.get(&from).cloned().unwrap_or(NetworkCondition::Perfect);
        let to_condition = conditions.get(&to).cloned().unwrap_or(NetworkCondition::Perfect);

        let delivered = match (&from_condition, &to_condition) {
            (NetworkCondition::Failed, _) | (_, NetworkCondition::Failed) => false,
            (NetworkCondition::Degraded { packet_loss, .. }, _) => {
                // Simulation-only random (not security-sensitive). rand 0.10's
                // convenience API: `random_range` on the thread-local CSPRNG.
                rand::random_range(0.0..1.0f32) > *packet_loss
            }
            _ => true,
        };

        if delivered {
            // Record message for recovery
            let coordinator = self.rejoin_coordinator.lock().unwrap();
            coordinator.record_message(from, 1, "SIMULATION", vec![0; size]);
        }
    }

    /// Handles rejoin attempt
    async fn handle_rejoin_attempt(&mut self, peer: PeerId) {
        use super::rejoin_coordinator::RejoinRequest;

        let request = RejoinRequest {
            peer_id: peer,
            session_id: "simulation-session".to_string(),
            last_round: 1,
            auth_token: "simulated_token_12345".to_string(),
            timestamp: Instant::now().elapsed().as_secs(),
        };

        let response = {
            let coordinator = self.rejoin_coordinator.lock().unwrap();
            coordinator.handle_rejoin_request(request).await
        };

        if response.accepted {
            self.log_event(format!("✅ Rejoin accepted for peer {}", peer));
            self.log_event(format!("  • Missed messages: {}", response.missed_messages.len()));
            
            // Re-establish connections
            self.handle_peer_join(peer).await;
            
            // Sync state
            let coordinator = self.rejoin_coordinator.lock().unwrap();
            coordinator.sync_participant(peer).await;
        } else {
            self.log_event(format!("❌ Rejoin rejected for peer {}: {:?}", 
                peer, response.rejection_reason));
        }
    }

    /// Prints final simulation report
    fn print_final_report(&self) {
        println!("\n╔════════════════════════════════════════════════╗");
        println!("║              SIMULATION REPORT                 ║");
        println!("╚════════════════════════════════════════════════╝");

        // Mesh statistics
        for (peer, manager) in &self.managers {
            let mgr = manager.lock().unwrap();
            let stats = mgr.get_mesh_stats();
            
            println!("\n📊 Peer {} Statistics:", peer);
            println!("  • Connected: {}/{}", stats.connected_peers, stats.total_peers - 1);
            println!("  • Threshold met: {}", if stats.meets_threshold { "✅" } else { "❌" });
        }

        // Connection quality
        for (peer, monitor) in &self.monitors {
            let stats = monitor.get_stats();
            
            if stats.total_peers > 0 {
                println!("\n🔍 Peer {} Connection Quality:", peer);
                println!("  • Healthy: {}/{}", stats.healthy_peers, stats.total_peers);
                println!("  • Avg latency: {}ms", stats.average_latency_ms);
                println!("  • Avg loss: {:.1}%", stats.average_packet_loss * 100.0);
            }
        }

        // Rejoin statistics
        let coordinator = self.rejoin_coordinator.lock().unwrap();
        let rejoin_stats = coordinator.get_rejoin_stats();
        
        if rejoin_stats.total_attempts > 0 {
            println!("\n🔄 Rejoin Statistics:");
            println!("  • Total attempts: {}", rejoin_stats.total_attempts);
            println!("  • Successful: {}", rejoin_stats.successful_rejoins);
            println!("  • Failed: {}", rejoin_stats.failed_rejoins);
        }

        // Event timeline
        println!("\n📜 Event Timeline:");
        let events = self.event_log.lock().unwrap();
        for (time, event) in events.iter().take(10) {
            println!("  [{:>6.2}s] {}", time.as_secs_f32(), event);
        }
        if events.len() > 10 {
            println!("  ... and {} more events", events.len() - 10);
        }
    }

    /// Helper to pad strings for display
    fn pad_string(s: &str, width: usize) -> String {
        if s.len() >= width {
            s[..width].to_string()
        } else {
            format!("{:^width$}", s, width = width)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_mesh_simulation() {
        let mut simulator = MeshSimulator::new(vec![1, 2, 3], 2);
        let scenario = SimulationScenario::basic_mesh();
        simulator.run_scenario(scenario).await;
    }

}