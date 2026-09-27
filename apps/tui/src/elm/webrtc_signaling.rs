//! WebRTC signaling handler — the consumer side of the signal-server `Relay` frame.
//!
//! Every DKG driver (creator and joiner) needs to react to WebRTC offer / answer /
//! ICE-candidate traffic the same way: open a peer connection, set SDP, generate
//! a counter-message, shove it back through the shared WebSocket. Before this
//! module the logic lived inline, twice, inside `Command::StartDKG` and
//! `Command::JoinDKG` — ~400 lines of deeply nested callbacks, copy-pasted.
//! Now both drivers just forward each `ServerMsg::Relay { from, data }` here.
//!
//! webrtc 0.21 replaced the callback-registration API (`pc.on_ice_candidate(...)`,
//! `pc.on_data_channel(...)`, `pc.on_peer_connection_state_change(...)`) with a single
//! `PeerConnectionEventHandler` installed once, at construction time, via
//! `PeerConnectionBuilder::with_handler`. `SignalingHandler` below is that handler; it
//! is shared by both the offerer path (`network::webrtc::initiate_webrtc_with_channel`)
//! and the answerer path (`ensure_peer_connection` in this file) via
//! `build_peer_connection`, so every peer connection in the mesh gets the same wiring
//! regardless of which side created it.

use crate::elm::message::Message;
use crate::utils::appstate_compat::AppState;
use frost_core::{Ciphersuite, Field, Group};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc::UnboundedSender};
use tracing::{error, info};
use webrtc::data_channel::DataChannel;
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder,
    RTCIceServer, RTCPeerConnectionIceEvent, RTCPeerConnectionState,
};

/// Public STUN servers, shared with the browser extension so every client
/// gathers the same server-reflexive candidates. Without any, peers behind
/// NAT only offer host candidates and can't reach each other across
/// networks. STUN only — there is no public TURN; strict NATs still need a
/// relay of our own.
pub const STUN_SERVERS: &[&str] = &[
    "stun:stun.l.google.com:19302",
    "stun:stun1.l.google.com:19302",
    "stun:stun2.l.google.com:19302",
];

fn ice_servers() -> Vec<RTCIceServer> {
    vec![RTCIceServer {
        urls: STUN_SERVERS.iter().map(|u| u.to_string()).collect(),
        ..Default::default()
    }]
}

/// Process a `ServerMsg::Relay` frame.
///
/// - `self_device_id` / `our_session_id` are used to filter server-originated
///   `participant_update` frames to our own session only.
/// - All outbound WebRTC responses (answer, ICE candidates) go through the
///   shared primary WebSocket channel fetched from `app_state.websocket_msg_tx`.
pub(crate) async fn handle_relay<C>(
    from: String,
    data: serde_json::Value,
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: UnboundedSender<Message>,
    self_device_id: String,
    our_session_id: Option<String>,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let _ = tx_msg.send(Message::Info {
        message: format!("📨 Received relay from {}", from),
    });

    if from != "server" {
        handle_webrtc_signal(from, data, app_state, tx_msg, self_device_id).await;
    } else {
        // Server-originated frame (currently only `participant_update`).
        handle_server_frame(data, app_state, tx_msg, self_device_id, our_session_id).await;
    }
}

/// Handle a peer-originated WebRTC offer / answer / ICE candidate.
async fn handle_webrtc_signal<C>(
    from: String,
    data: serde_json::Value,
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: UnboundedSender<Message>,
    self_device_id: String,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let Some("WebRTCSignal") = data.get("websocket_msg_type").and_then(|v| v.as_str()) else {
        return;
    };
    info!("🎯 Received WebRTC signal from {}", from);

    if let Some(offer_data) = data.get("Offer") {
        if let Some(sdp) = offer_data.get("sdp").and_then(|v| v.as_str()) {
            let _ = tx_msg.send(Message::Info {
                message: format!(
                    "📥 Received WebRTC offer from {}, preparing answer...",
                    from
                ),
            });
            // Decided inline (relay frames are handled in order) so the ICE
            // candidates that follow this offer are buffered for the new
            // connection instead of landing on the one being replaced.
            if !crate::network::peer_recovery::prepare_for_offer(
                &app_state,
                &tx_msg,
                &self_device_id,
                &from,
            )
            .await
            {
                return;
            }
            spawn_offer_handler(from, sdp.to_string(), app_state, tx_msg, self_device_id);
        }
    } else if let Some(answer_data) = data.get("Answer") {
        if let Some(sdp) = answer_data.get("sdp").and_then(|v| v.as_str()) {
            let _ = tx_msg.send(Message::Info {
                message: format!(
                    "📥 Received WebRTC answer from {}, setting remote description...",
                    from
                ),
            });
            spawn_answer_handler(from, sdp.to_string(), app_state);
        }
    } else if let Some(ice_data) = data.get("Candidate")
        && let (Some(candidate), Some(sdp_mid), Some(sdp_mline_index)) = (
            ice_data.get("candidate").and_then(|v| v.as_str()),
            ice_data.get("sdpMid").and_then(|v| v.as_str()),
            ice_data.get("sdpMLineIndex").and_then(|v| v.as_u64()),
        )
    {
        info!("📥 Received ICE candidate from {}", from);
        spawn_ice_handler(
            from,
            candidate.to_string(),
            sdp_mid.to_string(),
            sdp_mline_index as u16,
            app_state,
        );
    }
}

/// Handle the `participant_update` frame the signal server emits when a new
/// device joins a session we're the creator or a member of.
async fn handle_server_frame<C>(
    data: serde_json::Value,
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: UnboundedSender<Message>,
    self_device_id: String,
    our_session_id: Option<String>,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let Some("participant_update") = data.get("type").and_then(|v| v.as_str()) else {
        return;
    };
    let (Some(session_id), Some(session_info)) = (
        data.get("session_id").and_then(|v| v.as_str()),
        data.get("session_info"),
    ) else {
        return;
    };
    let is_our_session = our_session_id
        .as_ref()
        .map(|s| s == session_id)
        .unwrap_or(false);
    if !is_our_session {
        return;
    }

    let new_participants: Vec<String> = session_info
        .get("participants")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .filter(|&p| p != self_device_id)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    if new_participants.is_empty() {
        return;
    }

    // Session-scoped gate: don't initiate WebRTC until the session has reached
    // its advertised total size. The server emits one `participant_update` per
    // join, so an N-of-M session will trigger this M-1 times. If we fire
    // `InitiateWebRTCWithParticipants` on every partial update, we spawn a new
    // mesh-check task each time with a different expected peer count — the
    // early one fires "mesh ready" at 1/2, runs FROST Round 1 against a
    // 2-person session, and when mpc-3 finally joins we run Round 1 AGAIN
    // against a 3-person session. The second run regenerates the Round-1
    // secret and the packages stop matching. Gate on `len >= session.total`
    // so FROST only starts once, against the finalized participant set.
    let session_total_opt = session_info.get("total").and_then(|v| v.as_u64());
    let participants_len = new_participants.len() + 1; // include self
    if let Some(total) = session_total_opt
        && (participants_len as u64) < total
    {
        info!(
            "⏳ participant_update: {}/{} joined — waiting for full session before \
                 initiating WebRTC",
            participants_len, total
        );
        // Still keep session.participants in sync so the UI sees the
        // joiner roster; just don't trigger WebRTC yet.
        let all_parts: Vec<String> = new_participants
            .iter()
            .cloned()
            .chain(std::iter::once(self_device_id.clone()))
            .collect();
        let mut state = app_state.lock().await;
        if let Some(ref mut session) = state.session {
            session.participants = all_parts.clone();
        }
        drop(state);
        let _ = tx_msg.send(Message::UpdateParticipants {
            participants: all_parts,
        });
        return;
    }

    info!(
        "📡 Received participant update (session full: {}), triggering WebRTC",
        participants_len
    );
    let all_participants = {
        let mut state = app_state.lock().await;
        let mut all_parts = new_participants.clone();
        all_parts.push(self_device_id.clone());
        if let Some(ref mut session) = state.session {
            session.participants = all_parts.clone();
            info!("✅ Updated session participants: {:?}", all_parts);
        }
        all_parts
    };
    // Sync the Elm model's `active_session.participants` — the DKG Progress
    // UI reads `model.active_session.participants`, which is separate from
    // `app_state.session.participants`. Without this dispatch, the Elm
    // model's session stays at whatever the partial-update branch last
    // set it to (e.g. 2-of-3), and the participants sidebar keeps showing
    // only the earlier joiners even though the mesh is fully up.
    let _ = tx_msg.send(Message::UpdateParticipants {
        participants: all_participants.clone(),
    });
    info!("🚀 Triggering WebRTC initiation from participant update");
    let _ = tx_msg.send(Message::InitiateWebRTCWithParticipants {
        participants: all_participants,
    });
    let _ = tx_msg.send(Message::Info {
        message: format!(
            "📡 Triggered WebRTC with participants: {:?}",
            new_participants
        ),
    });
}

/// Spawn a task to accept a remote WebRTC offer: create peer connection, set
/// remote + local SDP, send answer back through the shared WebSocket channel.
fn spawn_offer_handler<C>(
    from_device: String,
    sdp: String,
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: UnboundedSender<Message>,
    _self_device_id: String,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    tokio::spawn(async move {
        info!("🎯 Processing WebRTC offer from {}", from_device);

        let ws_tx = {
            let state = app_state.lock().await;
            match state.websocket_msg_tx.clone() {
                Some(tx) => tx,
                None => {
                    error!(
                        "❌ No primary WebSocket channel when answering {}",
                        from_device
                    );
                    return;
                }
            }
        };

        let pc = match ensure_peer_connection(&from_device, &app_state, &tx_msg, &ws_tx).await {
            Some(pc) => pc,
            None => return,
        };

        let offer = match webrtc::peer_connection::RTCSessionDescription::offer(sdp) {
            Ok(s) => s,
            Err(e) => {
                error!("❌ Invalid offer SDP from {}: {}", from_device, e);
                return;
            }
        };
        if let Err(e) = pc.set_remote_description(offer).await {
            error!(
                "❌ Failed to set remote description for {}: {}",
                from_device, e
            );
            return;
        }
        info!("✅ Set remote description (offer) from {}", from_device);
        flush_buffered_candidates(&from_device, &pc, &app_state).await;

        let answer = match pc.create_answer(None).await {
            Ok(a) => a,
            Err(e) => {
                error!("❌ Failed to create answer for {}: {}", from_device, e);
                return;
            }
        };
        info!("✅ Created answer for {}", from_device);

        if let Err(e) = pc.set_local_description(answer.clone()).await {
            error!(
                "❌ Failed to set local description for {}: {}",
                from_device, e
            );
            return;
        }
        info!("✅ Set local description (answer) for {}", from_device);

        send_answer(&from_device, answer.sdp, &ws_tx);
    });
}

/// Spawn a task to consume a remote answer to our offer (just set remote SDP).
fn spawn_answer_handler<C>(from_device: String, sdp: String, app_state: Arc<Mutex<AppState<C>>>)
where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    tokio::spawn(async move {
        info!("🎯 Processing WebRTC answer from {}", from_device);

        let device_connections = {
            let state = app_state.lock().await;
            state.device_connections.clone()
        };
        let conns = device_connections.lock().await;
        let Some(pc) = conns.get(&from_device).cloned() else {
            error!(
                "❌ No peer connection found for {} when receiving answer",
                from_device
            );
            return;
        };
        drop(conns);

        let answer = match webrtc::peer_connection::RTCSessionDescription::answer(sdp) {
            Ok(a) => a,
            Err(e) => {
                error!("❌ Invalid answer SDP from {}: {}", from_device, e);
                return;
            }
        };
        if let Err(e) = pc.set_remote_description(answer).await {
            error!(
                "❌ Failed to set remote description (answer) for {}: {}",
                from_device, e
            );
        } else {
            info!(
                "✅ Set remote description (answer) from {}, connection establishing",
                from_device
            );
            flush_buffered_candidates(&from_device, &pc, &app_state).await;
        }
    });
}

/// Spawn a task to add a peer-supplied ICE candidate to the existing PC.
fn spawn_ice_handler<C>(
    from_device: String,
    candidate: String,
    sdp_mid: String,
    sdp_mline_index: u16,
    app_state: Arc<Mutex<AppState<C>>>,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    tokio::spawn(async move {
        let init = webrtc::peer_connection::RTCIceCandidateInit {
            candidate,
            sdp_mid: Some(sdp_mid),
            sdp_mline_index: Some(sdp_mline_index),
            username_fragment: None,
            url: None,
        };
        // Each signal runs on its own task, so a candidate can outrun the
        // offer/answer it belongs to (or even the peer connection's creation).
        // Check-and-buffer under the AppState lock; the SDP handler marks the
        // peer and drains the buffer under the same lock, so none is lost.
        let device_connections = {
            let mut state = app_state.lock().await;
            if !state.remote_description_set.contains(&from_device) {
                state
                    .pending_ice_candidates
                    .entry(from_device.clone())
                    .or_default()
                    .push(init);
                info!(
                    "⏳ Buffered ICE candidate from {} until its remote description is set",
                    from_device
                );
                return;
            }
            state.device_connections.clone()
        };
        let Some(pc) = device_connections.lock().await.get(&from_device).cloned() else {
            error!(
                "❌ No peer connection found for {} when adding ICE candidate",
                from_device
            );
            return;
        };
        add_candidate(&from_device, &pc, init).await;
    });
}

/// Mark `device_id`'s remote description as applied and feed it every
/// candidate that arrived early.
async fn flush_buffered_candidates<C>(
    device_id: &str,
    pc: &Arc<dyn PeerConnection>,
    app_state: &Arc<Mutex<AppState<C>>>,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let buffered = {
        let mut state = app_state.lock().await;
        state.remote_description_set.insert(device_id.to_string());
        state
            .pending_ice_candidates
            .remove(device_id)
            .unwrap_or_default()
    };
    for init in buffered {
        add_candidate(device_id, pc, init).await;
    }
}

async fn add_candidate(
    device_id: &str,
    pc: &Arc<dyn PeerConnection>,
    init: webrtc::peer_connection::RTCIceCandidateInit,
) {
    if let Err(e) = pc.add_ice_candidate(init).await {
        error!("❌ Failed to add ICE candidate from {}: {}", device_id, e);
    } else {
        info!("✅ Added ICE candidate from {}", device_id);
    }
}

/// Get an existing peer connection for `device_id`, or create + wire a new
/// one with a `SignalingHandler` (data-channel / connection-state / ICE
/// handling) installed at construction time.
async fn ensure_peer_connection<C>(
    device_id: &str,
    app_state: &Arc<Mutex<AppState<C>>>,
    tx_msg: &UnboundedSender<Message>,
    ws_tx: &UnboundedSender<String>,
) -> Option<Arc<dyn PeerConnection>>
where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let device_connections = {
        let state = app_state.lock().await;
        state.device_connections.clone()
    };
    let mut conns = device_connections.lock().await;
    if let Some(existing) = conns.get(device_id) {
        return Some(existing.clone());
    }

    info!(
        "📱 Creating peer connection for {} (to handle offer)",
        device_id
    );
    let pc = build_peer_connection(device_id.to_string(), app_state.clone(), tx_msg, ws_tx).await?;

    conns.insert(device_id.to_string(), pc.clone());
    Some(pc)
}

/// Build a peer connection with a `SignalingHandler` wired in at construction
/// time (webrtc 0.21 has no post-construction `on_*` registration — the whole
/// event surface is one handler passed to `PeerConnectionBuilder::with_handler`).
///
/// Shared by both sides: the offerer (`network::webrtc::initiate_webrtc_with_channel`,
/// which additionally calls `create_data_channel` + `create_offer` on the result) and
/// the answerer (`ensure_peer_connection` above, which calls `set_remote_description`
/// + `create_answer`). Both need identical ICE-candidate / connection-state / incoming
/// data-channel handling, which now lives entirely in `SignalingHandler`.
pub(crate) async fn build_peer_connection<C>(
    device_id: String,
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: &UnboundedSender<Message>,
    ws_tx: &UnboundedSender<String>,
) -> Option<Arc<dyn PeerConnection>>
where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let slot = crate::network::peer_recovery::PcSlot::default();
    let handler: Arc<dyn PeerConnectionEventHandler> = Arc::new(SignalingHandler {
        device_id: device_id.clone(),
        tx_msg: tx_msg.clone(),
        ws_tx: ws_tx.clone(),
        app_state,
        slot: slot.clone(),
    });

    let config = RTCConfigurationBuilder::new()
        .with_ice_servers(ice_servers())
        .build();
    match PeerConnectionBuilder::new()
        .with_configuration(config)
        .with_handler(handler)
        .with_udp_addrs(vec!["0.0.0.0:0".to_string()])
        .build()
        .await
    {
        Ok(pc) => {
            let pc = Arc::new(pc) as Arc<dyn PeerConnection>;
            let _ = slot.set(Arc::downgrade(&pc));
            Some(pc)
        }
        Err(e) => {
            error!(
                "❌ Failed to create peer connection for {}: {}",
                device_id, e
            );
            None
        }
    }
}

/// The single `PeerConnectionEventHandler` for a mesh peer connection.
///
/// Replaces the old per-closure `pc.on_ice_candidate` / `pc.on_data_channel` /
/// `pc.on_peer_connection_state_change` registrations. Installed once at
/// construction (see `build_peer_connection`), so it applies to a connection
/// regardless of which side (offerer or answerer) created it.
struct SignalingHandler<C: Ciphersuite + Send + Sync + 'static>
where
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    device_id: String,
    tx_msg: UnboundedSender<Message>,
    ws_tx: UnboundedSender<String>,
    app_state: Arc<Mutex<AppState<C>>>,
    /// The connection this handler belongs to; events from a connection that
    /// has since been replaced for `device_id` are ignored.
    slot: crate::network::peer_recovery::PcSlot,
}

#[async_trait::async_trait]
impl<C> PeerConnectionEventHandler for SignalingHandler<C>
where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        info!("🧊 Generated ICE candidate for {}", self.device_id);
        let Ok(c) = event.candidate.to_json() else {
            return;
        };
        let signal = crate::protocal::signal::WebRTCSignal::Candidate(
            crate::protocal::signal::CandidateInfo {
                candidate: c.candidate,
                sdp_mid: c.sdp_mid,
                sdp_mline_index: c.sdp_mline_index,
            },
        );
        let wrapper = crate::protocal::signal::WebSocketMessage::WebRTCSignal(signal);
        let Ok(payload) = serde_json::to_value(wrapper) else {
            return;
        };
        let relay = starlab_signal_server::ClientMsg::Relay {
            to: self.device_id.clone(),
            data: payload,
        };
        let Ok(json) = serde_json::to_string(&relay) else {
            return;
        };
        info!(
            "📤 Sending ICE candidate to {} via WebSocket",
            self.device_id
        );
        let _ = self.ws_tx.send(json);
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        if !crate::network::peer_recovery::is_current(&self.app_state, &self.device_id, &self.slot)
            .await
        {
            info!(
                "Ignoring {:?} from a replaced connection to {}",
                state, self.device_id
            );
            return;
        }
        // webrtc 0.21's `PeerConnection` trait has no synchronous/async `connection_state()`
        // accessor anymore (state is event-delivered only, see the crate's
        // `docs/transport-objects.md`), so callers that used to poll `pc.connection_state()`
        // (the mesh-readiness checkers in `elm::command`) now read this map instead.
        {
            let mut app_state = self.app_state.lock().await;
            app_state
                .device_statuses
                .insert(self.device_id.clone(), state);
        }
        let is_connected = matches!(state, RTCPeerConnectionState::Connected);
        let _ = self.tx_msg.send(Message::UpdateParticipantWebRTCStatus {
            device_id: self.device_id.clone(),
            webrtc_connected: is_connected,
            // Updated again when the data channel opens.
            data_channel_open: false,
        });
        match state {
            RTCPeerConnectionState::Connected => {
                info!("✅ WebRTC connection ESTABLISHED with {}", self.device_id);
            }
            RTCPeerConnectionState::Failed => {
                error!("❌ WebRTC connection FAILED with {}", self.device_id);
            }
            RTCPeerConnectionState::Disconnected => {
                tracing::warn!("⚠️ WebRTC connection DISCONNECTED from {}", self.device_id);
            }
            RTCPeerConnectionState::Closed => {
                info!("🔒 WebRTC connection CLOSED with {}", self.device_id);
            }
            other => info!(
                "WebRTC connection state with {}: {:?}",
                self.device_id, other
            ),
        }
        // Grace / rebuild (never block the event handler on it).
        tokio::spawn(crate::network::peer_recovery::handle_state_change(
            self.app_state.clone(),
            self.tx_msg.clone(),
            self.device_id.clone(),
            self.slot.clone(),
            state,
        ));
    }

    async fn on_data_channel(&self, dc: Arc<dyn DataChannel>) {
        let device_id = self.device_id.clone();
        let tx_msg = self.tx_msg.clone();
        let app_state = self.app_state.clone();
        info!("📂 Incoming data channel from {}", device_id);
        tokio::spawn(async move {
            run_answer_side_data_channel::<C>(dc, device_id, app_state, tx_msg).await;
        });
    }
}

/// Poll loop for a data channel WE received via `on_data_channel` (the
/// answerer side). Mirrors the offerer-side loop in
/// `network::webrtc::run_offer_side_data_channel`, minus the `channel_open` /
/// `mesh_ready` sends — those are only emitted by the side that created the
/// channel.
async fn run_answer_side_data_channel<C>(
    dc: Arc<dyn DataChannel>,
    device_id: String,
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: UnboundedSender<Message>,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    use webrtc::data_channel::DataChannelEvent;

    while let Some(event) = dc.poll().await {
        match event {
            DataChannelEvent::OnOpen => {
                info!("📂 Data channel OPENED from {}", device_id);
                {
                    let mut state = app_state.lock().await;
                    state.data_channels.insert(device_id.clone(), dc.clone());
                    info!(
                        "📦 Stored incoming data channel for {} in AppState",
                        device_id
                    );
                }
                crate::network::peer_recovery::resend_ceremony_frames(&app_state, &device_id, &dc)
                    .await;
                let _ = tx_msg.send(Message::UpdateParticipantWebRTCStatus {
                    device_id: device_id.clone(),
                    webrtc_connected: true,
                    data_channel_open: true,
                });
            }
            DataChannelEvent::OnMessage(msg) => {
                // Delegate to the shared dispatcher. This is the answerer path
                // (PC created via `on_data_channel` on the passive side); previously
                // this was a log-only stub, so any DKG package the initiator sent
                // across this DC direction was silently dropped. DKG Round 1 only
                // completed one-way and the protocol stalled at "Initialization".
                crate::network::webrtc::dispatch_data_channel_msg::<C>(
                    msg.data.to_vec(),
                    device_id.clone(),
                    app_state.clone(),
                    Some(tx_msg.clone()),
                )
                .await;
            }
            DataChannelEvent::OnClose => {
                info!("📪 Data channel CLOSED from {}", device_id);
                break;
            }
            _ => {}
        }
    }
}

/// Serialize + enqueue a WebRTC answer back to the peer that sent the offer.
fn send_answer(from_device: &str, sdp: String, ws_tx: &UnboundedSender<String>) {
    let signal =
        crate::protocal::signal::WebRTCSignal::Answer(crate::protocal::signal::SDPInfo { sdp });
    let wrapper = crate::protocal::signal::WebSocketMessage::WebRTCSignal(signal);
    let Ok(payload) = serde_json::to_value(wrapper) else {
        error!(
            "❌ Failed to serialize WebRTC answer wrapper for {}",
            from_device
        );
        return;
    };
    let relay = starlab_signal_server::ClientMsg::Relay {
        to: from_device.to_string(),
        data: payload,
    };
    let Ok(json) = serde_json::to_string(&relay) else {
        error!("❌ Failed to serialize Relay(Answer) for {}", from_device);
        return;
    };
    info!("📤 Sending WebRTC answer to {} via WebSocket", from_device);
    if let Err(e) = ws_tx.send(json) {
        error!("❌ Failed to enqueue answer for {}: {}", from_device, e);
    } else {
        info!("✅ WebRTC answer sent to {}", from_device);
    }
}

#[cfg(test)]
mod ice_server_tests {
    use super::*;

    #[test]
    fn stun_servers_are_valid_ice_urls() {
        // An invalid URL here makes every peer connection fail to build.
        for server in ice_servers() {
            for url in &server.urls {
                assert!(url.starts_with("stun:"), "{url}");
            }
        }
        let config = RTCConfigurationBuilder::new()
            .with_ice_servers(ice_servers())
            .build();
        assert_eq!(config.ice_servers().len(), 1);
    }
}
