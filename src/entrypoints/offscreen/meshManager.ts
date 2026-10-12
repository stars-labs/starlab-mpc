/**
 * Mesh Manager - Handles mesh status and session management
 * 
 * This module manages:
 * - Session creation and joining
 * - Mesh readiness detection and status tracking
 * - Session participant management
 * - Mesh ready signal coordination
 * 
 * Extracted from the monolithic webrtc.ts for better maintainability
 */

import { MeshStatus, MeshStatusType } from "../../types/mesh";
import { SessionInfo } from "../../types/session";
import type { WebRTCAppMessage } from "../../types/webrtc";
import { match } from "ts-pattern";

/**
 * Callback interface for mesh events
 */
export interface MeshManagerCallbacks {
    onLog: (message: string) => void;
    onMeshStatusUpdate: (status: MeshStatus) => void;
    onSessionUpdate: (sessionInfo: SessionInfo | null, invites: SessionInfo[]) => void;
}

/**
 * Mesh Manager class handles session and mesh coordination
 */
export class MeshManager {
    private localPeerId: string;
    private callbacks: MeshManagerCallbacks;

    // Session and mesh state
    public sessionInfo: SessionInfo | null = null;
    public invites: SessionInfo[] = [];
    public meshStatus: MeshStatus = { type: MeshStatusType.Incomplete };

    // Mesh ready tracking to prevent duplicate signals
    private ownMeshReadySent: boolean = false;
    private meshReadyReceived: Set<string> = new Set();

    constructor(localPeerId: string, callbacks: MeshManagerCallbacks) {
        this.localPeerId = localPeerId;
        this.callbacks = callbacks;
        this._log("MeshManager initialized");
    }

    /**
     * Create a new session
     */
    async createSession(sessionId: string, threshold: number): Promise<void> {
        try {
            this._log(`Creating session: ${sessionId} with threshold: ${threshold}`);

            this.sessionInfo = {
                session_id: sessionId,
                proposer_id: this.localPeerId,
                total: threshold,
                threshold: threshold,
                participants: [this.localPeerId],
                accepted_devices: [this.localPeerId]
            };

            this.ownMeshReadySent = false;
            this.meshReadyReceived.clear();
            this.updateMeshStatus();

            this.callbacks.onSessionUpdate(this.sessionInfo, this.invites);
            this._log(`Session created successfully: ${sessionId}`);
        } catch (error) {
            this._log(`Error creating session: ${error}`);
            throw error;
        }
    }

    /**
     * Join an existing session
     */
    async joinSession(sessionId: string): Promise<void> {
        try {
            this._log(`Joining session: ${sessionId}`);

            // Find the session in invites
            const inviteIndex = this.invites.findIndex(invite => invite.session_id === sessionId);
            if (inviteIndex === -1) {
                throw new Error(`Session ${sessionId} not found in invites`);
            }

            const sessionToJoin = this.invites[inviteIndex];

            // Set as current session
            this.sessionInfo = {
                ...sessionToJoin,
                accepted_devices: [...sessionToJoin.accepted_devices, this.localPeerId]
            };

            // Remove from invites
            this.invites.splice(inviteIndex, 1);

            this.ownMeshReadySent = false;
            this.meshReadyReceived.clear();
            this.updateMeshStatus();

            this.callbacks.onSessionUpdate(this.sessionInfo, this.invites);
            this._log(`Successfully joined session: ${sessionId}`);
        } catch (error) {
            this._log(`Error joining session: ${error}`);
            throw error;
        }
    }

    /**
     * Add a session invite
     */
    addSessionInvite(sessionInfo: SessionInfo): void {
        this._log(`Received session invite: ${sessionInfo.session_id}`);

        // Check if invite already exists
        const existingIndex = this.invites.findIndex(invite => invite.session_id === sessionInfo.session_id);
        if (existingIndex >= 0) {
            this.invites[existingIndex] = sessionInfo;
            this._log(`Updated existing session invite: ${sessionInfo.session_id}`);
        } else {
            this.invites.push(sessionInfo);
            this._log(`Added new session invite: ${sessionInfo.session_id}`);
        }

        this.callbacks.onSessionUpdate(this.sessionInfo, this.invites);
    }

    /**
     * Handle incoming mesh-related messages
     */
    async handleMessage(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        this._log(`Handling mesh message from ${fromPeerId}: ${message.webrtc_msg_type}`);

        try {
            await match(message.webrtc_msg_type)
                .with('MeshReady', async () => {
                    await this.handleMeshReady(fromPeerId, message);
                })
                .otherwise(() => {
                    this._log(`Unhandled mesh message type: ${message.webrtc_msg_type}`);
                    return Promise.resolve();
                });
        } catch (error) {
            this._log(`Error handling mesh message: ${error}`);
        }
    }

    /**
     * Handle mesh ready signal from peers
     */
    private async handleMeshReady(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        this._log(`Received mesh ready from ${fromPeerId}`);
        this.meshReadyReceived.add(fromPeerId);
        this.updateMeshStatus();
    }

    /**
     * Send mesh ready signal to all peers
     */
    async sendMeshReady(sendMessageCallback: (toPeerId: string, message: WebRTCAppMessage) => void): Promise<void> {
        if (this.ownMeshReadySent) {
            this._log("Mesh ready already sent, skipping");
            return;
        }

        if (!this.sessionInfo) {
            this._log("No session info available, cannot send mesh ready");
            return;
        }

        this._log("Sending mesh ready to all peers");

        const meshReadyMessage: WebRTCAppMessage = {
            webrtc_msg_type: 'MeshReady',
            session_id: this.sessionInfo.session_id,
            device_id: this.localPeerId
        };

        // Send to all participants except ourselves
        const otherParticipants = this.sessionInfo.accepted_devices.filter(p => p !== this.localPeerId);
        for (const peerId of otherParticipants) {
            try {
                sendMessageCallback(peerId, meshReadyMessage);
            } catch (error) {
                this._log(`Failed to send mesh ready to ${peerId}: ${error}`);
            }
        }

        this.ownMeshReadySent = true;
        this.updateMeshStatus();
    }

    /**
     * Update mesh status based on current state
     */
    private updateMeshStatus(): void {
        if (!this.sessionInfo) {
            this.meshStatus = { type: MeshStatusType.Incomplete };
            this.callbacks.onMeshStatusUpdate(this.meshStatus);
            return;
        }

        const requiredPeers = this.sessionInfo.threshold;
        const totalParticipants = this.sessionInfo.accepted_devices.length;
        const readyCount = this.meshReadyReceived.size + (this.ownMeshReadySent ? 1 : 0);

        this._log(`Mesh status check: totalParticipants=${totalParticipants}, requiredPeers=${requiredPeers}, readyCount=${readyCount}`);

        if (totalParticipants >= requiredPeers && readyCount >= requiredPeers) {
            this.meshStatus = { type: MeshStatusType.Ready };
            this._log("Mesh is ready!");
        } else {
            this.meshStatus = { type: MeshStatusType.Incomplete };
        }

        this.callbacks.onMeshStatusUpdate(this.meshStatus);
    }

    /**
     * Check if mesh is ready for operations
     */
    isMeshReady(): boolean {
        return this.meshStatus.type === MeshStatusType.Ready;
    }

    /**
     * Get current session information
     */
    getSessionInfo(): SessionInfo | null {
        return this.sessionInfo;
    }

    /**
     * Get pending invites
     */
    getInvites(): SessionInfo[] {
        return [...this.invites];
    }

    /**
     * Update connection status for a peer
     */
    updateConnectionStatus(peerId: string, connected: boolean): void {
        this._log(`Connection status update: ${peerId} -> ${connected ? 'connected' : 'disconnected'}`);

        if (!connected) {
            // Remove disconnected peer from mesh ready tracking
            this.meshReadyReceived.delete(peerId);
            this._log(`Removed ${peerId} from mesh ready tracking due to disconnection`);
        }

        // Update mesh status when connection states change
        this.updateMeshStatus();
    }

    /**
     * Cleanup resources
     */
    async cleanup(): Promise<void> {
        this._log("Cleaning up MeshManager");
        this.sessionInfo = null;
        this.invites = [];
        this.meshReadyReceived.clear();
        this.ownMeshReadySent = false;
        this.meshStatus = { type: MeshStatusType.Incomplete };
    }

    /**
     * Internal logging with peer ID prefix
     */
    private _log(message: string): void {
        const logMessage = `[MeshManager:${this.localPeerId}] ${message}`;
        this.callbacks.onLog(logMessage);
    }
}
