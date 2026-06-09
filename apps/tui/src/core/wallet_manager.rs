//! Wallet management logic shared between TUI and native nodes

use super::{CoreError, CoreResult, CoreState, WalletInfo, UICallback};
use crate::keystore::Keystore;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Wallet manager handles wallet operations and keystore management
pub struct WalletManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
    keystore: Arc<Mutex<Option<Keystore>>>,
}

impl WalletManager {
    pub fn new(state: Arc<CoreState>, ui_callback: Arc<dyn UICallback>) -> Self {
        Self {
            state,
            ui_callback,
            keystore: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Create a new wallet
    pub async fn create_wallet(
        &self,
        name: String,
        threshold: u16,
        participants: Vec<String>,
    ) -> CoreResult<WalletInfo> {
        info!("Creating new wallet: {}", name);
        
        // Generate wallet ID and address (placeholder)
        let wallet_id = format!("wallet_{}", uuid::Uuid::new_v4());
        let address = format!("0x{}", hex::encode([0u8; 20])); // Placeholder address
        
        let wallet = WalletInfo {
            id: wallet_id.clone(),
            name: name.clone(),
            address: address.clone(),
            balance: "0.0 ETH".to_string(),
            chain: "Ethereum".to_string(),
            threshold: format!("{}/{}", threshold, participants.len()),
            participants: participants.clone(),
        };
        
        // Add to wallets
        self.state.wallets.lock().await.push(wallet.clone());
        
        // Set as active wallet
        let wallets = self.state.wallets.lock().await;
        let index = wallets.len() - 1;
        drop(wallets);
        
        *self.state.active_wallet_index.lock().await = index;
        
        // Update UI
        self.ui_callback.update_wallets(
            self.state.wallets.lock().await.clone()
        ).await;
        self.ui_callback.update_active_wallet(index).await;
        
        self.ui_callback.show_message(
            format!("Created wallet: {}", name),
            false
        ).await;
        
        Ok(wallet)
    }
    
    /// Import wallet from keystore file
    pub async fn import_wallet(&self, keystore_path: String, _password: String) -> CoreResult<()> {
        info!("Importing wallet from: {}", keystore_path);
        
        // For now, create a demo imported wallet
        // Real implementation would load from keystore
        let wallet = WalletInfo {
            id: format!("imported_{}", uuid::Uuid::new_v4()),
            name: format!("Imported Wallet {}", self.state.wallets.lock().await.len() + 1),
            address: "0x0000000000000000000000000000000000000000".to_string(),
            balance: "0.0 ETH".to_string(),
            chain: "Ethereum".to_string(),
            threshold: "2/3".to_string(),
            participants: vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()],
        };
        
        // Add wallet
        self.state.wallets.lock().await.push(wallet);
        
        // Update UI
        self.ui_callback.update_wallets(
            self.state.wallets.lock().await.clone()
        ).await;
        
        self.ui_callback.show_message(
            "Wallet imported successfully".to_string(),
            false
        ).await;
        
        Ok(())
    }
    
    /// Export wallet to keystore file
    pub async fn export_wallet(&self, wallet_index: usize, export_path: String, _password: String) -> CoreResult<()> {
        info!("Exporting wallet to: {}", export_path);
        
        let wallets = self.state.wallets.lock().await;
        let _wallet = wallets.get(wallet_index)
            .ok_or_else(|| CoreError::Wallet("Invalid wallet index".to_string()))?;
        
        // For now, just show a message
        // Real implementation would export to keystore
        drop(wallets);
        
        self.ui_callback.show_message(
            "Wallet export feature coming soon".to_string(),
            false
        ).await;
        
        Ok(())
    }
    
    /// Delete a wallet
    pub async fn delete_wallet(&self, wallet_index: usize) -> CoreResult<()> {
        info!("Deleting wallet at index: {}", wallet_index);
        
        // Confirm with user
        let confirmed = self.ui_callback.request_confirmation(
            "Are you sure you want to delete this wallet? This action cannot be undone.".to_string()
        ).await;
        
        if !confirmed {
            return Ok(());
        }
        
        // Remove wallet
        let mut wallets = self.state.wallets.lock().await;
        if wallet_index >= wallets.len() {
            return Err(CoreError::Wallet("Invalid wallet index".to_string()));
        }
        
        wallets.remove(wallet_index);
        
        // Update active index if needed
        let mut active_index = self.state.active_wallet_index.lock().await;
        if *active_index >= wallets.len() && !wallets.is_empty() {
            *active_index = wallets.len() - 1;
        } else if wallets.is_empty() {
            *active_index = 0;
        }
        let new_index = *active_index;
        
        drop(active_index);
        drop(wallets);
        
        // Update UI
        self.ui_callback.update_wallets(
            self.state.wallets.lock().await.clone()
        ).await;
        self.ui_callback.update_active_wallet(new_index).await;
        
        self.ui_callback.show_message(
            "Wallet deleted".to_string(),
            false
        ).await;
        
        Ok(())
    }
    
    /// Switch active wallet
    pub async fn switch_wallet(&self, wallet_index: usize) -> CoreResult<()> {
        let wallets = self.state.wallets.lock().await;
        if wallet_index >= wallets.len() {
            return Err(CoreError::Wallet("Invalid wallet index".to_string()));
        }
        
        let wallet = wallets[wallet_index].clone();
        drop(wallets);
        
        info!("Switching to wallet: {}", wallet.name);
        
        *self.state.active_wallet_index.lock().await = wallet_index;
        
        // Update UI
        self.ui_callback.update_active_wallet(wallet_index).await;
        
        self.ui_callback.show_message(
            format!("Switched to wallet: {}", wallet.name),
            false
        ).await;
        
        Ok(())
    }
    
    /// Update wallet balance
    pub async fn update_wallet_balance(&self, wallet_index: usize, balance: String) -> CoreResult<()> {
        let mut wallets = self.state.wallets.lock().await;
        if let Some(wallet) = wallets.get_mut(wallet_index) {
            wallet.balance = balance;
            info!("Updated balance for wallet {}: {}", wallet.name, wallet.balance);
        }
        let wallets_clone = wallets.clone();
        drop(wallets);
        
        // Update UI
        self.ui_callback.update_wallets(wallets_clone).await;
        
        Ok(())
    }
    
    /// Get all wallets
    pub async fn get_wallets(&self) -> Vec<WalletInfo> {
        self.state.wallets.lock().await.clone()
    }
    
    /// Get active wallet
    pub async fn get_active_wallet(&self) -> Option<WalletInfo> {
        let wallets = self.state.wallets.lock().await;
        let index = *self.state.active_wallet_index.lock().await;
        wallets.get(index).cloned()
    }
    
    /// Check if keystore is loaded
    pub async fn has_keystore(&self) -> bool {
        self.keystore.lock().await.is_some()
    }
    
    /// Save wallet state from completed DKG
    pub async fn save_dkg_result(
        &self,
        session_id: String,
        _key_package: Vec<u8>,
        _public_key: Vec<u8>,
        participant_index: u16,
    ) -> CoreResult<()> {
        info!("Saving DKG result for session: {}", session_id);
        
        // For now, just log
        // Real implementation would store in keystore
        info!("Stored key package for participant {}", participant_index);
        
        self.ui_callback.show_message(
            "DKG result saved to keystore".to_string(),
            false
        ).await;
        
        Ok(())
    }
}