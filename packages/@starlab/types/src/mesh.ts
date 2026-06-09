// ===================================================================
// MESH NETWORK STATUS TYPES
// ===================================================================
//
// This file contains types for managing the status of the mesh network
// in the MPC wallet system. The mesh network represents the WebRTC 
// connections between all participants in a session.
//
// Key Concepts for Junior Developers:
// - Mesh Network: A network topology where each participant connects
//   directly to every other participant (peer-to-peer)
// - WebRTC: Technology for direct browser-to-browser communication
// - Ready State: All participants are connected and ready for operations
// - Partial State: Some participants are connected, others are not
// ===================================================================

/**
 * The different states a mesh network can be in.
 * This helps track whether all participants are properly connected.
 */
export enum MeshStatusType {
    /** No connections established yet, or major connectivity issues */
    Incomplete = 0,

    /** Some participants are connected, but not all */
    PartiallyReady = 1,

    /** All participants are connected and ready for operations */
    Ready = 2,
}

/**
 * Detailed information about the mesh network status.
 * This uses a discriminated union pattern - each status type
 * has different associated data.
 */
export type MeshStatus =
    | {
        /** Network is incomplete - no additional data needed */
        type: MeshStatusType.Incomplete
    }
    | {
        /** Network is partially ready */
        type: MeshStatusType.PartiallyReady;
        /** Set of device IDs that are currently connected and ready */
        ready_devices: Set<string>;
        /** Total number of devices that should be connected */
        total_devices: number;
    }
    | {
        /** Network is fully ready - no additional data needed */
        type: MeshStatusType.Ready
    };

/**
 * Information about a specific peer connection in the mesh.
 */
export interface PeerConnectionInfo {
    /** Device ID of the peer */
    peerId: string;

    /** Whether the WebRTC connection is established */
    connected: boolean;

    /** Whether the data channel is open and ready */
    dataChannelReady: boolean;

    /** Current state of the WebRTC connection */
    connectionState: 'new' | 'connecting' | 'connected' | 'disconnected' | 'failed' | 'closed';

    /** Current state of the data channel */
    dataChannelState: 'connecting' | 'open' | 'closing' | 'closed';

    /** When this connection was established */
    connectedAt?: number;

    /** Last time we received data from this peer */
    lastActivity?: number;
}

/**
 * Complete status of all mesh connections.
 */
export interface MeshNetworkStatus {
    /** Overall mesh status */
    meshStatus: MeshStatus;

    /** Detailed information about each peer connection */
    peerConnections: Map<string, PeerConnectionInfo>;

    /** List of device IDs that we're currently trying to connect to */
    connectingTo: string[];

    /** List of device IDs that failed to connect */
    failedConnections: string[];

    /** Whether we've sent our "mesh ready" signal to all peers */
    meshReadySignalSent: boolean;
}

/**
 * Events related to mesh network status changes.
 */
export type MeshNetworkEvent =
    | { type: 'StatusChanged'; oldStatus: MeshStatusType; newStatus: MeshStatusType }
    | { type: 'PeerConnected'; peerId: string }
    | { type: 'PeerDisconnected'; peerId: string }
    | { type: 'DataChannelOpened'; peerId: string }
    | { type: 'DataChannelClosed'; peerId: string }
    | { type: 'MeshReady' }
    | { type: 'ConnectionFailed'; peerId: string; reason: string };

/**
 * Utility type for checking mesh readiness conditions.
 */
export interface MeshReadinessCheck {
    /** Whether all expected peers are connected */
    allPeersConnected: boolean;

    /** Whether all data channels are open */
    allDataChannelsReady: boolean;

    /** Whether we can proceed with DKG operations */
    readyForDkg: boolean;

    /** List of peers that are not yet ready */
    pendingPeers: string[];
}

// Ensure this file is treated as a module
export { };
