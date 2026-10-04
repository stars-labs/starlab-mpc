//! ECDSA part of a wallet DKG: runs after the FROST ceremonies, among the
//! same participants, and gates `DKGFinalized` on the ECDSA share being
//! persisted.

use super::worker::{self, Job, Outcome};
use crate::elm::message::Message;
use crate::utils::appstate_compat::AppState;
use frost_core::Ciphersuite;
use starlab_core::ecdsa::ECDSA_CURVE;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tracing::{error, info};

/// Bound on aux info + keygen once primes are in hand (aux info is ~20 s
/// of CPU per party natively; slow wasm peers need much more).
pub const DKG_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// What the FROST finalize hands over: enough to persist the ECDSA share
/// next to the FROST ones and then report the whole wallet.
pub struct Finalize {
    pub wallet_id: String,
    pub password: String,
    pub keystore_path: String,
    pub label: Option<String>,
    /// The FROST outcome, reported unchanged in `DKGFinalized`.
    pub group_pubkey_hex: String,
    pub curve_type: String,
    /// FROST account-0 addresses; the ECDSA Ethereum address is prepended.
    pub addresses: Vec<(String, String)>,
}

/// Run ECDSA aux info + keygen for the current DKG session, persist the
/// share (`secp256k1-ecdsa/<wallet_id>.json`, same password), then emit
/// `DKGFinalized` — or `DKGFailed`. Awaits the whole ceremony; call from a
/// spawned command task.
pub async fn run_after_frost<C>(
    app_state: Arc<Mutex<AppState<C>>>,
    tx: UnboundedSender<Message>,
    fin: Finalize,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let fail = |error: String| {
        error!("{error}");
        let _ = tx.send(Message::DKGFailed { error });
    };

    let (session_id, participants, index, threshold, device_id, primes) = {
        let state = app_state.lock().await;
        let Some(session) = state.session.clone() else {
            fail("ECDSA DKG: no active session".into());
            return;
        };
        let mut participants = session.participants.clone();
        participants.sort();
        let Some(index) = participants
            .iter()
            .position(|p| p == &state.device_id)
            .and_then(|i| u16::try_from(i).ok())
        else {
            fail(format!(
                "ECDSA DKG: {} is not a participant of {:?}",
                state.device_id, participants
            ));
            return;
        };
        (
            session.session_id,
            participants,
            index,
            session.threshold,
            state.device_id.clone(),
            state.ecdsa.primes.clone(),
        )
    };

    // The existing finalize password unlocks the per-device cache. Keep all
    // KDF/filesystem work off the runtime and retain only a cache-domain secret.
    let cache_supply = primes.clone();
    let cache_path = std::path::PathBuf::from(&fin.keystore_path);
    let cache_device = device_id.clone();
    let cache_password = fin.password.clone();
    if let Err(e) = tokio::task::spawn_blocking(move || {
        cache_supply.configure_cache(&cache_path, &cache_device, &cache_password);
    })
    .await
    {
        tracing::warn!("ECDSA prime cache setup failed; fresh generation remains available: {e}");
    }

    let (out_tx, out_rx) = unbounded_channel();
    let (status_tx, status_rx) = unbounded_channel();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let job = Job::Dkg {
        session_id: session_id.clone(),
        participants: participants.clone(),
        index,
        threshold,
        primes,
    };
    let mut state = app_state.lock().await;
    if !owns_dkg_session(&state, &session_id) {
        return;
    }
    let inbox = match worker::spawn(job, DKG_TIMEOUT, out_tx, status_tx, done_tx) {
        Ok(inbox) => inbox,
        Err(e) => {
            fail(format!("ECDSA DKG: could not start the worker thread: {e}"));
            return;
        }
    };
    state.ecdsa.install_worker(inbox);
    drop(state);
    super::spawn_transport(app_state.clone(), out_rx);
    super::spawn_status(status_rx, tx.clone());
    info!(
        "🔐 ECDSA DKG started for wallet {} (party {} of {}, threshold {})",
        fin.wallet_id,
        index,
        participants.len(),
        threshold
    );

    let result = done_rx
        .await
        .unwrap_or_else(|_| Err("ECDSA worker vanished".into()));
    {
        let mut state = app_state.lock().await;
        // Aborting a worker closes its inbox. Its eventual result must not
        // clear a newer worker or report failure for a different ceremony.
        if !owns_dkg_session(&state, &session_id) {
            return;
        }
        if let Err(e) = &result {
            fail(format!("ECDSA key generation failed: {e}"));
        }
        state.ecdsa.remove_worker();
        state.ceremony_outbox.clear();
    }
    let share = match result {
        Ok(Outcome::Dkg(share)) => share,
        Ok(Outcome::Sign(_)) => unreachable!("a DKG job yields a key share"),
        Err(_) => return,
    };

    let mut ks = match crate::keystore::Keystore::new(&fin.keystore_path, &device_id) {
        Ok(ks) => ks,
        Err(e) => {
            let state = app_state.lock().await;
            if owns_dkg_session(&state, &session_id) {
                fail(format!("ECDSA DKG: keystore open failed: {e}"));
            }
            return;
        }
    };
    if let Err(e) = ks.save_ecdsa_share(&fin.wallet_id, &share, &fin.password, fin.label.clone()) {
        let state = app_state.lock().await;
        if owns_dkg_session(&state, &session_id) {
            fail(format!("ECDSA DKG: persisting the share failed: {e}"));
        }
        return;
    }
    drop(fin.password);

    let group = share.group_public_key();
    let mut addresses: Vec<(String, String)> =
        starlab_core::accounts::account_addresses(ECDSA_CURVE, &group, 0)
            .unwrap_or_default()
            .into_iter()
            .map(|(chain, _path, address)| (chain, address))
            .collect();
    addresses.extend(fin.addresses);
    info!(
        "✅ ECDSA share persisted for wallet {} (group {})",
        fin.wallet_id,
        hex::encode(group)
    );

    {
        let mut state = app_state.lock().await;
        publish_if_current(&mut state, &session_id, |state| {
            state.keystore = Some(Arc::new(ks));
            state.ecdsa.key_share = Some((fin.wallet_id.clone(), *share));
            let _ = tx.send(Message::DKGFinalized {
                wallet_id: fin.wallet_id,
                group_pubkey_hex: fin.group_pubkey_hex,
                curve_type: fin.curve_type,
                addresses,
            });
        });
    }
}

fn owns_dkg_session<C: Ciphersuite>(state: &AppState<C>, id: &str) -> bool {
    state.session.as_ref().is_some_and(|session| {
        session.session_id == id
            && matches!(
                session.session_type,
                crate::protocal::signal::SessionType::DKG
            )
    })
}

fn publish_if_current<C: Ciphersuite>(
    state: &mut AppState<C>,
    id: &str,
    publish: impl FnOnce(&mut AppState<C>),
) {
    if owns_dkg_session(state, id) {
        publish(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocal::signal::{SessionInfo, SessionType};
    use frost_ed25519::Ed25519Sha512;

    #[test]
    fn persisted_old_result_cannot_publish_after_runtime_session_changes() {
        let mut state = AppState::<Ed25519Sha512>::new();
        state.session = Some(SessionInfo {
            session_id: "new".into(),
            proposer_id: "self".into(),
            total: 2,
            threshold: 2,
            participants: vec![],
            session_type: SessionType::DKG,
            curve_type: "ed25519".into(),
            coordination_type: "online".into(),
            signing_message_hex: None,
        });
        state.current_wallet_id = Some("new-wallet".into());
        let (tx, mut rx) = unbounded_channel();
        // Represents the publication boundary after persistence released the lock.
        publish_if_current(&mut state, "old", |state| {
            state.current_wallet_id = Some("old-wallet".into());
            tx.send(Message::DKGFinalized {
                wallet_id: "old-wallet".into(),
                group_pubkey_hex: "public".into(),
                curve_type: "ecdsa".into(),
                addresses: vec![],
            })
            .unwrap();
        });
        assert_eq!(state.current_wallet_id.as_deref(), Some("new-wallet"));
        assert!(rx.try_recv().is_err());
        publish_if_current(&mut state, "new", |_| {
            tx.send(Message::Info {
                message: "current result".into(),
            })
            .unwrap();
        });
        assert!(matches!(rx.try_recv().unwrap(), Message::Info { .. }));
        state.session = None;
        publish_if_current(&mut state, "new", |_| {
            panic!("cancelled result cannot publish")
        });
    }
}
