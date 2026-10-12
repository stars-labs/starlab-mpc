export interface SessionInfo {
  session_id: string;
  proposer_id: string;
  total: number; // u16 in Rust, number in TS
  threshold: number; // u16 in Rust, number in TS
  participants: string[];
  accepted_devices: string[];
  status?: string; // Add optional status field
}


// --- Enums for State Management ---
export enum DkgState {
  Idle,
  Initializing,
  Round1InProgress,
  Round1Complete,
  Round2InProgress,
  Round2Complete,
  Finalizing,
  Complete,
  Failed,
}

export enum MeshStatusType {
  Incomplete,
  PartiallyReady,
  Ready,
}

export type MeshStatus =
  | { type: MeshStatusType.Incomplete }
  | { type: MeshStatusType.PartiallyReady; ready_devices: Set<string>; total_devices: number }
  | { type: MeshStatusType.Ready };

// --- WebRTC Signaling ---
export interface SessionProposal {
  session_id: string;
  total: number;
  threshold: number;
  participants: string[];
}

export interface SessionResponse {
  session_id: string;
  accepted: boolean;
}

/**
 * Supported blockchain networks.
 * Grouped by cryptographic curve compatibility.
 */
export type SupportedChain =
  // secp256k1-based chains
  | "ethereum"
  | "polygon"
  | "arbitrum"
  | "optimism"
  | "base"
  // ed25519-based chains
  | "solana"
  | "sui";

/**
 * Mapping of cryptographic curves to their compatible blockchain networks.
 * This enables proper validation while maintaining independence.
 */
export const CURVE_COMPATIBLE_CHAINS = {
  secp256k1: ["ethereum", "polygon", "arbitrum", "optimism", "base"] as const,
  ed25519: ["solana", "sui"] as const,
} as const;

/**
 * Helper function to get compatible chains for a given curve.
 */
export function getCompatibleChains(curve: "secp256k1" | "ed25519"): readonly SupportedChain[] {
  return CURVE_COMPATIBLE_CHAINS[curve];
}

/**
 * Helper function to get the required curve for a given chain.
 */
export function getRequiredCurve(chain: SupportedChain): "secp256k1" | "ed25519" {
  if (CURVE_COMPATIBLE_CHAINS.secp256k1.includes(chain as any)) {
    return "secp256k1";
  }
  if (CURVE_COMPATIBLE_CHAINS.ed25519.includes(chain as any)) {
    return "ed25519";
  }
  throw new Error(`Unknown chain: ${chain}`);
}

export type WebRTCAppMessage =
  | { webrtc_msg_type: 'ChannelOpen'; device_id: string }
  | { webrtc_msg_type: 'MeshReady'; session_id: string; device_id: string }
  | { webrtc_msg_type: 'SimpleMessage'; text: string }
  | { webrtc_msg_type: 'DkgRound1Package'; package: any }
  | { webrtc_msg_type: 'DkgRound2Package'; package: any };

// Define the main application state interface
export interface AppState {
  deviceId: string;
  connecteddevices: string[];
  wsConnected: boolean;
  sessionInfo: SessionInfo | null;
  invites: SessionInfo[];
  meshStatus: MeshStatus;
  dkgState: DkgState;
  webrtcConnections: Record<string, boolean>;
  blockchain?: "ethereum" | "solana"; // Current blockchain selection for the active session
  chain?: "ethereum" | "solana"; // Add chain property for compatibility
}

// Export the initial state for use in components
export const INITIAL_APP_STATE: AppState = {
  deviceId: '',
  connecteddevices: [],
  wsConnected: false,
  sessionInfo: null,
  invites: [],
  meshStatus: { type: MeshStatusType.Incomplete },
  dkgState: DkgState.Idle,
  webrtcConnections: {},
  chain: 'ethereum', // Default chain for UI
};