// ===================================================================
// SESSION MANAGEMENT MODULE
// ===================================================================
//
// This module handles MPC session lifecycle management including:
// - Session persistence across extension restarts
// - Session proposal handling
// - Session response processing
// - Session state validation
// ===================================================================

import { AppState } from "@starlab/types/appstate";
import { SessionInfo, SessionProposal, SessionResponse } from "@starlab/types/session";
import { MeshStatus } from "@starlab/types/mesh";
import { DkgState } from "@starlab/types/dkg";
import { WebSocketClient } from "./websocket";
import { validateSessionProposal, validateSessionAcceptance } from "@starlab/types/messages";
import type { BackgroundToPopupMessage, OffscreenMessage } from "@starlab/types/messages";
import { buildWireSessionInfo } from "../../utils/session-parse";

// Session persistence removed - sessions are ephemeral for security

/**
 * Handles MPC session lifecycle and coordination
 */
export class SessionManager {
    private appState: AppState;
    private wsClient: WebSocketClient | null;
    private broadcastToPopup: (message: BackgroundToPopupMessage) => void;
    private sendToOffscreen: (message: OffscreenMessage, description: string) => Promise<{ success: boolean; error?: string }>;
    private stateManager: any; // StateManager reference

    constructor(
        appState: AppState,
        wsClient: WebSocketClient | null,
        broadcastToPopup: (message: BackgroundToPopupMessage) => void,
        sendToOffscreen: (message: OffscreenMessage, description: string) => Promise<{ success: boolean; error?: string }>,
        stateManager?: any
    ) {
        this.appState = appState;
        this.wsClient = wsClient;
        this.broadcastToPopup = broadcastToPopup;
        this.sendToOffscreen = sendToOffscreen;
        this.stateManager = stateManager;
    }

    /**
     * Update the WebSocket client reference (used when WebSocket reconnects)
     */
    updateWebSocketClient(wsClient: WebSocketClient | null): void {
        this.wsClient = wsClient;
        console.log("[SessionManager] WebSocket client reference updated");
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
    async handleSessionProposal(fromPeerId: string, proposalData: any) {
        console.log("[SessionManager] Processing session proposal from:", fromPeerId);
        console.log("[SessionManager] Proposal data:", {
            session_id: proposalData?.session_id,
            total: proposalData?.total,
            threshold: proposalData?.threshold,
            participants: proposalData?.participants,
            websocket_msg_type: proposalData?.websocket_msg_type
        });

        if (!this.validateWebSocketSessionProposal(proposalData)) {
            console.error("[SessionManager] Invalid session proposal data:", proposalData);
            console.error("[SessionManager] Expected: session_id (string), total (number), threshold (number), participants (array), websocket_msg_type='SessionProposal'");
            return;
        }

        // Sort participants to ensure consistent indexing across all peers
        const sortedParticipants = [...(proposalData.participants || [])].sort();
        
        const sessionInfo: SessionInfo = {
            session_id: proposalData.session_id,
            proposer_id: fromPeerId,
            participants: sortedParticipants,
            threshold: proposalData.threshold,
            total: proposalData.total,
            accepted_devices: [fromPeerId], // Proposer automatically accepts
            status: "proposed"
        };

        console.log("[SessionManager] Session proposal validated:", sessionInfo);

        // Get current device ID from StateManager if available
        const currentDeviceId = this.stateManager ? this.stateManager.getState().deviceId : this.appState.deviceId;
        
        console.log("[SessionManager] Checking inclusion - currentDeviceId:", currentDeviceId, "participants:", sessionInfo.participants);
        
        // Check if this peer is included in the session
        if (sessionInfo.participants.includes(currentDeviceId)) {
            console.log("[SessionManager] This peer is included in session proposal");

            // Get current invites from StateManager
            const currentState = this.stateManager ? this.stateManager.getState() : this.appState;
            const invites = [...currentState.invites];
            
            // Check for existing invite
            const existingInviteIndex = invites.findIndex(inv =>
                inv.session_id === sessionInfo.session_id
            );

            if (existingInviteIndex !== -1) {
                console.log("[SessionManager] Updating existing session invite");
                invites[existingInviteIndex] = sessionInfo;
            } else {
                console.log("[SessionManager] Adding new session invite");
                invites.push(sessionInfo);
            }
            
            // Update local state
            this.appState.invites = invites;
            
            // Update StateManager with new invites
            if (this.stateManager) {
                this.stateManager.updateStateProperty('invites', invites);
                // Also update the local appState to sync
                this.appState = this.stateManager.getState();
            }

            // If this peer is the proposer, automatically accept and set up WebRTC
            if (fromPeerId === currentDeviceId) {
                console.log("[SessionManager] This peer is the proposer, auto-accepting and setting up WebRTC");

                const acceptedSessionInfo = { ...sessionInfo, status: "accepted" as const };
                const updatedInvites = invites.filter(inv => inv.session_id !== sessionInfo.session_id);
                
                // Update local state
                this.appState.sessionInfo = acceptedSessionInfo;
                this.appState.invites = updatedInvites;
                
                // Update StateManager with session changes
                if (this.stateManager) {
                    this.stateManager.updateState({
                        sessionInfo: acceptedSessionInfo,
                        invites: updatedInvites
                    });
                    // Sync local state
                    this.appState = this.stateManager.getState();
                }

                // No persistence - sessions are ephemeral

                // Forward to offscreen for WebRTC setup. We only
                // reach here when a session was successfully set on
                // appState (createSession → updateState); non-null
                // assertion is sound in this branch.
                this.sendToOffscreen({
                    type: "sessionAccepted",
                    sessionInfo: this.appState.sessionInfo!,
                    currentdeviceId: this.appState.deviceId,
                    blockchain: this.appState.blockchain || "solana"
                }, "proposerWebRTCSetup");
            }

            // Broadcast session update to popup
            this.broadcastToPopup({
                type: "sessionUpdate",
                sessionInfo: this.appState.sessionInfo,
                invites: this.appState.invites
            } as any);

            console.log("[SessionManager] Session proposal processed and broadcasted to popup");
        } else {
            console.log("[SessionManager] This peer is not included in session proposal, ignoring");
        }
    }

    /**
     * Handle session response from participants
     */
    handleSessionResponse(fromPeerId: string, responseData: any) {
        console.log("[SessionManager] Processing session response from:", fromPeerId);

        // Validate WebSocket session response data
        if (!responseData || 
            typeof responseData.session_id !== 'string' || 
            typeof responseData.accepted !== 'boolean' ||
            responseData.websocket_msg_type !== 'SessionResponse') {
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

                // Update the sessionInfo in appState if this is the active session
                if (this.appState.sessionInfo && this.appState.sessionInfo.session_id === session_id) {
                    this.appState.sessionInfo = { ...session };
                    if (this.stateManager) {
                        this.stateManager.updateStateProperty('sessionInfo', this.appState.sessionInfo);
                    }
                }
                
                if (allAccepted) {
                    console.log("[SessionManager] All participants have accepted the session! Notifying offscreen for mesh readiness.");

                    // Send updated session info to offscreen to trigger mesh readiness check
                    // Use the updated session object, not the potentially stale appState.sessionInfo
                    const sessionAllAcceptedMessage: OffscreenMessage = {
                        type: "sessionAllAccepted",
                        sessionInfo: session,
                        currentdeviceId: this.stateManager ? this.stateManager.getState().deviceId : this.appState.deviceId,
                        blockchain: this.appState.blockchain || "solana" // Use stored blockchain or default to solana
                    };

                    this.sendToOffscreen(sessionAllAcceptedMessage, "sessionAllAccepted");
                } else {
                    console.log(`[SessionManager] Not all participants accepted yet.`);

                    // Still send update to offscreen for tracking
                    const sessionResponseUpdateMessage: OffscreenMessage = {
                        type: "sessionResponseUpdate",
                        sessionInfo: session,
                        currentdeviceId: this.stateManager ? this.stateManager.getState().deviceId : this.appState.deviceId
                    };

                    this.sendToOffscreen(sessionResponseUpdateMessage, "sessionResponseUpdate");
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

        // Get current state from StateManager
        const currentState = this.stateManager ? this.stateManager.getState() : this.appState;
        const invites = [...currentState.invites];
        
        const sessionIndex = invites.findIndex(inv => inv.session_id === sessionId);

        if (sessionIndex === -1) {
            console.error(`[SessionManager] Session ${sessionId} not found in invites:`, invites);
            return { success: false, error: "Session not found in invites" };
        }

        const session = invites[sessionIndex];
        console.log(`[SessionManager] Found session to accept:`, session);

        // Store blockchain selection
        this.appState.blockchain = blockchain;

        // Get current device ID
        const currentDeviceId = this.stateManager ? this.stateManager.getState().deviceId : this.appState.deviceId;
        
        // Move session to active and update status, adding current device to accepted_devices
        const newSessionInfo = { 
            ...session, 
            status: "accepted" as const,
            accepted_devices: [...new Set([...session.accepted_devices, currentDeviceId])] // Add current device and dedupe
        };
        invites.splice(sessionIndex, 1);
        
        // Update local state
        this.appState.sessionInfo = newSessionInfo;
        this.appState.invites = invites;
        
        // Update StateManager with session changes
        if (this.stateManager) {
            this.stateManager.updateState({
                sessionInfo: newSessionInfo,
                invites: invites,
                blockchain: blockchain
            });
            // Sync local state with StateManager
            this.appState = this.stateManager.getState();
        }

        // No persistence - sessions are ephemeral

        // Send acceptance message to other participants
        const acceptanceData = {
            websocket_msg_type: "SessionResponse",
            session_id: sessionId,
            accepted: true
        };

        if (this.wsClient?.getReadyState() === WebSocket.OPEN) {
            // Send to all other participants
            const currentDeviceId = this.stateManager ? this.stateManager.getState().deviceId : this.appState.deviceId;
            const otherParticipants = newSessionInfo.participants.filter((p: string) => p !== currentDeviceId);

            try {
                await Promise.all(otherParticipants.map(async (peerId: string) => {
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

                await this.sendToOffscreen({
                    type: "sessionAccepted",
                    sessionInfo: newSessionInfo,
                    currentdeviceId: this.stateManager ? this.stateManager.getState().deviceId : this.appState.deviceId,
                    blockchain: blockchain
                }, "sessionAccepted");
                
                // Broadcast session update to popup to ensure UI updates
                this.broadcastToPopup({
                    type: "sessionUpdate",
                    sessionInfo: newSessionInfo,
                    invites: invites
                } as any);

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
        participants: string[],
        blockchain: "ethereum" | "solana" = "solana"
    ): Promise<{ success: boolean; error?: string }> {
        console.log(`[SessionManager] Proposing session: ${sessionId} with blockchain: ${blockchain}`);

        if (!this.wsClient || this.wsClient.getReadyState() !== WebSocket.OPEN) {
            return { success: false, error: "WebSocket not connected" };
        }

        const currentDeviceId = this.stateManager ? this.stateManager.getState().deviceId : this.appState.deviceId;
        
        // Store blockchain selection
        this.appState.blockchain = blockchain;
        if (this.stateManager) {
            this.stateManager.updateState({ blockchain });
        }
        
        // Sort participants to ensure consistent indexing across all peers
        const sortedParticipants = [...participants].sort();
        
        const proposalData = {
            websocket_msg_type: "SessionProposal",
            session_id: sessionId,
            proposer_id: currentDeviceId,
            participants: sortedParticipants,
            threshold: threshold,
            total: totalParticipants
        };

        try {
            // Send proposal to all other participants
            const otherParticipants = participants.filter(p => p !== currentDeviceId);

            await Promise.all(otherParticipants.map(async (peerId) => {
                try {
                    await this.wsClient!.relayMessage(peerId, proposalData);
                    console.log(`[SessionManager] Session proposal sent to ${peerId}`);
                } catch (error) {
                    console.error(`[SessionManager] Failed to send proposal to ${peerId}:`, error);
                }
            }));

            // Handle our own proposal (auto-accept for proposer)
            await this.handleSessionProposal(currentDeviceId, proposalData);

            return { success: true };
        } catch (error) {
            return { success: false, error: (error as Error).message };
        }
    }

    /**
     * Ext-1b: create a new MPC wallet via DKG using the TUI-compatible
     * announcement channel. Unlike `proposeSession` (which relays a
     * `SessionProposal` to pre-selected peers), this broadcasts via
     * `announce_session` — any client on the signal server can discover
     * it and join up to the threshold.
     *
     * Steps:
     *   1. Build a TUI-shaped SessionInfo with `participants: [self]`
     *      (joiners append themselves when they accept).
     *   2. Store locally as `appState.sessionInfo` + set
     *      `dkgState = DkgState.Initializing` so the UI reflects the
     *      creator-state.
     *   3. Broadcast via `wsClient.announceSession(buildWireSessionInfo(...))`.
     *
     * Returns the generated `session_id` so the popup can show "waiting
     * for joiners on session dkg_XXX…".
     */
    async createDkgWallet(config: {
        name?: string;
        total: number;
        threshold: number;
        curve: "secp256k1" | "ed25519";
    }): Promise<{ success: boolean; sessionId?: string; error?: string }> {
        if (!this.wsClient || this.wsClient.getReadyState() !== WebSocket.OPEN) {
            return { success: false, error: "WebSocket not connected" };
        }
        if (!Number.isInteger(config.total) || config.total < 2) {
            return { success: false, error: "total must be ≥2" };
        }
        if (
            !Number.isInteger(config.threshold) ||
            config.threshold < 1 ||
            config.threshold > config.total
        ) {
            return { success: false, error: "threshold must be 1..total" };
        }

        const currentDeviceId = this.stateManager
            ? this.stateManager.getState().deviceId
            : this.appState.deviceId;

        // Session id mirrors TUI's convention — loosely: `dkg_<12-hex>`
        // (TUI uses a 4-hex suffix, which collides too easily; widen
        // here. TUI still parses the id as an opaque string so length
        // doesn't matter to it.)
        const sessionId = `dkg_${cryptoHex(12)}`;

        const sessionInfo: SessionInfo = {
            session_id: sessionId,
            proposer_id: currentDeviceId,
            total: config.total,
            threshold: config.threshold,
            participants: [currentDeviceId],
            session_type: "dkg",
            curve_type: config.curve,
            coordination_type: "Network",
            accepted_devices: [currentDeviceId],
        };

        // Mirror TUI's `creating_wallet` state: the popup needs to
        // know we're the creator so it shows "Waiting for joiners"
        // rather than "Session received".
        this.appState.sessionInfo = sessionInfo;
        this.appState.invites = [
            ...(this.appState.invites ?? []).filter(
                (s) => s.session_id !== sessionId,
            ),
            sessionInfo,
        ];
        this.appState.dkgState = DkgState.Initializing;
        if (this.stateManager) {
            this.stateManager.updateState({
                sessionInfo,
                invites: this.appState.invites,
                dkgState: DkgState.Initializing,
                blockchain:
                    config.curve === "secp256k1" ? "ethereum" : "solana",
            });
        }

        try {
            const wire = buildWireSessionInfo(sessionInfo);
            this.wsClient.announceSession(wire);
            console.log(
                `[SessionManager] Created DKG wallet: ${sessionId} ` +
                    `(${config.threshold}/${config.total} ${config.curve})`,
            );
            this.broadcastToPopup({
                type: "dkgWalletCreated",
                sessionInfo,
            } as any);
            return { success: true, sessionId };
        } catch (error) {
            console.error("[SessionManager] createDkgWallet failed:", error);
            // Roll back state on failure so the user can retry cleanly.
            this.appState.sessionInfo = null;
            this.appState.dkgState = DkgState.Idle;
            if (this.stateManager) {
                this.stateManager.updateState({
                    sessionInfo: null,
                    dkgState: DkgState.Idle,
                });
            }
            return { success: false, error: (error as Error).message };
        }
    }

    /**
     * Ext-1e: accept a DKG session that arrived via
     * `session_available` discovery. Mirrors TUI's `JoinDKG` command:
     *   1. Look up the invite (authoritative copy carries the
     *      creator's threshold/total/curve — we inherit them rather
     *      than trust the UI's config).
     *   2. Sanity check: we're not the proposer (creators join
     *      implicitly via createDkgWallet), we're not already in
     *      `participants` (idempotent), there's room for one more
     *      participant.
     *   3. Stash as our own `sessionInfo` and push dkgState to
     *      Initializing so the popup's wallet-status-banner shows
     *      the ceremony in progress.
     *   4. Emit `session_status_update` — server appends us to the
     *      participants list and broadcasts a refreshed
     *      `session_available` to everyone.
     *
     * Does NOT start FROST rounds. Ceremony kickoff is the next
     * wire-up (Ext-1e full) — once total participants === total,
     * everyone has enough info to init DkgManager and start WebRTC
     * mesh setup.
     */
    async joinDkgSession(sessionId: string): Promise<{
        success: boolean;
        sessionInfo?: SessionInfo;
        error?: string;
    }> {
        if (!this.wsClient || this.wsClient.getReadyState() !== WebSocket.OPEN) {
            return { success: false, error: "WebSocket not connected" };
        }
        const invite = (this.appState.invites ?? []).find(
            (s) => s.session_id === sessionId,
        );
        if (!invite) {
            return {
                success: false,
                error: `No known invite for session ${sessionId}`,
            };
        }
        const currentDeviceId = this.stateManager
            ? this.stateManager.getState().deviceId
            : this.appState.deviceId;
        if (!currentDeviceId) {
            return { success: false, error: "Device id not set yet" };
        }
        if (invite.proposer_id === currentDeviceId) {
            return {
                success: false,
                error: "You're the proposer of this session — no need to join",
            };
        }
        // Already a participant? Still succeed (idempotent) but
        // don't re-emit the status update.
        const alreadyJoined = invite.participants.includes(currentDeviceId);
        if (
            !alreadyJoined &&
            invite.participants.length >= invite.total
        ) {
            return {
                success: false,
                error: "Session is full",
            };
        }

        // Inherit authoritative session_info from the creator's
        // announcement. We optimistically append ourselves locally
        // so the popup reflects the joined state before the
        // server echoes the update; the echo will overwrite this
        // entry with the canonical `participants` list from server.
        const newParticipants = alreadyJoined
            ? [...invite.participants]
            : [...invite.participants, currentDeviceId];
        const local: SessionInfo = {
            ...invite,
            participants: newParticipants,
            accepted_devices: Array.from(
                new Set([...(invite.accepted_devices ?? []), currentDeviceId]),
            ),
        };
        this.appState.sessionInfo = local;
        this.appState.dkgState = DkgState.Initializing;
        if (this.stateManager) {
            this.stateManager.updateState({
                sessionInfo: local,
                dkgState: DkgState.Initializing,
                blockchain:
                    (local.curve_type ?? "secp256k1") === "ed25519"
                        ? "solana"
                        : "ethereum",
            });
        }

        try {
            if (!alreadyJoined) {
                this.wsClient.sendSessionStatusUpdate(sessionId, currentDeviceId);
                console.log(
                    `[SessionManager] Joined DKG session ${sessionId} as ${currentDeviceId}`,
                );
            } else {
                console.log(
                    `[SessionManager] Already in session ${sessionId}, no update sent`,
                );
            }
            this.broadcastToPopup({
                type: "dkgSessionJoined",
                sessionInfo: local,
            } as any);
            return { success: true, sessionInfo: local };
        } catch (err) {
            // Roll back local state on wire failure so the UI doesn't
            // show a false "joined" state.
            this.appState.sessionInfo = null;
            this.appState.dkgState = DkgState.Idle;
            if (this.stateManager) {
                this.stateManager.updateState({
                    sessionInfo: null,
                    dkgState: DkgState.Idle,
                });
            }
            return { success: false, error: (err as Error).message };
        }
    }

    /**
     * Ext-2: announce a threshold-signing ceremony for an existing
     * wallet. Mirror of `createDkgWallet` but on the signing side of
     * the TUI protocol:
     *
     *   - session_type: "signing" (flat string, matches TUI wire)
     *   - wallet_name, group_public_key, blockchain, and
     *     signing_message_hex filled in top-level per TUI's parser
     *     (command.rs:286 line, signing-sibling fields).
     *
     * For secp256k1 wallets, the `message` parameter is hashed with
     * EIP-191 before going on the wire — the signature must be
     * ecrecover-compatible so dApps can verify it as a standard
     * personal_sign. For ed25519, raw bytes are signed (Ed25519
     * signs variable-length input natively).
     *
     * Does NOT actually run FROST rounds — that's the signing
     * auto-trigger (Ext-2d). This commit just announces.
     */
    async createSigningSession(config: {
        walletId: string;
        walletName: string;
        groupPublicKey: string;
        blockchain: "ethereum" | "solana";
        threshold: number;
        total: number;
        /** Hex-encoded bytes that FROST will sign. Caller is
         *  responsible for EIP-191 wrapping if appropriate. */
        signingMessageHex: string;
    }): Promise<{ success: boolean; sessionId?: string; error?: string }> {
        if (!this.wsClient || this.wsClient.getReadyState() !== WebSocket.OPEN) {
            return { success: false, error: "WebSocket not connected" };
        }
        if (!config.walletId || !config.groupPublicKey) {
            return { success: false, error: "walletId and groupPublicKey required" };
        }
        if (!/^[0-9a-fA-F]*$/.test(config.signingMessageHex)) {
            return { success: false, error: "signingMessageHex must be hex" };
        }

        const currentDeviceId = this.stateManager
            ? this.stateManager.getState().deviceId
            : this.appState.deviceId;
        const sessionId = `sign_${cryptoHex(12)}`;
        const curveType =
            config.blockchain === "ethereum" ? "secp256k1" : "ed25519";

        const sessionInfo: SessionInfo = {
            session_id: sessionId,
            proposer_id: currentDeviceId,
            total: config.total,
            threshold: config.threshold,
            participants: [currentDeviceId],
            session_type: "signing",
            curve_type: curveType,
            coordination_type: "Network",
            // Signing-specific top-level siblings — TUI parser reads
            // these at the top level (not nested).
            wallet_name: config.walletName,
            group_public_key: config.groupPublicKey,
            blockchain: config.blockchain,
            signing_message_hex: config.signingMessageHex,
            accepted_devices: [currentDeviceId],
        };

        // Stash as active session so the popup's ceremony banner
        // shows the signing in progress. Same dkgState enum is
        // reused because offscreen's WebRTC+SigningManager keys off
        // it (dkgState is a misnomer — it tracks ANY in-progress
        // ceremony).
        this.appState.sessionInfo = sessionInfo;
        this.appState.invites = [
            ...(this.appState.invites ?? []).filter(
                (s) => s.session_id !== sessionId,
            ),
            sessionInfo,
        ];
        this.appState.dkgState = DkgState.Initializing;
        if (this.stateManager) {
            this.stateManager.updateState({
                sessionInfo,
                invites: this.appState.invites,
                dkgState: DkgState.Initializing,
                blockchain: config.blockchain,
            });
        }

        try {
            const wire = buildWireSessionInfo(sessionInfo);
            this.wsClient.announceSession(wire);
            console.log(
                `[SessionManager] Created signing session ${sessionId} for wallet ${config.walletId} (${config.threshold}/${config.total} ${curveType})`,
            );
            this.broadcastToPopup({
                type: "signingSessionCreated",
                sessionInfo,
            } as any);
            return { success: true, sessionId };
        } catch (error) {
            console.error(
                "[SessionManager] createSigningSession failed:",
                error,
            );
            this.appState.sessionInfo = null;
            this.appState.dkgState = DkgState.Idle;
            if (this.stateManager) {
                this.stateManager.updateState({
                    sessionInfo: null,
                    dkgState: DkgState.Idle,
                });
            }
            return { success: false, error: (error as Error).message };
        }
    }
}

/**
 * 12-char hex helper for session ids. Uses WebCrypto — available in
 * service workers and offscreen documents. Not cryptographically
 * strong (we don't need that for session ids), just collision-resistant
 * enough for "a few thousand sessions".
 */
function cryptoHex(bytes: number): string {
    const buf = new Uint8Array(bytes);
    crypto.getRandomValues(buf);
    let out = "";
    for (const b of buf) out += b.toString(16).padStart(2, "0");
    return out;
}
