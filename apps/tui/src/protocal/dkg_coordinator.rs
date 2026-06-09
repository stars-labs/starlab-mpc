//! DKG Coordinator - Manages the real FROST DKG protocol execution
//! 
//! This module coordinates the multi-round FROST DKG protocol between participants,
//! handling message exchange, state management, and protocol progression.

use frost_core::{Ciphersuite, Identifier};
use frost_core::keys::dkg::{part1, part2, part3};
use frost_core::keys::{KeyPackage, PublicKeyPackage};
use std::collections::BTreeMap;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use serde::{Serialize, Deserialize};
use tracing::{info, warn, debug};
use anyhow::{Result, anyhow};

/// Messages exchanged during DKG protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DKGMessage {
    /// Round 1: Broadcast commitment
    Round1Commitment {
        sender_id: Vec<u8>,
        package: String, // JSON serialized round1::Package
    },
    /// Round 2: Encrypted share for specific recipient
    Round2Share {
        sender_id: Vec<u8>,
        recipient_id: Vec<u8>,
        package: String, // JSON serialized round2::Package
    },
    /// DKG completion notification
    Complete {
        sender_id: Vec<u8>,
        public_key: Vec<u8>,
    },
    /// Error occurred during DKG
    Error {
        sender_id: Vec<u8>,
        error: String,
    },
}

/// DKG protocol state for a participant
pub struct DKGParticipant<C: Ciphersuite> {
    /// Our participant ID
    pub id: Identifier<C>,
    /// Total number of participants
    pub max_signers: u16,
    /// Threshold (minimum signers required)
    pub min_signers: u16,
    /// Round 1 secret package (kept private)
    round1_secret: Option<frost_core::keys::dkg::round1::SecretPackage<C>>,
    /// Our Round 1 public package
    round1_package: Option<frost_core::keys::dkg::round1::Package<C>>,
    /// Round 2 secret package (kept private)
    round2_secret: Option<frost_core::keys::dkg::round2::SecretPackage<C>>,
    /// Received Round 1 packages from other participants
    round1_packages_received: BTreeMap<Identifier<C>, frost_core::keys::dkg::round1::Package<C>>,
    /// Received Round 2 packages addressed to us
    round2_packages_received: BTreeMap<Identifier<C>, frost_core::keys::dkg::round2::Package<C>>,
    /// Final key package (our share)
    pub key_package: Option<KeyPackage<C>>,
    /// Public key package (group public key)
    pub pubkey_package: Option<PublicKeyPackage<C>>,
}

impl<C: Ciphersuite> DKGParticipant<C> {
    /// Create a new DKG participant
    pub fn new(id: Identifier<C>, max_signers: u16, min_signers: u16) -> Self {
        info!("Creating DKG participant with ID {:?}, max_signers={}, min_signers={}", 
              id, max_signers, min_signers);
        Self {
            id,
            max_signers,
            min_signers,
            round1_secret: None,
            round1_package: None,
            round2_secret: None,
            round1_packages_received: BTreeMap::new(),
            round2_packages_received: BTreeMap::new(),
            key_package: None,
            pubkey_package: None,
        }
    }

    /// Start Round 1: Generate and return commitment
    pub fn start_round1(&mut self) -> Result<String> {
        info!("Starting DKG Round 1 for participant {:?}", self.id);
        
        // Generate Round 1 commitment. FROST 2.2's `part1` takes an RNG that
        // implements `rand_core 0.6`'s `RngCore + CryptoRng`. `frost_core`
        // doesn't re-export rand_core, but `frost-ed25519` does (same rand_core
        // version since they share it transitively).
        let rng = frost_ed25519::rand_core::OsRng;
        let (secret, package) = part1(self.id, self.max_signers, self.min_signers, rng)
            .map_err(|e| anyhow!("Failed to generate Round 1 package: {:?}", e))?;
        
        // Store our packages
        self.round1_secret = Some(secret);
        self.round1_package = Some(package.clone());
        
        // Also store our own package in received
        self.round1_packages_received.insert(self.id, package.clone());
        
        // Serialize package for transmission
        let package_json = serde_json::to_string(&package)
            .map_err(|e| anyhow!("Failed to serialize Round 1 package: {}", e))?;
        
        info!("Generated Round 1 commitment for participant {:?}", self.id);
        Ok(package_json)
    }

    /// Process received Round 1 commitment from another participant
    pub fn receive_round1(&mut self, sender_id: Identifier<C>, package_json: &str) -> Result<()> {
        debug!("Receiving Round 1 package from {:?}", sender_id);
        
        // Don't process our own package again
        if sender_id == self.id {
            return Ok(());
        }
        
        // Deserialize the package
        let package: frost_core::keys::dkg::round1::Package<C> = serde_json::from_str(package_json)
            .map_err(|e| anyhow!("Failed to deserialize Round 1 package: {}", e))?;
        
        // Store the package
        self.round1_packages_received.insert(sender_id, package);
        
        info!("Stored Round 1 package from {:?}. Total: {}/{}", 
              sender_id, self.round1_packages_received.len(), self.max_signers);
        
        Ok(())
    }

    /// Check if we have all Round 1 packages
    pub fn ready_for_round2(&self) -> bool {
        self.round1_packages_received.len() == self.max_signers as usize
    }

    /// Start Round 2: Generate shares for other participants
    pub fn start_round2(&mut self) -> Result<BTreeMap<Identifier<C>, String>> {
        info!("Starting DKG Round 2 for participant {:?}", self.id);
        
        if !self.ready_for_round2() {
            return Err(anyhow!("Not ready for Round 2: missing Round 1 packages"));
        }
        
        // Get packages from other participants (excluding our own)
        let round1_packages_from_others: BTreeMap<_, _> = self.round1_packages_received
            .iter()
            .filter(|(id, _)| **id != self.id)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();
        
        // Generate Round 2 shares
        let round1_secret = self.round1_secret.take()
            .ok_or_else(|| anyhow!("Missing Round 1 secret"))?;
        
        let (secret, packages) = part2(round1_secret, &round1_packages_from_others)
            .map_err(|e| anyhow!("Failed to generate Round 2 packages: {:?}", e))?;
        
        self.round2_secret = Some(secret);
        
        // Serialize packages for transmission
        let mut serialized_packages = BTreeMap::new();
        for (recipient_id, package) in packages {
            let package_json = serde_json::to_string(&package)
                .map_err(|e| anyhow!("Failed to serialize Round 2 package: {}", e))?;
            serialized_packages.insert(recipient_id, package_json);
        }
        
        info!("Generated Round 2 shares for {} participants", serialized_packages.len());
        Ok(serialized_packages)
    }

    /// Process received Round 2 share addressed to us
    pub fn receive_round2(&mut self, sender_id: Identifier<C>, package_json: &str) -> Result<()> {
        debug!("Receiving Round 2 package from {:?}", sender_id);
        
        // Deserialize the package
        let package: frost_core::keys::dkg::round2::Package<C> = serde_json::from_str(package_json)
            .map_err(|e| anyhow!("Failed to deserialize Round 2 package: {}", e))?;
        
        // Store the package
        self.round2_packages_received.insert(sender_id, package);
        
        info!("Stored Round 2 package from {:?}. Total: {}/{}", 
              sender_id, self.round2_packages_received.len(), self.max_signers - 1);
        
        Ok(())
    }

    /// Check if we have all Round 2 packages
    pub fn ready_for_round3(&self) -> bool {
        // We should have packages from all other participants (not ourselves)
        self.round2_packages_received.len() == (self.max_signers - 1) as usize
    }

    /// Finalize DKG: Compute final key shares
    pub fn finalize(&mut self) -> Result<()> {
        info!("Finalizing DKG for participant {:?}", self.id);
        
        if !self.ready_for_round3() {
            return Err(anyhow!("Not ready for finalization: missing Round 2 packages"));
        }
        
        // Get Round 1 packages from others (for verification)
        let round1_packages_from_others: BTreeMap<_, _> = self.round1_packages_received
            .iter()
            .filter(|(id, _)| **id != self.id)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();
        
        // Get Round 2 secret
        let round2_secret = self.round2_secret.as_ref()
            .ok_or_else(|| anyhow!("Missing Round 2 secret"))?;
        
        // Finalize the DKG
        let (key_package, pubkey_package) = part3(
            round2_secret,
            &round1_packages_from_others,
            &self.round2_packages_received,
        ).map_err(|e| anyhow!("Failed to finalize DKG: {:?}", e))?;
        
        self.key_package = Some(key_package);
        self.pubkey_package = Some(pubkey_package.clone());
        
        info!("DKG finalized successfully for participant {:?}", self.id);
        info!("Group public key: {:?}", pubkey_package.verifying_key());
        
        Ok(())
    }
}

/// DKG Coordinator - Manages the DKG protocol execution
pub struct DKGCoordinator<C: Ciphersuite> {
    /// DKG participant state
    participant: DKGParticipant<C>,
    /// Channel to send messages to the network
    network_tx: UnboundedSender<DKGMessage>,
    /// Channel to receive messages from the network
    network_rx: UnboundedReceiver<DKGMessage>,
    /// Session ID for this DKG instance
    session_id: String,
    /// Current round of the protocol
    current_round: u8,
}

impl<C: Ciphersuite> DKGCoordinator<C> {
    /// Create a new DKG coordinator
    pub fn new(
        participant_id: u16,
        max_signers: u16,
        min_signers: u16,
        session_id: String,
        network_tx: UnboundedSender<DKGMessage>,
        network_rx: UnboundedReceiver<DKGMessage>,
    ) -> Result<Self> {
        let id = Identifier::try_from(participant_id)
            .map_err(|e| anyhow!("Invalid participant ID: {:?}", e))?;
        
        Ok(Self {
            participant: DKGParticipant::new(id, max_signers, min_signers),
            network_tx,
            network_rx,
            session_id,
            current_round: 0,
        })
    }

    /// Run the DKG protocol to completion
    pub async fn run(&mut self) -> Result<(KeyPackage<C>, PublicKeyPackage<C>)> {
        info!("Starting DKG protocol for session {}", self.session_id);
        
        // Round 1: Generate and broadcast commitment
        self.execute_round1().await?;
        
        // Wait for all Round 1 messages
        self.wait_for_round1_completion().await?;
        
        // Round 2: Generate and send shares
        self.execute_round2().await?;
        
        // Wait for all Round 2 messages
        self.wait_for_round2_completion().await?;
        
        // Round 3: Finalize
        self.execute_round3().await?;
        
        // Return the results
        let key_package = self.participant.key_package.clone()
            .ok_or_else(|| anyhow!("Missing key package after DKG"))?;
        let pubkey_package = self.participant.pubkey_package.clone()
            .ok_or_else(|| anyhow!("Missing public key package after DKG"))?;
        
        info!("DKG protocol completed successfully for session {}", self.session_id);
        Ok((key_package, pubkey_package))
    }

    /// Execute Round 1: Generate and broadcast commitment
    async fn execute_round1(&mut self) -> Result<()> {
        info!("Executing Round 1");
        self.current_round = 1;
        
        // Generate our Round 1 commitment
        let package_json = self.participant.start_round1()?;
        
        // Broadcast to all participants
        let msg = DKGMessage::Round1Commitment {
            sender_id: self.participant.id.serialize().to_vec(),
            package: package_json,
        };
        
        self.network_tx.send(msg)
            .map_err(|e| anyhow!("Failed to send Round 1 commitment: {}", e))?;
        
        Ok(())
    }

    /// Wait for all Round 1 messages
    async fn wait_for_round1_completion(&mut self) -> Result<()> {
        info!("Waiting for Round 1 messages from other participants");
        
        while !self.participant.ready_for_round2() {
            // Wait for next message with timeout
            let msg = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.network_rx.recv()
            ).await
            .map_err(|_| anyhow!("Timeout waiting for Round 1 messages"))?
            .ok_or_else(|| anyhow!("Network channel closed"))?;
            
            match msg {
                DKGMessage::Round1Commitment { sender_id, package } => {
                    // Parse sender ID
                    let sender = Identifier::<C>::deserialize(&sender_id)
                        .map_err(|e| anyhow!("Invalid sender ID: {:?}", e))?;
                    
                    // Process the commitment
                    self.participant.receive_round1(sender, &package)?;
                }
                DKGMessage::Error { sender_id: _, error } => {
                    return Err(anyhow!("Received error from participant: {}", error));
                }
                _ => {
                    warn!("Unexpected message type in Round 1: {:?}", msg);
                }
            }
        }
        
        info!("All Round 1 messages received");
        Ok(())
    }

    /// Execute Round 2: Generate and send shares
    async fn execute_round2(&mut self) -> Result<()> {
        info!("Executing Round 2");
        self.current_round = 2;
        
        // Generate Round 2 shares for other participants
        let shares = self.participant.start_round2()?;
        
        // Send each share to its recipient
        for (recipient_id, package_json) in shares {
            let msg = DKGMessage::Round2Share {
                sender_id: self.participant.id.serialize().to_vec(),
                recipient_id: recipient_id.serialize().to_vec(),
                package: package_json,
            };
            
            self.network_tx.send(msg)
                .map_err(|e| anyhow!("Failed to send Round 2 share: {}", e))?;
        }
        
        Ok(())
    }

    /// Wait for all Round 2 messages addressed to us
    async fn wait_for_round2_completion(&mut self) -> Result<()> {
        info!("Waiting for Round 2 shares from other participants");
        
        while !self.participant.ready_for_round3() {
            // Wait for next message with timeout
            let msg = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.network_rx.recv()
            ).await
            .map_err(|_| anyhow!("Timeout waiting for Round 2 messages"))?
            .ok_or_else(|| anyhow!("Network channel closed"))?;
            
            match msg {
                DKGMessage::Round2Share { sender_id, recipient_id, package } => {
                    // Check if this share is for us
                    let recipient = Identifier::<C>::deserialize(&recipient_id)
                        .map_err(|e| anyhow!("Invalid recipient ID: {:?}", e))?;
                    
                    if recipient == self.participant.id {
                        // Parse sender ID
                        let sender = Identifier::<C>::deserialize(&sender_id)
                            .map_err(|e| anyhow!("Invalid sender ID: {:?}", e))?;
                        
                        // Process the share
                        self.participant.receive_round2(sender, &package)?;
                    }
                }
                DKGMessage::Error { sender_id: _, error } => {
                    return Err(anyhow!("Received error from participant: {}", error));
                }
                _ => {
                    // Might receive shares for other participants, ignore
                    debug!("Ignoring message not for us in Round 2");
                }
            }
        }
        
        info!("All Round 2 shares received");
        Ok(())
    }

    /// Execute Round 3: Finalize DKG
    async fn execute_round3(&mut self) -> Result<()> {
        info!("Executing Round 3 (finalization)");
        self.current_round = 3;
        
        // Finalize the DKG
        self.participant.finalize()?;
        
        // Broadcast completion
        let pubkey = self.participant.pubkey_package
            .as_ref()
            .ok_or_else(|| anyhow!("Missing public key package"))?
            .verifying_key()
            .serialize();
        
        let pubkey_bytes = match pubkey {
            Ok(bytes) => bytes,
            Err(e) => return Err(anyhow!("Failed to serialize public key: {:?}", e)),
        };
        
        let msg = DKGMessage::Complete {
            sender_id: self.participant.id.serialize().to_vec(),
            public_key: pubkey_bytes.to_vec(),
        };
        
        self.network_tx.send(msg)
            .map_err(|e| anyhow!("Failed to send completion message: {}", e))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frost_ed25519::Ed25519Sha512;
    
    #[tokio::test]
    async fn test_dkg_participant_round1() {
        let id = Identifier::<Ed25519Sha512>::try_from(1).unwrap();
        let mut participant = DKGParticipant::new(id, 3, 2);
        
        // Should be able to start Round 1
        let package = participant.start_round1().unwrap();
        assert!(!package.is_empty());
        
        // Should have stored our own package
        assert!(participant.round1_package.is_some());
        assert_eq!(participant.round1_packages_received.len(), 1);
    }
    
    #[tokio::test]
    async fn test_dkg_participant_receive_round1() {
        let id1 = Identifier::<Ed25519Sha512>::try_from(1).unwrap();
        let id2 = Identifier::<Ed25519Sha512>::try_from(2).unwrap();
        
        let mut participant1 = DKGParticipant::new(id1, 3, 2);
        let mut participant2 = DKGParticipant::new(id2, 3, 2);
        
        // Participant 2 generates Round 1
        let package = participant2.start_round1().unwrap();
        
        // Participant 1 receives it
        participant1.receive_round1(id2, &package).unwrap();
        
        // Should not be ready for Round 2 yet (need all 3 participants)
        assert!(!participant1.ready_for_round2());
    }
}