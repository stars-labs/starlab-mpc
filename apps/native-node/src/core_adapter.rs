//! Adapter to integrate TUI node's shared core with native UI

use slint::Weak;
use std::sync::Arc;
use tui_node::core::{
    connection_manager::ConnectionManager,
    dkg_manager::DkgManager,
    offline_manager::OfflineManager,
    session_manager::SessionManager,
    signing_manager::SigningManager,
    wallet_manager::WalletManager,
    CoreState, UICallback,
};

use crate::slint_generatedMainWindow::MainWindow;
use crate::ui_callback::NativeUICallback;

/// Core adapter that manages all the shared business logic
pub struct CoreAdapter {
    pub state: Arc<CoreState>,
    pub connection_manager: Arc<ConnectionManager>,
    pub session_manager: Arc<SessionManager>,
    pub dkg_manager: Arc<DkgManager>,
    pub wallet_manager: Arc<WalletManager>,
    pub offline_manager: Arc<OfflineManager>,
    pub signing_manager: Arc<SigningManager>,
    ui_callback: Arc<dyn UICallback>,
}

impl CoreAdapter {
    /// Create new core adapter with native UI callback
    pub fn new(window: Weak<MainWindow>) -> Self {
        let state = Arc::new(CoreState::new());
        let ui_callback: Arc<dyn UICallback> = Arc::new(NativeUICallback::new(window));
        
        Self {
            connection_manager: Arc::new(ConnectionManager::new(state.clone(), ui_callback.clone())),
            session_manager: Arc::new(SessionManager::new(state.clone(), ui_callback.clone())),
            dkg_manager: Arc::new(DkgManager::new(state.clone(), ui_callback.clone())),
            wallet_manager: Arc::new(WalletManager::new(state.clone(), ui_callback.clone())),
            offline_manager: Arc::new(OfflineManager::new(state.clone(), ui_callback.clone())),
            signing_manager: Arc::new(SigningManager::new(state.clone(), ui_callback.clone())),
            state,
            ui_callback,
        }
    }

    /// Create a new signing request. Typically called from a
    /// "Sign Message" button in Settings; opens the confirm modal
    /// via `UICallback::update_signing_request`. Returns the
    /// request id so the caller can pair approve/reject later.
    pub async fn request_signing(
        &self,
        message_hex: String,
        chain: String,
        display_label: Option<String>,
    ) -> Result<String, String> {
        self.signing_manager
            .request_signing(message_hex, chain, display_label)
            .await
            .map_err(|e| e.to_string())
    }

    /// User approved the pending signing request from the confirm
    /// modal. Drives state through commitment / share / aggregate.
    pub async fn approve_signing(&self, request_id: String) -> Result<(), String> {
        self.signing_manager
            .approve(&request_id)
            .await
            .map_err(|e| e.to_string())
    }

    /// User rejected the pending signing request.
    pub async fn reject_signing(&self, request_id: String) -> Result<(), String> {
        self.signing_manager
            .reject(&request_id)
            .await
            .map_err(|e| e.to_string())
    }

    /// Export an artefact to a user-chosen SD-card directory.
    ///
    /// Opens an rfd folder-picker; the caller's `data_type` is used
    /// as the filename stem (e.g. "dkg_round1", "signing_share").
    /// The actual bytes written are a placeholder until the core
    /// wires real DKG / signing artefacts out through
    /// `OfflineDataPackage` — see the native-node README "Next
    /// steps" #2. This commit ships the UI side so the buttons are
    /// live.
    pub async fn export_to_sd_card(&self, data_type: String) -> Result<(), String> {
        let Some(handle) = tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .set_title("Select SD card directory")
                .pick_folder()
        })
        .await
        .map_err(|e| e.to_string())?
        else {
            self.ui_callback
                .show_message("SD-card export cancelled".to_string(), false)
                .await;
            return Ok(());
        };

        let sd_dir = handle.to_path_buf();
        let filename = format!(
            "{}_{}.json",
            data_type,
            chrono::Utc::now().timestamp()
        );
        let target = sd_dir.join("mpc_wallet_export").join(&filename);

        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
        }
        // Placeholder content — swap for a real OfflineDataPackage
        // once the elm Message loop exposes its artefacts to core.
        let placeholder = serde_json::json!({
            "placeholder": true,
            "data_type": data_type,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "note": "native-node SD export — real artefact pending FROST hookup"
        });
        tokio::fs::write(&target, serde_json::to_vec_pretty(&placeholder).unwrap())
            .await
            .map_err(|e| e.to_string())?;

        self.ui_callback
            .show_message(
                format!("Wrote placeholder {} to {}", data_type, target.display()),
                false,
            )
            .await;
        Ok(())
    }

    /// Read SD-card artefacts from a user-chosen directory.
    ///
    /// Delegates to `OfflineManager::import_from_sd_card` after
    /// temporarily redirecting its path — since `OfflineManager`
    /// reads its `sd_card_path` field (set at construction), we
    /// can't override per-call yet. Until that setter lands, this
    /// handler just opens the folder picker to confirm the user
    /// knows where their SD card is mounted, then delegates.
    pub async fn import_from_sd_card(&self) -> Result<(), String> {
        let Some(handle) = tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .set_title("Select SD card directory to import from")
                .pick_folder()
        })
        .await
        .map_err(|e| e.to_string())?
        else {
            self.ui_callback
                .show_message("SD-card import cancelled".to_string(), false)
                .await;
            return Ok(());
        };

        let sd_dir = handle.to_path_buf();
        let import_subdir = sd_dir.join("mpc_wallet_import");
        if !import_subdir.exists() {
            self.ui_callback
                .show_message(
                    format!(
                        "No mpc_wallet_import/ directory under {}",
                        sd_dir.display()
                    ),
                    true,
                )
                .await;
            return Ok(());
        }

        // Enumerate + surface count via the UI. Real parse flow
        // would construct OfflineDataPackage values and feed them
        // into the elm handlers — deferred to Next step #2.
        let mut count = 0usize;
        let mut entries = tokio::fs::read_dir(&import_subdir)
            .await
            .map_err(|e| e.to_string())?;
        while let Some(_entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
            count += 1;
        }

        self.ui_callback
            .show_message(
                format!("Found {count} SD-card artefacts in {}", import_subdir.display()),
                false,
            )
            .await;
        Ok(())
    }

    /// Clear SD-card MPC wallet directories from a user-chosen
    /// folder. Same caveat as export/import re: OfflineManager's
    /// fixed path — this variant operates on the user-picked path
    /// directly.
    pub async fn clear_sd_card(&self) -> Result<(), String> {
        let Some(handle) = tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .set_title("Select SD card directory to clear")
                .pick_folder()
        })
        .await
        .map_err(|e| e.to_string())?
        else {
            self.ui_callback
                .show_message("SD-card clear cancelled".to_string(), false)
                .await;
            return Ok(());
        };

        let sd_dir = handle.to_path_buf();
        let confirmed = self
            .ui_callback
            .request_confirmation(format!(
                "Clear mpc_wallet_* directories under {}?",
                sd_dir.display()
            ))
            .await;
        if !confirmed {
            return Ok(());
        }

        for sub in ["mpc_wallet_export", "mpc_wallet_import"] {
            let p = sd_dir.join(sub);
            if p.exists() {
                tokio::fs::remove_dir_all(&p).await.map_err(|e| e.to_string())?;
            }
        }

        self.state.pending_sd_operations.lock().await.clear();
        self.ui_callback.update_sd_operations(Vec::new()).await;
        self.ui_callback
            .show_message(format!("Cleared SD-card data under {}", sd_dir.display()), false)
            .await;
        Ok(())
    }
    
    /// Connect to WebSocket server
    pub async fn connect_websocket(&self, url: String) -> Result<(), String> {
        self.connection_manager
            .connect_websocket(url)
            .await
            .map_err(|e| e.to_string())
    }
    
    /// Create a new wallet
    pub async fn create_wallet(&self) -> Result<(), String> {
        // For demo, create with default parameters
        self.wallet_manager
            .create_wallet(
                "New Wallet".to_string(),
                2,
                vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    
    /// Import a keystore from disk. Opens a native file-picker for
    /// the `.dat` path, then passes the user-supplied password to
    /// `WalletManager::import_wallet` for decryption. Pass `""` if
    /// the keystore is unencrypted.
    pub async fn import_wallet(&self, password: String) -> Result<(), String> {
        // `rfd::AsyncFileDialog` is async-friendly but its await
        // point runs on the GUI thread; keep this on tokio's
        // blocking scheduler to avoid blocking the Slint event loop.
        let Some(handle) = tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .add_filter("MPC keystore", &["dat", "json"])
                .set_title("Import MPC keystore")
                .pick_file()
        })
        .await
        .map_err(|e| e.to_string())?
        else {
            self.ui_callback
                .show_message("Import cancelled".to_string(), false)
                .await;
            return Ok(());
        };

        let path = handle.to_string_lossy().into_owned();
        let msg = if password.is_empty() {
            format!("Importing keystore from {path} (no password)...")
        } else {
            format!("Importing keystore from {path}...")
        };
        self.ui_callback.show_message(msg, false).await;

        self.wallet_manager
            .import_wallet(path, password)
            .await
            .map_err(|e| e.to_string())
    }

    /// Export the active wallet to a keystore file. Opens a native
    /// save dialog for the destination path; the user-supplied
    /// password is used to encrypt the output. Pass `""` to write
    /// an unencrypted keystore.
    pub async fn export_wallet(&self, password: String) -> Result<(), String> {
        let Some(handle) = tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .add_filter("MPC keystore", &["dat"])
                .set_title("Export MPC keystore")
                .set_file_name("mpc-wallet.dat")
                .save_file()
        })
        .await
        .map_err(|e| e.to_string())?
        else {
            self.ui_callback
                .show_message("Export cancelled".to_string(), false)
                .await;
            return Ok(());
        };

        let path = handle.to_string_lossy().into_owned();

        // Export the currently-active wallet. CoreState tracks the
        // active index alongside the wallet list.
        let active_index = *self.state.active_wallet_index.lock().await;
        let msg = if password.is_empty() {
            format!("Exporting wallet to {path} (unencrypted)...")
        } else {
            format!("Exporting wallet to {path}...")
        };
        self.ui_callback.show_message(msg, false).await;

        self.wallet_manager
            .export_wallet(active_index, path, password)
            .await
            .map_err(|e| e.to_string())
    }
    
    /// Create a new session
    pub async fn create_session(&self) -> Result<(), String> {
        // Get device ID (would be from config in real app)
        let device_id = "native-node-001".to_string();
        
        self.session_manager
            .create_session(device_id, 2, 3)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    
    /// Join an existing session
    pub async fn join_session(&self, session_id: String) -> Result<(), String> {
        let device_id = "native-node-001".to_string();
        
        self.session_manager
            .join_session(session_id, device_id)
            .await
            .map_err(|e| e.to_string())
    }
    
    /// Leave current session
    pub async fn leave_session(&self) -> Result<(), String> {
        let device_id = "native-node-001".to_string();
        
        self.session_manager
            .leave_session(device_id)
            .await
            .map_err(|e| e.to_string())
    }
    
    /// Refresh available sessions
    pub async fn refresh_sessions(&self) -> Result<(), String> {
        self.session_manager
            .refresh_sessions()
            .await
            .map_err(|e| e.to_string())
    }
    
    /// Toggle offline mode
    pub async fn toggle_offline_mode(&self) -> Result<(), String> {
        self.offline_manager
            .toggle_offline_mode()
            .await
            .map_err(|e| e.to_string())
    }
    
    /// Start DKG process
    pub async fn start_dkg(&self) -> Result<(), String> {
        // Get active session
        let session = self.session_manager
            .get_active_session()
            .await
            .ok_or_else(|| "No active session".to_string())?;
        
        // Start DKG with session participants
        self.dkg_manager
            .start_dkg(session.threshold.0, session.participants)
            .await
            .map_err(|e| e.to_string())
    }
    
    /// Abort DKG process
    pub async fn abort_dkg(&self) -> Result<(), String> {
        self.dkg_manager
            .abort_dkg()
            .await
            .map_err(|e| e.to_string())
    }
}