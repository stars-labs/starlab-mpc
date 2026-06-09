//! Import functionality for offline data

use std::path::Path;
use std::fs;
use super::{
    types::*,
    OfflineError, Result,
    validate_import_file,
    OfflineConfig,
};

/// Import any offline data file
pub fn import_offline_data(path: &Path, config: &OfflineConfig) -> Result<OfflineData> {
    // Validate file
    validate_import_file(path, config)?;
    
    // Read file
    let contents = fs::read_to_string(path)?;
    
    // Parse JSON
    let data: OfflineData = serde_json::from_str(&contents)
        .map_err(|e| OfflineError::InvalidFormat(format!("Invalid JSON: {}", e)))?;
    
    // Validate data
    data.validate()?;
    
    Ok(data)
}

/// Import and extract signing request
pub fn import_signing_request(path: &Path, config: &OfflineConfig) -> Result<SigningRequest> {
    let data = import_offline_data(path, config)?;
    
    // Verify type
    if data.data_type != OfflineDataType::SigningRequest {
        return Err(OfflineError::InvalidFormat(format!(
            "Expected signing_request, got {:?}",
            data.data_type
        )));
    }
    
    data.extract()
}

/// Import and extract commitments
pub fn import_commitments(path: &Path, config: &OfflineConfig) -> Result<CommitmentsData> {
    let data = import_offline_data(path, config)?;
    
    // Verify type
    if data.data_type != OfflineDataType::Commitments {
        return Err(OfflineError::InvalidFormat(format!(
            "Expected commitments, got {:?}",
            data.data_type
        )));
    }
    
    data.extract()
}

/// Import and extract signing package
pub fn import_signing_package(path: &Path, config: &OfflineConfig) -> Result<SigningPackage> {
    let data = import_offline_data(path, config)?;
    
    // Verify type
    if data.data_type != OfflineDataType::SigningPackage {
        return Err(OfflineError::InvalidFormat(format!(
            "Expected signing_package, got {:?}",
            data.data_type
        )));
    }
    
    data.extract()
}

/// Import and extract signature share
pub fn import_signature_share(path: &Path, config: &OfflineConfig) -> Result<SignatureShareData> {
    let data = import_offline_data(path, config)?;
    
    // Verify type
    if data.data_type != OfflineDataType::SignatureShare {
        return Err(OfflineError::InvalidFormat(format!(
            "Expected signature_share, got {:?}",
            data.data_type
        )));
    }
    
    data.extract()
}

/// Import and extract aggregated signature
pub fn import_aggregated_signature(path: &Path, config: &OfflineConfig) -> Result<AggregatedSignature> {
    let data = import_offline_data(path, config)?;
    
    // Verify type
    if data.data_type != OfflineDataType::AggregatedSignature {
        return Err(OfflineError::InvalidFormat(format!(
            "Expected aggregated_signature, got {:?}",
            data.data_type
        )));
    }
    
    data.extract()
}

/// Import multiple files from a directory
pub fn import_from_directory(
    dir_path: &Path,
    config: &OfflineConfig,
    session_id: Option<&str>,
) -> Result<Vec<(String, OfflineData)>> {
    if !dir_path.is_dir() {
        return Err(OfflineError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Not a directory: {}", dir_path.display())
        )));
    }
    
    let mut imported = Vec::new();
    
    // Read directory entries
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        
        // Skip non-JSON files
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        
        // Try to import
        match import_offline_data(&path, config) {
            Ok(data) => {
                // Filter by session ID if provided
                if let Some(sid) = session_id
                    && data.session_id != sid {
                        continue;
                    }
                
                let filename = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                    
                imported.push((filename, data));
            }
            Err(e) => {
                // Log error but continue with other files
                eprintln!("Failed to import {}: {}", path.display(), e);
            }
        }
    }
    
    Ok(imported)
}

/// Auto-detect and import offline data based on type
pub fn auto_import(data: OfflineData) -> Result<ImportResult> {
    match data.data_type {
        OfflineDataType::SigningRequest => {
            let request: SigningRequest = data.extract()?;
            Ok(ImportResult::SigningRequest(request))
        }
        OfflineDataType::Commitments => {
            let commitments: CommitmentsData = data.extract()?;
            Ok(ImportResult::Commitments(commitments))
        }
        OfflineDataType::SigningPackage => {
            let package: SigningPackage = data.extract()?;
            Ok(ImportResult::SigningPackage(package))
        }
        OfflineDataType::SignatureShare => {
            let share: SignatureShareData = data.extract()?;
            Ok(ImportResult::SignatureShare(share))
        }
        OfflineDataType::AggregatedSignature => {
            let sig: AggregatedSignature = data.extract()?;
            Ok(ImportResult::AggregatedSignature(sig))
        }
    }
}

/// Result of auto-import
#[derive(Debug)]
pub enum ImportResult {
    SigningRequest(SigningRequest),
    Commitments(CommitmentsData),
    SigningPackage(SigningPackage),
    SignatureShare(SignatureShareData),
    AggregatedSignature(AggregatedSignature),
}