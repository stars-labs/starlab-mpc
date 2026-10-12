import { WebRTCManager } from './webrtc'; // Adjust path as necessary
import type { SessionInfo, MeshStatus, DkgState } from '../../types/appstate'; // Fixed import
import type { WebRTCAppMessage } from '../../types/webrtc';
import { ServerMsg, ClientMsg, WebSocketMessagePayload, WebRTCSignal } from '../../types/websocket';

// Import WASM modules for FROST DKG
import wasmInit, { FrostDkgEd25519, FrostDkgSecp256k1 } from '../../../pkg/mpc_wallet.js';

console.log("Offscreen script loaded.");

// Initialize WASM modules for FROST DKG
let wasmInitialized = false;

async function initializeWasmModules() {
    try {
        console.log("🔧 Initializing FROST DKG WASM modules...");
        console.log("🔧 WASM Init: typeof wasmInit:", typeof wasmInit);
        console.log("🔧 WASM Init: typeof FrostDkgEd25519:", typeof FrostDkgEd25519);
        console.log("🔧 WASM Init: typeof FrostDkgSecp256k1:", typeof FrostDkgSecp256k1);

        await wasmInit();
        console.log("🔧 WASM Init: wasmInit() completed successfully");

        // Make WASM classes available globally for WebRTCManager
        (globalThis as any).FrostDkgEd25519 = FrostDkgEd25519;
        (globalThis as any).FrostDkgSecp256k1 = FrostDkgSecp256k1;

        console.log("🔧 WASM Init: Set (globalThis as any).FrostDkgEd25519 to:", typeof (globalThis as any).FrostDkgEd25519);
        console.log("🔧 WASM Init: Set (globalThis as any).FrostDkgSecp256k1 to:", typeof (globalThis as any).FrostDkgSecp256k1);

        // Also set on global if available (for Node.js-like environments)
        if (typeof global !== 'undefined') {
            (global as any).FrostDkgEd25519 = FrostDkgEd25519;
            (global as any).FrostDkgSecp256k1 = FrostDkgSecp256k1;
            console.log("🔧 WASM Init: Also set on global for Node.js compatibility");
        }

        // Test instance creation to verify WASM is working
        try {
            const testInstance = new FrostDkgSecp256k1();
            console.log("🔧 WASM Init: Test instance creation SUCCESS");
            console.log("🔧 WASM Init: Test instance type:", testInstance.constructor.name);
            console.log("🔧 WASM Init: Test instance has add_round1_package:", typeof testInstance.add_round1_package);
        } catch (testError) {
            console.log("🔧 WASM Init: Test instance creation FAILED:", testError);
        }

        wasmInitialized = true;
        console.log("✅ FROST DKG WASM modules initialized successfully");
        console.log("📦 Available modules: FrostDkgEd25519, FrostDkgSecp256k1");
    } catch (error) {
        console.error("❌ Failed to initialize FROST DKG WASM modules:", error);
        console.error("❌ Error details:", JSON.stringify(error));
        console.error("❌ Error stack:", error instanceof Error ? error.stack : 'No stack trace');
        wasmInitialized = false;
    }
}

// Initialize WASM immediately when the offscreen document loads
initializeWasmModules();

let webRTCManager: WebRTCManager | null = null;
let localdeviceId: string | null = null;
// Track WebRTC connections
let webrtcConnections: Record<string, boolean> = {};

// Removed wsRelayCallback as WebRTCManager will use a direct callback for sending payloads

// Function to send messages to the background script
function sendToBackground(message: { type: string; payload: unknown }) {
    console.log("Offscreen: Sending message to background:", message);
    chrome.runtime.sendMessage(message, (response) => {
        if (chrome.runtime.lastError) {
            console.error("Offscreen: Error sending message to background or receiving ack:", chrome.runtime.lastError.message, "Original message:", message);
        } else {
            console.log("Offscreen: Message to background acknowledged:", response, "Original message:", message);
        }
    });
}

// Listen for messages from the background script
chrome.runtime.onMessage.addListener((message: { type?: string; payload?: any }, sender, sendResponse) => {
    console.log("Offscreen: Message received from background:", message);

    let msgType: string | undefined;
    let actualPayload: any = {};

    // Ensure message and message.payload are defined before accessing properties
    if (message && message.payload && typeof message.payload.type === 'string') {
        // Message format: { payload: { type: "...", ...data } }
        msgType = message.payload.type;
        const { type, ...rest } = message.payload;
        actualPayload = rest;
        console.log(`Offscreen: Processing wrapped message. Type: ${msgType}, Payload:`, actualPayload);
    } else if (message && typeof message.type === 'string') {
        // Message format: { type: "...", ...data }
        msgType = message.type;
        const { type, ...rest } = message;
        actualPayload = rest;
        console.log(`Offscreen: Processing top-level type message. Type: ${msgType}, Payload:`, actualPayload);
    } else {
        console.warn("Offscreen: Received message with unknown structure or missing type:", message);
        sendResponse({ success: false, error: "Malformed or untyped message" });
        return false;
    }

    const payload = actualPayload;

    switch (msgType) {
        case "createOffscreen":
            console.log("Offscreen: Received 'createOffscreen' command. Document is already active.", payload);
            sendResponse({ success: true, message: "Offscreen document is already active." });
            break;
        case "init":
            console.log("Offscreen: Received 'init' command", payload);

            if (!payload.deviceId) {
                console.error("Offscreen: Init message missing deviceId:", payload);
                sendResponse({ success: false, error: "Missing deviceId in init message" });
                break;
            }

            localdeviceId = payload.deviceId;

            if (localdeviceId) {
                // Define how the offscreen WebRTCManager will send WebSocket payloads out
                // (via the background script)
                const sendPayloadToBackgroundForRelay = (todeviceId: string, payloadData: WebSocketMessagePayload) => {
                    console.log(`🔄 [Offscreen Relay] *** SENDING WEBRTC SIGNAL TO ${todeviceId} VIA BACKGROUND ***`);
                    console.log(`📦 [Offscreen Relay] Full payload:`, payloadData);

                    // Add debugging to see what type of data we're sending
                    if (payloadData && typeof payloadData === 'object') {
                        console.log(`✅ [Offscreen Relay] Payload type check - websocket_msg_type: ${payloadData.websocket_msg_type}`);
                        if (payloadData.websocket_msg_type === 'WebRTCSignal') {
                            console.log(`📡 [Offscreen Relay] This is a WebRTC signal, should be relayed to WebSocket`);

                            // Check what type of WebRTC signal this is
                            if ((payloadData as any).Offer) {
                                console.log(`📤 [Offscreen Relay] This is a WebRTC OFFER signal - SDP length: ${(payloadData as any).Offer?.sdp?.length || 0}`);
                            } else if ((payloadData as any).Answer) {
                                console.log(`📥 [Offscreen Relay] This is a WebRTC ANSWER signal`);
                            } else if ((payloadData as any).Candidate) {
                                console.log(`🧊 [Offscreen Relay] This is a WebRTC ICE CANDIDATE signal`);
                            }
                        }
                    }

                    const relayMessage = {
                        type: "fromOffscreen",
                        payload: {
                            type: "relayViaWs",
                            to: todeviceId,
                            data: payloadData, // This is the full WebSocketMessagePayload
                        }
                    };

                    console.log(`🚀 [Offscreen Relay] Sending relay message to background:`, relayMessage);
                    sendToBackground(relayMessage);
                    console.log(`✅ [Offscreen Relay] *** RELAY MESSAGE SENT TO BACKGROUND FOR ${todeviceId} ***`);
                };

                console.log(`Offscreen: Creating WebRTCManager for peer ID: ${localdeviceId}`);
                webRTCManager = new WebRTCManager(localdeviceId, sendPayloadToBackgroundForRelay);

                webRTCManager.onLog = (logMessage) => {
                    console.log(`[Offscreen WebRTC] ${logMessage}`);
                    // Instead of sending all log messages, parse and send specific status updates

                    // Check for data channel status changes
                    if (logMessage.includes("Data channel") && logMessage.includes("opened")) {
                        const peerMatch = logMessage.match(/with ([\w-]+)/);
                        const channelMatch = logMessage.match(/'([^']+)'/);
                        if (peerMatch && channelMatch) {
                            sendToBackground({
                                type: "fromOffscreen",
                                payload: {
                                    type: "dataChannelStatusUpdate",
                                    deviceId: peerMatch[1],
                                    channelName: channelMatch[1],
                                    state: "open"
                                }
                            });
                        }
                    }

                    // Check for connection status changes
                    if (logMessage.includes("data channel to") && logMessage.includes("is now open")) {
                        const peerMatch = logMessage.match(/to ([\w-]+)/);
                        if (peerMatch) {
                            sendToBackground({
                                type: "fromOffscreen",
                                payload: {
                                    type: "webrtcStatusUpdate",
                                    deviceId: peerMatch[1],
                                    status: "connected"
                                }
                            });
                        }
                    }

                    // Only send important operational messages, not routine status
                    if (logMessage.includes("Error") ||
                        logMessage.includes("Failed") ||
                        logMessage.includes("Warning")) {
                        console.warn(`[Offscreen WebRTC] Important: ${logMessage}`);
                    }
                };
                webRTCManager.onSessionUpdate = (sessionInfo, invites) => {
                    console.log("Offscreen: Session update:", { sessionInfo, invites });
                    sendToBackground({ type: "fromOffscreen", payload: { type: "sessionUpdate", sessionInfo, invites } });
                };
                webRTCManager.onMeshStatusUpdate = (status) => {
                    console.log("Offscreen: Mesh status update:", status);
                    sendToBackground({ type: "fromOffscreen", payload: { type: "meshStatusUpdate", status } });
                };
                webRTCManager.onWebRTCAppMessage = (fromdeviceId: string, appMessage: WebRTCAppMessage) => {
                    console.log("Offscreen: WebRTC app message:", { fromdeviceId, appMessage });
                    sendToBackground({ type: "fromOffscreen", payload: { type: "webrtcMessage", fromdeviceId, message: appMessage } });
                };
                webRTCManager.onDkgStateUpdate = (state) => {
                    console.log("Offscreen: DKG state update:", state);
                    sendToBackground({ type: "fromOffscreen", payload: { type: "dkgStateUpdate", state } });
                };

                webRTCManager.onWebRTCConnectionUpdate = (deviceId: string, connected: boolean) => {
                    console.log("Offscreen: WebRTC connection update:", deviceId, connected);

                    // Update local tracking
                    webrtcConnections[deviceId] = connected;

                    sendToBackground({
                        type: "fromOffscreen",
                        payload: {
                            type: "webrtcConnectionUpdate",
                            deviceId,
                            connected
                        }
                    });
                };

                console.log(`Offscreen: WebRTC Manager successfully initialized for peer ID: ${localdeviceId}.`);

                // Request session state restoration from background if available
                console.log("🔄 Offscreen: Requesting session state restoration from background");
                chrome.runtime.sendMessage({ type: "requestSessionRestore" }, (restoreResponse) => {
                    if (chrome.runtime.lastError) {
                        console.warn("❌ Offscreen: Failed to request session restore:", chrome.runtime.lastError.message);
                    } else if (restoreResponse && restoreResponse.success && restoreResponse.sessionInfo) {
                        console.log("✅ Offscreen: Received restored session info:", {
                            sessionId: restoreResponse.sessionInfo.session_id,
                            participants: restoreResponse.sessionInfo.participants,
                            acceptedDevices: restoreResponse.sessionInfo.accepted_devices,
                            status: restoreResponse.sessionInfo.status
                        });
                        // Session restoration will be handled via sessionAccepted message from background
                    } else {
                        console.log("ℹ️ Offscreen: No session state to restore or request failed");
                    }
                });

                sendResponse({ success: true, message: "Offscreen initialized with WebRTCManager." });
            } else {
                console.error("Offscreen: LocaldeviceId is falsy after assignment:", localdeviceId);
                sendResponse({ success: false, error: "LocaldeviceId assignment failed." });
            }
            break;

        case "relayViaWs":
            console.log("Offscreen: Received 'relayViaWs' (WebSocket payload) from background", payload);
            if (webRTCManager && payload.data) {
                // The payload should contain either 'fromdeviceId' or we need to extract it from the data
                let fromdeviceId = payload.to;
                if (fromdeviceId) {
                    console.log(`Offscreen: Calling webRTCManager.handleWebSocketMessagePayload with fromdeviceId: ${fromdeviceId}, data:`, payload.data);
                    // The payload.data is expected to be WebSocketMessagePayload
                    webRTCManager.handleWebSocketMessagePayload(fromdeviceId, payload.data as WebSocketMessagePayload);
                    console.log("Offscreen: Relayed message to WebRTCManager for peer:", fromdeviceId);
                    sendResponse({ success: true, message: "Message relayed to WebRTCManager." });
                } else {
                    const debugInfo = {
                        webRTCManagerReady: !!webRTCManager,
                        hasData: !!payload.data,
                        localdeviceId,
                        payload,
                        missingFromdeviceId: "fromdeviceId not found in payload or payload.data.from"
                    };
                    console.warn("Offscreen: Cannot handle relayViaWs - missing fromdeviceId.", debugInfo);
                    sendResponse({ success: false, error: "Missing fromdeviceId in relayViaWs payload.", debugInfo });
                }
            } else {
                const debugInfo = {
                    webRTCManagerReady: !!webRTCManager,
                    hasData: !!payload.data,
                    localdeviceId,
                    payload
                };
                console.warn("Offscreen: Cannot handle relayViaWs - WebRTCManager not ready or missing data.", debugInfo);
                sendResponse({ success: false, error: "WebRTCManager not ready or missing data in relayViaWs payload.", debugInfo });
            }
            break;

        case "sessionAccepted":
            console.log("🎉 *** OFFSCREEN: RECEIVED SESSION ACCEPTED - INITIATING WEBRTC OFFERS ***", payload);
            if (webRTCManager && payload.sessionInfo && payload.currentdeviceId) {
                console.log(`📋 [SessionAccepted] Setting up WebRTC for accepted session: ${payload.sessionInfo.session_id}`);
                console.log(`👤 [SessionAccepted] Current peer: ${payload.currentdeviceId}`);
                console.log(`👥 [SessionAccepted] All participants:`, payload.sessionInfo.participants);

                // Extract blockchain parameter from payload
                const blockchain = payload.blockchain || "solana"; // Default to solana if not specified
                console.log("🔗 [SessionAccepted] Blockchain selection for session setup:", blockchain);

                // Set the blockchain selection on WebRTCManager
                webRTCManager.setBlockchain(blockchain);

                // Update the WebRTCManager with the session info and trigger mesh status check
                console.log("📊 [SessionAccepted] Updating WebRTCManager with session info...");
                webRTCManager.updateSessionInfo(payload.sessionInfo);

                // Initiate WebRTC connections to devices with lexicographically larger IDs
                const currentdeviceId: string = payload.currentdeviceId;
                const participants: string[] = payload.sessionInfo.participants || [];
                const devicesToConnect: string[] = participants.filter((deviceId: string) =>
                    deviceId !== currentdeviceId && deviceId > currentdeviceId
                );

                console.log(`🎯 [SessionAccepted] *** WEBRTC OFFER INITIATION LOGIC ***`);
                console.log(`📍 Current device ID: ${currentdeviceId}`);
                console.log(`👥 All participants: [${participants.join(', ')}]`);
                console.log(`🚀 Devices to initiate offers to (ID > ${currentdeviceId}): [${devicesToConnect.join(', ')}]`);

                if (devicesToConnect.length > 0) {
                    console.log(`✅ [SessionAccepted] INITIATING ${devicesToConnect.length} WEBRTC OFFERS...`);
                    devicesToConnect.forEach((deviceId: string) => {
                        console.log(`🚀 [SessionAccepted] >>> INITIATING WEBRTC OFFER TO ${deviceId} <<<`);
                        webRTCManager!.initiatePeerConnection(deviceId);
                    });
                    console.log(`✅ [SessionAccepted] *** ALL WEBRTC OFFERS INITIATED ***`);
                } else {
                    console.log(`⏳ [SessionAccepted] No devices to initiate offers to based on ID ordering. Waiting for incoming offers.`);
                }

                sendResponse({ success: true, message: `Session accepted and WebRTC setup initiated with blockchain: ${blockchain}.` });
            } else {
                const debugInfo = {
                    webRTCManagerReady: !!webRTCManager,
                    hasSessionInfo: !!payload.sessionInfo,
                    hasCurrentdeviceId: !!payload.currentdeviceId,
                    localdeviceId,
                    payload
                };
                console.error("❌ [SessionAccepted] Cannot handle sessionAccepted - missing required data.", debugInfo);
                sendResponse({ success: false, error: "WebRTCManager not ready or missing sessionInfo/currentdeviceId in sessionAccepted payload.", debugInfo });
            }
            break;

        case "sessionRestore":
            console.log("🔄 *** OFFSCREEN: RECEIVED SESSION RESTORE - RESTORING WITHOUT AUTO-ACCEPTANCE ***", payload);
            if (webRTCManager && payload.sessionInfo && payload.currentdeviceId) {
                console.log(`📋 [SessionRestore] Restoring session state: ${payload.sessionInfo.session_id}`);
                console.log(`👤 [SessionRestore] Current peer: ${payload.currentdeviceId}`);
                console.log(`👥 [SessionRestore] All participants:`, payload.sessionInfo.participants);

                // Extract blockchain parameter from payload
                const blockchain = payload.blockchain || "solana"; // Default to solana if not specified
                console.log("🔗 [SessionRestore] Blockchain selection for restored session:", blockchain);

                // Set the blockchain selection on WebRTCManager
                webRTCManager.setBlockchain(blockchain);

                // Update the WebRTCManager with the session info 
                console.log("📊 [SessionRestore] Updating WebRTCManager with restored session info...");
                webRTCManager.updateSessionInfo(payload.sessionInfo);

                // CRITICAL: Do NOT initiate WebRTC offers during restoration
                // Restoration should only restore state, not trigger new connections or send messages
                console.log("⚠️ [SessionRestore] RESTORATION MODE: NOT initiating WebRTC offers");
                console.log("⚠️ [SessionRestore] Session state restored without triggering auto-acceptance behaviors");

                sendResponse({ success: true, message: `Session state restored with blockchain: ${blockchain}. No auto-acceptance triggered.` });
            } else {
                const debugInfo = {
                    webRTCManagerReady: !!webRTCManager,
                    hasSessionInfo: !!payload.sessionInfo,
                    hasCurrentdeviceId: !!payload.currentdeviceId,
                    localdeviceId,
                    payload
                };
                console.error("❌ [SessionRestore] Cannot handle sessionRestore - missing required data.", debugInfo);
                sendResponse({ success: false, error: "WebRTCManager not ready or missing sessionInfo/currentdeviceId in sessionRestore payload.", debugInfo });
            }
            break;

        case "sessionAllAccepted":
            console.log("🎉 Offscreen: Received 'sessionAllAccepted' command - all participants have accepted!", payload);
            if (webRTCManager && payload.sessionInfo) {
                console.log(`📋 Session info: participants=[${payload.sessionInfo.participants.join(', ')}], accepted=[${payload.sessionInfo.accepted_devices.join(', ')}]`);

                // Extract blockchain parameter from payload
                const blockchain = payload.blockchain || "solana"; // Default to solana if not specified
                console.log("🔗 Offscreen: Blockchain selection for DKG:", blockchain);

                // Set the blockchain selection on WebRTCManager
                webRTCManager.setBlockchain(blockchain);

                // Update session info and trigger mesh readiness check with blockchain parameter
                webRTCManager.updateSessionInfo(payload.sessionInfo);

                // Trigger DKG with the correct blockchain parameter
                console.log("🚀 Offscreen: Triggering DKG with blockchain:", blockchain);
                webRTCManager.checkAndTriggerDkg(blockchain);

                console.log("✅ Offscreen: Updated session info and triggered mesh readiness check with blockchain");
                sendResponse({ success: true, message: "Session all accepted processed - mesh readiness and DKG triggered with blockchain." });
            } else {
                console.warn("❌ Offscreen: Cannot handle sessionAllAccepted - WebRTCManager not ready or missing sessionInfo");
                sendResponse({ success: false, error: "WebRTCManager not ready or missing sessionInfo" });
            }
            break;

        case "sessionResponseUpdate":
            console.log("Offscreen: Received 'sessionResponseUpdate' command", payload);
            if (webRTCManager && payload.sessionInfo) {
                // Update session info for tracking acceptance progress
                webRTCManager.updateSessionInfo(payload.sessionInfo);
                console.log("Offscreen: Updated session info with latest acceptance status");
                sendResponse({ success: true, message: "Session response update processed." });
            } else {
                console.warn("Offscreen: Cannot handle sessionResponseUpdate - WebRTCManager not ready or missing sessionInfo");
                sendResponse({ success: false, error: "WebRTCManager not ready or missing sessionInfo" });
            }
            break;

        case "acceptSession":
            console.log("Offscreen: Received 'acceptSession' command", payload);
            // This message type should be handled by background script only
            console.warn("Offscreen: acceptSession should be handled by background script, not offscreen. Ignoring.");
            sendResponse({ success: true, message: "acceptSession ignored - should be handled by background script." });
            break;

        case "getState":
            console.log(`Offscreen: Received '${msgType}' command`, payload);
            if (webRTCManager && localdeviceId) {
                const state = {
                    initialized: true,
                    localdeviceId: localdeviceId,
                    webrtcConnections: webrtcConnections, // Include tracked connections
                    sessionInfo: webRTCManager.sessionInfo,
                    invites: webRTCManager.invites,
                    dkgState: webRTCManager.dkgState,
                    meshStatus: webRTCManager.meshStatus,
                    dataChannelStatus: webRTCManager.getDataChannelStatus(),
                    connecteddevices: webRTCManager.getConnectedPeers(),
                    peerConnectionStatus: webRTCManager.getPeerConnectionStatus()
                };
                console.log("Offscreen: Sending combined state to background:", state);
                sendResponse({ success: true, data: state });
            } else {
                console.log("Offscreen: WebRTCManager not ready, sending uninitialized state.");
                sendResponse({ success: true, data: { initialized: false, localdeviceId: localdeviceId, webrtcConnections: {} } });
            }
            break;

        case "sendDirectMessage":
            console.log("Offscreen: Received 'sendDirectMessage' command", payload);
            if (webRTCManager && payload.todeviceId && payload.message) {
                const success = webRTCManager.sendDirectMessage(payload.todeviceId, payload.message);
                if (!success) {
                    console.warn(`Offscreen: Failed to send direct message to ${payload.todeviceId}`);
                }
                sendResponse({ success, message: success ? "Message sent" : "Failed to send message" });
            } else {
                const debugInfo = {
                    webRTCManagerReady: !!webRTCManager,
                    hasTodeviceId: !!payload.todeviceId,
                    hasMessage: !!payload.message,
                    localdeviceId,
                    payload
                };
                console.warn("Offscreen: Cannot send direct message - missing required data.", debugInfo);
                sendResponse({ success: false, error: "WebRTCManager not ready or missing todeviceId/message in payload.", debugInfo });
            }
            break;

        case "getWebRTCStatus":
            console.log("Offscreen: Received 'getWebRTCStatus' command", payload);
            if (webRTCManager) {
                const status = {
                    dataChannelStatus: webRTCManager.getDataChannelStatus(),
                    connecteddevices: webRTCManager.getConnecteddevices(),
                    peerConnectionStatus: webRTCManager.getPeerConnectionStatus(),
                    sessionInfo: webRTCManager.sessionInfo,
                    meshStatus: webRTCManager.meshStatus
                };
                console.log("Offscreen: Sending WebRTC status:", status);
                sendResponse({ success: true, data: status });
            } else {
                console.log("Offscreen: WebRTCManager not ready for status request.");
                sendResponse({ success: true, data: { initialized: false } });
            }
            break;

        case "getDkgStatus":
            console.log("Offscreen: Received 'getDkgStatus' command", payload);
            if (webRTCManager) {
                const dkgStatus = webRTCManager.getDkgStatus();
                console.log("Offscreen: Sending DKG status:", dkgStatus);
                sendResponse({ success: true, data: dkgStatus });
            } else {
                console.log("Offscreen: WebRTCManager not ready for DKG status request.");
                sendResponse({ success: true, data: { initialized: false } });
            }
            break;

        case "getGroupPublicKey":
            console.log("Offscreen: Received 'getGroupPublicKey' command", payload);
            if (webRTCManager) {
                const groupPublicKey = webRTCManager.getGroupPublicKey();
                console.log("Offscreen: Sending group public key:", groupPublicKey);
                sendResponse({ success: true, data: { groupPublicKey } });
            } else {
                console.log("Offscreen: WebRTCManager not ready for group public key request.");
                sendResponse({ success: false, error: "WebRTCManager not initialized" });
            }
            break;

        case "getSolanaAddress":
            console.log("Offscreen: Received 'getSolanaAddress' command", payload);
            if (webRTCManager) {
                const solanaAddress = webRTCManager.getSolanaAddress();
                console.log("Offscreen: Sending Solana address:", solanaAddress);
                sendResponse({ success: true, data: { solanaAddress } });
            } else {
                console.log("Offscreen: WebRTCManager not ready for Solana address request.");
                sendResponse({ success: false, error: "WebRTCManager not initialized" });
            }
            break;

        case "getEthereumAddress":
            console.log("Offscreen: Received 'getEthereumAddress' command", payload);
            if (webRTCManager) {
                const ethereumAddress = webRTCManager.getEthereumAddress();
                console.log("Offscreen: Sending Ethereum address:", ethereumAddress);
                sendResponse({ success: true, data: { ethereumAddress } });
            } else {
                console.log("Offscreen: WebRTCManager not ready for Ethereum address request.");
                sendResponse({ success: false, error: "WebRTCManager not initialized" });
            }
            break;

        case "setBlockchain":
            console.log("Offscreen: Received 'setBlockchain' command", payload);
            if (webRTCManager && payload.blockchain) {
                console.log("Offscreen: Setting blockchain to:", payload.blockchain);
                webRTCManager.setBlockchain(payload.blockchain);
                sendResponse({ success: true, message: `Blockchain set to ${payload.blockchain}` });
            } else {
                console.warn("Offscreen: Cannot set blockchain - WebRTCManager not ready or missing blockchain parameter");
                sendResponse({ success: false, error: "WebRTCManager not ready or missing blockchain parameter" });
            }
            break;

        default:
            console.warn("Offscreen: Received unhandled message type from background:", msgType, payload);
            sendResponse({ success: false, error: `Unknown message type: ${msgType}` });
            break;
    }
    // Return true if sendResponse will be called asynchronously.
    // For most of these, sendResponse is called synchronously.
    return false;
});

// Signal to the background script that the offscreen document is ready
console.log("Offscreen: All listeners set up. Sending 'offscreenReady' to background.");

// Add a small delay to ensure background script is ready to receive messages

chrome.runtime.sendMessage({ type: "offscreenReady" }, (response) => {
    if (chrome.runtime.lastError) {
        console.error("Offscreen: Error sending 'offscreenReady' or receiving ack from background:", chrome.runtime.lastError.message);

        // Retry sending the ready signal if it failed
        setTimeout(() => {
            console.log("Offscreen: Retrying 'offscreenReady' signal...");
            chrome.runtime.sendMessage({ type: "offscreenReady" }, (retryResponse) => {
                if (chrome.runtime.lastError) {
                    console.error("Offscreen: Retry also failed:", chrome.runtime.lastError.message);

                    // Try one more time with longer delay
                    setTimeout(() => {
                        console.log("Offscreen: Final retry 'offscreenReady' signal...");
                        chrome.runtime.sendMessage({ type: "offscreenReady" }, (finalResponse) => {
                            if (chrome.runtime.lastError) {
                                console.error("Offscreen: Final retry failed:", chrome.runtime.lastError.message);
                            } else {
                                console.log("Offscreen: 'offscreenReady' final retry successful:", finalResponse);
                            }
                        });
                    }, 2000);
                } else {
                    console.log("Offscreen: 'offscreenReady' retry successful:", retryResponse);
                }
            });
        }, 1000);
    } else {
        console.log("Offscreen: 'offscreenReady' signal sent and acknowledged by background:", response);

        // Check if we received a successful response and expect init soon
        if (response && response.success) {
            // Set a timeout to check if init was received
            setTimeout(() => {
                if (!webRTCManager || !localdeviceId) {
                    console.warn("Offscreen: Init data not received within expected time. WebRTCManager:", !!webRTCManager, "localdeviceId:", localdeviceId);
                    console.warn("Offscreen: This may indicate the background script failed to send init data.");

                    // Request init data manually
                    chrome.runtime.sendMessage({ type: "requestInit" }, (initResponse) => {
                        if (chrome.runtime.lastError) {
                            console.error("Offscreen: Error requesting init data:", chrome.runtime.lastError.message);
                        } else {
                            console.log("Offscreen: Init data request response:", initResponse);
                        }
                    });
                }
            }, 3000);
        }
    }
});


console.log("Offscreen document setup complete and active.");
