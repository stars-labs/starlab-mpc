//! WebRTC mesh network implementation for P2P communication

pub mod connection_monitor;
pub mod mesh_manager;
pub mod mesh_simulator;
pub mod rejoin_coordinator;

pub use connection_monitor::{ConnectionMonitor, ConnectionQuality};
pub use mesh_manager::{ConnectionState, MeshTopology, WebRTCMeshManager};
pub use mesh_simulator::{MeshSimulator, NetworkCondition, SimulationEvent, SimulationScenario};
pub use rejoin_coordinator::{RejoinCoordinator, RejoinRequest, SessionState};
