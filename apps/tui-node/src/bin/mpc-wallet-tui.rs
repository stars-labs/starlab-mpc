//! MPC Wallet TUI - Terminal User Interface using Elm Architecture
//! 
//! This is the main entry point for the MPC Wallet Terminal Interface.
//! It uses the Elm Architecture pattern for clean, predictable state management.

use clap::Parser;
use frost_secp256k1::Secp256K1Sha256;
use std::io::IsTerminal;
use std::sync::Arc;
use tracing::info;
use tui_node::elm::ElmApp;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log file location
    #[arg(long, default_value = "~/.frost_keystore/logs/mpc-wallet.log")]
    log_location: String,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Device ID for this instance (must be unique)
    /// If not provided, uses hostname. The keystore is always at
    /// ~/.frost_keystore (not per-device-id — device_id is the
    /// participant identity in the FROST mesh, not a filesystem prefix).
    #[arg(long = "device-id")]
    device_id: Option<String>,

    /// Run in offline mode (no network connections)
    #[arg(long)]
    offline: bool,

    /// Signal server URL
    /// Example: --signal-server ws://localhost:9000
    #[arg(long, default_value = "wss://panda.qzz.io")]
    signal_server: String,

    /// Tenant room (REQUIRED by the deployed server). A strong shared id all
    /// participants of a ceremony use; merged into the signal URL as
    /// `?room=<id>`. Generate with `uuidgen`. Without it the server rejects
    /// the connection.
    #[arg(long)]
    room: Option<String>,
}

/// Merge a `room` into the signal URL as a query param (`?room=` / `&room=`).
/// No-op when absent — the server then rejects with a clear error.
fn with_room(url: &str, room: Option<&str>) -> String {
    match room {
        Some(r) if !r.is_empty() && !url.contains("room=") => {
            if url.contains('?') {
                format!("{url}&room={r}")
            } else if url.splitn(2, "://").nth(1).unwrap_or(url).contains('/') {
                format!("{url}?room={r}")
            } else {
                format!("{url}/?room={r}") // no path → WS handshake needs one
            }
        }
        _ => url.to_string(),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Determine device ID
    let device_id = args.device_id.unwrap_or_else(|| {
        gethostname::gethostname()
            .into_string()
            .unwrap_or_else(|_| "default-node".to_string())
    });

    // Setup logging to file (since TUI takes over terminal)
    let log_filename = format!("mpc-wallet-{}.log", device_id);
    println!("Logging to: {}", log_filename);
    println!(
        "Current directory: {:?}",
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    );

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true) // Start fresh each run
        .open(&log_filename)
        .unwrap_or_else(|e| {
            eprintln!("Failed to create log file {}: {}", log_filename, e);
            std::fs::File::create("/dev/null").unwrap()
        });

    tracing_subscriber::fmt()
        .with_writer(log_file)
        .with_env_filter(args.log_level)
        .with_ansi(false)
        .init();

    info!("=== MPC Wallet TUI Started ===");
    info!("Device ID: {}", device_id);
    info!(
        "Working directory: {:?}",
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    );
    info!("Log file: {}", log_filename);
    info!("Signal server: {}", args.signal_server);
    info!("Offline mode: {}", args.offline);

    // Check if we're in a TTY environment
    if !std::io::stdout().is_terminal() {
        return Err(anyhow::anyhow!(
            "TUI requires a TTY environment. Run in a proper terminal."
        ));
    }

    // Run the Elm-based TUI application
    run_elm_tui(
        device_id,
        with_room(&args.signal_server, args.room.as_deref()),
        args.offline,
    )
    .await
}

/// Run the Elm Architecture TUI
async fn run_elm_tui(device_id: String, signal_server: String, offline: bool) -> anyhow::Result<()> {
    use crossterm::{
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        execute,
    };
    use std::io;

    info!("🚀 Starting Elm Architecture TUI");

    // Create app state with device ID and signal server URL
    let app_state = Arc::new(tokio::sync::Mutex::new(
        tui_node::utils::appstate_compat::AppState::<Secp256K1Sha256>::with_device_id_and_server(
            device_id.clone(),
            signal_server.clone()
        )
    ));

    // Create and initialize Elm app
    let mut elm_app = ElmApp::new(device_id.clone(), app_state.clone())?;
    
    // Initialize keystore automatically
    let keystore_path = format!("{}/.frost_keystore", std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    info!("Initializing keystore at: {} for device: {}", keystore_path, device_id);
    
    // Initialize keystore in app state
    {
        let mut state = app_state.lock().await;
        match tui_node::keystore::Keystore::new(&keystore_path, &device_id) {
            Ok(keystore) => {
                state.keystore = Some(Arc::new(keystore));
                info!("✅ Keystore initialized successfully");
            }
            Err(e) => {
                tracing::error!("❌ Failed to initialize keystore: {}", e);
            }
        }
    }

    // Setup terminal with panic handler for cleanup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    
    // Set up Ctrl+C handler for graceful shutdown
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Clean up terminal on panic
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // If not offline, kick off the initial WebSocket connection via the Elm
    // command system. `TriggerReconnect` reads `signal_server_url` from
    // `AppState` (populated above) and dispatches `Command::ReconnectWebSocket`,
    // which is the code path that actually runs `connect_async`.
    if !offline {
        info!("Connecting to signal server: {}", signal_server);
        if let Err(e) = elm_app
            .get_message_sender()
            .send(tui_node::elm::message::Message::TriggerReconnect)
        {
            tracing::error!("Failed to queue initial WebSocket connect: {}", e);
        }
    } else {
        info!("Running in offline mode - no network connections");
    }

    // Run the Elm app (blocks until user quits)
    let result = elm_app.run().await;
    
    // Cleanup - restore terminal
    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;
    
    match result {
        Ok(()) => {
            info!("✅ TUI exited successfully");
            Ok(())
        }
        Err(e) => {
            tracing::error!("❌ TUI error: {}", e);
            Err(e)
        }
    }
}