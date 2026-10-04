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
    let inbox = match worker::spawn(job, DKG_TIMEOUT, out_tx, status_tx, done_tx) {
        Ok(inbox) => inbox,
        Err(e) => {
            fail(format!("ECDSA DKG: could not start the worker thread: {e}"));
            return;
        }
    };
    app_state.lock().await.ecdsa.install_worker(inbox);
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
        state.ecdsa.remove_worker();
        state.ceremony_outbox.clear();
    }
    let share = match result {
        Ok(Outcome::Dkg(share)) => share,
        Ok(Outcome::Sign(_)) => unreachable!("a DKG job yields a key share"),
        Err(e) => {
            fail(format!("ECDSA key generation failed: {e}"));
            return;
        }
    };

    let mut ks = match crate::keystore::Keystore::new(&fin.keystore_path, &device_id) {
        Ok(ks) => ks,
        Err(e) => {
            fail(format!("ECDSA DKG: keystore open failed: {e}"));
            return;
        }
    };
    if let Err(e) = ks.save_ecdsa_share(&fin.wallet_id, &share, &fin.password, fin.label.clone()) {
        fail(format!("ECDSA DKG: persisting the share failed: {e}"));
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
        state.keystore = Some(Arc::new(ks));
        state.ecdsa.key_share = Some((fin.wallet_id.clone(), *share));
    }
    let _ = tx.send(Message::DKGFinalized {
        wallet_id: fin.wallet_id,
        group_pubkey_hex: fin.group_pubkey_hex,
        curve_type: fin.curve_type,
        addresses,
    });
}
