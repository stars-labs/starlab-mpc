// ===================================================================
// WEBSOCKET MANAGEMENT MODULE
// ===================================================================
//
// This module manages WebSocket connections to the signaling server
// for MPC coordination. It handles:
// - WebSocket lifecycle management
// - Message routing and relay
// - Device discovery and management
// - Connection state synchronization
// ===================================================================

import { WebSocketClient } from "./websocket";
import { AppState } from "../../types/appstate";
import { SessionManager } from "./sessionManager";
import type {
    BackgroundToPopupMessage,
    InitialStateMessage,
    OffscreenMessage
} from "../../types/messages";
import { ServerMsg, WebSocketMessagePayload } from '../../types/websocket';
import { match } from "ts-pattern";

/**
 * Manages WebSocket connections and message handling for MPC coordination
 */
export class WebSocketManager {
    private wsClient: WebSocketClient | null = null;
    private devices: string[] = [];
    private appState: AppState;
    private sessionManager: SessionManager;
    private broadcastToPopup: (message: BackgroundToPopupMessage) => void;
    private sendToOffscreen: (message: OffscreenMessage, description: string) => Promise<{ success: boolean; error?: string }>;
    private stateManager?: any; // StateManager for persistence

    constructor(
        appState: AppState,
        sessionManager: SessionManager,
        broadcastToPopup: (message: BackgroundToPopupMessage) => void,
        sendToOffscreen: (message: OffscreenMessage, description: string) => Promise<{ success: boolean; error?: string }>,
        stateManager?: any // Optional StateManager for persistence
    ) {
        this.appState = appState;
        this.sessionManager = sessionManager;
        this.broadcastToPopup = broadcastToPopup;
        this.sendToOffscreen = sendToOffscreen;
        this.stateManager = stateManager;
    }

    /**
     * Initialize WebSocket connection with URL and device ID
     */
    async initialize(url: string, deviceId: string): Promise<void> {
        try {
            this.wsClient = new WebSocketClient(url);

            // Set device ID
            this.appState.deviceId = deviceId;
            console.log("[WebSocketManager] Using device ID:", deviceId);

            this.setupEventHandlers();

            console.log("[WebSocketManager] Event handlers configured, attempting to connect to WebSocket:", url);
            this.wsClient.connect();
            console.log("[WebSocketManager] WebSocket connect() method completed");

        } catch (error) {
            console.error("[WebSocketManager] Failed to initialize WebSocket:", error);

            // Use StateManager to update and persist WebSocket status
            if (this.stateManager) {
                this.stateManager.updateWebSocketStatus(false, error instanceof Error ? error.message : "Unknown error");
            } else {
                this.appState.wsConnected = false;
            }

            this.broadcastToPopup({
                type: "wsError",
                error: error instanceof Error ? error.message : "Unknown error"
            } as any);
            this.broadcastToPopup({ type: "wsStatus", connected: false });
        }
    }

    /**
     * Initialize WebSocket connection (legacy method)
     */
    async initializeWebSocket(): Promise<void> {
        return this.initialize("wss://auto-life.tech", "mpc-2");
    }

    /**
     * Set up WebSocket event handlers
     */
    private setupEventHandlers(): void {
        if (!this.wsClient) return;

        console.log("[WebSocketManager] Setting up WebSocket event handlers");

        // Handle connection open
        this.wsClient.onOpen(() => {
            console.log("[WebSocketManager] WebSocket onOpen event triggered - connection established");

            // Use StateManager to update and persist WebSocket status
            if (this.stateManager) {
                this.stateManager.updateWebSocketStatus(true);
            } else {
                this.appState.wsConnected = true;
            }

            // Broadcast connection status immediately to any connected popups
            console.log("[WebSocketManager] Broadcasting wsConnected=true to popups");
            this.broadcastToPopup({ type: "wsStatus", connected: true });

            // Register with server
            console.log("[WebSocketManager] Registering with server as peer:", this.appState.deviceId);
            try {
                this.wsClient!.register(this.appState.deviceId);
                console.log("[WebSocketManager] Registration sent to server");
            } catch (regError) {
                console.error("[WebSocketManager] Error during registration:", regError);
            }

            // Request initial peer list with delay to ensure registration is processed
            setTimeout(() => {
                console.log("[WebSocketManager] Requesting initial peer list from server");
                if (this.wsClient && this.wsClient.getReadyState() === WebSocket.OPEN) {
                    this.wsClient.listdevices();
                    console.log("[WebSocketManager] Initial peer list request sent successfully");
                } else {
                    console.warn("[WebSocketManager] WebSocket not ready for peer list request");
                }
            }, 1000); // 1 second delay
        });

        // Handle connection close
        this.wsClient.onClose((event) => {
            console.log("[WebSocketManager] WebSocket onClose event triggered, event:", event);

            // Use StateManager to update and persist WebSocket status
            if (this.stateManager) {
                this.stateManager.updateWebSocketStatus(false, `Connection closed: ${event.code} ${event.reason}`);
            } else {
                this.appState.wsConnected = false;
            }

            // Broadcast disconnection status
            console.log("[WebSocketManager] Broadcasting wsConnected=false to popups");
            this.broadcastToPopup({ type: "wsStatus", connected: false });
        });

        // Handle connection errors
        this.wsClient.onError((error) => {
            console.error("[WebSocketManager] WebSocket onError event triggered, error:", error);

            // Use StateManager to update and persist WebSocket status
            if (this.stateManager) {
                this.stateManager.updateWebSocketStatus(false, error.toString());
            } else {
                this.appState.wsConnected = false;
            }

            // Broadcast error and disconnection status
            this.broadcastToPopup({
                type: "wsError",
                error: error.toString()
            } as any);
            this.broadcastToPopup({ type: "wsStatus", connected: false });
        });

        // Set up the message handler
        this.wsClient.onMessage((message: any) => {
            this.handleWebSocketMessage(message);
        });
    }

    /**
     * Handle incoming WebSocket messages
     */
    private handleWebSocketMessage(message: any): void {
        console.log("[WebSocketManager] WebSocket message received:", message);

        // Cast to ServerMsg after receiving
        const serverMessage = message as ServerMsg;
        this.broadcastToPopup({ type: "wsMessage", message: serverMessage });

        // Handle specific message types with proper null checks
        match(serverMessage.type)
            .with("devices", () => {
                // Handle lowercase "devices" messages from server
                this.handleDeviceListMessage(serverMessage as ServerMsg & { type: "devices" }, serverMessage.type);
            })
            .with("relay", () => {
                // Handle lowercase "relay" messages from server
                this.handleRelayMessage(serverMessage as ServerMsg & { type: "relay" }, serverMessage.type);
            })
            .with("error", () => {
                this.handleErrorMessage(serverMessage as ServerMsg & { type: "error" });
            })
            .otherwise(() => {
                console.log("[WebSocketManager] Unhandled WebSocket message type:", (serverMessage as any).type);
            });
    }

    /**
     * Handle device list messages from server
     */
    private handleDeviceListMessage(msg: ServerMsg & { type: "devices" | "DEVICES" }, messageType: string): void {
        const deviceList = msg.devices || [];
        this.devices = deviceList;

        // Exclude current peer from connected devices list
        const connectedDevices = deviceList.filter((deviceId: string) => deviceId !== this.appState.deviceId);

        // Use StateManager to update and persist connected devices
        if (this.stateManager) {
            this.stateManager.updateConnectedDevices(deviceList);
        } else {
            this.appState.connecteddevices = connectedDevices;
        }

        console.log(`[WebSocketManager] Updated peer list from server (${messageType}):`, deviceList);
        console.log(`[WebSocketManager] Connected devices (excluding self):`, connectedDevices);

        // Broadcast device list update only (StateManager already broadcasts this)
        // Removed redundant "initialState" broadcast that was causing UI state reset
        console.log(`[WebSocketManager] Device list update handled via StateManager`);
    }

    /**
     * Handle relay messages from server
     */
    private handleRelayMessage(msg: ServerMsg & { type: "relay" | "RELAY" }, messageType: string): void {
        console.log(`[WebSocketManager] Received ${messageType} message from server:`, msg);
        const data = msg.data as WebSocketMessagePayload;

        if (!data || !data.websocket_msg_type) {
            console.warn("[WebSocketManager] Invalid relay message data:", data);
            return;
        }

        match(data.websocket_msg_type)
            .with("WebRTCSignal", () => {
                console.log("[WebSocketManager] WebRTC signal received:", data);
                // Forward WebRTC signal to offscreen
                const relayViaWs: OffscreenMessage = {
                    type: "relayViaWs",
                    to: msg.from,
                    data: data
                };

                this.sendToOffscreen(relayViaWs, "webrtc signal").then(result => {
                    if (!result.success) {
                        console.warn("[WebSocketManager] Failed to relay WebRTC signal to offscreen:", result.error);
                    }
                });
            })
            .with("SessionProposal", () => {
                console.log("[WebSocketManager] Session proposal received:", data);
                console.log("[WebSocketManager] SessionProposal details:", {
                    session_id: (data as any).session_id,
                    from: msg.from,
                    total: (data as any).total,
                    threshold: (data as any).threshold,
                    participants: (data as any).participants
                });
                // Handle session proposal
                this.sessionManager.handleSessionProposal(msg.from, data);
            })
            .with("SessionResponse", () => {
                console.log("[WebSocketManager] Session response received:", data);
                console.log("[WebSocketManager] SessionResponse details:", {
                    session_id: (data as any).session_id,
                    from: msg.from,
                    accepted: (data as any).accepted
                });
                this.sessionManager.handleSessionResponse(msg.from, data);
            })
            .otherwise(() => {
                console.warn("[WebSocketManager] Unknown relay message type:", (data as any).websocket_msg_type);
            });
    }

    /**
     * Handle error messages from server
     */
    private handleErrorMessage(msg: ServerMsg & { type: "error" }): void {
        console.error("[WebSocketManager] Received error from server:", msg);

        this.broadcastToPopup({
            type: "wsError",
            error: msg.error || "Unknown server error"
        } as any);
    }

    /**
     * Send a relay message to another peer
     */
    async relayMessage(toPeerId: string, data: any): Promise<{ success: boolean; error?: string }> {
        if (!this.wsClient || this.wsClient.getReadyState() !== WebSocket.OPEN) {
            return { success: false, error: "WebSocket not connected" };
        }

        try {
            await this.wsClient.relayMessage(toPeerId, data);
            return { success: true };
        } catch (error) {
            console.error("[WebSocketManager] Error relaying message:", error);
            return { success: false, error: (error as Error).message };
        }
    }

    /**
     * Request list of connected devices
     */
    async listDevices(): Promise<{ success: boolean; error?: string }> {
        if (!this.wsClient || this.wsClient.getReadyState() !== WebSocket.OPEN) {
            return { success: false, error: "WebSocket not connected" };
        }

        try {
            this.wsClient.listdevices();
            return { success: true };
        } catch (error) {
            console.error("[WebSocketManager] Error requesting device list:", error);
            return { success: false, error: (error as Error).message };
        }
    }

    /**
     * Get WebSocket connection status
     */
    getConnectionStatus(): {
        connected: boolean;
        readyState?: number;
        url?: string;
    } {
        return {
            connected: this.appState.wsConnected,
            readyState: this.wsClient?.getReadyState(),
            url: this.wsClient ? "wss://auto-life.tech" : undefined
        };
    }

    /**
     * Get the WebSocket client instance
     */
    getClient(): WebSocketClient | null {
        return this.wsClient;
    }

    /**
     * Get connected devices list
     */
    getConnectedDevices(): string[] {
        return this.appState.connecteddevices;
    }

    /**
     * Check if WebSocket is ready for communication
     */
    isReady(): boolean {
        return this.wsClient?.getReadyState() === WebSocket.OPEN;
    }
}
