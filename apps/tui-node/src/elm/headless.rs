//! Headless Elm runner.
//!
//! Drives the exact same Elm core (`update()` + `Command::execute()` +
//! the real WebSocket/WebRTC/DKG protocol layers) as the terminal
//! [`ElmApp`](crate::elm::ElmApp), but WITHOUT a terminal. Front-ends that
//! render their own UI (e.g. the native Iced app) use this to get real
//! DKG, signing and keystore persistence while keeping their own widgets.
//!
//! The loop is identical in spirit to `ElmApp::run`'s message arm:
//! `recv(Message) → update(&mut model, msg) → spawn command.execute(...)`.
//! After every processed message it invokes a caller-supplied `on_sync`
//! closure with `&Model` so the front-end can mirror state into its own
//! UI. UI actions are injected by sending [`Message`]s on the channel
//! returned by [`HeadlessRunner::sender`] (or the typed helpers).
//!
//! Reuse, not duplication: zero protocol logic lives here. Every byte of
//! FROST/networking comes from the shared `update`/`Command` path.

use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::elm::message::Message;
use crate::elm::model::{Model, WalletConfig};
use crate::elm::update::update;
use crate::utils::appstate_compat::AppState;

/// A non-terminal driver for the Elm core.
pub struct HeadlessRunner<C>
where
    C: frost_core::Ciphersuite + Send + Sync + 'static,
{
    model: Model,
    app_state: Arc<Mutex<AppState<C>>>,
    tx: UnboundedSender<Message>,
    rx: UnboundedReceiver<Message>,
    /// Called after every processed message with the post-update model and
    /// the message that produced it (`None` for the initial pre-loop sync).
    /// Front-ends use the message to emit precise events (e.g. signature
    /// bytes that don't live on the model) and the model for state deltas.
    on_sync: Box<dyn Fn(&Model, Option<&Message>) + Send>,
    device_id: String,
    keystore_path: String,
}

impl<C> HeadlessRunner<C>
where
    C: frost_core::Ciphersuite + Send + Sync + 'static,
    <<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar:
        Send + Sync,
    C: crate::utils::curve_traits::CurveIdentifier,
{
    /// Build a runner. `keystore_path` is stamped onto the model so the
    /// DKG-finalize command knows where to write; the caller is expected
    /// to have already set `app_state.keystore` to a `Keystore` at the
    /// same path (mirrors `run_elm_tui` in the TUI binary). `on_sync` is
    /// invoked after every message with the current model.
    pub fn new(
        device_id: String,
        keystore_path: String,
        app_state: Arc<Mutex<AppState<C>>>,
        on_sync: Box<dyn Fn(&Model, Option<&Message>) + Send>,
    ) -> Self {
        let (tx, rx) = unbounded_channel();
        let mut model = Model::new(device_id.clone());
        model.wallet_state.keystore_path = keystore_path.clone();
        // Seed the running ciphersuite from `C` exactly as `ElmApp::new<C>`
        // does — otherwise update-layer sites that read `curve_type` (session
        // announcements, address derivation) would all assume the default
        // (secp256k1) and an ed25519 runner would mis-announce its curve.
        model.wallet_state.curve_type =
            <C as crate::utils::curve_traits::CurveIdentifier>::curve_type();
        Self {
            model,
            app_state,
            tx,
            rx,
            on_sync,
            device_id,
            keystore_path,
        }
    }

    /// A clonable handle for injecting messages from the front-end.
    pub fn sender(&self) -> UnboundedSender<Message> {
        self.tx.clone()
    }

    /// Connect to the signal server (reads the URL from `AppState`).
    pub fn connect(&self) {
        let _ = self.tx.send(Message::TriggerReconnect);
    }

    /// Refresh the on-disk wallet list into the model.
    pub fn refresh_wallets(&self) {
        let _ = self.tx.send(Message::ListWallets);
    }

    /// Start a brand-new wallet as the DKG creator.
    pub fn create_wallet(&self, config: WalletConfig, password: String, label: String) {
        let _ = self.tx.send(Message::HeadlessCreateWallet {
            config,
            password,
            label,
        });
    }

    /// Ask the signal server to replay all active sessions (cold-start
    /// discovery of sessions announced before this node connected).
    pub fn refresh_sessions(&self) {
        let _ = self.tx.send(Message::HeadlessRefreshSessions);
    }

    /// Join a discovered DKG/signing session as a participant.
    pub fn join_session(&self, session_id: String, password: String, label: String) {
        let _ = self.tx.send(Message::HeadlessJoinSession {
            session_id,
            password,
            label,
        });
    }

    /// Run the event loop until a [`Message::Quit`] or the channel closes.
    /// Consumes `self`; spawn it on the async runtime.
    pub async fn run(mut self) {
        info!("Headless Elm runner started");

        // Open the keystore on AppState (mirrors run_elm_tui in the TUI
        // binary) so LoadWallets / FinalizeWalletFromDkg have somewhere to
        // read/write. Done here rather than in `new` so construction stays
        // synchronous and runtime-agnostic.
        {
            let mut st = self.app_state.lock().await;
            if st.keystore.is_none() {
                match crate::keystore::Keystore::new(&self.keystore_path, &self.device_id) {
                    Ok(ks) => {
                        st.keystore = Some(Arc::new(ks));
                        info!("Headless runner: keystore opened at {}", self.keystore_path);
                    }
                    Err(e) => error!("Headless runner: failed to open keystore: {}", e),
                }
            }
        }

        // Load any existing wallets into the model immediately.
        let _ = self.tx.send(Message::ListWallets);

        // Initial sync so the front-end shows the current state.
        (self.on_sync)(&self.model, None);

        while let Some(msg) = self.rx.recv().await {
            if matches!(msg, Message::Quit) {
                info!("Headless runner received Quit — stopping");
                break;
            }

            // Keep a copy for the post-update callback (some outcomes ride
            // on the message, not the model). Messages are small; the few
            // package-carrying ones are infrequent.
            let processed = msg.clone();

            if let Some(command) = update(&mut self.model, msg) {
                let tx = self.tx.clone();
                let app_state = self.app_state.clone();
                tokio::spawn(async move {
                    if let Err(e) = command.execute(tx, &app_state).await {
                        error!("Headless command execution failed: {}", e);
                    }
                });
            }

            // Mirror the freshly-updated model + the message into the UI.
            (self.on_sync)(&self.model, Some(&processed));
        }
    }
}

/// Convenience constructor for secp256k1 (Ethereum/EVM) front-ends that
/// don't want to name the ciphersuite or depend on `frost-secp256k1`
/// themselves (e.g. the native Iced app). Builds an `AppState`, spawns
/// the [`HeadlessRunner`] on the current Tokio runtime, and returns the
/// `Message` sender for injecting UI actions. Must be called from within
/// a Tokio runtime context.
pub fn spawn_secp256k1<F>(
    device_id: String,
    keystore_path: String,
    signal_server_url: String,
    on_sync: F,
) -> UnboundedSender<Message>
where
    F: Fn(&Model, Option<&Message>) + Send + 'static,
{
    use frost_secp256k1::Secp256K1Sha256;
    let app_state = Arc::new(Mutex::new(
        AppState::<Secp256K1Sha256>::with_device_id_and_server(
            device_id.clone(),
            signal_server_url,
        ),
    ));
    let runner = HeadlessRunner::<Secp256K1Sha256>::new(
        device_id,
        keystore_path,
        app_state,
        Box::new(on_sync),
    );
    let tx = runner.sender();
    tokio::spawn(runner.run());
    tx
}

/// Convenience constructor for ed25519 (Solana) front-ends — the ed25519
/// counterpart of [`spawn_secp256k1`]. Builds an `AppState<Ed25519Sha512>`,
/// spawns the runner, and returns the `Message` sender. Must be called from
/// within a Tokio runtime.
pub fn spawn_ed25519<F>(
    device_id: String,
    keystore_path: String,
    signal_server_url: String,
    on_sync: F,
) -> UnboundedSender<Message>
where
    F: Fn(&Model, Option<&Message>) + Send + 'static,
{
    use frost_ed25519::Ed25519Sha512;
    let app_state = Arc::new(Mutex::new(
        AppState::<Ed25519Sha512>::with_device_id_and_server(
            device_id.clone(),
            signal_server_url,
        ),
    ));
    let runner = HeadlessRunner::<Ed25519Sha512>::new(
        device_id,
        keystore_path,
        app_state,
        Box::new(on_sync),
    );
    let tx = runner.sender();
    tokio::spawn(runner.run());
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use frost_secp256k1::Secp256K1Sha256;
    use std::sync::Mutex as StdMutex;

    /// The sync callback must receive the message that was just processed
    /// (so a front-end can emit precise events), with `None` for the
    /// initial pre-loop sync.
    #[tokio::test]
    async fn sync_callback_receives_processed_message() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let app_state = Arc::new(Mutex::new(
            AppState::<Secp256K1Sha256>::with_device_id_and_server("t".into(), String::new()),
        ));

        // Record a debug-string of each (msg) the callback saw.
        let seen: Arc<StdMutex<Vec<Option<String>>>> = Arc::new(StdMutex::new(Vec::new()));
        let seen_cb = seen.clone();

        let runner = HeadlessRunner::<Secp256K1Sha256>::new(
            "t".into(),
            tmp.path().to_string_lossy().into_owned(),
            app_state,
            Box::new(move |_model, msg| {
                seen_cb
                    .lock()
                    .unwrap()
                    .push(msg.map(|m| format!("{:?}", m)));
            }),
        );
        let tx = runner.sender();
        let handle = tokio::spawn(runner.run());

        // FIFO channel: startup ListWallets, then our message, then Quit.
        tx.send(Message::NavigateHome).unwrap();
        tx.send(Message::Quit).unwrap();
        let _ = handle.await;

        let seen = seen.lock().unwrap();
        // Initial sync passes None.
        assert!(seen.iter().any(|m| m.is_none()), "expected an initial None sync");
        // The processed NavigateHome must have been surfaced.
        assert!(
            seen.iter()
                .any(|m| m.as_deref() == Some("NavigateHome")),
            "callback never saw the processed NavigateHome message: {:?}",
            *seen
        );
    }
}
