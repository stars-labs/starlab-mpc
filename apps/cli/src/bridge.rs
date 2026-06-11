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

use starlab_client::elm::{Message, Model};
use starlab_client::keystore::WalletMetadata;
use starlab_client::protocal::signal::{SessionInfo, SessionType};

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
        //
        // `prev_wallet_ids == None` means "keystore scan hasn't reported yet":
        // the model's empty wallet list at startup is NOT a real state, so no
        // delta fires for it. Emitting on that pre-scan emptiness was a race —
        // `wallet list` matched the spurious empty Wallets event and exited
        // before WalletsLoaded round-tripped (the keystore actually had the
        // wallet all along). The authoritative emit is the WalletsLoaded tap
        // below; this delta only reports LATER changes (e.g. a DKG finalizing
        // mid-serve).
        let ids: Vec<String> = model
            .wallet_state
            .wallets
            .iter()
            .map(|w| w.session_id.clone())
            .collect();
        let mut wallets_emitted = false;
        if self.prev_wallet_ids.is_some() && self.prev_wallet_ids.as_ref() != Some(&ids) {
            self.prev_wallet_ids = Some(ids.clone());
            wallets_emitted = true;
            events.push(CliEvent::Wallets {
                wallets: wallet_entries(&model.wallet_state.wallets),
            });
        }

        // --- message-tap: one-shot outcomes ---
        if let Some(msg) = msg {
            match msg {
                // Authoritative wallet list: the keystore scan completed.
                // Always emit (even an empty list — "no wallets" is a real
                // answer), unless the delta above already fired this sync.
                Message::WalletsLoaded { .. } => {
                    self.prev_wallet_ids = Some(ids.clone());
                    if !wallets_emitted {
                        events.push(CliEvent::Wallets {
                            wallets: wallet_entries(&model.wallet_state.wallets),
                        });
                    }
                }
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
                        // Account 0's primary-chain address (BIP-44 model).
                        let primary_chain =
                            if curve_type == "ed25519" { "solana" } else { "ethereum" };
                        let primary = hex::decode(group_pubkey_hex)
                            .ok()
                            .and_then(|g| {
                                let path = starlab_core::accounts::standard_path(primary_chain, 0)?;
                                let parsed = starlab_core::DerivationPath::parse(&path).ok()?;
                                let child = if curve_type == "ed25519" {
                                    starlab_core::derive_child_verifying_key_path::<
                                        frost_ed25519::Ed25519Sha512,
                                    >(&g, &parsed)
                                    .ok()?
                                } else {
                                    starlab_core::derive_child_verifying_key_path::<
                                        frost_secp256k1::Secp256K1Sha256,
                                    >(&g, &parsed)
                                    .ok()?
                                };
                                starlab_core::accounts::address_for_chain(
                                    primary_chain,
                                    curve_type,
                                    &child,
                                )
                                .ok()
                            })
                            .unwrap_or_default();
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
                Message::ReshareComplete {
                    wallet_id,
                    group_public_key,
                } => {
                    events.push(CliEvent::ReshareComplete {
                        correlates: None, // serve stamps the reshare-command id
                        wallet_id: wallet_id.clone(),
                        group_public_key: group_public_key.clone(),
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

/// Single source of truth: starlab_core::accounts (shared with WASM/desktop).
pub(crate) use starlab_core::accounts::chains_for_curve;

/// Derive one chain address from a hex-encoded FROST group key. Returns ""
/// if the key can't be decoded/derived (never fails the listing).
pub(crate) fn derive_address(group_public_key_hex: &str, curve: &str, chain_key: &str) -> String {
    match hex::decode(group_public_key_hex) {
        Ok(bytes) => starlab_core::accounts::address_for_chain(chain_key, curve, &bytes)
            .unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// Wallet-centric grouping: the keystore stores ONE FILE PER CURVE, but a
/// wallet (one DKG ceremony, one id) may span both curves — the unified DKG
/// writes the same id under ed25519/ and secp256k1/. The UI must never leak
/// that storage layout: group by wallet id, merge curves, and derive every
/// chain address each curve controls.
fn wallet_entries(wallets: &[WalletMetadata]) -> Vec<WalletEntry> {
    use std::collections::BTreeMap;
    // id → (first-seen order index, entry under construction)
    let mut grouped: BTreeMap<String, (usize, WalletEntry)> = BTreeMap::new();
    for (i, w) in wallets.iter().enumerate() {
        // Materialized HD account children carry their derivation path as the
        // label ("m/44'/…"). They're an implementation detail of signing —
        // listing them would double-derive (account 0 OF an account) and
        // clutter the wallet view. `wallet accounts` is their UI.
        if w.label.as_deref().is_some_and(|l| l.starts_with("m/")) {
            continue;
        }
        let entry = grouped.entry(w.session_id.clone()).or_insert_with(|| {
            (
                i,
                WalletEntry {
                    id: w.session_id.clone(),
                    name: w.display_name().to_string(),
                    address: String::new(),
                    chain: String::new(),
                    threshold: format!("{}/{}", w.threshold, w.total_participants),
                    curves: Vec::new(),
                    addresses: Vec::new(),
                },
            )
        });
        let e = &mut entry.1;
        // Prefer a human label over the id-fallback display name.
        if e.name == e.id && w.display_name() != w.session_id {
            e.name = w.display_name().to_string();
        }
        if !e.curves.contains(&w.curve_type) {
            e.curves.push(w.curve_type.clone());
            // BIP-44 all the way: the listed addresses are ACCOUNT 0's
            // (pinned standard paths), not the raw group-key address. The
            // root key exists only as the derivation parent — it is never
            // shown or used as an address anywhere.
            if let Ok(group) = hex::decode(&w.group_public_key) {
                if let Ok(entries) =
                    starlab_core::accounts::account_addresses(&w.curve_type, &group, 0)
                {
                    for (display, path, address) in entries {
                        e.addresses.push(crate::protocol::ChainAddress {
                            chain: display,
                            address,
                            path: Some(path),
                        });
                    }
                }
            }
        }
    }
    let mut out: Vec<(usize, WalletEntry)> = grouped.into_values().collect();
    out.sort_by_key(|(i, _)| *i); // preserve keystore order
    out.into_iter()
        .map(|(_, mut e)| {
            e.curves.sort();
            // Primary pair (driver compat): Ethereum if present, else first.
            if let Some(primary) = e
                .addresses
                .iter()
                .find(|a| a.chain == "Ethereum")
                .or_else(|| e.addresses.first())
            {
                e.chain = primary.chain.clone();
                e.address = primary.address.clone();
            }
            e
        })
        .collect()
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
    fn initial_sync_emits_connection_but_no_premature_wallets() {
        let mut b = Bridge::new();
        let model = Model::new("d".to_string());
        let evts = b.on_sync(&model, None);
        // Disconnected on first sync.
        assert!(matches!(
            evts.iter().find(|e| matches!(e, CliEvent::Connection { .. })),
            Some(CliEvent::Connection { connected: false })
        ));
        // CRITICAL (wallet-list race): the pre-scan empty model must NOT
        // produce a Wallets event — `wallet list` would match it and exit
        // before the keystore scan's WalletsLoaded arrives.
        assert!(!evts.iter().any(|e| matches!(e, CliEvent::Wallets { .. })));
        // No change on a second identical sync → no events.
        assert!(b.on_sync(&model, None).is_empty());
    }

    #[test]
    fn wallets_loaded_tap_is_authoritative_even_when_empty() {
        let mut b = Bridge::new();
        let model = Model::new("d".to_string());
        let _ = b.on_sync(&model, None); // prime (no Wallets yet)
        // Keystore scan reports: genuinely zero wallets. The tap must still
        // emit, so `wallet list` answers instead of hanging to timeout.
        let evts = b.on_sync(&model, Some(&Message::WalletsLoaded { wallets: vec![] }));
        let count = evts
            .iter()
            .filter(|e| matches!(e, CliEvent::Wallets { wallets } if wallets.is_empty()))
            .count();
        assert_eq!(count, 1, "exactly one authoritative empty Wallets event");
        // Re-loading the same (empty) list emits again — list is a query,
        // every WalletsLoaded answers it — but never duplicates per sync.
        let evts2 = b.on_sync(&model, Some(&Message::WalletsLoaded { wallets: vec![] }));
        assert_eq!(
            evts2.iter().filter(|e| matches!(e, CliEvent::Wallets { .. })).count(),
            1
        );
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
        // A REAL curve point is required now: DkgComplete derives ACCOUNT 0
        // from the group key (BIP-44 model), and derivation deserializes the
        // point. The ed25519 basepoint is the canonical valid key.
        let ed_g = "5866666666666666666666666666666666666666666666666666666666666666";
        let msg = Message::DKGFinalized {
            wallet_id: "wallet-ed".to_string(),
            group_pubkey_hex: ed_g.to_string(),
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
        // Account 0's Solana address: a base58 string derived from the group
        // key (not the group key itself — BIP-44 model). Base58 of a 32-byte
        // key is 32–44 chars.
        assert!(
            (32..=44).contains(&addr.len()),
            "unexpected solana address shape: {addr}"
        );
    }

    #[test]
    fn derives_ethereum_address_from_secp256k1_group_key() {
        // A real compressed secp256k1 group key from a DKG run.
        let key = "0207eb4473c42b74a8a3c72762af295c26fdd40dcaf14e2c65df89aeb6f89073cf";
        let addr = derive_address(key, "secp256k1", "ethereum");
        assert!(addr.starts_with("0x"), "expected 0x-prefixed eth address, got {addr}");
        assert_eq!(addr.len(), 42, "eth address should be 20 bytes hex: {addr}");
    }

    #[test]
    fn materialized_account_children_are_hidden_from_the_wallet_list() {
        let secp_g = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let mk = |id: &str, label: Option<&str>| WalletMetadata {
            session_id: id.into(),
            device_id: "d".into(),
            curve_type: "secp256k1".into(),
            threshold: 2,
            total_participants: 3,
            participant_index: 1,
            group_public_key: secp_g.into(),
            participants: vec![],
            created_at: "t".into(),
            last_modified: "t".into(),
            label: label.map(Into::into),
            device_name: None,
            blockchains: vec![],
            blockchain: None,
            public_address: None,
            identifier: None,
            tags: None,
            description: None,
        };
        let entries = wallet_entries(&[
            mk("parent1", None),
            mk("parent1-ethereum-0", Some("m/44'/60'/0'/0/0")), // hidden
        ]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "parent1");
    }

    #[test]
    fn unified_wallet_groups_into_one_entry_with_all_chains() {
        // The unified DKG writes the SAME wallet id under ed25519/ and
        // secp256k1/ — storage detail that must never leak as "two wallets".
        // Generator points are valid keys for both curves.
        let secp_g = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let ed_g = "5866666666666666666666666666666666666666666666666666666666666666";
        let mk = |curve: &str, gk: &str| WalletMetadata {
            session_id: "uni1".into(),
            device_id: "d".into(),
            curve_type: curve.into(),
            threshold: 2,
            total_participants: 2,
            participant_index: 1,
            group_public_key: gk.into(),
            participants: vec![],
            created_at: "t".into(),
            last_modified: "t".into(),
            label: Some("My Unified".into()),
            device_name: None,
            blockchains: vec![],
            blockchain: None,
            public_address: None,
            identifier: None,
            tags: None,
            description: None,
        };
        let entries = wallet_entries(&[mk("ed25519", ed_g), mk("secp256k1", secp_g)]);
        assert_eq!(entries.len(), 1, "one wallet, not one row per curve file");
        let e = &entries[0];
        assert_eq!(e.curves, vec!["ed25519", "secp256k1"]);
        let chains: Vec<&str> = e.addresses.iter().map(|a| a.chain.as_str()).collect();
        assert!(chains.contains(&"Ethereum") && chains.contains(&"Bitcoin"));
        assert!(chains.contains(&"Solana") && chains.contains(&"Sui"));
        // Primary pair = Ethereum (driver compat).
        assert_eq!(e.chain, "Ethereum");
        assert!(e.address.starts_with("0x"));
        assert_eq!(e.name, "My Unified");
    }

    #[test]
    fn derive_address_handles_bad_hex() {
        assert_eq!(derive_address("not-hex", "secp256k1", "ethereum"), "");
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
            derive_address(g, "secp256k1", "ethereum").to_lowercase(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
    }

    #[test]
    fn golden_bitcoin_p2wpkh_for_generator_g() {
        // BIP-173 worked example: compressed G → this mainnet P2WPKH address.
        let g = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = hex::decode(g).unwrap();
        let addr = starlab_client::blockchain_config::generate_address_for_chain(
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
            derive_address(&zeros, "ed25519", "solana"),
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
