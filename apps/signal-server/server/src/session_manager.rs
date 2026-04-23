use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Core session data structure shared between implementations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredSession {
    pub session_info: Value,
    pub active_participants: Vec<String>,
}

/// Session manager trait for different storage backends
pub trait SessionStorage {
    /// Store a session
    fn store_session(&mut self, session_key: String, session: StoredSession);
    
    /// Get a session
    fn get_session(&self, session_key: &str) -> Option<&StoredSession>;
    
    /// Get mutable session
    fn get_session_mut(&mut self, session_key: &str) -> Option<&mut StoredSession>;
    
    /// Remove a session
    fn remove_session(&mut self, session_key: &str) -> Option<StoredSession>;
    
    /// Get all sessions
    fn get_all_sessions(&self) -> Vec<(&String, &StoredSession)>;
    
    /// Track device to sessions mapping
    fn add_device_session(&mut self, device_id: String, session_key: String);
    
    /// Get sessions for a device
    fn get_device_sessions(&self, device_id: &str) -> Vec<String>;
    
    /// Remove device and get its sessions
    fn remove_device(&mut self, device_id: &str) -> Vec<String>;
}

/// In-memory implementation for standalone server
pub struct InMemorySessionStorage {
    sessions: HashMap<String, StoredSession>,
    device_sessions: HashMap<String, Vec<String>>,
}

impl Default for InMemorySessionStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySessionStorage {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            device_sessions: HashMap::new(),
        }
    }
}

impl SessionStorage for InMemorySessionStorage {
    fn store_session(&mut self, session_key: String, session: StoredSession) {
        self.sessions.insert(session_key, session);
    }
    
    fn get_session(&self, session_key: &str) -> Option<&StoredSession> {
        self.sessions.get(session_key)
    }
    
    fn get_session_mut(&mut self, session_key: &str) -> Option<&mut StoredSession> {
        self.sessions.get_mut(session_key)
    }
    
    fn remove_session(&mut self, session_key: &str) -> Option<StoredSession> {
        self.sessions.remove(session_key)
    }
    
    fn get_all_sessions(&self) -> Vec<(&String, &StoredSession)> {
        self.sessions.iter().collect()
    }
    
    fn add_device_session(&mut self, device_id: String, session_key: String) {
        self.device_sessions
            .entry(device_id)
            .or_default()
            .push(session_key);
    }
    
    fn get_device_sessions(&self, device_id: &str) -> Vec<String> {
        self.device_sessions
            .get(device_id)
            .cloned()
            .unwrap_or_default()
    }
    
    fn remove_device(&mut self, device_id: &str) -> Vec<String> {
        self.device_sessions.remove(device_id).unwrap_or_default()
    }
}

/// Core session management logic shared between implementations
pub struct SessionManager;

impl SessionManager {
    /// Extract session key from session info
    pub fn extract_session_key(session_info: &Value) -> String {
        session_info.get("session_code")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                // Fallback to timestamp-based key
                format!("session-{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis())
            })
    }
    
    /// Check if a device is a participant in a session
    pub fn is_participant(session_info: &Value, device_id: &str) -> bool {
        session_info.get("participants")
            .and_then(|v| v.as_array())
            .map(|participants| {
                participants.iter()
                    .any(|p| p.as_str() == Some(device_id))
            })
            .unwrap_or(false)
    }
    
    /// Process SessionUpdate message to track participants
    pub fn process_session_update(
        data: &Value,
        storage: &mut dyn SessionStorage,
        connected_devices: &[String],
    ) -> Option<String> {
        if data.get("type").and_then(|v| v.as_str()) != Some("SessionUpdate") {
            return None;
        }
        
        let session_code = data.get("session_code").and_then(|v| v.as_str())?;
        let participants = data.get("participants").and_then(|v| v.as_array())?;
        
        // Update session if it exists
        if let Some(session) = storage.get_session_mut(session_code) {
            // Update active participants based on who's connected
            session.active_participants.clear();
            for p in participants {
                if let Some(participant_id) = p.as_str()
                    && connected_devices.contains(&participant_id.to_string()) {
                        session.active_participants.push(participant_id.to_string());
                    }
            }
            
            // Update session info
            session.session_info = data.clone();
            
            // Track new participants
            for p in participants {
                if let Some(participant_id) = p.as_str() {
                    storage.add_device_session(
                        participant_id.to_string(),
                        session_code.to_string()
                    );
                }
            }
        }
        
        Some(session_code.to_string())
    }
    
    /// Handle device disconnect - returns sessions to remove
    pub fn handle_device_disconnect(
        device_id: &str,
        storage: &mut dyn SessionStorage,
    ) -> Vec<String> {
        let mut sessions_to_remove = Vec::new();
        let session_ids = storage.get_device_sessions(device_id);
        
        for session_id in &session_ids {
            if let Some(session) = storage.get_session_mut(session_id) {
                // Remove from active participants
                session.active_participants.retain(|p| p != device_id);
                
                // Only remove session if NO active participants remain
                if session.active_participants.is_empty() {
                    sessions_to_remove.push(session_id.clone());
                }
            }
        }
        
        // Remove sessions with no active participants
        for session_id in &sessions_to_remove {
            storage.remove_session(session_id);
        }
        
        // Clean up device tracking
        storage.remove_device(device_id);
        
        sessions_to_remove
    }
    
    /// Handle device rejoin - returns sessions they're in
    pub fn handle_device_rejoin(
        device_id: &str,
        storage: &mut dyn SessionStorage,
    ) -> Vec<Value> {
        let mut my_sessions = Vec::new();
        let mut session_keys_to_track = Vec::new();
        
        // First collect sessions where device is participant
        let all_sessions: Vec<(String, Value, bool)> = storage.get_all_sessions()
            .iter()
            .filter_map(|(key, session)| {
                if Self::is_participant(&session.session_info, device_id) {
                    let needs_update = !session.active_participants.contains(&device_id.to_string());
                    Some(((*key).clone(), session.session_info.clone(), needs_update))
                } else {
                    None
                }
            })
            .collect();
        
        // Now update active participants
        for (key, session_info, needs_update) in all_sessions {
            if needs_update
                && let Some(session_mut) = storage.get_session_mut(&key) {
                    session_mut.active_participants.push(device_id.to_string());
                }
            my_sessions.push(session_info);
            session_keys_to_track.push(key);
        }
        
        // Update device sessions tracking
        for session_key in session_keys_to_track {
            storage.add_device_session(device_id.to_string(), session_key);
        }
        
        my_sessions
    }
}