//! Starlab Hardware Share (`SLHW v1`) protocol, encrypted NVS vault,
//! pluggable [`HardwareHal`], and FROST signing engine.
//!
//! Designed so the exact same Rust state machine compiles and runs on:
//! - Real low-cost ESP32-S3 / ESP32-C3 microcontrollers (`hardware/esp32-share/`)
//! - Host-side software simulators and conformance tests (`SimHal`)
//! - Future USB-CDC / WebSerial host bridges in `starlab-cli` and `starlab-desktop`

pub mod airgap;
pub mod engine;
pub mod hal;
pub mod protocol;
pub mod sim;
pub mod vault;

/// Wire protocol version (NDJSON over USB-CDC / UART / MicroSD).
pub const SLHW_PROTOCOL_VERSION: u32 = 1;

/// Default firmware version reported in `device_info`.
pub const DEFAULT_FIRMWARE_VERSION: &str = "0.1.0";

/// Consecutive wrong PIN attempts before irreversible cryptographic Flash wipe.
pub const MAX_PIN_ATTEMPTS: u8 = 10;

pub use airgap::{AirGapEnvelope, OfflineDataPackage};
pub use engine::HardwareShareEngine;
pub use hal::{HalError, HalFrostRng, HardwareHal};
pub use protocol::{
    AddressInfo, CurveKind, HwCommand, HwErrorCode, HwEvent, HwRequest, HwResponse,
    SignDisplayInfo, WalletSummary,
};
pub use sim::SimHal;
pub use vault::VaultError;
