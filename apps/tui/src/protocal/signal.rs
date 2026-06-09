use frost_core::Ciphersuite;
use serde::{Deserialize, Serialize};

use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

/// Curve type for cryptographic operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CurveType {
    Secp256k1,
    Ed25519,
}

/// Coordination type for session management
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CoordinationType {
    Network,
    Offline,
}

fn default_coordination_type() -> String {
    "Network".to_string()
}

/// Session type enum - represents different types of signing networks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum SessionType {
    /// Distributed Key Generation session
    DKG,
    /// Signing session with existing wallet
    Signing {
        wallet_name: String,
        curve_type: String,
        blockchain: String,
        group_public_key: String,
    },
    /// Share refresh / resharing of an existing wallet (#45). Carries the OLD
    /// group public key so a joiner only participates if it owns that wallet;
    /// `participants` (on the enclosing `SessionInfo`) is the RETAINED signer
    /// set after the refresh (a removed device simply isn't listed).
    Reshare {
        wallet_name: String,
        curve_type: String,
        group_public_key: String,
    },
}
// Import the DKG Package type
// Import round1 and round2 packages

// --- Session Info Struct ---
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub session_id: String,
    pub proposer_id: String, // Added field
    pub total: u16,
    pub threshold: u16,
    pub participants: Vec<String>, // List of device_ids that have joined the session
    pub session_type: SessionType,
    /// Cryptographic curve type from the proposer
    pub curve_type: String,
    /// Coordination type from the proposer
    #[serde(default = "default_coordination_type")]
    pub coordination_type: String,
    /// For signing sessions only: hex-encoded bytes the ceremony should
    /// sign. The creator publishes this in the announce so joiners can
    /// confirm+sign the exact same payload without an extra round-trip.
    /// `None` on DKG sessions and on older signing announces that
    /// pre-date this field. Default lets us round-trip legacy wire data.
    #[serde(default)]
    pub signing_message_hex: Option<String>,
}

// --- WebRTC Signaling Data (sent via Relay) ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WebRTCSignal {
    Offer(SDPInfo),
    Answer(SDPInfo),
    Candidate(CandidateInfo),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SDPInfo {
    pub sdp: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CandidateInfo {
    pub candidate: String,
    #[serde(rename = "sdpMid")]
    pub sdp_mid: Option<String>,
    #[serde(rename = "sdpMLineIndex")]
    pub sdp_mline_index: Option<u16>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "websocket_msg_type")]
pub enum WebSocketMessage {
    // Relay Messages
    /// Session proposal message
    SessionProposal(SessionProposal),
    /// Session response message
    SessionResponse(SessionResponse),
    /// Session update message (participant list changes)
    SessionUpdate(SessionUpdate),
    /// Session join request (for joining/rejoining)
    SessionJoinRequest(SessionJoinRequest),
    /// Session offer (compatibility with message validator)
    SessionOffer(SessionInfo),
    /// Session accepted (compatibility with message validator)
    SessionAccepted { device_id: String, session_id: String },
    WebRTCSignal(WebRTCSignal),
}

/// Session proposal information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionProposal {
    pub session_id: String,
    pub total: u16,
    pub threshold: u16,
    pub participants: Vec<String>,
    pub session_type: SessionType,
    /// Device ID of the wallet creator/proposer
    pub proposer_device_id: String,
    /// Cryptographic curve type (secp256k1 or ed25519)
    pub curve_type: String,
    /// Coordination type (network or file)
    #[serde(default = "default_coordination_type")]
    pub coordination_type: String,
}

/// Session join request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJoinRequest {
    pub session_id: String,
    pub device_id: String,
    pub is_rejoin: bool,
}

/// Session announcement for discovery
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionAnnouncement {
    pub session_code: String,
    pub wallet_type: String,
    pub threshold: u16,
    pub total: u16,
    pub curve_type: String,
    pub creator_device: String,
    pub participants_joined: u16,
    pub description: Option<String>,
    pub timestamp: u64,
}

/// Session response information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    pub session_id: String,
    pub from_device_id: String,  // Added to identify sender
    pub accepted: bool,
    pub wallet_status: Option<WalletStatus>,
    pub reason: Option<String>,   // Added for rejoin reason
}

/// Session update information - broadcast when participants join/leave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUpdate {
    pub session_id: String,
    pub participants: Vec<String>,
    pub update_type: SessionUpdateType,
    pub timestamp: u64,  // Added for ordering updates
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionUpdateType {
    ParticipantJoined,
    ParticipantLeft,
    ParticipantRejoined,  // Added for rejoin scenario
    FullSync,
}

/// Wallet status for signing sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletStatus {
    pub has_wallet: bool,
    pub wallet_valid: bool,
    pub identifier: Option<u16>,
    pub error_reason: Option<String>,
}

impl SessionInfo {
    /// Determines the consensus leader using a deterministic algorithm
    /// based on lexicographic ordering of participants.
    /// This ensures all nodes agree on the leader without central coordination.
    pub fn get_consensus_leader(&self) -> String {
        // If we have participants, use the first one lexicographically
        if !self.participants.is_empty() {
            let mut sorted_devices = self.participants.clone();
            sorted_devices.sort();
            sorted_devices[0].clone()
        } else {
            // Fallback to proposer if no participants yet
            self.proposer_id.clone()
        }
    }
}

// --- Application-Level Messages (sent over established WebRTC Data Channel) ---
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "webrtc_msg_type")]
#[serde(bound(
    serialize = "frost_core::keys::dkg::round1::Package<C>: serde::Serialize, frost_core::keys::dkg::round2::Package<C>: serde::Serialize, frost_core::round1::SigningCommitments<C>: serde::Serialize, frost_core::round2::SignatureShare<C>: serde::Serialize, frost_core::Identifier<C>: serde::Serialize",
    deserialize = "frost_core::keys::dkg::round1::Package<C>: serde::Deserialize<'de>, frost_core::keys::dkg::round2::Package<C>: serde::Deserialize<'de>, frost_core::round1::SigningCommitments<C>: serde::Deserialize<'de>, frost_core::round2::SignatureShare<C>: serde::Deserialize<'de>, frost_core::Identifier<C>: serde::Deserialize<'de>"
))]
pub enum WebRTCMessage<C: Ciphersuite> {
    // DKG Messages
    SimpleMessage {
        text: String,
    },
    DkgRound1Package {
        package: frost_core::keys::dkg::round1::Package<C>,
    },
    // Add other message types as needed (e.g., for signing)
    DkgRound2Package {
        package: frost_core::keys::dkg::round2::Package<C>,
    },
    /// Reshare (share refresh) round-1 package — broadcast to all retained
    /// peers, exactly like `DkgRound1Package`. Refresh reuses frost's dkg
    /// round-1 Package type (#45).
    ReshareRound1Package {
        package: frost_core::keys::dkg::round1::Package<C>,
    },
    /// Reshare round-2 package — sent point-to-point to one recipient (the
    /// sender is the data-channel peer), mirroring `DkgRound2Package`.
    ReshareRound2Package {
        package: frost_core::keys::dkg::round2::Package<C>,
    },
    /// Data channel opened notification
    ChannelOpen {
        device_id: String,
    },
    /// Mesh readiness notification
    MeshReady {
        session_id: String,
        device_id: String,
    },

    // --- Signing Messages ---
    /// Transaction signing request
    SigningRequest {
        signing_id: String,
        transaction_data: String, // Hex-encoded transaction data
        required_signers: usize,
        blockchain: String,       // Blockchain identifier
        chain_id: Option<u64>,    // Chain ID for EVM chains
    },

    /// Acceptance of a signing request
    SigningAcceptance {
        signing_id: String,
        accepted: bool,
    },

    /// Selected signers for threshold signing
    SignerSelection {
        signing_id: String,
        selected_signers: Vec<frost_core::Identifier<C>>,
    },

    /// FROST signing commitments (Round 1)
    SigningCommitment {
        signing_id: String,
        sender_identifier: frost_core::Identifier<C>,
        commitment: frost_core::round1::SigningCommitments<C>,
    },

    /// FROST signature shares (Round 2)
    SignatureShare {
        signing_id: String,
        sender_identifier: frost_core::Identifier<C>,
        share: frost_core::round2::SignatureShare<C>,
    },

    /// Final aggregated signature
    AggregatedSignature {
        signing_id: String,
        signature: Vec<u8>, // The final signature bytes
    },
}

// Helper to convert RTCIceCandidate to CandidateInfo
impl From<RTCIceCandidateInit> for CandidateInfo {
    fn from(init: RTCIceCandidateInit) -> Self {
        CandidateInfo {
            candidate: init.candidate,
            sdp_mid: init.sdp_mid,
            sdp_mline_index: init.sdp_mline_index,
        }
    }
}

// Helper to convert RTCSessionDescription to SDPInfo
impl From<RTCSessionDescription> for SDPInfo {
    fn from(desc: RTCSessionDescription) -> Self {
        SDPInfo { sdp: desc.sdp }
    }
}

#[cfg(test)]
mod reshare_type_tests {
    use super::*;

    #[test]
    fn reshare_session_type_serde_roundtrips() {
        let st = SessionType::Reshare {
            wallet_name: "wallet-ab12".into(),
            curve_type: "secp256k1".into(),
            group_public_key: "02deadbeef".into(),
        };
        let json = serde_json::to_string(&st).unwrap();
        // Tagged shape: {"type":"Reshare","data":{...}} — co-existing with DKG/Signing.
        assert!(json.contains("\"type\":\"Reshare\""), "got {json}");
        let back: SessionType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, st);
    }

    #[test]
    fn reshare_session_info_roundtrips_with_retained_participants() {
        let info = SessionInfo {
            session_id: "reshare_x".into(),
            proposer_id: "alice".into(),
            total: 2,
            threshold: 2,
            participants: vec!["alice".into(), "carol".into()], // retained set (bob removed)
            session_type: SessionType::Reshare {
                wallet_name: "W".into(),
                curve_type: "ed25519".into(),
                group_public_key: "abcd".into(),
            },
            curve_type: "ed25519".into(),
            coordination_type: "online".into(),
            signing_message_hex: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back, info);
    }
}
