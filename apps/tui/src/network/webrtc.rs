// WebRTC connection management for P2P mesh networking
// This implementation avoids Ciphersuite bounds for better modularity
use crate::protocal::signal::{SDPInfo, WebRTCSignal, WebSocketMessage};
use crate::utils::appstate_compat::AppState;
use serde_json;
use starlab_signal_server::ClientMsg as SharedClientMsg;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::PeerConnection;

/// Parse and react to a single frame received on a WebRTC data channel.
///
/// Both the initiator side (this file's `initiate_webrtc_with_channel`
/// creates a DC locally) and the answerer side (`elm/webrtc_signaling.rs`
/// receives a DC via `on_data_channel`) need to process DKG Round 1 / 2
/// packages, `mesh_ready` signals, etc. Previously the answerer's handler
/// was a log-only stub, so one direction of every DKG message was silently
/// dropped and Round 1 never completed. Extracted here so both sites call
/// the same body.
pub async fn dispatch_data_channel_msg<C>(
    msg_data: Vec<u8>,
    device_id_recv: String,
    app_state: Arc<Mutex<AppState<C>>>,
    ui_msg_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::elm::message::Message>>,
) where
    C: frost_core::Ciphersuite + Send + Sync + 'static,
    frost_core::keys::dkg::round1::Package<C>: serde::de::DeserializeOwned,
    frost_core::keys::dkg::round2::Package<C>: serde::de::DeserializeOwned,
    <<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
{
    info!(
        "📥 Received message from {} via data channel: {} bytes",
        device_id_recv,
        msg_data.len()
    );

    let text = match String::from_utf8(msg_data.clone()) {
        Ok(t) => t,
        Err(e) => {
            warn!(
                "DC message from {} is not UTF-8 ({}); first 32 bytes: {:?}",
                device_id_recv,
                e,
                &msg_data.iter().take(32).collect::<Vec<_>>()
            );
            return;
        }
    };
    // Log a prefix of the raw JSON to catch format drifts (prior attempts
    // showed "📥 Received message" firing and then no further log, meaning
    // the match below silently falls through — helps identify if the
    // payload is UTF-8 but not the shape we expect).
    info!(
        "  ↳ [{}] payload preview (first 160 chars): {}",
        device_id_recv,
        text.chars().take(160).collect::<String>()
    );
    let json_msg = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                "DC message from {} failed JSON parse: {} — raw text: {}",
                device_id_recv, e, text
            );
            return;
        }
    };
    info!(
        "  ↳ [{}] JSON keys at root: {:?}",
        device_id_recv,
        json_msg
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    );

    // `WebRTCMessage<C>` is serialised with `#[serde(tag = "webrtc_msg_type")]`
    // (internally tagged), so the JSON shape is `{"webrtc_msg_type":"SimpleMessage","text":"..."}`,
    // NOT `{"SimpleMessage":{"text":"..."}}`. The previous externally-tagged parser
    // silently dropped every DKG Round 1/2 package.
    let webrtc_tag = json_msg.get("webrtc_msg_type").and_then(|v| v.as_str());
    if webrtc_tag == Some("DkgRound1Package") {
        info!(
            "🔑 Received structured DKG Round 1 package from {}",
            device_id_recv
        );
        match json_msg
            .get("package")
            .cloned()
            .ok_or_else(|| "missing package".to_string())
            .and_then(|value| {
                serde_json::from_value::<frost_core::keys::dkg::round1::Package<C>>(value)
                    .map_err(|e| e.to_string())
            })
            .and_then(|package| package.serialize().map_err(|e| format!("{e:?}")))
        {
            Ok(package_bytes) => {
                if let Some(tx) = &ui_msg_tx {
                    let _ = tx.send(crate::elm::message::Message::ProcessDKGRound1 {
                        from_device: device_id_recv.clone(),
                        package_bytes,
                    });
                }
            }
            Err(e) => error!(
                "Failed to parse structured DKG Round 1 package from {}: {}",
                device_id_recv, e
            ),
        }
        return;
    }
    if webrtc_tag == Some("DkgRound2Package") {
        info!(
            "🔐 Received structured DKG Round 2 package from {}",
            device_id_recv
        );
        match json_msg
            .get("package")
            .cloned()
            .ok_or_else(|| "missing package".to_string())
            .and_then(|value| {
                serde_json::from_value::<frost_core::keys::dkg::round2::Package<C>>(value)
                    .map_err(|e| e.to_string())
            })
            .and_then(|package| package.serialize().map_err(|e| format!("{e:?}")))
        {
            Ok(package_bytes) => {
                if let Some(tx) = &ui_msg_tx {
                    let _ = tx.send(crate::elm::message::Message::ProcessDKGRound2 {
                        from_device: device_id_recv.clone(),
                        package_bytes,
                    });
                }
            }
            Err(e) => error!(
                "Failed to parse structured DKG Round 2 package from {}: {}",
                device_id_recv, e
            ),
        }
        return;
    }
    if webrtc_tag == Some("SimpleMessage")
        && let Some(msg_text) = json_msg.get("text").and_then(|v| v.as_str())
    {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        // A peer resends its ceremony frames when our data channel reopens
        // (`network::peer_recovery`); only the first copy reaches the protocol.
        if crate::network::peer_recovery::is_ceremony_frame(msg_text)
            && !app_state
                .lock()
                .await
                .seen_ceremony_frames
                .first_delivery(&device_id_recv, msg_text)
        {
            info!(
                "♻️ Ignoring duplicate ceremony frame from {}",
                device_id_recv
            );
            return;
        }
        // Threshold-ECDSA engine frames (protocol chunks + signing control):
        // handled by the engine directly, never through the Elm loop.
        if crate::protocal::ecdsa::is_ecdsa_frame(msg_text) {
            crate::protocal::ecdsa::on_frame(
                &app_state,
                &device_id_recv,
                msg_text,
                ui_msg_tx.as_ref(),
            )
            .await;
            return;
        }
        // Unified-DKG frames (ed25519 + secp256k1 in one ceremony). Same
        // SimpleMessage transport as the single-curve DKG rounds, but the
        // payload is JSON (not base64) and routes to the unified driver.
        if let Some(package_json) =
            msg_text.strip_prefix(crate::elm::command::UNIFIED_DKG_ROUND1_PREFIX)
        {
            info!("🔑 Received UNIFIED DKG Round 1 from {}", device_id_recv);
            if let Some(tx) = &ui_msg_tx {
                let _ = tx.send(crate::elm::message::Message::ProcessUnifiedDKGRound1 {
                    from_device: device_id_recv.clone(),
                    package_json: package_json.to_string(),
                });
            }
            return;
        }
        if let Some(message_json) =
            msg_text.strip_prefix(crate::elm::command::UNIFIED_DKG_ROUND2_PREFIX)
        {
            info!("🔐 Received UNIFIED DKG Round 2 from {}", device_id_recv);
            if let Some(tx) = &ui_msg_tx {
                let _ = tx.send(crate::elm::message::Message::ProcessUnifiedDKGRound2 {
                    from_device: device_id_recv.clone(),
                    message_json: message_json.to_string(),
                });
            }
            return;
        }
        if let Some(package_data) = msg_text.strip_prefix("DKG_ROUND1:") {
            info!("🔑 Received DKG Round 1 package from {}", device_id_recv);
            match BASE64.decode(package_data) {
                Ok(package_bytes) => {
                    info!(
                        "📦 Processing DKG Round 1 package from {} ({} bytes)",
                        device_id_recv,
                        package_bytes.len()
                    );
                    if let Some(tx) = &ui_msg_tx {
                        let _ = tx.send(crate::elm::message::Message::ProcessDKGRound1 {
                            from_device: device_id_recv.clone(),
                            package_bytes,
                        });
                    }
                }
                Err(e) => error!(
                    "Failed to base64-decode DKG Round 1 package from {}: {}",
                    device_id_recv, e
                ),
            }
            return;
        }
        if let Some(package_data) = msg_text.strip_prefix("DKG_ROUND2:") {
            info!("🔐 Received DKG Round 2 package from {}", device_id_recv);
            match BASE64.decode(package_data) {
                Ok(package_bytes) => {
                    info!(
                        "📦 Processing DKG Round 2 package from {} ({} bytes)",
                        device_id_recv,
                        package_bytes.len()
                    );
                    if let Some(tx) = &ui_msg_tx {
                        let _ = tx.send(crate::elm::message::Message::ProcessDKGRound2 {
                            from_device: device_id_recv.clone(),
                            package_bytes,
                        });
                    }
                }
                Err(e) => error!(
                    "Failed to base64-decode DKG Round 2 package from {}: {}",
                    device_id_recv, e
                ),
            }
            return;
        }
        // Reshare-round frames (#45): same shape as DKG rounds, different
        // prefix. Routed to the reshare driver via Message::ProcessReshareRound*.
        if let Some(package_data) = msg_text.strip_prefix("RESHARE_ROUND1:") {
            info!(
                "🔄 Received RESHARE Round 1 package from {}",
                device_id_recv
            );
            match BASE64.decode(package_data) {
                Ok(package_bytes) => {
                    if let Some(tx) = &ui_msg_tx {
                        let _ = tx.send(crate::elm::message::Message::ProcessReshareRound1 {
                            from_device: device_id_recv.clone(),
                            package_bytes,
                        });
                    }
                }
                Err(e) => error!(
                    "Failed to base64-decode RESHARE Round 1 package from {}: {}",
                    device_id_recv, e
                ),
            }
            return;
        }
        if let Some(package_data) = msg_text.strip_prefix("RESHARE_ROUND2:") {
            info!(
                "🔄 Received RESHARE Round 2 package from {}",
                device_id_recv
            );
            match BASE64.decode(package_data) {
                Ok(package_bytes) => {
                    if let Some(tx) = &ui_msg_tx {
                        let _ = tx.send(crate::elm::message::Message::ProcessReshareRound2 {
                            from_device: device_id_recv.clone(),
                            package_bytes,
                        });
                    }
                }
                Err(e) => error!(
                    "Failed to base64-decode RESHARE Round 2 package from {}: {}",
                    device_id_recv, e
                ),
            }
            return;
        }
        // Phase C: signing-round frames. Same shape as DKG rounds,
        // different prefix. Constants in `protocal::signing` keep the
        // string literals single-sourced.
        if let Some(b64) = msg_text.strip_prefix(crate::protocal::signing::SIGN_COMMIT_PREFIX) {
            info!("🖊️  Received SIGN_COMMIT from {}", device_id_recv);
            match BASE64.decode(b64) {
                Ok(commitment_bytes) => {
                    if let Some(tx) = &ui_msg_tx {
                        let _ = tx.send(crate::elm::message::Message::ProcessSigningRound1 {
                            from_device: device_id_recv.clone(),
                            commitment_bytes,
                        });
                    }
                }
                Err(e) => error!(
                    "Failed to base64-decode SIGN_COMMIT from {}: {}",
                    device_id_recv, e
                ),
            }
            return;
        }
        if let Some(b64) = msg_text.strip_prefix(crate::protocal::signing::SIGN_SHARE_PREFIX) {
            info!("🖊️  Received SIGN_SHARE from {}", device_id_recv);
            match BASE64.decode(b64) {
                Ok(share_bytes) => {
                    if let Some(tx) = &ui_msg_tx {
                        let _ = tx.send(crate::elm::message::Message::ProcessSigningRound2 {
                            from_device: device_id_recv.clone(),
                            share_bytes,
                        });
                    }
                }
                Err(e) => error!(
                    "Failed to base64-decode SIGN_SHARE from {}: {}",
                    device_id_recv, e
                ),
            }
            return;
        }
        // The proposer's fixed signer set. Same shape as SIGN_COMMIT/SIGN_SHARE
        // (base64 payload after the prefix); the payload itself is a JSON
        // array of device-id strings, decoded by the protocol layer.
        if let Some(b64) = msg_text.strip_prefix(crate::protocal::signing::SIGN_SET_PREFIX) {
            info!("🔏 Received SIGN_SET from {}", device_id_recv);
            match BASE64.decode(b64) {
                Ok(signer_set_bytes) => {
                    if let Some(tx) = &ui_msg_tx {
                        let _ = tx.send(crate::elm::message::Message::ProcessSigningSet {
                            from_device: device_id_recv.clone(),
                            signer_set_bytes,
                        });
                    }
                }
                Err(e) => error!(
                    "Failed to base64-decode SIGN_SET from {}: {}",
                    device_id_recv, e
                ),
            }
            return;
        }
        info!("📨 SimpleMessage from {}: {}", device_id_recv, msg_text);
        return;
    }

    // Control frames: `channel_open`, `mesh_ready`.
    if let Some(msg_type) = json_msg.get("type").and_then(|v| v.as_str()) {
        match msg_type {
            "channel_open" => info!("📂 Received channel_open from {}", device_id_recv),
            "mesh_ready" => {
                info!("✅ Received mesh_ready from {}", device_id_recv);
                let mut state = app_state.lock().await;
                state
                    .pending_mesh_ready_signals
                    .insert(device_id_recv.clone());

                let session = state.session.clone();
                if let Some(session) = session {
                    let expected_peers = session.participants.len().saturating_sub(1);
                    let ready_peers = state.pending_mesh_ready_signals.len();
                    if ready_peers >= expected_peers && !state.own_mesh_ready_sent {
                        info!("🎉 All {} peers mesh-ready", ready_peers);
                        state.mesh_status = crate::utils::state::MeshStatus::Ready;
                        state.own_mesh_ready_sent = true;
                        if let Some(tx) = &ui_msg_tx {
                            let _ = tx.send(crate::elm::message::Message::StartDKGProtocol);
                        }
                    }
                }
            }
            other => info!(
                "📨 Unknown JSON message type {} from {}",
                other, device_id_recv
            ),
        }
    }
}

/// Poll loop for a data channel WE created via `create_data_channel` (the
/// offerer side). webrtc 0.21 replaced `dc.on_open` / `dc.on_message`
/// closures with `DataChannel::poll()`, consumed here in a loop. Mirrored by
/// `elm::webrtc_signaling::run_answer_side_data_channel` for the answerer
/// side, which skips the `channel_open` / `mesh_ready` sends below — only
/// the side that created the channel emits those.
async fn run_offer_side_data_channel<C>(
    dc: Arc<dyn DataChannel>,
    device_id: String,
    self_device_id: String,
    app_state: Arc<Mutex<AppState<C>>>,
    ui_msg_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::elm::message::Message>>,
) where
    C: frost_core::Ciphersuite + 'static + Send + Sync,
    <<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
{
    while let Some(event) = dc.poll().await {
        match event {
            DataChannelEvent::OnOpen => {
                info!("📂 Data channel OPENED with {}", device_id);

                // Store the data channel in AppState for DKG messaging
                {
                    let mut state = app_state.lock().await;
                    state.data_channels.insert(device_id.clone(), dc.clone());
                    info!("📦 Stored data channel for {} in AppState", device_id);
                }
                crate::network::peer_recovery::resend_ceremony_frames(&app_state, &device_id, &dc)
                    .await;

                // Send UI update for data channel open
                if let Some(tx) = ui_msg_tx.clone() {
                    let _ = tx.send(
                        crate::elm::message::Message::UpdateParticipantWebRTCStatus {
                            device_id: device_id.clone(),
                            webrtc_connected: true,
                            data_channel_open: true,
                        },
                    );
                }

                // Send channel_open message to peer
                let channel_open_msg = serde_json::json!({
                    "type": "channel_open",
                    "payload": {
                        "device_id": self_device_id
                    }
                });

                if let Ok(msg_str) = serde_json::to_string(&channel_open_msg) {
                    let _ = dc.send_text(&msg_str).await;
                    info!("📤 Sent channel_open message to {}", device_id);
                }

                // Check if all channels are open and send mesh_ready if so
                let state = app_state.lock().await;
                let session = state.session.clone();
                let participants = session
                    .as_ref()
                    .map(|s| s.participants.clone())
                    .unwrap_or_default();
                let device_conns = state.device_connections.clone();
                let own_mesh_ready_sent = state.own_mesh_ready_sent;
                drop(state);

                // Check if all expected connections are established
                let conns = device_conns.lock().await;
                let expected_count = participants.len().saturating_sub(1); // Exclude self
                let connected_count = conns.len();
                drop(conns);

                if connected_count >= expected_count && expected_count > 0 && !own_mesh_ready_sent {
                    info!(
                        "✅ All {} peer connections established, sending mesh_ready",
                        connected_count
                    );

                    // Send mesh_ready to all peers
                    let mesh_ready_msg = serde_json::json!({
                        "type": "mesh_ready",
                        "payload": {
                            "session_id": session.as_ref().map(|s| s.session_id.clone()).unwrap_or_default(),
                            "device_id": self_device_id
                        }
                    });

                    if let Ok(msg_str) = serde_json::to_string(&mesh_ready_msg) {
                        let _ = dc.send_text(&msg_str).await;
                        info!("📤 Sent mesh_ready signal via data channel");

                        // Mark as sent and check if all participants are ready
                        let mut state = app_state.lock().await;
                        state.own_mesh_ready_sent = true;

                        // Since we're sending mesh_ready, check if we've received mesh_ready from all others
                        let ready_peers = state.pending_mesh_ready_signals.len();
                        let expected_peers = expected_count;

                        if ready_peers >= expected_peers {
                            info!("🎉 All peers ready - triggering DKG protocol!");
                            state.mesh_status = crate::utils::state::MeshStatus::Ready;

                            // Trigger DKG protocol start
                            if let Some(tx) = &ui_msg_tx {
                                info!("🚀 Sending StartDKGProtocol message!");
                                let _ = tx.send(crate::elm::message::Message::StartDKGProtocol);
                            }
                        }
                    }
                }
            }
            DataChannelEvent::OnMessage(msg) => {
                // Delegate to the shared dispatcher so initiator + answerer DCs both
                // run the same protocol handling (previously the answerer's on_message
                // was a log-only stub — DKG Round 1 packages went into the void on one
                // direction of every peer pair).
                dispatch_data_channel_msg::<C>(
                    msg.data.to_vec(),
                    device_id.clone(),
                    app_state.clone(),
                    ui_msg_tx.clone(),
                )
                .await;
            }
            DataChannelEvent::OnClose => {
                info!("📪 Data channel CLOSED with {}", device_id);
                break;
            }
            _ => {}
        }
    }
}

/// WebRTC connection initiation using existing WebSocket channel
pub async fn initiate_webrtc_with_channel<C>(
    self_device_id: String,
    participants: Vec<String>,
    device_connections: Arc<Mutex<HashMap<String, Arc<dyn PeerConnection>>>>,
    app_state: Arc<Mutex<AppState<C>>>,
    ui_msg_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::elm::message::Message>>,
) where
    C: frost_core::Ciphersuite + 'static + Send + Sync,
    <<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
{
    info!(
        "🚀 Simple WebRTC initiation for {} participants",
        participants.len()
    );

    // Get the WebSocket message channel from AppState (string-based for Send compatibility)
    let ws_msg_tx = {
        let state = app_state.lock().await;
        match &state.websocket_msg_tx {
            Some(tx) => {
                info!("✅ Got WebSocket message channel from AppState");
                tx.clone()
            }
            None => {
                error!(
                    "❌ No WebSocket message channel found in AppState - WebRTC offers cannot be sent!"
                );
                return;
            }
        }
    };

    // Create debug log
    let debug_msg = format!(
        "[{}] 🚀 simple_initiate_webrtc called: self={}, participants={:?}",
        chrono::Local::now().format("%H:%M:%S%.3f"),
        self_device_id,
        participants
    );
    let _ = std::fs::write(
        format!("/tmp/{}-webrtc-simple.log", self_device_id),
        &debug_msg,
    );

    // Filter out self
    let other_participants: Vec<String> = participants
        .into_iter()
        .filter(|p| p != &self_device_id)
        .collect();

    if other_participants.is_empty() {
        info!("No other participants to connect to");
        return;
    }

    // webrtc 0.21's `PeerConnectionEventHandler` is installed once at construction
    // (`PeerConnectionBuilder::with_handler`), so it needs a concrete `UnboundedSender<Message>`
    // rather than the `Option` this function threads through historically. When `ui_msg_tx` is
    // `None`, hand the handler a channel whose receiver is immediately dropped — every send on
    // it silently no-ops, reproducing the old `if let Some(tx) = ui_msg_tx { ... }` behavior.
    let tx_for_handler = match ui_msg_tx.clone() {
        Some(tx) => tx,
        None => {
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            tx
        }
    };

    // Pre-create PCs ONLY for peers we're going to initiate to (self_id < peer_id
    // in perfect-negotiation terms). For the "wait for offer" side we MUST NOT
    // create the PC here — if we do, the later offer arrives, `ensure_peer_connection`
    // in `webrtc_signaling.rs` sees an existing PC and returns it without attaching
    // `on_data_channel` / `on_ice_candidate` / `on_peer_connection_state_change`.
    // The answerer then establishes ICE but never stashes the incoming data channel
    // in `state.data_channels`, and the DKG Round 1 broadcast fails with
    // "Data channel not ready". Letting `ensure_peer_connection` create the PC
    // for answerers guarantees the handler set is installed exactly once.
    info!(
        "🔧 [{}] Creating peer connections for peers we initiate to (perfect negotiation)",
        self_device_id
    );

    // Only a PC created by THIS call gets an offer. Commands run as concurrent tasks and several
    // paths dispatch `InitiateWebRTCWithParticipants`, so initiations overlap. The existence
    // check and the insert therefore share one lock hold (as in `ensure_peer_connection`):
    // checking, releasing the lock across the build and inserting afterwards let two calls
    // each build a PC, the second replacing the first in the map after the first's offer was
    // out. The peer's answer to that first offer was then applied to the second PC, whose ICE
    // credentials never matched the peer's (`ErrMismatchUsername`), and the connection FAILED.
    // An existing PC already has its offer in flight; re-offering on it would race that
    // negotiation, so it is left alone.
    let mut devices_to_offer: Vec<String> = Vec::new();
    for participant in other_participants.iter() {
        if self_device_id >= *participant {
            // We're the answerer — wait for the offer, let ensure_peer_connection
            // create the PC with a full handler set.
            continue;
        }
        // The server drops an offer to a device it doesn't know, and the PC
        // would then sit in have-local-offer and block every later offer (a
        // peer that is restarting). Offer once it is back on the roster.
        if !app_state.lock().await.is_online(participant) {
            info!(
                "⏸ [{}] {} is not on the signal server; offering when it rejoins",
                self_device_id, participant
            );
            continue;
        }
        let mut conns = device_connections.lock().await;
        if conns.contains_key(participant) {
            info!(
                "✓ [{}] Peer connection already exists for {}, negotiation already started",
                self_device_id, participant
            );
            continue;
        }

        info!(
            "📱 [{}] Creating NEW peer connection for {}",
            self_device_id, participant
        );

        // Build the peer connection with the shared `SignalingHandler` installed
        // (webrtc 0.21: the handler is constructor-only, see `build_peer_connection`'s
        // doc comment). Both this offerer path and the answerer path in
        // `webrtc_signaling::ensure_peer_connection` go through the same helper so
        // every PC in the mesh gets identical ICE / connection-state / data-channel
        // wiring regardless of which side created it.
        match crate::elm::webrtc_signaling::build_peer_connection(
            participant.clone(),
            app_state.clone(),
            &tx_for_handler,
            &ws_msg_tx,
        )
        .await
        {
            Some(pc) => {
                conns.insert(participant.clone(), pc);
                devices_to_offer.push(participant.clone());
                info!(
                    "✅ [{}] Successfully created peer connection for {}",
                    self_device_id, participant
                );
            }
            None => {
                error!(
                    "❌ [{}] Failed to create peer connection for {}",
                    self_device_id, participant
                );
            }
        }
    }

    info!(
        "📤 [{}] Will send offers to {} devices: {:?}",
        self_device_id,
        devices_to_offer.len(),
        devices_to_offer
    );

    // IMPORTANT: Log what connections we expect to receive offers for
    let devices_expecting_offers: Vec<String> = other_participants
        .clone()
        .into_iter()
        .filter(|p| self_device_id > *p)
        .collect();

    if !devices_expecting_offers.is_empty() {
        info!(
            "📥 [{}] Expecting to receive offers from {} devices: {:?}",
            self_device_id,
            devices_expecting_offers.len(),
            devices_expecting_offers
        );
    }

    // Check current connections state before creating offers
    {
        let conns = device_connections.lock().await;
        info!(
            "📊 [{}] Current peer connections: {:?}",
            self_device_id,
            conns.keys().collect::<Vec<_>>()
        );
    }

    for device_id in devices_to_offer {
        let conns = device_connections.lock().await;
        let pc = conns.get(&device_id).cloned();
        drop(conns);
        if let Some(pc) = pc {
            info!("🎯 [{}] Creating offer for {}", self_device_id, device_id);

            // Connection-state / ICE-candidate handling is installed once, at PC
            // construction time, via the shared `SignalingHandler` — see
            // `elm::webrtc_signaling::build_peer_connection`. webrtc 0.21 has no
            // post-construction `pc.on_*` registration, so there is nothing to attach
            // here anymore.

            match pc.create_data_channel("data", None).await {
                Ok(dc) => {
                    info!("✅ Created data channel for {}", device_id);

                    // Poll loop replaces the old `dc.on_open` / `dc.on_message` closures
                    // (webrtc 0.21: `DataChannel` events are consumed via `poll()` in a
                    // loop rather than registered callbacks).
                    let dc_for_poll = dc.clone();
                    let device_id_poll = device_id.clone();
                    let self_device_id_poll = self_device_id.clone();
                    let app_state_poll = app_state.clone();
                    let ui_msg_tx_poll = ui_msg_tx.clone();
                    tokio::spawn(async move {
                        run_offer_side_data_channel::<C>(
                            dc_for_poll,
                            device_id_poll,
                            self_device_id_poll,
                            app_state_poll,
                            ui_msg_tx_poll,
                        )
                        .await;
                    });

                    // Now create offer
                    match pc.create_offer(None).await {
                        Ok(offer) => {
                            info!("✅ Created offer for {}", device_id);

                            // Set local description
                            if let Err(e) = pc.set_local_description(offer.clone()).await {
                                error!("Failed to set local description: {}", e);
                            } else {
                                info!("✅ Set local description for {}", device_id);

                                // Send offer via existing WebSocket channel
                                let signal = WebRTCSignal::Offer(SDPInfo { sdp: offer.sdp });
                                let websocket_message = WebSocketMessage::WebRTCSignal(signal);

                                match serde_json::to_value(websocket_message) {
                                    Ok(json_val) => {
                                        let relay_msg = SharedClientMsg::Relay {
                                            to: device_id.clone(),
                                            data: json_val,
                                        };

                                        // Serialize the message immediately to avoid Send issues
                                        match serde_json::to_string(&relay_msg) {
                                            Ok(json) => {
                                                info!(
                                                    "📤 Sending WebRTC offer to {} via WebSocket",
                                                    device_id
                                                );
                                                if let Err(e) = ws_msg_tx.send(json) {
                                                    error!(
                                                        "❌ Failed to send offer to {}: {}",
                                                        device_id, e
                                                    );
                                                } else {
                                                    info!(
                                                        "✅ WebRTC offer sent to {} via WebSocket",
                                                        device_id
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                error!(
                                                    "❌ Failed to serialize relay message for {}: {}",
                                                    device_id, e
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "❌ Failed to serialize offer for {}: {}",
                                            device_id, e
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("❌ Failed to create offer for {}: {}", device_id, e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Failed to create data channel for {}: {}", device_id, e);
                }
            }
        }
    }

    info!("✅ Simple WebRTC initiation complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use frost_secp256k1_tr::Secp256K1Sha256TR;

    /// Two `InitiateWebRTCWithParticipants` commands can run at the same time (each command is
    /// its own spawned task). They must end up with ONE peer connection per peer and send ONE
    /// offer: a second PC would replace the first in `device_connections`, and the answer to
    /// the first PC's offer would then be applied to the second — its ICE credentials never
    /// match the peer's, so the connection fails with `ErrMismatchUsername`.
    #[tokio::test]
    async fn concurrent_initiations_create_one_peer_connection_and_one_offer() {
        let (ws_tx, mut ws_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let mut state = AppState::<Secp256K1Sha256TR>::new();
        state.websocket_msg_tx = Some(ws_tx);
        let device_connections = state.device_connections.clone();
        let app_state = Arc::new(Mutex::new(state));
        let participants = vec!["test-offerer-a".to_string(), "test-offerer-b".to_string()];

        tokio::join!(
            initiate_webrtc_with_channel(
                "test-offerer-a".to_string(),
                participants.clone(),
                device_connections.clone(),
                app_state.clone(),
                None,
            ),
            initiate_webrtc_with_channel(
                "test-offerer-a".to_string(),
                participants.clone(),
                device_connections.clone(),
                app_state.clone(),
                None,
            ),
        );

        let mut offers = 0;
        while let Ok(frame) = ws_rx.try_recv() {
            if frame.contains("\"Offer\"") {
                offers += 1;
            }
        }
        assert_eq!(offers, 1, "exactly one offer must be sent to the peer");
        assert_eq!(device_connections.lock().await.len(), 1);
    }
}
