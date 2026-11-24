//! Interactive virtual ESP32-S3 hardware share simulator over stdin/stdout NDJSON.
//!
//! Run with:
//! ```bash
//! cargo run -p starlab-hw-protocol --example esp32_sim_bridge
//! ```
//! Then paste SLHW v1 JSON lines (one per line), e.g.:
//! ```json
//! {"id":1,"cmd":"device_info"}
//! {"id":2,"cmd":"set_pin","pin":"123456"}
//! ```

use starlab_hw_protocol::{HardwareShareEngine, SimHal};
use std::io::{self, BufRead, Write};

fn main() {
    let mut engine = HardwareShareEngine::new(SimHal::new_auto_approve());
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    eprintln!("[starlab-esp32-sim] SLHW v1 ready on stdin/stdout (auto-approve button mode)");
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let resp = engine.handle_line(&line);
        if stdout.write_all(resp.as_bytes()).is_err() || stdout.flush().is_err() {
            break;
        }
    }
}
