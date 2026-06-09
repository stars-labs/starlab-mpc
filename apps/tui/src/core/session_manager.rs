//! Session management logic shared between TUI and native nodes

use super::{CoreError, CoreResult, CoreState, SessionInfo, SessionStatus, UICallback};
use std::sync::Arc;
use tracing::info;

/// Session manager that handles session lifecycle
pub struct SessionManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
}

impl SessionManager {
    pub fn new(state: Arc<CoreState>, ui_callback: Arc<dyn UICallback>) -> Self {
        Self { state, ui_callback }
    }
    
    /// Create a new session
    pub async fn create_session(
        &self,
        device_id: String,
        threshold: u16,
        total: u16,
    ) -> CoreResult<String> {
        info!("Creating new session with threshold {}/{}", threshold, total);
        
        // Generate session ID
        let session_id = format!("session_{}", uuid::Uuid::new_v4());
        
        // Create session info
        let session = SessionInfo {
            session_id: session_id.clone(),
            initiator: device_id.clone(),
            participants: vec![device_id],
            threshold: (threshold, total),
            status: SessionStatus::Waiting,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        
        // Update state
        *self.state.active_session.lock().await = Some(session.clone());
        
        // Add to available sessions
        self.state.available_sessions.lock().await.push(session.clone());
        
        // Update UI
        self.ui_callback.update_active_session(Some(session.clone())).await;
        self.ui_callback.update_available_sessions(
            self.state.available_sessions.lock().await.clone()
        ).await;
        
        self.ui_callback.show_message(
            format!("Created session: {}", session_id),
            false
        ).await;
        
        Ok(session_id)
    }
    
    /// Join an existing session
    pub async fn join_session(&self, session_id: String, device_id: String) -> CoreResult<()> {
        info!("Joining session: {}", session_id);
        
        // Find session in available sessions
        let mut sessions = self.state.available_sessions.lock().await;
        let session = sessions.iter_mut()
            .find(|s| s.session_id == session_id)
            .ok_or_else(|| CoreError::Session(format!("Session {} not found", session_id)))?;
        
        // Check if already in session
        if session.participants.contains(&device_id) {
            return Err(CoreError::Session("Already in session".to_string()));
        }
        
        // Check if session is full
        if session.participants.len() >= session.threshold.1 as usize {
            return Err(CoreError::Session("Session is full".to_string()));
        }
        
        // Add participant
        session.participants.push(device_id.clone());
        
        // Update status if we have enough participants
        if session.participants.len() >= session.threshold.0 as usize {
            session.status = SessionStatus::InProgress;
        }
        
        let session_clone = session.clone();
        drop(sessions);
        
        // Set as active session
        *self.state.active_session.lock().await = Some(session_clone.clone());
        
        // Update UI
        self.ui_callback.update_active_session(Some(session_clone)).await;
        self.ui_callback.update_available_sessions(
            self.state.available_sessions.lock().await.clone()
        ).await;
        
        self.ui_callback.show_message(
            format!("Joined session: {}", session_id),
            false
        ).await;
        
        Ok(())
    }
    
    /// Leave the current session
    pub async fn leave_session(&self, device_id: String) -> CoreResult<()> {
        info!("Leaving current session");
        
        let active_session = self.state.active_session.lock().await.clone();
        if let Some(session) = active_session {
            // Remove from participants
            let mut sessions = self.state.available_sessions.lock().await;
            if let Some(s) = sessions.iter_mut().find(|s| s.session_id == session.session_id) {
                s.participants.retain(|p| p != &device_id);
                
                // Update status
                if s.participants.len() < s.threshold.0 as usize {
                    s.status = SessionStatus::Waiting;
                }
                
                // Remove session if empty
                if s.participants.is_empty() {
                    sessions.retain(|s| s.session_id != session.session_id);
                }
            }
            drop(sessions);
            
            // Clear active session
            *self.state.active_session.lock().await = None;
            
            // Update UI
            self.ui_callback.update_active_session(None).await;
            self.ui_callback.update_available_sessions(
                self.state.available_sessions.lock().await.clone()
            ).await;
            
            self.ui_callback.show_message(
                format!("Left session: {}", session.session_id),
                false
            ).await;
        }
        
        Ok(())
    }
    
    /// Refresh available sessions
    pub async fn refresh_sessions(&self) -> CoreResult<()> {
        info!("Refreshing available sessions");
        
        // In a real implementation, this would query the signaling server
        // For now, just update the UI with current state
        self.ui_callback.update_available_sessions(
            self.state.available_sessions.lock().await.clone()
        ).await;
        
        self.ui_callback.show_message(
            "Sessions refreshed".to_string(),
            false
        ).await;
        
        Ok(())
    }
    
    
    /// Get current active session
    pub async fn get_active_session(&self) -> Option<SessionInfo> {
        self.state.active_session.lock().await.clone()
    }
    
    /// Get all available sessions
    pub async fn get_available_sessions(&self) -> Vec<SessionInfo> {
        self.state.available_sessions.lock().await.clone()
    }
}