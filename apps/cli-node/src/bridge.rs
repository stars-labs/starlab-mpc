//! Maps the Elm `Model` + the just-processed `Message` into `CliEvent`s.
//!
//! Two strategies, combined:
//!  - **state diff** for connection + wallet/session lists (emit deltas,
//!    not full dumps, to keep the stream lean), and
//!  - **message tap** for one-shot outcomes that ride on the message
//!    rather than the model (`DKGFinalized` carries the addresses,
//!    `SessionDiscovered` carries the announcement).
//!
//! Pure + deterministic → unit-testable without a runtime.

use std::collections::HashSet;

use tui_node::elm::{Message, Model};
use tui_node::keystore::WalletMetadata;
use tui_node::protocal::signal::{SessionInfo, SessionType};

use crate::protocol::{CliEvent, SessionEntry, WalletEntry};

/// Stateful so it can emit only what changed between syncs.
#[derive(Default)]
pub struct Bridge {
    prev_connected: Option<bool>,
    prev_wallet_ids: Option<Vec<String>>,
    announced_sessions: HashSet<String>,
}

impl Bridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Produce events for one runner sync. `msg` is the message that was
    /// just processed (`None` for the initial pre-loop sync).
    pub fn on_sync(&mut self, model: &Model, msg: Option<&Message>) -> Vec<CliEvent> {
        let mut events = Vec::new();

        // --- connection delta ---
        let connected = model.network_state.connected;
        if self.prev_connected != Some(connected) {
            self.prev_connected = Some(connected);
            events.push(CliEvent::Connection { connected });
        }

        // --- wallet-list delta (by id set) ---
        let ids: Vec<String> = model
            .wallet_state
            .wallets
            .iter()
            .map(|w| w.session_id.clone())
            .collect();
        if self.prev_wallet_ids.as_ref() != Some(&ids) {
            self.prev_wallet_ids = Some(ids);
            events.push(CliEvent::Wallets {
                wallets: wallet_entries(&model.wallet_state.wallets),
            });
        }

        // --- message-tap: one-shot outcomes ---
        if let Some(msg) = msg {
            match msg {
                Message::DKGFinalized {
                    wallet_id,
                    group_pubkey_hex,
                    curve_type,
                    addresses,
                } => {
                    // Use the curve's canonical primary address (ed25519 →
                    // Solana base58, secp256k1 → Ethereum) instead of
                    // `addresses.first()`: the source `addresses` vec is built
                    // by iterating a HashMap of compatible chains, so its order
                    // is non-deterministic and could surface e.g. a Sui/Aptos
                    // 0x-hex address for an ed25519 wallet (#43). Fall back to
                    // the first entry only if canonical derivation fails.
                    let address = {
                        let primary = derive_address(group_pubkey_hex, curve_type);
                        if primary.is_empty() {
                            addresses
                                .first()
                                .map(|(_chain, addr)| addr.clone())
                                .unwrap_or_default()
                        } else {
                            primary
                        }
                    };
                    events.push(CliEvent::DkgComplete {
                        correlates: None, // the serve layer stamps the create-cmd id
                        wallet_id: wallet_id.clone(),
                        address,
                        group_public_key: group_pubkey_hex.clone(),
                    });
                }
                Message::UpdateDKGSessionId { real_session_id } => {
                    // Creator side: the real DKG session id is now known.
                    // Emit once (dedupe via the same set so a re-sync of the
                    // same id doesn't re-announce). `correlates` is stamped by
                    // the serve layer from the originating create command.
                    if self.announced_sessions.insert(real_session_id.clone()) {
                        events.push(CliEvent::SessionAnnounced {
                            correlates: None,
                            session_id: real_session_id.clone(),
                        });
                    }
                }
                Message::SessionDiscovered { session } => {
                    // Dedup per (kind, id), NOT id alone: the warm signing path
                    // REUSES the DKG session id (StartSigning just flips the
                    // existing session's type to Signing), so an id-only dedup
                    // would suppress the signing_request for a session whose id
                    // we already saw during DKG — silently breaking discovery
                    // for co-signers (incl. auto-approve). Keying on kind lets a
                    // dkg→signing transition on the same id surface once each.
                    let dedup_key = match &session.session_type {
                        SessionType::Signing { .. } => format!("sign:{}", session.session_id),
                        SessionType::Reshare { .. } => format!("reshare:{}", session.session_id),
                        SessionType::DKG => format!("dkg:{}", session.session_id),
                    };
                    if self.announced_sessions.insert(dedup_key) {
                        // Signing sessions surface as a signing_request (a
                        // co-signer can approve by joining); DKG sessions as
                        // session_available.
                        match &session.session_type {
                            SessionType::Signing { wallet_name, .. } => {
                                events.push(CliEvent::SigningRequest {
                                    session_id: session.session_id.clone(),
                                    wallet: wallet_name.clone(),
                                    threshold: session.threshold,
                                    total: session.total,
                                    proposer: session.proposer_id.clone(),
                                });
                            }
                            SessionType::Reshare { wallet_name, .. } => {
                                // A co-signer approves a reshare by joining its
                                // session (contributing a refreshed share), same
                                // as signing. Surface it as a reshare_request.
                                events.push(CliEvent::ReshareRequest {
                                    session_id: session.session_id.clone(),
                                    wallet: wallet_name.clone(),
                                    threshold: session.threshold,
                                    total: session.total,
                                    proposer: session.proposer_id.clone(),
                                });
                            }
                            SessionType::DKG => {
                                events.push(CliEvent::SessionAvailable {
                                    session: session_entry(session),
                                });
                            }
                        }
                    }
                }
                Message::SigningComplete {
                    message, signature, ..
                } => {
                    events.push(CliEvent::SignatureComplete {
                        correlates: None, // serve stamps the sign-command id
                        signature: format!("0x{}", hex::encode(signature)),
                        message_hash: format!("0x{}", hex::encode(message)),
                    });
                }
                _ => {}
            }
        }

        events
    }

    /// Build a one-shot snapshot (for the `status` command).
    pub fn status(&self, model: &Model) -> CliEvent {
        CliEvent::Status {
            connected: model.network_state.connected,
            device_id: model.device_id.clone(),
            wallets: wallet_entries(&model.wallet_state.wallets),
        }
    }

    /// Current discovered sessions (for the `list_sessions` command).
    pub fn sessions(&self, model: &Model) -> CliEvent {
        CliEvent::Sessions {
            sessions: model.session_invites.iter().map(session_entry).collect(),
        }
    }

    /// Current wallets (for the `list_wallets` command).
    pub fn wallets(&self, model: &Model) -> CliEvent {
        CliEvent::Wallets {
            wallets: wallet_entries(&model.wallet_state.wallets),
        }
    }

    /// Lightweight full snapshot for the serve layer to cache and answer
    /// `status` / `list_wallets` / `list_sessions` without owning the model.
    pub fn snapshot(&self, model: &Model) -> Snapshot {
        Snapshot {
            connected: model.network_state.connected,
            device_id: model.device_id.clone(),
            wallets: wallet_entries(&model.wallet_state.wallets),
            sessions: model.session_invites.iter().map(session_entry).collect(),
        }
    }
}

/// Cached current state owned by the serve layer.
#[derive(Default, Clone)]
pub struct Snapshot {
    pub connected: bool,
    pub device_id: String,
    pub wallets: Vec<WalletEntry>,
    pub sessions: Vec<SessionEntry>,
}

fn chain_for_curve(curve: &str) -> String {
    if curve == "ed25519" {
        "Solana".to_string()
    } else {
        "Ethereum".to_string()
    }
}

fn wallet_entries(wallets: &[WalletMetadata]) -> Vec<WalletEntry> {
    wallets
        .iter()
        .map(|w| WalletEntry {
            id: w.session_id.clone(),
            name: w.display_name().to_string(),
            address: derive_address(&w.group_public_key, &w.curve_type),
            chain: chain_for_curve(&w.curve_type),
            threshold: format!("{}/{}", w.threshold, w.total_participants),
        })
        .collect()
}

/// Derive the primary chain address from a hex-encoded FROST group key
/// (#18). secp256k1 → Ethereum (keccak), ed25519 → Solana (base58).
/// Returns "" if the key can't be decoded/derived.
fn derive_address(group_public_key_hex: &str, curve: &str) -> String {
    let chain = if curve == "ed25519" { "solana" } else { "ethereum" };
    match hex::decode(group_public_key_hex) {
        Ok(bytes) => {
            tui_node::blockchain_config::generate_address_for_chain(&bytes, curve, chain)
                .unwrap_or_default()
        }
        Err(_) => String::new(),
    }
}

fn session_entry(s: &SessionInfo) -> SessionEntry {
    let session_type = match &s.session_type {
        SessionType::DKG => "dkg".to_string(),
        SessionType::Signing { .. } => "signing".to_string(),
        SessionType::Reshare { .. } => "reshare".to_string(),
    };
    SessionEntry {
        session_id: s.session_id.clone(),
        session_type,
        threshold: s.threshold,
        total: s.total,
        proposer: s.proposer_id.clone(),
        participants: s.participants.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session(id: &str) -> SessionInfo {
        SessionInfo {
            session_id: id.to_string(),
            proposer_id: "node-a".to_string(),
            total: 3,
            threshold: 2,
            participants: vec!["node-a".to_string()],
            session_type: SessionType::DKG,
            curve_type: "secp256k1".to_string(),
            coordination_type: "online".to_string(),
            signing_message_hex: None,
        }
    }

    #[test]
    fn initial_sync_emits_connection_and_wallets() {
        let mut b = Bridge::new();
        let model = Model::new("d".to_string());
        let evts = b.on_sync(&model, None);
        // Disconnected + empty wallet list on first sync.
        assert!(matches!(
            evts.iter().find(|e| matches!(e, CliEvent::Connection { .. })),
            Some(CliEvent::Connection { connected: false })
        ));
        assert!(evts
            .iter()
            .any(|e| matches!(e, CliEvent::Wallets { wallets } if wallets.is_empty())));
        // No change on a second identical sync → no events.
        assert!(b.on_sync(&model, None).is_empty());
    }

    #[test]
    fn connection_flip_emits_once() {
        let mut b = Bridge::new();
        let mut model = Model::new("d".to_string());
        let _ = b.on_sync(&model, None); // prime
        model.network_state.connected = true;
        let evts = b.on_sync(&model, None);
        assert_eq!(evts.len(), 1);
        assert!(matches!(evts[0], CliEvent::Connection { connected: true }));
        // Stable → no repeat.
        assert!(b.on_sync(&model, None).is_empty());
    }

    #[test]
    fn dkg_finalized_message_emits_dkg_complete_with_address() {
        let mut b = Bridge::new();
        let model = Model::new("d".to_string());
        let _ = b.on_sync(&model, None);
        let msg = Message::DKGFinalized {
            wallet_id: "wallet-ab12".to_string(),
            group_pubkey_hex: "deadbeef".to_string(),
            curve_type: "secp256k1".to_string(),
            addresses: vec![("ethereum".to_string(), "0xabc123".to_string())],
        };
        let evts = b.on_sync(&model, Some(&msg));
        let done = evts
            .iter()
            .find_map(|e| match e {
                CliEvent::DkgComplete {
                    wallet_id,
                    address,
                    group_public_key,
                    ..
                } => Some((wallet_id.clone(), address.clone(), group_public_key.clone())),
                _ => None,
            })
            .expect("dkg_complete emitted");
        assert_eq!(done.0, "wallet-ab12");
        assert_eq!(done.1, "0xabc123");
        assert_eq!(done.2, "deadbeef");
    }

    #[test]
    fn dkg_complete_reports_canonical_solana_address_for_ed25519() {
        // Regression for #43: the source `addresses` vec is built by iterating a
        // HashMap of compatible chains, so a 0x-hex Sui/Aptos address can land
        // first. DkgComplete must still report the canonical Solana base58
        // derived from the group key, never the 0x one.
        let mut b = Bridge::new();
        let model = Model::new("d".to_string());
        let _ = b.on_sync(&model, None);
        let zeros = "0".repeat(64); // valid 32-byte all-zero ed25519 key
        let msg = Message::DKGFinalized {
            wallet_id: "wallet-ed".to_string(),
            group_pubkey_hex: zeros.clone(),
            curve_type: "ed25519".to_string(),
            addresses: vec![
                ("sui".to_string(), "0xdeadbeefcafe".to_string()), // first, but wrong
                ("solana".to_string(), "11111111111111111111111111111111".to_string()),
            ],
        };
        let addr = b
            .on_sync(&model, Some(&msg))
            .iter()
            .find_map(|e| match e {
                CliEvent::DkgComplete { address, .. } => Some(address.clone()),
                _ => None,
            })
            .expect("dkg_complete emitted");
        assert!(!addr.starts_with("0x"), "ed25519 must not report a 0x address: {addr}");
        assert_eq!(
            addr, "11111111111111111111111111111111",
            "should be the Solana base58 of the all-zero group key"
        );
    }

    #[test]
    fn derives_ethereum_address_from_secp256k1_group_key() {
        // A real compressed secp256k1 group key from a DKG run.
        let key = "0207eb4473c42b74a8a3c72762af295c26fdd40dcaf14e2c65df89aeb6f89073cf";
        let addr = derive_address(key, "secp256k1");
        assert!(addr.starts_with("0x"), "expected 0x-prefixed eth address, got {addr}");
        assert_eq!(addr.len(), 42, "eth address should be 20 bytes hex: {addr}");
    }

    #[test]
    fn derive_address_handles_bad_hex() {
        assert_eq!(derive_address("not-hex", "secp256k1"), "");
    }

    // Address-derivation golden (L4 §5.4): pin derivation against the same
    // external ground-truth vectors the yubiwallet repo uses, so the CLI's
    // address oracle and the hardware-wallet derivations can't silently drift
    // apart. secp256k1 generator G (pubkey for privkey=1) and the all-zero
    // ed25519 key are the canonical, externally-verifiable inputs.
    #[test]
    fn golden_ethereum_address_for_generator_g() {
        // Compressed secp256k1 G → the well-known address for privkey=1.
        let g = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        assert_eq!(
            derive_address(g, "secp256k1").to_lowercase(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
    }

    #[test]
    fn golden_bitcoin_p2wpkh_for_generator_g() {
        // BIP-173 worked example: compressed G → this mainnet P2WPKH address.
        let g = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = hex::decode(g).unwrap();
        let addr = tui_node::blockchain_config::generate_address_for_chain(
            &bytes,
            "secp256k1",
            "bitcoin",
        )
        .expect("derive btc address");
        assert_eq!(addr, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
    }

    #[test]
    fn golden_solana_address_for_zero_key() {
        // base58 of 32 zero bytes is the Solana System Program id.
        let zeros = "0".repeat(64);
        assert_eq!(
            derive_address(&zeros, "ed25519"),
            "11111111111111111111111111111111"
        );
    }

    #[test]
    fn update_dkg_session_id_emits_session_announced_once() {
        let mut b = Bridge::new();
        let model = Model::new("d".to_string());
        let _ = b.on_sync(&model, None);
        let msg = Message::UpdateDKGSessionId {
            real_session_id: "dkg_real_7".to_string(),
        };
        let first = b.on_sync(&model, Some(&msg));
        let ann = first.iter().find_map(|e| match e {
            CliEvent::SessionAnnounced { session_id, correlates } => {
                Some((session_id.clone(), *correlates))
            }
            _ => None,
        });
        assert_eq!(ann, Some(("dkg_real_7".to_string(), None)));
        // Same id again → no duplicate announcement.
        let second = b.on_sync(&model, Some(&msg));
        assert!(!second
            .iter()
            .any(|e| matches!(e, CliEvent::SessionAnnounced { .. })));
    }

    #[test]
    fn session_discovered_dedupes() {
        let mut b = Bridge::new();
        let model = Model::new("d".to_string());
        let _ = b.on_sync(&model, None);
        let msg = Message::SessionDiscovered {
            session: sample_session("dkg_1"),
        };
        let first = b.on_sync(&model, Some(&msg));
        assert!(first
            .iter()
            .any(|e| matches!(e, CliEvent::SessionAvailable { .. })));
        // Same session id again → no duplicate event.
        let second = b.on_sync(&model, Some(&msg));
        assert!(!second
            .iter()
            .any(|e| matches!(e, CliEvent::SessionAvailable { .. })));
    }
}
