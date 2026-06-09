//! WebSocket runtime plumbing for the primary signaling socket.
//!
//! `Command::ReconnectWebSocket` was a 200+ line single function that conflated
//! "what to do" (reconnect + register + re-announce + start pumping) with "how
//! to do each step" (locks, channels, task spawns, error branches). This module
//! splits those into narrow helpers — each does one thing and is read top-to-
//! bottom in a dozen lines. The command arm now reads like a script.
//!
//! Lifetime: there's exactly one primary WebSocket per process. On reconnect,
//! the old channels in `AppState` get replaced before the new socket's tasks
//! are spawned, so stale senders just fail silently.
//!
//! **Ownership:** the new `mpsc` (outbound) and `broadcast` (inbound) channels
//! are minted here and stashed in `AppState`. Subsystems (DKG driver, Elm loop)
//! obtain handles by cloning from `AppState` after connection completes.

use crate::elm::message::Message;
use crate::protocal::signal::SessionInfo;
use crate::utils::appstate_compat::AppState;
use frost_core::{Ciphersuite, Field, Group};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::{error, warn};

/// A split tokio-tungstenite stream as used by the primary socket.
pub(crate) type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub(crate) type WsSink = SplitSink<WsStream, WsMessage>;
pub(crate) type WsRx = SplitStream<WsStream>;

/// Parameters captured from `AppState` before we dial. Also flags the state as
/// "connecting" and drops the stale outbound channel so no caller sends onto
/// a dead socket between now and `install_handles`.
pub(crate) struct ConnectParams {
    pub url: String,
    pub device_id: String,
    pub existing_session: Option<SessionInfo>,
}

pub(crate) async fn read_connect_params<C>(
    app_state: &Arc<Mutex<AppState<C>>>,
) -> ConnectParams
where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let mut state = app_state.lock().await;
    state.websocket_connected = false;
    state.websocket_connecting = true;
    state.websocket_msg_tx = None;
    ConnectParams {
        url: state.signal_server_url.clone(),
        device_id: state.device_id.clone(),
        existing_session: state.session.clone(),
    }
}

/// Dial the signal server. Returns the split stream so the caller can move the
/// sink and receiver into independent tasks.
pub(crate) async fn dial(
    url: &str,
) -> Result<(WsSink, WsRx), tokio_tungstenite::tungstenite::Error> {
    let (stream, _) = tokio_tungstenite::connect_async(url).await?;
    Ok(stream.split())
}

/// Mint the outbound `mpsc` and the inbound `broadcast`, stash them in
/// `AppState` so DKG drivers / LoadSessions / etc. can get at them.
/// Returns the local handles the reconnect arm still needs:
///   - `ws_msg_rx`: drained by the sender task
///   - `broadcast_tx`: cloned into the reader task (broadcast::Sender is the
///     publisher; subscribers call `subscribe()` to get a receiver).
pub(crate) struct InstalledChannels {
    pub ws_msg_rx: mpsc::UnboundedReceiver<String>,
    pub broadcast_tx: broadcast::Sender<Arc<starlab_signal_server::ServerMsg>>,
}

pub(crate) async fn install_handles<C>(
    app_state: &Arc<Mutex<AppState<C>>>,
) -> InstalledChannels
where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let (ws_msg_tx, ws_msg_rx) = mpsc::unbounded_channel::<String>();
    let (broadcast_tx, _) =
        broadcast::channel::<Arc<starlab_signal_server::ServerMsg>>(128);
    {
        let mut state = app_state.lock().await;
        state.websocket_connected = true;
        state.websocket_connecting = false;
        state.websocket_msg_tx = Some(ws_msg_tx);
        state.server_msg_broadcast_tx = Some(broadcast_tx.clone());
    }
    InstalledChannels {
        ws_msg_rx,
        broadcast_tx,
    }
}

/// Send `ClientMsg::Register { device_id }` directly on the sink. Intentionally
/// infallible on the outside (logs on error) — if registration fails the socket
/// is already broken and the reader will surface that as `WebSocketDisconnected`.
pub(crate) async fn send_register(sink: &mut WsSink, device_id: &str) {
    let msg = starlab_signal_server::ClientMsg::Register {
        device_id: device_id.to_string(),
    };
    match serde_json::to_string(&msg) {
        Ok(json) => {
            if let Err(e) = sink.send(WsMessage::text(json)).await {
                error!("Failed to re-register on reconnect: {}", e);
            }
        }
        Err(e) => error!("Failed to serialize Register: {}", e),
    }
}

/// Re-broadcast our own session after a reconnect so peers that missed the
/// initial `AnnounceSession` can still discover us. No-op for joiners (their
/// session's `proposer_id` is someone else — server broadcasts already cover
/// that).
pub(crate) async fn send_reannounce(
    sink: &mut WsSink,
    session: &SessionInfo,
    tx: &mpsc::UnboundedSender<Message>,
) {
    let session_info = serde_json::json!({
        "session_id": session.session_id,
        "total": session.total,
        "threshold": session.threshold,
        "session_type": "dkg",
        "proposer_id": session.proposer_id,
        "participants": session.participants,
        "curve_type": session.curve_type,
        "coordination_type": session.coordination_type,
    });
    let announce = starlab_signal_server::ClientMsg::AnnounceSession { session_info };
    let json = match serde_json::to_string(&announce) {
        Ok(j) => j,
        Err(e) => {
            error!("Failed to serialize reconnect AnnounceSession: {}", e);
            return;
        }
    };
    if let Err(e) = sink.send(WsMessage::text(json)).await {
        error!("Failed to re-announce session on reconnect: {}", e);
    } else {
        let _ = tx.send(Message::Info {
            message: "Session re-announced after reconnect".to_string(),
        });
    }
}

/// Drain the outbound `mpsc` into the socket, with a 30s ping to keep
/// idle connections alive (Cloudflare Workers otherwise idle-close after
/// ~100s). Exits when either the channel closes or a send fails.
pub(crate) fn spawn_sender_task(
    mut sink: WsSink,
    mut rx: mpsc::UnboundedReceiver<String>,
) {
    tokio::spawn(async move {
        let mut ping_interval =
            tokio::time::interval(tokio::time::Duration::from_secs(30));
        ping_interval.tick().await; // Skip the immediate initial tick.
        loop {
            tokio::select! {
                msg = rx.recv() => match msg {
                    Some(payload) => {
                        if let Err(e) = sink.send(WsMessage::text(payload)).await {
                            error!("❌ WS sender: send failed: {}", e);
                            break;
                        }
                    }
                    None => break, // All senders dropped.
                },
                _ = ping_interval.tick() => {
                    if let Err(e) = sink.send(WsMessage::Ping(vec![].into())).await {
                        error!("❌ WS sender: ping failed: {}", e);
                        break;
                    }
                }
            }
        }
    });
}

/// Read the socket, parse each frame once, fan out to (a) Elm messages for the
/// UI loop and (b) `Arc<ServerMsg>` broadcast for domain subscribers.
///
/// Note: `Relay` / `SessionListRequest` / `SessionsForDevice` are broadcast-only
/// — domain code (WebRTC signaling handler) consumes those via the broadcast.
pub(crate) fn spawn_reader_task(
    mut rx: WsRx,
    tx_elm: mpsc::UnboundedSender<Message>,
    broadcast_tx: broadcast::Sender<Arc<starlab_signal_server::ServerMsg>>,
) {
    tokio::spawn(async move {
        while let Some(frame) = rx.next().await {
            match frame {
                Ok(WsMessage::Text(txt)) => {
                    dispatch_frame(&txt, &tx_elm, &broadcast_tx);
                }
                Ok(WsMessage::Close(_)) | Err(_) => {
                    let _ = tx_elm.send(Message::WebSocketDisconnected);
                    break;
                }
                _ => {}
            }
        }
    });
}

/// Spawn the always-on relay handler for the whole connection lifetime.
///
/// Peer WebRTC signals (offer/answer/ICE) and the server's `participant_update`
/// arrive as `ServerMsg::Relay` on the inbound broadcast. Relay handling used
/// to live ONLY inside the DKG driver loops (`StartDKG`/`JoinDKG`), so it was
/// alive only while a DKG ran — or, since those loops persist for the
/// connection, while a DKG had run *this session*. A cold-started signer (load
/// keystore → sign, no DKG this session) therefore had no relay handler when
/// the initiator's offer arrived, and the offer — published to a broadcast with
/// no live subscriber — was silently dropped, stalling the signing mesh. This
/// task subscribes once at connect so relay frames are always handled.
///
/// It reads the current session id from `AppState` per frame (rather than
/// capturing one) so `participant_update` filtering tracks whatever session
/// we're in. It exits when the broadcast closes (i.e. on reconnect, when the
/// previous socket's channels are replaced), so handlers never stack.
pub(crate) fn spawn_relay_handler_task<C>(
    broadcast_tx: broadcast::Sender<Arc<starlab_signal_server::ServerMsg>>,
    app_state: Arc<Mutex<AppState<C>>>,
    tx_elm: mpsc::UnboundedSender<Message>,
    self_device_id: String,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    // Subscribe synchronously (before the reader starts publishing) so no early
    // frame is missed.
    let mut rx = broadcast_tx.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(shared) => {
                    if let starlab_signal_server::ServerMsg::Relay { from, data } = &*shared {
                        // `our_session_id` is ONLY needed to filter
                        // server-originated `participant_update` frames; peer
                        // WebRTC signals (offer/answer/the high-volume ICE
                        // candidates) don't use it. Avoid an app_state lock per
                        // candidate — that lock contends with the FROST
                        // ceremony and badly slows large meshes.
                        let our_session_id = if from == "server" {
                            app_state
                                .lock()
                                .await
                                .session
                                .as_ref()
                                .map(|s| s.session_id.clone())
                        } else {
                            None
                        };
                        crate::elm::webrtc_signaling::handle_relay(
                            from.clone(),
                            data.clone(),
                            app_state.clone(),
                            tx_elm.clone(),
                            self_device_id.clone(),
                            our_session_id,
                        )
                        .await;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("relay handler lagged {} messages; continuing", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn dispatch_frame(
    txt: &str,
    tx_elm: &mpsc::UnboundedSender<Message>,
    broadcast_tx: &broadcast::Sender<Arc<starlab_signal_server::ServerMsg>>,
) {
    let parsed = match serde_json::from_str::<starlab_signal_server::ServerMsg>(txt) {
        Ok(m) => m,
        Err(e) => {
            warn!("Primary WS: unparseable server message ({}): {}", e, txt);
            return;
        }
    };

    // Wrap once; `broadcast::send` returns Err iff there are no receivers —
    // that's the common state at startup, so we swallow it.
    let shared = Arc::new(parsed);
    let _ = broadcast_tx.send(shared.clone());

    match &*shared {
        starlab_signal_server::ServerMsg::SessionAvailable { session_info } => {
            match super::command::parse_session_info(session_info) {
                Some(session) => {
                    let _ = tx_elm.send(Message::SessionDiscovered { session });
                }
                None => warn!(
                    "Primary WS: session_info missing required fields: {}",
                    session_info
                ),
            }
        }
        starlab_signal_server::ServerMsg::SessionRemoved { session_id, .. } => {
            let _ = tx_elm.send(Message::RemoveSession {
                session_id: session_id.clone(),
            });
        }
        starlab_signal_server::ServerMsg::Devices { devices } => {
            let _ = tx_elm.send(Message::UpdateParticipants {
                participants: devices.clone(),
            });
        }
        starlab_signal_server::ServerMsg::Error { error } => {
            warn!("Server-side error frame: {}", error);
            let _ = tx_elm.send(Message::Error {
                message: error.clone(),
            });
        }
        // Relay / SessionListRequest / SessionsForDevice flow only through the
        // broadcast — domain-specific subscribers (e.g. the WebRTC signaling
        // handler) consume them there, not via the Elm loop.
        _ => {}
    }
}

/// Handle the dial failure: mark state disconnected and tell Elm.
pub(crate) async fn handle_dial_failure<C>(
    err: tokio_tungstenite::tungstenite::Error,
    tx: &mpsc::UnboundedSender<Message>,
    app_state: &Arc<Mutex<AppState<C>>>,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    {
        let mut state = app_state.lock().await;
        state.websocket_connecting = false;
    }
    error!("Reconnect failed: {}", err);
    let _ = tx.send(Message::WebSocketDisconnected);
}
