//! Starlab Key — ESP32-S3 / ESP32-C3 Hardware MPC Share entrypoint.
//!
//! Runs a clean NDJSON (SLHW v1) request/response loop over native USB-CDC
//! (`stdin`/`stdout`), delegating all FROST cryptography, encrypted NVS vault
//! storage, and physical clear-signing confirmation to `HardwareShareEngine`.

mod board;
mod display;
mod esp_hal;

use esp_hal::EspHardwareHal;
use starlab_hw_protocol::HardwareShareEngine;
use std::io::{self, BufRead, Write};

fn main() {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let hal = EspHardwareHal::init().expect("failed to initialize ESP32 hardware HAL");
    let mut engine = HardwareShareEngine::new(hal);

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Emit initial ready banner as a valid JSON event on boot.
    let boot_info = engine.handle_line(r#"{"cmd":"device_info"}"#);
    let _ = stdout.write_all(boot_info.as_bytes());
    let _ = stdout.flush();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        let resp = engine.handle_line(&line);
        let _ = stdout.write_all(resp.as_bytes());
        let _ = stdout.flush();
    }
}
