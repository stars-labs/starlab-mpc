// ===================================================================
// WEBRTC CONNECTION MODULE
// ===================================================================
//
// This module handles pure WebRTC connection management including
// peer connections, data channels, and ICE candidate handling.
// It focuses solely on the networking aspects of WebRTC.
//
// Responsibilities:
// - Manage RTCPeerConnection instances
// - Handle data channel creation and management
// - Process ICE candidates and connection states
// - Provide connection status monitoring
// - Handle WebRTC signaling (offers, answers, candidates)
// ===================================================================

import { WebSocketMessagePayload, WebRTCSignal } from '../../types/websocket';
import { match } from "ts-pattern";

/**
 * Configuration for WebRTC connections
 */
const ICE_SERVERS = [{ urls: 'stun:stun.l.google.com:19302' }];
const DATA_CHANNEL_LABEL = 'frost-dkg'; // Standardized channel label

/**
 * Connection state for a peer
 */
export interface PeerConnectionState {
    peerId: string;
    connection: RTCPeerConnection;
    dataChannel: RTCDataChannel | null;
    connectionState: RTCPeerConnectionState;
    iceCandidatesReceived: number;
    isConnected: boolean;
}

/**
 * WebRTC Connection Manager
 * Handles the low-level WebRTC connection mechanics
 */
export class WebRTCConnectionManager {
    private localPeerId: string;
    private peerConnections: Map<string, RTCPeerConnection> = new Map();
    private dataChannels: Map<string, RTCDataChannel> = new Map();
    private pendingIceCandidates: Map<string, RTCIceCandidateInit[]> = new Map();
    private connectionStates: Map<string, PeerConnectionState> = new Map();

    // Callbacks for events
    public onDataChannelMessage: (fromPeerId: string, data: any) => void = () => { };
    public onConnectionStateChange: (peerId: string, state: RTCPeerConnectionState) => void = () => { };
    public onDataChannelOpen: (peerId: string) => void = () => { };
    public onDataChannelClose: (peerId: string) => void = () => { };
    public onSignalNeeded: (toPeerId: string, signal: WebRTCSignal) => void = () => { };

    constructor(localPeerId: string) {
        this.localPeerId = localPeerId;
        console.log(`🔗 [WebRTCConnection] Initialized for peer: ${localPeerId}`);
    }

    /**
     * Create a new peer connection
     */
    async createPeerConnection(peerId: string, createDataChannel: boolean = true): Promise<RTCPeerConnection> {
        console.log(`🔗 [WebRTCConnection] Creating connection to ${peerId}`);

        const peerConnection = new RTCPeerConnection({ iceServers: ICE_SERVERS });

        // Set up connection state monitoring
        peerConnection.onconnectionstatechange = () => {
            const state = peerConnection.connectionState;
            console.log(`🔗 [WebRTCConnection] ${peerId} connection state: ${state}`);

            this.updateConnectionState(peerId, state);
            this.onConnectionStateChange(peerId, state);
        };

        // Handle ICE candidates
        peerConnection.onicecandidate = (event) => {
            if (event.candidate) {
                console.log(`🧊 [WebRTCConnection] Sending ICE candidate to ${peerId}`);
                this.onSignalNeeded(peerId, {
                    Candidate: {
                        candidate: event.candidate.candidate,
                        sdpMid: event.candidate.sdpMid,
                        sdpMLineIndex: event.candidate.sdpMLineIndex
                    }
                });
            }
        };

        // Handle incoming data channels
        peerConnection.ondatachannel = (event) => {
            console.log(`📨 [WebRTCConnection] Received data channel from ${peerId}:`, event.channel.label);
            this.setupDataChannel(peerId, event.channel);
        };

        // Create outgoing data channel if requested
        if (createDataChannel) {
            const dataChannel = peerConnection.createDataChannel(DATA_CHANNEL_LABEL, {
                ordered: true
            });
            console.log(`📤 [WebRTCConnection] Created data channel to ${peerId}: ${DATA_CHANNEL_LABEL}`);
            this.setupDataChannel(peerId, dataChannel);
        }

        this.peerConnections.set(peerId, peerConnection);
        this.initializeConnectionState(peerId, peerConnection);

        return peerConnection;
    }

    /**
     * Set up data channel event handlers
     */
    private setupDataChannel(peerId: string, dataChannel: RTCDataChannel): void {
        this.dataChannels.set(peerId, dataChannel);

        dataChannel.onopen = () => {
            console.log(`✅ [WebRTCConnection] Data channel opened with ${peerId}`);
            this.markChannelConnected(peerId, true);
            this.onDataChannelOpen(peerId);
        };

        dataChannel.onclose = () => {
            console.log(`❌ [WebRTCConnection] Data channel closed with ${peerId}`);
            this.markChannelConnected(peerId, false);
            this.onDataChannelClose(peerId);
        };

        dataChannel.onerror = (error) => {
            console.error(`💥 [WebRTCConnection] Data channel error with ${peerId}:`, error);
        };

        dataChannel.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                console.log(`📨 [WebRTCConnection] Message from ${peerId}:`, data);
                this.onDataChannelMessage(peerId, data);
            } catch (error) {
                console.error(`❌ [WebRTCConnection] Failed to parse message from ${peerId}:`, error);
            }
        };
    }

    /**
     * Handle WebRTC signaling (offers, answers, candidates)
     */
    async handleSignal(fromPeerId: string, signal: any): Promise<void> {
        console.log(`📡 [WebRTCConnection] Handling signal from ${fromPeerId}:`, signal.signal_type);

        let peerConnection = this.peerConnections.get(fromPeerId);

        // Create connection if it doesn't exist
        if (!peerConnection) {
            peerConnection = await this.createPeerConnection(fromPeerId, false);
        }

        try {
            await match(signal.signal_type)
                .with('offer', async () => {
                    await this.handleOffer(fromPeerId, peerConnection, signal.offer);
                })
                .with('answer', async () => {
                    await this.handleAnswer(fromPeerId, peerConnection, signal.answer);
                })
                .with('candidate', async () => {
                    await this.handleIceCandidate(fromPeerId, peerConnection, signal.candidate);
                })
                .otherwise(() => {
                    console.warn(`⚠️ [WebRTCConnection] Unknown signal type: ${signal.signal_type}`);
                    return Promise.resolve();
                });
        } catch (error) {
            console.error(`❌ [WebRTCConnection] Error handling signal from ${fromPeerId}:`, error);
        }
    }

    /**
     * Handle incoming offer
     */
    private async handleOffer(peerId: string, peerConnection: RTCPeerConnection, offer: RTCSessionDescriptionInit): Promise<void> {
        console.log(`📥 [WebRTCConnection] Handling offer from ${peerId}`);

        await peerConnection.setRemoteDescription(offer);
        const answer = await peerConnection.createAnswer();
        await peerConnection.setLocalDescription(answer);

        this.onSignalNeeded(peerId, {
            Answer: {
                sdp: answer.sdp || ""
            }
        });

        // Process any pending ICE candidates
        await this.processPendingIceCandidates(peerId, peerConnection);
    }

    /**
     * Handle incoming answer
     */
    private async handleAnswer(peerId: string, peerConnection: RTCPeerConnection, answer: RTCSessionDescriptionInit): Promise<void> {
        console.log(`📥 [WebRTCConnection] Handling answer from ${peerId}`);

        await peerConnection.setRemoteDescription(answer);

        // Process any pending ICE candidates
        await this.processPendingIceCandidates(peerId, peerConnection);
    }

    /**
     * Handle incoming ICE candidate
     */
    private async handleIceCandidate(peerId: string, peerConnection: RTCPeerConnection, candidate: RTCIceCandidateInit): Promise<void> {
        console.log(`🧊 [WebRTCConnection] Handling ICE candidate from ${peerId}`);

        if (peerConnection.remoteDescription) {
            await peerConnection.addIceCandidate(candidate);
            this.incrementIceCandidatesReceived(peerId);
        } else {
            // Queue candidate for later processing
            if (!this.pendingIceCandidates.has(peerId)) {
                this.pendingIceCandidates.set(peerId, []);
            }
            this.pendingIceCandidates.get(peerId)!.push(candidate);
            console.log(`🧊 [WebRTCConnection] Queued ICE candidate from ${peerId} (no remote description yet)`);
        }
    }

    /**
     * Process pending ICE candidates
     */
    private async processPendingIceCandidates(peerId: string, peerConnection: RTCPeerConnection): Promise<void> {
        const pending = this.pendingIceCandidates.get(peerId);
        if (pending && pending.length > 0) {
            console.log(`🧊 [WebRTCConnection] Processing ${pending.length} pending ICE candidates for ${peerId}`);

            for (const candidate of pending) {
                try {
                    await peerConnection.addIceCandidate(candidate);
                    this.incrementIceCandidatesReceived(peerId);
                } catch (error) {
                    console.error(`❌ [WebRTCConnection] Failed to add pending ICE candidate:`, error);
                }
            }

            this.pendingIceCandidates.delete(peerId);
        }
    }

    /**
     * Create and send an offer to a peer
     */
    async createOffer(peerId: string): Promise<void> {
        console.log(`📤 [WebRTCConnection] Creating offer for ${peerId}`);

        const peerConnection = await this.createPeerConnection(peerId, true);
        const offer = await peerConnection.createOffer();
        await peerConnection.setLocalDescription(offer);

        this.onSignalNeeded(peerId, {
            Offer: {
                sdp: offer.sdp || ""
            }
        });
    }

    /**
     * Send data to a peer
     */
    sendToPeer(peerId: string, data: any): boolean {
        const dataChannel = this.dataChannels.get(peerId);

        if (!dataChannel || dataChannel.readyState !== 'open') {
            console.warn(`⚠️ [WebRTCConnection] Cannot send to ${peerId}: channel not ready`);
            return false;
        }

        try {
            dataChannel.send(JSON.stringify(data));
            console.log(`📤 [WebRTCConnection] Sent data to ${peerId}:`, data);
            return true;
        } catch (error) {
            console.error(`❌ [WebRTCConnection] Failed to send to ${peerId}:`, error);
            return false;
        }
    }

    /**
     * Close connection to a peer
     */
    closePeerConnection(peerId: string): void {
        console.log(`🔌 [WebRTCConnection] Closing connection to ${peerId}`);

        const dataChannel = this.dataChannels.get(peerId);
        if (dataChannel) {
            dataChannel.close();
            this.dataChannels.delete(peerId);
        }

        const peerConnection = this.peerConnections.get(peerId);
        if (peerConnection) {
            peerConnection.close();
            this.peerConnections.delete(peerId);
        }

        this.connectionStates.delete(peerId);
        this.pendingIceCandidates.delete(peerId);
    }

    /**
     * Get connection status for a peer
     */
    getConnectionStatus(peerId: string): PeerConnectionState | null {
        return this.connectionStates.get(peerId) || null;
    }

    /**
     * Get all connected peers
     */
    getConnectedPeers(): string[] {
        return Array.from(this.connectionStates.entries())
            .filter(([_, state]) => state.isConnected)
            .map(([peerId, _]) => peerId);
    }

    /**
     * Check if peer is connected
     */
    isPeerConnected(peerId: string): boolean {
        const state = this.connectionStates.get(peerId);
        return state ? state.isConnected : false;
    }

    /**
     * Initialize connection state tracking
     */
    private initializeConnectionState(peerId: string, connection: RTCPeerConnection): void {
        this.connectionStates.set(peerId, {
            peerId,
            connection,
            dataChannel: null,
            connectionState: connection.connectionState,
            iceCandidatesReceived: 0,
            isConnected: false
        });
    }

    /**
     * Update connection state
     */
    private updateConnectionState(peerId: string, state: RTCPeerConnectionState): void {
        const connectionState = this.connectionStates.get(peerId);
        if (connectionState) {
            connectionState.connectionState = state;
            connectionState.isConnected = state === 'connected';
        }
    }

    /**
     * Mark data channel as connected/disconnected
     */
    private markChannelConnected(peerId: string, connected: boolean): void {
        const connectionState = this.connectionStates.get(peerId);
        if (connectionState) {
            connectionState.isConnected = connected;
            connectionState.dataChannel = this.dataChannels.get(peerId) || null;
        }
    }

    /**
     * Increment ICE candidates received counter
     */
    private incrementIceCandidatesReceived(peerId: string): void {
        const connectionState = this.connectionStates.get(peerId);
        if (connectionState) {
            connectionState.iceCandidatesReceived++;
        }
    }

    /**
     * Clean up all connections
     */
    cleanup(): void {
        console.log("🧹 [WebRTCConnection] Cleaning up all connections");

        for (const peerId of Array.from(this.peerConnections.keys())) {
            this.closePeerConnection(peerId);
        }
    }
}
