// ===================================================================
// STATE MANAGEMENT MODULE
// ===================================================================
//
// This module manages the central application state and provides
// utilities for state synchronization across different components.
// It handles:
// - Application state management
// - Popup port communication
// - State broadcasting and updates
// - Cross-component state consistency
// ===================================================================

import { AppState, INITIAL_APP_STATE } from "@mpc-wallet/types/appstate";
import { MeshStatusType } from "@mpc-wallet/types/mesh";
import { DkgState } from "@mpc-wallet/types/dkg";
import type {
    BackgroundToPopupMessage,
    InitialStateMessage,
    OffscreenToBackgroundMessage
} from "@mpc-wallet/types/messages";

/** Listener fired whenever `appState.dkgState` changes. Registered
 *  via `addDkgStateListener`. Used by the keepalive controller to
 *  start/stop the offscreen ping timer; also available for any
 *  future subscriber (e.g. logging, metrics). */
export type DkgStateListener = (state: DkgState) => void;

/**
 * Manages central application state and popup communication
 */
export class StateManager {
    private appState: AppState;
    private popupPorts = new Set<chrome.runtime.Port>();
    private static readonly STATE_STORAGE_KEY = 'mpc_wallet_background_state';
    private isStateLoaded = false;
    private pendingPopupPorts: chrome.runtime.Port[] = [];
    private rpcHandler?: any; // Will be set after initialization
    /** External observers of dkgState transitions. Keepalive uses
     *  this; see `keepaliveController.ts`. */
    private dkgStateListeners: Set<DkgStateListener> = new Set();
    /** Last observed dkgState, used to dedupe same-value emissions
     *  so listeners don't see spurious "change" events when we
     *  broadcast full state for unrelated reasons. */
    private lastBroadcastDkgState: DkgState = DkgState.Idle;

    constructor(initialState?: Partial<AppState>) {
        this.appState = {
            ...INITIAL_APP_STATE,
            ...initialState
        };
        console.log("[StateManager] Constructor - starting async state loading...");
        // Load persisted state asynchronously
        this.loadPersistedState();
    }

    /**
     * Load persisted state asynchronously from Chrome storage
     */
    private async loadPersistedState(): Promise<void> {
        console.log("[StateManager] Loading persisted state from Chrome storage...");
        try {
            const result = await chrome.storage.local.get(StateManager.STATE_STORAGE_KEY);
            if (result[StateManager.STATE_STORAGE_KEY]) {
                const persistedState = result[StateManager.STATE_STORAGE_KEY];
                console.log("[StateManager] Loading persisted state:", persistedState);

                // Merge persisted state with current state, preserving important runtime values
                this.appState = {
                    ...this.appState,
                    ...persistedState,
                    // Reset transient connection states that shouldn't persist
                    wsConnected: false,
                    meshStatus: { type: MeshStatusType.Incomplete },
                    webrtcConnections: {},
                    // Architectural reminder #1: session data is
                    // intentionally ephemeral (see `persistState`
                    // line 117 comment). Belt-and-suspenders forces
                    // a reset even if a prior code version or manual
                    // chrome.storage write left session fields in
                    // persisted storage. Without this, a SW that
                    // woke up from a killed ceremony could start in
                    // a mid-round dkgState with no offscreen doc or
                    // WebRTC mesh to back it up — silent zombie
                    // state. Forcing Idle here makes the recovery
                    // boundary sharp: session data never survives
                    // SW restart, period.
                    dkgState: DkgState.Idle,
                    sessionInfo: null,
                    invites: [],
                };
                // `dkgAddress` / `dkgError` are off-type properties
                // this module writes via bracket access (see fetchAndUpdateDkgAddress).
                // Untyped cast so defensive cleanup compiles.
                (this.appState as any).dkgAddress = "";
                (this.appState as any).dkgError = "";
                // Ext-1d: pending keystore JSON holds the decrypted
                // key package in memory only. On a SW restart we
                // can't recover it — force-reset so a "dangling
                // pendingKeystoreReady" flag doesn't lie to the UI.
                (this.appState as any).pendingKeystoreJson = null;
                (this.appState as any).pendingKeystoreReady = false;
                console.log("[StateManager] State restored from persistence");
            } else {
                console.log("[StateManager] No persisted state found, using initial state");
            }
        } catch (error) {
            console.warn("[StateManager] Failed to load persisted state:", error);
        } finally {
            // Mark state as loaded and process any pending popup connections
            console.log("[StateManager] State loading complete, processing pending popup connections...");
            this.isStateLoaded = true;
            this.processPendingPopupPorts();
        }
    }

    /**
     * Process any popup ports that connected before state was loaded
     */
    private processPendingPopupPorts(): void {
        console.log(`[StateManager] Processing ${this.pendingPopupPorts.length} pending popup ports`);

        // Process each pending port
        this.pendingPopupPorts.forEach((port, index) => {
            try {
                console.log(`[StateManager] Processing pending popup port ${index + 1}/${this.pendingPopupPorts.length}`);
                this.addPopupPortInternal(port);
            } catch (error) {
                console.error(`[StateManager] Error processing pending popup port ${index + 1}:`, error);
            }
        });

        // Clear the pending ports array
        this.pendingPopupPorts = [];
        console.log("[StateManager] All pending popup ports processed");
    }

    /**
     * Set the RPC handler for signature callbacks
     */
    setRpcHandler(handler: any): void {
        this.rpcHandler = handler;
        console.log("[StateManager] RPC handler set");
    }

    /**
     * Subscribe to dkgState transitions. Fires each time the state
     * actually changes (no duplicate emissions for broadcasts that
     * don't change dkgState). Returns an unsubscribe function.
     *
     * Primary consumer: `KeepaliveController` — it needs to toggle
     * offscreen-ping cadence whenever a ceremony begins or ends.
     */
    addDkgStateListener(listener: DkgStateListener): () => void {
        this.dkgStateListeners.add(listener);
        // Fire immediately with current state so late subscribers
        // don't miss an in-progress ceremony.
        try {
            listener(this.appState.dkgState);
        } catch (err) {
            console.warn(
                "[StateManager] initial dkgState listener invocation threw:",
                err,
            );
        }
        return () => this.dkgStateListeners.delete(listener);
    }

    /**
     * Internal helper: called from every site that mutates
     * `appState.dkgState`. Compares against last broadcast and fires
     * listeners only on an actual transition. Safe to call even if
     * nothing changed (it'll short-circuit).
     */
    private notifyDkgStateListenersIfChanged(): void {
        const current = this.appState.dkgState;
        if (current === this.lastBroadcastDkgState) return;
        this.lastBroadcastDkgState = current;
        for (const listener of this.dkgStateListeners) {
            try {
                listener(current);
            } catch (err) {
                console.warn(
                    "[StateManager] dkgState listener threw:",
                    err,
                );
            }
        }
    }

    /**
     * Persist state to Chrome storage
     */
    private async persistState(): Promise<void> {
        try {
            // Only persist UI preferences and device info, NOT session data
            const stateToPersist = {
                deviceId: this.appState.deviceId,
                chain: this.appState.chain,
                curve: this.appState.curve,
                // Don't persist: sessionInfo, invites, dkgState, dkgAddress, threshold, totalParticipants
                // Don't persist: wsConnected, meshStatus, webrtcConnections, connecteddevices
            };

            await chrome.storage.local.set({
                [StateManager.STATE_STORAGE_KEY]: stateToPersist
            });
            console.log("[StateManager] State persisted to storage");
        } catch (error) {
            console.warn("[StateManager] Failed to persist state:", error);
        }
    }

    /**
     * Get current application state
     */
    getState(): AppState {
        return { ...this.appState };
    }

    /**
     * Update application state
     */
    updateState(updates: Partial<AppState>): void {
//         console.log("[StateManager] Updating state:", updates);
        this.appState = {
            ...this.appState,
            ...updates
        };
        // Persist state changes
        this.persistState();
        // Fire dkgState listeners if the patch touched dkgState.
        // Call the helper unconditionally — it short-circuits when
        // the value didn't actually change, so it's cheap.
        this.notifyDkgStateListenersIfChanged();
    }

    /**
     * Update specific state properties with deep merge support
     */
    updateStateProperty<K extends keyof AppState>(key: K, value: AppState[K]): void {
//         console.log(`[StateManager] Updating state property ${String(key)}:`, value);
        this.appState[key] = value;
        // Persist state changes
        this.persistState();

        // Broadcast the specific update based on the property
        if (key === 'invites' || key === 'sessionInfo') {
            this.broadcastToPopupPorts({
                type: "sessionUpdate",
                sessionInfo: this.appState.sessionInfo,
                invites: this.appState.invites
            } as any);
        } else {
            // For other properties, broadcast the full state
            this.broadcastCurrentState();
        }

        // If the caller mutated dkgState via this single-property
        // setter, wake any subscribers (same semantics as updateState).
        if (key === "dkgState") {
            this.notifyDkgStateListenersIfChanged();
        }
    }

    /**
     * Update WebSocket connection status and persist it
     */
    updateWebSocketStatus(connected: boolean, error?: string): void {
        console.log(`[StateManager] Updating WebSocket status: connected=${connected}, error=${error || 'none'}`);
        this.appState.wsConnected = connected;
        if (error) {
            this.appState.wsError = error;
        } else {
            this.appState.wsError = "";
        }

        // Broadcast status update
        this.broadcastToPopupPorts({ type: "wsStatus", connected });

        // Persist the WebSocket status update
        this.persistState();
    }

    /**
     * Update connected devices list and persist it
     */
    updateConnectedDevices(devices: string[]): void {
        console.log(`[StateManager] Updating connected devices:`, devices);
        
        // Validate device ID exists before filtering
        if (!this.appState.deviceId) {
            console.warn("[StateManager] No device ID set, cannot filter connected devices properly");
            return;
        }
        
        // Exclude current device from connected devices list
        const filteredDevices = devices.filter(deviceId => deviceId !== this.appState.deviceId);
        
        // Only update if the list has actually changed
        const devicesChanged = JSON.stringify(filteredDevices) !== JSON.stringify(this.appState.connecteddevices);
        
        if (devicesChanged) {
            this.appState.connecteddevices = filteredDevices;
            console.log(`[StateManager] Connected devices changed:`, this.appState.connecteddevices);
            
            // Broadcast device list update
            this.broadcastToPopupPorts({
                type: "deviceList",
                devices: this.appState.connecteddevices
            });
            
            // Persist the devices update
            this.persistState();
        } else {
            console.log(`[StateManager] Connected devices unchanged:`, this.appState.connecteddevices);
        }
    }

    /**
     * Add a popup port connection
     */
    addPopupPort(port: chrome.runtime.Port): void {
        console.log("[StateManager] Adding popup port, state loaded:", this.isStateLoaded, "pending ports:", this.pendingPopupPorts.length);

        if (!this.isStateLoaded) {
            // State not loaded yet, queue the port for later
            console.log("[StateManager] State not loaded yet, queuing popup port");
            this.pendingPopupPorts.push(port);

            // Set up disconnect handler for queued ports to prevent memory leaks
            port.onDisconnect.addListener(() => {
                console.log("[StateManager] Queued popup port disconnected, removing from pending list");
                const index = this.pendingPopupPorts.indexOf(port);
                if (index > -1) {
                    this.pendingPopupPorts.splice(index, 1);
                }
            });
            return;
        }

        this.addPopupPortInternal(port);
    }

    /**
     * Internal method to add popup port once state is loaded
     */
    private addPopupPortInternal(port: chrome.runtime.Port): void {
        console.log("[StateManager] Adding popup port (internal)");
        this.popupPorts.add(port);

        // Send current state to newly connected popup
        const initialStateMessage: InitialStateMessage = {
            type: "initialState",
            ...this.appState
        };
        console.log("[StateManager] Sending current state to popup:", {
            deviceId: this.appState.deviceId,
            wsConnected: this.appState.wsConnected,
            connecteddevices: this.appState.connecteddevices?.length || 0,
            sessionInfo: !!this.appState.sessionInfo,
            dkgState: this.appState.dkgState,
            dkgAddress: this.appState.dkgAddress
        });
        port.postMessage(initialStateMessage);

        port.onDisconnect.addListener(() => {
            console.log("[StateManager] Popup disconnected");
            this.popupPorts.delete(port);
        });
    }

    /**
     * Broadcast message to all connected popup ports
     */
    broadcastToPopupPorts(message: BackgroundToPopupMessage): void {
        console.log("[StateManager] Broadcasting to", this.popupPorts.size, "popup ports:", message);
        this.popupPorts.forEach(port => {
            try {
                port.postMessage(message);
                console.log("[StateManager] Successfully sent message to popup port");
            } catch (error) {
                console.error("[StateManager] Error sending message to popup port:", error);
                this.popupPorts.delete(port);
            }
        });
    }

    /**
     * Broadcast current state to all popup ports
     */
    broadcastCurrentState(): void {
        const stateMessage: InitialStateMessage = {
            type: "initialState",
            ...this.appState
        };
        this.broadcastToPopupPorts(stateMessage as any);
    }

    /**
     * Handle state updates from offscreen document
     */
    handleOffscreenStateUpdate(payload: OffscreenToBackgroundMessage): void {
//         console.log("[StateManager] Handling offscreen state update:", payload);

        switch (payload.type) {
            case "webrtcConnectionUpdate":
                if ('deviceId' in payload && 'connected' in payload) {
                    console.log("[StateManager] Received WebRTC connection update:", {
                        deviceId: payload.deviceId,
                        connected: payload.connected
                    });

                    // Update appState with WebRTC connection info
                    this.appState.webrtcConnections[payload.deviceId] = payload.connected;
                    console.log("[StateManager] Updated appState.webrtcConnections:", this.appState.webrtcConnections);

                    // Check if we should transition from KeystoreImported to Complete
                    if (this.appState.dkgState === DkgState.KeystoreImported && this.appState.sessionInfo) {
                        // For imported keystores, we need at least threshold participants connected
                        const connectedParticipants = Object.keys(this.appState.webrtcConnections)
                            .filter(peer => this.appState.webrtcConnections[peer] === true).length + 1; // +1 for self
                        
                        console.log(`[StateManager] WebRTC update - checking keystore imported transition: connected=${connectedParticipants}, threshold=${this.appState.sessionInfo.threshold}`);
                        
                        if (connectedParticipants >= this.appState.sessionInfo.threshold) {
                            console.log("[StateManager] Transitioning from KeystoreImported to Complete - enough participants connected via WebRTC");
                            this.appState.dkgState = DkgState.Complete;
                            this.notifyDkgStateListenersIfChanged();
                            
                            // Broadcast DKG state update
                            this.broadcastToPopupPorts({
                                type: "dkgStateUpdate",
                                state: DkgState.Complete
                            } as any);
                            
                            // Auto-fetch DKG address when transitioning to Complete
                            this.fetchAndUpdateDkgAddress();
                        }
                    }

                    // Send WebRTC connection update directly to popup
                    const webrtcMessage = {
                        type: "webrtcConnectionUpdate",
                        deviceId: payload.deviceId,
                        connected: payload.connected
                    };

//                     console.log("[StateManager] Sending WebRTC connection update to popup:", webrtcMessage);
                    this.broadcastToPopupPorts(webrtcMessage as any);
                } else {
                    console.warn("[StateManager] Invalid WebRTC connection update payload:", payload);
                }
                break;

            case "meshStatusUpdate":
//                 console.log("[StateManager] Received mesh status update from offscreen:", payload);
                this.appState.meshStatus = payload.status || { type: MeshStatusType.Incomplete };

                // Check if we should transition from KeystoreImported to Complete
                if (this.appState.dkgState === DkgState.KeystoreImported && 
                    this.appState.meshStatus.type === MeshStatusType.Ready &&
                    this.appState.sessionInfo) {
                    
                    // For imported keystores, we need at least threshold participants connected
                    const connectedParticipants = Object.keys(this.appState.webrtcConnections)
                        .filter(peer => this.appState.webrtcConnections[peer] === true).length + 1; // +1 for self
                    
                    console.log(`[StateManager] Checking keystore imported transition: connected=${connectedParticipants}, threshold=${this.appState.sessionInfo.threshold}`);
                    
                    if (connectedParticipants >= this.appState.sessionInfo.threshold) {
                        console.log("[StateManager] Transitioning from KeystoreImported to Complete - enough participants connected");
                        this.appState.dkgState = DkgState.Complete;
                        this.notifyDkgStateListenersIfChanged();

                        // Broadcast DKG state update
                        this.broadcastToPopupPorts({
                            type: "dkgStateUpdate",
                            state: DkgState.Complete
                        } as any);

                        // Auto-fetch DKG address when transitioning to Complete
                        this.fetchAndUpdateDkgAddress();
                    }
                }

                // No persistence - sessions are ephemeral

                // Broadcast mesh status update directly to popup
                this.broadcastToPopupPorts({
                    type: "meshStatusUpdate",
                    status: this.appState.meshStatus
                } as any);
                break;

            case "dkgStateUpdate":
//                 console.log("[StateManager] Received DKG state update from offscreen:", payload);
                this.appState.dkgState = payload.state || DkgState.Idle;
                // Fire keepalive / other subscribers; offscreen is
                // the authoritative source of DKG state transitions,
                // so these events are the primary trigger for
                // Round1InProgress → Round2InProgress → Finalizing.
                this.notifyDkgStateListenersIfChanged();

                // No persistence - sessions are ephemeral

                // Auto-fetch DKG address when DKG completes (business logic moved from popup)
                if (this.appState.dkgState === DkgState.Complete && this.appState.sessionInfo) {
                    console.log("[StateManager] DKG completed, auto-fetching DKG address");
                    this.fetchAndUpdateDkgAddress();
                }

                // Broadcast DKG state update directly to popup
                this.broadcastToPopupPorts({
                    type: "dkgStateUpdate",
                    state: this.appState.dkgState
                } as any);
                break;

            case "dkgComplete":
                // Ext-1d: the offscreen has run FROST finalize. Stash
                // the completion snapshot PLUS the raw keystore JSON
                // the WASM emitted — this is what the save-wallet
                // handler later consumes to build the KeyShareData
                // and call KeystoreManager.addWallet.
                //
                // Security note: pendingKeystoreJson holds the
                // decrypted key package material (base64 inside
                // JSON). It lives ONLY in in-memory appState; never
                // written to chrome.storage.local. A SW restart
                // zeroes it — which is correct: the user would need
                // to redo DKG. Architectural reminder #1's reset
                // block explicitly re-includes this in the ephemeral
                // set.
                {
                    const info: any = payload;
                    (this.appState as any).dkgAddress = info.address ?? "";
                    (this.appState as any).dkgGroupPublicKey =
                        info.groupPublicKey ?? "";
                    (this.appState as any).dkgLastResult = {
                        groupPublicKey: info.groupPublicKey,
                        address: info.address,
                        blockchain: info.blockchain,
                        sessionId: info.sessionId,
                        threshold: info.threshold,
                        total: info.total,
                        participants: info.participants,
                        participantIndex: info.participantIndex,
                        completedAt: Date.now(),
                    };
                    (this.appState as any).pendingKeystoreJson =
                        info.keystoreJson ?? null;
                    (this.appState as any).pendingKeystoreReady =
                        typeof info.keystoreJson === "string" &&
                        info.keystoreJson.length > 0;
                    console.log(
                        "[StateManager] DKG complete received:",
                        (this.appState as any).dkgLastResult,
                        `(keystore JSON ${info.keystoreJson ? "PRESENT" : "MISSING"})`,
                    );
                    this.broadcastToPopupPorts({
                        type: "dkgCompleted",
                        ...info,
                    } as any);
                    this.broadcastCurrentState();
                }
                break;

            case "signingProgress":
                // Ext-2d-progress: per-peer roster snapshot.
                // Stashed in appState so late popup mounts can
                // read current state without waiting for the next
                // event; broadcast too for live reactivity on
                // already-open popups.
                {
                    const info: any = payload;
                    (this.appState as any).signingProgress = {
                        signingId: info.signingId,
                        state: info.state,
                        selectedSigners: info.selectedSigners ?? [],
                        commitmentsReceived: info.commitmentsReceived ?? [],
                        sharesReceived: info.sharesReceived ?? [],
                    };
                    this.broadcastToPopupPorts({
                        type: "signingProgress",
                        ...info,
                    } as any);
                }
                break;

            case "signingComplete":
                // Ext-2d-offscreen-rounds: the FROST signing ceremony
                // finalized. Stash the signature + metadata in appState
                // so the popup can render a SignatureComplete banner,
                // and clear the in-progress session so the user can
                // start a new ceremony. Kept in in-memory appState only
                // (same reasoning as pendingKeystoreJson — a SW restart
                // intentionally zeroes it; signatures are relevant only
                // to the session where they were produced).
                {
                    const info: any = payload;
                    (this.appState as any).lastSignature = {
                        signingId: info.signingId,
                        signature: info.signature,
                        messageHex: info.messageHex,
                        blockchain: info.blockchain,
                        sessionId: info.sessionId,
                        completedAt: Date.now(),
                    };
                    // Clear the active signing session so "+ Sign" is
                    // available again. The invite is left in
                    // `appState.invites` for now (it'll age out via
                    // session_removed from the server or get cleaned
                    // up on next cold start).
                    if (
                        this.appState.sessionInfo?.session_type === "signing" &&
                        this.appState.sessionInfo?.session_id === info.sessionId
                    ) {
                        this.appState.sessionInfo = null;
                        this.appState.dkgState = DkgState.Idle;
                    }
                    // Clear the live progress roster now that the
                    // ceremony has finished — the success banner
                    // supersedes the per-peer check marks.
                    (this.appState as any).signingProgress = null;
                    console.log(
                        "[StateManager] Signing complete received:",
                        (this.appState as any).lastSignature,
                    );
                    this.broadcastToPopupPorts({
                        type: "signingCompleted",
                        ...info,
                    } as any);
                    this.broadcastCurrentState();
                }
                break;

            case "sessionUpdate":
                if ('sessionInfo' in payload && 'invites' in payload) {
//                     console.log("[StateManager] Received session update from offscreen:", payload);

                    this.appState.sessionInfo = payload.sessionInfo || null;
                    this.appState.invites = payload.invites || [];

                    // Broadcast session update to popup
                    this.broadcastToPopupPorts({
                        type: "sessionUpdate",
                        sessionInfo: this.appState.sessionInfo,
                        invites: this.appState.invites
                    } as any);
                }
                break;

            case "webrtcStatusUpdate":
                if ('deviceId' in payload && 'status' in payload) {
//                     console.log(`[StateManager] WebRTC status update for ${payload.deviceId}: ${payload.status}`);
                    // Forward to popup if needed
                    this.broadcastToPopupPorts({
                        type: "webrtcStatusUpdate",
                        deviceId: payload.deviceId,
                        status: payload.status
                    } as any);
                }
                break;

            case "peerConnectionStatusUpdate":
                if ('deviceId' in payload && 'connectionState' in payload) {
//                     console.log(`[StateManager] Peer connection status update for ${payload.deviceId}: ${payload.connectionState}`);
                }
                break;

            case "dataChannelStatusUpdate":
                if ('deviceId' in payload && 'channelName' in payload && 'state' in payload) {
//                     console.log(`[StateManager] Data channel ${payload.channelName} for ${payload.deviceId}: ${payload.state}`);
                }
                break;

            case "messageSignatureComplete":
                if ('signingId' in payload && 'signature' in payload) {
                    console.log(`[StateManager] Message signature complete for ${payload.signingId}`);
                    // Forward to RPC handler
                    if (this.rpcHandler && typeof this.rpcHandler.handleSignatureComplete === 'function') {
                        this.rpcHandler.handleSignatureComplete(payload.signingId, payload.signature);
                    }
                    // Also broadcast to popup for UI updates
                    this.broadcastToPopupPorts({
                        type: "signatureComplete",
                        signingId: payload.signingId,
                        signature: payload.signature
                    } as any);
                }
                break;

            case "messageSignatureError":
                if ('signingId' in payload && 'error' in payload) {
                    console.log(`[StateManager] Message signature error for ${payload.signingId}: ${payload.error}`);
                    // Forward to RPC handler
                    if (this.rpcHandler && typeof this.rpcHandler.handleSignatureError === 'function') {
                        this.rpcHandler.handleSignatureError(payload.signingId, payload.error);
                    }
                    // Also broadcast to popup for UI updates
                    this.broadcastToPopupPorts({
                        type: "signatureError",
                        signingId: payload.signingId,
                        error: payload.error
                    } as any);
                }
                break;

            default:
//                 console.log("[StateManager] Forwarding unknown message to popup:", payload);
                this.broadcastToPopupPorts({
                    type: "fromOffscreen",
                    payload
                } as any);
                break;
        }
    }

    /**
     * Update session information
     */
    updateSessionInfo(sessionInfo: typeof this.appState.sessionInfo): void {
        this.appState.sessionInfo = sessionInfo;

        this.broadcastToPopupPorts({
            type: "sessionUpdate",
            sessionInfo: this.appState.sessionInfo,
            invites: this.appState.invites
        } as any);
    }

    /**
     * Update session invites
     */
    updateInvites(invites: typeof this.appState.invites): void {
        this.appState.invites = invites;

        this.broadcastToPopupPorts({
            type: "sessionUpdate",
            sessionInfo: this.appState.sessionInfo,
            invites: this.appState.invites
        } as any);
    }

    /**
     * Clear session-related state
     */
    clearSessionState(): void {
        this.appState.sessionInfo = null;
        this.appState.invites = [];
        this.appState.meshStatus = { type: MeshStatusType.Incomplete };
        this.appState.dkgState = DkgState.Idle;
        this.appState.webrtcConnections = {};

        console.log("[StateManager] Cleared session state");
        this.broadcastCurrentState();
    }

    /**
     * Get popup ports count for debugging
     */
    getPopupPortsCount(): number {
        return this.popupPorts.size;
    }

    /**
     * Set device ID
     */
    setDeviceId(deviceId: string): void {
        this.appState.deviceId = deviceId;
        console.log("[StateManager] Set device ID:", deviceId);
    }

    /**
     * Set blockchain selection (maintains blockchain field for backward compatibility)
     */
    setBlockchain(blockchain: "ethereum" | "solana"): void {
        // Convert blockchain to curve for new state model
        this.appState.curve = blockchain === "ethereum" ? "secp256k1" : "ed25519";
        console.log("[StateManager] Set blockchain:", blockchain, "-> curve:", this.appState.curve);
    }

    /**
     * Set curve selection
     */
    setCurve(curve: "ed25519" | "secp256k1"): void {
        this.appState.curve = curve;
        console.log("[StateManager] Set curve:", curve);
    }

    /**
     * Get specific state properties
     */
    getDeviceId(): string { return this.appState.deviceId; }
    getSessionInfo() { return this.appState.sessionInfo; }
    getInvites() { return this.appState.invites; }
    getConnectedDevices(): string[] { return this.appState.connecteddevices; }
    getWebRTCConnections() { return this.appState.webrtcConnections; }
    isWebSocketConnected(): boolean { return this.appState.wsConnected; }
    getMeshStatus() { return this.appState.meshStatus; }
    getDkgState() { return this.appState.dkgState; }
    getCurve() { return this.appState.curve; }
    getBlockchain() {
        // Convert curve back to blockchain for backward compatibility
        return this.appState.curve === "secp256k1" ? "ethereum" : "solana";
    }

    /**
     * Auto-fetch DKG address when DKG completes (moved from popup)
     * This handles the business logic that was previously in the popup reactive statement
     */
    private async fetchAndUpdateDkgAddress(): Promise<void> {
        try {
            const blockchain = this.getBlockchain();
            const command = blockchain === "ethereum" ? "getEthereumAddress" : "getSolanaAddress";

            console.log("[StateManager] Auto-fetching DKG address for blockchain:", blockchain);

            // Send message to offscreen document to get DKG address
            const response = await chrome.runtime.sendMessage({
                type: command,
                payload: {},
            });

            if (response && response.success) {
                const addressKey = blockchain === "ethereum" ? "ethereumAddress" : "solanaAddress";
                const dkgAddress = response.data[addressKey] || "";

                if (dkgAddress) {
                    console.log("[StateManager] Successfully fetched DKG address:", dkgAddress);

                    // Update app state
                    this.appState.dkgAddress = dkgAddress;
                    this.appState.dkgError = "";

                    // Store address in chrome.storage.local for content script access
                    if (blockchain === "ethereum") {
                        chrome.storage.local.set({ 
                            'mpc_ethereum_address': dkgAddress 
                        }, () => {
                            console.log("[StateManager] Stored Ethereum address in chrome.storage.local");
                        });
                    } else if (blockchain === "solana") {
                        chrome.storage.local.set({ 
                            'mpc_solana_address': dkgAddress 
                        }, () => {
                            console.log("[StateManager] Stored Solana address in chrome.storage.local");
                        });
                    }

                    // Broadcast DKG address update to popup
                    this.broadcastToPopupPorts({
                        type: "dkgAddressUpdate",
                        address: dkgAddress,
                        blockchain: blockchain
                    } as any);
                } else {
                    const error = `No DKG ${blockchain} address available. Please complete DKG first.`;
                    console.warn("[StateManager]", error);
                    this.appState.dkgError = error;
                    this.appState.dkgAddress = "";
                }
            } else {
                const error = response?.error || `Failed to get DKG ${blockchain} address`;
                console.error("[StateManager] DKG address fetch failed:", error);
                this.appState.dkgError = error;
                this.appState.dkgAddress = "";
            }
        } catch (error: any) {
            const errorMessage = `Error fetching DKG address: ${error.message || error}`;
            console.error("[StateManager]", errorMessage);
            this.appState.dkgError = errorMessage;
            this.appState.dkgAddress = "";
        }

        // Broadcast current state to ensure popup is updated
        this.broadcastCurrentState();
    }
}
