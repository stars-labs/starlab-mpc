//! In-memory simulated [`HardwareHal`] for host unit/interop tests, CI
//! conformance checks, and interactive CLI simulation without physical ESP32
//! hardware.

use crate::hal::{HalError, HardwareHal};
use crate::protocol::SignDisplayInfo;
use std::collections::{BTreeMap, VecDeque};

/// Scripted or automatic physical button policy for [`SimHal`].
#[derive(Debug, Clone)]
pub enum ButtonPolicy {
    /// Always press physical APPROVE.
    AlwaysApprove,
    /// Always press physical REJECT.
    AlwaysReject,
    /// Pop one decision per confirmation prompt (falls back to `false` when empty).
    Scripted(VecDeque<bool>),
}

/// Virtual ESP32 board backed by in-memory NVS Flash and OS CSPRNG.
pub struct SimHal {
    board: &'static str,
    chip: &'static str,
    hw_secret: [u8; 32],
    nvs: BTreeMap<String, Vec<u8>>,
    button_policy: ButtonPolicy,
    /// Every signing prompt displayed on the simulated screen (newest last).
    pub displayed_prompts: Vec<SignDisplayInfo>,
    /// Status lines rendered to the simulated screen.
    pub status_log: Vec<(String, String)>,
}

impl Default for SimHal {
    fn default() -> Self {
        Self::new_auto_approve()
    }
}

impl SimHal {
    pub fn new_auto_approve() -> Self {
        Self {
            board: "lilygo-t-display-s3-sim",
            chip: "ESP32-S3 (Sim)",
            hw_secret: [0x5a; 32],
            nvs: BTreeMap::new(),
            button_policy: ButtonPolicy::AlwaysApprove,
            displayed_prompts: Vec::new(),
            status_log: Vec::new(),
        }
    }

    pub fn with_policy(mut self, policy: ButtonPolicy) -> Self {
        self.button_policy = policy;
        self
    }

    pub fn set_policy(&mut self, policy: ButtonPolicy) {
        self.button_policy = policy;
    }

    pub fn with_hw_secret(mut self, secret: [u8; 32]) -> Self {
        self.hw_secret = secret;
        self
    }

    /// Inspect raw NVS Flash contents (useful in security tests to verify that
    /// secret shares are never written in plaintext).
    pub fn raw_nvs(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.nvs
    }

    fn next_button_decision(&mut self) -> bool {
        match &mut self.button_policy {
            ButtonPolicy::AlwaysApprove => true,
            ButtonPolicy::AlwaysReject => false,
            ButtonPolicy::Scripted(q) => q.pop_front().unwrap_or(false),
        }
    }
}

impl HardwareHal for SimHal {
    fn board_name(&self) -> &'static str {
        self.board
    }

    fn chip_name(&self) -> &'static str {
        self.chip
    }

    fn fill_random(&mut self, dest: &mut [u8]) -> Result<(), HalError> {
        use starlab_core::rng::RngCore as _;
        let mut rng = starlab_core::rng::os_rng();
        rng.fill_bytes(dest);
        Ok(())
    }

    fn device_hw_secret(&self) -> [u8; 32] {
        self.hw_secret
    }

    fn nvs_get(&self, key: &str) -> Result<Option<Vec<u8>>, HalError> {
        Ok(self.nvs.get(key).cloned())
    }

    fn nvs_set(&mut self, key: &str, value: &[u8]) -> Result<(), HalError> {
        self.nvs.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn nvs_remove(&mut self, key: &str) -> Result<(), HalError> {
        self.nvs.remove(key);
        Ok(())
    }

    fn nvs_list_prefix(&self, prefix: &str) -> Result<Vec<String>, HalError> {
        Ok(self
            .nvs
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }

    fn confirm_signing(&mut self, info: &SignDisplayInfo) -> Result<bool, HalError> {
        self.displayed_prompts.push(info.clone());
        Ok(self.next_button_decision())
    }

    fn confirm_destructive(&mut self, title: &str, detail: &str) -> Result<bool, HalError> {
        self.status_log
            .push((format!("CONFIRM: {title}"), detail.to_string()));
        Ok(self.next_button_decision())
    }

    fn show_status(&mut self, title: &str, subtitle: &str) {
        self.status_log
            .push((title.to_string(), subtitle.to_string()));
    }
}
