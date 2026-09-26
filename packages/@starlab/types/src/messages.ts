import type { SessionInfo, SessionProposal, SessionResponse } from './session';
import type { DkgState } from './dkg';
import type { MeshStatus } from './mesh';
import type { AppState, ProtocolBlockchain, SupportedChain } from './appstate';
import type { WebRTCAppMessage as DataChannelMessage } from './webrtc';
import type { Account as WalletAccount } from './account';
import { ServerMsg, ClientMsg, WebSocketMessagePayload, WebRTCSignal } from './websocket';

// ===================================================================
// MESSAGE TYPES WITH CLEAR DIRECTION NAMING
// ===================================================================
// 
// This file defines message types with clear directional naming to make
// it obvious for developers which direction the messages flow:
//
// - PopupToBackgroundMessage: Messages sent FROM popup TO background
// - BackgroundToPopupMessage: Messages sent FROM background TO popup  
// - BackgroundToOffscreenMessage: Messages sent FROM background TO offscreen
// - OffscreenToBackgroundMessage: Messages sent FROM offscreen TO background
//
// Wrapper types are used for the actual chrome.runtime.sendMessage calls:
// - BackgroundToOffscreenWrapper: Wraps payload in { type: 'fromBackground', payload: ... }
// - OffscreenToBackgroundWrapper: Wraps payload in { type: 'fromOffscreen', payload: ... }
//
// Legacy type aliases are provided for backward compatibility.
// ===================================================================
// --- Core Message Structure ---
export interface BaseMessage {
    type: string;
    [key: string]: any; // Add index signature for compatibility
}

// --- JSON-RPC Types ---
export interface JsonRpcRequest {
    id: number | string;
    jsonrpc: '2.0';
    method: string;
    params?: unknown;
}

export interface JsonRpcResponse {
    id: number | string;
    jsonrpc: '2.0';
    result?: unknown;
    error?: {
        code: number;
        message: string;
        data?: unknown;
    };
}

// --- Popup to Background Message Types (Popup sends to Background) ---
export type PopupToBackgroundMessage = BaseMessage & (
    // Core wallet operations
    | { type: 'getState' }
    | { type: 'listdevices' }
    | { type: 'sendDirectMessage'; todeviceId: string; message: string }
    | { type: 'getWebRTCStatus' }
    | { type: 'getEthereumAddress' }
    | { type: 'getSolanaAddress' }
    | { type: 'setBlockchain'; blockchain: SupportedChain }

    // session management
    | { type: 'proposeSession'; session_id: string; total: number; threshold: number; participants: string[] }
    | { type: 'acceptSession'; session_id: string; accepted: boolean; blockchain?: ProtocolBlockchain }
    
    // MPC signing operations
    | { type: 'requestSigning'; signingId: string; transactionData: string; requiredSigners: number }
    | { type: 'acceptSigning'; signingId: string; accepted: boolean }
    | { type: 'requestMessageSignature'; message: string; fromAddress: string; origin: string }
    | { type: 'approveMessageSignature'; requestId: string; approved: boolean }

    // Management operations
    | { type: 'createOffscreen' }
    | { type: 'getOffscreenStatus' }
    | { type: 'offscreenReady' }

    // Communication
    | { type: 'relay'; to: string; data: WebSocketMessagePayload }
    | { type: 'fromOffscreen'; payload: OffscreenToBackgroundMessage }

    // RPC operations
    | { type: string; payload: JsonRpcRequest; action?: string; method?: string; params?: unknown[] }
);

// --- Background to Offscreen Message Types (Background sends to Offscreen) ---
export type BackgroundToOffscreenMessage = BaseMessage & (
    | { type: 'getState' }
    | { type: 'sendDirectMessage'; todeviceId: string; message: string }
    | { type: 'getWebRTCStatus' }
    | { type: 'init'; deviceId: string; wsUrl: string }
    | { type: 'relayViaWs'; to: string; data: any }
    | { type: 'sessionAccepted'; sessionInfo: SessionInfo; currentdeviceId: string; blockchain?: ProtocolBlockchain }
    | { type: 'sessionAllAccepted'; sessionInfo: SessionInfo; currentdeviceId: string; blockchain?: ProtocolBlockchain }
    | { type: 'sessionResponseUpdate'; sessionInfo: SessionInfo; currentdeviceId: string }
    | { type: 'getEthereumAddress' }
    | { type: 'getSolanaAddress' }
    | { type: 'getDkgStatus' }
    | { type: 'getGroupPublicKey' }
    | { type: 'setBlockchain'; blockchain: ProtocolBlockchain }
    | { type: 'requestSigning'; signingId: string; transactionData: string; requiredSigners: number }
    | { type: 'requestMessageSignature'; signingId: string; message: string; fromAddress: string }
    | { type: 'requestTransactionSignature'; signingId: string; transactionData: string; fromAddress: string }
    // Ext-2d-offscreen: signing-ceremony trigger. Fires when a
    // session reaches threshold (see webSocketManager.maybeTriggerCeremony);
    // offscreen loads keystore + kicks off FROST round 1.
    | { type: 'sessionReadyForSigning'; sessionInfo: SessionInfo; blockchain?: ProtocolBlockchain }
    // M3 reshare: ceremony trigger. Fires when a reshare session's
    // full NEW cohort has joined (maybeTriggerCeremony); offscreen
    // fetches the wallet's keystore (getWalletKeystore) and runs
    // the FROST reshare rounds over the mesh.
    | { type: 'sessionReadyForReshare'; sessionInfo: SessionInfo; blockchain?: ProtocolBlockchain; walletId: string }
    // Keystore WASM export: background requests offscreen dump the
    // serialized keystore JSON after DKG finalize or import, so it
    // can be encrypted + persisted via KeystoreManager.
    | { type: 'exportKeystore'; chain?: ProtocolBlockchain }
    // Keystore WASM import: background hands offscreen raw keystore
    // JSON (e.g. from CLI .dat conversion) to load into the FROST
    // instance before signing.
    | { type: 'importKeystore'; chain: ProtocolBlockchain; keystoreData: string }
);

// --- Offscreen to Background Message Types (Offscreen sends to Background) ---
export type OffscreenToBackgroundMessage = BaseMessage & (
    | { type: 'webrtcStatusUpdate'; deviceId: string; status: string }
    | { type: 'sessionUpdate'; sessionInfo: SessionInfo | null; invites: SessionInfo[] }
    | { type: 'peerConnectionStatusUpdate'; deviceId: string; connectionState: string }
    | { type: 'dataChannelStatusUpdate'; deviceId: string; channelName: string; state: string }
    | { type: 'webrtcConnectionUpdate'; deviceId: string; connected: boolean }
    | { type: 'meshStatusUpdate'; status: MeshStatus }
    | { type: 'dkgStateUpdate'; state: DkgState }
    | { type: 'relayViaWs'; to: string; data: any }
    | { type: 'webrtcMessage'; fromdeviceId: string; message: any }
    | { type: 'log'; payload: { message: string; source: string } }
    // Ext-1d: DKG ceremony finalized. Offscreen emits after FROST
    // finalize succeeds; stateManager stashes pendingKeystoreJson
    // for the save-wallet flow.
    | {
        type: 'dkgComplete';
        groupPublicKey: string;
        address: string | null;
        blockchain: ProtocolBlockchain;
        sessionId: string | null;
        threshold: number;
        total: number;
        participants: string[];
        participantIndex: number | null;
        keystoreJson: string | null;
      }
    // Ext-2d-progress: per-peer roster snapshot fired on every
    // signing-round milestone (commit sent/received, share
    // sent/received). Wire form of the WebRTCManager signingCommitments
    // and signingShares Maps keyed by peer-id.
    | {
        type: 'signingProgress';
        signingId: string;
        state: string;
        selectedSigners: string[];
        commitmentsReceived: string[];
        sharesReceived: string[];
      }
    // Ext-2d-offscreen-rounds: final aggregated signature. New shape
    // supersedes the legacy dApp-bridge single-party `signingComplete`
    // (which only had {signingId, signature}) by adding the full
    // ceremony context. The extra fields are optional so the type
    // covers both emitters — legacy path omits them, new FROST path
    // fills them.
    | {
        type: 'signingComplete';
        signingId: string;
        signature: string;
        messageHex?: string;
        blockchain?: 'ethereum' | 'solana';
        sessionId?: string;
      }
    | { type: 'signingError'; signingId: string; error: string }
    // M3 reshare: per-round progress. `state` is the ReshareState
    // string name; received counts let the popup render "round N
    // (k/n packages)".
    | {
        type: 'reshareStateUpdate';
        sessionId: string;
        walletId: string;
        state: string;
        round1Received: string[];
        round2Received: string[];
        participants: string[];
        error?: string;
      }
    // M3 reshare: ceremony finalized. Carries the NEW keystore JSON
    // (same group public key, fresh shares) — stateManager replaces
    // the wallet's stored encrypted share with this.
    | {
        type: 'reshareComplete';
        sessionId: string;
        walletId: string;
        newKeystoreJson: string;
        groupPublicKey: string;
        threshold: number;
        total: number;
        participants: string[];
        participantIndex: number | null;
        blockchain: ProtocolBlockchain;
      }
    | { type: 'messageSignatureComplete'; signingId: string; signature: string }
    | { type: 'messageSignatureError'; signingId: string; error: string }
);

// Add the missing InitialStateMessage type
export interface InitialStateMessage extends BaseMessage, AppState {
    type: 'initialState';
    deviceId: string;
    connecteddevices: string[];
    wsConnected: boolean;
    sessionInfo: SessionInfo | null;
    invites: SessionInfo[];
    meshStatus: { type: number };
    dkgState: number;
    webrtcConnections: Record<string, boolean>; // Add WebRTC connection state
}

// --- Background to Popup Message Types (Background sends to Popup) ---
export type BackgroundToPopupMessage =
    | InitialStateMessage
    | { type: "wsStatus"; connected: boolean } & BaseMessage
    | { type: "wsError"; error: string } & BaseMessage
    | { type: "wsMessage"; message: any } & BaseMessage
    | { type: "deviceList"; devices: string[] } & BaseMessage
    | { type: "sessionUpdate"; sessionInfo: SessionInfo | null; invites: SessionInfo[] } & BaseMessage
    | { type: "webrtcConnectionUpdate"; deviceId: string; connected: boolean } & BaseMessage
    | { type: "webrtcStatusUpdate"; deviceId: string; status: string } & BaseMessage
    | { type: "meshStatusUpdate"; status: MeshStatus } & BaseMessage
    | { type: "dkgStateUpdate"; state: DkgState } & BaseMessage
    | { type: "fromOffscreen"; payload: any } & BaseMessage
    | { type: "signatureRequest"; signingId: string; message: string; origin: string; fromAddress: string } & BaseMessage
    | { type: "signatureComplete"; signingId: string; signature: string } & BaseMessage
    | { type: "signatureError"; signingId: string; error: string } & BaseMessage
    | { type: "transactionRequest"; signingId: string; transaction: any; origin: string; fromAddress: string } & BaseMessage
    // Ext-1d: account list changed (wallet saved / removed). Popup
    // re-fetches the relevant blockchain's accounts to refresh the
    // picker. Emitted by completeAccountCreation and similar flows.
    | { type: "accountsUpdated"; blockchain: ProtocolBlockchain; accounts: WalletAccount[] } & BaseMessage;

// --- Wrapper Message Types for Communication Direction ---
export type BackgroundToOffscreenWrapper = {
    type: 'fromBackground';
    payload: BackgroundToOffscreenMessage;
};

export type OffscreenToBackgroundWrapper = {
    type: 'fromOffscreen';
    payload: OffscreenToBackgroundMessage;
};

// --- Legacy Type Aliases (for backward compatibility) ---
/**
 * @deprecated Use PopupToBackgroundMessage instead
 */
export type BackgroundMessage = PopupToBackgroundMessage;

/**
 * @deprecated Use BackgroundToPopupMessage instead
 */
export type PopupMessage = BackgroundToPopupMessage;

/**
 * @deprecated Use BackgroundToOffscreenMessage instead
 */
export type OffscreenMessage = BackgroundToOffscreenMessage;

// --- Legacy Support Types (kept for compatibility) ---
export type ContentToInjectedMsg = BaseMessage;
export type InjectedToContentMsg = BaseMessage;
export type ContentToBackgroundMsg = BaseMessage;
export type BackgroundToContentMsg = BaseMessage;
/**
 * @deprecated Use PopupToBackgroundMessage instead
 */
export type PopupToBackgroundMsg = PopupToBackgroundMessage;
/**
 * @deprecated Use BackgroundToPopupMessage instead
 */
export type BackgroundToPopupMsg = BackgroundToPopupMessage;
/**
 * @deprecated Use BackgroundToOffscreenWrapper instead
 */
export type BackgroundToOffscreenMsg = BackgroundToOffscreenWrapper;
/**
 * @deprecated Use OffscreenToBackgroundWrapper instead
 */
export type OffscreenToBackgroundMsg = OffscreenToBackgroundWrapper;
export type WebSocketClientMsg = BaseMessage;
export type WebSocketServerMsg = BaseMessage;
export type AnyMessage = BaseMessage;
export function isRpcMessage(msg: PopupToBackgroundMessage): msg is PopupToBackgroundMessage & { payload: JsonRpcRequest } {
    return 'payload' in msg && typeof msg.payload === 'object' && msg.payload !== null && 'jsonrpc' in msg.payload;
}

export function isAccountManagement(msg: PopupToBackgroundMessage): boolean {
    return msg.type === 'ACCOUNT_MANAGEMENT';
}

export function isNetworkManagement(msg: PopupToBackgroundMessage): boolean {
    return msg.type === 'NETWORK_MANAGEMENT';
}

export function isUIRequest(msg: PopupToBackgroundMessage): msg is PopupToBackgroundMessage & { payload: { method: string; params: unknown[] } } {
    return msg.type === 'UI_REQUEST' && 'payload' in msg && typeof msg.payload === 'object' && msg.payload !== null && 'method' in msg.payload;
}

// --- Validation Helpers ---
export function validateMessage(msg: unknown): msg is PopupToBackgroundMessage {
    return typeof msg === 'object' && msg !== null && 'type' in msg && typeof (msg as any).type === 'string';
}

export function validateSessionProposal(msg: PopupToBackgroundMessage): msg is PopupToBackgroundMessage & { session_id: string; total: number; threshold: number; participants: string[] } {
    return msg.type === 'proposeSession' &&
        'session_id' in msg && typeof msg.session_id === 'string' &&
        'total' in msg && typeof msg.total === 'number' &&
        'threshold' in msg && typeof msg.threshold === 'number' &&
        'participants' in msg && Array.isArray(msg.participants);
}

export function validateSessionAcceptance(msg: PopupToBackgroundMessage): msg is PopupToBackgroundMessage & { session_id: string; accepted: boolean; blockchain?: ProtocolBlockchain } {
    return msg.type === 'acceptSession' &&
        'session_id' in msg && typeof msg.session_id === 'string' &&
        'accepted' in msg && typeof msg.accepted === 'boolean' &&
        (!('blockchain' in msg) || (typeof msg.blockchain === 'string' && ['ethereum', 'solana'].includes(msg.blockchain)));
}

// --- Legacy Types (kept for compatibility) ---
export type Account = { address: string;[key: string]: unknown };
export type Network = { id: number | string; name?: string;[key: string]: unknown };

// --- Message Constants ---
export const MESSAGE_TYPES = {
    GET_STATE: "getState",
    LIST_DEVICES: "listDevices",
    PROPOSE_SESSION: "proposeSession",
    ACCEPT_SESSION: "acceptSession",
    // Ext-1b: Create a new MPC wallet via DKG. Unlike PROPOSE_SESSION
    // (legacy per-peer relay path), this emits a TUI-compatible
    // `announce_session` broadcast so any client — extension or TUI —
    // can discover and join. The payload carries config only (name,
    // total, threshold, curve); background generates the session_id
    // and device_id.
    CREATE_DKG_WALLET: "createDkgWallet",
    // Ext-1e: popup → background message for "I want to join this
    // discovered DKG session". The session_id is the sole payload;
    // background looks it up in appState.invites, sets local
    // sessionInfo, and emits `session_status_update` so the creator
    // and server learn we've joined.
    JOIN_DKG_SESSION: "joinDkgSession",
    // Ext-1d: popup → background "DKG finished, here's the password
    // to encrypt + save the keyshare". Background reads
    // appState.pendingKeystoreJson + dkgLastResult to build a
    // KeyShareData and calls KeystoreManager.addWallet. If no
    // keystore exists yet, create one with this password. Payload
    // is {password, walletName?}.
    SAVE_DKG_WALLET: "saveDkgWallet",
    // Ext-2a/b: initiate a threshold signing ceremony. Mirror of
    // CREATE_DKG_WALLET but for signing: builds session_info with
    // session_type="signing", wallet_name, group_public_key, and
    // signing_message_hex, then announces. Payload:
    // {walletId, message, curve}.
    CREATE_SIGNING_SESSION: "createSigningSession",
    // Ext-3c: popup → background "I'm declining this signing
    // session invite". Background relays a SigningDecline via
    // the signal server to the proposer so they see a toast
    // (not a silent timeout). Payload: {session_id}.
    DECLINE_SIGNING_SESSION: "declineSigningSession",
    // M3 reshare: rotate / redistribute an existing wallet's key
    // shares without changing the group public key. Mirror of
    // CREATE_SIGNING_SESSION on the announce side: builds
    // session_info with session_type="reshare", wallet_name,
    // group_public_key, blockchain, and reshare_old_participants,
    // then announces. Payload:
    // {walletId, newThreshold?, removeDeviceIds?}.
    CREATE_RESHARE_SESSION: "createReshareSession",
    // M3 reshare: popup → background "what cohort does wallet X
    // have?" — returns {threshold, total, participants, deviceId,
    // groupPublicKey, curve} from the decrypted key share (keystore
    // must be unlocked). Payload: {walletId}.
    GET_WALLET_COHORT: "getWalletCohort",
    RELAY: "relay",
    FROM_OFFSCREEN: "fromOffscreen",
    OFFSCREEN_READY: "offscreenReady",
    CREATE_OFFSCREEN: "createOffscreen",
    GET_OFFSCREEN_STATUS: "getOffscreenStatus",
    GET_WEBRTC_STATE: "getWebRTCState",
    SEND_DIRECT_MESSAGE: "sendDirectMessage",
    GET_WEBRTC_STATUS: "getWebRTCStatus",
    WEBRTC_STATUS_UPDATE: "webrtcStatusUpdate",
    SESSION_UPDATE: "sessionUpdate",
    PEER_CONNECTION_STATUS_UPDATE: "peerConnectionStatusUpdate",
    DATA_CHANNEL_STATUS_UPDATE: "dataChannelStatusUpdate",
    GET_ETHEREUM_ADDRESS: "getEthereumAddress",
    GET_SOLANA_ADDRESS: "getSolanaAddress",
    SET_BLOCKCHAIN: "setBlockchain",
    REQUEST_SIGNING: "requestSigning",
    ACCEPT_SIGNING: "acceptSigning",
    SIGNING_COMPLETE: "signingComplete",
    SIGNING_ERROR: "signingError",
    // Keystore management
    UNLOCK_KEYSTORE: "unlockKeystore",
    LOCK_KEYSTORE: "lockKeystore",
    CREATE_KEYSTORE: "createKeystore",
    GET_KEYSTORE_STATUS: "getKeystoreStatus",
    SWITCH_WALLET: "switchWallet",
    MIGRATE_KEYSTORES: "migrateKeystores",
    // Legacy support
    ACCOUNT_MANAGEMENT: "ACCOUNT_MANAGEMENT",
    NETWORK_MANAGEMENT: "NETWORK_MANAGEMENT",
    UI_REQUEST: "UI_REQUEST",
} as const;
