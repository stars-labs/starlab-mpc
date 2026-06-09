//! Transport-agnostic driver for the **unified** networked DKG.
//!
//! Wraps [`starlab_core::unified_dkg::UnifiedDkg`] (which runs FROST DKG for
//! ed25519 + secp256k1 simultaneously from one root secret per participant) and
//! adapts it to the WebRTC mesh used by the rest of the app. It mirrors the
//! single-curve driver in [`crate::protocal::dkg`] but produces a wallet with
//! BOTH curves (→ Ethereum/Bitcoin AND Solana/Sui addresses).
//!
//! The driver is intentionally thin: it owns no networking. The caller
//! (command/dispatch layer) is responsible for serializing the round messages
//! over data channels and routing incoming ones back in. The driver exposes:
//!
//! - [`start_round1`]    — create + init the `UnifiedDkg`, return our round-1 pkg
//! - [`ingest_round1`]   — fold in a peer's round-1 pkg; true ⇒ ready for round 2
//! - [`start_round2`]    — run part2, return the per-recipient round-2 packages
//! - [`ingest_round2`]   — fold in a peer's round-2 pkg; true ⇒ ready to finalize
//! - [`finalize`]        — run part3 for both curves and persist the wallet
//!
//! The `UnifiedDkg` is concrete (not generic over `C`); it lives in the
//! `app_state.unified_dkg` field. The Elm app's generic `C` DKG fields are
//! ignored on this path.

use crate::utils::appstate_compat::AppState;
use frost_core::Ciphersuite;
use starlab_core::unified_dkg::{UnifiedDkg, UnifiedRound2Packages};
// Re-export the core round-1 package so callers (command layer, dispatch) can
// name it through this driver module — single import surface for the unified path.
pub use starlab_core::unified_dkg::UnifiedRound1Package;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// One participant's round-2 packages, addressed to their recipients.
///
/// Round 2 in FROST is targeted: participant A produces a distinct package for
/// each other participant. We serialize the whole per-recipient map once and let
/// each receiver pick out the entry keyed by *their* 1-based participant index.
/// (Simpler over the mesh than crafting N separate frames, and tiny.)
#[derive(Serialize, Deserialize)]
pub struct UnifiedRound2Message {
    /// 1-based participant index of the sender (canonical-sorted order).
    pub from_index: u16,
    /// ed25519: recipient_index → hex-encoded round-2 package.
    pub ed25519: std::collections::BTreeMap<u16, String>,
    /// secp256k1: recipient_index → hex-encoded round-2 package.
    pub secp256k1: std::collections::BTreeMap<u16, String>,
}

/// Result of a finished unified ceremony — both group keys + canonical addresses.
pub struct UnifiedDkgOutcome {
    pub wallet_id: String,
    pub ed25519_group_public_key: String,
    pub secp256k1_group_public_key: String,
    pub solana_address: String,
    pub eth_address: String,
}

/// Canonical 1-based participant index for `device_id`, over the SORTED
/// participant list — identical on every node (matches `dkg::canonical_identifier`).
pub fn canonical_index(participants: &[String], device_id: &str) -> Option<u16> {
    let mut sorted: Vec<&String> = participants.iter().collect();
    sorted.sort();
    let idx = sorted.iter().position(|p| p.as_str() == device_id)?;
    u16::try_from(idx).ok()?.checked_add(1)
}

/// Create a fresh `UnifiedDkg`, init it with this node's canonical index + the
/// session params, and produce our round-1 package (for both curves).
///
/// Returns the round-1 package JSON to broadcast, or `None` on a protocol-level
/// error (already logged + reflected onto `dkg_state`).
pub async fn start_round1<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
) -> Option<UnifiedRound1Package>
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let mut guard = state.lock().await;

    let session = match &guard.session {
        Some(s) => s.clone(),
        None => {
            error!("unified DKG: no session available for round 1");
            guard.dkg_state = crate::utils::state::DkgState::Failed("No session".into());
            return None;
        }
    };

    let my_index = match canonical_index(&session.participants, &self_device_id) {
        Some(i) => i,
        None => {
            error!(
                "unified DKG: self_device_id {} not in participants {:?}",
                self_device_id, session.participants
            );
            guard.dkg_state =
                crate::utils::state::DkgState::Failed("self not in participants".into());
            return None;
        }
    };

    let mut dkg = UnifiedDkg::new();
    dkg.init_dkg(my_index, session.total, session.threshold);

    let round1 = match dkg.generate_round1() {
        Ok(pkg) => pkg,
        Err(e) => {
            error!("unified DKG: generate_round1 failed: {}", e);
            guard.dkg_state =
                crate::utils::state::DkgState::Failed(format!("unified round1: {e}"));
            return None;
        }
    };

    guard.dkg_state = crate::utils::state::DkgState::Round1InProgress;
    guard.unified_dkg = Some(dkg);
    info!(
        "unified DKG: round 1 generated for {} (index {}/{}, threshold {})",
        self_device_id, my_index, session.total, session.threshold
    );
    Some(round1)
}

/// Fold in a peer's round-1 package. Returns `true` when all peers' round-1
/// packages have arrived (ready to run round 2).
pub async fn ingest_round1<C>(
    state: Arc<Mutex<AppState<C>>>,
    from_device_id: String,
    package: UnifiedRound1Package,
) -> bool
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let mut guard = state.lock().await;

    let session = match &guard.session {
        Some(s) => s.clone(),
        None => return false,
    };
    let from_index = match canonical_index(&session.participants, &from_device_id) {
        Some(i) => i,
        None => {
            error!("unified DKG: round1 from unknown device {}", from_device_id);
            return false;
        }
    };

    let dkg = match guard.unified_dkg.as_mut() {
        Some(d) => d,
        None => {
            warn!("unified DKG: round1 arrived before our start_round1 ran — ignoring");
            return false;
        }
    };
    if let Err(e) = dkg.add_round1_package(from_index, &package) {
        error!("unified DKG: add_round1_package({}) failed: {}", from_index, e);
        return false;
    }
    let ready = dkg.can_start_round2();
    info!(
        "unified DKG: ingested round1 from {} (index {}); can_start_round2={}",
        from_device_id, from_index, ready
    );
    ready
}

/// Run part2 for both curves; returns the per-recipient round-2 packages to send.
pub async fn start_round2<C>(state: Arc<Mutex<AppState<C>>>) -> Option<UnifiedRound2Message>
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let mut guard = state.lock().await;
    let from_index = guard.unified_dkg.as_ref().map(|d| d.participant_index())?;
    let dkg = guard.unified_dkg.as_mut()?;
    match dkg.generate_round2() {
        Ok(UnifiedRound2Packages { ed25519, secp256k1 }) => {
            guard.dkg_state = crate::utils::state::DkgState::Round2InProgress;
            info!(
                "unified DKG: round 2 generated ({} ed / {} secp recipients)",
                ed25519.len(),
                secp256k1.len()
            );
            Some(UnifiedRound2Message {
                from_index,
                ed25519,
                secp256k1,
            })
        }
        Err(e) => {
            error!("unified DKG: generate_round2 failed: {}", e);
            guard.dkg_state =
                crate::utils::state::DkgState::Failed(format!("unified round2: {e}"));
            None
        }
    }
}

/// Fold in a peer's round-2 message (we pick out the entry addressed to us).
/// Returns `true` when DKG can be finalized.
pub async fn ingest_round2<C>(
    state: Arc<Mutex<AppState<C>>>,
    msg: UnifiedRound2Message,
) -> bool
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let mut guard = state.lock().await;
    let my_index = match guard.unified_dkg.as_ref() {
        Some(d) => d.participant_index(),
        None => {
            warn!("unified DKG: round2 arrived before our DKG was initialised — ignoring");
            return false;
        }
    };

    // Pick out the package addressed to us from the sender's per-recipient maps.
    let ed_hex = match msg.ed25519.get(&my_index) {
        Some(h) => h.clone(),
        None => {
            warn!(
                "unified DKG: round2 from index {} has no ed25519 package for us (index {})",
                msg.from_index, my_index
            );
            return false;
        }
    };
    let secp_hex = match msg.secp256k1.get(&my_index) {
        Some(h) => h.clone(),
        None => {
            warn!(
                "unified DKG: round2 from index {} has no secp256k1 package for us (index {})",
                msg.from_index, my_index
            );
            return false;
        }
    };

    let dkg = match guard.unified_dkg.as_mut() {
        Some(d) => d,
        None => return false,
    };
    if let Err(e) = dkg.add_round2_package(msg.from_index, &ed_hex, &secp_hex) {
        error!(
            "unified DKG: add_round2_package({}) failed: {}",
            msg.from_index, e
        );
        return false;
    }
    let ready = dkg.can_finalize();
    info!(
        "unified DKG: ingested round2 from index {}; can_finalize={}",
        msg.from_index, ready
    );
    ready
}

/// Whether the unified DKG has all it needs to run part3 (both curves).
/// Used to catch the round-2-before-round-1 race: a peer's round-2 package can
/// land before our own part2 ran, so after we generate round 2 we re-check this.
pub async fn can_finalize<C>(state: Arc<Mutex<AppState<C>>>) -> bool
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let guard = state.lock().await;
    guard
        .unified_dkg
        .as_ref()
        .map(|d| d.can_finalize())
        .unwrap_or(false)
}

/// Run part3 for both curves, persist the wallet under BOTH `ed25519/` and
/// `secp256k1/` keystore dirs, and return the outcome (group keys + addresses).
///
/// `wallet_id` is the session-derived id (shared cluster-wide). The persisted
/// share blobs use the exact same `[kp_len][kp][pkp_len][pkp]` framing the
/// single-curve path writes, so the resulting wallets are signing-compatible.
pub async fn finalize<C>(
    state: Arc<Mutex<AppState<C>>>,
    wallet_id: String,
    password: String,
    keystore_path: String,
    wallet_label: Option<String>,
) -> Option<UnifiedDkgOutcome>
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let mut guard = state.lock().await;

    if matches!(guard.dkg_state, crate::utils::state::DkgState::Complete) {
        return None; // idempotent: already finalized
    }

    let session = guard.session.clone()?;
    let device_id = guard.device_id.clone();

    let dkg = guard.unified_dkg.as_mut()?;
    if let Err(e) = dkg.finalize_dkg() {
        error!("unified DKG: finalize_dkg failed: {}", e);
        guard.dkg_state =
            crate::utils::state::DkgState::Failed(format!("unified finalize: {e}"));
        return None;
    }

    // Pull both group keys + canonical addresses out of the finished DKG.
    let ed_group = dkg.get_ed25519_group_public_key().ok()?;
    let secp_group = dkg.get_secp256k1_group_public_key().ok()?;
    let solana_address = dkg.get_solana_address().unwrap_or_default();
    let eth_address = dkg.get_eth_address().unwrap_or_default();

    // Frame each curve's (KeyPackage, PublicKeyPackage) into the keystore blob.
    // `frost_ed25519::keys::KeyPackage` IS `KeyPackage<Ed25519Sha512>`, so the
    // generic blob encoder works directly.
    let ed_blob = match (dkg.ed25519_key_package(), dkg.ed25519_public_key_package()) {
        (Some(kp), Some(pkp)) => match crate::elm::command::encode_keystore_blob(kp, pkp) {
            Ok(b) => b,
            Err(e) => {
                error!("unified DKG: ed25519 blob encode failed: {}", e);
                return None;
            }
        },
        _ => {
            error!("unified DKG: missing ed25519 key packages after finalize");
            return None;
        }
    };
    let secp_blob = match (
        dkg.secp256k1_key_package(),
        dkg.secp256k1_public_key_package(),
    ) {
        (Some(kp), Some(pkp)) => match crate::elm::command::encode_keystore_blob(kp, pkp) {
            Ok(b) => b,
            Err(e) => {
                error!("unified DKG: secp256k1 blob encode failed: {}", e);
                return None;
            }
        },
        _ => {
            error!("unified DKG: missing secp256k1 key packages after finalize");
            return None;
        }
    };

    guard.dkg_state = crate::utils::state::DkgState::Complete;
    guard.current_wallet_id = Some(wallet_id.clone());
    // Drop the borrow before we touch the keystore.
    drop(guard);

    // Canonical (sorted) participant order — same as the single-curve path so
    // the persisted participant_index matches every node's FROST identifier.
    let mut sorted = session.participants.clone();
    sorted.sort();
    let participant_index = match sorted.iter().position(|p| p == &device_id) {
        Some(idx) => (idx as u16) + 1,
        None => {
            error!("unified DKG: device_id {} not in participants", device_id);
            return None;
        }
    };

    use crate::keystore::Keystore;
    let mut ks = match Keystore::new(&keystore_path, &device_id) {
        Ok(k) => k,
        Err(e) => {
            error!("unified DKG: Keystore::new failed: {}", e);
            return None;
        }
    };

    // Persist BOTH curves under the same wallet_id (different curve dirs).
    if let Err(e) = ks.create_wallet_unified(
        &wallet_id,
        session.threshold,
        session.total,
        participant_index,
        sorted.clone(),
        wallet_label,
        &password,
        &ed_group,
        &ed_blob,
        &secp_group,
        &secp_blob,
    ) {
        error!("unified DKG: persist both curves failed: {}", e);
        return None;
    }
    info!("unified DKG: persisted ed25519 + secp256k1 shares for wallet {}", wallet_id);
    drop(password);

    // Re-hydrate the shared read-only keystore so the next LoadWallets sees both.
    if let Ok(fresh) = Keystore::new(&keystore_path, &device_id) {
        let mut state = state.lock().await;
        state.keystore = Some(Arc::new(fresh));
    }

    info!(
        "✅ unified DKG complete: wallet={} eth={} sol={}",
        wallet_id, eth_address, solana_address
    );
    Some(UnifiedDkgOutcome {
        wallet_id,
        ed25519_group_public_key: ed_group,
        secp256k1_group_public_key: secp_group,
        solana_address,
        eth_address,
    })
}
