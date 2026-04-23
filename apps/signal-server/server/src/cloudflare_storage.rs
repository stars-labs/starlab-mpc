/// Cloudflare Worker storage adapter for shared session management
/// 
/// This module provides a bridge between the shared session management logic
/// and Cloudflare's Durable Objects storage API.
use crate::session_manager::{SessionStorage, StoredSession};
use serde_json::Value;
use std::collections::HashMap;

/// Cloudflare storage adapter that can be used from Worker context
/// This is a temporary in-memory storage for request handling
/// The actual persistence happens through Durable Objects API calls
pub struct CloudflareSessionStorage {
    // Temporary cache for the duration of a request
    sessions: HashMap<String, StoredSession>,
    device_sessions: HashMap<String, Vec<String>>,
    // Track changes to persist back to Durable Objects
    pub changed_sessions: Vec<String>,
    pub removed_sessions: Vec<String>,
}

impl Default for CloudflareSessionStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudflareSessionStorage {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            device_sessions: HashMap::new(),
            changed_sessions: Vec::new(),
            removed_sessions: Vec::new(),
        }
    }
    
    /// Load session from JSON (from Durable Objects storage)
    pub fn load_session(&mut self, session_key: String, json_data: Value) {
        if let Ok(session) = serde_json::from_value::<StoredSession>(json_data) {
            self.sessions.insert(session_key, session);
        }
    }
    
    /// Load all sessions from a list of (key, value) pairs
    pub fn load_sessions(&mut self, sessions: Vec<(String, Value)>) {
        for (key, value) in sessions {
            // Remove "session:" prefix if present
            let session_key = key.replace("session:", "");
            self.load_session(session_key, value);
        }
    }
    
    /// Get sessions that need to be persisted back to Durable Objects
    pub fn get_changed_sessions(&self) -> Vec<(String, StoredSession)> {
        self.changed_sessions.iter()
            .filter_map(|key| {
                self.sessions.get(key).map(|s| (key.clone(), s.clone()))
            })
            .collect()
    }
    
    /// Load device sessions mapping
    pub fn load_device_sessions(&mut self, device_id: String, session_ids: Vec<String>) {
        self.device_sessions.insert(device_id, session_ids);
    }
}

impl SessionStorage for CloudflareSessionStorage {
    fn store_session(&mut self, session_key: String, session: StoredSession) {
        self.sessions.insert(session_key.clone(), session);
        if !self.changed_sessions.contains(&session_key) {
            self.changed_sessions.push(session_key);
        }
    }
    
    fn get_session(&self, session_key: &str) -> Option<&StoredSession> {
        self.sessions.get(session_key)
    }
    
    fn get_session_mut(&mut self, session_key: &str) -> Option<&mut StoredSession> {
        if !self.changed_sessions.contains(&session_key.to_string()) {
            self.changed_sessions.push(session_key.to_string());
        }
        self.sessions.get_mut(session_key)
    }
    
    fn remove_session(&mut self, session_key: &str) -> Option<StoredSession> {
        self.removed_sessions.push(session_key.to_string());
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

/// Helper functions for Cloudflare Worker integration
impl CloudflareSessionStorage {
    /// Convert session data for storage in Durable Objects
    pub fn session_to_json(session: &StoredSession) -> Value {
        serde_json::to_value(session).unwrap_or(serde_json::json!({}))
    }
    
    /// Check if a session key exists in cache
    pub fn has_session(&self, session_key: &str) -> bool {
        self.sessions.contains_key(session_key)
    }
    
    /// Get all device session mappings that changed
    pub fn get_device_sessions_map(&self) -> &HashMap<String, Vec<String>> {
        &self.device_sessions
    }
}