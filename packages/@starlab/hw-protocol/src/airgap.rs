//! Air-gap MicroSD data exchange compatible with `starlab-client::core::offline_manager`.
//!
//! Matches the exact `starlab_export/` & `starlab_import/` `OfflineDataPackage`
//! JSON schema shared between `starlab-tui` and `starlab-desktop`, so a
//! zero-connectivity ESP32-S3 board with a MicroSD slot (e.g. CYD-S3 2.8") can
//! participate in air-gapped DKG and cold signing ceremonies.

use crate::protocol::{HwCommand, HwEvent};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Canonical directory names on the MicroSD card (shared with `offline_manager.rs`).
pub const SD_EXPORT_DIR: &str = "starlab_export";
pub const SD_IMPORT_DIR: &str = "starlab_import";

/// Field-for-field compatible with `starlab_client::core::offline_manager::OfflineDataPackage`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineDataPackage {
    pub package_id: String,
    pub operation_type: String,
    pub round: u8,
    pub participant_id: String,
    pub timestamp: String,
    pub data: Vec<u8>,
    pub checksum: String,
}

impl OfflineDataPackage {
    /// Construct a package with a canonical SHA-256 hex checksum over `data`.
    pub fn new(
        package_id: impl Into<String>,
        operation_type: impl Into<String>,
        round: u8,
        participant_id: impl Into<String>,
        timestamp: impl Into<String>,
        data: Vec<u8>,
    ) -> Self {
        let checksum = compute_checksum(&data);
        Self {
            package_id: package_id.into(),
            operation_type: operation_type.into(),
            round,
            participant_id: participant_id.into(),
            timestamp: timestamp.into(),
            data,
            checksum,
        }
    }

    /// Verify that `self.checksum` matches `SHA-256(self.data)`.
    pub fn verify_checksum(&self) -> bool {
        self.checksum.eq_ignore_ascii_case(&compute_checksum(&self.data))
    }

    /// Canonical filename convention matching `offline_manager.rs`:
    /// `{operation_type}_{round}_{participant_id}_{timestamp}.json`.
    pub fn suggested_filename(&self) -> String {
        format!(
            "{}_{}_{}_{}.json",
            self.operation_type, self.round, self.participant_id, self.timestamp
        )
    }
}

/// Structured payload placed inside `OfflineDataPackage.data` (UTF-8 JSON) for
/// SLHW v1 air-gap commands/responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AirGapEnvelope {
    Request { command: HwCommand },
    Response { event: HwEvent },
}

pub fn compute_checksum(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}
