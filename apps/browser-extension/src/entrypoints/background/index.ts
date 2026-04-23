// filepath: /home/freeman.xiong/Documents/github/hecoinfo/mpc-wallet/src/entrypoints/background/index.ts
// ===================================================================
// MAIN BACKGROUND SCRIPT COORDINATOR
// ===================================================================
//
// This is the main background script that coordinates all modular
// components for the MPC wallet extension. It imports and initializes
// all specialized managers and handlers:
//
// - SessionManager: Handles MPC session lifecycle
// - RpcHandler: Processes JSON-RPC and UI requests  
// - OffscreenManager: Manages Chrome Extension offscreen documents
// - WebSocketManager: Handles signaling server connections
// - StateManager: Manages central application state
// - Message Handlers: Process inter-component communications
// ===================================================================

import { defineBackground } from '#imports';
import { MESSAGE_PREFIX, MessageType } from '../../constants';
import AccountService from '../../services/accountService';
import NetworkService from '../../services/networkService';
import WalletClientService from '../../services/walletClient';
import { KeystoreManager } from '../../services/keystoreManager';
import { toHex } from 'viem';
import WalletController from "../../services/walletController";
import { WebSocketClient } from "./websocket";

// Import modular components
import { SessionManager } from './sessionManager';
import { RpcHandler, UIRequestHandler } from './rpcHandler';
import { OffscreenManager } from './offscreenManager';
import { WebSocketManager } from './webSocketManager';
import { SigningNotifier } from './signingNotification';
import { StateManager } from './stateManager';
import { PopupMessageHandler, OffscreenMessageHandler } from './messageHandlers';
import { KeepaliveController } from './keepaliveController';

// Import types
import { AppState, INITIAL_APP_STATE } from "@mpc-wallet/types/appstate";
import { SessionProposal, SessionResponse, SessionInfo } from "@mpc-wallet/types/session";
import { getSignalServerUrl } from "../../config/signal-server";
import { MeshStatusType, MeshStatus } from "@mpc-wallet/types/mesh";
import { DkgState } from "@mpc-wallet/types/dkg";
import {
    type JsonRpcRequest,
    type PopupToBackgroundMessage,
    type BackgroundToOffscreenMessage,
    type BackgroundToPopupMessage,
    type OffscreenToBackgroundMessage,
    type BackgroundToOffscreenWrapper,
    type InitialStateMessage,
    validateMessage,
    validateSessionProposal,
    validateSessionAcceptance,
    isRpcMessage,
    isAccountManagement,
    isNetworkManagement,
    isUIRequest,
    MESSAGE_TYPES,
    // Legacy aliases for backward compatibility
    type BackgroundMessage,
    type OffscreenMessage,
    type PopupMessage,
} from "@mpc-wallet/types/messages";
import { ServerMsg, ClientMsg, WebSocketMessagePayload, WebRTCSignal } from "@mpc-wallet/types/websocket";

// ===================================================================
// SERVICE INITIALIZATION AND GLOBAL STATE
// ===================================================================

// Initialize services
const accountService = AccountService.getInstance();
const networkService = NetworkService.getInstance();
const walletClientService = WalletClientService.getInstance();

// Initialize managers and handlers
let stateManager: StateManager;
let sessionManager: SessionManager;
let rpcHandler: RpcHandler;
let uiRequestHandler: UIRequestHandler;
let offscreenManager: OffscreenManager;
let webSocketManager: WebSocketManager;
let popupMessageHandler: PopupMessageHandler;
let offscreenMessageHandler: OffscreenMessageHandler;
let keepaliveController: KeepaliveController;

// Global state variables for legacy compatibility
let wsClient: WebSocketClient | null = null;
let devices: string[] = [];

// ===================================================================
// COMPONENT INITIALIZATION
// ===================================================================

/**
 * Initialize all modular components
 */
function initializeComponents(): void {
    console.log("�� [Background] Initializing modular components...");

    // Initialize state manager with initial state
    stateManager = new StateManager(INITIAL_APP_STATE);

    // Initialize RPC and UI request handlers (no parameters needed)
    rpcHandler = new RpcHandler();
    uiRequestHandler = new UIRequestHandler();
    
    // Set RPC handler on StateManager for signature callbacks
    stateManager.setRpcHandler(rpcHandler);

    // Initialize offscreen manager (needs app state)
    offscreenManager = new OffscreenManager(stateManager.getState());

    // Initialize session manager
    sessionManager = new SessionManager(
        stateManager.getState(),
        wsClient,
        (message) => stateManager.broadcastToPopupPorts(message),
        (message, description) => offscreenManager.sendToOffscreen(message, description),
        stateManager
    );

    // Ext-4: inject SessionManager into RpcHandler so
    // personal_sign / eth_sign RPCs can route through the TUI-
    // compatible FROST signing flow (createSigningSession) instead
    // of the legacy single-party DkgManager path.
    rpcHandler.setSessionManager(sessionManager);

    // Ext-3a: chrome.notifications push on incoming signing invites.
    // Guarded because chrome.notifications is only present when the
    // `notifications` manifest permission is granted AND we're in an
    // extension context (undefined in isolated bun test env). Passing
    // null disables push notifications silently — the sessionAvailable
    // flow still lights up the popup UI via broadcastToPopup.
    const signingNotifier =
        typeof chrome !== "undefined" && chrome.notifications
            ? new SigningNotifier({
                  notifications: chrome.notifications as any,
              })
            : undefined;

    // Initialize WebSocket manager (needs app state, session manager, broadcast function, send to offscreen function, and state manager)
    webSocketManager = new WebSocketManager(
        stateManager.getState(),
        sessionManager,
        (message) => stateManager.broadcastToPopupPorts(message),
        (message, description) => offscreenManager.sendToOffscreen(message, description),
        stateManager, // Add StateManager for persistence
        signingNotifier,
    );

    // Initialize message handlers with all dependencies
    popupMessageHandler = new PopupMessageHandler(
        stateManager,
        offscreenManager,
        webSocketManager,
        sessionManager,
        rpcHandler,
        uiRequestHandler
    );

    offscreenMessageHandler = new OffscreenMessageHandler(
        stateManager,
        webSocketManager
    );

    // Architectural reminder #2: keepalive for the offscreen document
    // during active DKG / signing ceremonies. Chrome kills offscreen
    // after ~30s idle; without these pings, mid-ceremony rounds die
    // and peers see timeouts. Subscribing here wires the controller
    // to StateManager's dkgState transitions, and the listener fires
    // immediately with the current state so a SW that woke with a
    // non-Idle dkgState already warms up the offscreen (defensive —
    // in practice SW wake starts from Idle because session data is
    // intentionally ephemeral).
    keepaliveController = new KeepaliveController();
    stateManager.addDkgStateListener((state) => {
        keepaliveController.onDkgStateChange(state);
    });

//     console.log("✅ [Background] All components initialized successfully");
}

// ===================================================================
// POPUP PORT MANAGEMENT
// ===================================================================

/**
 * Set up popup port connections
 */
function setupPopupConnections(): void {
    chrome.runtime.onConnect.addListener((port) => {
        if (port.name === "popup") {
//             console.log("🔌 [Background] Popup connected");
            stateManager.addPopupPort(port);
        }
    });
}

// ===================================================================
// MESSAGE HANDLING
// ===================================================================

/**
 * Handle incoming messages from popup and content scripts
 */
function setupMessageHandlers(): void {
    chrome.runtime.onMessage.addListener((message: unknown, sender, sendResponse) => {
        // Enhanced logging for message routing with RPC detection
        const isOffscreenSender = sender.url?.includes('offscreen') || sender.url?.includes('offscreen.html');
        const senderType = sender.tab ? 'content-script' : (sender.url?.includes('popup') ? 'popup' : (isOffscreenSender ? 'offscreen' : 'unknown'));
        const tabInfo = sender.tab ? `tab-${sender.tab.id}` : 'no-tab';

//         console.log("┌─────────────────────────────────────────────────────────────────");
        console.log(`│ [Background Router] 📨 Message Received`);
        console.log(`│ Type: ${(message as any)?.type || 'unknown'}`);
        console.log(`│ From: ${senderType} (${tabInfo})`);
        console.log(`│ URL: ${sender.url || 'unknown'}`);
        console.log(`│ Message:`, message);
//         console.log("└─────────────────────────────────────────────────────────────────");

        // Validate basic message structure
        if (!validateMessage(message)) {
            console.warn("❌ [Background] Invalid message structure:", message);
            sendResponse({ success: false, error: "Invalid message structure" });
            return true;
        }

        // Handle async operations
        (async () => {
            const startTime = Date.now();
            const messageType = (message as any).type;

            // Detect if this is an RPC message for special logging
            const isRpc = isRpcMessage(message as PopupToBackgroundMessage);
            const rpcMethod = isRpc ? (message as any).payload?.method : null;
            const rpcId = isRpc ? (message as any).payload?.id : null;

            try {
                if (isRpc) {
                    console.log(`🔄 [Background Router] Processing RPC ${rpcMethod} (ID: ${rpcId})...`);
                } else {
//                     console.log(`🔄 [Background Router] Processing ${messageType} message...`);
                }

                // Handle specific offscreen message types FIRST (before generic routing)

                // Handle offscreen ready signal
                if (message.type === MESSAGE_TYPES.OFFSCREEN_READY) {
                    console.log("🎯 [Background] Handling OFFSCREEN_READY signal");
                    await offscreenManager.handleOffscreenReady();

                    // Send init data when offscreen becomes ready
                    const currentState = stateManager.getState();
                    if (currentState.deviceId) {
                        console.log("🔄 [Background] Sending init data to offscreen");
                        const initResult = await offscreenManager.sendInitData(currentState.deviceId);
                        if (initResult.success) {
                            console.log("✅ [Background] Successfully sent init data to offscreen");
                        } else {
                            console.warn("❌ [Background] Failed to send init data to offscreen:", initResult.error);
                        }
                    }

                    console.log("✅ [Background] OffscreenReady handled successfully");
                    sendResponse({ success: true });
                    return;
                }

                // Route messages to appropriate handlers based on sender and message type
                if (message.type === "fromOffscreen" || senderType === 'offscreen' ||
                    (message.type === 'log' && isOffscreenSender)) {
//                     console.log("📤 [Background] Routing to OffscreenMessageHandler");

                    let payload: OffscreenToBackgroundMessage;
                    if (message.type === "fromOffscreen" && 'payload' in message) {
                        // Wrapped message format
                        payload = message.payload as OffscreenToBackgroundMessage;
                    } else {
                        // Direct message format from offscreen - convert safely
                        payload = message as unknown as OffscreenToBackgroundMessage;
                    }

                    await offscreenMessageHandler.handleOffscreenMessage(payload);
//                     console.log("✅ [Background] OffscreenMessage handled successfully");
                    sendResponse({ success: true });
                    return;
                }


                // Handle init requests
                if (message.type === "requestInit") {
//                     console.log("🔧 [Background] Handling requestInit from offscreen");
                    const result = await offscreenManager.handleInitRequest();
//                     console.log("✅ [Background] Init request completed:", result);
                    sendResponse(result);
                    return;
                }

                // Route to popup message handler for most messages
//                 console.log("📋 [Background] Routing to PopupMessageHandler");
                await popupMessageHandler.handlePopupMessage(message, sendResponse);

            } catch (error) {
                const duration = Date.now() - startTime;
                if (isRpc) {
                    console.error(`❌ [Background Router] RPC ${rpcMethod} (ID: ${rpcId}) failed after ${duration}ms:`, error);
                } else {
                    console.error(`❌ [Background Router] Error handling ${messageType} message after ${duration}ms:`, error);
                }
                sendResponse({ success: false, error: (error as Error).message });
            } finally {
                const duration = Date.now() - startTime;
                if (isRpc) {
                    console.log(`⏱️ [Background Router] 🔗 RPC ${rpcMethod} (ID: ${rpcId}) completed in ${duration}ms`);
                } else {
                    console.log(`⏱️ [Background Router] ${messageType} message processing completed in ${duration}ms`);
                }
            }
        })();

        return true;
    });
}

// ===================================================================
// SESSION RESTORATION
// ===================================================================


// ===================================================================
// INITIALIZATION AND CLEANUP
// ===================================================================

/**
 * Check for existing keystores and restore state if found
 */
export async function checkAndRestoreKeystores(): Promise<void> {
    console.log("🔑 [Background] Checking for existing keystores...");
    
    try {
        const keystoreManager = KeystoreManager.getInstance();
        
        // Initialize with device ID
        const deviceId = stateManager.getState().deviceId || 'mpc-2';
        await keystoreManager.initialize(deviceId);
        
        // Check if keystore is initialized
        if (!await keystoreManager.isInitialized()) {
            console.log("🔑 [Background] No keystore found");
            return;
        }
        
        // Check if keystore is locked
        if (keystoreManager.isLocked()) {
            console.log("🔑 [Background] Keystore is locked - password required");
            // The popup will handle password prompt
            return;
        }
        
        // Get active wallet
        const activeWallet = keystoreManager.getActiveWallet();
        if (!activeWallet) {
            console.log("🔑 [Background] No active wallet found");
            return;
        }
        
        console.log(`🔑 [Background] Restoring wallet: ${activeWallet.id}`);
        
        // Update DKG state to complete
        stateManager.updateStateProperty('dkgState', DkgState.Complete);
        
        // Update session info
        stateManager.updateStateProperty('sessionInfo', {
            session_id: activeWallet.session_id,
            proposer_id: activeWallet.id,
            participants: [activeWallet.id],
            accepted_devices: [activeWallet.id],
            threshold: 1, // Default for imported keystores
            total: 1,
        } as any);
        
        // Store addresses based on active wallet's blockchain
        if (activeWallet.blockchain === 'ethereum' && activeWallet.address) {
            stateManager.updateStateProperty('ethereumAddress', activeWallet.address);
            chrome.storage.local.set({ 
                'mpc_ethereum_address': activeWallet.address 
            });
        } else if (activeWallet.blockchain === 'solana' && activeWallet.address) {
            stateManager.updateStateProperty('solanaAddress', activeWallet.address);
            chrome.storage.local.set({ 
                'mpc_solana_address': activeWallet.address 
            });
        }
        
        stateManager.updateStateProperty('dkgAddress', activeWallet.address);
        
        console.log(`🔑 [Background] Wallet restored successfully: ${activeWallet.address}`);
    } catch (error) {
        console.error("❌ [Background] Error checking keystores:", error);
    }
}

/**
 * Initialize WebSocket connection
 */
async function initializeWebSocket(): Promise<void> {
    try {
        // Resolve from config: user override via chrome.storage.local
        // ['signalServerUrl'] wins; otherwise the TUI-matching default.
        // Migrating away from the old hardcoded `auto-life.tech` so TUI
        // and extension nodes actually land on the same signal server.
        const WEBSOCKET_URL = await getSignalServerUrl();

        // Generate device ID
        const deviceId = "mpc-2"; // TODO: Generate unique device ID
        stateManager.updateState({ deviceId });

        // Initialize WebSocket manager and connect
        await webSocketManager.initialize(WEBSOCKET_URL, deviceId);

        // Store WebSocket client reference for legacy compatibility
        wsClient = webSocketManager.getClient();

        console.log("🌐 [Background] WebSocket initialization complete");
    } catch (error) {
        console.error("❌ [Background] Failed to initialize WebSocket:", error);
        stateManager.updateWebSocketStatus(false, (error as Error).message);
    }
}

/**
 * Main background script entry point
 */
export default defineBackground(async () => {
//     console.log("🚀 [Background] Background script starting...");

    // Initialize all components
    initializeComponents();

    // Set up popup connections
    setupPopupConnections();

    // Set up message handlers
    setupMessageHandlers();

    // Ext-3a: open the popup when the user clicks a signing-request
    // notification. chrome.action.openPopup() requires user gesture
    // context, which a notification click provides. If the popup
    // API isn't available (Firefox lacks it), fall back to opening
    // popup.html in a tab so the user still has a path forward.
    if (typeof chrome !== "undefined" && chrome.notifications) {
        chrome.notifications.onClicked.addListener(async (notificationId) => {
            if (!notificationId.startsWith("mpc-signing-req:")) return;
            try {
                if (chrome.action && (chrome.action as any).openPopup) {
                    await (chrome.action as any).openPopup();
                } else {
                    await chrome.tabs.create({
                        url: chrome.runtime.getURL("popup.html"),
                    });
                }
                chrome.notifications.clear(notificationId);
            } catch (e) {
                console.warn(
                    "[Background] Failed to open popup from notification:",
                    e,
                );
            }
        });
    }

    // Check for existing keystores and restore state
    await checkAndRestoreKeystores();

    // Initialize offscreen document on startup
    offscreenManager.createOffscreenDocument().then((result: any) => {
        console.log("🖥️ [Background] Initial offscreen document setup:", result);
    });

    // Initialize WebSocket connection
    initializeWebSocket();

    // Start with fresh session state on extension startup
//     console.log("🔄 [Background] Extension starting up - sessions are ephemeral, starting fresh");
//     console.log("✅ [Background] Extension ready");

    // No need to clean up session state on shutdown since we don't persist it

//     console.log("🎉 [Background] Background script initialized successfully");
});
