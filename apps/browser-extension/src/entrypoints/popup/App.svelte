<script lang="ts">
    import svelteLogo from "../../assets/svelte.svg";
    // Removed single-party WASM functions - this is now an MPC-only wallet
    import { onMount, onDestroy } from "svelte";
    import { storage } from "#imports";
    import type { AppState } from "@mpc-wallet/types/appstate";
    import { MeshStatusType } from "@mpc-wallet/types/mesh";
    import { DkgState } from "@mpc-wallet/types/dkg";
    import { INITIAL_APP_STATE } from "@mpc-wallet/types/appstate";
    import Settings from "../../components/Settings.svelte";
    import AccountManager from "../../components/AccountManager.svelte";
    import SignatureRequest from "../../components/SignatureRequest.svelte";
    import PasswordPrompt from "./components/PasswordPrompt.svelte";
    import WalletSelector from "./components/WalletSelector.svelte";
    import CreateWalletForm from "../../components/CreateWalletForm.svelte";
    import { MESSAGE_TYPES } from "@mpc-wallet/types/messages";

    // Ext-1b: toggled when the user clicks "+ Create Wallet". Shows
    // the CreateWalletForm overlay until they submit or cancel.
    let showCreateWallet = false;

    // Application state (consolidated from background) - the single source of truth
    let appState: AppState = { ...INITIAL_APP_STATE };

    // Keep connection to background script
    let port: chrome.runtime.Port;
    
    // UI state flags
    let acceptingSession = false; // Prevent multiple session accept clicks
    let selectedDevices: Set<string> = new Set(); // Track selected devices for session proposal
    
    // Signature request tracking
    let signatureRequests: Array<{
        signingId: string;
        message: string;
        origin: string;
        fromAddress: string;
    }> = [];
    
    // Keystore state
    let keystoreStatus = {
        initialized: false,
        locked: true,
        wallets: [],
        activeWallet: null,
        pendingImport: false
    };
    
    // Password prompt state
    let showPasswordPrompt = false;
    let passwordPromptConfig = {
        title: "Unlock Keystore",
        message: "Enter your password to unlock the wallet",
        confirmMode: false,
        onSubmit: null,
        onCancel: null
    };

    // Local storage for UI preferences persistence (not real-time connection state)
    const UI_STATE_KEY = "mpc_wallet_ui_preferences";

    // Save ONLY UI preferences to localStorage (not real-time connection states)
    function saveUIState() {
        const uiState = {
            showSettings: appState.showSettings,
            proposedSessionIdInput: appState.proposedSessionIdInput,
            totalParticipants: appState.totalParticipants,
            threshold: appState.threshold,
            chain: appState.chain,
            timestamp: Date.now(),
        };
        try {
            localStorage.setItem(UI_STATE_KEY, JSON.stringify(uiState));
//             console.log("[UI] Saved UI preferences to localStorage:", uiState);
        } catch (error) {
            console.warn("[UI] Failed to save UI preferences:", error);
        }
    }

    // Load ONLY UI preferences from localStorage (not real-time connection states)
    function loadUIState(): Partial<AppState> {
        try {
            const stored = localStorage.getItem(UI_STATE_KEY);
            if (stored) {
                const uiState = JSON.parse(stored);
                // Check if state is not too old (1 hour)
                if (Date.now() - uiState.timestamp < 60 * 60 * 1000) {
                    console.log(
                        "[UI] Loaded UI preferences from localStorage:",
                        uiState,
                    );
                    return {
                        showSettings: uiState.showSettings || false,
                        proposedSessionIdInput:
                            uiState.proposedSessionIdInput || "",
                        totalParticipants: uiState.totalParticipants || 3,
                        threshold: uiState.threshold || 2,
                        chain: uiState.chain || "ethereum",
                    };
                } else {
//                     console.log("[UI] UI preferences expired, using defaults");
                    localStorage.removeItem(UI_STATE_KEY);
                }
            }
        } catch (error) {
            console.warn("[UI] Failed to load UI preferences:", error);
            localStorage.removeItem(UI_STATE_KEY);
        }
        return {};
    }

    // Debug logging to console (throttled to prevent spam)
    let lastDebugLog = "";
    $: {
        const debugInfo = JSON.stringify({
            dkgState: appState.dkgState,
            chainChanged: appState.chain,
            sessionActive: !!appState.sessionInfo,
            meshReady: appState.meshStatus?.type === MeshStatusType.Ready,
        });

        // Only log when significant state changes occur
        if (debugInfo !== lastDebugLog) {
            console.log("[UI Debug] Significant state change:", {
                dkgState: appState.dkgState,
                chain: appState.chain,
                hasSession: !!appState.sessionInfo,
                meshReady: appState.meshStatus?.type === MeshStatusType.Ready,
            });
            lastDebugLog = debugInfo;
        }
    }

    // Reactive computation for WebRTC connection status
    $: webrtcConnected =
        appState.sessionInfo &&
        appState.meshStatus?.type === MeshStatusType.Ready;
    $: webrtcConnecting =
        appState.sessionInfo &&
        appState.meshStatus?.type === MeshStatusType.PartiallyReady;

    // Add reactive statement to log WebRTC connection changes (throttled)
    let lastWebRTCState = "";
    $: {
        // Only log when WebRTC connection state actually changes
        const webrtcState = JSON.stringify(appState.webrtcConnections);
        if (webrtcState !== lastWebRTCState) {
            console.log(
                "[UI] WebRTC Connections updated:",
                appState.webrtcConnections,
            );
            if (Object.keys(appState.webrtcConnections).length > 0) {
                console.log(
                    "[UI] Active WebRTC connections:",
                    Object.entries(appState.webrtcConnections).filter(
                        ([_, connected]) => connected,
                    ),
                );
            }
            lastWebRTCState = webrtcState;
        }
    }

    // Common handler for messages from the background script
    function handleBackgroundMessage(message: any) {
        console.log(
            "[UI] Background message received - Type:",
            message.type,
            "Data:",
            message,
        );

        switch (message.type) {
            case "initialState":
                console.log(
                    "[UI] Processing initialState - state restoration from background",
                );

                // Load persisted UI preferences from localStorage (ONLY UI state, not real-time)
                const persistedUIState = loadUIState();

                // Preserve current UI-specific preferences that aren't managed by background
                const currentUIState = {
                    showSettings: appState.showSettings,
                    proposedSessionIdInput: appState.proposedSessionIdInput,
                    totalParticipants: appState.totalParticipants,
                    threshold: appState.threshold,
                    ...persistedUIState, // Override with persisted preferences if available
                };

                // Update the entire app state from background - real-time state comes from background
                appState = {
                    // Real-time state from background (never from localStorage)
                    deviceId: message.deviceId || "",
                    connecteddevices: [...(message.connecteddevices || [])],
                    wsConnected: message.wsConnected || false,
                    sessionInfo: message.sessionInfo || null,
                    invites: message.invites ? [...message.invites] : [],
                    meshStatus: message.meshStatus || {
                        type: MeshStatusType.Incomplete,
                    },
                    dkgState: message.dkgState || DkgState.Idle,
                    webrtcConnections: message.webrtcConnections || {},
                    curve:
                        message.curve ||
                        (message.blockchain === "ethereum"
                            ? "secp256k1"
                            : "ed25519"),
                    uiPreferences: message.uiPreferences || {
                        darkMode: false,
                        language: "en",
                        showAdvanced: false,
                    },
                    // UI preferences - preserve from current popup state or use localStorage
                    showSettings: currentUIState.showSettings,
                    chain:
                        message.blockchain ||
                        currentUIState.chain ||
                        "ethereum",
                    proposedSessionIdInput:
                        currentUIState.proposedSessionIdInput,
                    totalParticipants: currentUIState.totalParticipants,
                    threshold: currentUIState.threshold,
                    // Other state fields
                    dkgAddress: appState.dkgAddress || "",
                    dkgError: appState.dkgError || "",
                    sessionAcceptanceStatus:
                        message.sessionAcceptanceStatus ||
                        appState.sessionAcceptanceStatus ||
                        {},
                    wsError: message.wsError || appState.wsError || "",
                    isInitializing: message.isInitializing,
                    globalError: message.globalError,
                    setupComplete: message.setupComplete,
                };
                console.log(
                    "[UI] App state updated from initialState:",
                    appState,
                );
                initialStateLoaded = true; // Enable reactive saving after state is loaded

                // Save the current UI preferences immediately after loading to ensure persistence
                saveUIState();
                
                // Check keystore status after initial state is loaded
                checkKeystoreStatus();

                // NOTE: No need for fresh state update - all real-time state comes from background automatically
                // WebSocket updates, device lists, etc. are pushed via port messages, not pulled via getState
                break;

            case "wsStatus":
//                 console.log("[UI] Processing wsStatus:", message);
                appState.wsConnected = message.connected || false;
                if (!message.connected && message.reason) {
                    appState.wsError = `WebSocket disconnected: ${message.reason}`;
                } else if (message.connected) {
                    appState.wsError = "";
                }
                // Trigger reactivity
                appState = { ...appState };
                break;

            case "wsMessage":
//                 console.log("[UI] Processing wsMessage:", message);
                if (message.message) {
                    console.log("[UI] Server message:", message.message);
                    // Device list updates are handled via "deviceList" messages
                    // No need to handle them here
                }
                break;

            case "wsError":
//                 console.log("[UI] Processing wsError:", message);
                appState.wsError = message.error;
                console.error("[UI] WebSocket error:", message.error);
                // Trigger reactivity
                appState = { ...appState };
                break;

            case "deviceList":
//                 console.log("[UI] Processing deviceList:", message);
                // Only update if we have a valid devices array
                if (Array.isArray(message.devices)) {
                    const newConnectedDevices = [...message.devices];
                    console.log("[UI] Updated connected devices:", newConnectedDevices);
                    // Update the entire appState to ensure Svelte detects the change
                    appState = {
                        ...appState,
                        connecteddevices: newConnectedDevices
                    };
                } else {
                    console.warn("[UI] Invalid device list received:", message.devices);
                }
                break;

            case "sessionUpdate":
//                 console.log("[UI] Processing sessionUpdate:", message);
                appState.sessionInfo = message.sessionInfo || null;
                appState.invites = message.invites ? [...message.invites] : [];
                console.log("[UI] Session update:", {
                    sessionInfo: appState.sessionInfo,
                    invites: appState.invites,
                });

                // Log accepted devices for debugging
                if (
                    appState.sessionInfo &&
                    appState.sessionInfo.accepted_devices
                ) {
                    console.log(
                        "[UI] Session accepted devices:",
                        appState.sessionInfo.accepted_devices,
                    );
                    // Filter out any null/undefined values that might have been added
                    appState.sessionInfo.accepted_devices =
                        appState.sessionInfo.accepted_devices.filter(
                            (peer) => peer != null && peer !== undefined,
                        );
                }
                // Trigger reactivity
                appState = { ...appState };
                break;

            case "meshStatusUpdate":
//                 console.log("[UI] Processing meshStatusUpdate:", message);
                const newMeshStatus = message.status || {
                    type: MeshStatusType.Incomplete,
                };
                console.log("[UI] Mesh status update:", newMeshStatus);
                // Update the entire appState to ensure Svelte detects the change
                appState = {
                    ...appState,
                    meshStatus: newMeshStatus
                };
                break;

            case "webrtcConnectionUpdate":
//                 console.log("[UI] Processing webrtcConnectionUpdate:", message);

                if (
                    message.deviceId &&
                    typeof message.connected === "boolean"
                ) {
                    console.log(
                        "[UI] Updating peer connection:",
                        message.deviceId,
                        "->",
                        message.connected,
                    );

                    // Create a new webrtcConnections object to ensure reactivity
                    const newWebrtcConnections = {
                        ...appState.webrtcConnections,
                        [message.deviceId]: message.connected,
                    };

                    // Update the entire appState to ensure Svelte detects the change
                    appState = {
                        ...appState,
                        webrtcConnections: newWebrtcConnections
                    };

                    console.log(
                        "[UI] Updated webrtcConnections:",
                        appState.webrtcConnections,
                    );
                } else {
                    console.warn(
                        "[UI] Invalid webrtcConnectionUpdate message:",
                        message,
                    );
                }
                break;

            case "dkgStateUpdate":
//                 console.log("[UI] Processing dkgStateUpdate:", message);
                appState.dkgState = message.state || DkgState.Idle;
                console.log("[UI] DKG state update:", appState.dkgState);
                // Trigger reactivity
                appState = { ...appState };
                break;

            case "fromOffscreen":
//                 console.log("[UI] Processing fromOffscreen wrapper:", message);
                // Handle wrapped messages from offscreen
                if (message.payload) {
                    console.log(
                        "[UI] Unwrapping and processing payload:",
                        message.payload,
                    );
                    handleBackgroundMessage(message.payload);
                }
                break;

            case "webrtcStatusUpdate":
//                 console.log("[UI] Processing webrtcStatusUpdate:", message);
                if (message.deviceId && message.status) {
                    console.log(
                        `[UI] WebRTC status for ${message.deviceId}: ${message.status}`,
                    );
                    // Update UI state based on WebRTC status if needed
                }
                break;

            case "dataChannelStatusUpdate":
                console.log(
                    "[UI] Processing dataChannelStatusUpdate:",
                    message,
                );
                if (message.deviceId && message.channelName && message.state) {
                    console.log(
                        `[UI] Data channel ${message.channelName} for ${message.deviceId}: ${message.state}`,
                    );
                }
                break;

            case "peerConnectionStatusUpdate":
                console.log(
                    "[UI] Processing peerConnectionStatusUpdate:",
                    message,
                );
                if (message.deviceId && message.connectionState) {
                    console.log(
                        `[UI] Peer connection for ${message.deviceId}: ${message.connectionState}`,
                    );
                }
                break;

            case "dkgAddressUpdate":
//                 console.log("[UI] Processing dkgAddressUpdate:", message);
                if (message.address && message.blockchain) {
                    console.log(
                        "[UI] DKG address automatically fetched:",
                        message.address,
                        "for",
                        message.blockchain,
                    );
                    appState.dkgAddress = message.address;
                    appState.dkgError = "";
                    // Trigger reactivity
                    appState = { ...appState };
                }
                break;
                
            case "accountsUpdated":
//                 console.log("[UI] Processing accountsUpdated:", message);
                // Trigger account refresh in AccountManager component
                // The AccountManager component will automatically refresh when AccountService notifies it
                if (message.blockchain && message.accounts) {
                    console.log(`[UI] Accounts updated for ${message.blockchain}:`, message.accounts);
                    // Force a re-render by updating app state
                    appState = { ...appState, accountsUpdated: Date.now() };
                }
                break;
                
            case "signatureRequest":
//                 console.log("[UI] Processing signatureRequest:", message);
                if (message.signingId && message.message && message.origin && message.fromAddress) {
                    // Add new signature request to the list
                    signatureRequests = [...signatureRequests, {
                        signingId: message.signingId,
                        message: message.message,
                        origin: message.origin,
                        fromAddress: message.fromAddress
                    }];
                }
                break;
                
            case "signatureComplete":
//                 console.log("[UI] Processing signatureComplete:", message);
                if (message.signingId) {
                    // Remove the completed request from the list
                    signatureRequests = signatureRequests.filter(req => req.signingId !== message.signingId);
                }
                break;
                
            case "signatureError":
//                 console.log("[UI] Processing signatureError:", message);
                if (message.signingId) {
                    // Remove the failed request from the list
                    signatureRequests = signatureRequests.filter(req => req.signingId !== message.signingId);
                    // TODO: Show error notification
                }
                break;

            default:
                console.log(
                    "[UI] Unhandled message type:",
                    message.type,
                    message,
                );
        }
    }

    // Removed ensurePrivateKey() and ensureOffscreenDocument() - this is now an MPC-only wallet
    // Offscreen document management is handled entirely by the background script

    onMount(async () => {
        console.log("[UI] Component mounting");

        // Set up state tracking before connecting port
        let stateReceived = false;
        let fallbackTimeoutId: ReturnType<typeof setTimeout>;

        // Initialize as false to prevent reactive statements from running
        initialStateLoaded = false;

        port = chrome.runtime.connect({ name: "popup" });
        console.log(
            "[UI] Port connected to background, waiting for initial state...",
        );

        port.onMessage.addListener((message) => {
            console.log("[UI] Port message received:", message.type, message);
            // Track when initial state is received
            if (message.type === "initialState" && !stateReceived) {
                stateReceived = true;
                console.log(
                    "[UI] Initial state received successfully from StateManager",
                );
                // Clear fallback timeout since we received state
                if (fallbackTimeoutId) {
                    clearTimeout(fallbackTimeoutId);
                }
            }
            handleBackgroundMessage(message);
        });

        port.onDisconnect.addListener(() => {
            console.error("[UI] Port disconnected from background");
            appState.wsConnected = false;
            // Clear fallback timeout if port disconnects
            if (fallbackTimeoutId) {
                clearTimeout(fallbackTimeoutId);
            }
        });

        console.log(
            "[UI] Port connected, StateManager should automatically send state...",
        );

        // Add fallback in case automatic state is delayed (StateManager still loading, etc.)
        fallbackTimeoutId = setTimeout(() => {
            if (!stateReceived) {
                console.warn(
                    "[UI] Automatic state not received within 2 seconds, requesting manually as fallback",
                );
                chrome.runtime.sendMessage({ type: "getState" }, (response) => {
                    if (chrome.runtime.lastError) {
                        console.error(
                            "[UI] Fallback getState error:",
                            chrome.runtime.lastError.message,
                        );
                        return;
                    }
                    if (response && !stateReceived) {
                        console.log(
                            "[UI] Fallback state response received:",
                            response,
                        );
                        handleBackgroundMessage({
                            type: "initialState",
                            ...response,
                        });
                        stateReceived = true;
                    }
                });
            } else {
                console.log(
                    "[UI] State was received automatically, no fallback needed",
                );
            }
        }, 2000); // Increased to 2 seconds to account for async state loading

        // Removed ensurePrivateKey() call - this is now an MPC-only wallet
        // Removed ensureOffscreenDocument() call - offscreen management is handled by background script
    });

    onDestroy(() => {
        console.log("[UI] Component destroying, cleaning up port connection");
        if (port) {
            port.disconnect();
        }
    });

    // Removed reactive statement for ensurePrivateKey() - this is now an MPC-only wallet

    // Removed single-party reactive statements - this is now an MPC-only wallet

    // REMOVED: All reactive business logic moved to background script
    // The popup should ONLY contain pure UI reactive statements
    // All blockchain selection, state management, etc. happens in background
    // DKG address fetching is now handled automatically by StateManager

    // Reactive statements to save UI preferences to localStorage (UI-only)
    // Only save after initial state has been loaded to prevent premature saves
    let initialStateLoaded = false;
    let lastSavedState = "";

    // Throttled save function to prevent excessive localStorage writes (UI preferences only)
    function throttledSaveUIState() {
        const currentStateStr = JSON.stringify({
            showSettings: appState.showSettings,
            proposedSessionIdInput: appState.proposedSessionIdInput,
            totalParticipants: appState.totalParticipants,
            threshold: appState.threshold,
            chain: appState.chain,
        });

        // Only save if UI preferences actually changed
        if (currentStateStr !== lastSavedState) {
            saveUIState();
            lastSavedState = currentStateStr;
        }
    }

    // Save showSettings changes
    $: if (initialStateLoaded && typeof appState.showSettings !== "undefined") {
        throttledSaveUIState();
    }

    // Save proposedSessionIdInput changes
    $: if (
        initialStateLoaded &&
        typeof appState.proposedSessionIdInput !== "undefined"
    ) {
        throttledSaveUIState();
    }

    // Save totalParticipants changes
    $: if (
        initialStateLoaded &&
        typeof appState.totalParticipants !== "undefined"
    ) {
        throttledSaveUIState();
    }

    // Save threshold changes
    $: if (initialStateLoaded && typeof appState.threshold !== "undefined") {
        throttledSaveUIState();
    }

    // Save chain changes (in addition to the blockchain message sending)
    $: if (initialStateLoaded && typeof appState.chain !== "undefined") {
        throttledSaveUIState();
    }

    // Removed fetchAddress() and fetchDkgAddress() - this is now an MPC-only wallet
    // DKG address fetching is now handled automatically by StateManager when DKG completes

    // Removed signDemoMessage() - this is now an MPC-only wallet

    function requestdeviceList() {
        console.log("[UI] Requesting peer list");
        chrome.runtime.sendMessage({ type: "listdevices" }, (response) => {
            if (chrome.runtime.lastError) {
                console.error(
                    "[UI] Error requesting peer list:",
                    chrome.runtime.lastError.message,
                );
                return;
            }
            console.log("[UI] listdevices response:", response);
        });
    }

    function proposeSession() {
        // Convert Set to Array for selected devices
        const selectedDevicesList = Array.from(selectedDevices);
        
        // Validate selection
        if (selectedDevicesList.length !== appState.totalParticipants - 1) {
            console.error(
                `Please select exactly ${appState.totalParticipants - 1} devices for a ${appState.totalParticipants}-participant session. Currently selected: ${selectedDevicesList.length}`,
            );
            alert(`Please select exactly ${appState.totalParticipants - 1} devices for a ${appState.totalParticipants}-participant session`);
            return;
        }

        if (appState.threshold > appState.totalParticipants) {
            console.error(
                "Threshold cannot be greater than total participants",
            );
            return;
        }

        if (appState.threshold < 1) {
            console.error("Threshold must be at least 1");
            return;
        }

        // Include self and selected devices
        const allParticipants = [appState.deviceId, ...selectedDevicesList];

        const sessionId =
            appState.proposedSessionIdInput.trim() ||
            `wallet_${appState.threshold}of${appState.totalParticipants}_${Date.now()}`;

        chrome.runtime.sendMessage({
            type: "proposeSession",
            session_id: sessionId,
            total: appState.totalParticipants,
            threshold: appState.threshold,
            participants: allParticipants,
            blockchain: appState.chain, // Include blockchain selection
        });
        console.log(
            "[UI] Proposing session:",
            sessionId,
            `(${appState.threshold}-of-${appState.totalParticipants})`,
            "with participants:",
            allParticipants,
        );
        
        // Clear selection after proposing
        selectedDevices.clear();
        selectedDevices = selectedDevices; // Trigger reactivity
    }

    function acceptInvite(sessionId: string) {
        // Prevent multiple clicks
        if (acceptingSession) {
            console.warn("[UI] Already processing a session acceptance");
            return;
        }
        
        // Check if invite still exists before accepting
        const invite = appState.invites.find(inv => inv.session_id === sessionId);
        if (!invite) {
            console.warn("[UI] Session invite not found:", sessionId);
            return;
        }
        
        // Check if we already have an active session
        if (appState.sessionInfo && appState.sessionInfo.session_id === sessionId) {
            console.warn("[UI] Session already accepted:", sessionId);
            return;
        }
        
        // Set flag to prevent multiple clicks
        acceptingSession = true;
        
        chrome.runtime.sendMessage({
            type: "acceptSession",
            session_id: sessionId,
            accepted: true,
            blockchain: appState.chain, // Include blockchain selection
        }, (response) => {
            acceptingSession = false; // Reset flag
            console.log("[UI] Accept session response:", response);
            if (!response || !response.success) {
                console.error("[UI] Failed to accept session:", response?.error || "Unknown error");
                // Restore the invite if acceptance failed
                appState.invites = [...appState.invites, invite];
            }
        });
        console.log(
            "[UI] Accepting session invite:",
            sessionId,
            "with blockchain:",
            appState.chain,
        );
        
        // Optimistically update UI to show processing state
        appState.invites = appState.invites.filter(inv => inv.session_id !== sessionId);
    }

    function rejectInvite(sessionId: string) {
        // Remove the invite from local state
        appState.invites = appState.invites.filter(inv => inv.session_id !== sessionId);
        console.log("[UI] Rejected session invite:", sessionId);
        
        // Optionally send rejection to background (for future implementation)
        chrome.runtime.sendMessage({
            type: "rejectSession",
            session_id: sessionId
        });
    }

    // Add function to send direct message for testing
    function sendDirectMessage(todeviceId: string) {
        const testMessage = `Hello from ${appState.deviceId} at ${new Date().toLocaleTimeString()}`;
        chrome.runtime.sendMessage(
            {
                type: "sendDirectMessage",
                todeviceId: todeviceId,
                message: testMessage,
            },
            (response) => {
                if (chrome.runtime.lastError) {
                    console.error(
                        "[UI] Error sending direct message:",
                        chrome.runtime.lastError.message,
                    );
                } else {
                    console.log("[UI] Direct message response:", response);
                    if (!response.success) {
                        console.error(
                            `Failed to send message: ${response.error}`,
                        );
                    }
                }
            },
        );
        console.log(
            "[UI] Sending direct message to:",
            todeviceId,
            "Message:",
            testMessage,
        );
    }

    // Helper function to get WebRTC status for a peer
    function getWebRTCStatus(
        deviceId: string,
    ): "connected" | "connecting" | "disconnected" {
        console.log(
            "[UI] Getting WebRTC status for peer:",
            deviceId,
            "from webrtcConnections:",
            appState.webrtcConnections,
        );

        // Check direct connection status first
        if (appState.webrtcConnections[deviceId] === true) {
            return "connected";
        } else if (
            appState.sessionInfo &&
            appState.sessionInfo.participants.includes(deviceId) &&
            appState.meshStatus?.type === MeshStatusType.PartiallyReady
        ) {
            return "connecting";
        } else {
            return "disconnected";
        }
    }

    // Helper function to get session acceptance status
    function getSessionAcceptanceStatus(
        sessionId: string,
        deviceId: string,
    ): boolean | undefined {
        if (!appState.sessionAcceptanceStatus[sessionId]) {
            return undefined;
        }
        return appState.sessionAcceptanceStatus[sessionId][deviceId];
    }

    // Handle signature request completion
    function handleSignatureRequestComplete(event: CustomEvent) {
        const signingId = event.detail;
        // Remove from list (already handled by background message)
        signatureRequests = signatureRequests.filter(req => req.signingId !== signingId);
    }
    
    // Test MPC signing function
    function testMPCSigning() {
        console.log("[UI] Testing MPC signing");
        
        // Generate a test signing ID
        const signingId = `test_signing_${Date.now()}`;
        
        // Use hex of "hello" (68656c6c6f) to match CLI node test
        const testTransactionData = "68656c6c6f";
        
        // Use the threshold from the current session
        const requiredSigners = appState.sessionInfo?.threshold || 2;
        
        chrome.runtime.sendMessage({
            type: "requestSigning",
            signingId: signingId,
            transactionData: testTransactionData,
            requiredSigners: requiredSigners
        }, (response) => {
            if (chrome.runtime.lastError) {
                console.error("[UI] Error requesting signing:", chrome.runtime.lastError.message);
                return;
            }
            console.log("[UI] Signing request response:", response);
            if (!response.success) {
                console.error(`Failed to initiate signing: ${response.error}`);
            }
        });
        
        console.log("[UI] Sent signing request:", {
            signingId,
            transactionData: testTransactionData + ' (hex of "hello")',
            requiredSigners
        });
    }
    
    // Keystore management functions
    async function checkKeystoreStatus() {
        chrome.runtime.sendMessage({ type: MESSAGE_TYPES.GET_KEYSTORE_STATUS }, (response) => {
            if (chrome.runtime.lastError) {
                console.error("[UI] Error getting keystore status:", chrome.runtime.lastError.message);
                return;
            }
            if (response && response.success) {
                keystoreStatus = response.status;
                console.log("[UI] Keystore status:", keystoreStatus);
                
                // Check if we need to prompt for password
                if (keystoreStatus.initialized && keystoreStatus.locked) {
                    showUnlockPrompt();
                }
                
                // Check for pending imports
                checkPendingImports();
            }
        });
    }
    
    function showUnlockPrompt() {
        passwordPromptConfig = {
            title: "Unlock Wallet",
            message: "Enter your password to unlock the wallet",
            confirmMode: false,
            onSubmit: handleUnlockPassword,
            onCancel: () => { showPasswordPrompt = false; }
        };
        showPasswordPrompt = true;
    }
    
    function showCreateKeystorePrompt() {
        passwordPromptConfig = {
            title: "Create Wallet",
            message: "Create a password to secure your wallet",
            confirmMode: true,
            onSubmit: handleCreateKeystore,
            onCancel: () => { showPasswordPrompt = false; }
        };
        showPasswordPrompt = true;
    }
    
    async function handleUnlockPassword(event: CustomEvent) {
        const password = event.detail.password;
        showPasswordPrompt = false;
        
        chrome.runtime.sendMessage({ 
            type: MESSAGE_TYPES.UNLOCK_KEYSTORE,
            password,
            rememberDuration: 15 * 60 * 1000 // 15 minutes
        }, (response) => {
            if (chrome.runtime.lastError) {
                console.error("[UI] Error unlocking keystore:", chrome.runtime.lastError.message);
                return;
            }
            if (response && response.success) {
                keystoreStatus.locked = false;
                keystoreStatus.wallets = response.wallets;
                keystoreStatus.activeWallet = response.activeWallet;
                console.log("[UI] Keystore unlocked successfully");
                
                // Check for pending imports after unlock
                checkPendingImports();
            } else {
                console.error("[UI] Failed to unlock keystore:", response?.error);
                // Show error and prompt again
                setTimeout(() => showUnlockPrompt(), 500);
            }
        });
    }
    
    async function handleCreateKeystore(event: CustomEvent) {
        const password = event.detail.password;
        showPasswordPrompt = false;
        
        chrome.runtime.sendMessage({ 
            type: MESSAGE_TYPES.CREATE_KEYSTORE,
            password
        }, (response) => {
            if (chrome.runtime.lastError) {
                console.error("[UI] Error creating keystore:", chrome.runtime.lastError.message);
                return;
            }
            if (response && response.success) {
                keystoreStatus.initialized = true;
                keystoreStatus.locked = false;
                console.log("[UI] Keystore created successfully");
                checkKeystoreStatus();
            } else {
                console.error("[UI] Failed to create keystore:", response?.error);
            }
        });
    }
    
    async function checkPendingImports() {
        const result = await chrome.storage.local.get(['mpc_pending_import']);
        if (result.mpc_pending_import && !keystoreStatus.locked) {
            // Prompt for migration
            if (confirm("You have imported keystores waiting to be migrated. Would you like to migrate them now?")) {
                migrateKeystores();
            }
        }
    }
    
    async function migrateKeystores() {
        chrome.runtime.sendMessage({ 
            type: MESSAGE_TYPES.MIGRATE_KEYSTORES,
            password: "" // Will be prompted in background if needed
        }, (response) => {
            if (chrome.runtime.lastError) {
                console.error("[UI] Error migrating keystores:", chrome.runtime.lastError.message);
                return;
            }
            if (response && response.success) {
                console.log(`[UI] Migrated ${response.migratedCount} keystores`);
                keystoreStatus.wallets = response.wallets;
                chrome.storage.local.remove(['mpc_pending_import']);
            } else {
                console.error("[UI] Failed to migrate keystores:", response?.error);
            }
        });
    }
    
    function handleWalletSelect(event: CustomEvent) {
        const wallet = event.detail;
        chrome.runtime.sendMessage({ 
            type: MESSAGE_TYPES.SWITCH_WALLET,
            walletId: wallet.id
        }, (response) => {
            if (chrome.runtime.lastError) {
                console.error("[UI] Error switching wallet:", chrome.runtime.lastError.message);
                return;
            }
            if (response && response.success) {
                keystoreStatus.activeWallet = wallet;
                console.log("[UI] Switched to wallet:", wallet.id);
            } else {
                console.error("[UI] Failed to switch wallet:", response?.error);
            }
        });
    }
    
    function lockKeystore() {
        chrome.runtime.sendMessage({ type: MESSAGE_TYPES.LOCK_KEYSTORE }, (response) => {
            if (chrome.runtime.lastError) {
                console.error("[UI] Error locking keystore:", chrome.runtime.lastError.message);
                return;
            }
            if (response && response.success) {
                keystoreStatus.locked = true;
                console.log("[UI] Keystore locked");
            }
        });
    }
</script>

<main class="p-4 max-w-2xl mx-auto">
    <div class="text-center mb-6 flex justify-between items-center">
        <img src={svelteLogo} class="logo svelte mb-2" alt="Svelte Logo" />
        <h1 class="text-3xl font-bold flex-grow text-center">MPC Wallet</h1>
        <button
            class="bg-blue-500 hover:bg-blue-600 text-white p-2 rounded-full"
            on:click={() => {
                appState.showSettings = !appState.showSettings;
                appState = { ...appState };
            }}
            aria-label="Settings"
            title="Settings"
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                class="h-6 w-6"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
            >
                <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
                />
                <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                />
            </svg>
        </button>
    </div>
    
    <!-- Wallet Selector -->
    {#if keystoreStatus.initialized && !keystoreStatus.locked && keystoreStatus.wallets.length > 0}
        <div class="mb-4">
            <WalletSelector
                wallets={keystoreStatus.wallets}
                activeWallet={keystoreStatus.activeWallet}
                on:select={handleWalletSelect}
                on:add={() => console.log("[UI] Add wallet clicked")}
                on:manage={() => console.log("[UI] Manage wallets clicked")}
            />
        </div>
    {/if}

    {#if appState.showSettings}
        <Settings
            on:backToWallet={({ detail }) => {
                if (detail.chain === "ethereum" || detail.chain === "solana") {
                    appState.chain = detail.chain;
                }
                if (
                    detail.curve === "secp256k1" ||
                    detail.curve === "ed25519"
                ) {
                    appState.curve = detail.curve;
                }
                appState.showSettings = false;
                appState = { ...appState };
            }}
        />
    {:else if showCreateWallet}
        <!-- Ext-1b: DKG wallet creation form. Covers the popup while
             the user configures threshold/total/curve. On success we
             flip back to the main view; the session_available
             broadcast will populate appState.sessionInfo and the
             existing wallet-status banner will reflect the new state. -->
        <CreateWalletForm
            deviceId={appState.deviceId}
            wsConnected={appState.wsConnected}
            on:created={({ detail }) => {
                console.log(
                    "[UI] DKG wallet creation announced:",
                    detail.sessionId,
                );
                showCreateWallet = false;
            }}
            on:cancel={() => {
                showCreateWallet = false;
            }}
        />
    {:else if !keystoreStatus.initialized}
        <!-- Keystore not initialized -->
        <div class="text-center py-8">
            <h2 class="text-xl font-semibold mb-4">Welcome to MPC Wallet</h2>
            <p class="text-gray-600 mb-6">Create a secure keystore to get started</p>
            <button
                class="bg-blue-500 hover:bg-blue-600 text-white px-6 py-3 rounded-lg font-medium"
                on:click={showCreateKeystorePrompt}
            >
                Create Keystore
            </button>
        </div>
    {:else}
        <!-- Ext-1b: Create Wallet entry point. Shown whenever we're
             not already in a DKG session or settings overlay. Clicking
             opens the CreateWalletForm above which announces a TUI-
             compatible `announce_session` broadcast. -->
        {#if !appState.sessionInfo || appState.dkgState === DkgState.Complete || appState.dkgState === DkgState.KeystoreImported}
            <div class="mb-3 flex justify-end">
                <button
                    type="button"
                    class="rounded bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-700"
                    on:click={() => (showCreateWallet = true)}
                    disabled={!appState.wsConnected}
                    title={appState.wsConnected
                        ? "Initiate a new DKG ceremony"
                        : "Signal server not connected"}
                >
                    + Create Wallet
                </button>
            </div>
        {/if}

        <!-- Wallet Status Banner -->
        <div class="mb-4 p-3 border rounded">
            <div class="mb-2">
                <div class="font-bold">Current Network:</div>
            </div>

            <div class="p-2 bg-blue-50 border border-blue-200 rounded mb-2">
                <p class="text-blue-700">
                    {appState.chain === "ethereum"
                        ? "Ethereum (secp256k1)"
                        : "Solana (ed25519)"}
                </p>
            </div>

            {#if appState.sessionInfo && appState.dkgState === DkgState.Complete}
                <div class="p-2 bg-green-50 border border-green-200 rounded">
                    <p class="text-sm text-green-700">
                        ✓ DKG Complete - MPC addresses available for {appState.chain}
                    </p>
                </div>
            {:else if appState.sessionInfo && appState.dkgState === DkgState.KeystoreImported}
                <div class="p-2 bg-orange-50 border border-orange-200 rounded">
                    <p class="text-sm text-orange-700">
                        ⚠ Keystore imported - Waiting for peers to enable signing
                    </p>
                </div>
            {:else if appState.sessionInfo && appState.dkgState === DkgState.Initializing}
                <!-- Ext-1c: creator-side waiting-for-joiners banner.
                     Matches TUI's DKGProgress screen "waiting for
                     participants" state. Shows the session id (for
                     copy-paste / debug), participant list with the
                     creator and currently-joined peers, and the
                     outstanding count. When a peer joins via
                     session_available update, their device id appears
                     in participants and the counter decrements. -->
                {@const needed = (appState.sessionInfo.total ?? 0) - (appState.sessionInfo.participants?.length ?? 0)}
                <div class="p-2 bg-blue-50 border border-blue-200 rounded">
                    <p class="text-sm font-medium text-blue-900 mb-1">
                        📡 Waiting for joiners to discover this session
                    </p>
                    <p class="text-xs text-blue-700 font-mono break-all">
                        Session: {appState.sessionInfo.session_id}
                    </p>
                    <p class="text-xs text-blue-700 mt-1">
                        Threshold: {appState.sessionInfo.threshold}-of-{appState.sessionInfo.total}
                        • Curve: {appState.sessionInfo.curve_type ?? "secp256k1"}
                    </p>
                    <div class="mt-2">
                        <span class="text-xs font-semibold text-blue-900">Participants ({appState.sessionInfo.participants?.length ?? 0}/{appState.sessionInfo.total ?? "?"}):</span>
                        <ul class="text-xs text-blue-800 font-mono ml-3 mt-1">
                            {#each appState.sessionInfo.participants ?? [] as pid}
                                <li>{pid === appState.deviceId ? `● ${pid} (you)` : `○ ${pid}`}</li>
                            {/each}
                        </ul>
                    </div>
                    {#if needed > 0}
                        <p class="text-xs text-blue-700 mt-2">
                            Still need {needed} more participant{needed === 1 ? "" : "s"} to reach the total.
                        </p>
                    {/if}
                </div>
            {:else if appState.sessionInfo && appState.dkgState !== DkgState.Idle}
                <div class="p-2 bg-yellow-50 border border-yellow-200 rounded">
                    <p class="text-sm text-yellow-700">
                        🔄 DKG in progress - MPC addresses will be available
                        when complete
                    </p>
                </div>
            {/if}
        </div>
    {/if}

    <!-- Account Manager - Multi-account support -->
    {#if appState.dkgState === DkgState.Complete || appState.dkgState === DkgState.KeystoreImported}
        <div class="mb-4">
            <AccountManager 
                blockchain={appState.chain}
            />
        </div>
    {/if}
    
    <!-- Signature Requests -->
    {#if signatureRequests.length > 0}
        <div class="mb-4">
            <h2 class="text-xl font-semibold mb-3">🔏 Signature Requests</h2>
            {#each signatureRequests as request (request.signingId)}
                <SignatureRequest
                    signingId={request.signingId}
                    message={request.message}
                    origin={request.origin}
                    fromAddress={request.fromAddress}
                    on:complete={handleSignatureRequestComplete}
                />
            {/each}
        </div>
    {/if}

    <!-- Network Status -->
    <div class="mb-4 p-3 border rounded">
        <h2 class="text-xl font-semibold mb-3">Network Status</h2>

        <div class="grid grid-cols-1 gap-3 mb-3">
            <div>
                <span class="block font-bold mb-1">Peer ID:</span>
                <code class="block bg-gray-100 p-2 rounded text-sm">
                    {appState.deviceId || "Not connected"}
                </code>
            </div>
            <div>
                <span class="block font-bold mb-1">WebSocket:</span>
                <span
                    class="inline-block px-2 py-1 rounded text-sm {appState.wsConnected
                        ? 'bg-green-100 text-green-800'
                        : 'bg-red-100 text-red-800'}"
                >
                    {appState.wsConnected ? "Connected" : "Disconnected"}
                </span>
            </div>
        </div>
    </div>

    <!-- Connected devices - Combined with session selection when needed -->
    <div class="mb-4 p-3 border rounded">
        <h2 class="text-xl font-semibold mb-3">
            Connected Devices ({appState.connecteddevices.length})
        </h2>

        {#if appState.connecteddevices && appState.connecteddevices.length > 0}
            <ul class="space-y-2">
                {#each appState.connecteddevices as peer}
                    {@const webrtcStatus = appState.webrtcConnections[peer]}
                    {@const isOwnDevice = peer === appState.deviceId}
                    {@const showCheckbox = !isOwnDevice && !appState.invites?.length && !appState.sessionInfo}
                    {@const isInSession = appState.sessionInfo && appState.sessionInfo.participants.includes(peer)}
                    <li
                        class="flex items-center justify-between p-3 bg-gray-50 rounded {showCheckbox ? 'hover:bg-gray-100' : ''}"
                    >
                        <div class="flex items-center gap-3">
                            {#if showCheckbox}
                                <input
                                    type="checkbox"
                                    checked={selectedDevices.has(peer)}
                                    disabled={!selectedDevices.has(peer) && selectedDevices.size >= appState.totalParticipants - 1}
                                    on:change={(e) => {
                                        if (e.currentTarget.checked) {
                                            selectedDevices.add(peer);
                                        } else {
                                            selectedDevices.delete(peer);
                                        }
                                        selectedDevices = selectedDevices;
                                    }}
                                    class="w-4 h-4"
                                />
                            {/if}
                            <code class="text-sm font-mono">{peer}</code>
                            {#if isOwnDevice}
                                <span class="text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded">You</span>
                            {/if}
                            {#if isInSession}
                                <span class="text-xs bg-purple-100 text-purple-800 px-2 py-1 rounded">In Session</span>
                            {/if}
                        </div>

                        {#if !isOwnDevice}
                            <div class="flex items-center gap-2">
                                <span class="text-xs text-gray-500">WebRTC:</span>
                                {#if webrtcStatus === true}
                                    <span class="text-xs bg-green-100 text-green-800 px-2 py-1 rounded">Connected</span>
                                    {#if appState.sessionInfo && appState.meshStatus?.type === MeshStatusType.Ready && isInSession}
                                        <button
                                            class="text-xs bg-blue-500 hover:bg-blue-700 text-white px-2 py-1 rounded"
                                            on:click={() => sendDirectMessage(peer)}
                                        >
                                            Test Message
                                        </button>
                                    {/if}
                                {:else}
                                    <span class="text-xs bg-red-100 text-red-800 px-2 py-1 rounded">Disconnected</span>
                                {/if}
                            </div>
                        {/if}
                    </li>
                {/each}
            </ul>
            
            {#if selectedDevices.size > 0 && !appState.invites?.length && !appState.sessionInfo}
                <p class="text-sm text-blue-600 mt-2">
                    Selected {selectedDevices.size} of {appState.totalParticipants - 1} required participants
                </p>
            {/if}
        {:else}
            <p class="text-gray-500 text-center py-4">No devices connected</p>
        {/if}
    </div>

    <!-- MPC Session Management -->
    <div class="mb-4 p-3 border rounded">
        <h2 class="text-xl font-semibold mb-3">MPC Session</h2>

        {#if appState.sessionInfo}
            <!-- Active Session -->
            <div class="bg-gradient-to-r from-green-50 to-emerald-50 border-2 border-green-300 rounded-lg p-4 shadow-md">
                <div class="flex items-center justify-between mb-3">
                    <h3 class="text-lg font-bold text-green-800 flex items-center">
                        <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                        </svg>
                        Active Session
                    </h3>
                    <div class="flex items-center space-x-2">
                        {#if appState.dkgState === DkgState.Complete}
                            <span class="text-xs bg-green-100 text-green-800 px-2 py-1 rounded-full font-semibold">
                                ✓ Ready to Sign
                            </span>
                        {:else if appState.dkgState === DkgState.KeystoreImported}
                            <span class="text-xs bg-orange-100 text-orange-800 px-2 py-1 rounded-full font-semibold">
                                ⚠ Keystore Imported - Connect Peers
                            </span>
                        {:else if appState.dkgState === DkgState.Initializing || 
                                  appState.dkgState === DkgState.Round1InProgress || 
                                  appState.dkgState === DkgState.Round1Complete || 
                                  appState.dkgState === DkgState.Round2InProgress || 
                                  appState.dkgState === DkgState.Round2Complete || 
                                  appState.dkgState === DkgState.Finalizing}
                            <span class="text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded-full font-semibold animate-pulse">
                                DKG in Progress...
                            </span>
                        {:else if appState.dkgState === DkgState.Failed}
                            <span class="text-xs bg-red-100 text-red-800 px-2 py-1 rounded-full font-semibold">
                                DKG Failed
                            </span>
                        {:else if appState.meshStatus?.type === MeshStatusType.Ready}
                            <span class="text-xs bg-yellow-100 text-yellow-800 px-2 py-1 rounded-full font-semibold">
                                Ready for DKG
                            </span>
                        {:else if appState.meshStatus?.type === MeshStatusType.PartiallyReady || appState.sessionInfo.accepted_devices?.length !== appState.sessionInfo.participants.length}
                            <span class="text-xs bg-yellow-100 text-yellow-800 px-2 py-1 rounded-full font-semibold animate-pulse">
                                Waiting for Participants...
                            </span>
                        {:else}
                            <span class="text-xs bg-gray-100 text-gray-800 px-2 py-1 rounded-full font-semibold">
                                Setting Up... (DKG: {appState.dkgState})
                            </span>
                        {/if}
                    </div>
                </div>
                
                <div class="bg-white bg-opacity-70 rounded p-3 mb-3">
                    <div class="grid grid-cols-2 gap-3 text-sm">
                        <div>
                            <span class="text-gray-600">Session ID:</span>
                            <p class="font-mono text-xs bg-gray-100 px-2 py-1 rounded mt-1 truncate">
                                {appState.sessionInfo.session_id}
                            </p>
                        </div>
                        <div>
                            <span class="text-gray-600">Threshold:</span>
                            <p class="font-bold text-green-700 text-lg">
                                {appState.sessionInfo.threshold} of {appState.sessionInfo.total}
                            </p>
                        </div>
                        <div>
                            <span class="text-gray-600">DKG Status:</span>
                            <p class="font-semibold {appState.dkgState === DkgState.Complete ? 'text-green-700' : appState.dkgState === DkgState.Failed ? 'text-red-700' : 'text-yellow-700'}">
                                {DkgState[appState.dkgState] || "Unknown"}
                            </p>
                        </div>
                        <div>
                            <span class="text-gray-600">Proposer:</span>
                            <p class="font-mono text-xs truncate {appState.sessionInfo.proposer_id === appState.deviceId ? 'text-blue-700 font-semibold' : ''}">
                                {appState.sessionInfo.proposer_id}{appState.sessionInfo.proposer_id === appState.deviceId ? ' (you)' : ''}
                            </p>
                        </div>
                    </div>
                </div>

                <div class="mb-3">
                    <div class="flex justify-between items-center mb-2">
                        <span class="text-sm font-semibold text-gray-700">Participants:</span>
                        <span class="text-xs text-gray-500">
                            {appState.sessionInfo.accepted_devices?.length || 0}/{appState.sessionInfo.participants.length} accepted
                        </span>
                    </div>
                    <div class="space-y-1">
                        {#each appState.sessionInfo.participants as participant}
                            {@const isAccepted = appState.sessionInfo.accepted_devices?.includes(participant)}
                            {@const isConnected = appState.webrtcConnections[participant]}
                            <div class="flex items-center justify-between p-2 rounded {participant === appState.deviceId ? 'bg-blue-50' : 'bg-gray-50'}">
                                <span class="text-sm font-mono {participant === appState.deviceId ? 'text-blue-700 font-semibold' : ''}">
                                    {participant}{participant === appState.deviceId ? ' (you)' : ''}
                                </span>
                                <div class="flex items-center space-x-2">
                                    {#if isAccepted}
                                        <span class="text-xs bg-green-100 text-green-700 px-2 py-0.5 rounded">
                                            Accepted
                                        </span>
                                    {:else}
                                        <span class="text-xs bg-gray-100 text-gray-600 px-2 py-0.5 rounded">
                                            Pending
                                        </span>
                                    {/if}
                                    {#if participant !== appState.deviceId}
                                        {#if isConnected}
                                            <span class="w-2 h-2 bg-green-500 rounded-full" title="Connected"></span>
                                        {:else}
                                            <span class="w-2 h-2 bg-gray-300 rounded-full" title="Not connected"></span>
                                        {/if}
                                    {/if}
                                </div>
                            </div>
                        {/each}
                    </div>
                </div>

                {#if appState.meshStatus?.type === MeshStatusType.Ready && appState.dkgState === DkgState.Idle}
                    <div class="border-t border-green-200 pt-3">
                        <p class="text-sm text-gray-600 mb-2">
                            All participants are connected. Ready to start the Distributed Key Generation process.
                        </p>
                        <button class="w-full bg-gradient-to-r from-blue-500 to-blue-600 hover:from-blue-600 hover:to-blue-700 text-white font-bold py-2 px-4 rounded-lg shadow-md">
                            Start DKG Process
                        </button>
                    </div>
                {:else if appState.dkgState === DkgState.Complete}
                    <div class="border-t border-green-200 pt-3">
                        <div class="bg-green-50 border border-green-200 rounded-lg p-3 mb-3">
                            <p class="text-sm text-green-800 font-semibold flex items-center">
                                <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                                </svg>
                                Key Generation Complete
                            </p>
                            <p class="text-xs text-green-700 mt-1">
                                Your MPC wallet is ready. Any {appState.sessionInfo.threshold} of {appState.sessionInfo.total} participants can now sign transactions together.
                            </p>
                        </div>
                        <button
                            class="w-full bg-gradient-to-r from-purple-500 to-purple-600 hover:from-purple-600 hover:to-purple-700 text-white font-bold py-2 px-4 rounded-lg shadow-md flex items-center justify-center"
                            on:click={testMPCSigning}
                        >
                            <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"></path>
                            </svg>
                            Test MPC Signing
                        </button>
                    </div>
                {:else if appState.dkgState === DkgState.KeystoreImported}
                    <div class="border-t border-green-200 pt-3">
                        <div class="bg-orange-50 border border-orange-200 rounded-lg p-3 mb-3">
                            <p class="text-sm text-orange-800 font-semibold flex items-center">
                                <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path>
                                </svg>
                                Keystore Imported - Peer Connection Required
                            </p>
                            <p class="text-xs text-orange-700 mt-1">
                                Your keystore has been imported successfully. To enable signing, you need at least {appState.sessionInfo.threshold - 1} other participant{appState.sessionInfo.threshold - 1 > 1 ? 's' : ''} from the original {appState.sessionInfo.threshold}-of-{appState.sessionInfo.total} setup to connect and join this session.
                            </p>
                        </div>
                        <div class="bg-gray-50 border border-gray-200 rounded-lg p-3">
                            <p class="text-sm text-gray-700 font-semibold mb-2">Next Steps:</p>
                            <ol class="text-xs text-gray-600 space-y-1 list-decimal list-inside">
                                <li>Share the session ID with other keystore holders</li>
                                <li>Wait for at least {appState.sessionInfo.threshold - 1} participant{appState.sessionInfo.threshold - 1 > 1 ? 's' : ''} to connect</li>
                                <li>Once connected, the wallet will be ready for signing</li>
                            </ol>
                        </div>
                    </div>
                {:else if appState.dkgState === DkgState.Initializing || 
                          appState.dkgState === DkgState.Round1InProgress || 
                          appState.dkgState === DkgState.Round1Complete || 
                          appState.dkgState === DkgState.Round2InProgress || 
                          appState.dkgState === DkgState.Round2Complete || 
                          appState.dkgState === DkgState.Finalizing}
                    <div class="border-t border-green-200 pt-3">
                        <div class="bg-blue-50 border border-blue-200 rounded-lg p-3">
                            <p class="text-sm text-blue-800 font-semibold flex items-center">
                                <svg class="animate-spin h-4 w-4 mr-2" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                </svg>
                                Generating Keys...
                            </p>
                            <p class="text-xs text-blue-700 mt-1">
                                Please wait while the distributed key generation protocol completes.
                            </p>
                        </div>
                    </div>
                {/if}
            </div>
        {:else if appState.invites && appState.invites.length > 0}
            <!-- Pending Invitations -->
            <div class="space-y-3">
                {#each appState.invites as invite}
                    <div class="bg-gradient-to-r from-yellow-50 to-orange-50 border-2 border-yellow-300 rounded-lg p-4 shadow-lg">
                        <div class="flex items-center justify-between mb-3">
                            <h3 class="text-lg font-bold text-yellow-900 flex items-center">
                                <svg class="w-5 h-5 mr-2 animate-pulse" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"></path>
                                </svg>
                                New Session Invitation
                            </h3>
                            <span class="text-xs text-gray-500">
                                {new Date().toLocaleTimeString()}
                            </span>
                        </div>
                        
                        <div class="bg-white bg-opacity-70 rounded p-3 mb-3">
                            <div class="text-sm space-y-2">
                                <div class="flex justify-between">
                                    <span class="font-semibold text-gray-600">Session ID:</span>
                                    <span class="font-mono text-xs bg-gray-100 px-2 py-1 rounded">{invite.session_id}</span>
                                </div>
                                <div class="flex justify-between">
                                    <span class="font-semibold text-gray-600">Proposer:</span>
                                    <span class="font-mono text-xs {invite.proposer_id === appState.deviceId ? 'bg-blue-100 text-blue-800' : 'bg-gray-100'} px-2 py-1 rounded">
                                        {invite.proposer_id}{invite.proposer_id === appState.deviceId ? ' (you)' : ''}
                                    </span>
                                </div>
                                <div class="flex justify-between">
                                    <span class="font-semibold text-gray-600">Threshold:</span>
                                    <span class="font-bold text-orange-600">{invite.threshold} of {invite.total}</span>
                                </div>
                            </div>
                        </div>

                        <div class="mb-3">
                            <p class="text-sm font-semibold text-gray-700 mb-2">Participants ({invite.participants?.length || 0}):</p>
                            <div class="flex flex-wrap gap-1">
                                {#each invite.participants || [] as participant}
                                    <span class="text-xs px-2 py-1 rounded-full {participant === appState.deviceId ? 'bg-blue-100 text-blue-800 font-semibold' : 'bg-gray-100 text-gray-700'}">
                                        {participant}{participant === appState.deviceId ? ' (you)' : ''}
                                    </span>
                                {/each}
                            </div>
                        </div>

                        <div class="border-t border-yellow-200 pt-3">
                            <p class="text-sm text-gray-600 mb-3">
                                You have been invited to join a {invite.threshold}-of-{invite.total} threshold signature session. 
                                This will allow any {invite.threshold} participants to create valid signatures together.
                            </p>
                            
                            <div class="flex gap-2">
                                <button
                                    class="flex-1 bg-gradient-to-r from-green-500 to-green-600 hover:from-green-600 hover:to-green-700 text-white font-bold py-2 px-4 rounded-lg shadow-md transform transition hover:scale-105 flex items-center justify-center disabled:opacity-50 disabled:cursor-not-allowed"
                                    on:click={() => acceptInvite(invite.session_id)}
                                    disabled={acceptingSession}
                                >
                                    {#if acceptingSession}
                                        <svg class="animate-spin h-5 w-5 mr-2" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                        </svg>
                                        Processing...
                                    {:else}
                                        <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
                                        </svg>
                                        Accept & Join
                                    {/if}
                                </button>
                                <button
                                    class="flex-1 bg-gray-300 hover:bg-gray-400 text-gray-700 font-bold py-2 px-4 rounded-lg shadow-md transform transition hover:scale-105"
                                    on:click={() => rejectInvite(invite.session_id)}
                                >
                                    <svg class="w-5 h-5 mr-2 inline" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                                    </svg>
                                    Decline
                                </button>
                            </div>
                        </div>
                    </div>
                {/each}
            </div>
        {:else}
            <!-- Create New Session -->
            <div class="space-y-3">
                <p class="text-sm text-gray-600">
                    Select devices from the list above to create a new MPC session, or import an existing keystore.
                </p>
                
                <!-- Import/Export Keystore Buttons -->
                <div class="mb-4 space-y-2">
                    <!-- Import Keystore Button -->
                    <button
                        class="w-full bg-purple-500 hover:bg-purple-700 text-white font-bold py-2 px-4 rounded flex items-center justify-center"
                        on:click={() => {
                            const input = document.createElement('input');
                            input.type = 'file';
                            input.accept = '.json';
                            input.onchange = async (e) => {
                                const file = (e.target as HTMLInputElement).files?.[0];
                                if (file) {
                                    const reader = new FileReader();
                                    reader.onload = async (event) => {
                                        const keystoreData = event.target?.result as string;
                                        try {
                                            // Parse to check if encrypted
                                            const parsedKeystore = JSON.parse(keystoreData);
                                            let password = undefined;
                                            
                                            // Check if keystore is encrypted
                                            if (parsedKeystore.encrypted === true) {
                                                password = prompt("This keystore is encrypted. Please enter the password:");
                                                if (!password) {
                                                    alert("Password is required for encrypted keystores");
                                                    return;
                                                }
                                            }
                                            
                                            chrome.runtime.sendMessage({
                                                type: "importKeystore",
                                                keystoreData,
                                                password,
                                                chain: appState.chain
                                            }, (response) => {
                                                if (chrome.runtime.lastError) {
                                                    console.error("[UI] Error importing keystore:", chrome.runtime.lastError.message);
                                                    alert("Failed to import keystore: " + chrome.runtime.lastError.message);
                                                    return;
                                                }
                                                if (response.success) {
                                                    console.log("[UI] Keystore imported successfully");
                                                    alert("Keystore imported successfully!");
                                                } else {
                                                    console.error("[UI] Failed to import keystore:", response.error);
                                                    alert("Failed to import keystore: " + response.error);
                                                }
                                            });
                                        } catch (err) {
                                            console.error("[UI] Error reading keystore file:", err);
                                            alert("Invalid keystore file");
                                        }
                                    };
                                    reader.readAsText(file);
                                }
                            };
                            input.click();
                        }}
                    >
                        <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"></path>
                        </svg>
                        Import Keystore from CLI
                    </button>
                    
                    <!-- Export Keystore Button - Only show when DKG is complete -->
                    {#if appState.dkgState === DkgState.Complete}
                        <button
                            class="w-full bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded flex items-center justify-center"
                            on:click={() => {
                                chrome.runtime.sendMessage({
                                    type: "exportKeystore",
                                    chain: appState.chain
                                }, (response) => {
                                    if (chrome.runtime.lastError) {
                                        console.error("[UI] Error exporting keystore:", chrome.runtime.lastError.message);
                                        alert("Failed to export keystore: " + chrome.runtime.lastError.message);
                                        return;
                                    }
                                    if (response.success && response.keystoreData) {
                                        // Create a blob and download link
                                        const blob = new Blob([response.keystoreData], { type: 'application/json' });
                                        const url = URL.createObjectURL(blob);
                                        const a = document.createElement('a');
                                        a.href = url;
                                        a.download = `mpc-wallet-keystore-${appState.chain}-${Date.now()}.json`;
                                        document.body.appendChild(a);
                                        a.click();
                                        document.body.removeChild(a);
                                        URL.revokeObjectURL(url);
                                        console.log("[UI] Keystore exported successfully");
                                    } else {
                                        console.error("[UI] Failed to export keystore:", response.error);
                                        alert("Failed to export keystore: " + response.error);
                                    }
                                });
                            }}
                        >
                            <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 10l3-3m0 0l3 3m-3-3v12"></path>
                            </svg>
                            Export Keystore for Backup
                        </button>
                    {/if}
                </div>
                
                <div class="relative">
                    <div class="absolute inset-0 flex items-center">
                        <div class="w-full border-t border-gray-300"></div>
                    </div>
                    <div class="relative flex justify-center text-sm">
                        <span class="px-2 bg-white text-gray-500">OR</span>
                    </div>
                </div>
                
                <div>
                    <label for="session-id-input" class="block font-bold mb-1"
                        >Session ID (optional):</label
                    >
                    <input
                        id="session-id-input"
                        type="text"
                        bind:value={appState.proposedSessionIdInput}
                        class="w-full border p-2 rounded"
                        placeholder="Auto-generated if empty"
                    />
                </div>

                <div class="grid grid-cols-2 gap-3">
                    <div>
                        <label
                            for="total-participants"
                            class="block font-bold mb-1"
                            >Total Participants:</label
                        >
                        <input
                            id="total-participants"
                            type="number"
                            bind:value={appState.totalParticipants}
                            min="2"
                            max={appState.connecteddevices.length}
                            class="w-full border p-2 rounded"
                            on:change={() => {
                                // Clear selection when total participants changes
                                selectedDevices.clear();
                                selectedDevices = selectedDevices;
                            }}
                        />
                    </div>
                    <div>
                        <label
                            for="threshold-input"
                            class="block font-bold mb-1">Threshold:</label
                        >
                        <input
                            id="threshold-input"
                            type="number"
                            bind:value={appState.threshold}
                            min="1"
                            max={appState.totalParticipants}
                            class="w-full border p-2 rounded"
                        />
                    </div>
                </div>

                <button
                    class="w-full bg-indigo-500 hover:bg-indigo-700 text-white font-bold py-2 px-4 rounded disabled:bg-gray-400 disabled:cursor-not-allowed"
                    on:click={proposeSession}
                    disabled={!appState.wsConnected ||
                        selectedDevices.size !== appState.totalParticipants - 1 ||
                        appState.threshold > appState.totalParticipants ||
                        appState.threshold < 1}
                >
                    Propose New Session ({appState.threshold}-of-{appState.totalParticipants})
                </button>

                {#if !appState.wsConnected}
                    <p class="text-sm text-red-500 text-center">
                        WebSocket not connected
                    </p>
                {:else if appState.connecteddevices.filter((p) => p !== appState.deviceId).length < appState.totalParticipants - 1}
                    <p class="text-sm text-gray-500 text-center">
                        Need at least {appState.totalParticipants - 1} other devices
                        for a {appState.totalParticipants}-participant session
                    </p>
                {:else if appState.threshold > appState.totalParticipants || appState.threshold < 1}
                    <p class="text-sm text-red-500 text-center">
                        Invalid threshold: must be between 1 and {appState.totalParticipants}
                    </p>
                {:else if selectedDevices.size !== appState.totalParticipants - 1}
                    <p class="text-sm text-yellow-500 text-center">
                        Please select {appState.totalParticipants - 1} device{appState.totalParticipants - 1 > 1 ? 's' : ''} to include in the session
                    </p>
                {/if}
            </div>
        {/if}
    </div>

    <!-- WebSocket Error Display -->
    {#if appState.wsError}
        <div class="mb-4 p-3 bg-red-50 border border-red-200 rounded">
            <div class="flex justify-between items-center">
                <span class="text-red-600">{appState.wsError}</span>
                <button
                    class="text-sm bg-red-100 hover:bg-red-200 px-2 py-1 rounded"
                    on:click={() => {
                        appState.wsError = "";
                        appState = { ...appState };
                    }}
                >
                    ×
                </button>
            </div>
        </div>
    {/if}
</main>

<!-- Password Prompt Modal -->
{#if showPasswordPrompt}
    <PasswordPrompt
        title={passwordPromptConfig.title}
        message={passwordPromptConfig.message}
        confirmMode={passwordPromptConfig.confirmMode}
        on:submit={passwordPromptConfig.onSubmit}
        on:cancel={passwordPromptConfig.onCancel}
    />
{/if}

<style>
    :global(body) {
        width: 400px;
        height: 600px;
        overflow: auto;
    }

    /* Dark mode styles */
    :global(.dark) {
        color-scheme: dark;
    }

    :global(.dark body) {
        background-color: #1a1a1a;
        color: #e5e5e5;
    }

    :global(.dark .border) {
        border-color: #333333;
    }

    :global(.dark .bg-gray-50) {
        background-color: #262626;
    }

    :global(.dark .bg-gray-100) {
        background-color: #333333;
    }

    :global(.dark .bg-green-50) {
        background-color: #064e3b;
        color: #a7f3d0;
    }

    :global(.dark .bg-yellow-50) {
        background-color: #78350f;
        color: #fde68a;
    }

    :global(.dark .bg-blue-50) {
        background-color: #082f49;
        color: #bae6fd;
    }

    :global(.dark .text-green-700) {
        color: #a7f3d0;
    }

    :global(.dark .text-yellow-700) {
        color: #fde68a;
    }

    :global(.dark .text-blue-700) {
        color: #bae6fd;
    }

    :global(.dark .text-blue-600) {
        color: #60a5fa;
    }

    :global(.dark .bg-blue-500) {
        background-color: #2563eb;
    }

    :global(.dark .bg-blue-600) {
        background-color: #1d4ed8;
    }

    :global(.dark .border-green-200) {
        border-color: #065f46;
    }

    :global(.dark .border-yellow-200) {
        border-color: #92400e;
    }

    :global(.dark .border-blue-200) {
        border-color: #0c4a6e;
    }

    .logo {
        height: 40px;
        width: 40px;
    }
</style>
