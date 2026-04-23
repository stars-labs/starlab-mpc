mod ui_callback;
mod core_adapter;

use anyhow::Result;
use core_adapter::CoreAdapter;
use slint::ComponentHandle;
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber;

slint::include_modules!();

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    // Create the main window
    let window = MainWindow::new()?;
    
    // Set initial device ID
    let app_state = window.global::<AppState>();
    app_state.set_device_id("native-node-001".into());
    
    // Create core adapter with shared logic
    let adapter = Arc::new(CoreAdapter::new(window.as_weak()));
    
    // Set up UI callbacks
    {
        let adapter = adapter.clone();
        window.on_connect_websocket(move |url| {
            let adapter = adapter.clone();
            let url = url.to_string();
            tokio::spawn(async move {
                if let Err(e) = adapter.connect_websocket(url).await {
                    println!("Failed to connect WebSocket: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_create_wallet(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.create_wallet().await {
                    println!("Failed to create wallet: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_import_wallet(move |password| {
            let adapter = adapter.clone();
            let password = password.to_string();
            tokio::spawn(async move {
                if let Err(e) = adapter.import_wallet(password).await {
                    println!("Failed to import wallet: {}", e);
                }
            });
        });
    }

    {
        let adapter = adapter.clone();
        window.on_export_wallet(move |password| {
            let adapter = adapter.clone();
            let password = password.to_string();
            tokio::spawn(async move {
                if let Err(e) = adapter.export_wallet(password).await {
                    println!("Failed to export wallet: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_create_session(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.create_session().await {
                    println!("Failed to create session: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_join_session(move |session_id| {
            let adapter = adapter.clone();
            let session_id = session_id.to_string();
            tokio::spawn(async move {
                if let Err(e) = adapter.join_session(session_id).await {
                    println!("Failed to join session: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_leave_session(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.leave_session().await {
                    println!("Failed to leave session: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_refresh_sessions(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.refresh_sessions().await {
                    println!("Failed to refresh sessions: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_toggle_offline_mode(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.toggle_offline_mode().await {
                    println!("Failed to toggle offline mode: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_start_dkg(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.start_dkg().await {
                    println!("Failed to start DKG: {}", e);
                }
            });
        });
    }
    
    {
        let adapter = adapter.clone();
        window.on_abort_dkg(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.abort_dkg().await {
                    println!("Failed to abort DKG: {}", e);
                }
            });
        });
    }

    // Signing: open the confirm modal from a hex message.
    {
        let adapter = adapter.clone();
        window.on_sign_message(move |message_hex, chain| {
            let adapter = adapter.clone();
            let message_hex = message_hex.to_string();
            let chain = chain.to_string();
            tokio::spawn(async move {
                if let Err(e) = adapter
                    .request_signing(message_hex, chain, None)
                    .await
                {
                    println!("Failed to request signing: {}", e);
                }
            });
        });
    }

    {
        let adapter = adapter.clone();
        window.on_approve_signing(move |request_id| {
            let adapter = adapter.clone();
            let request_id = request_id.to_string();
            tokio::spawn(async move {
                if let Err(e) = adapter.approve_signing(request_id).await {
                    println!("Failed to approve signing: {}", e);
                }
            });
        });
    }

    {
        let adapter = adapter.clone();
        window.on_reject_signing(move |request_id| {
            let adapter = adapter.clone();
            let request_id = request_id.to_string();
            tokio::spawn(async move {
                if let Err(e) = adapter.reject_signing(request_id).await {
                    println!("Failed to reject signing: {}", e);
                }
            });
        });
    }

    // SD-card operations. All three open an rfd folder picker so
    // the user points at whatever mount-point their SD card has
    // (the default /media/sdcard in OfflineManager doesn't cover
    // macOS or Windows).
    {
        let adapter = adapter.clone();
        window.on_export_to_sd_card(move |data_type| {
            let adapter = adapter.clone();
            let data_type = data_type.to_string();
            tokio::spawn(async move {
                if let Err(e) = adapter.export_to_sd_card(data_type).await {
                    println!("Failed to export to SD card: {}", e);
                }
            });
        });
    }

    {
        let adapter = adapter.clone();
        window.on_import_from_sd_card(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.import_from_sd_card().await {
                    println!("Failed to import from SD card: {}", e);
                }
            });
        });
    }

    {
        let adapter = adapter.clone();
        window.on_clear_sd_card(move || {
            let adapter = adapter.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter.clear_sd_card().await {
                    println!("Failed to clear SD card: {}", e);
                }
            });
        });
    }

    // Run the UI.
    //
    // The previous `adapter.initialize_demo()` call that lived here
    // seeded two fake wallets ("Demo Wallet 1", "Demo Wallet 2") and
    // two fake sessions every time the app started. That made the
    // UI look populated during early development but hid the real
    // empty state from users — if the app appears to have wallets,
    // a new user doesn't realise they still need to create one. The
    // real keystore (imported via the file dialog) is the only
    // source of wallets; the session list is populated when the WS
    // connects and the signal server broadcasts available sessions.
    window.run()?;
    
    Ok(())
}