//! WebRTC mesh network implementation for P2P communication

pub mod mesh_manager;
pub mod connection_monitor;
pub mod rejoin_coordinator;
pub mod mesh_simulator;

pub use mesh_manager::{WebRTCMeshManager, MeshTopology, ConnectionState};
pub use connection_monitor::{ConnectionMonitor, ConnectionQuality};
pub use rejoin_coordinator::{RejoinCoordinator, RejoinRequest, SessionState};
pub use mesh_simulator::{MeshSimulator, NetworkCondition, SimulationEvent, SimulationScenario};