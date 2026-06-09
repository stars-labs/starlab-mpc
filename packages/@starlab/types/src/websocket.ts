// ===================================================================
// WEBSOCKET SIGNALING TYPES
// ===================================================================
//
// This file contains types for WebSocket-based signaling used to establish
// WebRTC connections between participants. This is the "signaling layer"
// that helps peers find each other and exchange connection information.
//
// Key Concepts for Junior Developers:
// - WebSocket: A persistent connection to a signaling server
// - Signaling: The process of helping two peers establish a direct connection
// - SDP: Session Description Protocol - describes connection capabilities
// - ICE Candidates: Information about how to reach a peer (IP addresses, ports)
// - Relay Server: A server that forwards messages between peers
// ===================================================================

// WebRTC Signaling Data (sent via WebSocket Relay)

/**
 * Session Description Protocol (SDP) information.
 * SDP describes the capabilities and configuration of a WebRTC endpoint.
 */
export interface SDPInfo {
  /** The SDP string containing connection information */
  sdp: string;
}

/**
 * Information about an ICE (Interactive Connectivity Establishment) candidate.
 * ICE candidates represent potential connection paths between peers.
 */
export interface CandidateInfo {
  /** The ICE candidate string (contains IP, port, protocol info) */
  candidate: string;

  /** Media stream identifier (optional) */
  sdpMid?: string | null; // sdp_mid in Rust

  /** Media line index in the SDP (optional) */
  sdpMLineIndex?: number | null; // sdp_mline_index in Rust (u16)
}

/**
 * WebRTC signaling messages exchanged to establish peer connections.
 * This uses Rust's externally tagged enum serialization format.
 */
export type WebRTCSignal =
  | { Offer: SDPInfo }   // Initial connection offer from caller
  | { Answer: SDPInfo }  // Response to offer from callee  
  | { Candidate: CandidateInfo }; // ICE candidate information

// WebSocket Messages (content of Relay data)

/**
 * WebSocket message payload - contains the actual data being relayed.
 * This models Rust's WebSocketMessage enum with discriminated unions.
 */
export type WebSocketMessagePayload =
  | ({ websocket_msg_type: 'SessionProposal' } & SessionProposal)
  | ({ websocket_msg_type: 'SessionResponse' } & SessionResponse)
  | ({ websocket_msg_type: 'WebRTCSignal' } & WebRTCSignal)
  // Ext-3c: peer-to-peer signing decline. Relayed via the signal
  // server since the decliner hasn't joined the WebRTC mesh yet.
  // Proposer receives as a `relay` frame and surfaces a toast.
  | {
      websocket_msg_type: 'SigningDecline';
      signing_id: string;
      /** Optional — older clients may omit it; proposer falls back
       *  to `msg.from` from the wrapping relay frame. */
      decliner_id?: string;
    };

/**
 * Messages sent FROM the server TO clients.
 * Uses Rust's internally tagged enum serialization with "type" field
 * (snake_case). Shape-identical to
 * `apps/signal-server/cloudflare-worker/src/lib.rs::ServerMsg`.
 */
export type ServerMsg =
  | { type: "devices"; devices: string[] }  // List of online devices
  | { type: "relay"; from: string; data: WebSocketMessagePayload }  // Relayed message
  | { type: "error"; error: string }
  // Session-discovery channel (TUI's primary path; added to extension
  // in Ext-1a). The server broadcasts a `session_available` to every
  // OTHER connected client whenever any client sends an
  // `announce_session`. `session_info` is a loose JSON blob because
  // the server itself doesn't care about the shape — it just stores
  // and forwards. The extension decodes it as `SessionInfo` via a
  // tolerant parser that fills in defaults.
  | { type: "session_available"; session_info: SessionInfo }
  | { type: "session_removed"; session_id: string; reason: string }
  | { type: "sessions_for_device"; sessions: SessionInfo[] };

/**
 * Messages sent FROM clients TO the server.
 */
export type ClientMsg =
  | { type: "register"; device_id: string }  // Register this device with server
  | { type: "list_devices" }  // Request list of online devices
  | { type: "relay"; to: string; data: WebSocketMessagePayload }  // Send message to another device
  // Session-discovery outbound (Ext-1a). Mirrors TUI's DKG/Signing
  // announcement path. Server broadcasts this as `session_available`
  // to every other connected client.
  | { type: "announce_session"; session_info: SessionInfo }
  | { type: "request_active_sessions" }
  | { type: "session_status_update"; session_info: SessionInfo };

/**
 * Status of the WebSocket connection to the signaling server.
 */
export interface WebSocketStatus {
  /** Whether we're currently connected to the signaling server */
  connected: boolean;

  /** URL of the signaling server */
  serverUrl: string;

  /** Our registered device ID on the server */
  deviceId: string | null;

  /** List of other devices currently online */
  onlineDevices: string[];

  /** Number of connection attempts made */
  connectionAttempts: number;

  /** Timestamp of last successful connection */
  lastConnected?: number;

  /** Any current error message */
  errorMessage?: string;
}

/**
 * Events related to WebSocket signaling.
 */
export type WebSocketEvent =
  | { type: 'Connected'; serverUrl: string }
  | { type: 'Disconnected'; reason?: string }
  | { type: 'DeviceListUpdated'; devices: string[] }
  | { type: 'MessageReceived'; from: string; data: WebSocketMessagePayload }
  | { type: 'MessageSent'; to: string; data: WebSocketMessagePayload }
  | { type: 'Error'; error: string }
  | { type: 'Registered'; deviceId: string };

// Re-export session types for convenience (they're used in WebSocket messages)
import type { SessionProposal, SessionResponse, SessionInfo } from './session';