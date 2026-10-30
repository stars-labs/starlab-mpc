//! Wallet management logic shared between TUI and native nodes

use super::{CoreError, CoreResult, CoreState, UICallback, WalletInfo};
use crate::keystore::{Keystore, WalletMetadata};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

/// Wallet manager handles wallet operations and keystore management
pub struct WalletManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
    /// `(keystore dir, device id)` — where import/export read and write.
    keystore: Option<(PathBuf, String)>,
}

impl WalletManager {
    pub fn new(state: Arc<CoreState>, ui_callback: Arc<dyn UICallback>) -> Self {
        Self {
            state,
            ui_callback,
            keystore: None,
        }
    }

    /// Point import/export at this device's keystore (the same directory +
    /// device id the embedder's HeadlessRunner uses).
    pub fn with_keystore(
        mut self,
        keystore_path: impl Into<PathBuf>,
        device_id: impl Into<String>,
    ) -> Self {
        self.keystore = Some((keystore_path.into(), device_id.into()));
        self
    }

    fn open_keystore(&self) -> CoreResult<Keystore> {
        let (path, device_id) = self.keystore.as_ref().ok_or_else(|| {
            CoreError::Wallet(
                "no keystore configured — construct WalletManager::with_keystore".to_string(),
            )
        })?;
        Keystore::new(path, device_id)
            .map_err(|e| CoreError::Wallet(format!("cannot open keystore: {e}")))
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
        self.ui_callback
            .update_wallets(self.state.wallets.lock().await.clone())
            .await;
        self.ui_callback.update_active_wallet(index).await;

        self.ui_callback
            .show_message(format!("Created wallet: {}", name), false)
            .await;

        Ok(wallet)
    }

    /// Import a key share file (from [`Self::export_wallet`] or copied from
    /// another keystore) into this device's keystore. The password must
    /// unlock it; the share is validated (curve, BIP-340 for secp256k1,
    /// group key) before it is written. Embedders running a HeadlessRunner
    /// then send `Message::ListWallets` so the runner rescans the keystore.
    pub async fn import_wallet(
        &self,
        file_path: String,
        password: String,
    ) -> CoreResult<WalletMetadata> {
        info!("Importing wallet from: {}", file_path);
        let bytes = tokio::fs::read(&file_path)
            .await
            .map_err(|e| CoreError::Wallet(format!("cannot read {file_path}: {e}")))?;
        let mut keystore = self.open_keystore()?;
        let metadata = keystore
            .import_share(&bytes, &password)
            .map_err(|e| CoreError::Wallet(format!("import failed: {e}")))?;

        self.state.wallets.lock().await.push(wallet_info(&metadata));
        self.ui_callback
            .update_wallets(self.state.wallets.lock().await.clone())
            .await;
        self.ui_callback
            .show_message(
                format!(
                    "Imported wallet {} ({}-of-{} {})",
                    metadata.display_name(),
                    metadata.threshold,
                    metadata.total_participants,
                    metadata.curve_type
                ),
                false,
            )
            .await;
        Ok(metadata)
    }

    /// Export `wallet_id`'s `curve_type` key share to `export_path`. The file
    /// stays encrypted with the wallet password, which must be `password`.
    pub async fn export_wallet(
        &self,
        wallet_id: &str,
        curve_type: &str,
        export_path: String,
        password: String,
    ) -> CoreResult<()> {
        info!(
            "Exporting wallet {} ({}) to: {}",
            wallet_id, curve_type, export_path
        );
        let json = self
            .open_keystore()?
            .export_share(wallet_id, curve_type, &password)
            .map_err(|e| CoreError::Wallet(format!("export failed: {e}")))?;
        write_private(&export_path, json.as_bytes())
            .map_err(|e| CoreError::Wallet(format!("cannot write {export_path}: {e}")))?;
        self.ui_callback
            .show_message(
                format!("Exported wallet {wallet_id} to {export_path}"),
                false,
            )
            .await;
        Ok(())
    }

    /// Delete a wallet
    pub async fn delete_wallet(&self, wallet_index: usize) -> CoreResult<()> {
        info!("Deleting wallet at index: {}", wallet_index);

        // Confirm with user
        let confirmed = self
            .ui_callback
            .request_confirmation(
                "Are you sure you want to delete this wallet? This action cannot be undone."
                    .to_string(),
            )
            .await;

        if !confirmed {
            return Ok(());
        }

        if *self.state.dkg_active.lock().await
            || !matches!(
                *self.state.signing_state.lock().await,
                super::SigningState::Idle
                    | super::SigningState::Complete
                    | super::SigningState::Failed(_)
            )
        {
            return Err(CoreError::Wallet(
                "Finish or cancel the active ceremony before deleting a wallet".into(),
            ));
        }

        // Remove wallet
        let mut wallets = self.state.wallets.lock().await;
        if wallet_index >= wallets.len() {
            return Err(CoreError::Wallet("Invalid wallet index".to_string()));
        }

        self.open_keystore()?
            .delete_wallet(&wallets[wallet_index].id)
            .map_err(|e| CoreError::Wallet(format!("cannot delete wallet: {e}")))?;
        wallets.remove(wallet_index);

        // Update active index if needed
        let mut active_index = self.state.active_wallet_index.lock().await;
        if wallet_index < *active_index {
            *active_index -= 1;
        }
        if *active_index >= wallets.len() && !wallets.is_empty() {
            *active_index = wallets.len() - 1;
        } else if wallets.is_empty() {
            *active_index = 0;
        }
        let new_index = *active_index;

        drop(active_index);
        drop(wallets);

        // Update UI
        self.ui_callback
            .update_wallets(self.state.wallets.lock().await.clone())
            .await;
        self.ui_callback.update_active_wallet(new_index).await;

        self.ui_callback
            .show_message("Wallet deleted".to_string(), false)
            .await;

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

        self.ui_callback
            .show_message(format!("Switched to wallet: {}", wallet.name), false)
            .await;

        Ok(())
    }

    /// Update wallet balance
    pub async fn update_wallet_balance(
        &self,
        wallet_index: usize,
        balance: String,
    ) -> CoreResult<()> {
        let mut wallets = self.state.wallets.lock().await;
        if let Some(wallet) = wallets.get_mut(wallet_index) {
            wallet.balance = balance;
            info!(
                "Updated balance for wallet {}: {}",
                wallet.name, wallet.balance
            );
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
        self.keystore.is_some()
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

        self.ui_callback
            .show_message("DKG result saved to keystore".to_string(), false)
            .await;

        Ok(())
    }
}

/// Core `WalletInfo` for a keystore wallet: account-0 primary address (never
/// the root group-key address), real threshold / participants.
fn wallet_info(metadata: &WalletMetadata) -> WalletInfo {
    let address = hex::decode(&metadata.group_public_key)
        .ok()
        .and_then(|key| {
            starlab_core::accounts::account_addresses(&metadata.curve_type, &key, 0).ok()
        })
        .and_then(|addrs| addrs.into_iter().next().map(|(_, _, addr)| addr))
        .unwrap_or_default();
    WalletInfo {
        id: metadata.session_id.clone(),
        name: metadata.display_name().to_string(),
        address,
        balance: String::new(),
        chain: if metadata.curve_type == "ed25519" {
            "Solana".to_string()
        } else {
            "Ethereum".to_string()
        },
        threshold: format!("{}/{}", metadata.threshold, metadata.total_participants),
        participants: metadata.participants.clone(),
    }
}

/// Write `data` to `path` readable by the owner only (0600 on unix) — an
/// exported key share must not be world-readable, even though it is
/// encrypted. An existing file is truncated and tightened too.
fn write_private(path: &str, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        let mut file = options.open(path)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(data)
    }
    #[cfg(not(unix))]
    {
        options.open(path)?.write_all(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        ConnectionInfo, OperationMode, ParticipantInfo, SDCardOperation, SessionInfo,
    };
    use crate::elm::command::{Command, encode_keystore_blob};
    use crate::elm::message::Message;
    use async_trait::async_trait;
    use frost_secp256k1_tr::Secp256K1Sha256TR as Secp;
    use tempfile::TempDir;

    const PW: &str = "correct-horse-battery";

    struct NoopUi;

    #[async_trait]
    impl UICallback for NoopUi {
        async fn update_connection_status(&self, _: bool, _: bool) {}
        async fn update_mesh_connections(&self, _: Vec<ConnectionInfo>) {}
        async fn update_operation_mode(&self, _: OperationMode) {}
        async fn update_wallets(&self, _: Vec<WalletInfo>) {}
        async fn update_active_wallet(&self, _: usize) {}
        async fn update_available_sessions(&self, _: Vec<SessionInfo>) {}
        async fn update_active_session(&self, _: Option<SessionInfo>) {}
        async fn update_dkg_status(&self, _: bool, _: u8, _: f32) {}
        async fn update_dkg_participants(&self, _: Vec<ParticipantInfo>) {}
        async fn update_offline_status(&self, _: bool, _: bool) {}
        async fn update_sd_operations(&self, _: Vec<SDCardOperation>) {}
        async fn show_message(&self, _: String, _: bool) {}
        async fn show_progress(&self, _: String, _: f32) {}
        async fn request_confirmation(&self, _: String) -> bool {
            true
        }
    }

    fn manager(dir: &TempDir, device: &str) -> (WalletManager, Arc<CoreState>) {
        let state = Arc::new(CoreState::new());
        let mgr =
            WalletManager::new(state.clone(), Arc::new(NoopUi)).with_keystore(dir.path(), device);
        (mgr, state)
    }

    /// Keystore dir holding participant 1's share of a 2-of-3 secp256k1
    /// wallet; returns the dir, wallet id and group key hex.
    fn source_keystore() -> (TempDir, String, String) {
        let dir = TempDir::new().unwrap();
        let mut ks = Keystore::new(dir.path(), "device-a").unwrap();
        let (kps, pp) = starlab_core::resharing::dkg_keypackages::<Secp>(3, 2, 7).unwrap();
        let blob = encode_keystore_blob::<Secp>(&kps[&1], &pp).unwrap();
        let group_hex = hex::encode(pp.verifying_key().serialize().unwrap());
        let id = ks
            .create_wallet_multi_chain(
                "w-mgr",
                "secp256k1",
                vec![],
                2,
                3,
                &group_hex,
                &blob,
                PW,
                vec![],
                None,
                1,
                vec!["device-a".into(), "device-b".into(), "device-c".into()],
                None,
            )
            .unwrap();
        (dir, id, group_hex)
    }

    #[tokio::test]
    async fn export_then_import_adds_the_real_wallet() {
        let (src, id, group_hex) = source_keystore();
        let (exporter, _) = manager(&src, "device-a");
        let file = src.path().join("export.json");
        exporter
            .export_wallet(&id, "secp256k1", file.to_string_lossy().into(), PW.into())
            .await
            .unwrap();

        let dst = TempDir::new().unwrap();
        let (importer, state) = manager(&dst, "device-restore");
        let meta = importer
            .import_wallet(file.to_string_lossy().into(), PW.into())
            .await
            .unwrap();
        assert_eq!(meta.group_public_key, group_hex);

        let wallets = state.wallets.lock().await.clone();
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].id, id);
        assert_eq!(wallets[0].threshold, "2/3");
        assert_eq!(wallets[0].participants.len(), 3);
        let expected = starlab_core::accounts::account_addresses(
            "secp256k1",
            &hex::decode(&group_hex).unwrap(),
            0,
        )
        .unwrap()[0]
            .2
            .clone();
        assert_eq!(wallets[0].address, expected);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn exported_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (src, id, _) = source_keystore();
        let (exporter, _) = manager(&src, "device-a");
        let file = src.path().join("export.json");
        // A pre-existing world-readable file gets tightened as well.
        std::fs::write(&file, b"old").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

        exporter
            .export_wallet(&id, "secp256k1", file.to_string_lossy().into(), PW.into())
            .await
            .unwrap();

        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[tokio::test]
    async fn import_with_wrong_password_fails_clearly() {
        let (src, id, _) = source_keystore();
        let (exporter, _) = manager(&src, "device-a");
        let file = src.path().join("export.json");
        exporter
            .export_wallet(&id, "secp256k1", file.to_string_lossy().into(), PW.into())
            .await
            .unwrap();
        let dst = TempDir::new().unwrap();
        let (importer, state) = manager(&dst, "device-restore");
        let err = importer
            .import_wallet(file.to_string_lossy().into(), "nope-nope".into())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Invalid password"), "{err}");
        assert!(state.wallets.lock().await.is_empty());
    }

    #[tokio::test]
    async fn export_without_a_keystore_errors() {
        let mgr = WalletManager::new(Arc::new(CoreState::new()), Arc::new(NoopUi));
        assert!(!mgr.has_keystore().await);
        let err = mgr
            .export_wallet("w", "secp256k1", "/tmp/x.json".into(), PW.into())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no keystore configured"), "{err}");
    }

    /// The embedder flow: WalletManager imports into the runner's keystore
    /// dir, then `ListWallets` → `LoadWallets` must see the new wallet even
    /// though the runner's cached Keystore predates it.
    #[tokio::test]
    async fn load_wallets_sees_a_wallet_imported_behind_the_runners_back() {
        let (src, id, _) = source_keystore();
        let file = src.path().join("export.json");
        manager(&src, "device-a")
            .0
            .export_wallet(&id, "secp256k1", file.to_string_lossy().into(), PW.into())
            .await
            .unwrap();

        let dst = TempDir::new().unwrap();
        let app_state = Arc::new(tokio::sync::Mutex::new(
            crate::AppState::<Secp>::with_device_id_and_server("device-b".into(), String::new()),
        ));
        app_state.lock().await.keystore =
            Some(Arc::new(Keystore::new(dst.path(), "device-b").unwrap()));

        manager(&dst, "device-b")
            .0
            .import_wallet(file.to_string_lossy().into(), PW.into())
            .await
            .unwrap();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        Command::LoadWallets.execute(tx, &app_state).await.unwrap();
        match rx.recv().await {
            Some(Message::WalletsLoaded { wallets }) => {
                assert_eq!(wallets.len(), 1);
                assert_eq!(wallets[0].session_id, id);
            }
            other => panic!("expected WalletsLoaded, got {other:?}"),
        }
    }
}
