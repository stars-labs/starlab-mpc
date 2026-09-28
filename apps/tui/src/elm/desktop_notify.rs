//! Desktop notifications for events that need the user while the terminal
//! is not focused: an incoming signing request, DKG done / failed, signing
//! done / failed.
//!
//! Best-effort by design. On Linux `notify-rust` talks to the
//! `org.freedesktop.Notifications` service over the D-Bus session bus (pure
//! Rust zbus, no libdbus). Over SSH, on a headless box, or on an air-gapped
//! machine there is no session bus — the failure is logged at debug and the
//! TUI carries on. The call runs on its own thread so a slow or hung bus
//! never stalls the update loop.

use std::thread::JoinHandle;
use tracing::debug;

const APP_NAME: &str = "Starlab";

/// Send `summary` / `body` as a desktop notification if `enabled`, off the
/// calling thread. Returns the worker's handle when a send was dispatched
/// (callers normally drop it; tests join it).
pub fn send(enabled: bool, summary: &str, body: &str) -> Option<JoinHandle<()>> {
    if !enabled {
        return None;
    }
    let (summary, body) = (summary.to_string(), body.to_string());
    std::thread::Builder::new()
        .name("desktop-notify".into())
        .spawn(move || {
            if let Err(e) = show_blocking(&summary, &body) {
                debug!("desktop notification not shown ({summary}): {e}");
            }
        })
        .inspect_err(|e| debug!("desktop notification thread not started: {e}"))
        .ok()
}

/// Show the notification on the current thread. Blocks on the bus
/// round-trip; errors when there is no notification service.
pub fn show_blocking(summary: &str, body: &str) -> Result<(), notify_rust::error::Error> {
    notify_rust::Notification::new()
        .appname(APP_NAME)
        .summary(summary)
        .body(body)
        .show()
        .map(|_| ())
}
