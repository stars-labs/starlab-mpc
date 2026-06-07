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
import { AppState } from "@mpc-wallet/types/appstate";
import type { SessionInfo } from "@mpc-wallet/types/session";
import { SessionManager } from "./sessionManager";
import { SigningNotifier } from "./signingNotification";
import { getSignalServerUrl } from "../../config/signal-server";
import { parseSessionInfoFromWire } from "../../utils/session-parse";
import type {
    BackgroundToPopupMessage,
    InitialStateMessage,
    OffscreenMessage
} from "@mpc-wallet/types/messages";
import { ServerMsg, WebSocketMessagePayload } from "@mpc-wallet/types/websocket";

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
    /**
     * The URL we actually called `initialize` with. Kept so the status
     * reporter can surface the real endpoint instead of a hardcoded
     * string that drifts whenever the default changes.
     */
    private connectedUrl: string | undefined;
    /**
     * session_id of the last session we fired `sessionAllAccepted` for.
     * Guards against re-dispatching on server re-broadcasts (which
     * can arrive multiple times if peers reconnect). Reset is implicit:
     * a new session gets a fresh id, so the check naturally passes.
     * Cleared to undefined on session clear / NavigateHome-equivalent
     * flows via `clearDkgTriggerFor`.
     */
    private dkgTriggerFiredFor: string | undefined;
    /**
     * Ext-3a: emits chrome.notifications on signing invites we're a
     * participant in. Injected (not constructed inline) so tests can
     * stub chrome.notifications. Null means "notifications disabled"
     * — chrome.notifications is undefined in service worker contexts
     * without the permission, and in bun tests.
     */
    private signingNotifier: SigningNotifier | null = null;

    constructor(
        appState: AppState,
        sessionManager: SessionManager,
        broadcastToPopup: (message: BackgroundToPopupMessage) => void,
        sendToOffscreen: (message: OffscreenMessage, description: string) => Promise<{ success: boolean; error?: string }>,
        stateManager?: any, // Optional StateManager for persistence
        signingNotifier?: SigningNotifier,
    ) {
        this.appState = appState;
        this.sessionManager = sessionManager;
        this.broadcastToPopup = broadcastToPopup;
        this.sendToOffscreen = sendToOffscreen;
        this.stateManager = stateManager;
        this.signingNotifier = signingNotifier ?? null;
    }

    /**
     * Update the session manager reference (used when session is restored)
     */
    updateSessionManager(sessionManager: SessionManager): void {
        this.sessionManager = sessionManager;
        console.log("[WebSocketManager] Session manager reference updated");
    }

    /**
     * Initialize WebSocket connection with URL and device ID
     */
    async initialize(url: string, deviceId: string): Promise<void> {
        try {
            this.connectedUrl = url;
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

            // Also broadcast updated state
            const initErrorState: InitialStateMessage = {
                type: "initialState",
                ...this.appState
            };
            this.broadcastToPopup(initErrorState as any);
        }
    }

    /**
     * Initialize WebSocket connection (legacy method)
     */
    async initializeWebSocket(): Promise<void> {
        // Legacy entry point without explicit URL/device id. Resolve
        // the URL from config so this path matches the main
        // initializeWebSocket() in background/index.ts.
        const url = await getSignalServerUrl();
        return this.initialize(url, "mpc-2");
    }

    /**
     * Tear down the current connection (if any) and reconnect using a freshly
     * resolved signal-server URL. This is how a room / signal-server override
     * saved AFTER startup takes effect: the startup connect runs before any room
     * is set, so it dials a roomless URL the multi-tenant server rejects (#31).
     * Re-resolving here picks up the saved `?room=…` so the socket is accepted.
     * Wired to the popup's "Save room" action (Settings) and used by the L3c
     * interop harness so the extension joins the co-signers' room (#33).
     */
    async reconnect(): Promise<void> {
        console.log("[WebSocketManager] reconnect requested — re-resolving URL with saved room");
        try {
            this.wsClient?.disconnect();
        } catch (e) {
            console.warn("[WebSocketManager] reconnect: error closing existing client:", e);
        }
        const url = await getSignalServerUrl();
        await this.initialize(url, this.appState.deviceId || "mpc-2");
    }

    /**
     * Set up WebSocket event handlers
     */
    private setupEventHandlers(): void {
        if (!this.wsClient) return;

        // Capture the client these handlers belong to. After a reconnect() the
        // superseded client's close/error fires asynchronously (the close
        // handshake outlives `disconnect()`), which would clobber the NEW
        // connection's wsConnected=true back to false. Ignore any event whose
        // client is no longer the active one. (#33)
        const client = this.wsClient;
        const isStale = () => this.wsClient !== client;

        console.log("[WebSocketManager] Setting up WebSocket event handlers");

        // Handle connection open
        this.wsClient.onOpen(() => {
            if (isStale()) return;
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

            // Also broadcast updated full state
            const stateUpdate: InitialStateMessage = {
                type: "initialState",
                ...this.appState
            };
            console.log("[WebSocketManager] Broadcasting full state update:", stateUpdate);
            this.broadcastToPopup(stateUpdate as any);

            // Update SessionManager's WebSocket client reference
            if (this.sessionManager) {
                this.sessionManager.updateWebSocketClient(this.wsClient);
            }

            // Register with server
            console.log("[WebSocketManager] Registering with server as peer:", this.appState.deviceId);
            try {
                this.wsClient!.register(this.appState.deviceId);
                console.log("[WebSocketManager] Registration sent to server");
            } catch (regError) {
                console.error("[WebSocketManager] Error during registration:", regError);
            }

            // Request initial peer list with delay and retry logic
            let retryCount = 0;
            const maxRetries = 3;
            const requestDeviceList = () => {
                console.log(`[WebSocketManager] Requesting peer list from server (attempt ${retryCount + 1}/${maxRetries})`);
                if (this.wsClient && this.wsClient.getReadyState() === WebSocket.OPEN) {
                    this.wsClient.listdevices();
                    console.log("[WebSocketManager] Peer list request sent successfully");
                } else if (retryCount < maxRetries - 1) {
                    retryCount++;
                    console.warn("[WebSocketManager] WebSocket not ready, retrying in 1 second...");
                    setTimeout(requestDeviceList, 1000);
                } else {
                    console.error("[WebSocketManager] Failed to request peer list after all retries");
                }
            };
            
            // Initial delay to ensure registration is processed
            setTimeout(requestDeviceList, 1500); // 1.5 second initial delay

            // Cold-start session discovery (Ext-1a+): the
            // `session_available` channel is a broadcast — only
            // clients already connected at announcement time see
            // it. If a TUI node ran `announce_session` before the
            // extension's popup opened, we'd be blind to that
            // session without an explicit replay query. Mirrors
            // TUI's ws_runtime.rs which fires `RequestActiveSessions`
            // on connect. The server answers with
            // `sessions_for_device` (array of session_info blobs),
            // which our incoming handler routes through
            // `handleSessionAvailable` per entry.
            //
            // Fire-and-forget; sent after a slightly longer delay
            // than the peer-list request so both don't race on
            // serialization order (the server processes in arrival
            // order, but peer-list arriving first gives the popup
            // consistent "devices first, sessions next" UX).
            setTimeout(() => {
                try {
                    if (
                        this.wsClient &&
                        this.wsClient.getReadyState() === WebSocket.OPEN
                    ) {
                        this.wsClient.requestActiveSessions();
                        console.log(
                            "[WebSocketManager] request_active_sessions sent — cold-start replay",
                        );
                    }
                } catch (err) {
                    console.warn(
                        "[WebSocketManager] request_active_sessions failed:",
                        err,
                    );
                }
            }, 2000);
        });

        // Handle connection close
        this.wsClient.onClose((event) => {
            if (isStale()) {
                console.log("[WebSocketManager] Ignoring onClose from a superseded client (post-reconnect)");
                return;
            }
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

            // Also broadcast updated state
            const disconnectedState: InitialStateMessage = {
                type: "initialState",
                ...this.appState
            };
            this.broadcastToPopup(disconnectedState as any);
        });

        // Handle connection errors
        this.wsClient.onError((error) => {
            if (isStale()) {
                console.log("[WebSocketManager] Ignoring onError from a superseded client (post-reconnect)");
                return;
            }
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

            // Also broadcast updated state
            const errorState: InitialStateMessage = {
                type: "initialState",
                ...this.appState
            };
            this.broadcastToPopup(errorState as any);
        });

        // Set up the message handler
        this.wsClient.onMessage((message: any) => {
            if (isStale()) return;
            this.handleWebSocketMessage(message);
        });
    }

    /**
     * Ext-3c: send a peer-to-peer relay via the signal server.
     * Used for SigningDecline delivery (co-signer rejecting an
     * invite before WebRTC mesh exists, so we can't use data
     * channels). The wire shape matches what the server's `relay`
     * handler expects — server forwards to `to` as a `relay` frame
     * with `from` stamped in.
     *
     * Returns true if we dispatched the send, false if WebSocket
     * isn't connected.
     */
    public relayToPeer(toDeviceId: string, data: any): boolean {
        if (!this.wsClient || this.wsClient.getReadyState() !== WebSocket.OPEN) {
            console.warn(
                "[WebSocketManager] Cannot relay to peer: WebSocket not open",
            );
            return false;
        }
        this.wsClient.relayMessage(toDeviceId, data);
        return true;
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
        switch (serverMessage.type) {
            case "devices": // Handle lowercase "devices" messages from server
                this.handleDeviceListMessage(serverMessage as ServerMsg & { type: "devices" }, serverMessage.type);
                break;

            case "relay": // Handle lowercase "relay" messages from server
                this.handleRelayMessage(serverMessage as ServerMsg & { type: "relay" }, serverMessage.type);
                break;

            case "error":
                this.handleErrorMessage(serverMessage as ServerMsg & { type: "error" });
                break;

            // Session-discovery channel shared with TUI (Ext-1a). The
            // server broadcasts these whenever any client (extension or
            // TUI) emits `announce_session`, so this is the ONLY
            // incoming path that picks up TUI-originated DKG/signing
            // invites. Previously the extension silently dropped these
            // frames in the `default` arm.
            case "session_available":
                this.handleSessionAvailable(serverMessage);
                break;

            case "session_removed":
                this.handleSessionRemoved(serverMessage);
                break;

            case "sessions_for_device":
                // Bulk reply to `request_active_sessions` — same merge
                // semantics as session_available, per session in the
                // list. Tolerant to empty/malformed entries.
                if (Array.isArray((serverMessage as any).sessions)) {
                    for (const raw of (serverMessage as any).sessions) {
                        this.handleSessionAvailable({ type: "session_available", session_info: raw } as any);
                    }
                }
                break;

            default:
                console.log("[WebSocketManager] Unhandled WebSocket message type:", (serverMessage as any).type);
                break;
        }
    }

    /**
     * A peer (TUI or extension) announced a session. Server broadcast
     * delivered it to us as `session_available`. Parse tolerantly —
     * TUI's wire format omits `accepted_devices`; the parser
     * synthesises `[]` so downstream code can always index it.
     */
    private handleSessionAvailable(
        msg: ServerMsg & { type: "session_available" },
    ): void {
        const parsed = parseSessionInfoFromWire((msg as any).session_info);
        if (!parsed) {
            console.warn(
                "[WebSocketManager] Dropped malformed session_available payload:",
                msg,
            );
            return;
        }
        console.log(
            `[WebSocketManager] session_available: ${parsed.session_id} (${parsed.session_type ?? "dkg"}, ${parsed.threshold}/${parsed.total}, proposer=${parsed.proposer_id})`,
        );

        // Merge-update into invites: replace if we already have this
        // session_id (e.g. status_update), else append.
        const invites = this.appState.invites ?? [];
        const idx = invites.findIndex((s) => s.session_id === parsed.session_id);
        if (idx >= 0) {
            invites[idx] = parsed;
        } else {
            invites.push(parsed);
        }
        this.appState.invites = invites;

        // Notify popup so the Join Session tab refreshes.
        this.broadcastToPopup({ type: "sessionAvailable", session: parsed } as any);
        if (this.stateManager?.updateInvites) {
            this.stateManager.updateInvites(invites);
        }

        // Ext-3a: desktop push-notification for signing invites we're
        // a co-signer on. No-op for DKG sessions, for sessions we're
        // proposing, or for sessions we've already notified for.
        // Skipped silently when `signingNotifier` isn't wired (tests,
        // or manifests without the notifications permission).
        if (this.signingNotifier) {
            this.signingNotifier.maybeNotify(parsed, this.appState.deviceId);
        }

        // Ext-1c + Ext-2d auto-trigger: a session update that brings
        // participants.length up to the "ready" threshold means we
        // can kick off the WebRTC mesh + FROST ceremony.
        //
        // Gate differs by session_type:
        //   - DKG (default): exactly N-of-N joined (`=== total`).
        //   - Signing: at least threshold signers joined (`>= threshold`).
        //     FROST signing only needs threshold cosigners, not total;
        //     waiting for all N would stall if a non-signing
        //     participant is offline. Matches TUI's signing flow
        //     (53c2f16) where mesh setup fires as peers join up to
        //     threshold.
        //
        // Common gate conditions (both ceremonies):
        //   1. This session is OUR active session (we're the
        //      creator OR we previously joined). Avoids triggering
        //      for sessions we haven't committed to.
        //   2. We're listed as a participant (belt-and-suspenders
        //      given #1).
        //   3. Dedup by session_id in `dkgTriggerFiredFor` (same
        //      field reused for signing; one trigger per session).
        this.maybeTriggerCeremony(parsed);
    }

    /**
     * Fire the appropriate "ready to start" event to offscreen when
     * a session reaches its ceremony-kickoff threshold. Idempotent
     * per-session-id via `dkgTriggerFiredFor` (the name dates from
     * when only DKG was supported — still accurate as a "we've
     * fired the ceremony trigger for this session" marker).
     *
     *   - DKG (session_type "dkg" or absent): wait for all N.
     *     Fires `sessionAllAccepted` — offscreen then does
     *     setBlockchain + updateSessionInfo + checkAndTriggerDkg
     *     (starts FROST DKG round 1).
     *   - Signing (session_type "signing"): wait for threshold.
     *     Fires `sessionReadyForSigning` — offscreen will load the
     *     keystore for `wallet_id`, set up mesh, and kick off FROST
     *     signing round 1. (Offscreen handler landed separately in
     *     the offscreen-signing wiring.)
     */
    private maybeTriggerCeremony(session: SessionInfo): void {
        const mySessionId = this.appState.sessionInfo?.session_id;
        if (!mySessionId || mySessionId !== session.session_id) {
            return; // Not our session.
        }
        const deviceId = this.appState.deviceId;
        if (!deviceId || !session.participants.includes(deviceId)) {
            return; // We're not a listed participant.
        }
        if (this.dkgTriggerFiredFor === session.session_id) {
            return; // Already fired for this exact session.
        }

        const sessionType = session.session_type ?? "dkg";
        const joined = session.participants.length;

        // Gate by session type — signing threshold vs DKG total.
        if (sessionType === "signing") {
            if (joined < session.threshold) {
                return; // Not enough signers yet.
            }
        } else {
            if (joined !== session.total) {
                return; // DKG needs all N.
            }
        }

        this.dkgTriggerFiredFor = session.session_id;

        // The offscreen handlers expect `accepted_devices` to be
        // populated (used for "all accepted" checks). TUI's wire
        // format doesn't carry this — `parseSessionInfoFromWire`
        // synthesizes `[]`. In the TUI-compat flow, being in
        // `participants` IS being accepted — fill it in here.
        const sessionWithAccepted: SessionInfo = {
            ...session,
            accepted_devices: [...session.participants],
        };

        const blockchain =
            (session.curve_type ?? "secp256k1") === "ed25519"
                ? "solana"
                : "ethereum";

        if (sessionType === "signing") {
            console.log(
                `[WebSocketManager] 🖋️  Signing threshold reached (${joined}/${session.threshold}) for session ${session.session_id} — triggering signing ceremony (${blockchain})`,
            );
            void this.sendToOffscreen(
                {
                    type: "sessionReadyForSigning",
                    sessionInfo: sessionWithAccepted,
                    blockchain,
                } as any,
                `sessionReadyForSigning(${session.session_id})`,
            );
            return;
        }

        console.log(
            `[WebSocketManager] 🎉 All ${session.total} participants joined session ${session.session_id} — triggering DKG (${blockchain})`,
        );

        // Fire-and-forget. The offscreen response isn't awaited
        // here because the Svelte UI will pick up ceremony
        // progress via dkgState updates flowing back through
        // StateManager.
        void this.sendToOffscreen(
            {
                type: "sessionAllAccepted",
                sessionInfo: sessionWithAccepted,
                blockchain,
            } as any,
            `sessionAllAccepted(${session.session_id})`,
        );
    }

    /**
     * A previously announced session was withdrawn (creator cancelled
     * or server garbage-collected it). Drop it from the invite list
     * so stale rows don't stay on screen.
     */
    private handleSessionRemoved(
        msg: ServerMsg & { type: "session_removed" },
    ): void {
        const sessionId = (msg as any).session_id as string | undefined;
        if (!sessionId) return;
        const before = this.appState.invites?.length ?? 0;
        this.appState.invites = (this.appState.invites ?? []).filter(
            (s) => s.session_id !== sessionId,
        );
        if ((this.appState.invites?.length ?? 0) !== before) {
            console.log(`[WebSocketManager] session_removed: ${sessionId}`);
            this.broadcastToPopup({ type: "sessionRemoved", sessionId } as any);
            if (this.stateManager?.updateInvites) {
                this.stateManager.updateInvites(this.appState.invites);
            }
        }
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
            // StateManager will handle filtering, persistence, and broadcasting
            this.stateManager.updateConnectedDevices(deviceList);
        } else {
            // Fallback if no StateManager (shouldn't happen in normal operation)
            this.appState.connecteddevices = connectedDevices;
            // Only broadcast device list update, not full state
            this.broadcastToPopup({ type: "deviceList", devices: connectedDevices });
        }

        console.log(`[WebSocketManager] Updated peer list from server (${messageType}):`, deviceList);
        console.log(`[WebSocketManager] Connected devices (excluding self):`, connectedDevices);
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

        switch (data.websocket_msg_type) {
            case "WebRTCSignal":
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
                break;

            case "SessionProposal":
                console.log("[WebSocketManager] Session proposal received:", data);
                // Handle session proposal
                this.sessionManager.handleSessionProposal(msg.from, data).catch(error => {
                    console.error("[WebSocketManager] Error handling session proposal:", error);
                });
                break;

            case "SessionResponse":
                console.log("[WebSocketManager] Session response received:", data);
                this.sessionManager.handleSessionResponse(msg.from, data);
                break;

            case "SigningDecline":
                // Ext-3c: a co-signer rejected our signing invite via
                // relay (they hadn't joined the WebRTC mesh yet so
                // data channels weren't available). Surface to popup
                // as a toast; if an active ceremony is running for
                // this session_id, the progress roster can overlay a
                // ✗ badge on the decliner.
                {
                    const d = data as any;
                    const signingId = d.signing_id;
                    const declinerId = d.decliner_id || msg.from;
                    console.log(
                        `[WebSocketManager] SigningDecline from ${declinerId} for ${signingId}`,
                    );
                    this.broadcastToPopup({
                        type: "signingPeerDeclined",
                        sessionId: signingId,
                        declinerId,
                    } as any);
                }
                break;

            default:
                console.warn("[WebSocketManager] Unknown relay message type:", (data as any).websocket_msg_type);
                break;
        }
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
            url: this.wsClient ? this.connectedUrl : undefined,
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
