use crate::protocal::signal::{SDPInfo, WebRTCSignal, WebSocketMessage};
use crate::utils::appstate_compat::AppState;
use frost_core::Ciphersuite;

use crate::utils::state::InternalCommand;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, mpsc};

use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use starlab_signal_server::ClientMsg as SharedClientMsg;

pub async fn initiate_offers_for_session<C>(
    participants: Vec<String>,
    self_device_id: String,
    device_connections: Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>,
    cmd_tx: mpsc::UnboundedSender<InternalCommand<C>>,
    state: Arc<Mutex<AppState<C>>>,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar:
        Send + Sync,
{
    state
        .lock()
        .await
        .log
        .push(format!("🎯 initiate_offers_for_session: Starting offer creation for {} devices", participants.len()));

    // Lock connections once
    let device_conns = device_connections.lock().await;

    for device_id in participants {
        if device_id == self_device_id {
            continue;
        }

        let should_initiate = self_device_id < device_id;


        if should_initiate
            && let Some(pc_arc) = device_conns.get(&device_id)
        {
            state
                .lock()
                .await
                .log
                .push(format!("Found PC object for device {}", device_id));
            let current_state = pc_arc.connection_state();
            let signaling_state = pc_arc.signaling_state();

                // We need to create an offer if:
                // 1. Connection is new/closed/failed/disconnected
                // 2. Connection is connecting but we haven't sent an offer yet (signaling state is stable)
                // 3. Connection is connected but signaling state indicates renegotiation is needed
                let negotiation_needed = match current_state {
                    RTCPeerConnectionState::New
                    | RTCPeerConnectionState::Closed
                    | RTCPeerConnectionState::Disconnected
                    | RTCPeerConnectionState::Failed => true,
                    RTCPeerConnectionState::Connecting => {
                        // If we're connecting but signaling is stable, we haven't sent an offer yet
                        matches!(signaling_state, 
                            webrtc::peer_connection::signaling_state::RTCSignalingState::Stable)
                    },
                    RTCPeerConnectionState::Connected => {
                        // Check if we need renegotiation (e.g., after ICE restart)
                        false // For now, don't renegotiate connected peers
                    },
                    _ => false,
                };


                if !negotiation_needed {
                    continue;
                }

                let is_already_making_offer = state
                    .lock()
                    .await
                    .making_offer
                    .get(&device_id)
                    .copied()
                    .unwrap_or(false);


                if is_already_making_offer {
                    continue;
                }

                let pc_arc_clone = pc_arc.clone();
                let device_id_clone = device_id.clone();
                let state_clone = state.clone();
                let cmd_tx_clone = cmd_tx.clone();

                tokio::spawn(async move {
                    state_clone
                        .lock()
                        .await
                        .making_offer
                        .insert(device_id_clone.clone(), true);
                    state_clone
                        .lock()
                        .await
                        .log
                        .push(format!("Set making_offer=true for {}", device_id_clone));

                    let offer_result = async {

                        match pc_arc_clone.create_offer(None).await {
                            Ok(offer) => {

                                if let Err(_e) = pc_arc_clone.set_local_description(offer.clone()).await {
                                    return Err(());
                                }

                                let signal = WebRTCSignal::Offer(SDPInfo { sdp: offer.sdp });
                                let websocket_message = WebSocketMessage::WebRTCSignal(signal);

                                match serde_json::to_value(websocket_message) {
                                    Ok(json_val) => {
                                        let relay_cmd = InternalCommand::SendToServer(SharedClientMsg::Relay {
                                            to: device_id_clone.clone(),
                                            data: json_val,
                                        });
                                        let _ = cmd_tx_clone.send(relay_cmd);
                                        state_clone
                                            .lock()
                                            .await
                                            .log
                                            .push(format!("✅ OFFER SENT to {}! Waiting for answer...", device_id_clone));
                                    }
                                    Err(_e) => {
                                        return Err(());
                                    }
                                }
                            }
                            Err(_e) => {
                                state_clone
                                    .lock()
                                    .await
                                    .log
                                    .push(format!("Offer Task [{}]: Error creating offer: {}", device_id_clone, _e));
                                return Err(());
                            }
                        }
                        Ok(())
                    }.await;

                    let outcome = if offer_result.is_ok() {
                        "succeeded"
                    } else {
                        "failed"
                    };
                    tracing::debug!("Offer creation {}", outcome);
                    state_clone
                        .lock()
                        .await
                        .making_offer
                        .insert(device_id_clone.clone(), false);
                    
                    state_clone
                        .lock()
                        .await
                        .log
                        .push(format!("Set making_offer=false for {}", device_id_clone));
                });
        }
    }

    state
        .lock()
        .await
        .log
        .push("Finished WebRTC offers check.".to_string());
}
