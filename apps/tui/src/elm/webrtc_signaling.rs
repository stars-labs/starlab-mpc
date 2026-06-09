//! WebRTC signaling handler — the consumer side of the signal-server `Relay` frame.
//!
//! Every DKG driver (creator and joiner) needs to react to WebRTC offer / answer /
//! ICE-candidate traffic the same way: open a peer connection, set SDP, generate
//! a counter-message, shove it back through the shared WebSocket. Before this
//! module the logic lived inline, twice, inside `Command::StartDKG` and
//! `Command::JoinDKG` — ~400 lines of deeply nested callbacks, copy-pasted.
//! Now both drivers just forward each `ServerMsg::Relay { from, data }` here.

use crate::elm::message::Message;
use crate::utils::appstate_compat::AppState;
use frost_core::{Ciphersuite, Field, Group};
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use tracing::{error, info};

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
        handle_server_frame(
            data,
            app_state,
            tx_msg,
            self_device_id,
            our_session_id,
        )
        .await;
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
                message: format!("📥 Received WebRTC offer from {}, preparing answer...", from),
            });
            spawn_offer_handler(
                from,
                sdp.to_string(),
                app_state,
                tx_msg,
                self_device_id,
            );
        }
    } else if let Some(answer_data) = data.get("Answer") {
        if let Some(sdp) = answer_data.get("sdp").and_then(|v| v.as_str()) {
            let _ = tx_msg.send(Message::Info {
                message: format!("📥 Received WebRTC answer from {}, setting remote description...", from),
            });
            spawn_answer_handler(from, sdp.to_string(), app_state);
        }
    } else if let Some(ice_data) = data.get("Candidate")
        && let (Some(candidate), Some(sdp_mid), Some(sdp_mline_index)) = (
            ice_data.get("candidate").and_then(|v| v.as_str()),
            ice_data.get("sdpMid").and_then(|v| v.as_str()),
            ice_data.get("sdpMLineIndex").and_then(|v| v.as_u64()),
        ) {
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
        && (participants_len as u64) < total {
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
        message: format!("📡 Triggered WebRTC with participants: {:?}", new_participants),
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
                    error!("❌ No primary WebSocket channel when answering {}", from_device);
                    return;
                }
            }
        };

        let pc = match ensure_peer_connection(&from_device, &app_state, &tx_msg, &ws_tx).await {
            Some(pc) => pc,
            None => return,
        };

        let offer = match webrtc::peer_connection::sdp::session_description::RTCSessionDescription::offer(sdp) {
            Ok(s) => s,
            Err(e) => {
                error!("❌ Invalid offer SDP from {}: {}", from_device, e);
                return;
            }
        };
        if let Err(e) = pc.set_remote_description(offer).await {
            error!("❌ Failed to set remote description for {}: {}", from_device, e);
            return;
        }
        info!("✅ Set remote description (offer) from {}", from_device);

        let answer = match pc.create_answer(None).await {
            Ok(a) => a,
            Err(e) => {
                error!("❌ Failed to create answer for {}: {}", from_device, e);
                return;
            }
        };
        info!("✅ Created answer for {}", from_device);

        if let Err(e) = pc.set_local_description(answer.clone()).await {
            error!("❌ Failed to set local description for {}: {}", from_device, e);
            return;
        }
        info!("✅ Set local description (answer) for {}", from_device);

        send_answer(&from_device, answer.sdp, &ws_tx);
    });
}

/// Spawn a task to consume a remote answer to our offer (just set remote SDP).
fn spawn_answer_handler<C>(
    from_device: String,
    sdp: String,
    app_state: Arc<Mutex<AppState<C>>>,
) where
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
            error!("❌ No peer connection found for {} when receiving answer", from_device);
            return;
        };
        drop(conns);

        let answer = match webrtc::peer_connection::sdp::session_description::RTCSessionDescription::answer(sdp) {
            Ok(a) => a,
            Err(e) => {
                error!("❌ Invalid answer SDP from {}: {}", from_device, e);
                return;
            }
        };
        if let Err(e) = pc.set_remote_description(answer).await {
            error!("❌ Failed to set remote description (answer) for {}: {}", from_device, e);
        } else {
            info!("✅ Set remote description (answer) from {}, connection establishing", from_device);
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
        info!("🎯 Adding ICE candidate from {}", from_device);
        let device_connections = {
            let state = app_state.lock().await;
            state.device_connections.clone()
        };
        let conns = device_connections.lock().await;
        let Some(pc) = conns.get(&from_device).cloned() else {
            error!("❌ No peer connection found for {} when adding ICE candidate", from_device);
            return;
        };
        drop(conns);
        let init = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
            candidate,
            sdp_mid: Some(sdp_mid),
            sdp_mline_index: Some(sdp_mline_index),
            username_fragment: None,
        };
        if let Err(e) = pc.add_ice_candidate(init).await {
            error!("❌ Failed to add ICE candidate from {}: {}", from_device, e);
        } else {
            info!("✅ Added ICE candidate from {}", from_device);
        }
    });
}

/// Get an existing peer connection for `device_id`, or create + wire a new
/// one with data-channel / connection-state / ICE handlers attached.
async fn ensure_peer_connection<C>(
    device_id: &str,
    app_state: &Arc<Mutex<AppState<C>>>,
    tx_msg: &UnboundedSender<Message>,
    ws_tx: &UnboundedSender<String>,
) -> Option<Arc<webrtc::peer_connection::RTCPeerConnection>>
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

    info!("📱 Creating peer connection for {} (to handle offer)", device_id);
    let config = webrtc::peer_connection::configuration::RTCConfiguration {
        ice_servers: vec![],
        ..Default::default()
    };
    let pc = match webrtc::api::APIBuilder::new()
        .build()
        .new_peer_connection(config)
        .await
    {
        Ok(pc) => Arc::new(pc),
        Err(e) => {
            error!("❌ Failed to create peer connection for {}: {}", device_id, e);
            return None;
        }
    };

    attach_data_channel_handler(&pc, device_id.to_string(), tx_msg.clone(), app_state.clone());
    attach_connection_state_handler(&pc, device_id.to_string(), tx_msg.clone());
    attach_ice_candidate_handler(&pc, device_id.to_string(), ws_tx.clone());

    conns.insert(device_id.to_string(), pc.clone());
    Some(pc)
}

fn attach_data_channel_handler<C>(
    pc: &Arc<webrtc::peer_connection::RTCPeerConnection>,
    device_id: String,
    tx_msg: UnboundedSender<Message>,
    app_state: Arc<Mutex<AppState<C>>>,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    pc.on_data_channel(Box::new(move |dc: Arc<webrtc::data_channel::RTCDataChannel>| {
        let device_id = device_id.clone();
        let tx_msg = tx_msg.clone();
        let app_state = app_state.clone();
        Box::pin(async move {
            info!("📂 Incoming data channel from {}: {}", device_id, dc.label());

            // on_open: stash the data channel in AppState and notify the UI.
            let dc_for_open = dc.clone();
            let device_open = device_id.clone();
            let tx_open = tx_msg.clone();
            let app_state_open = app_state.clone();
            dc.on_open(Box::new(move || {
                let dc = dc_for_open.clone();
                let device_id = device_open.clone();
                let tx_msg = tx_open.clone();
                let app_state = app_state_open;
                Box::pin(async move {
                    info!("📂 Data channel OPENED from {}", device_id);
                    {
                        let mut state = app_state.lock().await;
                        state.data_channels.insert(device_id.clone(), dc.clone());
                        info!("📦 Stored incoming data channel for {} in AppState", device_id);
                    }
                    let _ = tx_msg.send(Message::UpdateParticipantWebRTCStatus {
                        device_id,
                        webrtc_connected: true,
                        data_channel_open: true,
                    });
                })
            }));

            // Delegate to the shared dispatcher. This is the answerer path
            // (PC created via `on_data_channel` on the passive side); previously
            // this was a log-only stub, so any DKG package the initiator sent
            // across this DC direction was silently dropped. DKG Round 1 only
            // completed one-way and the protocol stalled at "Initialization".
            let device_msg = device_id.clone();
            let app_state_msg = app_state.clone();
            let tx_for_msg = tx_msg.clone();
            dc.on_message(Box::new(move |msg: webrtc::data_channel::data_channel_message::DataChannelMessage| {
                let device_id_recv = device_msg.clone();
                let app_state_msg = app_state_msg.clone();
                let tx_for_msg = tx_for_msg.clone();
                Box::pin(async move {
                    crate::network::webrtc::dispatch_data_channel_msg::<C>(
                        msg.data.to_vec(),
                        device_id_recv,
                        app_state_msg,
                        Some(tx_for_msg),
                    )
                    .await;
                })
            }));
        })
    }));
}

fn attach_connection_state_handler(
    pc: &Arc<webrtc::peer_connection::RTCPeerConnection>,
    device_id: String,
    tx_msg: UnboundedSender<Message>,
) {
    pc.on_peer_connection_state_change(Box::new(
        move |state: webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState| {
            let device_id = device_id.clone();
            let tx_msg = tx_msg.clone();
            Box::pin(async move {
                let is_connected = matches!(
                    state,
                    webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Connected
                );
                let _ = tx_msg.send(Message::UpdateParticipantWebRTCStatus {
                    device_id: device_id.clone(),
                    webrtc_connected: is_connected,
                    // Updated again when the data channel opens.
                    data_channel_open: false,
                });
                match state {
                    webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Connected => {
                        info!("✅ WebRTC connection ESTABLISHED with {} (from answer)", device_id);
                    }
                    webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Failed => {
                        error!("❌ WebRTC connection FAILED with {} (from answer)", device_id);
                    }
                    other => info!("WebRTC connection state with {} (from answer): {:?}", device_id, other),
                }
            })
        },
    ));
}

fn attach_ice_candidate_handler(
    pc: &Arc<webrtc::peer_connection::RTCPeerConnection>,
    device_id: String,
    ws_tx: UnboundedSender<String>,
) {
    pc.on_ice_candidate(Box::new(
        move |candidate: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
            let device_id = device_id.clone();
            let ws_tx = ws_tx.clone();
            Box::pin(async move {
                let Some(candidate) = candidate else {
                    return;
                };
                info!("🧊 Generated ICE candidate for {}", device_id);
                let Ok(c) = candidate.to_json() else {
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
                let relay = webrtc_signal_server::ClientMsg::Relay {
                    to: device_id.clone(),
                    data: payload,
                };
                let Ok(json) = serde_json::to_string(&relay) else {
                    return;
                };
                info!("📤 Sending ICE candidate to {} via WebSocket", device_id);
                let _ = ws_tx.send(json);
            })
        },
    ));
}

/// Serialize + enqueue a WebRTC answer back to the peer that sent the offer.
fn send_answer(from_device: &str, sdp: String, ws_tx: &UnboundedSender<String>) {
    let signal = crate::protocal::signal::WebRTCSignal::Answer(
        crate::protocal::signal::SDPInfo { sdp },
    );
    let wrapper = crate::protocal::signal::WebSocketMessage::WebRTCSignal(signal);
    let Ok(payload) = serde_json::to_value(wrapper) else {
        error!("❌ Failed to serialize WebRTC answer wrapper for {}", from_device);
        return;
    };
    let relay = webrtc_signal_server::ClientMsg::Relay {
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
