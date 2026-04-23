// Library exports for mpc-wallet-tui

pub mod blockchain_config;
#[cfg(test)]
mod blockchain_config_test;
pub mod core;
pub mod keystore;
pub mod utils;
pub mod protocal;
pub mod network;
pub mod offline;
pub mod elm;
pub mod hybrid;
pub mod webrtc;

// Re-export commonly used types
pub use keystore::{Keystore, DeviceInfo};
pub use utils::appstate_compat::AppState;
pub use utils::state::{DkgState, MeshStatus, SigningState};
pub use protocal::signal::SessionInfo;

// Re-export Elm architecture types (now includes all UI functionality)
pub use elm::{ElmApp, Model, Message, Screen, UIProvider, NoOpUIProvider, WalletDisplayInfo};
pub use elm::components::{Id as ComponentId};