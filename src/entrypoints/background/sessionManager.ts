// ===================================================================
// SESSION MANAGEMENT MODULE
// ===================================================================
//
// This module handles MPC session lifecycle management including:
// - Session creation and invitation
// - Session proposal handling
// - Session response processing
// - Session state validation
// - Session persistence across extension restarts
// ===================================================================

import { AppState } from "../../types/appstate";
import { SessionInfo, SessionProposal, SessionResponse } from "../../types/session";
import { MeshStatus } from "../../types/mesh";
import { DkgState } from "../../types/dkg";
import { WebSocketClient } from "./websocket";
import { validateSessionProposal, validateSessionAcceptance } from "../../types/messages";
import type { BackgroundToPopupMessage, OffscreenMessage } from "../../types/messages";

/**
 * Validate WebSocket session response data
 */
function validateWebSocketSessionResponse(responseData: any): boolean {
    return responseData &&
        typeof responseData.session_id === 'string' &&
        typeof responseData.accepted === 'boolean' &&
        responseData.websocket_msg_type === 'SessionResponse';
}

// SessionPersistenceManager removed - using KISS principle

/**
 * Handles MPC session lifecycle and coordination
 */
export class SessionManager {
    private appState: AppState;
    private wsClient: WebSocketClient | null;
    private broadcastToPopup: (message: BackgroundToPopupMessage) => void;
    private sendToOffscreen: (message: OffscreenMessage, description: string) => Promise<{ success: boolean; error?: string }>;

    constructor(
        appState: AppState,
        wsClient: WebSocketClient | null,
        broadcastToPopup: (message: BackgroundToPopupMessage) => void,
        sendToOffscreen: (message: OffscreenMessage, description: string) => Promise<{ success: boolean; error?: string }>
    ) {
        this.appState = appState;
        this.wsClient = wsClient;
        this.broadcastToPopup = broadcastToPopup;
        this.sendToOffscreen = sendToOffscreen;

        console.log("[SessionManager] Initialized with appState deviceId:", this.appState.deviceId);
    }

    /**
     * Update the WebSocket client reference
     */
    setWebSocketClient(wsClient: WebSocketClient | null): void {
        this.wsClient = wsClient;
        console.log("[SessionManager] WebSocket client updated:", wsClient ? "connected" : "null");
    }

    /**
     * Validate WebSocket session proposal data
     */
    private validateWebSocketSessionProposal(proposalData: any): boolean {
        return proposalData &&
            typeof proposalData.session_id === 'string' &&
            typeof proposalData.total === 'number' &&
            typeof proposalData.threshold === 'number' &&
            Array.isArray(proposalData.participants) &&
            proposalData.websocket_msg_type === 'SessionProposal';
    }

    /**
     * Handle incoming session proposals
     */
    handleSessionProposal(fromPeerId: string, proposalData: any) {
        console.log("[SessionManager] Processing session proposal from:", fromPeerId);
        console.log("[SessionManager] Proposal data:", {
            session_id: proposalData?.session_id,
            total: proposalData?.total,
            threshold: proposalData?.threshold,
            participants: proposalData?.participants,
            websocket_msg_type: proposalData?.websocket_msg_type
        });

        // SAFETY CHECK: Prevent auto-acceptance bug
        // This method should ONLY handle invitations from OTHER peers, not proposer auto-acceptance
        if (fromPeerId === this.appState.deviceId) {
            console.error("[SessionManager] 🚨 BUG PREVENTION: handleSessionProposal called with fromPeerId === deviceId");
            console.error("[SessionManager] This suggests proposer auto-acceptance is using wrong code path");
            console.error("[SessionManager] Use handleProposerAutoAcceptance() instead for proposer logic");
            return;
        }

        if (!this.validateWebSocketSessionProposal(proposalData)) {
            console.error("[SessionManager] Invalid session proposal data:", proposalData);
            console.error("[SessionManager] Expected: session_id (string), total (number), threshold (number), participants (array), websocket_msg_type='SessionProposal'");
            return;
        }

        console.log("[SessionManager] ✅ Session proposal validation passed");

        const sessionInfo: SessionInfo = {
            session_id: proposalData.session_id,
            proposer_id: fromPeerId,
            participants: proposalData.participants || [],
            threshold: proposalData.threshold,
            total: proposalData.total,
            accepted_devices: [fromPeerId], // Proposer automatically accepts
            status: "proposed"
        };

        console.log("[SessionManager] Session proposal validated:", sessionInfo);
        console.log("[SessionManager] Created sessionInfo object:", sessionInfo);

        // Debug session inclusion logic
        console.log("[SessionManager] Checking participant inclusion:");
        console.log("- Current device ID:", this.appState.deviceId);
        console.log("- Session participants:", sessionInfo.participants);
        console.log("- Is included:", sessionInfo.participants.includes(this.appState.deviceId));

        // Check if this peer is included in the session
        if (sessionInfo.participants.includes(this.appState.deviceId)) {
            console.log("[SessionManager] ✅ This peer is included in session proposal");

            // Check for existing invite
            const existingInviteIndex = this.appState.invites.findIndex(inv =>
                inv.session_id === sessionInfo.session_id
            );

            if (existingInviteIndex !== -1) {
                console.log("[SessionManager] 🔄 Updating existing session invite at index:", existingInviteIndex);
                this.appState.invites[existingInviteIndex] = sessionInfo;
            } else {
                console.log("[SessionManager] ➕ Adding new session invite to appState.invites");
                console.log("[SessionManager] Current invites before:", this.appState.invites.length);
                this.appState.invites.push(sessionInfo);
                console.log("[SessionManager] Current invites after:", this.appState.invites.length);
                console.log("[SessionManager] Full invites array:", this.appState.invites);
            }

            // Note: Auto-acceptance for proposers is now handled separately in handleProposerAutoAcceptance()
            // This method should ONLY handle invitations received from other peers

            // Broadcast session update to popup
            console.log("[SessionManager] 📡 Broadcasting session update to popup");
            console.log("[SessionManager] Broadcasting sessionInfo:", this.appState.sessionInfo);
            console.log("[SessionManager] Broadcasting invites count:", this.appState.invites.length);

            this.broadcastToPopup({
                type: "sessionUpdate",
                sessionInfo: this.appState.sessionInfo,
                invites: this.appState.invites
            } as any);

            console.log("[SessionManager] ✅ Session proposal processed and broadcasted to popup");
        } else {
            console.log("[SessionManager] ❌ This peer is not included in session proposal, ignoring");
        }
    }

    /**
     * Handle session response from participants
     */
    handleSessionResponse(fromPeerId: string, responseData: any) {
        console.log("[SessionManager] Processing session response from:", fromPeerId);

        if (!validateWebSocketSessionResponse(responseData)) {
            console.error("[SessionManager] Invalid session response data:", responseData);
            console.error("[SessionManager] Expected: session_id (string), accepted (boolean), websocket_msg_type='SessionResponse'");
            return;
        }

        const { session_id, accepted } = responseData;

        // Find the session
        const session = this.appState.sessionInfo?.session_id === session_id
            ? this.appState.sessionInfo
            : this.appState.invites.find(inv => inv.session_id === session_id);

        if (session) {
            console.log(`[SessionManager] Found session ${session_id}, updating acceptance status`);

            if (accepted) {
                // Add to accepted devices if not already present
                if (!session.accepted_devices.includes(fromPeerId)) {
                    session.accepted_devices.push(fromPeerId);
                    console.log(`[SessionManager] Added ${fromPeerId} to accepted devices`);
                }

                // Check if all participants have accepted
                const allAccepted = session.participants.every(participantId =>
                    session.accepted_devices.includes(participantId)
                );

                if (allAccepted) {
                    console.log("[SessionManager] All participants have accepted the session! Notifying offscreen for mesh readiness.");

                    // Send updated session info to offscreen to trigger mesh readiness check
                    // Only send if sessionInfo is available
                    if (this.appState.sessionInfo) {
                        const sessionAllAcceptedMessage: OffscreenMessage = {
                            type: "sessionAllAccepted",
                            sessionInfo: this.appState.sessionInfo,
                            currentdeviceId: this.appState.deviceId,
                            blockchain: (this.appState.chain === "ethereum" || this.appState.chain === "solana")
                                ? this.appState.chain as "ethereum" | "solana"
                                : "solana" // Default fallback
                        };

                        this.sendToOffscreen(sessionAllAcceptedMessage, "sessionAllAccepted");
                    }
                } else {
                    console.log(`[SessionManager] Not all participants accepted yet.`);

                    // Still send update to offscreen for tracking
                    // Only send if sessionInfo is available
                    if (this.appState.sessionInfo) {
                        const sessionResponseUpdateMessage: OffscreenMessage = {
                            type: "sessionResponseUpdate",
                            sessionInfo: this.appState.sessionInfo,
                            currentdeviceId: this.appState.deviceId
                        };

                        this.sendToOffscreen(sessionResponseUpdateMessage, "sessionResponseUpdate");
                    }
                }
            }

            // Broadcast session update to popup
            this.broadcastToPopup({
                type: "sessionUpdate",
                sessionInfo: this.appState.sessionInfo,
                invites: this.appState.invites
            } as any);

            console.log("[SessionManager] Session response processed and broadcasted");
        } else {
            console.warn("[SessionManager] Received session response for unknown session:", session_id);
        }
    }

    /**
     * Accept a session invitation
     */
    async acceptSession(sessionId: string, blockchain: "ethereum" | "solana" = "solana"): Promise<{ success: boolean; error?: string }> {
        console.log(`[SessionManager] Accepting session: ${sessionId} with blockchain: ${blockchain}`);

        const sessionIndex = this.appState.invites.findIndex(inv => inv.session_id === sessionId);

        if (sessionIndex === -1) {
            return { success: false, error: "Session not found in invites" };
        }

        const session = this.appState.invites[sessionIndex];

        // Store blockchain selection
        this.appState.chain = blockchain;

        // Move session to active and update status
        this.appState.sessionInfo = { ...session, status: "accepted" };
        this.appState.invites.splice(sessionIndex, 1);

        // Persistence removed - using KISS principle

        // Send acceptance message to other participants
        const acceptanceData = {
            websocket_msg_type: "SessionResponse",
            session_id: sessionId,
            accepted: true
        };

        if (this.wsClient?.getReadyState() === WebSocket.OPEN) {
            // Send to all other participants
            const otherParticipants = session.participants.filter(p => p !== this.appState.deviceId);

            try {
                await Promise.all(otherParticipants.map(async (peerId) => {
                    try {
                        await this.wsClient!.relayMessage(peerId, acceptanceData);
                        console.log(`[SessionManager] Session acceptance sent to ${peerId}`);
                    } catch (error) {
                        console.error(`[SessionManager] Failed to send acceptance to ${peerId}:`, error);
                    }
                }));

                console.log("[SessionManager] All session acceptances sent");

                // Forward session info to offscreen for WebRTC setup
                console.log("[SessionManager] Forwarding session info to offscreen for WebRTC setup with blockchain:", blockchain);

                if (this.appState.sessionInfo) {
                    await this.sendToOffscreen({
                        type: "sessionAccepted",
                        sessionInfo: this.appState.sessionInfo,
                        currentdeviceId: this.appState.deviceId,
                        blockchain: blockchain
                    }, "sessionAccepted");
                } else {
                    console.error("[SessionManager] Cannot forward session info: sessionInfo is null");
                }

                return { success: true };
            } catch (error) {
                return { success: false, error: (error as Error).message };
            }
        } else {
            return { success: false, error: "WebSocket not connected" };
        }
    }

    /**
     * Propose a new session
     */
    async proposeSession(
        sessionId: string,
        totalParticipants: number,
        threshold: number,
        participants: string[]
    ): Promise<{ success: boolean; error?: string }> {
        console.log(`[SessionManager] Proposing session: ${sessionId}`);

        if (!this.wsClient || this.wsClient.getReadyState() !== WebSocket.OPEN) {
            return { success: false, error: "WebSocket not connected" };
        }

        const proposalData = {
            websocket_msg_type: "SessionProposal",
            session_id: sessionId,
            proposer_id: this.appState.deviceId,
            participants: participants,
            threshold: threshold,
            total: totalParticipants
        };

        try {
            // Send proposal to all other participants
            const otherParticipants = participants.filter(p => p !== this.appState.deviceId);

            await Promise.all(otherParticipants.map(async (peerId) => {
                try {
                    await this.wsClient!.relayMessage(peerId, proposalData);
                    console.log(`[SessionManager] Session proposal sent to ${peerId}`);
                } catch (error) {
                    console.error(`[SessionManager] Failed to send proposal to ${peerId}:`, error);
                }
            }));

            // Handle our own proposal (auto-accept for proposer)
            this.handleProposerAutoAcceptance(proposalData);

            return { success: true };
        } catch (error) {
            return { success: false, error: (error as Error).message };
        }
    }

    /**
     * Handle proposer auto-acceptance (separate from invitation reception)
     */
    private handleProposerAutoAcceptance(proposalData: any): void {
        console.log("[SessionManager] 🎯 Handling proposer auto-acceptance");

        if (!this.validateWebSocketSessionProposal(proposalData)) {
            console.error("[SessionManager] Invalid proposal data for auto-acceptance:", proposalData);
            return;
        }

        const sessionInfo: SessionInfo = {
            session_id: proposalData.session_id,
            proposer_id: this.appState.deviceId, // We are the proposer
            participants: proposalData.participants || [],
            threshold: proposalData.threshold,
            total: proposalData.total,
            accepted_devices: [this.appState.deviceId], // Proposer automatically accepts
            status: "accepted"
        };

        console.log("[SessionManager] ✅ Proposer auto-accepting session:", sessionInfo.session_id);

        // Set as active session (no invite needed for proposer)
        this.appState.sessionInfo = sessionInfo;

        // Forward to offscreen for WebRTC setup
        this.sendToOffscreen({
            type: "sessionAccepted",
            sessionInfo: this.appState.sessionInfo,
            currentdeviceId: this.appState.deviceId
        }, "proposerWebRTCSetup");

        // Broadcast session update to popup
        console.log("[SessionManager] 📡 Broadcasting proposer session update to popup");
        this.broadcastToPopup({
            type: "sessionUpdate",
            sessionInfo: this.appState.sessionInfo,
            invites: this.appState.invites
        } as any);

        console.log("[SessionManager] ✅ Proposer auto-acceptance completed");
    }
}
