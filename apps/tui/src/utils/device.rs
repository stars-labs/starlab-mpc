use crate::protocal::signal::{WebRTCMessage, WebRTCSignal};
use crate::utils::appstate_compat::AppState;
use crate::utils::state::{DkgState, InternalCommand};
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;

use frost_core::Ciphersuite;

use webrtc_signal_server::ClientMsg as SharedClientMsg;
use crate::protocal::signal::{CandidateInfo, WebSocketMessage}; // Updated path


pub const DATA_CHANNEL_LABEL: &str = "frost-dkg"; 

pub async fn send_webrtc_message<C>(
    target_device_id: &str,
    message: &WebRTCMessage<C>,
    state_log: Arc<Mutex<AppState<C>>>,
) -> Result<(), String> where C: Ciphersuite {
    // Enhanced debugging to trace data channel access
    let data_channel = {
        let guard = state_log.lock().await;
        tracing::debug!("🔍 Looking for data channel for device: {}", target_device_id);
        tracing::debug!("🔍 Available data channels: {:?}", guard.data_channels.keys().collect::<Vec<_>>());
        guard.data_channels.get(target_device_id).cloned()
    };

    if let Some(dc) = data_channel {
        let ready_state = dc.ready_state();
        tracing::debug!("🔍 Data channel for {} found, state: {:?}", target_device_id, ready_state);
        
        if ready_state == RTCDataChannelState::Open {
            let msg_json = serde_json::to_string(&message)
                .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

            if let Err(_e) = dc.send_text(msg_json).await {
                return Err(format!("Failed to send message: {}", _e));
            }

            Ok(())
        } else {
            let err_msg = format!(
                "Data channel for {} is not open (state: {:?})",
                target_device_id,
                ready_state
            );
            tracing::warn!("❌ {}", err_msg);
            Err(err_msg)
        }
    } else {
        let err_msg = format!("Data channel not found for device {}", target_device_id);
        // Add more detailed debugging
        let available_channels = {
            let guard = state_log.lock().await;
            guard.data_channels.keys().cloned().collect::<Vec<_>>()
        };
        tracing::warn!("❌ {} - Available channels: {:?}", err_msg, available_channels);
        Err(err_msg)
    }
}

pub async fn create_and_setup_device_connection<C>(
    device_id: String,
    self_device_id: String, // Pass self_device_id
    device_connections_arc: Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>,
    cmd_tx: mpsc::UnboundedSender<InternalCommand<C>>, // Use InternalCommand
    state_log: Arc<Mutex<AppState<C>>>,
    api: &'static webrtc::api::API,
    config: &'static RTCConfiguration,
) -> Result<Arc<RTCPeerConnection>, String> where C: Ciphersuite + Send + Sync + 'static, 
<<C as Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync, 
<<<C as Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,     
{
    // Clone variables for use after timeout
    let device_id_for_timeout = device_id.clone();
    // State log cloned for timeout handler (used in error case below)
    
    // Add timeout to prevent hanging during connection creation
    let connection_creation = async move {
        // Double-check pattern with immediate insertion to prevent race conditions
        let pc_arc = {
            let mut device_conns = device_connections_arc.lock().await;
            if let Some(existing_pc) = device_conns.get(&device_id) {
                return Ok(existing_pc.clone());
            }

            state_log
                .lock()
                .await
                .log
                .push(format!("Creating WebRTC connection object for {}", device_id));

            // Use passed-in api and config
            match api.new_peer_connection(config.clone()).await {
                Ok(pc) => {
                    let pc_arc = Arc::new(pc);
                    // Store immediately to prevent duplicate creation
                    device_conns.insert(device_id.clone(), pc_arc.clone());
                    state_log
                        .lock()
                        .await
                        .log
                        .push(format!("Stored WebRTC connection object for {}", device_id));
                    pc_arc
                }
                Err(_e) => {
                    let err_msg = format!(
                        "Error creating device connection object for {}: {}",
                        device_id, _e
                    );
                    return Err(err_msg);
                }
            }
        }; // Release the lock here

        // Now set up callbacks and data channel (outside the lock).
        // Only the higher-id side of the pair opens the data channel;
        // the lower-id side gets it via the RTCPeerConnection's
        // on_data_channel callback set up below.
        if self_device_id < device_id
            && let Ok(dc) = pc_arc.create_data_channel(DATA_CHANNEL_LABEL, None).await
        {
            tracing::debug!("Data channel state: {:?}", dc.ready_state());
            setup_data_channel_callbacks(
                dc,
                device_id.clone(),
                state_log.clone(),
                cmd_tx.clone(),
            ).await;
        }

        let device_id_on_ice = device_id.clone();
        let cmd_tx_on_ice = cmd_tx.clone(); // Clones the sender for internal ClientMsg
        let state_log_on_ice = state_log.clone();
        pc_arc.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
                    let device_id = device_id_on_ice.clone();
                    let cmd_tx = cmd_tx_on_ice.clone();
                    let state_log = state_log_on_ice.clone();
                    Box::pin(async move {
                        if let Some(c) = candidate {
                            // ... existing ICE candidate sending logic ...
                            match c.to_json() {
                                Ok(init) => {
                                    tracing::info!("🧊 ICE candidate generated for {}: {}", device_id, init.candidate);
                                    let signal = WebRTCSignal::Candidate(CandidateInfo {
                                        candidate: init.candidate,
                                        sdp_mid: init.sdp_mid,
                                        sdp_mline_index: init.sdp_mline_index,
                                    });
                                    let websocket_msg = WebSocketMessage::WebRTCSignal(signal);
                                    match serde_json::to_value(websocket_msg) {
                                        Ok(json_val) => {
                                            // Wrap the Relay message inside SendToServer command
                                            let relay_cmd =
                                                InternalCommand::SendToServer(SharedClientMsg::Relay {
                                                    to: device_id.clone(),
                                                    data: json_val,
                                                });
                                            tracing::info!("📮 Sending ICE candidate to {}", device_id);
                                            let _ = cmd_tx.send(relay_cmd); // Send the internal command
                                            state_log
                                                .lock()
                                                .await
                                                .log
                                                .push(format!("Sent ICE candidate to {}", device_id));
                                        }
                                        // FIX: Use error variable 'e'
                                        Err(_e) => {
                                        }
                                    }
                                }
                                // FIX: Use error variable 'e'
                                Err(_e) => {
                                }
                            }
                        }
                    })
                }));

        // Setup state change handler with DKG trigger logic
        let state_log_on_state = state_log.clone();
        let device_id_on_state = device_id.clone();
        let cmd_tx_on_state = cmd_tx.clone();
        // Clone for ICE handler
        let state_log_on_state_ice = state_log.clone();
        let device_id_on_state_ice = device_id.clone();
        // Fix the setup_device_connection_callbacks function
        // Clone before moving into closure
        let pc_arc_for_state = pc_arc.clone();
        pc_arc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            // Fix: Use pc_arc directly instead of undefined pc_arc_for_state
            let pc_arc = pc_arc_for_state.clone();
            let device_id = device_id_on_state.clone();
            let state_log = state_log_on_state.clone();
            let cmd_tx = cmd_tx_on_state.clone();

            // Log both connectionState and iceConnectionState together
            let ice_state = pc_arc.ice_connection_state();
            tracing::debug!(
                "Device {}: connectionState={:?}, iceConnectionState={:?}",
                device_id, s, ice_state
            );

            // Send WebRTC status update
            let webrtc_connected = matches!(s, RTCPeerConnectionState::Connected);
            let _ = cmd_tx.send(InternalCommand::UpdateParticipantWebRTCStatus {
                device_id: device_id.clone(),
                webrtc_connected,
                data_channel_open: false, // Will be updated when data channel opens
            });

            if let Ok(mut app_state_guard) = state_log.try_lock() {
                app_state_guard.device_statuses.insert(device_id.clone(), s);
            }

                // Handle state changes with improved logic
                match s {
                    RTCPeerConnectionState::Connected => {
                        if let Ok(mut guard) = state_log.try_lock() {
                            // Record successful connection time
                            guard.reconnection_tracker.insert(device_id.clone(), std::time::Instant::now());
                            
                            // Data channel status check
                            if guard.data_channels.contains_key(&device_id) {
                                // Data channel exists
                            } else {
                                // No data channel yet
                            }
                        }
                    }
                    RTCPeerConnectionState::Disconnected => {
                        // Handle disconnection with more aggressive reconnection
                        if let Ok(mut guard) = state_log.try_lock() {
                                                        
                            // Reset DKG state if a device disconnects during DKG
                            if guard.dkg_state != DkgState::Idle && guard.dkg_state != DkgState::Complete {
                                guard.dkg_state = DkgState::Failed(format!("Device {} disconnected", device_id));
                                // Clear intermediate DKG data if needed
                                guard.dkg_part1_public_package = None;
                                guard.dkg_part1_secret_package = None;
                                guard.received_dkg_packages.clear();
                            }
                            
                            // Always attempt immediate reconnection on Disconnected state
                            if let Some(current_session) = guard.session.clone() {
                                tracing::info!("Will attempt to rejoin session: {}", current_session.session_id);
                                // Drop the guard before sending the command
                                drop(guard);
                                // No JoinSession message sent
                            }
                        }
                    }
                    RTCPeerConnectionState::Failed => {
                        if let Ok(mut guard) = state_log.try_lock() {
                            
                            // Reset DKG state if a device disconnects during DKG
                            if guard.dkg_state != DkgState::Idle && guard.dkg_state != DkgState::Complete {
                                guard.dkg_state = DkgState::Failed(format!("Device {} connection failed", device_id));
                                guard.dkg_part1_public_package = None;
                                guard.dkg_part1_secret_package = None;
                                guard.received_dkg_packages.clear();
                            }
                            
                            // Check if we should attempt reconnection (simple time-based check)
                            let should_reconnect = match guard.reconnection_tracker.get(&device_id) {
                                Some(last_attempt) => last_attempt.elapsed() > std::time::Duration::from_secs(5),
                                None => true,
                            };
                            
                            if should_reconnect {
                                // Update last attempt time
                                guard.reconnection_tracker.insert(device_id, std::time::Instant::now());
                                
                                if let Some(current_session) = guard.session.clone() {
                                    tracing::info!("Will attempt to rejoin session: {}", current_session.session_id);
                                    // Drop the guard before any async operations
                                    drop(guard);
                                    // Reconnection logic would go here
                                }
                            }
                        }
                    }
                    RTCPeerConnectionState::Connecting | RTCPeerConnectionState::New => {
                        // We don't need special handling for these states,
                        // they're already logged above when updating device_statuses
                    }
                    RTCPeerConnectionState::Closed => {
                        if let Ok(_guard) = state_log.try_lock() {
                        }
                    }
                    // Handle the Unspecified state to fix the compilation error
                    RTCPeerConnectionState::Unspecified => {
                        if let Ok(_guard) = state_log.try_lock() {
                            // No specific action needed for unspecified state
                        }
                    }
                }
                Box::pin(async {})
            }));

            // --- Setup ICE connection monitoring callback ---
            let state_log_ice = state_log_on_state_ice.clone();
            let device_id_ice = device_id_on_state_ice.clone();
            let pc_arc_for_ice = pc_arc.clone();
            pc_arc.on_ice_connection_state_change(Box::new(move |ice_state| {
                let state_log = state_log_ice.clone();
                let device_id = device_id_ice.clone();
                let pc_arc = pc_arc_for_ice.clone();

                // Log both connectionState and iceConnectionState together
                let conn_state = pc_arc.connection_state();
                tracing::debug!(
                    "Device {}: connectionState={:?}, iceConnectionState={:?}",
                    device_id, conn_state, ice_state
                );
                if let Ok(_guard) = state_log.try_lock() {
                }
                // No async work, just return a ready future
                Box::pin(async {})
            }));

            // --- Only set up callbacks for the main data channel (responder side) ---
            let state_log_on_data = state_log_on_state_ice;
            let device_id_on_data = device_id_on_state_ice;
            let cmd_tx_on_data = cmd_tx.clone();
            pc_arc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
                let state_log = state_log_on_data.clone();
                let device_id = device_id_on_data.clone();
                let cmd_tx_clone = cmd_tx_on_data.clone();

                Box::pin(async move {
                    if dc.label() == DATA_CHANNEL_LABEL {
                        tracing::debug!("Received data channel state: {:?}", dc.ready_state());
                        setup_data_channel_callbacks(dc, device_id, state_log, cmd_tx_clone).await;
                    }
                })
            }));

        // Connection already stored in the double-check pattern above
        Ok(pc_arc)
    }; // Close the async block

    // Apply timeout to prevent hanging
    match tokio::time::timeout(std::time::Duration::from_secs(30), connection_creation).await {
        Ok(result) => {
            result
        }
        Err(_) => {
            let timeout_msg = format!(
                "⏰ TIMEOUT: WebRTC connection creation for {} took longer than 30 seconds, aborting",
                device_id_for_timeout
            );
            Err(timeout_msg)
        }
    }
}

pub async fn setup_data_channel_callbacks<C>(
    dc: Arc<RTCDataChannel>,
    device_id: String,
    state: Arc<Mutex<AppState<C>>>,
    // Update the sender type here
    cmd_tx: mpsc::UnboundedSender<InternalCommand<C>>, // Use InternalCommand
) where C: Ciphersuite + Send + Sync + 'static, 
<<C as Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync, 
<<<C as Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,     
 {
    let dc_arc = dc.clone(); // Clone the Arc for the data channel

    // Log entry to setup_data_channel_callbacks

    // Only store and set up callbacks for the main data channel
    if dc_arc.label() == DATA_CHANNEL_LABEL {
        let mut guard = state.lock().await;
        let channel_count = guard.data_channels.len() + 1; // Calculate before inserting
        guard.data_channels.insert(device_id.clone(), dc_arc.clone());
        guard
            .log
            .push(format!("💾 Data channel for {} stored in app state (total channels: {})", 
                device_id, channel_count));
    }

    let _state_log_open = state.clone();  // Reserved for future logging
    let device_id_open = device_id.clone();
    let _dc_clone = dc_arc.clone();  // Reserved for future use
    let cmd_tx_open = cmd_tx.clone();
    dc_arc.on_open(Box::new(move || {
        // Clone for async closure
        let device_id_open = device_id_open.clone();
        let cmd_tx_open = cmd_tx_open;
        Box::pin(async move {
            
            // Send ReportChannelOpen command to trigger mesh ready signaling

            // Also send status update that data channel is open
            let _ = cmd_tx_open.send(InternalCommand::UpdateParticipantWebRTCStatus {
                device_id: device_id_open.clone(),
                webrtc_connected: true,
                data_channel_open: true,
            });

            let _ = cmd_tx_open.send(InternalCommand::ReportChannelOpen {
                device_id: device_id_open.clone(),
            });
        })
    }));

    let state_log_msg = state.clone();
    let device_id_msg = device_id.clone();
    let cmd_tx_msg = cmd_tx.clone(); // Clone internal cmd_tx for on_message
    let dc_arc_msg = dc_arc.clone(); // Clone for use inside async block
    dc_arc.on_message(Box::new(move |msg: DataChannelMessage| {
        let device_id = device_id_msg.clone();
        let state_log = state_log_msg.clone();
        let cmd_tx = cmd_tx_msg.clone();
        let dc_arc = dc_arc_msg.clone(); // Use a clone inside the async block

        Box::pin(async move {
            // Only process messages if this is the main frost-dkg channel
            if dc_arc.label() != DATA_CHANNEL_LABEL {
                return;
            }

            if let Ok(text) = String::from_utf8(msg.data.to_vec()) {
                // DEBUG: Log the raw message content to see exactly what we're receiving
                
                // Parse envelope
                match serde_json::from_str::<WebRTCMessage<C>>(&text) {
                    Ok(envelope) => {
                        match envelope {
                            WebRTCMessage::DkgRound1Package { package } => {
                                    let _ = cmd_tx.send(InternalCommand::ProcessDkgRound1 {
                                        from_device_id: device_id.clone(),
                                        package,
                                    });
                            }
                            WebRTCMessage::DkgRound2Package { package } => {
                                // FIX: Add type annotation for from_value
                                    let _ = cmd_tx.send(InternalCommand::ProcessDkgRound2 {
                                        from_device_id: device_id.clone(),
                                        package,
                                    });
                            }
                            WebRTCMessage::ReshareRound1Package { .. }
                            | WebRTCMessage::ReshareRound2Package { .. } => {
                                // The reshare ceremony (#45) runs on the elm headless
                                // path (network/webrtc.rs → Message::ProcessReshareRound*).
                                // This legacy ratatui InternalCommand path doesn't drive
                                // reshare yet (phase 5), so ignore here.
                                tracing::debug!(
                                    "reshare round package from {} on legacy device.rs path — ignored \
                                     (reshare runs via the elm path)",
                                    device_id
                                );
                            }
                            WebRTCMessage::SimpleMessage { text } => {
                                // Parse DKG messages from SimpleMessage format
                                if text.starts_with("DKG_ROUND1:") {
                                    if let Some(base64_data) = text.strip_prefix("DKG_ROUND1:") {
                                        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
                                        match BASE64.decode(base64_data) {
                                            Ok(package_bytes) => {
                                                tracing::info!("Received DKG Round 1 package from {}", device_id);
                                                let _ = cmd_tx.send(InternalCommand::ProcessSimpleDkgRound1 {
                                                    from_device_id: device_id.clone(),
                                                    package_bytes,
                                                });
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to decode DKG Round 1 base64 data: {}", e);
                                            }
                                        }
                                    }
                                } else if text.starts_with("DKG_ROUND2:") {
                                    if let Some(base64_data) = text.strip_prefix("DKG_ROUND2:") {
                                        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
                                        match BASE64.decode(base64_data) {
                                            Ok(package_bytes) => {
                                                tracing::info!("Received DKG Round 2 package from {}", device_id);
                                                let _ = cmd_tx.send(InternalCommand::ProcessSimpleDkgRound2 {
                                                    from_device_id: device_id.clone(),
                                                    package_bytes,
                                                });
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to decode DKG Round 2 base64 data: {}", e);
                                            }
                                        }
                                    }
                                } else {
                                    tracing::debug!("Received unhandled SimpleMessage: {}", text);
                                }
                            },
                            WebRTCMessage::ChannelOpen { device_id: _ } => {
                                // Just log the channel open notification, don't trigger ReportChannelOpen
                                // to avoid infinite feedback loops
                            },
                            WebRTCMessage::MeshReady { session_id: _, device_id } => {
                                let _ = cmd_tx.send(InternalCommand::ProcessMeshReady {
                                    device_id,
                                });
                            },
                            // Signing message handlers
                            WebRTCMessage::SigningRequest { signing_id, transaction_data, required_signers: _, blockchain, chain_id } => {
                                let _ = cmd_tx.send(InternalCommand::ProcessSigningRequest {
                                    from_device_id: device_id.clone(),
                                    signing_id,
                                    transaction_data,
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    blockchain,
                                    chain_id,
                                });
                            },
                            WebRTCMessage::SigningAcceptance { signing_id, accepted: _ } => {
                                let _ = cmd_tx.send(InternalCommand::ProcessSigningAcceptance {
                                    from_device_id: device_id.clone(),
                                    signing_id,
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                });
                            },
                            WebRTCMessage::SignerSelection { signing_id, selected_signers } => {
                                let _ = cmd_tx.send(InternalCommand::ProcessSignerSelection {
                                    from_device_id: device_id.clone(),
                                    signing_id,
                                    selected_signers,
                                });
                            },
                            WebRTCMessage::SigningCommitment { signing_id, sender_identifier: _, commitment } => {
                                let _ = cmd_tx.send(InternalCommand::ProcessSigningCommitment {
                                    from_device_id: device_id.clone(),
                                    signing_id,
                                    commitment,
                                });
                            },
                            WebRTCMessage::SignatureShare { signing_id, sender_identifier: _, share } => {
                                let _ = cmd_tx.send(InternalCommand::ProcessSignatureShare {
                                    from_device_id: device_id.clone(),
                                    signing_id,
                                    share,
                                });
                            },
                            WebRTCMessage::AggregatedSignature { signing_id, signature } => {
                                let _ = cmd_tx.send(InternalCommand::ProcessAggregatedSignature {
                                    from_device_id: device_id.clone(),
                                    signing_id,
                                    signature,
                                });
                            }
                        }
                    }
                    Err(_e) => {
                        state_log
                            .lock()
                            .await
                            .log
                            .push(format!("Failed to parse envelope from {}: {}", device_id, _e));
                    }
                }
            } else {
                state_log
                    .lock()
                    .await
                    .log
                    .push(format!("Received non-UTF8 data from {}", device_id));
            }
        })
    }));

    dc.on_close(Box::new(move || {
        Box::pin(async move {
            // Closure handler for data channel close event
        })
    }));

    dc.on_error(Box::new(move |e| {
        Box::pin(async move {
            tracing::error!("Data channel error: {:?}", e);
        })
    }));
}

// Apply any pending ICE candidates for a device
pub async fn apply_pending_candidates<C>(
    device_id: &str,
    pc: Arc<RTCPeerConnection>,
    state_log: Arc<Mutex<AppState<C>>>,
) where C: Ciphersuite {
    // Take the pending candidates for this device
    let candidates = {
        let mut _state_guard = state_log.lock().await;
        let pending = _state_guard.pending_ice_candidates.remove(device_id);
        if let Some(candidates) = &pending
            && !candidates.is_empty() {
            }
        pending
    };

    // If there are pending candidates, apply them
    if let Some(candidates) = candidates {
        // Apply each candidate
        for candidate in candidates {
            match pc.add_ice_candidate(candidate.clone()).await {
                Ok(_) => {
                    let mut _state_guard = state_log.lock().await;
                    _state_guard
                        .log
                        .push(format!("Applied stored ICE candidate for {}", device_id));
                    // apply candidate to the device connection
                    
                }
                Err(_e) => {
                    let mut _state_guard = state_log.lock().await;
                }
            }
        }
    }
}

pub async fn check_and_send_mesh_ready<C>( //all data channels are open and send mesh_ready if needed
   state: Arc<Mutex<AppState<C>>>,
    cmd_tx: mpsc::UnboundedSender<InternalCommand<C>>,
) where C: Ciphersuite + Send + Sync + 'static, 
<<C as Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync, 
<<<C as Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,     
{
    let mut all_channels_open = false;
    let mut all_channels_ready = false;
    let mut already_sent_own_ready = false;
    let mut session_exists = false;

    {
        let state_guard = state.lock().await;
        if let Some(session) = &state_guard.session {
            session_exists = true;
            
            // Simple check: Do we have all required participants?
            if session.participants.len() < session.total as usize {
                return; // Wait for more participants
            }
            
            let device_id = state_guard.device_id.clone();
            let participants_to_check: Vec<String> = session
                .participants
                .iter()
                .filter(|p| **p != device_id)
                .cloned()
                .collect();
            

            all_channels_open = participants_to_check
                .iter()
                .all(|p| state_guard.data_channels.contains_key(p));

            // Check if all data channels are open
            all_channels_ready = participants_to_check
                .iter()
                .all(|participant_id| {
                    state_guard.data_channels.get(participant_id)
                        .map(|dc| dc.ready_state() == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open)
                        .unwrap_or(false)
                });
            
            already_sent_own_ready = state_guard.own_mesh_ready_sent;
        }
    } // state_guard is dropped

    if session_exists && all_channels_open && all_channels_ready && !already_sent_own_ready {
        let _ = cmd_tx.send(InternalCommand::SendOwnMeshReadySignal);
    }
}