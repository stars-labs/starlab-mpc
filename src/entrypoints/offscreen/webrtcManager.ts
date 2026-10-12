// ===================================================================
// WEBRTC MANAGER MODULE
// ===================================================================
//
// This module is the main coordinator for all WebRTC operations.
// It orchestrates the modular components:
// - WebRTCConnectionManager: Handles peer connections and data channels
// - DkgManager: Manages distributed key generation
// - SigningManager: Handles threshold signing operations
// - MeshManager: Manages session and mesh status
//
// This modular approach makes the code much easier for junior developers
// to understand, debug, and extend.
// ===================================================================

import type { SessionInfo } from "../../types/session";
import type { DkgState } from "../../types/dkg";
import type { MeshStatus, MeshStatusType } from "../../types/mesh";
import type { WebRTCAppMessage } from "../../types/webrtc";
import type { WebSocketMessagePayload } from '../../types/websocket';
import { match } from "ts-pattern";

// Import our modular components
import { WebRTCConnectionManager } from './webrtcConnection';
import { DkgManager } from './dkgManager';
import { SigningManager, SigningState, SigningInfo } from './signingManager';
import { MeshManager } from './meshManager';

// Re-export types for convenience
export { DkgState, MeshStatusType, SigningState, SigningInfo };

/**
 * Main WebRTC Manager class that coordinates all modular components
 */
export class WebRTCManager {
    private deviceId: string;
    private sendMessageToBackground: (toPeerId: string, payload: WebSocketMessagePayload) => void;

    // Component instances
    private webrtcConnection: WebRTCConnectionManager;
    private dkgManager: DkgManager;
    private signingManager: SigningManager;
    private meshManager: MeshManager;

    // Current state
    private sessionInfo: SessionInfo | null = null;
    private blockchain: "ethereum" | "solana" = "solana";

    // Callback handlers
    public onLog: (message: string) => void = () => { };
    public onSessionUpdate: (sessionInfo: SessionInfo | null, invites: SessionInfo[]) => void = () => { };
    public onMeshStatusUpdate: (status: MeshStatus) => void = () => { };
    public onDkgStateUpdate: (state: DkgState) => void = () => { };
    public onSigningStateUpdate: (state: any, info: any) => void = () => { };
    public onWebRTCConnectionUpdate: (peerId: string, connected: boolean) => void = () => { };

    constructor(deviceId: string, sendMessageToBackground: (toPeerId: string, payload: WebSocketMessagePayload) => void) {
        this.deviceId = deviceId;
        this.sendMessageToBackground = sendMessageToBackground;

        this._log(`Initializing WebRTCManager for device: ${deviceId}`);

        // Initialize WebRTC connection manager
        this.webrtcConnection = new WebRTCConnectionManager(deviceId);
        this.webrtcConnection.onDataChannelMessage = (fromPeerId: string, data: any) => {
            this._handleDataChannelMessage(fromPeerId, data);
        };
        this.webrtcConnection.onConnectionStateChange = (peerId: string, state: RTCPeerConnectionState) => {
            this._handleConnectionStateChange(peerId, state);
        };
        this.webrtcConnection.onDataChannelOpen = (peerId: string) => {
            this._log(`Data channel opened with ${peerId}`);
        };
        this.webrtcConnection.onSignalNeeded = (toPeerId: string, signal: any) => {
            this._sendWebRTCSignal(toPeerId, signal);
        };

        // Initialize DKG manager
        this.dkgManager = new DkgManager(deviceId, {
            onLog: (message: string) => this._log(message),
            onDkgStateUpdate: (state: DkgState) => this.onDkgStateUpdate(state),
            onSendMessage: (toPeerId: string, message: WebRTCAppMessage) => {
                this.webrtcConnection.sendToPeer(toPeerId, message);
            }
        });

        // Initialize signing manager
        this.signingManager = new SigningManager(deviceId, {
            onLog: (message: string) => this._log(message),
            onSigningStateUpdate: (state: any, info: any) => this.onSigningStateUpdate(state, info),
            onSendMessage: (toPeerId: string, message: WebRTCAppMessage) => {
                this.webrtcConnection.sendToPeer(toPeerId, message);
            }
        });

        // Initialize mesh manager
        this.meshManager = new MeshManager(deviceId, {
            onLog: (message: string) => this._log(message),
            onMeshStatusUpdate: (status: MeshStatus) => this.onMeshStatusUpdate(status),
            onSessionUpdate: (sessionInfo: SessionInfo | null, invites: SessionInfo[]) => {
                this.onSessionUpdate(sessionInfo, invites);
            }
        });

        this._log("WebRTCManager initialized successfully");
    }

    /**
     * Create a new session
     */
    async createSession(sessionId: string, threshold: number): Promise<void> {
        this._log(`Creating session: ${sessionId} with threshold: ${threshold}`);
        await this.meshManager.createSession(sessionId, threshold);
        this.sessionInfo = this.meshManager.sessionInfo;
        this._updateCallbacks();
    }

    /**
     * Join an existing session
     */
    async joinSession(sessionId: string): Promise<void> {
        this._log(`Joining session: ${sessionId}`);
        await this.meshManager.joinSession(sessionId);
        this.sessionInfo = this.meshManager.sessionInfo;
        this._updateCallbacks();
    }

    /**
     * Start the DKG process
     */
    async startDkg(): Promise<void> {
        if (!this.sessionInfo) {
            throw new Error("No active session");
        }

        this._log("Starting DKG process");
        await this.dkgManager.startDkg();
        this._updateCallbacks();
    }

    /**
     * Request threshold signing
     */
    async requestSigning(transactionData: any): Promise<void> {
        this._log("Requesting threshold signing");
        await this.signingManager.requestSigning(transactionData, this.sessionInfo);
        this._updateCallbacks();
    }

    /**
     * Accept a signing request
     */
    async acceptSigning(signingId: string): Promise<void> {
        this._log(`Accepting signing request: ${signingId}`);
        await this.signingManager.acceptSigning(signingId);
        this._updateCallbacks();
    }

    /**
     * Set the blockchain type
     */
    setBlockchain(blockchain: "ethereum" | "solana"): void {
        this._log(`Setting blockchain to: ${blockchain}`);
        this.blockchain = blockchain;
    }

    /**
     * Get available addresses
     */
    getAddresses(): Record<string, string> {
        const addresses: Record<string, string> = {};

        const currentAddress = this.dkgManager.getCurrentAddress();
        if (currentAddress) {
            addresses.current = currentAddress;
        }

        const ethAddress = this.dkgManager.getEthereumAddress();
        if (ethAddress) {
            addresses.ethereum = ethAddress;
        }

        const solAddress = this.dkgManager.getSolanaAddress();
        if (solAddress) {
            addresses.solana = solAddress;
        }

        return addresses;
    }

    /**
     * Handle WebRTC signals from other peers
     */
    async handleWebRTCSignal(fromPeerId: string, signal: any): Promise<void> {
        this._log(`Handling WebRTC signal from ${fromPeerId}`);
        await this.webrtcConnection.handleSignal(fromPeerId, signal);
    }

    /**
     * Handle WebSocket message payloads from the signaling server
     */
    handleWebSocketMessagePayload(fromPeerId: string, msg: WebSocketMessagePayload): void {
        this._log(`Received WebSocketMessage from ${fromPeerId}: ${msg.websocket_msg_type}`);
        this._log(`Full message payload: ${JSON.stringify(msg)}`);

        match(msg.websocket_msg_type)
            .with('WebRTCSignal', () => {
                this._log(`WebRTCSignal data: ${JSON.stringify(msg)}`);

                // Accept WebRTC signals from any peer - no session requirement
                this._log(`Processing WebRTC signal from ${fromPeerId} (no session check)`);

                // Handle different message structures
                let signalData = null;
                if ((msg as any).data) {
                    // Standard structure: { data: { type: "Offer/Answer/Candidate", data: {...} } }
                    signalData = (msg as any).data;
                } else if ((msg as any).Offer) {
                    // Server structure: { Offer: {...}, websocket_msg_type: "WebRTCSignal" }
                    signalData = { type: 'Offer', data: (msg as any).Offer };
                } else if ((msg as any).Answer) {
                    // Server structure: { Answer: {...}, websocket_msg_type: "WebRTCSignal" }
                    signalData = { type: 'Answer', data: (msg as any).Answer };
                } else if ((msg as any).Candidate) {
                    // Server structure: { Candidate: {...}, websocket_msg_type: "WebRTCSignal" }
                    signalData = { type: 'Candidate', data: (msg as any).Candidate };
                }

                if (signalData) {
                    this._log(`Extracted WebRTC signal: ${JSON.stringify(signalData)}`);
                    this.handleWebRTCSignal(fromPeerId, signalData);
                } else {
                    this._log(`WebRTCSignal from ${fromPeerId} missing data - full msg: ${JSON.stringify(msg)}`);
                }
            })
            .otherwise(() => {
                // Handle unknown message types with proper logging
                this._log(`Unknown WebSocketMessage type from ${fromPeerId}: ${(msg as any).websocket_msg_type}. Full payload: ${JSON.stringify(msg)}`);
            });
    }

    // ===================================================================
    // GETTERS FOR COMPONENT STATE
    // ===================================================================

    getDkgState(): DkgState {
        return this.dkgManager.dkgState;
    }

    getSigningState(): SigningState {
        return this.signingManager.signingState;
    }

    getSessionInfo(): SessionInfo | null {
        return this.meshManager.sessionInfo;
    }

    getMeshStatus(): MeshStatus {
        return this.meshManager.meshStatus;
    }

    /**
     * Handle application messages from data channels
     */
    private async _handleDataChannelMessage(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        this._log(`Received app message from ${fromPeerId}: ${message.webrtc_msg_type}`);

        // Route message to appropriate manager based on type
        const msgType = message.webrtc_msg_type;

        if (msgType.includes('Dkg') || msgType.includes('Round')) {
            await this.dkgManager.handleMessage(fromPeerId, message);
        } else if (msgType.includes('Signing') || msgType.includes('Signature')) {
            await this.signingManager.handleMessage(fromPeerId, message);
        } else if (msgType.includes('Session') || msgType.includes('Mesh') || msgType.includes('Ready')) {
            await this.meshManager.handleMessage(fromPeerId, message);
        } else {
            this._log(`Unknown message type: ${msgType}`);
        }

        this._updateCallbacks();
    }

    /**
     * Handle connection state changes
     */
    private _handleConnectionStateChange(peerId: string, state: RTCPeerConnectionState): void {
        const connected = state === 'connected';
        this._log(`Connection state changed: ${peerId} -> ${state}`);
        this.meshManager.updateConnectionStatus(peerId, connected);
        this.onWebRTCConnectionUpdate(peerId, connected);
        this._updateCallbacks();
    }

    /**
     * Send WebRTC signal to peer via background
     */
    private _sendWebRTCSignal(toPeerId: string, signal: any): void {
        // Format the signal according to WebSocketMessagePayload type
        // The type expects a structure like: { websocket_msg_type: 'WebRTCSignal'; Offer: SDPInfo }
        let payload: WebSocketMessagePayload;

        if (signal.type === 'Offer' && signal.data) {
            payload = {
                websocket_msg_type: 'WebRTCSignal',
                Offer: signal.data
            } as WebSocketMessagePayload;
        } else if (signal.type === 'Answer' && signal.data) {
            payload = {
                websocket_msg_type: 'WebRTCSignal',
                Answer: signal.data
            } as WebSocketMessagePayload;
        } else if (signal.type === 'Candidate' && signal.data) {
            payload = {
                websocket_msg_type: 'WebRTCSignal',
                Candidate: signal.data
            } as WebSocketMessagePayload;
        } else {
            // Fallback - try to use the signal directly if it's already in the correct format
            payload = {
                websocket_msg_type: 'WebRTCSignal',
                ...signal
            } as WebSocketMessagePayload;
        }

        this.sendMessageToBackground(toPeerId, payload);
    }

    /**
     * Update all callback functions with current state
     */
    private _updateCallbacks(): void {
        // Update session status
        this.onSessionUpdate(this.sessionInfo, this.meshManager.invites);

        // Update mesh status
        this.onMeshStatusUpdate(this.meshManager.meshStatus);

        // Update DKG state
        this.onDkgStateUpdate(this.dkgManager.dkgState);

        // Update signing state
        if (this.signingManager.signingInfo) {
            this.onSigningStateUpdate(this.signingManager.signingState, this.signingManager.signingInfo);
        }
    }

    /**
     * Log with prefix
     */
    private _log(message: string): void {
        const logMessage = `[WebRTCManager] ${message}`;
        console.log(logMessage);
        this.onLog(logMessage);
    }

    /**
     * Cleanup resources
     */
    async cleanup(): Promise<void> {
        this._log("Cleaning up WebRTCManager");
        await this.webrtcConnection.cleanup();
        await this.dkgManager.cleanup();
        await this.signingManager.cleanup();
        await this.meshManager.cleanup();
    }
}
