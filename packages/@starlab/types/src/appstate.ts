// Re-export domain types so existing imports from '@stars-labs/types/appstate' still work.
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

export type WalletCurve = 'secp256k1' | 'ed25519';
export type ProtocolBlockchain = 'ethereum' | 'solana';
export type EvmChain = 'ethereum' | 'polygon' | 'arbitrum' | 'optimism' | 'base';
export type LimitedChain = 'bitcoin' | 'sui';
export type SupportedChain = EvmChain | 'solana' | LimitedChain;
export type KnownChain = SupportedChain;

export interface ChainMetadata {
  id: KnownChain;
  label: string;
  shortLabel: string;
  family: 'EVM' | 'Solana' | 'Bitcoin' | 'Sui';
  curve: WalletCurve;
  protocolBlockchain: ProtocolBlockchain;
  enabled: boolean;
  status: 'enabled' | 'limited';
  note?: string;
}

export const CHAIN_METADATA: Record<KnownChain, ChainMetadata> = {
  ethereum: {
    id: 'ethereum',
    label: 'Ethereum',
    shortLabel: 'Ethereum',
    family: 'EVM',
    curve: 'secp256k1',
    protocolBlockchain: 'ethereum',
    enabled: true,
    status: 'enabled',
  },
  polygon: {
    id: 'polygon',
    label: 'Polygon',
    shortLabel: 'Polygon',
    family: 'EVM',
    curve: 'secp256k1',
    protocolBlockchain: 'ethereum',
    enabled: true,
    status: 'enabled',
  },
  arbitrum: {
    id: 'arbitrum',
    label: 'Arbitrum',
    shortLabel: 'Arbitrum',
    family: 'EVM',
    curve: 'secp256k1',
    protocolBlockchain: 'ethereum',
    enabled: true,
    status: 'enabled',
  },
  optimism: {
    id: 'optimism',
    label: 'Optimism',
    shortLabel: 'Optimism',
    family: 'EVM',
    curve: 'secp256k1',
    protocolBlockchain: 'ethereum',
    enabled: true,
    status: 'enabled',
  },
  base: {
    id: 'base',
    label: 'Base',
    shortLabel: 'Base',
    family: 'EVM',
    curve: 'secp256k1',
    protocolBlockchain: 'ethereum',
    enabled: true,
    status: 'enabled',
  },
  solana: {
    id: 'solana',
    label: 'Solana',
    shortLabel: 'Solana',
    family: 'Solana',
    curve: 'ed25519',
    protocolBlockchain: 'solana',
    enabled: true,
    status: 'enabled',
  },
  bitcoin: {
    id: 'bitcoin',
    label: 'Bitcoin',
    shortLabel: 'Bitcoin',
    family: 'Bitcoin',
    curve: 'secp256k1',
    protocolBlockchain: 'ethereum',
    enabled: true,
    status: 'limited',
    note: 'Address derivation and threshold signing are available; Bitcoin transaction building/broadcast is not wired yet.',
  },
  sui: {
    id: 'sui',
    label: 'Sui',
    shortLabel: 'Sui',
    family: 'Sui',
    curve: 'ed25519',
    protocolBlockchain: 'solana',
    enabled: true,
    status: 'limited',
    note: 'Address derivation and threshold signing are available; Sui transaction building/broadcast is not wired yet.',
  },
};

export const SUPPORTED_CHAINS: readonly SupportedChain[] = [
  'ethereum',
  'polygon',
  'arbitrum',
  'optimism',
  'base',
  'solana',
  'bitcoin',
  'sui',
] as const;

export const LIMITED_CHAINS: readonly LimitedChain[] = ['bitcoin', 'sui'] as const;
export const EVM_CHAINS: readonly EvmChain[] = [
  'ethereum',
  'polygon',
  'arbitrum',
  'optimism',
  'base',
] as const;

export const CURVE_COMPATIBLE_CHAINS: Record<WalletCurve, SupportedChain[]> = {
  secp256k1: ['ethereum', 'polygon', 'arbitrum', 'optimism', 'base', 'bitcoin'],
  ed25519: ['solana', 'sui'],
};

export function isKnownChain(chain: unknown): chain is KnownChain {
  return (
    typeof chain === 'string' &&
    Object.prototype.hasOwnProperty.call(CHAIN_METADATA, chain)
  );
}

export function isSupportedChain(chain: unknown): chain is SupportedChain {
  return isKnownChain(chain) && CHAIN_METADATA[chain].enabled;
}

export function isEvmChain(chain: unknown): chain is EvmChain {
  return typeof chain === 'string' && (EVM_CHAINS as readonly string[]).includes(chain);
}

export function getChainLabel(chain: string | undefined | null): string {
  return isKnownChain(chain) ? CHAIN_METADATA[chain].label : 'Unknown chain';
}

export function getChainShortLabel(chain: string | undefined | null): string {
  return isKnownChain(chain) ? CHAIN_METADATA[chain].shortLabel : 'Unknown';
}

export function getCompatibleChains(curveType: string): SupportedChain[] {
  return CURVE_COMPATIBLE_CHAINS[curveType as WalletCurve] || [];
}

export function getRequiredCurve(chain: SupportedChain): WalletCurve {
  return CHAIN_METADATA[chain].curve;
}

export function getProtocolBlockchainForChain(chain: SupportedChain): ProtocolBlockchain {
  return CHAIN_METADATA[chain].protocolBlockchain;
}

export function getDefaultChainForCurve(curve: WalletCurve): SupportedChain {
  return curve === 'ed25519' ? 'solana' : 'ethereum';
}

export function normalizeDerivedAddressChain(chain: string | undefined | null): SupportedChain | null {
  const normalized = String(chain ?? '').trim().toLowerCase().replace(/[\s_-]+/g, '');
  switch (normalized) {
    case 'eth':
    case 'ethereum':
      return 'ethereum';
    case 'btc':
    case 'bitcoin':
      return 'bitcoin';
    case 'sol':
    case 'solana':
      return 'solana';
    case 'sui':
      return 'sui';
    case 'polygon':
    case 'matic':
      return 'polygon';
    case 'arbitrum':
    case 'arbitrumone':
      return 'arbitrum';
    case 'optimism':
    case 'op':
      return 'optimism';
    case 'base':
      return 'base';
    default:
      return null;
  }
}

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
  /** True while background is still booting. Popup uses this to
   *  show a loading state before initialState arrives. */
  isInitializing?: boolean;
  /** Global background error surface. Popup shows this as a
   *  full-frame banner when set. */
  globalError?: string;
  /** Flag indicating background bootstrap finished
   *  (keystoreStatus fetched, offscreen ready, websocket connected
   *  or known-down). Popup UI unblocks on this. */
  setupComplete?: boolean;
  sessionInfo: SessionInfo | null;
  invites: SessionInfo[];
  meshStatus: MeshStatus;
  dkgState: DkgState;
  webrtcConnections: Record<string, boolean>;
  blockchain?: ProtocolBlockchain;
  /**
   * FROST ciphersuite selection for the current wallet. Historically
   * tracked alongside `blockchain` for legacy code paths; setters
   * derive one from the other (secp256k1 ↔ ethereum, ed25519 ↔ solana).
   * Writers: StateManager.setBlockchain / setCurve.
   * Readers: StateManager.getCurve / getBlockchain.
   */
  curve?: WalletCurve;
  /**
   * User-facing "chain" alias for blockchain. Some older code paths
   * persist + read this key; kept as an alias field so both work.
   * New code should prefer `blockchain`.
   */
  chain?: SupportedChain;

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
    blockchain: ProtocolBlockchain;
    chain?: SupportedChain;
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
    blockchain: ProtocolBlockchain;
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
