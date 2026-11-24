//! Clear-Signing screen layout formatter for OLED (128x64) and IPS (320x170) displays.
//!
//! Formats human-verifiable signing prompts so the user always sees the exact
//! wallet, chain, BIP-44/86 derivation path, signer address, optional amount /
//! recipient, and truncated 32-byte digest before pressing the physical button.

use starlab_hw_protocol::SignDisplayInfo;

/// Compact 6-line screen model suitable for both 0.96" OLED and 1.9" IPS displays.
#[derive(Debug, Clone)]
pub struct ClearSignScreen {
    pub lines: [String; 6],
}

impl ClearSignScreen {
    pub fn from_info(info: &SignDisplayInfo) -> Self {
        let header = format!(
            "SIGN {} ({})",
            info.chain.to_ascii_uppercase(),
            truncate_mid(&info.wallet_id, 10)
        );
        let amount_line = match &info.amount {
            Some(amt) => format!("Amt: {amt}"),
            None => format!("Path: {}", info.path),
        };
        let to_line = match &info.recipient {
            Some(r) => format!("To: {}", truncate_mid(r, 16)),
            None => format!("From: {}", truncate_mid(&info.signer_address, 14)),
        };
        let digest_line = format!("Hash: {}", truncate_mid(&info.message_digest_hex, 14));
        let summary_line = info
            .summary
            .as_deref()
            .map(|s| truncate_end(s, 20))
            .unwrap_or_else(|| format!("Addr: {}", truncate_mid(&info.signer_address, 14)));
        let footer = "[LEFT=OK] [RIGHT=NO]".to_string();

        Self {
            lines: [
                header,
                amount_line,
                to_line,
                digest_line,
                summary_line,
                footer,
            ],
        }
    }
}

fn truncate_mid(s: &str, max: usize) -> String {
    if s.len() <= max || max < 7 {
        return s.to_string();
    }
    let half = (max - 3) / 2;
    format!("{}...{}", &s[..half], &s[s.len() - half..])
}

fn truncate_end(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
