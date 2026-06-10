//! Protocol-trace normalization (Phase 2, L2 of docs/cli-conformance-testing.md).
//!
//! Golden fixtures pin the *shape* of the JSONL event contract — the field
//! names, tags, and structure clients depend on — without pinning values that
//! legitimately vary run-to-run (session ids, group keys, addresses,
//! signatures, …). [`normalize_event`] replaces those volatile values with
//! stable, self-describing placeholders (`"<group_public_key>"`) while keeping
//! every structural/enum/numeric field literal. The result is a deterministic
//! JSON value that a golden file can pin and the L4 differential oracle can
//! compare the browser extension's emitted protocol against.

use serde_json::Value;

use crate::protocol::CliEvent;

/// Object keys whose *values* are volatile (per-run crypto material or ids) and
/// must be redacted to a placeholder before comparison.
const VOLATILE_SCALARS: &[&str] = &[
    "session_id",
    // WalletEntry.id is the per-run wallet/session id.
    "id",
    "group_public_key",
    "address",
    "signature",
    "message_hash",
    "device_id",
    "wallet_id",
    "wallet",
    "proposer",
    // Error text is dynamic (file paths, underlying messages).
    "message",
];

/// Normalize one event to its shape: serialize, then recursively replace
/// volatile scalar values with `"<key>"` placeholders and any `participants`
/// list with a single `"<device>"` marker. Structural fields (`event`, `type`,
/// `protocol`, `connected`, `threshold`, `total`, `round`, `received`, `need`,
/// `code`, `correlates`, `name`, `chain`, `curve`) are kept literal.
pub fn normalize_event(ev: &CliEvent) -> Value {
    let mut v = serde_json::to_value(ev).unwrap_or(Value::Null);
    redact(&mut v);
    v
}

/// JSONL line form of [`normalize_event`] (compact, no trailing newline).
pub fn normalize_event_line(ev: &CliEvent) -> String {
    serde_json::to_string(&normalize_event(ev)).unwrap_or_default()
}

fn redact(v: &mut Value) {
    match v {
        Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                if k == "participants" {
                    // Collapse the (variable-length, id-bearing) list to one
                    // marker so the shape is pinned without the membership.
                    *val = Value::Array(vec![Value::String("<device>".into())]);
                } else if VOLATILE_SCALARS.contains(&k.as_str()) && val.is_string() {
                    *val = Value::String(format!("<{k}>"));
                } else {
                    redact(val);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{SessionEntry, WalletEntry};

    #[test]
    fn redacts_volatile_keeps_structure() {
        let ev = CliEvent::DkgComplete {
            correlates: Some(3),
            wallet_id: "wallet-dkg_abcd".into(),
            address: "0xdeadbeef".into(),
            group_public_key: "0299aabb".into(),
        };
        let v = normalize_event(&ev);
        assert_eq!(v["event"], "dkg_complete");
        assert_eq!(v["correlates"], 3, "structural field stays literal");
        assert_eq!(v["wallet_id"], "<wallet_id>");
        assert_eq!(v["address"], "<address>");
        assert_eq!(v["group_public_key"], "<group_public_key>");
    }

    #[test]
    fn normalization_is_value_independent() {
        // Two runs differ only in volatile values → identical normal forms.
        let a = CliEvent::SignatureComplete {
            correlates: Some(1),
            signature: "0xaaaa".into(),
            message_hash: "0x1111".into(),
        };
        let b = CliEvent::SignatureComplete {
            correlates: Some(1),
            signature: "0xffff0000".into(),
            message_hash: "0x2222".into(),
        };
        assert_eq!(normalize_event(&a), normalize_event(&b));
    }

    #[test]
    fn participants_collapsed_to_marker() {
        let ev = CliEvent::SessionAvailable {
            session: SessionEntry {
                session_id: "dkg_x".into(),
                session_type: "dkg".into(),
                threshold: 2,
                total: 3,
                proposer: "node-a".into(),
                participants: vec!["node-a".into(), "node-b".into()],
            },
        };
        let v = normalize_event(&ev);
        assert_eq!(v["session"]["session_id"], "<session_id>");
        assert_eq!(v["session"]["proposer"], "<proposer>");
        assert_eq!(v["session"]["participants"], serde_json::json!(["<device>"]));
        // Structural fields kept.
        assert_eq!(v["session"]["type"], "dkg");
        assert_eq!(v["session"]["threshold"], 2);
        assert_eq!(v["session"]["total"], 3);
    }

    #[test]
    fn wallet_entry_address_redacted_name_kept() {
        let ev = CliEvent::Wallets {
            wallets: vec![WalletEntry {
                id: "w1".into(),
                name: "My Wallet".into(),
                address: "0xabc".into(),
                chain: "Ethereum".into(),
                threshold: "2/3".into(),
                curves: vec![],
                addresses: vec![],
            }],
        };
        let v = normalize_event(&ev);
        assert_eq!(v["wallets"][0]["address"], "<address>");
        assert_eq!(v["wallets"][0]["name"], "My Wallet", "name is structural");
        assert_eq!(v["wallets"][0]["chain"], "Ethereum");
    }
}
