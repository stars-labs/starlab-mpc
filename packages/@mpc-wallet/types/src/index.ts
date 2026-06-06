// ===================================================================
// TYPES INDEX - CENTRALIZED TYPE EXPORTS
// ===================================================================
//
// This file provides a centralized location to import the most commonly
// used types across the MPC wallet application. This makes it easier
// for developers to find and import types without needing to know
// which specific file contains each type.
//
// Usage Example:
// import { AppState, SessionInfo, DkgState } from '../types';
// 
// Key Concepts for Junior Developers:
// - Index File: A common pattern to re-export types from multiple files
// - Barrel Export: Collecting and re-exporting from a single entry point
// - Type Organization: Grouping related types for easier discovery
// ===================================================================

// Core Application State
export type { AppState, SupportedChain } from './appstate';
export { INITIAL_APP_STATE, CURVE_COMPATIBLE_CHAINS, getCompatibleChains, getRequiredCurve, signingCaveat } from './appstate';
// Note: Constants are exported for components that need them

// Session Management
export type {
    SessionInfo,
    SessionProposal,
    SessionResponse,
    SessionValidation,
    SessionValidator
} from './session';

// DKG (Distributed Key Generation)
export { DkgState } from './dkg';
export type {
    DkgPackageInfo,
    DkgStatus,
    DkgEvent
} from './dkg';

// Mesh Network
export type {
    MeshStatus
} from './mesh';
export { MeshStatusType } from './mesh';

// WebRTC Communication
export type {
    WebRTCAppMessage,
    DataChannelInfo,
    WebRTCConnectionStatus,
    WebRTCEvent
} from './webrtc';

// WebSocket Signaling
export type {
    SDPInfo,
    CandidateInfo,
    WebRTCSignal,
    WebSocketMessagePayload,
    ServerMsg,
    ClientMsg,
    WebSocketEvent
} from './websocket';

// Account Management
export type {
    Account,
    AccountBalance,
    AccountStorage,
    AccountEvent
} from './account';

// Network Configuration
export type {
    NetworkConfig,
    NetworkEvent,
    Chain
} from './network';

// Keystore Types
export type {
    KeyShareData,
    ExtensionWalletMetadata,
    KeystoreIndex,
    EncryptedKeyShare,
    KeystoreBackup,
    CLIKeystoreBackup,
    WalletFile,
    BlockchainInfo,
    NewAccountSession,
    WalletMetadata
} from './keystore';

// Message Types (commonly used for inter-component communication)
export type {
    PopupToBackgroundMessage,
    BackgroundToPopupMessage,
    BackgroundToOffscreenMessage,
    OffscreenToBackgroundMessage,
    BackgroundToOffscreenWrapper,
    OffscreenToBackgroundWrapper,
    InitialStateMessage,
    JsonRpcRequest,
    JsonRpcResponse,
    ContentToBackgroundMsg,
    BackgroundToContentMsg,
    InjectedToContentMsg,
    ContentToInjectedMsg,
    WebSocketClientMsg,
    WebSocketServerMsg,
    OffscreenMessage,
    BackgroundMessage,
    PopupMessage
} from './messages';

// Message validation helpers
export {
    validateMessage,
    validateSessionProposal,
    validateSessionAcceptance,
    isRpcMessage,
    isAccountManagement,
    isNetworkManagement,
    isUIRequest,
    MESSAGE_TYPES
} from './messages';
