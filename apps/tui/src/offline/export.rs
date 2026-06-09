//! Export functionality for offline data

use std::path::Path;
use std::fs::{self, File};
use std::io::Write;
use super::{
    types::*,
    OfflineError, Result,
    create_filename,
};

/// Export signing request to file
pub fn export_signing_request(
    request: &SigningRequest,
    session_id: &str,
    output_path: &Path,
    expiration_minutes: u64,
) -> Result<()> {
    let data = OfflineData::new(
        OfflineDataType::SigningRequest,
        session_id.to_string(),
        request,
        expiration_minutes,
    )?;
    
    write_offline_data(&data, output_path)
}

/// Export commitments to file
pub fn export_commitments(
    commitments: &CommitmentsData,
    output_path: &Path,
    expiration_minutes: u64,
) -> Result<()> {
    let data = OfflineData::new(
        OfflineDataType::Commitments,
        commitments.session_id.clone(),
        commitments,
        expiration_minutes,
    )?;
    
    write_offline_data(&data, output_path)
}

/// Export signing package to file
pub fn export_signing_package(
    package: &SigningPackage,
    output_path: &Path,
    expiration_minutes: u64,
) -> Result<()> {
    let data = OfflineData::new(
        OfflineDataType::SigningPackage,
        package.session_id.clone(),
        package,
        expiration_minutes,
    )?;
    
    write_offline_data(&data, output_path)
}

/// Export signature share to file
pub fn export_signature_share(
    share: &SignatureShareData,
    output_path: &Path,
    expiration_minutes: u64,
) -> Result<()> {
    let data = OfflineData::new(
        OfflineDataType::SignatureShare,
        share.session_id.clone(),
        share,
        expiration_minutes,
    )?;
    
    write_offline_data(&data, output_path)
}

/// Export aggregated signature to file
pub fn export_aggregated_signature(
    signature: &AggregatedSignature,
    output_path: &Path,
    expiration_minutes: u64,
) -> Result<()> {
    let data = OfflineData::new(
        OfflineDataType::AggregatedSignature,
        signature.session_id.clone(),
        signature,
        expiration_minutes,
    )?;
    
    write_offline_data(&data, output_path)
}

/// Write offline data to a file
fn write_offline_data(data: &OfflineData, path: &Path) -> Result<()> {
    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Serialize to pretty JSON
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| OfflineError::SerializationError(e.to_string()))?;
    
    // Write to file
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    
    Ok(())
}

/// Export helper that creates standardized filenames
pub fn export_with_standard_name(
    data_type: &str,
    session_id: &str,
    device_id: Option<&str>,
    data: impl serde::Serialize,
    output_dir: &Path,
    expiration_minutes: u64,
) -> Result<String> {
    let filename = create_filename(data_type, session_id, device_id);
    let output_path = output_dir.join(&filename);
    
    let offline_data = OfflineData::new(
        match data_type {
            "request" => OfflineDataType::SigningRequest,
            "commitments" => OfflineDataType::Commitments,
            "package" => OfflineDataType::SigningPackage,
            "share" => OfflineDataType::SignatureShare,
            "signature" => OfflineDataType::AggregatedSignature,
            _ => return Err(OfflineError::InvalidFormat(format!("Unknown data type: {}", data_type))),
        },
        session_id.to_string(),
        data,
        expiration_minutes,
    )?;
    
    write_offline_data(&offline_data, &output_path)?;
    
    Ok(filename)
}