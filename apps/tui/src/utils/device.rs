use crate::protocal::signal::WebRTCMessage;
use crate::utils::appstate_compat::AppState;

use std::sync::Arc;

use tokio::sync::Mutex;

use webrtc::data_channel::RTCDataChannelState;

use frost_core::Ciphersuite;

// NOTE: this module used to also carry a pre-Elm ratatui WebRTC wiring path
// (`create_and_setup_device_connection`, `setup_data_channel_callbacks`,
// `apply_pending_candidates`, `check_and_send_mesh_ready`, and the
// `DATA_CHANNEL_LABEL` constant they shared) built on webrtc 0.17's
// callback-registration API (`pc.on_ice_candidate`, `dc.on_open`, ...). It had
// zero callers — the live mesh path is `elm::webrtc_signaling` +
// `network::webrtc` — so during the webrtc 0.21 port (which replaced that
// callback API with a constructor-installed `PeerConnectionEventHandler` and
// `DataChannel::poll()`) it was deleted outright rather than translated.
// `send_webrtc_message` below is the one function from this module the live
// path still calls (dkg.rs, signing.rs, reshare.rs, command.rs).

pub async fn send_webrtc_message<C>(
    target_device_id: &str,
    message: &WebRTCMessage<C>,
    state_log: Arc<Mutex<AppState<C>>>,
) -> Result<(), String>
where
    C: Ciphersuite,
{
    let msg_json = serde_json::to_string(&message)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    // Enhanced debugging to trace data channel access
    let data_channel = {
        let mut guard = state_log.lock().await;
        // Remember ceremony frames whether or not this send gets through: a
        // frame that dies with a broken connection is resent when the peer's
        // data channel reopens (`network::peer_recovery`).
        if let WebRTCMessage::SimpleMessage { text } = message
            && crate::network::peer_recovery::is_ceremony_frame(text)
        {
            guard.ceremony_outbox.record(target_device_id, &msg_json);
        }
        tracing::debug!(
            "🔍 Looking for data channel for device: {}",
            target_device_id
        );
        tracing::debug!(
            "🔍 Available data channels: {:?}",
            guard.data_channels.keys().collect::<Vec<_>>()
        );
        guard.data_channels.get(target_device_id).cloned()
    };

    if let Some(dc) = data_channel {
        let ready_state = dc
            .ready_state()
            .await
            .unwrap_or(RTCDataChannelState::Closed);
        tracing::debug!(
            "🔍 Data channel for {} found, state: {:?}",
            target_device_id,
            ready_state
        );

        if ready_state == RTCDataChannelState::Open {
            if let Err(_e) = dc.send_text(&msg_json).await {
                return Err(format!("Failed to send message: {}", _e));
            }

            Ok(())
        } else {
            let err_msg = format!(
                "Data channel for {} is not open (state: {:?})",
                target_device_id, ready_state
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
        tracing::warn!(
            "❌ {} - Available channels: {:?}",
            err_msg,
            available_channels
        );
        Err(err_msg)
    }
}
