//! Offline mode functionality for air-gapped signing operations
//!
//! This module provides support for threshold signing without network connectivity,
//! using SD cards or other removable media for data transfer.

pub mod types;
pub mod export;
pub mod import;
pub mod session;

pub use types::*;
pub use session::OfflineSession;

use std::path::Path;
use std::fs;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

/// Result type for offline operations
pub type Result<T> = std::result::Result<T, OfflineError>;

/// Errors that can occur during offline operations
#[derive(Debug, thiserror::Error)]
pub enum OfflineError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid data format: {0}")]
    InvalidFormat(String),

    #[error("Session expired at {0}")]
    SessionExpired(DateTime<Utc>),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid session state: {0}")]
    InvalidState(String),

    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    #[error("Device not authorized for session: {0}")]
    UnauthorizedDevice(String),

    #[error("Threshold not met: got {0}, need {1}")]
    ThresholdNotMet(usize, usize),

    #[error("General offline error: {0}")]
    General(String),
}

/// Offline mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineConfig {
    /// Default session expiration duration (in minutes)
    pub default_expiration_minutes: u64,
    
    /// Path to SD card mount point
    pub sdcard_path: Option<String>,
    
    /// Auto-import files on detection
    pub auto_import: bool,
    
    /// Delete files after successful import
    pub delete_after_import: bool,
    
    /// Maximum file size to import (in bytes)
    pub max_file_size: usize,
}

impl Default for OfflineConfig {
    fn default() -> Self {
        Self {
            default_expiration_minutes: 60,
            sdcard_path: None,
            auto_import: false,
            delete_after_import: false,
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// Check if a path looks like an SD card or removable media
pub fn is_removable_media(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    
    // Common mount points for removable media
    path_str.contains("/mnt/") ||
    path_str.contains("/media/") ||
    path_str.contains("/Volumes/") || // macOS
    path_str.contains("/run/media/") || // Some Linux distros
    path_str.starts_with("/dev/sd") || // Direct device access
    path_str.contains("removable") ||
    path_str.contains("usb") ||
    path_str.contains("sdcard")
}

/// Validate that a file is safe to import
pub fn validate_import_file(path: &Path, config: &OfflineConfig) -> Result<()> {
    // Check file exists
    if !path.exists() {
        return Err(OfflineError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", path.display())
        )));
    }

    // Check file size
    let metadata = fs::metadata(path)?;
    if metadata.len() as usize > config.max_file_size {
        return Err(OfflineError::InvalidFormat(format!(
            "File too large: {} bytes (max: {} bytes)",
            metadata.len(),
            config.max_file_size
        )));
    }

    // Check file extension
    if path.extension().and_then(|s| s.to_str()) != Some("json") {
        return Err(OfflineError::InvalidFormat(
            "Only JSON files are supported".to_string()
        ));
    }

    Ok(())
}

/// Create a standardized filename for offline data
pub fn create_filename(data_type: &str, session_id: &str, device_id: Option<&str>) -> String {
    if let Some(device) = device_id {
        format!("{}_{}__{}.json", session_id, data_type, device)
    } else {
        format!("{}_{}.json", session_id, data_type)
    }
}