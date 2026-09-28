//! Elm Architecture implementation for the TUI
//!
//! This module implements the Elm Architecture pattern using tui-realm,
//! providing a clean, functional approach to building the terminal interface
//! with predictable state management.

pub mod app;
pub mod command;
pub mod components;
pub mod desktop_notify;
pub mod error_help;
pub mod headless;
pub mod log_safe;
pub mod message;
pub mod model;
pub mod provider;
pub mod update;
pub mod webrtc_signaling;
pub mod ws_runtime;

pub use app::ElmApp;
pub use command::Command;
pub use headless::HeadlessRunner;
pub use message::Message;
pub use model::{Model, NetworkState, Screen, UIState, WalletState};
pub use provider::{NoOpUIProvider, UIProvider, WalletDisplayInfo};
pub use update::update;
