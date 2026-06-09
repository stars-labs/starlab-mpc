//! Offline mode management for air-gapped operations

use super::{CoreError, CoreResult, CoreState, OperationMode, SDCardOperation, SDOperationType, UICallback};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tracing::{info, warn};

/// Data package for offline exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineDataPackage {
    pub package_id: String,
    pub operation_type: String,
    pub round: u8,
    pub participant_id: String,
    pub timestamp: String,
    pub data: Vec<u8>,
    pub checksum: String,
}

/// Offline manager handles SD card operations and air-gapped data exchange
pub struct OfflineManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
    sd_card_path: PathBuf,
}

impl OfflineManager {
    pub fn new(state: Arc<CoreState>, ui_callback: Arc<dyn UICallback>) -> Self {
        Self {
            state,
            ui_callback,
            // Default SD card mount point - can be configured
            sd_card_path: PathBuf::from("/media/sdcard"),
        }
    }
    
    /// Toggle offline mode
    pub async fn toggle_offline_mode(&self) -> CoreResult<()> {
        let current_mode = self.state.operation_mode.lock().await.clone();
        
        let new_mode = match current_mode {
            OperationMode::Online => {
                info!("Switching to offline mode");
                OperationMode::Offline
            }
            OperationMode::Offline => {
                info!("Switching to online mode");
                OperationMode::Online
            }
            OperationMode::Hybrid => {
                info!("Switching from hybrid to offline mode");
                OperationMode::Offline
            }
        };
        
        *self.state.operation_mode.lock().await = new_mode.clone();
        *self.state.offline_enabled.lock().await = new_mode == OperationMode::Offline;
        
        // Check SD card if entering offline mode
        if new_mode == OperationMode::Offline {
            self.check_sd_card().await?;
        }
        
        // Update UI
        self.ui_callback.update_operation_mode(new_mode.clone()).await;
        self.ui_callback.update_offline_status(
            new_mode == OperationMode::Offline,
            *self.state.sd_card_detected.lock().await,
        ).await;
        
        self.ui_callback.show_message(
            format!("Switched to {} mode", match new_mode {
                OperationMode::Online => "online",
                OperationMode::Offline => "offline",
                OperationMode::Hybrid => "hybrid",
            }),
            false
        ).await;
        
        Ok(())
    }
    
    /// Check if SD card is available
    pub async fn check_sd_card(&self) -> CoreResult<bool> {
        let detected = self.sd_card_path.exists() && self.sd_card_path.is_dir();
        
        *self.state.sd_card_detected.lock().await = detected;
        
        self.ui_callback.update_offline_status(
            *self.state.offline_enabled.lock().await,
            detected,
        ).await;
        
        if !detected && *self.state.offline_enabled.lock().await {
            self.ui_callback.show_message(
                "SD card not detected. Please insert SD card for offline operations.".to_string(),
                true
            ).await;
        }
        
        Ok(detected)
    }
    
    /// Export data to SD card
    pub async fn export_to_sd_card(&self, data: OfflineDataPackage) -> CoreResult<()> {
        info!("Exporting data to SD card: {}", data.package_id);
        
        // Check SD card
        if !self.check_sd_card().await? {
            return Err(CoreError::Offline("SD card not available".to_string()));
        }
        
        // Create export directory
        let export_dir = self.sd_card_path.join("starlab_export");
        fs::create_dir_all(&export_dir).await
            .map_err(|e| CoreError::Offline(format!("Failed to create export directory: {}", e)))?;
        
        // Generate filename
        let filename = format!(
            "{}_{}_{}_{}.json",
            data.operation_type,
            data.round,
            data.participant_id,
            chrono::Utc::now().timestamp()
        );
        
        let file_path = export_dir.join(filename);
        
        // Write data
        let json_data = serde_json::to_string_pretty(&data)
            .map_err(|e| CoreError::Offline(format!("Failed to serialize data: {}", e)))?;
        
        fs::write(&file_path, json_data).await
            .map_err(|e| CoreError::Offline(format!("Failed to write to SD card: {}", e)))?;
        
        // Add to pending operations
        let operation = SDCardOperation {
            operation_type: SDOperationType::Export,
            data_type: data.operation_type.clone(),
            participant: data.participant_id.clone(),
            timestamp: data.timestamp.clone(),
            data: data.data.clone(),
        };
        
        self.state.pending_sd_operations.lock().await.push(operation);
        
        // Update UI
        self.ui_callback.update_sd_operations(
            self.state.pending_sd_operations.lock().await.clone()
        ).await;
        
        self.ui_callback.show_message(
            format!("Data exported to SD card: {}", file_path.display()),
            false
        ).await;
        
        Ok(())
    }
    
    /// Import data from SD card
    pub async fn import_from_sd_card(&self) -> CoreResult<Vec<OfflineDataPackage>> {
        info!("Importing data from SD card");
        
        // Check SD card
        if !self.check_sd_card().await? {
            return Err(CoreError::Offline("SD card not available".to_string()));
        }
        
        // Look for import directory
        let import_dir = self.sd_card_path.join("starlab_import");
        if !import_dir.exists() {
            self.ui_callback.show_message(
                "No import directory found on SD card".to_string(),
                false
            ).await;
            return Ok(Vec::new());
        }
        
        let mut imported_packages = Vec::new();
        
        // Read all JSON files
        let mut entries = fs::read_dir(&import_dir).await
            .map_err(|e| CoreError::Offline(format!("Failed to read import directory: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| CoreError::Offline(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                // Read and parse file
                let content = fs::read_to_string(&path).await
                    .map_err(|e| CoreError::Offline(format!("Failed to read file {}: {}", path.display(), e)))?;
                
                match serde_json::from_str::<OfflineDataPackage>(&content) {
                    Ok(package) => {
                        info!("Imported package: {}", package.package_id);
                        
                        // Add to pending operations
                        let operation = SDCardOperation {
                            operation_type: SDOperationType::Import,
                            data_type: package.operation_type.clone(),
                            participant: package.participant_id.clone(),
                            timestamp: package.timestamp.clone(),
                            data: package.data.clone(),
                        };
                        
                        self.state.pending_sd_operations.lock().await.push(operation);
                        imported_packages.push(package);
                        
                        // Move file to processed directory
                        let processed_dir = import_dir.join("processed");
                        fs::create_dir_all(&processed_dir).await.ok();
                        let new_path = processed_dir.join(path.file_name().unwrap());
                        fs::rename(&path, new_path).await.ok();
                    }
                    Err(e) => {
                        warn!("Failed to parse file {}: {}", path.display(), e);
                    }
                }
            }
        }
        
        // Update UI
        self.ui_callback.update_sd_operations(
            self.state.pending_sd_operations.lock().await.clone()
        ).await;
        
        self.ui_callback.show_message(
            format!("Imported {} packages from SD card", imported_packages.len()),
            false
        ).await;
        
        Ok(imported_packages)
    }
    
    /// Clear SD card data
    pub async fn clear_sd_card(&self) -> CoreResult<()> {
        info!("Clearing SD card data");
        
        // Check SD card
        if !self.check_sd_card().await? {
            return Err(CoreError::Offline("SD card not available".to_string()));
        }
        
        // Confirm with user
        let confirmed = self.ui_callback.request_confirmation(
            "Are you sure you want to clear all MPC wallet data from the SD card?".to_string()
        ).await;
        
        if !confirmed {
            return Ok(());
        }
        
        // Remove export and import directories
        let export_dir = self.sd_card_path.join("starlab_export");
        let import_dir = self.sd_card_path.join("starlab_import");
        
        if export_dir.exists() {
            fs::remove_dir_all(&export_dir).await
                .map_err(|e| CoreError::Offline(format!("Failed to clear export directory: {}", e)))?;
        }
        
        if import_dir.exists() {
            fs::remove_dir_all(&import_dir).await
                .map_err(|e| CoreError::Offline(format!("Failed to clear import directory: {}", e)))?;
        }
        
        // Clear pending operations
        self.state.pending_sd_operations.lock().await.clear();
        
        // Update UI
        self.ui_callback.update_sd_operations(Vec::new()).await;
        
        self.ui_callback.show_message(
            "SD card data cleared".to_string(),
            false
        ).await;
        
        Ok(())
    }
    
    /// Process pending SD card operations
    pub async fn process_pending_operations(&self) -> CoreResult<()> {
        let operations = self.state.pending_sd_operations.lock().await.clone();
        
        info!("Processing {} pending SD card operations", operations.len());
        
        for op in operations {
            match op.operation_type {
                SDOperationType::Export => {
                    // Already exported, just log
                    info!("Export operation for {} already completed", op.data_type);
                }
                SDOperationType::Import => {
                    // Process imported data based on type
                    info!("Processing imported {} data from {}", op.data_type, op.participant);
                    
                    // Here we would dispatch to appropriate handlers
                    // based on data_type (dkg_round1, dkg_round2, signing_request, etc.)
                }
            }
        }
        
        // Clear processed operations
        self.state.pending_sd_operations.lock().await.clear();
        self.ui_callback.update_sd_operations(Vec::new()).await;
        
        self.ui_callback.show_message(
            "Processed all pending operations".to_string(),
            false
        ).await;
        
        Ok(())
    }
    
    /// Get current offline status
    pub async fn get_offline_status(&self) -> (bool, bool) {
        (
            *self.state.offline_enabled.lock().await,
            *self.state.sd_card_detected.lock().await,
        )
    }
}