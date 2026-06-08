//! User-facing error guidance.
//!
//! Turns raw / internal error strings into messages an investor fumbling the
//! live demo can act on: *what failed + the most likely cause + the next thing
//! to try*. These wrap the upstream error at the display chokepoints (the
//! `DKGFailed` / `SigningFailed` / `WalletUnlockFailed` / connection handlers)
//! so we add guidance in one place instead of rewriting every error site.
//!
//! Classification is intentionally keyword-based and conservative: when we
//! recognise a cause we say something specific; otherwise we keep the raw error
//! and append general "what to check" guidance. We never hide the original text.

/// Guidance appended to a DKG (wallet-creation) failure.
pub fn dkg(error: &str) -> String {
    let e = error.to_lowercase();
    let hint = if e.contains("timeout") || e.contains("timed out") || e.contains("waiting") {
        "\n→ DKG needs ALL participants online together. Check every device used the SAME \
         signal server + room, a UNIQUE device id each, and that the network is up."
    } else if e.contains("connect") || e.contains("websocket") || e.contains("signal") || e.contains("offline") {
        "\n→ Couldn't reach the signal server. Check your network / the signal-server URL, or \
         use a LAN server (ws://<host-ip>:9000)."
    } else if e.contains("room") {
        "\n→ The hosted server needs a strong room (≥16 chars), the SAME on every device."
    } else if e.contains("duplicate") || e.contains("already registered") {
        "\n→ Two devices share a device id. Give each participant a UNIQUE device id and retry."
    } else {
        "\n→ Make sure all participants are online in the same room with unique device ids. \
         If it persists, rebuild to the latest and retry."
    };
    format!("DKG didn't finish: {error}{hint}")
}

/// Guidance appended to a threshold-signing failure.
pub fn signing(error: &str) -> String {
    let e = error.to_lowercase();
    let hint = if e.contains("timeout") || e.contains("timed out") || e.contains("waiting") {
        "\n→ Signing needs a quorum (the threshold) to approve. Make sure enough co-signers are \
         online in the same room and approved the request."
    } else if e.contains("password") || e.contains("decrypt") || e.contains("unlock") {
        "\n→ Wrong password for this wallet on this device. Re-enter the password you set here."
    } else if e.contains("connect") || e.contains("signal") || e.contains("offline") {
        "\n→ Couldn't reach the signal server / co-signers. Check the network and that everyone \
         is in the same room."
    } else {
        "\n→ Check that at least the threshold number of co-signers are online in the same room \
         and approved the request."
    };
    format!("Signing didn't complete: {error}{hint}")
}

/// Classify an unlock failure into a `(title, message)` an end user can act on.
pub fn unlock(error: &str) -> (String, String) {
    let e = error.to_lowercase();
    if e.contains("invalid password")
        || e.contains("wrong password")
        || e.contains("decrypt")
        || e.contains("aead")
        || e.contains("mac")
    {
        return (
            "Wrong password".to_string(),
            "That password didn't unlock this wallet. Enter the password you set for THIS wallet \
             on THIS device — each device has its own password."
                .to_string(),
        );
    }
    if e.contains("not found") || e.contains("no wallet") || e.contains("missing") {
        return (
            "Wallet not found".to_string(),
            "No wallet with that id was found on this device. Check you're using the right device \
             id and keystore, and the wallet id from your wallet list. A wallet's shares live only \
             on the devices that ran its DKG."
                .to_string(),
        );
    }
    (
        "Unlock failed".to_string(),
        format!(
            "Couldn't open the wallet: {error}\n→ Check the password is correct and the keystore \
             file isn't corrupted."
        ),
    )
}

/// Guidance shown when the signal-server connection isn't up. We don't always
/// have the URL at the call site, so this covers both common causes.
pub fn not_connected() -> String {
    "Not connected to the signal server.\n→ If you're using the hosted server, it requires a \
     strong room (≥16 chars), the SAME on every device — restart with --room <id>.\n→ Otherwise \
     check your network / the --signal-server URL, or run a local LAN server (ws://<host-ip>:9000)."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_detects_wrong_password() {
        let (title, _) = unlock("UnlockWallet: load_wallet_file failed: Invalid password");
        assert_eq!(title, "Wrong password");
    }

    #[test]
    fn unlock_detects_missing_wallet() {
        let (title, _) = unlock("wallet 'abc' not found in keystore");
        assert_eq!(title, "Wallet not found");
    }

    #[test]
    fn unlock_falls_back_but_keeps_raw() {
        let (title, msg) = unlock("some novel keystore failure");
        assert_eq!(title, "Unlock failed");
        assert!(msg.contains("some novel keystore failure"));
    }

    #[test]
    fn dkg_and_signing_keep_raw_and_add_hint() {
        let d = dkg("timed out after 90s");
        assert!(d.contains("timed out after 90s") && d.contains("→"));
        let s = signing("peer offline");
        assert!(s.contains("peer offline") && s.contains("→"));
    }
}
