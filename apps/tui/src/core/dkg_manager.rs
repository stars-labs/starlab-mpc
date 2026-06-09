//! DKG management logic shared between TUI and native nodes

use super::{CoreResult, CoreState, ParticipantInfo, ParticipantStatus, UICallback};
use std::sync::Arc;
use tracing::{error, info};

/// DKG manager that handles the distributed key generation process
pub struct DkgManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
}

impl DkgManager {
    pub fn new(state: Arc<CoreState>, ui_callback: Arc<dyn UICallback>) -> Self {
        Self { state, ui_callback }
    }
    
    /// Start the DKG process
    pub async fn start_dkg(&self, threshold: u16, participants: Vec<String>) -> CoreResult<()> {
        info!("Starting DKG with threshold {}/{}", threshold, participants.len());
        
        // Update state
        *self.state.dkg_active.lock().await = true;
        *self.state.dkg_round.lock().await = 1;
        *self.state.dkg_progress.lock().await = 0.0;
        
        // Create participant info
        let mut participant_infos = Vec::new();
        for (i, name) in participants.iter().enumerate() {
            participant_infos.push(ParticipantInfo {
                id: format!("P{}", i + 1),
                name: name.clone(),
                status: ParticipantStatus::Ready,
                round_completed: 0,
            });
        }
        
        *self.state.dkg_participants.lock().await = participant_infos.clone();
        
        // Update UI
        self.ui_callback.update_dkg_status(true, 1, 0.0).await;
        self.ui_callback.update_dkg_participants(participant_infos).await;
        
        // Start the actual DKG process
        self.execute_dkg_rounds(threshold, participants.len() as u16).await
    }
    
    /// Execute DKG rounds
    async fn execute_dkg_rounds(&self, _threshold: u16, _total: u16) -> CoreResult<()> {
        // Round 1: Generate commitments
        self.execute_round1().await?;
        
        // Round 2: Generate shares
        self.execute_round2().await?;
        
        // Round 3: Finalize
        self.execute_round3().await?;
        
        // Complete
        *self.state.dkg_active.lock().await = false;
        self.ui_callback.update_dkg_status(false, 3, 1.0).await;
        self.ui_callback.show_message("DKG completed successfully!".to_string(), false).await;
        
        Ok(())
    }
    
    /// Execute round 1: Generate commitments
    async fn execute_round1(&self) -> CoreResult<()> {
        info!("Executing DKG round 1: Generating commitments");
        
        *self.state.dkg_round.lock().await = 1;
        *self.state.dkg_progress.lock().await = 0.33;
        
        // Update participants status
        let mut participants = self.state.dkg_participants.lock().await;
        for p in participants.iter_mut() {
            p.status = ParticipantStatus::Processing;
            p.round_completed = 0;
        }
        let participants_clone = participants.clone();
        drop(participants);
        
        self.ui_callback.update_dkg_status(true, 1, 0.33).await;
        self.ui_callback.update_dkg_participants(participants_clone).await;
        
        // Simulate round 1 processing
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // Update participants after round 1
        let mut participants = self.state.dkg_participants.lock().await;
        for p in participants.iter_mut() {
            p.status = ParticipantStatus::Completed;
            p.round_completed = 1;
        }
        let participants_clone = participants.clone();
        drop(participants);
        
        self.ui_callback.update_dkg_participants(participants_clone).await;
        
        Ok(())
    }
    
    /// Execute round 2: Generate shares
    async fn execute_round2(&self) -> CoreResult<()> {
        info!("Executing DKG round 2: Generating shares");
        
        *self.state.dkg_round.lock().await = 2;
        *self.state.dkg_progress.lock().await = 0.66;
        
        // Update participants status
        let mut participants = self.state.dkg_participants.lock().await;
        for p in participants.iter_mut() {
            p.status = ParticipantStatus::Processing;
        }
        let participants_clone = participants.clone();
        drop(participants);
        
        self.ui_callback.update_dkg_status(true, 2, 0.66).await;
        self.ui_callback.update_dkg_participants(participants_clone).await;
        
        // Simulate round 2 processing
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // Update participants after round 2
        let mut participants = self.state.dkg_participants.lock().await;
        for p in participants.iter_mut() {
            p.status = ParticipantStatus::Completed;
            p.round_completed = 2;
        }
        let participants_clone = participants.clone();
        drop(participants);
        
        self.ui_callback.update_dkg_participants(participants_clone).await;
        
        Ok(())
    }
    
    /// Execute round 3: Finalize
    async fn execute_round3(&self) -> CoreResult<()> {
        info!("Executing DKG round 3: Finalizing keys");
        
        *self.state.dkg_round.lock().await = 3;
        *self.state.dkg_progress.lock().await = 0.9;
        
        // Update participants status
        let mut participants = self.state.dkg_participants.lock().await;
        for p in participants.iter_mut() {
            p.status = ParticipantStatus::Processing;
        }
        let participants_clone = participants.clone();
        drop(participants);
        
        self.ui_callback.update_dkg_status(true, 3, 0.9).await;
        self.ui_callback.update_dkg_participants(participants_clone).await;
        
        // Simulate round 3 processing
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        // Update participants after completion
        let mut participants = self.state.dkg_participants.lock().await;
        for p in participants.iter_mut() {
            p.status = ParticipantStatus::Completed;
            p.round_completed = 3;
        }
        let participants_clone = participants.clone();
        drop(participants);
        
        *self.state.dkg_progress.lock().await = 1.0;
        
        self.ui_callback.update_dkg_participants(participants_clone).await;
        self.ui_callback.update_dkg_status(true, 3, 1.0).await;
        
        Ok(())
    }
    
    /// Abort the DKG process
    pub async fn abort_dkg(&self) -> CoreResult<()> {
        info!("Aborting DKG process");
        
        *self.state.dkg_active.lock().await = false;
        *self.state.dkg_round.lock().await = 0;
        *self.state.dkg_progress.lock().await = 0.0;
        
        // Clear participants
        self.state.dkg_participants.lock().await.clear();
        
        // Update UI
        self.ui_callback.update_dkg_status(false, 0, 0.0).await;
        self.ui_callback.update_dkg_participants(Vec::new()).await;
        self.ui_callback.show_message("DKG aborted".to_string(), false).await;
        
        Ok(())
    }
    
    /// Handle participant disconnect during DKG
    pub async fn handle_participant_disconnect(&self, participant_id: String) -> CoreResult<()> {
        info!("Participant {} disconnected during DKG", participant_id);
        
        let mut participants = self.state.dkg_participants.lock().await;
        if let Some(p) = participants.iter_mut().find(|p| p.id == participant_id) {
            p.status = ParticipantStatus::Offline;
        }
        let participants_clone = participants.clone();
        drop(participants);
        
        self.ui_callback.update_dkg_participants(participants_clone).await;
        
        // Check if we still meet threshold
        let participants = self.state.dkg_participants.lock().await;
        let online_count = participants.iter()
            .filter(|p| p.status != ParticipantStatus::Offline && p.status != ParticipantStatus::Failed)
            .count();
        
        if let Some(session) = self.state.active_session.lock().await.as_ref() {
            let threshold = session.threshold.0 as usize;
            if online_count < threshold {
                error!("Below threshold after disconnect, aborting DKG");
                drop(participants);
                return self.abort_dkg().await;
            }
        }
        
        Ok(())
    }
    
    /// Handle participant rejoin during DKG
    pub async fn handle_participant_rejoin(&self, participant_id: String) -> CoreResult<()> {
        info!("Participant {} rejoining DKG", participant_id);
        
        let mut participants = self.state.dkg_participants.lock().await;
        if let Some(p) = participants.iter_mut().find(|p| p.id == participant_id) {
            // Check current round to determine status
            let current_round = *self.state.dkg_round.lock().await;
            p.status = if p.round_completed < current_round {
                ParticipantStatus::Processing
            } else {
                ParticipantStatus::Ready
            };
        }
        let participants_clone = participants.clone();
        drop(participants);
        
        self.ui_callback.update_dkg_participants(participants_clone).await;
        self.ui_callback.show_message(
            format!("Participant {} rejoined", participant_id),
            false
        ).await;
        
        Ok(())
    }
}