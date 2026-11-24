//! Board-specific pinout definitions selected via Cargo features.
//!
//! Supported off-the-shelf boards:
//! - `board-t-display-s3` (default): LilyGO T-Display-S3 (ESP32-S3 N16R8, 1.9" IPS, 2 buttons)
//! - `board-xiao-c3`: Seeed XIAO ESP32-C3 / SuperMini + 0.96" I2C OLED + 2 buttons
//! - `board-cyd-s3`: Cheap Yellow Display 2.8" ESP32-S3 + MicroSD air-gap slot
//! - `board-devkit-s3`: Generic ESP32-S3-DevKitC-1 + optional I2C OLED

pub struct BoardConfig {
    pub name: &'static str,
    pub chip: &'static str,
    /// Active-low GPIO number for physical APPROVE button.
    pub btn_approve_gpio: i32,
    /// Active-low GPIO number for physical REJECT button.
    pub btn_reject_gpio: i32,
    /// Optional I2C OLED pins `(sda, scl)`.
    pub i2c_oled: Option<(i32, i32)>,
    /// Whether the board includes an onboard MicroSD SPI slot for air-gap mode.
    pub has_sd_card: bool,
}

#[cfg(feature = "board-t-display-s3")]
pub const BOARD: BoardConfig = BoardConfig {
    name: "lilygo-t-display-s3",
    chip: "ESP32-S3",
    btn_approve_gpio: 0,  // Left / BOOT button
    btn_reject_gpio: 14,  // Right user button
    i2c_oled: None,       // Uses onboard 1.9" ST7789 8-bit parallel IPS LCD
    has_sd_card: false,
};

#[cfg(feature = "board-xiao-c3")]
pub const BOARD: BoardConfig = BoardConfig {
    name: "xiao-esp32c3-oled",
    chip: "ESP32-C3",
    btn_approve_gpio: 2,
    btn_reject_gpio: 3,
    i2c_oled: Some((6, 7)), // SDA=GPIO6, SCL=GPIO7 (SSD1306 128x64)
    has_sd_card: false,
};

#[cfg(feature = "board-cyd-s3")]
pub const BOARD: BoardConfig = BoardConfig {
    name: "cyd-esp32s3-28",
    chip: "ESP32-S3",
    btn_approve_gpio: 0,
    btn_reject_gpio: 4,
    i2c_oled: None, // Uses onboard 2.8" SPI LCD
    has_sd_card: true,
};

#[cfg(feature = "board-devkit-s3")]
pub const BOARD: BoardConfig = BoardConfig {
    name: "esp32s3-devkitc-1",
    chip: "ESP32-S3",
    btn_approve_gpio: 0, // BOOT button
    btn_reject_gpio: 4,  // External tactile switch to GND
    i2c_oled: Some((8, 9)),
    has_sd_card: false,
};
