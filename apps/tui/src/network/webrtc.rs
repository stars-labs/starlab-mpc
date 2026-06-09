// WebRTC connection management for P2P mesh networking
// This implementation avoids Ciphersuite bounds for better modularity
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use webrtc::peer_connection::RTCPeerConnection;
use tracing::{info, error, warn};
use crate::protocal::signal::{WebRTCSignal, SDPInfo, WebSocketMessage};
use webrtc_signal_server::ClientMsg as SharedClientMsg;
use crate::utils::appstate_compat::AppState;
use serde_json;

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
    if webrtc_tag == Some("SimpleMessage")
        && let Some(msg_text) = json_msg.get("text").and_then(|v| v.as_str()) {
            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
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
                info!("🔄 Received RESHARE Round 1 package from {}", device_id_recv);
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
                info!("🔄 Received RESHARE Round 2 package from {}", device_id_recv);
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
                            let _ = tx.send(
                                crate::elm::message::Message::ProcessSigningRound1 {
                                    from_device: device_id_recv.clone(),
                                    commitment_bytes,
                                },
                            );
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
                            let _ = tx.send(
                                crate::elm::message::Message::ProcessSigningRound2 {
                                    from_device: device_id_recv.clone(),
                                    share_bytes,
                                },
                            );
                        }
                    }
                    Err(e) => error!(
                        "Failed to base64-decode SIGN_SHARE from {}: {}",
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
            other => info!("📨 Unknown JSON message type {} from {}", other, device_id_recv),
        }
    }
}

/// WebRTC connection initiation using existing WebSocket channel
pub async fn initiate_webrtc_with_channel<C>(
    self_device_id: String,
    participants: Vec<String>,
    device_connections: Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>,
    app_state: Arc<Mutex<AppState<C>>>,
    ui_msg_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::elm::message::Message>>,
) where
    C: frost_core::Ciphersuite + 'static + Send + Sync,
    <<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
{
    info!("🚀 Simple WebRTC initiation for {} participants", participants.len());

    // Get the WebSocket message channel from AppState (string-based for Send compatibility)
    let ws_msg_tx = {
        let state = app_state.lock().await;
        match &state.websocket_msg_tx {
            Some(tx) => {
                info!("✅ Got WebSocket message channel from AppState");
                tx.clone()
            }
            None => {
                error!("❌ No WebSocket message channel found in AppState - WebRTC offers cannot be sent!");
                return;
            }
        }
    };

    // Create debug log
    let debug_msg = format!(
        "[{}] 🚀 simple_initiate_webrtc called: self={}, participants={:?}",
        chrono::Local::now().format("%H:%M:%S%.3f"),
        self_device_id, participants
    );
    let _ = std::fs::write(format!("/tmp/{}-webrtc-simple.log", self_device_id), &debug_msg);

    // Filter out self
    let other_participants: Vec<String> = participants
        .into_iter()
        .filter(|p| p != &self_device_id)
        .collect();

    if other_participants.is_empty() {
        info!("No other participants to connect to");
        return;
    }

    // Pre-create PCs ONLY for peers we're going to initiate to (self_id < peer_id
    // in perfect-negotiation terms). For the "wait for offer" side we MUST NOT
    // create the PC here — if we do, the later offer arrives, `ensure_peer_connection`
    // in `webrtc_signaling.rs` sees an existing PC and returns it without attaching
    // `on_data_channel` / `on_ice_candidate` / `on_peer_connection_state_change`.
    // The answerer then establishes ICE but never stashes the incoming data channel
    // in `state.data_channels`, and the DKG Round 1 broadcast fails with
    // "Data channel not ready". Letting `ensure_peer_connection` create the PC
    // for answerers guarantees the handler set is installed exactly once.
    info!("🔧 [{}] Creating peer connections for peers we initiate to (perfect negotiation)",
         self_device_id);

    for participant in other_participants.iter() {
        if self_device_id >= *participant {
            // We're the answerer — wait for the offer, let ensure_peer_connection
            // create the PC with a full handler set.
            continue;
        }
        let needs_creation = {
            let conns = device_connections.lock().await;
            !conns.contains_key(participant)
        };

        if needs_creation {
            info!("📱 [{}] Creating NEW peer connection for {}", self_device_id, participant);

            // Create a simple peer connection using webrtc crate directly
            let config = webrtc::peer_connection::configuration::RTCConfiguration {
                ice_servers: vec![],
                ..Default::default()
            };

            match webrtc::api::APIBuilder::new()
                .build()
                .new_peer_connection(config)
                .await
            {
                Ok(pc) => {
                    let mut conns = device_connections.lock().await;
                    conns.insert(participant.clone(), Arc::new(pc));
                    info!("✅ [{}] Successfully created peer connection for {}", self_device_id, participant);
                }
                Err(e) => {
                    error!("❌ [{}] Failed to create peer connection for {}: {}", self_device_id, participant, e);
                }
            }
        } else {
            info!("✓ [{}] Peer connection already exists for {}, will reuse", self_device_id, participant);
        }
    }

    // Now create offers for participants where we have lower ID (perfect negotiation)
    let devices_to_offer: Vec<String> = other_participants.clone()
        .into_iter()
        .filter(|p| self_device_id < *p)
        .collect();

    info!("📤 [{}] Will send offers to {} devices: {:?}", self_device_id, devices_to_offer.len(), devices_to_offer);
    
    // IMPORTANT: Log what connections we expect to receive offers for
    let devices_expecting_offers: Vec<String> = other_participants.clone()
        .into_iter()
        .filter(|p| self_device_id > *p)
        .collect();
    
    if !devices_expecting_offers.is_empty() {
        info!("📥 [{}] Expecting to receive offers from {} devices: {:?}", 
               self_device_id, devices_expecting_offers.len(), devices_expecting_offers);
    }

    // Check current connections state before creating offers
    {
        let conns = device_connections.lock().await;
        info!("📊 [{}] Current peer connections: {:?}", self_device_id, conns.keys().collect::<Vec<_>>());
    }

    for device_id in devices_to_offer {
        // Check if we already have a data channel for this participant
        let has_data_channel = {
            let state = app_state.lock().await;
            state.data_channels.contains_key(&device_id)
        };

        if has_data_channel {
            info!("✓ [{}] Data channel already exists for {}, skipping offer creation", self_device_id, device_id);
            continue;
        }

        let conns = device_connections.lock().await;
        if let Some(pc) = conns.get(&device_id) {
            info!("🎯 [{}] Creating offer for {}", self_device_id, device_id);

            // Create data channel first
            // Set up connection state handler
            let device_id_state = device_id.clone();
            let ui_msg_tx_state = ui_msg_tx.clone();
            pc.on_peer_connection_state_change(Box::new(move |state: webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState| {
                let device_id_state = device_id_state.clone();
                let ui_msg_tx_state = ui_msg_tx_state.clone();
                Box::pin(async move {
                    let is_connected = matches!(state, webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Connected);
                    
                    // Send UI update
                    if let Some(tx) = ui_msg_tx_state {
                        let _ = tx.send(crate::elm::message::Message::UpdateParticipantWebRTCStatus {
                            device_id: device_id_state.clone(),
                            webrtc_connected: is_connected,
                            data_channel_open: false, // Will be updated when data channel opens
                        });
                    }
                    
                    match state {
                        webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Connected => {
                            info!("✅ WebRTC connection ESTABLISHED with {}", device_id_state);
                        }
                        webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Failed => {
                            error!("❌ WebRTC connection FAILED with {}", device_id_state);
                        }
                        webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Disconnected => {
                            warn!("⚠️ WebRTC connection DISCONNECTED from {}", device_id_state);
                        }
                        webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Closed => {
                            info!("🔒 WebRTC connection CLOSED with {}", device_id_state);
                        }
                        _ => {
                            info!("WebRTC connection state with {}: {:?}", device_id_state, state);
                        }
                    }
                })
            }));

            // Set up ICE candidate handler before creating offer
            let device_id_ice = device_id.clone();
            let ws_msg_tx_ice = ws_msg_tx.clone();
            let _pc_weak = Arc::downgrade(pc);

            pc.on_ice_candidate(Box::new(move |candidate: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
                let device_id_ice = device_id_ice.clone();
                let ws_msg_tx_ice = ws_msg_tx_ice.clone();
                let _pc_weak = _pc_weak.clone();

                Box::pin(async move {
                    if let Some(candidate) = candidate {
                        info!("🧊 Generated ICE candidate for {}", device_id_ice);

                        // Send ICE candidate to peer
                        let candidate_json = candidate.to_json().unwrap();
                        let ice_signal = crate::protocal::signal::WebRTCSignal::Candidate(
                            crate::protocal::signal::CandidateInfo {
                                candidate: candidate_json.candidate,
                                sdp_mid: candidate_json.sdp_mid,
                                sdp_mline_index: candidate_json.sdp_mline_index,
                            }
                        );

                        let websocket_message = crate::protocal::signal::WebSocketMessage::WebRTCSignal(ice_signal);

                        if let Ok(json_val) = serde_json::to_value(websocket_message) {
                            let relay_msg = webrtc_signal_server::ClientMsg::Relay {
                                to: device_id_ice.clone(),
                                data: json_val,
                            };

                            if let Ok(json) = serde_json::to_string(&relay_msg) {
                                info!("📤 Sending ICE candidate to {} via WebSocket", device_id_ice);
                                let _ = ws_msg_tx_ice.send(json);
                            }
                        }
                    }
                })
            }));

            match pc.create_data_channel("data", None).await {
                Ok(dc) => {
                    info!("✅ Created data channel for {}", device_id);

                    // Set up data channel handlers
                    let device_id_dc = device_id.clone();
                    let self_device_id_dc = self_device_id.clone();
                    let dc_for_open = dc.clone();
                    let app_state_for_mesh = app_state.clone();
                    
                    let ui_msg_tx_open = ui_msg_tx.clone();
                    dc.on_open(Box::new(move || {
                        let device_id_open = device_id_dc.clone();
                        let self_id = self_device_id_dc.clone();
                        let dc_open = dc_for_open.clone();
                        let app_state_mesh = app_state_for_mesh.clone();
                        let ui_msg_tx_open = ui_msg_tx_open;
                        
                        Box::pin(async move {
                            info!("📂 Data channel OPENED with {}", device_id_open);
                            
                            // Store the data channel in AppState for DKG messaging
                            {
                                let mut state = app_state_mesh.lock().await;
                                state.data_channels.insert(device_id_open.clone(), dc_open.clone());
                                info!("📦 Stored data channel for {} in AppState", device_id_open);
                            }
                            
                            // Send UI update for data channel open
                            if let Some(tx) = ui_msg_tx_open.clone() {
                                let _ = tx.send(crate::elm::message::Message::UpdateParticipantWebRTCStatus {
                                    device_id: device_id_open.clone(),
                                    webrtc_connected: true,
                                    data_channel_open: true,
                                });
                            }
                            
                            // Send channel_open message to peer
                            let channel_open_msg = serde_json::json!({
                                "type": "channel_open",
                                "payload": {
                                    "device_id": self_id
                                }
                            });
                            
                            if let Ok(msg_str) = serde_json::to_string(&channel_open_msg) {
                                let _ = dc_open.send_text(msg_str).await;
                                info!("📤 Sent channel_open message to {}", device_id_open);
                            }
                            
                            // Check if all channels are open and send mesh_ready if so
                            // Note: Cannot use tokio::spawn due to Send constraints
                            // Small delay to allow other channels to open  
                            
                            let state = app_state_mesh.lock().await;
                            let session = state.session.clone();
                            let participants = session.as_ref().map(|s| s.participants.clone()).unwrap_or_default();
                            let device_conns = state.device_connections.clone();
                            let own_mesh_ready_sent = state.own_mesh_ready_sent;
                            drop(state);
                            
                            // Check if all expected connections are established
                            let conns = device_conns.lock().await;
                            let expected_count = participants.len().saturating_sub(1); // Exclude self
                            let connected_count = conns.len();
                            
                            if connected_count >= expected_count && expected_count > 0 && !own_mesh_ready_sent {
                                info!("✅ All {} peer connections established, sending mesh_ready", connected_count);
                                
                                // Send mesh_ready to all peers
                                let mesh_ready_msg = serde_json::json!({
                                    "type": "mesh_ready",
                                    "payload": {
                                        "session_id": session.as_ref().map(|s| s.session_id.clone()).unwrap_or_default(),
                                        "device_id": self_id
                                    }
                                });
                                
                                if let Ok(msg_str) = serde_json::to_string(&mesh_ready_msg) {
                                    let _ = dc_open.send_text(msg_str).await;
                                    info!("📤 Sent mesh_ready signal via data channel");
                                    
                                    // Mark as sent and check if all participants are ready
                                    let mut state = app_state_mesh.lock().await;
                                    state.own_mesh_ready_sent = true;
                                    
                                    // Since we're sending mesh_ready, check if we've received mesh_ready from all others
                                    let ready_peers = state.pending_mesh_ready_signals.len();
                                    let expected_peers = expected_count;
                                    
                                    if ready_peers >= expected_peers {
                                        info!("🎉 All peers ready - triggering DKG protocol!");
                                        state.mesh_status = crate::utils::state::MeshStatus::Ready;
                                        
                                        // Trigger DKG protocol start
                                        if let Some(tx) = &ui_msg_tx_open {
                                            info!("🚀 Sending StartDKGProtocol message!");
                                            let _ = tx.send(crate::elm::message::Message::StartDKGProtocol);
                                        }
                                    }
                                }
                            }
                        })
                    }));

                    let device_id_msg = device_id.clone();
                    let app_state_for_msg = app_state.clone();
                    let ui_msg_tx_for_msg = ui_msg_tx.clone();
                    // Delegate to the shared dispatcher so initiator + answerer DCs
                    // both run the same protocol handling (previously the answerer's
                    // on_message was a log-only stub — DKG Round 1 packages went into
                    // the void on one direction of every peer pair).
                    dc.on_message(Box::new(move |msg: webrtc::data_channel::data_channel_message::DataChannelMessage| {
                        let device_id_recv = device_id_msg.clone();
                        let app_state_msg = app_state_for_msg.clone();
                        let ui_msg_tx_msg = ui_msg_tx_for_msg.clone();
                        Box::pin(async move {
                            crate::network::webrtc::dispatch_data_channel_msg::<C>(
                                msg.data.to_vec(),
                                device_id_recv,
                                app_state_msg,
                                ui_msg_tx_msg,
                            )
                            .await;
                        })
                    }));

                    // TODO: Store the data channel for sending messages
                    // Note: Cannot access AppState here due to Ciphersuite Send constraint

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
                                                info!("📤 Sending WebRTC offer to {} via WebSocket", device_id);
                                                if let Err(e) = ws_msg_tx.send(json) {
                                                    error!("❌ Failed to send offer to {}: {}", device_id, e);
                                                } else {
                                                    info!("✅ WebRTC offer sent to {} via WebSocket", device_id);
                                                }
                                            }
                                            Err(e) => {
                                                error!("❌ Failed to serialize relay message for {}: {}", device_id, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("❌ Failed to serialize offer for {}: {}", device_id, e);
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