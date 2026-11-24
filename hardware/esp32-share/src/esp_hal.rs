//! Real ESP-IDF implementation of [`starlab_hw_protocol::HardwareHal`].
//!
//! Connects the transport-agnostic FROST hardware-share engine to:
//! - Hardware TRNG (`esp_fill_random`)
//! - Persistent NVS Flash (`EspNvs<NvsDefault>`) with automatic 15-char key
//!   hashing + index table (working around ESP-IDF's 15-byte NVS key limit)
//! - Hardware device secret derived from chip eFuse MAC / HMAC key slot
//! - Physical active-low Approve / Reject GPIO buttons with 40ms debounce and
//!   60-second automatic timeout rejection

use crate::board::BOARD;
use crate::display::ClearSignScreen;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use sha2::{Digest, Sha256};
use starlab_hw_protocol::{HalError, HardwareHal, SignDisplayInfo};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

const NVS_NAMESPACE: &str = "starlab_hw";
const NVS_INDEX_KEY: &str = "_idx";
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(60);

pub struct EspHardwareHal {
    nvs: EspNvs<NvsDefault>,
    hw_secret: [u8; 32],
}

impl EspHardwareHal {
    pub fn init() -> Result<Self, HalError> {
        let partition = EspDefaultNvsPartition::take()
            .map_err(|e| HalError::Storage(format!("NVS partition init failed: {e}")))?;
        let nvs = EspNvs::new(partition, NVS_NAMESPACE, true)
            .map_err(|e| HalError::Storage(format!("NVS namespace open failed: {e}")))?;

        configure_button_gpio(BOARD.btn_approve_gpio)?;
        configure_button_gpio(BOARD.btn_reject_gpio)?;

        let hw_secret = derive_chip_hw_secret();
        Ok(Self { nvs, hw_secret })
    }

    /// ESP-IDF NVS limits key names to 15 ASCII chars; map arbitrary logical
    /// keys deterministically to a 14-hex-char NVS slot name.
    fn nvs_slot_key(logical_key: &str) -> String {
        let digest = Sha256::digest(logical_key.as_bytes());
        format!("k{}", &hex::encode(digest)[..13])
    }

    fn load_key_index(&self) -> Result<BTreeSet<String>, HalError> {
        let mut buf = [0u8; 4096];
        match self.nvs.get_blob(NVS_INDEX_KEY, &mut buf) {
            Ok(Some(bytes)) => serde_json::from_slice(bytes)
                .map_err(|e| HalError::Storage(format!("corrupt NVS key index: {e}"))),
            Ok(None) => Ok(BTreeSet::new()),
            Err(e) => Err(HalError::Storage(format!("NVS index read: {e}"))),
        }
    }

    fn save_key_index(&mut self, index: &BTreeSet<String>) -> Result<(), HalError> {
        let bytes = serde_json::to_vec(index)
            .map_err(|e| HalError::Storage(format!("NVS index encode: {e}")))?;
        self.nvs
            .set_blob(NVS_INDEX_KEY, &bytes)
            .map_err(|e| HalError::Storage(format!("NVS index write: {e}")))
    }

    /// Poll physical Approve / Reject buttons with 40ms debounce until pressed
    /// or `CONFIRM_TIMEOUT` expires (timeout defaults to REJECT for safety).
    fn wait_physical_button(&self) -> bool {
        let start = Instant::now();
        while start.elapsed() < CONFIRM_TIMEOUT {
            let approve_low = read_gpio_low(BOARD.btn_approve_gpio);
            let reject_low = read_gpio_low(BOARD.btn_reject_gpio);

            if approve_low && !reject_low {
                FreeRtos::delay_ms(40);
                if read_gpio_low(BOARD.btn_approve_gpio) {
                    // Wait for release so a single long press can't approve twice.
                    while read_gpio_low(BOARD.btn_approve_gpio) {
                        FreeRtos::delay_ms(15);
                    }
                    return true;
                }
            }
            if reject_low && !approve_low {
                FreeRtos::delay_ms(40);
                if read_gpio_low(BOARD.btn_reject_gpio) {
                    while read_gpio_low(BOARD.btn_reject_gpio) {
                        FreeRtos::delay_ms(15);
                    }
                    return false;
                }
            }
            FreeRtos::delay_ms(15);
        }
        // Fail closed on timeout.
        false
    }
}

impl HardwareHal for EspHardwareHal {
    fn board_name(&self) -> &'static str {
        BOARD.name
    }

    fn chip_name(&self) -> &'static str {
        BOARD.chip
    }

    fn fill_random(&mut self, dest: &mut [u8]) -> Result<(), HalError> {
        // Uses ESP32 hardware TRNG (SAR ADC / RF entropy source).
        unsafe {
            esp_idf_sys::esp_fill_random(dest.as_mut_ptr() as *mut core::ffi::c_void, dest.len());
        }
        Ok(())
    }

    fn device_hw_secret(&self) -> [u8; 32] {
        self.hw_secret
    }

    fn nvs_get(&self, key: &str) -> Result<Option<Vec<u8>>, HalError> {
        let slot = Self::nvs_slot_key(key);
        let len = self
            .nvs
            .blob_len(&slot)
            .map_err(|e| HalError::Storage(format!("NVS blob_len({key}): {e}")))?;
        let Some(len) = len else {
            return Ok(None);
        };
        let mut buf = vec![0u8; len];
        let Some(slice) = self
            .nvs
            .get_blob(&slot, &mut buf)
            .map_err(|e| HalError::Storage(format!("NVS get({key}): {e}")))?
        else {
            return Ok(None);
        };
        Ok(Some(slice.to_vec()))
    }

    fn nvs_set(&mut self, key: &str, value: &[u8]) -> Result<(), HalError> {
        let slot = Self::nvs_slot_key(key);
        self.nvs
            .set_blob(&slot, value)
            .map_err(|e| HalError::Storage(format!("NVS set({key}): {e}")))?;
        let mut idx = self.load_key_index()?;
        if idx.insert(key.to_string()) {
            self.save_key_index(&idx)?;
        }
        Ok(())
    }

    fn nvs_remove(&mut self, key: &str) -> Result<(), HalError> {
        let slot = Self::nvs_slot_key(key);
        let _ = self.nvs.remove(&slot);
        let mut idx = self.load_key_index()?;
        if idx.remove(key) {
            self.save_key_index(&idx)?;
        }
        Ok(())
    }

    fn nvs_list_prefix(&self, prefix: &str) -> Result<Vec<String>, HalError> {
        let idx = self.load_key_index()?;
        Ok(idx
            .into_iter()
            .filter(|k| k.starts_with(prefix))
            .collect())
    }

    fn confirm_signing(&mut self, info: &SignDisplayInfo) -> Result<bool, HalError> {
        let screen = ClearSignScreen::from_info(info);
        // Log screen lines to stderr (leaving stdout clean for NDJSON) and hook
        // board-specific ST7789 / SSD1306 display driver here.
        for line in &screen.lines {
            log::info!("[SCREEN] {line}");
        }
        Ok(self.wait_physical_button())
    }

    fn confirm_destructive(&mut self, title: &str, detail: &str) -> Result<bool, HalError> {
        log::warn!("[SCREEN] {title} | {detail} | [LEFT=OK] [RIGHT=NO]");
        Ok(self.wait_physical_button())
    }

    fn show_status(&mut self, title: &str, subtitle: &str) {
        log::info!("[SCREEN] {title}: {subtitle}");
    }
}

/// Derive a 32-byte per-chip hardware secret.
///
/// In production builds (`sdkconfig.release`), burn a random 256-bit key into
/// `EFUSE_BLK_KEY0` with purpose `HMAC_UP` and replace the MAC fallback with
/// `esp_hmac_calculate(HMAC_KEY0, b"starlab-hw-v1", ...)`.
fn derive_chip_hw_secret() -> [u8; 32] {
    let mut mac = [0u8; 6];
    unsafe {
        let _ = esp_idf_sys::esp_efuse_mac_get_default(mac.as_mut_ptr());
    }
    let mut hasher = Sha256::new();
    hasher.update(b"starlab-hw/v1/chip-secret/");
    hasher.update(mac);
    hasher.finalize().into()
}

fn configure_button_gpio(pin: i32) -> Result<(), HalError> {
    unsafe {
        let cfg = esp_idf_sys::gpio_config_t {
            pin_bit_mask: 1u64 << (pin as u32),
            mode: esp_idf_sys::gpio_mode_t_GPIO_MODE_INPUT,
            pull_up_en: esp_idf_sys::gpio_pullup_t_GPIO_PULLUP_ENABLE,
            pull_down_en: esp_idf_sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: esp_idf_sys::gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        let err = esp_idf_sys::gpio_config(&cfg);
        if err != esp_idf_sys::ESP_OK {
            return Err(HalError::Ui(format!("gpio_config({pin}) failed: {err}")));
        }
    }
    Ok(())
}

fn read_gpio_low(pin: i32) -> bool {
    unsafe { esp_idf_sys::gpio_get_level(pin) == 0 }
}
