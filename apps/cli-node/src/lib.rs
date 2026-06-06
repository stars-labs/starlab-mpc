//! `mpc-wallet-cli` library surface.
//!
//! The CLI is split into a lib + a thin bin (`src/main.rs`) so the
//! protocol types, the serve daemon, and the multi-node `simulate`
//! orchestrator are reusable from integration tests and (later) the
//! one-shot subcommands — not buried in the binary.

pub mod bridge;
pub mod oneshot;
pub mod policy;
pub mod protocol;
pub mod reshare;
pub mod serve;
pub mod simulate;
pub mod trace;
