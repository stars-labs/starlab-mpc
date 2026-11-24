//! Hardware Abstraction Layer (`HardwareHal`) separating the pure-Rust FROST
//! hardware-share engine from board-specific ESP32 drivers (TRNG, NVS Flash,
//! eFuse HMAC key, OLED/IPS screen, and physical buttons).

use crate::protocol::SignDisplayInfo;
use starlab_core::rng as frost_rng;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HalError {
    #[error("hardware RNG failure: {0}")]
    Rng(String),
    #[error("NVS storage failure: {0}")]
    Storage(String),
    #[error("display/button UI failure: {0}")]
    Ui(String),
}

/// Board driver contract implemented by both real ESP32 firmware (`esp_hal.rs`)
/// and the in-memory host simulator (`SimHal`).
pub trait HardwareHal {
    fn board_name(&self) -> &'static str;
    fn chip_name(&self) -> &'static str;
    fn firmware_version(&self) -> &'static str {
        crate::DEFAULT_FIRMWARE_VERSION
    }

    /// Fill `dest` from the chip's hardware TRNG (`esp_fill_random` on ESP32).
    fn fill_random(&mut self, dest: &mut [u8]) -> Result<(), HalError>;

    /// Per-device 32-byte hardware secret (e.g. derived from an ESP32 eFuse
    /// `HMAC_KEY` slot) mixed into the PIN KDF so offline Flash dumps without
    /// eFuse access cannot brute-force the PIN.
    fn device_hw_secret(&self) -> [u8; 32];

    /// Read a blob from persistent NVS Flash.
    fn nvs_get(&self, key: &str) -> Result<Option<Vec<u8>>, HalError>;
    /// Write a blob to persistent NVS Flash (must flush/commit before returning).
    fn nvs_set(&mut self, key: &str, value: &[u8]) -> Result<(), HalError>;
    /// Remove a key from persistent NVS Flash.
    fn nvs_remove(&mut self, key: &str) -> Result<(), HalError>;
    /// List all stored keys beginning with `prefix`.
    fn nvs_list_prefix(&self, prefix: &str) -> Result<Vec<String>, HalError>;

    /// Clear-Signing gate: render `info` on the device screen and block until
    /// the user presses either the physical APPROVE button (`Ok(true)`) or
    /// REJECT button (`Ok(false)`).
    fn confirm_signing(&mut self, info: &SignDisplayInfo) -> Result<bool, HalError>;

    /// Physical confirmation gate for destructive operations (e.g. deleting a
    /// wallet share).
    fn confirm_destructive(&mut self, title: &str, detail: &str) -> Result<bool, HalError>;

    /// Non-blocking status update for the OLED/IPS display.
    fn show_status(&mut self, title: &str, subtitle: &str);
}

/// Bridges [`HardwareHal::fill_random`] directly to `frost-core 3.0`'s
/// `rand_core 0.6` (`RngCore + CryptoRng`) traits so all FROST DKG & signing
/// nonces come directly from hardware TRNG.
pub struct HalFrostRng<'a, H: HardwareHal> {
    hal: &'a mut H,
}

impl<'a, H: HardwareHal> HalFrostRng<'a, H> {
    pub fn new(hal: &'a mut H) -> Self {
        Self { hal }
    }
}

impl<H: HardwareHal> frost_rng::RngCore for HalFrostRng<'_, H> {
    fn next_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        self.fill_bytes(&mut buf);
        u32::from_le_bytes(buf)
    }

    fn next_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        self.fill_bytes(&mut buf);
        u64::from_le_bytes(buf)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.hal
            .fill_random(dest)
            .expect("hardware TRNG must not fail");
    }

    fn try_fill_bytes(
        &mut self,
        dest: &mut [u8],
    ) -> Result<(), frost_ed25519::rand_core::Error> {
        self.hal.fill_random(dest).map_err(|_| {
            // Non-zero custom error code per rand_core 0.6 convention
            let code = std::num::NonZeroU32::new(frost_ed25519::rand_core::Error::CUSTOM_START)
                .expect("non-zero");
            frost_ed25519::rand_core::Error::from(code)
        })
    }
}

impl<H: HardwareHal> frost_rng::CryptoRng for HalFrostRng<'_, H> {}
