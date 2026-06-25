// Library exports for starlab-tui

pub mod blockchain_config;
#[cfg(test)]
mod blockchain_config_test;
pub mod core;
pub mod elm;
pub mod hybrid;
pub mod keystore;
pub mod network;
pub mod offline;
pub mod protocal;
pub mod utils;
pub mod webrtc;

// Re-export commonly used types
pub use keystore::{DeviceInfo, Keystore};
pub use protocal::signal::SessionInfo;
pub use utils::appstate_compat::AppState;
pub use utils::state::{DkgState, MeshStatus, SigningState};

// Re-export Elm architecture types (now includes all UI functionality)
pub use elm::components::Id as ComponentId;
pub use elm::{ElmApp, Message, Model, NoOpUIProvider, Screen, UIProvider, WalletDisplayInfo};
