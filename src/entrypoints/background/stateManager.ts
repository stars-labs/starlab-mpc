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

import { AppState, INITIAL_APP_STATE } from "../../types/appstate";
import { MeshStatusType } from "../../types/mesh";
import { DkgState } from "../../types/dkg";
import type {
    BackgroundToPopupMessage,
    InitialStateMessage,
    OffscreenToBackgroundMessage
} from "../../types/messages";
import { match } from "ts-pattern";

/**
 * Manages central application state and popup communication
 */
export class StateManager {
    private appState: AppState;
    private popupPorts = new Set<chrome.runtime.Port>();

    constructor(initialState?: Partial<AppState>) {
        this.appState = {
            ...INITIAL_APP_STATE,
            ...initialState
        };
    }

    /**
     * Get current application state (returns a copy for external use)
     */
    getState(): AppState {
        return { ...this.appState };
    }

    /**
     * Get reference to application state (for internal components that need shared state)
     * WARNING: Only use this for components that need to share state references!
     */
    getStateReference(): AppState {
        return this.appState;
    }

    /**
     * Update application state
     */
    updateState(updates: Partial<AppState>): void {
        console.log("[StateManager] Updating state:", updates);
        this.appState = {
            ...this.appState,
            ...updates
        };
    }

    /**
     * Update specific state properties with deep merge support
     */
    updateStateProperty<K extends keyof AppState>(key: K, value: AppState[K]): void {
        console.log(`[StateManager] Updating state property ${String(key)}:`, value);
        this.appState[key] = value;
    }

    /**
     * Update WebSocket connection status
     */
    updateWebSocketStatus(connected: boolean, error?: string): void {
        console.log(`[StateManager] Updating WebSocket status: connected=${connected}, error=${error || 'none'}`);
        this.appState.wsConnected = connected;

        // Broadcast status update
        this.broadcastToPopupPorts({ type: "wsStatus", connected });
    }

    /**
     * Update connected devices list
     */
    updateConnectedDevices(devices: string[]): void {
        console.log(`[StateManager] Updating connected devices:`, devices);
        // Exclude current device from connected devices list
        this.appState.connecteddevices = devices.filter(deviceId => deviceId !== this.appState.deviceId);

        console.log(`[StateManager] Updated connected devices:`, this.appState.connecteddevices);

        // Broadcast device list update
        this.broadcastToPopupPorts({
            type: "deviceList",
            devices: this.appState.connecteddevices
        });
    }

    /**
     * Add a popup port connection
     */
    addPopupPort(port: chrome.runtime.Port): void {
        console.log("[StateManager] Adding popup port");
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
            dkgState: this.appState.dkgState
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
        console.log("[StateManager] Handling offscreen state update:", payload);

        match(payload.type)
            .with("webrtcConnectionUpdate", () => {
                if ('deviceId' in payload && 'connected' in payload) {
                    console.log("[StateManager] Received WebRTC connection update:", {
                        deviceId: payload.deviceId,
                        connected: payload.connected
                    });

                    // Update appState with WebRTC connection info
                    this.appState.webrtcConnections[payload.deviceId] = payload.connected;
                    console.log("[StateManager] Updated appState.webrtcConnections:", this.appState.webrtcConnections);

                    // Send WebRTC connection update directly to popup
                    const webrtcMessage = {
                        type: "webrtcConnectionUpdate",
                        deviceId: payload.deviceId,
                        connected: payload.connected
                    };

                    console.log("[StateManager] Sending WebRTC connection update to popup:", webrtcMessage);
                    this.broadcastToPopupPorts(webrtcMessage as any);
                } else {
                    console.warn("[StateManager] Invalid WebRTC connection update payload:", payload);
                }
                return true;
            })
            .with("meshStatusUpdate", () => {
                console.log("[StateManager] Received mesh status update from offscreen:", payload);
                this.appState.meshStatus = payload.status || { type: MeshStatusType.Incomplete };

                // Broadcast mesh status update directly to popup
                this.broadcastToPopupPorts({
                    type: "meshStatusUpdate",
                    status: this.appState.meshStatus
                } as any);
                return true;
            })
            .with("dkgStateUpdate", () => {
                console.log("[StateManager] Received DKG state update from offscreen:", payload);
                this.appState.dkgState = payload.state || DkgState.Idle;

                // Broadcast DKG state update directly to popup
                this.broadcastToPopupPorts({
                    type: "dkgStateUpdate",
                    state: this.appState.dkgState
                } as any);
                return true;
            })
            .with("sessionUpdate", () => {
                if ('sessionInfo' in payload && 'invites' in payload) {
                    console.log("[StateManager] Received session update from offscreen:", payload);

                    this.appState.sessionInfo = payload.sessionInfo || null;
                    this.appState.invites = payload.invites || [];

                    // Broadcast session update to popup
                    this.broadcastToPopupPorts({
                        type: "sessionUpdate",
                        sessionInfo: this.appState.sessionInfo,
                        invites: this.appState.invites
                    } as any);
                }
                return true;
            })
            .with("webrtcStatusUpdate", () => {
                if ('deviceId' in payload && 'status' in payload) {
                    console.log(`[StateManager] WebRTC status update for ${payload.deviceId}: ${payload.status}`);
                    // Forward to popup if needed
                    this.broadcastToPopupPorts({
                        type: "webrtcStatusUpdate",
                        deviceId: payload.deviceId,
                        status: payload.status
                    } as any);
                }
                return true;
            })
            .with("peerConnectionStatusUpdate", () => {
                if ('deviceId' in payload && 'connectionState' in payload) {
                    console.log(`[StateManager] Peer connection status update for ${payload.deviceId}: ${payload.connectionState}`);
                }
                return true;
            })
            .with("dataChannelStatusUpdate", () => {
                if ('deviceId' in payload && 'channelName' in payload && 'state' in payload) {
                    console.log(`[StateManager] Data channel ${payload.channelName} for ${payload.deviceId}: ${payload.state}`);
                }
                return true;
            })
            .otherwise(() => {
                console.log("[StateManager] Forwarding unknown message to popup:", payload);
                this.broadcastToPopupPorts({
                    type: "fromOffscreen",
                    payload
                } as any);
                return true;
            });
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
     * Set device ID
     */
    setDeviceId(deviceId: string): void {
        this.appState.deviceId = deviceId;
        console.log("[StateManager] Set device ID:", deviceId);
    }

    /**
     * Set blockchain selection
     */
    setBlockchain(blockchain: "ethereum" | "solana"): void {
        this.appState.blockchain = blockchain;
        this.appState.chain = blockchain; // For compatibility
        console.log("[StateManager] Set blockchain:", blockchain);
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
    getBlockchain() {
        return this.appState.blockchain || this.appState.chain || "ethereum";
    }

}
