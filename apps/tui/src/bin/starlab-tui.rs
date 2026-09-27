//! MPC Wallet TUI - Terminal User Interface using Elm Architecture
//!
//! This is the main entry point for the MPC Wallet Terminal Interface.
//! It uses the Elm Architecture pattern for clean, predictable state management.

use clap::Parser;
use frost_secp256k1_tr::Secp256K1Sha256TR;
use starlab_client::elm::ElmApp;
use std::io::IsTerminal;
use std::sync::Arc;
use tracing::info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log file location (`~/` is expanded). Truncated on each start.
    /// [default: ~/.frost_keystore/logs/starlab-mpc-<device-id>.log]
    #[arg(long)]
    log_location: Option<String>,

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

/// A "strong" room the hosted multi-tenant server will accept: ≥16 chars of
/// `[A-Za-z0-9_-]` (mirrors the server's `MIN_ROOM_LEN`). Kept identical to the
/// CLI's check so both front-ends reject the same weak rooms.
fn is_strong_room(r: &str) -> bool {
    r.chars().count() >= 16
        && r.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
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

    // Fail fast on a bad --room for the hosted server, before the TUI takes over
    // the terminal (same footgun the CLI guards: `--room test-1`). Offline mode
    // and a local ws:// server need no room, so only enforce for online wss://.
    if !args.offline && args.signal_server.starts_with("wss://") {
        match args.room.as_deref() {
            Some(r) if !is_strong_room(r) => {
                eprintln!(
                    "error: --room \"{r}\" is too weak for the hosted server ({}). It needs ≥16 \
                     chars of [A-Za-z0-9_-] (got {} char(s)). Generate a strong one and share the \
                     SAME value with every device:\n      --room \"$(uuidgen | tr -d -)\"\n    (a \
                     local --signal-server ws://<host-ip>:9000, or --offline, needs no room.)",
                    args.signal_server,
                    r.chars().count()
                );
                std::process::exit(1);
            }
            None => {
                eprintln!(
                    "warning: no --room set — the hosted server ({}) requires a strong room \
                     (≥16 chars) or it rejects the connection (you'll stay Offline). Add \
                     --room \"$(uuidgen | tr -d -)\" (the SAME on every device).",
                    args.signal_server
                );
            }
            _ => {}
        }
    }

    // Determine device ID
    let device_id = args.device_id.unwrap_or_else(|| {
        gethostname::gethostname()
            .into_string()
            .unwrap_or_else(|_| "default-node".to_string())
    });

    // Setup logging to file (since TUI takes over terminal). The default is
    // per device so several instances sharing a home don't clobber each other.
    let log_path = expand_home(
        &args
            .log_location
            .clone()
            .unwrap_or_else(|| format!("~/.frost_keystore/logs/starlab-mpc-{device_id}.log")),
    );
    if let Some(dir) = log_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    println!("Logging to: {}", log_path.display());

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true) // Start fresh each run
        .open(&log_path)
        .unwrap_or_else(|e| {
            eprintln!("Failed to create log file {}: {}", log_path.display(), e);
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
    info!("Log file: {}", log_path.display());
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
async fn run_elm_tui(
    device_id: String,
    signal_server: String,
    offline: bool,
) -> anyhow::Result<()> {
    use crossterm::{
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    };
    use std::io;

    info!("🚀 Starting Elm Architecture TUI");

    // Create app state with device ID and signal server URL
    let app_state = Arc::new(
        tokio::sync::Mutex::new(starlab_client::utils::appstate_compat::AppState::<
            Secp256K1Sha256TR,
        >::with_device_id_and_server(
            device_id.clone(), signal_server.clone()
        )),
    );

    // Create and initialize Elm app
    let mut elm_app = ElmApp::new(device_id.clone(), app_state.clone())?;

    // Initialize keystore automatically
    let keystore_path = format!(
        "{}/.frost_keystore",
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    );
    info!(
        "Initializing keystore at: {} for device: {}",
        keystore_path, device_id
    );

    // Initialize keystore in app state
    {
        let mut state = app_state.lock().await;
        match starlab_client::keystore::Keystore::new(&keystore_path, &device_id) {
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
            .send(starlab_client::elm::message::Message::TriggerReconnect)
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

/// Expand a leading `~/` to `$HOME` (same home resolution as the keystore).
fn expand_home(path: &str) -> std::path::PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => {
            std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(rest)
        }
        None => std::path::PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::expand_home;

    #[test]
    fn expands_leading_tilde_only() {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        assert_eq!(
            expand_home("~/logs/x.log"),
            std::path::Path::new(&home).join("logs/x.log")
        );
        assert_eq!(
            expand_home("/tmp/x.log"),
            std::path::PathBuf::from("/tmp/x.log")
        );
        assert_eq!(
            expand_home("rel/~/x.log"),
            std::path::PathBuf::from("rel/~/x.log")
        );
    }
}
