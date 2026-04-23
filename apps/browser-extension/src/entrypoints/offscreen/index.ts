import { WebRTCManager } from './webrtc'; // Adjust path as necessary
import type { SessionInfo, MeshStatus, DkgState } from "@mpc-wallet/types/appstate"; // Fixed import
import type { WebRTCAppMessage } from "@mpc-wallet/types/webrtc";
import { ServerMsg, ClientMsg, WebSocketMessagePayload, WebRTCSignal } from "@mpc-wallet/types/websocket";

// Import WASM modules for FROST DKG
import wasmInit, { FrostDkgEd25519, FrostDkgSecp256k1 } from '@mpc-wallet/core-wasm';

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

// Helper function to decrypt CLI keystore
async function decryptCLIKeystore(base64Data: string, password: string): Promise<string> {
    // CLI uses base64 encoding for the entire encrypted blob
    const encryptedData = Uint8Array.from(atob(base64Data), c => c.charCodeAt(0));
    
    // Extract salt (first 16 bytes), IV (next 12 bytes), and ciphertext + tag
    const salt = encryptedData.slice(0, 16);
    const iv = encryptedData.slice(16, 28);
    const ciphertextWithTag = encryptedData.slice(28);
    
    // Derive key using PBKDF2 (CLI uses 100,000 iterations)
    const encoder = new TextEncoder();
    const passwordKey = await crypto.subtle.importKey(
        'raw',
        encoder.encode(password),
        'PBKDF2',
        false,
        ['deriveKey']
    );
    
    const key = await crypto.subtle.deriveKey(
        {
            name: 'PBKDF2',
            salt: salt,
            iterations: 100000,
            hash: 'SHA-256'
        },
        passwordKey,
        { name: 'AES-GCM', length: 256 },
        false,
        ['decrypt']
    );
    
    // Decrypt using AES-GCM
    const decrypted = await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: iv },
        key,
        ciphertextWithTag
    );
    
    return new TextDecoder().decode(decrypted);
}

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
        case "keepalive":
            // Architectural reminder #2: background sends these every
            // 25s while a ceremony is in progress to stop Chrome from
            // killing this offscreen document for being idle. The act
            // of receiving and responding resets the idle timer;
            // nothing else is needed. Intentionally no-op at the
            // business-logic layer and silent at log level (ping
            // floods logs otherwise).
            sendResponse({ success: true, pong: true });
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
                    console.log(`Offscreen: Sending WebRTC signal to ${todeviceId} via background:`, payloadData);

                    // Add debugging to see what type of data we're sending
                    if (payloadData && typeof payloadData === 'object') {
                        console.log(`Offscreen: Payload type check - websocket_msg_type: ${payloadData.websocket_msg_type}`);
                        if (payloadData.websocket_msg_type === 'WebRTCSignal') {
                            console.log(`Offscreen: This is a WebRTC signal, should be relayed to WebSocket`);
                        }
                    }

                    sendToBackground({
                        type: "fromOffscreen",
                        payload: {
                            type: "relayViaWs",
                            to: todeviceId,
                            data: payloadData, // This is the full WebSocketMessagePayload
                        }
                    });
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
                // Ext-1d: completion-specific event with the derived
                // group public key + address. Separate channel from
                // dkgStateUpdate so late popup subscribers (popup that
                // just opened after DKG already finished) can query
                // appState to find out without needing to replay the
                // state-transition timeline. Propagated through
                // StateManager → popup via `dkgCompleted` broadcast.
                webRTCManager.onDkgComplete = (payload) => {
                    console.log("Offscreen: DKG complete:", payload);
                    sendToBackground({
                        type: "fromOffscreen",
                        payload: { type: "dkgComplete", ...payload },
                    });
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

                // Request keystore data from background to load into WASM
                chrome.runtime.sendMessage({ type: "getActiveKeystore" }, async (response) => {
                    if (chrome.runtime.lastError) {
                        console.error("Offscreen: Error getting active keystore:", chrome.runtime.lastError.message);
                        return;
                    }
                    
                    if (response && response.success && response.keyShare) {
                        console.log("Offscreen: Found active keystore to load");
                        
                        try {
                            const { FrostDkgSecp256k1, FrostDkgEd25519 } = await import("@mpc-wallet/core-wasm");
                            
                            // Determine curve type
                            const curveType = response.keyShare.curve || 'secp256k1';
                            
                            let dkgInstance;
                            if (curveType === 'secp256k1') {
                                dkgInstance = new FrostDkgSecp256k1();
                            } else {
                                dkgInstance = new FrostDkgEd25519();
                            }
                            
                            // Import the keystore
                            const keystoreData = {
                                key_package: response.keyShare.key_package,
                                group_public_key: response.keyShare.group_public_key,
                                session_id: response.keyShare.session_id,
                                device_id: response.keyShare.device_id,
                                participant_index: response.keyShare.participant_index,
                                threshold: response.keyShare.threshold,
                                total_participants: response.keyShare.total_participants
                            };
                            
                            dkgInstance.import_keystore(JSON.stringify(keystoreData));
                            console.log("Offscreen: Successfully loaded keystore into WASM");
                            
                            // Store the instance for later use (signing)
                            // Note: This should be integrated with WebRTCManager in production
                            (globalThis as any).__importedDkgInstance = dkgInstance;
                            (globalThis as any).__importedKeystoreCurveType = curveType;
                        } catch (error) {
                            console.error("Offscreen: Failed to load keystore into WASM:", error);
                        }
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
            console.log("🎯 Offscreen: Received 'sessionAccepted' command", payload);
            if (webRTCManager && payload.sessionInfo && payload.currentdeviceId) {
                console.log(`🔄 Offscreen: Setting up WebRTC for accepted session: ${payload.sessionInfo.session_id}`);
                console.log(`🔄 Offscreen: Current peer: ${payload.currentdeviceId}, Participants:`, payload.sessionInfo.participants);

                // Extract blockchain parameter from payload
                const blockchain = payload.blockchain || "solana"; // Default to solana if not specified
                console.log("🔗 Offscreen: Blockchain selection for session setup:", blockchain);

                // Set the blockchain selection on WebRTCManager
                webRTCManager.setBlockchain(blockchain);

                // Update the WebRTCManager with the session info and trigger mesh status check
                webRTCManager.updateSessionInfo(payload.sessionInfo);

                // Initiate WebRTC connections to devices with lexicographically larger IDs
                const currentdeviceId: string = payload.currentdeviceId;
                const participants: string[] = payload.sessionInfo.participants || [];
                const devicesToConnect: string[] = participants.filter((deviceId: string) =>
                    deviceId !== currentdeviceId && deviceId > currentdeviceId
                );

                console.log(`Offscreen: devices to initiate offers to (ID > ${currentdeviceId}):`, devicesToConnect);

                if (devicesToConnect.length > 0) {
                    devicesToConnect.forEach((deviceId: string) => {
                        console.log(`Offscreen: Initiating WebRTC connection to ${deviceId}`);
                        webRTCManager!.initiatePeerConnection(deviceId);
                    });
                } else {
                    console.log(`Offscreen: No devices to initiate offers to based on ID ordering. Waiting for incoming offers.`);
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
                console.warn("Offscreen: Cannot handle sessionAccepted - missing required data.", debugInfo);
                sendResponse({ success: false, error: "WebRTCManager not ready or missing sessionInfo/currentdeviceId in sessionAccepted payload.", debugInfo });
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

        // Ext-2d-offscreen (bootstrap): the `sessionReadyForSigning`
        // event fires when a signing session has hit its threshold
        // of joined participants (see webSocketManager.ts
        // maybeTriggerCeremony). Distinct from `sessionAllAccepted`
        // (the DKG trigger) because they require different offscreen
        // code paths: DKG starts key generation, signing starts a
        // signing ceremony over an already-loaded keystore. Conflating
        // them would try to regenerate a key share on top of an
        // existing wallet and corrupt it.
        //
        // This handler is the minimal bootstrap: set blockchain +
        // updateSessionInfo so the WebRTCManager has the signing
        // session in scope for future WebRTC message routing, log
        // the event so devtools confirms end-to-end trigger wiring.
        // Actual FROST signing_commit initiation is a follow-up commit
        // (Ext-2d-offscreen-round1).
        case "sessionReadyForSigning":
            console.log(
                "🖋️  Offscreen: Received 'sessionReadyForSigning' — threshold signers joined",
                payload,
            );
            if (webRTCManager && payload.sessionInfo) {
                const blockchain = payload.blockchain || "ethereum";
                const signingMessageHex =
                    payload.sessionInfo.signing_message_hex;
                console.log(
                    `🔗 Offscreen: Signing ceremony blockchain=${blockchain}, session=${payload.sessionInfo.session_id}, participants=[${payload.sessionInfo.participants.join(", ")}], threshold=${payload.sessionInfo.threshold}, messageHex=${signingMessageHex ?? "(none)"}`,
                );

                webRTCManager.setBlockchain(blockchain);
                webRTCManager.updateSessionInfo(payload.sessionInfo);

                if (!signingMessageHex) {
                    console.warn(
                        "❌ Offscreen: session has no signing_message_hex — can't start signing ceremony",
                    );
                    sendResponse({
                        success: false,
                        error: "Session missing signing_message_hex",
                    });
                    break;
                }

                // Ext-2d-offscreen-round1: kick off FROST round 1.
                // Fire-and-forget the heavy work; respond success
                // once keystore lookup + signing_commit are done
                // OR synchronously if we're still waiting on
                // keystore/mesh. The caller (background) doesn't
                // block on this — ceremony progress flows back via
                // signingStateUpdate + signingComplete events.
                (async () => {
                    // If the FROST instance isn't loaded yet, fetch
                    // the active keystore from background and load
                    // it. Uses an IIFE so the sendResponse below
                    // returns eagerly — load can take hundreds of
                    // ms on first call.
                    const status = webRTCManager!.getDkgStatus?.();
                    const alreadyLoaded = !!status?.frostDkgInitialized;
                    if (!alreadyLoaded) {
                        console.log(
                            "[sessionReadyForSigning] FROST not loaded — requesting active keystore",
                        );
                        const loaded = await new Promise<boolean>((resolve) => {
                            chrome.runtime.sendMessage(
                                { type: "getActiveKeystore" },
                                async (resp: any) => {
                                    if (
                                        !resp ||
                                        !resp.success ||
                                        !resp.keyShare
                                    ) {
                                        console.warn(
                                            "[sessionReadyForSigning] getActiveKeystore failed or empty",
                                            chrome.runtime.lastError?.message,
                                            resp?.error,
                                        );
                                        resolve(false);
                                        return;
                                    }
                                    try {
                                        await webRTCManager!.loadKeystoreForSigning(
                                            resp.keyShare,
                                            blockchain,
                                        );
                                        resolve(true);
                                    } catch (e) {
                                        console.error(
                                            "[sessionReadyForSigning] loadKeystoreForSigning failed:",
                                            e,
                                        );
                                        resolve(false);
                                    }
                                },
                            );
                        });
                        if (!loaded) return;
                    }
                    const ok = await webRTCManager!.initiateSigningCeremony(
                        payload.sessionInfo,
                        signingMessageHex,
                    );
                    console.log(
                        `[sessionReadyForSigning] initiateSigningCeremony → ${ok}`,
                    );
                })();

                sendResponse({
                    success: true,
                    message:
                        "Signing session registered. Round 1 kickoff in progress.",
                });
            } else {
                console.warn(
                    "❌ Offscreen: Cannot handle sessionReadyForSigning — WebRTCManager not ready or missing sessionInfo",
                );
                sendResponse({
                    success: false,
                    error: "WebRTCManager not ready or missing sessionInfo",
                });
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

        case "importKeystore":
            const importMessageId = `offscreen-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
            console.log(`Offscreen: Received 'importKeystore' command (ID: ${importMessageId})`, payload);
            console.log(`Offscreen: Message structure - keys: ${Object.keys(payload).join(', ')}`);
            
            // Wait for WASM to be initialized
            if (!wasmInitialized) {
                console.error(`Offscreen: WASM not initialized yet (ID: ${importMessageId})`);
                sendResponse({ success: false, error: "WASM not initialized" });
                break;
            }
            
            (async () => {
                try {
                    console.log(`Offscreen: Processing importKeystore (ID: ${importMessageId})`);
                    const { keystoreData, password, chain } = payload;
                    
                    if (!keystoreData) {
                        throw new Error("No keystore data provided");
                    }
                    
                    // Parse the keystore
                    const parsedKeystore = JSON.parse(keystoreData);
                    console.log("Offscreen: Parsed keystore metadata:", parsedKeystore.metadata);
                    
                    let keystoreToImport: any;
                    
                    // Check if keystore is encrypted
                    if (parsedKeystore.encrypted === true && parsedKeystore.algorithm === "AES-256-GCM-PBKDF2") {
                        if (!password) {
                            throw new Error("Password required for encrypted keystore");
                        }
                        
                        console.log("Offscreen: Decrypting CLI keystore...");
                        const decryptedData = await decryptCLIKeystore(parsedKeystore.data, password);
                        console.log("Offscreen: Successfully decrypted keystore");
                        
                        // Create a new keystore object with decrypted data
                        keystoreToImport = {
                            ...parsedKeystore,
                            encrypted: false,
                            algorithm: "none",
                            data: decryptedData
                        };
                    } else {
                        // Use the keystore as-is if not encrypted
                        keystoreToImport = parsedKeystore;
                    }
                    
                    // Import using WASM based on curve type
                    const metadata = keystoreToImport.metadata;
                    const curveType = metadata.curve_type;
                    
                    console.log("Offscreen: Importing keystore with WASM for curve:", curveType);
                    
                    let dkgInstance;
                    if (curveType === 'secp256k1') {
                        dkgInstance = new FrostDkgSecp256k1();
                    } else if (curveType === 'ed25519') {
                        dkgInstance = new FrostDkgEd25519();
                    } else {
                        throw new Error(`Unsupported curve type: ${curveType}`);
                    }
                    
                    // Import the keystore with decrypted data
                    try {
                        let keyDataToImport: any;
                        
                        // Parse the decrypted data if it's a string
                        if (typeof keystoreToImport.data === 'string' && keystoreToImport.data) {
                            try {
                                keyDataToImport = JSON.parse(keystoreToImport.data);
                                console.log("Offscreen: Parsed decrypted key data:", Object.keys(keyDataToImport));
                            } catch (e) {
                                console.error("Offscreen: Failed to parse decrypted data as JSON");
                                // If parsing fails, assume the data contains the key fields directly
                                keyDataToImport = keystoreToImport;
                            }
                        } else {
                            // Use the keystore directly if data is not a string
                            keyDataToImport = keystoreToImport;
                        }
                        
                        // Add metadata fields to the key data for WASM
                        if (keyDataToImport && !keyDataToImport.participant_index && keystoreToImport.metadata) {
                            keyDataToImport = {
                                ...keyDataToImport,
                                participant_index: keystoreToImport.metadata.participant_index,
                                total_participants: keystoreToImport.metadata.total_participants,
                                threshold: keystoreToImport.metadata.threshold,
                                group_public_key: keystoreToImport.metadata.group_public_key
                            };
                        }
                        
                        const dataToImportStr = JSON.stringify(keyDataToImport);
                        console.log("Offscreen: Importing keystore into WASM with structure:", Object.keys(keyDataToImport));
                        dkgInstance.import_keystore(dataToImportStr);
                        console.log("Offscreen: Successfully imported keystore");
                    } catch (wasmError: any) {
                        console.error("Offscreen: WASM import error:", wasmError);
                        console.error("Offscreen: WASM error message:", wasmError.message || 'Unknown error');
                        throw new Error(`Failed to import keystore: ${wasmError.message || wasmError}`);
                    }
                    
                    // Extract addresses based on blockchain selection
                    const addresses: Record<string, string> = {};
                    
                    if (metadata.blockchains) {
                        metadata.blockchains.forEach((blockchain: any) => {
                            addresses[blockchain.blockchain] = blockchain.address;
                        });
                    }
                    
                    // Get the primary address based on the requested chain
                    const primaryAddress = addresses[payload.chain] || 
                        (metadata.blockchains && metadata.blockchains[0]?.address) || 
                        '';
                    
                    sendResponse({
                        success: true,
                        sessionInfo: {
                            session_id: metadata.wallet_id || metadata.session_id,
                            device_id: metadata.device_id,
                            threshold: metadata.threshold,
                            total_participants: metadata.total_participants,
                            participant_index: metadata.participant_index,
                            curve_type: metadata.curve_type,
                            blockchains: metadata.blockchains,
                            group_public_key: metadata.group_public_key
                        },
                        group_public_key: metadata.group_public_key,
                        addresses,
                        address: primaryAddress // Include for backward compatibility
                    });
                    
                } catch (error) {
                    console.error("Offscreen: Error importing keystore:", error);
                    sendResponse({ 
                        success: false, 
                        error: error instanceof Error ? error.message : "Unknown error importing keystore"
                    });
                }
            })();
            
            // Return true to indicate async response
            return true;

        case "exportKeystore":
            console.log("Offscreen: Received 'exportKeystore' command", payload);
            
            // Ensure we have the WebRTC manager or DKG instance to export from
            if (!webRTCManager) {
                sendResponse({ success: false, error: "WebRTC manager not initialized" });
                return false;
            }
            
            (async () => {
                try {
                const chain = payload.chain || "ethereum";
                let keystoreData: string | null = null;
                
                // Try to get the keystore from the DKG manager
                const dkgManager = webRTCManager.getDkgManager();
                if (dkgManager) {
                    // Export keystore from WASM
                    const dkgInstance = dkgManager.getDkgInstance();
                    if (dkgInstance && typeof dkgInstance.export_keystore === 'function') {
                        keystoreData = dkgInstance.export_keystore();
                        console.log("Offscreen: Successfully exported keystore from DKG instance");
                    }
                }
                
                if (!keystoreData) {
                    // If no DKG manager, try direct WASM instances
                    // This handles the case where keystore was imported but DKG wasn't run
                    const { FrostDkgSecp256k1, FrostDkgEd25519 } = await import("@mpc-wallet/core-wasm");
                    
                    // Try both curve types since we don't know which one was imported
                    let dkgInstance;
                    if (chain === "ethereum") {
                        dkgInstance = new FrostDkgSecp256k1();
                    } else {
                        dkgInstance = new FrostDkgEd25519();
                    }
                    
                    // Note: This won't work if the keystore wasn't loaded in this session
                    // In a real implementation, you'd need to reload the keystore first
                    sendResponse({ 
                        success: false, 
                        error: "Keystore export requires an active session. Please ensure the wallet is loaded." 
                    });
                    return false;
                }
                
                sendResponse({
                    success: true,
                    keystoreData
                });
                
                } catch (error) {
                    console.error("Offscreen: Error exporting keystore:", error);
                    sendResponse({ 
                        success: false, 
                        error: error instanceof Error ? error.message : "Unknown error exporting keystore" 
                    });
                }
            })();
            
            // Return true to indicate async response
            return true;

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

// Track if we've already sent the ready signal
let readySignalSent = false;

// Add a small delay to ensure background script is ready to receive messages

chrome.runtime.sendMessage({ type: "offscreenReady" }, (response) => {
    if (chrome.runtime.lastError) {
        console.error("Offscreen: Error sending 'offscreenReady' or receiving ack from background:", chrome.runtime.lastError.message);

        // Retry sending the ready signal if it failed
        setTimeout(() => {
            if (readySignalSent) {
                console.log("Offscreen: Ready signal already acknowledged, skipping retry");
                return;
            }
            console.log("Offscreen: Retrying 'offscreenReady' signal...");
            chrome.runtime.sendMessage({ type: "offscreenReady" }, (retryResponse) => {
                if (chrome.runtime.lastError) {
                    console.error("Offscreen: Retry also failed:", chrome.runtime.lastError.message);

                    // Try one more time with longer delay
                    setTimeout(() => {
                        if (readySignalSent) {
                            console.log("Offscreen: Ready signal already acknowledged, skipping final retry");
                            return;
                        }
                        console.log("Offscreen: Final retry 'offscreenReady' signal...");
                        chrome.runtime.sendMessage({ type: "offscreenReady" }, (finalResponse) => {
                            if (chrome.runtime.lastError) {
                                console.error("Offscreen: Final retry failed:", chrome.runtime.lastError.message);
                            } else {
                                console.log("Offscreen: 'offscreenReady' final retry successful:", finalResponse);
                                readySignalSent = true;
                            }
                        });
                    }, 2000);
                } else {
                    console.log("Offscreen: 'offscreenReady' retry successful:", retryResponse);
                    readySignalSent = true;
                }
            });
        }, 1000);
    } else {
        console.log("Offscreen: 'offscreenReady' signal sent and acknowledged by background:", response);
        readySignalSent = true;

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
