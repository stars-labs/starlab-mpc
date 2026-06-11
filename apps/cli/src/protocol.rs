//! The JSON line protocol — the single source of truth for the CLI's
//! stdin commands and stdout events. Newline-delimited JSON (JSONL):
//! one object per line in, one per line out. Stable + versioned via the
//! `ready` event's `protocol` field.
//!
//! Commands are internally tagged on `cmd`; an optional top-level `id`
//! correlates a command with its terminal event (`correlates`). Events
//! are internally tagged on `event`.

use serde::{Deserialize, Serialize};

/// Bump on any breaking change to the command/event shapes.
pub const PROTOCOL_VERSION: u32 = 1;

/// A line read from stdin: an optional correlation `id` plus the command.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct CliRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub command: CliCommand,
}

/// Commands the front-end (LLM / test harness) can send.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum CliCommand {
    /// Connect to the configured signal server.
    Connect,
    /// Disconnect from the signal server.
    Disconnect,
    /// Request a full state snapshot (`status` event).
    Status,
    /// List wallets persisted in the keystore.
    ListWallets,
    /// List discovered (joinable) sessions.
    ListSessions,
    /// Create a new wallet as DKG creator. Announces a session and runs
    /// the ceremony once enough participants join.
    CreateWallet {
        #[serde(default)]
        name: String,
        threshold: u16,
        total: u16,
        /// "secp256k1" (Ethereum/EVM) or "ed25519" (Solana).
        #[serde(default = "default_curve")]
        curve: String,
        password: String,
    },
    /// Join a discovered DKG/signing session by id.
    JoinSession {
        session_id: String,
        password: String,
        #[serde(default)]
        label: String,
    },
    /// Initiate a threshold signing ceremony as the wallet owner.
    Sign {
        wallet_id: String,
        message: String,
        /// "utf8" (default) or "hex".
        #[serde(default = "default_encoding")]
        encoding: String,
        password: String,
    },
    /// Approve an incoming signing request by joining its session (a
    /// co-signer "approves" by contributing its share).
    ApproveSigning {
        session_id: String,
        password: String,
    },
    /// Initiate a networked share refresh/resharing of an existing wallet as
    /// the owner. Announces a reshare session the retained signers join; the
    /// group address is preserved and the refreshed share replaces the old one
    /// on disk (#56). Co-signers approve with `join_session` on the announced
    /// reshare session.
    Reshare {
        wallet_id: String,
        password: String,
    },
    /// Stop the runner and exit.
    Quit,
}

fn default_encoding() -> String {
    "utf8".to_string()
}

fn default_curve() -> String {
    "secp256k1".to_string()
}

/// Events emitted to stdout.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CliEvent {
    /// First line emitted on startup.
    Ready {
        protocol: u32,
        device_id: String,
        curve: String,
    },
    /// Immediate acknowledgement that a command with `id` was accepted.
    Ack { correlates: u64 },
    /// Signal-server connection state changed.
    Connection { connected: bool },
    /// Full snapshot (reply to `status`).
    Status {
        connected: bool,
        device_id: String,
        wallets: Vec<WalletEntry>,
    },
    /// A joinable session was discovered/announced.
    SessionAvailable { session: SessionEntry },
    /// This node created and announced a session (DKG/signing). Emitted as
    /// soon as the real session id is known, so a driver that issued
    /// `create_wallet` learns the generated id without scraping
    /// `session_available`. Carries `correlates` (the create command id).
    SessionAnnounced {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        correlates: Option<u64>,
        session_id: String,
    },
    /// Wallet list (reply to `list_wallets` / on change).
    Wallets { wallets: Vec<WalletEntry> },
    /// Sessions list (reply to `list_sessions`).
    Sessions { sessions: Vec<SessionEntry> },
    /// DKG round progress.
    DkgProgress {
        session_id: String,
        round: u8,
        received: usize,
        need: usize,
    },
    /// DKG finished and the wallet was persisted.
    DkgComplete {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        correlates: Option<u64>,
        wallet_id: String,
        #[serde(default)]
        address: String,
        group_public_key: String,
    },
    /// An incoming signing request we're a co-signer for (discovered).
    SigningRequest {
        session_id: String,
        wallet: String,
        threshold: u16,
        total: u16,
        proposer: String,
    },
    /// An incoming share-refresh/resharing request we're a participant for
    /// (discovered). A co-signer approves by joining the session (#45).
    ReshareRequest {
        session_id: String,
        wallet: String,
        threshold: u16,
        total: u16,
        proposer: String,
    },
    /// A share refresh/resharing ceremony finished. The wallet's group public
    /// key (and therefore its address) is unchanged; the refreshed share has
    /// been persisted, invalidating the old one (#45/#56).
    ReshareComplete {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        correlates: Option<u64>,
        wallet_id: String,
        group_public_key: String,
    },
    /// A threshold signing ceremony finished.
    SignatureComplete {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        correlates: Option<u64>,
        signature: String,
        message_hash: String,
    },
    /// HD account listing (reply to `wallet accounts`) — addresses for the
    /// pinned standard paths, derived from PUBLIC key material only.
    Accounts {
        wallet_id: String,
        accounts: Vec<AccountEntry>,
    },
    /// HD child addresses derived from a wallet (reply to `wallet derive`).
    DerivedAddresses {
        wallet_id: String,
        path: String,
        /// Deterministic child wallet id (same on every participant).
        child_id: String,
        addresses: Vec<ChainAddress>,
        /// Whether the child share was persisted to the keystore (--save).
        saved: bool,
    },
    /// Something went wrong.
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        correlates: Option<u64>,
        code: String,
        message: String,
    },
}

/// One HD account: a fixed index whose per-chain addresses come from the
/// pinned standard derivation paths.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountEntry {
    pub account: u32,
    pub addresses: Vec<ChainAddress>,
}

/// One chain's address for a wallet.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ChainAddress {
    pub chain: String,
    pub address: String,
    /// BIP-44 derivation path the address came from (pinned standard paths;
    /// `None` only for legacy emitters that predate the account model).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// UI-facing wallet summary (never leaks internal crypto types).
///
/// Wallet-centric: ONE entry per wallet. A wallet's key material may span
/// multiple curves (the unified DKG yields ed25519 + secp256k1 from one
/// ceremony), and each curve controls several chains — `curves` lists the
/// key shares held, `addresses` the derived per-chain addresses
/// (secp256k1 → Ethereum + Bitcoin; ed25519 → Solana + Sui).
/// `chain`/`address` remain as the PRIMARY pair for driver compatibility.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WalletEntry {
    pub id: String,
    pub name: String,
    pub address: String,
    pub chain: String,
    pub threshold: String,
    #[serde(default)]
    pub curves: Vec<String>,
    #[serde(default)]
    pub addresses: Vec<ChainAddress>,
}

/// UI-facing session summary.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SessionEntry {
    pub session_id: String,
    #[serde(rename = "type")]
    pub session_type: String,
    pub threshold: u16,
    pub total: u16,
    pub proposer: String,
    pub participants: Vec<String>,
}

impl CliEvent {
    /// Serialize to a single JSONL line (no trailing newline).
    pub fn to_line(&self) -> String {
        // Our types are plain data → serialization can't fail in practice;
        // fall back to a hand-built error line if it ever does.
        serde_json::to_string(self).unwrap_or_else(|e| {
            format!(r#"{{"event":"error","code":"serialize","message":"{e}"}}"#)
        })
    }
}

/// A machine-readable catalog of commands and events, for LLM/agent
/// self-discovery (`starlab-cli schema`). Kept hand-curated and terse;
/// the authoritative shapes are the serde types above.
pub fn schema_json() -> String {
    let schema = serde_json::json!({
        "protocol": PROTOCOL_VERSION,
        "transport": "newline-delimited JSON on stdin (commands) / stdout (events); logs on stderr",
        "commands": [
            {"cmd": "connect"},
            {"cmd": "disconnect"},
            {"cmd": "status"},
            {"cmd": "list_wallets"},
            {"cmd": "list_sessions"},
            {"cmd": "create_wallet", "fields": {"name?": "string", "threshold": "u16", "total": "u16", "curve?": "secp256k1|ed25519", "password": "string"}},
            {"cmd": "join_session", "fields": {"session_id": "string", "password": "string", "label?": "string"}},
            {"cmd": "sign", "fields": {"wallet_id": "string", "message": "string", "encoding?": "utf8|hex", "password": "string"}},
            {"cmd": "approve_signing", "fields": {"session_id": "string", "password": "string"}},
            {"cmd": "reshare", "fields": {"wallet_id": "string", "password": "string"}},
            {"cmd": "quit"},
            {"_note": "every command may include an `id` (u64); long-running ones echo it back as `correlates`"}
        ],
        "events": [
            {"event": "ready", "fields": {"protocol": "u32", "device_id": "string", "curve": "string"}},
            {"event": "ack", "fields": {"correlates": "u64"}},
            {"event": "connection", "fields": {"connected": "bool"}},
            {"event": "status", "fields": {"connected": "bool", "device_id": "string", "wallets": "[WalletEntry]"}},
            {"event": "session_available", "fields": {"session": "SessionEntry"}},
            {"event": "session_announced", "fields": {"correlates?": "u64", "session_id": "string"}},
            {"event": "wallets", "fields": {"wallets": "[WalletEntry]"}},
            {"event": "sessions", "fields": {"sessions": "[SessionEntry]"}},
            {"event": "dkg_progress", "fields": {"session_id": "string", "round": "u8", "received": "usize", "need": "usize"}},
            {"event": "dkg_complete", "fields": {"correlates?": "u64", "wallet_id": "string", "address": "string", "group_public_key": "string"}},
            {"event": "signing_request", "fields": {"session_id": "string", "wallet": "string", "threshold": "u16", "total": "u16", "proposer": "string"}},
            {"event": "reshare_request", "fields": {"session_id": "string", "wallet": "string", "threshold": "u16", "total": "u16", "proposer": "string"}},
            {"event": "signature_complete", "fields": {"correlates?": "u64", "signature": "string", "message_hash": "string"}},
            {"event": "reshare_complete", "fields": {"correlates?": "u64", "wallet_id": "string", "group_public_key": "string"}},
            {"event": "error", "fields": {"correlates?": "u64", "code": "string", "message": "string"}}
        ]
    });
    serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req_round_trip(line: &str) -> CliRequest {
        let parsed: CliRequest = serde_json::from_str(line).expect("parse request");
        let reser = serde_json::to_string(&parsed).expect("reserialize");
        let reparsed: CliRequest = serde_json::from_str(&reser).expect("reparse");
        assert_eq!(parsed, reparsed, "request round-trip mismatch");
        parsed
    }

    #[test]
    fn parses_create_wallet_with_id() {
        let r = req_round_trip(
            r#"{"id":3,"cmd":"create_wallet","name":"t","threshold":2,"total":3,"curve":"secp256k1","password":"pw"}"#,
        );
        assert_eq!(r.id, Some(3));
        match r.command {
            CliCommand::CreateWallet {
                name,
                threshold,
                total,
                curve,
                password,
            } => {
                assert_eq!(name, "t");
                assert_eq!((threshold, total), (2, 3));
                assert_eq!(curve, "secp256k1");
                assert_eq!(password, "pw");
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn curve_defaults_to_secp256k1() {
        let r: CliRequest = serde_json::from_str(
            r#"{"cmd":"create_wallet","threshold":2,"total":3,"password":"pw"}"#,
        )
        .unwrap();
        assert!(r.id.is_none());
        match r.command {
            CliCommand::CreateWallet { curve, name, .. } => {
                assert_eq!(curve, "secp256k1");
                assert_eq!(name, "");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parses_simple_commands() {
        for (line, want) in [
            (r#"{"cmd":"connect"}"#, CliCommand::Connect),
            (r#"{"cmd":"status"}"#, CliCommand::Status),
            (r#"{"cmd":"list_wallets"}"#, CliCommand::ListWallets),
            (r#"{"cmd":"quit"}"#, CliCommand::Quit),
        ] {
            let r: CliRequest = serde_json::from_str(line).unwrap();
            assert_eq!(r.command, want);
        }
    }

    #[test]
    fn join_session_label_optional() {
        let r: CliRequest =
            serde_json::from_str(r#"{"cmd":"join_session","session_id":"s","password":"p"}"#)
                .unwrap();
        match r.command {
            CliCommand::JoinSession { label, session_id, .. } => {
                assert_eq!(session_id, "s");
                assert_eq!(label, "");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn events_serialize_to_expected_json() {
        let ready = CliEvent::Ready {
            protocol: PROTOCOL_VERSION,
            device_id: "node-a".into(),
            curve: "secp256k1".into(),
        };
        let v: serde_json::Value = serde_json::from_str(&ready.to_line()).unwrap();
        assert_eq!(v["event"], "ready");
        assert_eq!(v["protocol"], 1);
        assert_eq!(v["device_id"], "node-a");

        let ack = CliEvent::Ack { correlates: 7 };
        let v: serde_json::Value = serde_json::from_str(&ack.to_line()).unwrap();
        assert_eq!(v["event"], "ack");
        assert_eq!(v["correlates"], 7);

        let done = CliEvent::DkgComplete {
            correlates: Some(3),
            wallet_id: "w".into(),
            address: "0xabc".into(),
            group_public_key: "deadbeef".into(),
        };
        let v: serde_json::Value = serde_json::from_str(&done.to_line()).unwrap();
        assert_eq!(v["event"], "dkg_complete");
        assert_eq!(v["correlates"], 3);
        assert_eq!(v["wallet_id"], "w");
    }

    #[test]
    fn session_announced_omits_correlates_when_none() {
        let ann = CliEvent::SessionAnnounced {
            correlates: None,
            session_id: "dkg_42".into(),
        };
        let v: serde_json::Value = serde_json::from_str(&ann.to_line()).unwrap();
        assert_eq!(v["event"], "session_announced");
        assert_eq!(v["session_id"], "dkg_42");
        assert!(v.get("correlates").is_none(), "None correlates must be omitted");

        let ann = CliEvent::SessionAnnounced {
            correlates: Some(9),
            session_id: "dkg_42".into(),
        };
        let v: serde_json::Value = serde_json::from_str(&ann.to_line()).unwrap();
        assert_eq!(v["correlates"], 9);
    }

    #[test]
    fn unknown_command_is_an_error() {
        let r: Result<CliRequest, _> = serde_json::from_str(r#"{"cmd":"frobnicate"}"#);
        assert!(r.is_err(), "unknown command must fail to parse");
    }
}
