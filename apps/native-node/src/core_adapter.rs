//! Adapter to integrate TUI node's shared core with native UI

use slint::Weak;
use std::sync::Arc;
use tui_node::core::{
    connection_manager::ConnectionManager,
    dkg_manager::DkgManager,
    offline_manager::OfflineManager,
    session_manager::SessionManager,
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
            state,
            ui_callback,
        }
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
    /// the `.dat` path, and (for now) uses an empty password — the
    /// full flow should surface a password-prompt modal in Slint
    /// before calling into here; see README "Next steps" #1.
    pub async fn import_wallet(&self) -> Result<(), String> {
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
        self.ui_callback
            .show_message(format!("Importing keystore from {path}..."), false)
            .await;

        // TODO(native-node): wire a password-prompt modal and pass
        // the user-supplied password in here. Empty string for now
        // — WalletManager::import_wallet will surface the real error
        // from keystore decryption.
        self.wallet_manager
            .import_wallet(path, String::new())
            .await
            .map_err(|e| e.to_string())
    }

    /// Export the active wallet to a keystore file. Opens a native
    /// save dialog for the destination path.
    pub async fn export_wallet(&self) -> Result<(), String> {
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
        self.ui_callback
            .show_message(format!("Exporting wallet to {path}..."), false)
            .await;

        // TODO(native-node): password-prompt modal (see import_wallet).
        self.wallet_manager
            .export_wallet(active_index, path, String::new())
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