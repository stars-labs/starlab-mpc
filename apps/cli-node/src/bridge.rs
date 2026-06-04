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
                    addresses,
                    ..
                } => {
                    events.push(CliEvent::DkgComplete {
                        correlates: None, // the serve layer stamps the create-cmd id
                        wallet_id: wallet_id.clone(),
                        address: addresses
                            .first()
                            .map(|(_chain, addr)| addr.clone())
                            .unwrap_or_default(),
                        group_public_key: group_pubkey_hex.clone(),
                    });
                }
                Message::SessionDiscovered { session } => {
                    if self.announced_sessions.insert(session.session_id.clone()) {
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
            // Address derivation from the group key is tracked in #18;
            // dkg_complete already carries the real address from the
            // finalize message.
            address: String::new(),
            chain: chain_for_curve(&w.curve_type),
            threshold: format!("{}/{}", w.threshold, w.total_participants),
        })
        .collect()
}

fn session_entry(s: &SessionInfo) -> SessionEntry {
    let session_type = match &s.session_type {
        SessionType::DKG => "dkg".to_string(),
        SessionType::Signing { .. } => "signing".to_string(),
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
