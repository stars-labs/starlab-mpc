// Re-export domain types so existing imports from '@mpc-wallet/types/appstate' still work.
// Canonical definitions live in session.ts, dkg.ts, mesh.ts, and webrtc.ts.
export type { SessionInfo, SessionProposal, SessionResponse } from './session';
export { DkgState } from './dkg';
export { MeshStatusType } from './mesh';
export type { MeshStatus } from './mesh';
export type { WebRTCAppMessage } from './webrtc';

// AppState and related utilities are unique to this file.
import type { SessionInfo } from './session';
import type { MeshStatus } from './mesh';
import { MeshStatusType } from './mesh';
import { DkgState } from './dkg';

export interface AppState {
  deviceId: string;
  connecteddevices: string[];
  wsConnected: boolean;
  /** Last WebSocket error; cleared when connection re-establishes. */
  wsError?: string;
  /**
   * Per-session per-device acceptance status. Outer key is
   * session_id, inner key is device_id; values are booleans for
   * "this device has accepted this session invite". Popup renders
   * this as the session-progress roster before DKG starts.
   * Optional because some AppState literals in tests predate this
   * field; callers should `?? {}` or nullish-guard before indexing.
   */
  sessionAcceptanceStatus?: Record<string, Record<string, boolean>>;
  /**
   * Popup-local UI preferences persisted in appState so a popup
   * reopen preserves the user's settings. Shape kept loose so the
   * popup can evolve without a type-round-trip through this file.
   */
  uiPreferences?: {
    darkMode?: boolean;
    language?: string;
    showAdvanced?: boolean;
    [key: string]: any;
  };
  /**
   * Latest account list update — populated when background
   * broadcasts `accountsUpdated`. Popup consumes this to refresh
   * the account picker. Shape is per-blockchain array of Account.
   */
  accountsUpdated?: any;
  sessionInfo: SessionInfo | null;
  invites: SessionInfo[];
  meshStatus: MeshStatus;
  dkgState: DkgState;
  webrtcConnections: Record<string, boolean>;
  blockchain?: "ethereum" | "solana";
  /**
   * FROST ciphersuite selection for the current wallet. Historically
   * tracked alongside `blockchain` for legacy code paths; setters
   * derive one from the other (secp256k1 ↔ ethereum, ed25519 ↔ solana).
   * Writers: StateManager.setBlockchain / setCurve.
   * Readers: StateManager.getCurve / getBlockchain.
   */
  curve?: "secp256k1" | "ed25519";
  /**
   * User-facing "chain" alias for blockchain. Some older code paths
   * persist + read this key; kept as an alias field so both work.
   * New code should prefer `blockchain`.
   */
  chain?: "ethereum" | "solana";

  // --- Popup UI state (persisted in appState so a popup reopen
  // sees the same form / toggle values) ---
  /** Session-proposal form: total participants input. Defaults
   *  to 3 in INITIAL_APP_STATE so callers doing arithmetic on
   *  this (e.g. `totalParticipants - 1`) don't hit NaN. */
  totalParticipants: number;
  /** Session-proposal form: signing threshold input. Defaults to
   *  2 (the 2-of-3 threshold that pairs with totalParticipants=3). */
  threshold: number;
  /** Session-proposal form: user-typed session id (can be blank
   *  → server generates). */
  proposedSessionIdInput?: string;
  /** Settings panel open/closed toggle. */
  showSettings?: boolean;

  // --- DKG completion context (Ext-1d) — stashed by stateManager
  // when offscreen emits `dkgComplete`, consumed by the save-wallet
  // popup flow. Intentionally in-memory only (SW restart clears
  // these; user has to redo DKG). ---
  /** Derived on-chain address from the DKG result. */
  dkgAddress?: string;
  /** Ethereum address derived from a secp256k1 wallet. Stored
   *  separately from dkgAddress so a user with both ethereum and
   *  solana wallets can surface each without overwriting the
   *  other on wallet switch. */
  ethereumAddress?: string;
  /** Solana address derived from an ed25519 wallet. See
   *  ethereumAddress note. */
  solanaAddress?: string;
  /** Last DKG error message (ceremony failed, peer dropped, etc.).
   *  Cleared to "" on new ceremony start; stateManager writes this
   *  from the fetchAndUpdateDkgAddress error path. */
  dkgError?: string;
  /** FROST group public key hex. */
  dkgGroupPublicKey?: string;
  /** Full DKG result snapshot for the save-wallet form. */
  dkgLastResult?: {
    groupPublicKey: string;
    address: string | null;
    blockchain: "ethereum" | "solana";
    sessionId: string | null;
    threshold: number;
    total: number;
    participants: string[];
    participantIndex: number | null;
    completedAt: number;
  };
  /** Raw JSON keystore emitted by WASM `export_keystore`. The
   *  save-wallet handler reads this, decrypts with user password,
   *  builds a KeyShareData, and persists via KeystoreManager. */
  pendingKeystoreJson?: string | null;
  /** Flag the popup watches to know whether to render the save form. */
  pendingKeystoreReady?: boolean;

  // --- Signing ceremony state (Ext-2) ---
  /** Live per-peer roster during an active signing ceremony. */
  signingProgress?: {
    signingId: string;
    state: string;
    selectedSigners: string[];
    commitmentsReceived: string[];
    sharesReceived: string[];
  } | null;
  /** Last aggregated signature produced — drives the
   *  SignatureComplete banner in the popup. */
  lastSignature?: {
    signingId: string;
    signature: string;
    messageHex: string;
    blockchain: "ethereum" | "solana";
    sessionId: string;
    completedAt: number;
  };
}

export const INITIAL_APP_STATE: AppState = {
  deviceId: '',
  connecteddevices: [],
  wsConnected: false,
  sessionInfo: null,
  invites: [],
  meshStatus: { type: MeshStatusType.Incomplete },
  dkgState: DkgState.Idle,
  webrtcConnections: {},
  blockchain: "ethereum",
  // 2-of-3 is the standard threshold-signing default.
  totalParticipants: 3,
  threshold: 2,
};

export type SupportedChain = 'ethereum' | 'solana';

export const CURVE_COMPATIBLE_CHAINS: Record<string, SupportedChain[]> = {
  'secp256k1': ['ethereum'],
  'ed25519': ['solana']
};

export function getCompatibleChains(curveType: string): SupportedChain[] {
  return CURVE_COMPATIBLE_CHAINS[curveType] || [];
}

export function getRequiredCurve(chain: SupportedChain): 'secp256k1' | 'ed25519' {
  for (const [curve, chains] of Object.entries(CURVE_COMPATIBLE_CHAINS)) {
    if (chains.includes(chain)) {
      // CURVE_COMPATIBLE_CHAINS keys are hardcoded 'secp256k1' / 'ed25519';
      // Object.entries widens to `string`, hence the cast.
      return curve as 'secp256k1' | 'ed25519';
    }
  }
  return 'secp256k1';
}
