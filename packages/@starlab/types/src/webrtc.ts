// ===================================================================
// WEBRTC APPLICATION MESSAGE TYPES
// ===================================================================
//
// This file defines the message types that are sent over established
// WebRTC data channels between participants in the MPC wallet system.
// These are application-level messages, not WebRTC signaling messages.
//
// Key Concepts for Junior Developers:
// - WebRTC Data Channel: A direct peer-to-peer connection for sending data
// - Application Messages: High-level messages our app sends/receives
// - DKG Packages: Cryptographic data exchanged during key generation
// - FROST Protocol: The specific cryptographic protocol we implement
// - Signing Process: Multi-party signature generation
// ===================================================================

// Application-Level Messages (sent over established WebRTC Data Channel)
// Format compatible with the TUI node (apps/tui) wire protocol
export type WebRTCAppMessage =
  // Basic communication
  | { webrtc_msg_type: 'SimpleMessage'; text: string }
  | { webrtc_msg_type: 'ChannelOpen'; device_id: string }
  | { webrtc_msg_type: 'MeshReady'; session_id: string; device_id: string }

  // DKG (Distributed Key Generation) Messages
  | { webrtc_msg_type: 'DkgRound1Package'; package: any } // frost_core::keys::dkg::round1::Package<Ed25519Sha512>
  | { webrtc_msg_type: 'DkgRound2Package'; package: any } // frost_core::keys::dkg::round2::Package<Ed25519Sha512>

  // FROST Signing Process Messages
  | { webrtc_msg_type: 'SigningRequest'; signing_id: string; transaction_data: string; required_signers: number }
  | { webrtc_msg_type: 'SigningAcceptance'; signing_id: string; accepted: boolean }
  | { webrtc_msg_type: 'SignerSelection'; signing_id: string; selected_signers: string[] } // Array of hex identifiers (64-char)
  | { webrtc_msg_type: 'SigningCommitment'; signing_id: string; sender_identifier: any; commitment: any } // FROST commitment
  | { webrtc_msg_type: 'SignatureShare'; signing_id: string; sender_identifier: any; share: any } // FROST signature share
  | { webrtc_msg_type: 'AggregatedSignature'; signing_id: string; signature: string } // Final signature as string

  // DKG Package Request Messages (for handling missing packages)
  | { webrtc_msg_type: 'DkgPackageRequest'; round: 1 | 2; requester: string } // Request a missing DKG package
  | { webrtc_msg_type: 'DkgPackageResend'; round: 1 | 2; package: any }; // Response with requested package

/**
 * Information about a WebRTC data channel connection.
 */
export interface DataChannelInfo {
  /** Device ID of the peer this channel connects to */
  peerId: string;

  /** Current state of the data channel */
  readyState: 'connecting' | 'open' | 'closing' | 'closed';

  /** Label/name of the data channel */
  label: string;

  /** Whether this channel is currently usable for sending messages */
  isUsable: boolean;

  /** When this channel was established */
  establishedAt?: number;

  /** Statistics about message throughput */
  messageStats?: {
    sent: number;
    received: number;
    lastActivity: number;
  };
}

/**
 * Status of all WebRTC connections and data channels.
 */
export interface WebRTCConnectionStatus {
  /** Map of peer ID to their connection state */
  peerConnections: Map<string, 'new' | 'connecting' | 'connected' | 'disconnected' | 'failed' | 'closed'>;

  /** Map of peer ID to their data channel information */
  dataChannels: Map<string, DataChannelInfo>;

  /** List of peers we can currently send messages to */
  availablePeers: string[];

  /** Whether the local WebRTC system is ready */
  localSystemReady: boolean;
}

/**
 * Events related to WebRTC data channel communication.
 */
export type WebRTCEvent =
  | { type: 'MessageReceived'; fromPeer: string; message: WebRTCAppMessage }
  | { type: 'MessageSent'; toPeer: string; message: WebRTCAppMessage }
  | { type: 'ChannelOpened'; peerId: string }
  | { type: 'ChannelClosed'; peerId: string }
  | { type: 'ConnectionEstablished'; peerId: string }
  | { type: 'ConnectionLost'; peerId: string }
  | { type: 'SendError'; toPeer: string; error: string };