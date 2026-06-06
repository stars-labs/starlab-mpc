use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};

pub mod session_manager;
pub mod cloudflare_storage;

type DeviceSender = mpsc::UnboundedSender<Message>;
type DeviceMap = Arc<Mutex<HashMap<String, DeviceSender>>>;

// KISS: Store minimal session info - just the announcement
#[derive(Clone)]
struct StoredSession {
    session_info: serde_json::Value,  // The full announcement as-is
    active_participants: Vec<String>, // Currently online participants
    last_active: std::time::Instant,  // Updated when participants leave; used for grace period
}

type SessionMap = Arc<Mutex<HashMap<String, StoredSession>>>;
// Map device_id to list of session_ids they're participating in
type DeviceSessionsMap = Arc<Mutex<HashMap<String, Vec<String>>>>;

/// Run the signal-server accept loop on an already-bound listener until the
/// future is dropped/aborted. Extracted from `main` so it can be embedded
/// in-process (e.g. the CLI's `simulate` mode + end-to-end tests) on an
/// ephemeral port, not just run as a standalone binary.
pub async fn run(listener: TcpListener) {
    let devices: DeviceMap = Arc::new(Mutex::new(HashMap::new()));
    let sessions: SessionMap = Arc::new(Mutex::new(HashMap::new()));
    let device_sessions: DeviceSessionsMap = Arc::new(Mutex::new(HashMap::new()));

    // Periodic cleanup: expire sessions with no active participants for >5 min.
    let sessions_cleanup = sessions.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut guard = sessions_cleanup.lock().unwrap();
            guard.retain(|id, session| {
                let age = session.last_active.elapsed();
                let keep = !session.active_participants.is_empty()
                    || age < std::time::Duration::from_secs(300);
                if !keep {
                    eprintln!("🗑️ Expiring session '{}' (no active participants for {:?})", id, age);
                }
                keep
            });
        }
    });

    while let Ok((stream, _)) = listener.accept().await {
        let devices = devices.clone();
        let sessions = sessions.clone();
        let device_sessions = device_sessions.clone();

        tokio::spawn(async move {
            // Handle WebSocket handshake errors gracefully
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("WebSocket handshake failed (this is normal for connection tests): {:?}", e);
                    return;
                }
            };
            let (mut ws_sink, mut ws_stream) = ws_stream.split();
            let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
            let mut device_id: Option<String> = None;

            // Task to forward messages from rx to ws_sink
            let ws_sink_task = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if ws_sink.send(msg).await.is_err() {
                        break;
                    }
                }
            });

            loop {
                tokio::select! {
                    Some(msg) = ws_stream.next() => {
                        let msg = match msg {
                            Ok(m) if m.is_ping() => {
                                let _ = tx.send(Message::Pong(m.into_data()));
                                continue;
                            }
                            Ok(m) if m.is_text() => m.into_text().unwrap(),
                            Ok(m) if m.is_close() => break,
                            _ => continue,
                        };

                        let parsed: Result<ClientMsg, _> = serde_json::from_str(&msg);

                        match parsed {
                            Ok(ClientMsg::Register { device_id: reg_id }) => {
                                let mut devices_guard = devices.lock().unwrap();
                                if devices_guard.contains_key(&reg_id) {
                                    let err = ServerMsg::Error { error: "device_id already registered".to_string() };
                                    let _ = tx.send(Message::Text(serde_json::to_string(&err).unwrap().into()));
                                    break;
                                }
                                device_id = Some(reg_id.clone());
                                devices_guard.insert(reg_id.clone(), tx.clone());
                                eprintln!("Registered device: {}", reg_id);

                                let device_list: Vec<String> = devices_guard.keys().cloned().collect();
                                let msg = ServerMsg::Devices { devices: device_list.clone() };
                                let msg_txt = serde_json::to_string(&msg).unwrap();
                                for (_id, ptx) in devices_guard.iter() {
                                    let _ = ptx.send(Message::Text(msg_txt.clone().into()));
                                }
                            }
                            Ok(ClientMsg::ListDevices) => {
                                let devices_guard = devices.lock().unwrap();
                                let device_list: Vec<String> = devices_guard.keys().cloned().collect();
                                let msg = ServerMsg::Devices { devices: device_list };
                                let _ = tx.send(Message::Text(serde_json::to_string(&msg).unwrap().into()));
                            }
                            Ok(ClientMsg::Relay { to, data }) => {
                                if data.get("websocket_msg_type").and_then(|v| v.as_str()) == Some("SessionProposal")
                                    && let (Some(session_id), Some(participants)) = (
                                        data.get("session_id").and_then(|v| v.as_str()),
                                        data.get("participants").and_then(|v| v.as_array())
                                    ) {
                                        let mut sessions_guard = sessions.lock().unwrap();
                                        if let Some(session) = sessions_guard.get_mut(session_id) {
                                            session.session_info = data.clone();
                                            session.active_participants.clear();
                                            let devices_guard = devices.lock().unwrap();
                                            for p in participants {
                                                if let Some(participant_id) = p.as_str() {
                                                    if devices_guard.contains_key(participant_id) {
                                                        session.active_participants.push(participant_id.to_string());
                                                    }
                                                }
                                            }
                                            drop(devices_guard);
                                            eprintln!("Updated session '{}' with participants: {:?} (active: {:?})",
                                                session_id, participants, session.active_participants);
                                        }
                                        drop(sessions_guard);

                                        let mut device_sessions_guard = device_sessions.lock().unwrap();
                                        for p in participants {
                                            if let Some(participant_id) = p.as_str() {
                                                let entry = device_sessions_guard
                                                    .entry(participant_id.to_string())
                                                    .or_default();
                                                if !entry.contains(&session_id.to_string()) {
                                                    entry.push(session_id.to_string());
                                                    eprintln!("Added session '{}' to device '{}' session list", session_id, participant_id);
                                                }
                                            }
                                        }
                                        drop(device_sessions_guard);
                                    }

                                if data.get("websocket_msg_type").and_then(|v| v.as_str()) == Some("SessionUpdate")
                                    && let (Some(session_id), Some(accepted_devices)) = (
                                        data.get("session_id").and_then(|v| v.as_str()),
                                        data.get("accepted_devices").and_then(|v| v.as_array())
                                    ) {
                                        let mut sessions_guard = sessions.lock().unwrap();
                                        if let Some(session) = sessions_guard.get_mut(session_id) {
                                            session.active_participants.clear();
                                            let devices_guard = devices.lock().unwrap();
                                            for p in accepted_devices {
                                                if let Some(participant_id) = p.as_str() {
                                                    if devices_guard.contains_key(participant_id) {
                                                        session.active_participants.push(participant_id.to_string());
                                                    }
                                                }
                                            }
                                            drop(devices_guard);
                                            eprintln!("Updated active participants for session '{}': {:?}",
                                                session_id, session.active_participants);

                                            if session.session_info.get("participants").and_then(|v| v.as_array()).is_some() {
                                                let mut updated_info = session.session_info.clone();
                                                updated_info.as_object_mut().unwrap().insert("accepted_devices".to_string(), serde_json::Value::Array(accepted_devices.clone()));
                                                session.session_info = updated_info;
                                            }
                                        }
                                        drop(sessions_guard);

                                        let mut device_sessions_guard = device_sessions.lock().unwrap();
                                        for p in accepted_devices {
                                            if let Some(participant_id) = p.as_str() {
                                                let entry = device_sessions_guard
                                                    .entry(participant_id.to_string())
                                                    .or_default();
                                                if !entry.contains(&session_id.to_string()) {
                                                    entry.push(session_id.to_string());
                                                }
                                            }
                                        }
                                        drop(device_sessions_guard);
                                    }

                                let devices_guard = devices.lock().unwrap();
                                if to == "*" {
                                    let relay = ServerMsg::Relay {
                                        from: device_id.as_deref().unwrap_or_default().to_string(),
                                        data: data.clone(),
                                    };
                                    let relay_text = serde_json::to_string(&relay).unwrap();
                                    eprintln!("Broadcasting relay from {} to all devices: {:?}",
                                        device_id.as_deref().unwrap_or("unknown"), data);
                                    for (id, device_tx) in devices_guard.iter() {
                                        if Some(id) != device_id.as_ref() {
                                            let _ = device_tx.send(Message::Text(relay_text.clone().into()));
                                        }
                                    }
                                } else {
                                    if let Some(device_tx) = devices_guard.get(&to) {
                                        let relay = ServerMsg::Relay {
                                            from: device_id.as_deref().unwrap_or_default().to_string(),
                                            data: data.clone(),
                                        };
                                        eprintln!("Relaying message from {} to {}: {:?}", device_id.as_deref().unwrap_or("unknown"), to, data);
                                        let _ = device_tx.send(Message::Text(serde_json::to_string(&relay).unwrap().into()));
                                    } else {
                                        eprintln!("Relay failed: unknown device {}", to);
                                        let err = ServerMsg::Error { error: format!("unknown device: {}", to) };
                                        let _ = tx.send(Message::Text(serde_json::to_string(&err).unwrap().into()));
                                    }
                                }
                                drop(devices_guard);
                            }
                            Ok(ClientMsg::AnnounceSession { session_info }) => {
                                if let Some(ref device) = device_id {
                                    let session_key = if let Some(id) = session_info.get("session_id")
                                        .and_then(|v| v.as_str()) {
                                        id.to_string()
                                    } else if let Some(code) = session_info.get("session_code")
                                        .and_then(|v| v.as_str()) {
                                        code.to_string()
                                    } else {
                                        format!("{}-{}", device, SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis())
                                    };

                                    let stored_session = StoredSession {
                                        session_info: session_info.clone(),
                                        active_participants: vec![device.clone()],
                                        last_active: std::time::Instant::now(),
                                    };

                                    let mut sessions_guard = sessions.lock().unwrap();
                                    sessions_guard.insert(session_key.clone(), stored_session);
                                    drop(sessions_guard);

                                    let mut device_sessions_guard = device_sessions.lock().unwrap();
                                    device_sessions_guard
                                        .entry(device.clone())
                                        .or_default()
                                        .push(session_key.clone());
                                    drop(device_sessions_guard);

                                    eprintln!("Stored session '{}' from device '{}'", session_key, device);
                                }

                                let devices_guard = devices.lock().unwrap();
                                let msg = ServerMsg::SessionAvailable { session_info };
                                let msg_txt = serde_json::to_string(&msg).unwrap();
                                eprintln!("Broadcasting session announcement from {}", device_id.as_deref().unwrap_or("unknown"));
                                for (id, device_tx) in devices_guard.iter() {
                                    if Some(id) != device_id.as_ref() {
                                        let _ = device_tx.send(Message::Text(msg_txt.clone().into()));
                                    }
                                }
                                drop(devices_guard);
                            }
                            Ok(ClientMsg::RequestActiveSessions) => {
                                eprintln!("Session list request from {}", device_id.as_deref().unwrap_or("unknown"));
                                let sessions_guard = sessions.lock().unwrap();
                                eprintln!("Found {} active sessions", sessions_guard.len());
                                for (session_key, stored_session) in sessions_guard.iter() {
                                    let msg = ServerMsg::SessionAvailable {
                                        session_info: stored_session.session_info.clone()
                                    };
                                    let msg_txt = serde_json::to_string(&msg).unwrap();
                                    eprintln!("Sending stored session '{}' to requester", session_key);
                                    let _ = tx.send(Message::Text(msg_txt.into()));
                                }
                                drop(sessions_guard);

                                let devices_guard = devices.lock().unwrap();
                                let msg = ServerMsg::SessionListRequest {
                                    from: device_id.as_deref().unwrap_or_default().to_string(),
                                };
                                let msg_txt = serde_json::to_string(&msg).unwrap();
                                for (id, device_tx) in devices_guard.iter() {
                                    if Some(id) != device_id.as_ref() {
                                        let _ = device_tx.send(Message::Text(msg_txt.clone().into()));
                                    }
                                }
                                drop(devices_guard);
                            }
                            Ok(ClientMsg::SessionStatusUpdate { session_info }) => {
                                eprintln!("Session status update from {}: {:?}", device_id.as_deref().unwrap_or("unknown"), session_info);
                                if let Some(participant_joined) = session_info.get("participant_joined")
                                    .and_then(|v| v.as_str())
                                    && let Some(session_id) = session_info.get("session_id")
                                        .and_then(|v| v.as_str()) {
                                        let mut sessions_guard = sessions.lock().unwrap();
                                        if let Some(stored_session) = sessions_guard.get_mut(session_id) {
                                            if let Some(participants) = stored_session.session_info
                                                .get_mut("participants")
                                                .and_then(|v| v.as_array_mut()) {
                                                let already_joined = participants.iter()
                                                    .any(|p| p.as_str() == Some(participant_joined));
                                                if !already_joined {
                                                    participants.push(serde_json::Value::String(participant_joined.to_string()));
                                                    eprintln!("Added {} to session {} participants", participant_joined, session_id);
                                                    if !stored_session.active_participants.contains(&participant_joined.to_string()) {
                                                        stored_session.active_participants.push(participant_joined.to_string());
                                                    }
                                                }
                                            }
                                            let updated_session_info = stored_session.session_info.clone();
                                            let participant_count = updated_session_info.get("participants")
                                                .and_then(|v| v.as_array())
                                                .map(|arr| arr.len())
                                                .unwrap_or(0);
                                            drop(sessions_guard);

                                            let update_msg = serde_json::json!({
                                                "type": "participant_update",
                                                "session_id": session_id,
                                                "session_info": updated_session_info.clone(),
                                            });

                                            let devices_guard = devices.lock().unwrap();
                                            eprintln!("Broadcasting participant update for session {} with {} participants",
                                                session_id, participant_count);
                                            for (id, device_tx) in devices_guard.iter() {
                                                let relay = ServerMsg::Relay {
                                                    from: "server".to_string(),
                                                    data: update_msg.clone(),
                                                };
                                                let msg_txt = serde_json::to_string(&relay).unwrap();
                                                let _ = device_tx.send(Message::Text(msg_txt.into()));
                                                eprintln!("Sent participant update to device: {}", id);
                                            }
                                            drop(devices_guard);
                                        } else {
                                            eprintln!("Session {} not found for participant update", session_id);
                                        }
                                    }
                            }
                            Ok(ClientMsg::QueryMyActiveSessions) => {
                                if let Some(ref dev_id) = device_id {
                                    eprintln!("Device '{}' querying for active sessions", dev_id);
                                    let mut sessions_guard = sessions.lock().unwrap();
                                    let mut my_sessions = Vec::new();
                                    let mut session_keys_to_track = Vec::new();
                                    for (key, session) in sessions_guard.iter_mut() {
                                        if let Some(participants) = session.session_info.get("participants")
                                            .and_then(|v| v.as_array()) {
                                            let is_participant = participants.iter()
                                                .any(|p| p.as_str() == Some(dev_id.as_str()));
                                            if is_participant {
                                                if !session.active_participants.contains(dev_id) {
                                                    session.active_participants.push(dev_id.clone());
                                                    eprintln!("Added '{}' back to active participants for session '{}'", dev_id, key);
                                                }
                                                my_sessions.push(session.session_info.clone());
                                                session_keys_to_track.push(key.clone());
                                            }
                                        }
                                    }
                                    drop(sessions_guard);

                                    let mut device_sessions_guard = device_sessions.lock().unwrap();
                                    device_sessions_guard.insert(dev_id.clone(), session_keys_to_track);
                                    drop(device_sessions_guard);

                                    let response = ServerMsg::SessionsForDevice {
                                        sessions: my_sessions.clone(),
                                    };
                                    let msg_txt = serde_json::to_string(&response).unwrap();
                                    eprintln!("Found {} sessions for device '{}'", my_sessions.len(), dev_id);
                                    let _ = tx.send(Message::Text(msg_txt.into()));
                                }
                            }
                            Err(_) => {
                                let err = ServerMsg::Error { error: "invalid message".to_string() };
                                let _ = tx.send(Message::Text(serde_json::to_string(&err).unwrap().into()));
                            }
                        }
                    }
                    else => break,
                }
            }

            // Cleanup on disconnect
            if let Some(my_id) = device_id {
                let device_sessions_guard = device_sessions.lock().unwrap();
                if let Some(session_ids) = device_sessions_guard.get(&my_id) {
                    let mut sessions_guard = sessions.lock().unwrap();
                    for session_id in session_ids {
                        if let Some(session) = sessions_guard.get_mut(session_id) {
                            session.active_participants.retain(|p| p != &my_id);
                            eprintln!("Removed '{}' from active participants in session '{}'", my_id, session_id);
                            if session.active_participants.is_empty() {
                                session.last_active = std::time::Instant::now();
                                eprintln!("Session '{}' has no active participants, keeping for grace period", session_id);
                            } else {
                                eprintln!("Session '{}' continues with {} active participants",
                                    session_id, session.active_participants.len());
                            }
                        }
                    }
                    drop(sessions_guard);
                }
                drop(device_sessions_guard);

                let mut device_sessions_guard = device_sessions.lock().unwrap();
                device_sessions_guard.remove(&my_id);
                drop(device_sessions_guard);

                let mut devices_guard = devices.lock().unwrap();
                devices_guard.remove(&my_id);
                eprintln!("Device {} disconnected", my_id);

                let device_list: Vec<String> = devices_guard.keys().cloned().collect();
                let msg = ServerMsg::Devices {
                    devices: device_list.clone(),
                };
                let msg_txt = serde_json::to_string(&msg).unwrap();
                for (_id, ptx) in devices_guard.iter() {
                    let _ = ptx.send(Message::Text(msg_txt.clone().into()));
                }
            }
            ws_sink_task.abort();
        });
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub total: usize,
    pub threshold: usize,
    pub participants: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Devices {
        devices: Vec<String>,
    },
    Relay {
        from: String,
        data: serde_json::Value,
    },
    Error {
        error: String,
    },
    // Session discovery messages
    SessionAvailable {
        session_info: serde_json::Value,
    },
    SessionListRequest {
        from: String,
    },
    // Simple session query response - just return what device was in
    SessionsForDevice {
        sessions: Vec<serde_json::Value>,  // List of session_info objects
    },
    // Notify when session is removed (creator disconnected)
    SessionRemoved {
        session_id: String,
        reason: String,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Register { device_id: String },
    ListDevices,
    Relay { to: String, data: serde_json::Value },
    // Session discovery messages
    AnnounceSession { session_info: serde_json::Value },
    RequestActiveSessions,
    SessionStatusUpdate { session_info: serde_json::Value },
    // Simple stateless rejoin support
    QueryMyActiveSessions,  // Device asks: "What sessions am I in?"
}
