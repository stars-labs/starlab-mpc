<script lang="ts">
  import svelteLogo from "../../assets/svelte.svg";
  import { onMount, onDestroy } from "svelte";
  import type { AppState } from "../../types/appstate";
  import { MeshStatusType } from "../../types/mesh";
  import { DkgState } from "../../types/dkg";
  import { INITIAL_APP_STATE } from "../../types/appstate";
  
  // Components
  import Settings from "../../components/Settings.svelte";
  import WalletStatus from "../../components/WalletStatus.svelte";
  import AddressDisplay from "../../components/AddressDisplay.svelte";
  import NetworkStatus from "../../components/NetworkStatus.svelte";
  import DeviceList from "../../components/DeviceList.svelte";
  import SessionManager from "../../components/SessionManager.svelte";
  import ErrorDisplay from "../../components/ErrorDisplay.svelte";
  
  // Utilities
  import { MessageHandler, type MessageHandlerCallbacks } from "../../utils/messageHandler";
  import { createSessionActions } from "../../utils/sessionActions";
  import { UIStateManager } from "../../utils/uiState";

  // Application state (consolidated from background) - the single source of truth
  let appState: AppState = { ...INITIAL_APP_STATE };

  // Local UI state (not part of AppState)
  let showSettings = false;
  let proposedSessionIdInput = "";
  let totalParticipants = 3;
  let threshold = 2;
  let dkgAddress = "";
  let dkgError = "";
  let wsError = "";
  let initialStateLoaded = false;

  // Track sessions being processed to prevent multiple clicks
  let processingInvites = new Set<string>();

  // Keep connection to background script
  let port: chrome.runtime.Port;

  // Session actions utility
  const sessionActions = createSessionActions();

  // Message handler setup
  const messageCallbacks: MessageHandlerCallbacks = {
    onInitialState: (newState: AppState) => {
      // Load persisted UI preferences from localStorage (ONLY UI state, not real-time)
      const savedUIState = UIStateManager.loadUIState();
      if (savedUIState) {
        showSettings = savedUIState.showSettings || false;
        proposedSessionIdInput = savedUIState.proposedSessionIdInput || "";
        totalParticipants = savedUIState.totalParticipants || 3;
        threshold = savedUIState.threshold || 2;
        if (savedUIState.chain && (savedUIState.chain === "ethereum" || savedUIState.chain === "solana")) {
          newState.chain = savedUIState.chain;
        }
      }

      appState = newState;
      initialStateLoaded = true;

      // Save the current UI preferences immediately after loading
      saveUIPreferences();
    },
    onWSStatusUpdate: (connected: boolean, error?: string) => {
      appState.wsConnected = connected;
      wsError = error || "";
      appState = { ...appState };
    },
    onDkgAddressUpdate: (address: string, blockchain: string) => {
      dkgAddress = address;
      dkgError = "";
    },
    onProcessingInvitesUpdate: (newProcessingInvites: Set<string>) => {
      processingInvites = newProcessingInvites;
    },
  };

  const messageHandler = new MessageHandler(messageCallbacks);

  // Helper function to save UI preferences
  function saveUIPreferences() {
    UIStateManager.throttledSaveUIState({
      showSettings,
      proposedSessionIdInput,
      totalParticipants,
      threshold,
      chain: appState.chain,
    });
  }

  // Reactive computation for WebRTC connection status
  $: webrtcConnected =
    appState.sessionInfo && appState.meshStatus?.type === MeshStatusType.Ready;
  $: webrtcConnecting =
    appState.sessionInfo &&
    appState.meshStatus?.type === MeshStatusType.PartiallyReady;

  // Debug logging for WebRTC connection changes (throttled)
  let lastWebRTCState = "";
  $: {
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

  // Reactive statements to save UI preferences (throttled)
  $: if (initialStateLoaded && typeof showSettings !== "undefined") {
    saveUIPreferences();
  }
  $: if (initialStateLoaded && typeof proposedSessionIdInput !== "undefined") {
    saveUIPreferences();
  }
  $: if (initialStateLoaded && typeof totalParticipants !== "undefined") {
    saveUIPreferences();
  }
  $: if (initialStateLoaded && typeof threshold !== "undefined") {
    saveUIPreferences();
  }
  $: if (initialStateLoaded && typeof appState.chain !== "undefined") {
    saveUIPreferences();
  }

  // Component event handlers
  function handleProposeSession() {
    sessionActions.proposeSession({
      proposedSessionIdInput,
      totalParticipants,
      threshold,
      deviceId: appState.deviceId,
      connectedDevices: appState.connecteddevices,
    });
  }

  function handleAcceptInvite(event: CustomEvent<{ sessionId: string }>) {
    const { sessionId } = event.detail;
    
    // Prevent multiple rapid clicks
    if (processingInvites.has(sessionId)) {
      return;
    }

    messageHandler.addProcessingInvite(sessionId);
    
    // Safety timeout to clear processing state
    const timeoutId = setTimeout(() => {
      messageHandler.removeProcessingInvite(sessionId);
    }, 10000);

    sessionActions.acceptSession({
      sessionId,
      blockchain: appState.chain || "ethereum",
    });

    // Clear timeout when accepted (handled in message handler)
    // The response will clear the processing state via messageHandler
  }

  function handleSendDirectMessage(event: CustomEvent<{ deviceId: string }>) {
    const { deviceId } = event.detail;
    sessionActions.sendDirectMessage({
      toDeviceId: deviceId,
      fromDeviceId: appState.deviceId,
    });
  }

  function handleDismissError() {
    wsError = "";
  }

  onMount(async () => {
    console.log("[UI] Component mounting");

    let stateReceived = false;
    initialStateLoaded = false;

    port = chrome.runtime.connect({ name: "popup" });
    console.log(
      "[UI] Port connected to background, waiting for initial state...",
    );

    port.onMessage.addListener((message) => {
      console.log("[UI] Port message received:", message.type, message);
      
      if (message.type === "initialState" && !stateReceived) {
        stateReceived = true;
        console.log(
          "[UI] Initial state received successfully from StateManager",
        );
      }
      
      // Use message handler to process all messages
      appState = messageHandler.handleBackgroundMessage(message, appState);
    });

    port.onDisconnect.addListener(() => {
      console.error("[UI] Port disconnected from background");
      appState.wsConnected = false;
    });

    console.log(
      "[UI] Port connected, StateManager should automatically send state...",
    );

    // Fallback in case automatic state is delayed
    setTimeout(() => {
      if (!stateReceived) {
        console.warn(
          "[UI] Automatic state not received within 1 second, requesting manually as fallback",
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
            console.log("[UI] Fallback state response received:", response);
            appState = messageHandler.handleBackgroundMessage({
              type: "initialState",
              ...response,
            }, appState);
            stateReceived = true;
          }
        });
      } else {
        console.log(
          "[UI] State was received automatically, no fallback needed",
        );
      }
    }, 1000);
  });

  onDestroy(() => {
    if (port) {
      port.disconnect();
    }
  });
</script>

<main class="p-4 max-w-2xl mx-auto">
  <div class="text-center mb-6 flex justify-between items-center">
    <img src={svelteLogo} class="logo svelte mb-2" alt="Svelte Logo" />
    <h1 class="text-3xl font-bold flex-grow text-center">MPC Wallet</h1>
    <button
      class="bg-blue-500 hover:bg-blue-600 text-white p-2 rounded-full"
      on:click={() => {
        showSettings = !showSettings;
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

  {#if showSettings}
    <Settings
      on:backToWallet={({ detail }) => {
        if (detail.chain === "ethereum" || detail.chain === "solana") {
          appState.chain = detail.chain;
        }
        showSettings = false;
        appState = { ...appState };
      }}
    />
  {:else}
    <!-- Use extracted components -->
    <WalletStatus {appState} />
    
    <AddressDisplay {appState} {dkgAddress} {dkgError} />
    
    <NetworkStatus {appState} />
    
    <DeviceList {appState} on:sendDirectMessage={handleSendDirectMessage} />
    
    <SessionManager 
      {appState} 
      bind:proposedSessionIdInput 
      bind:totalParticipants 
      bind:threshold 
      {processingInvites}
      on:proposeSession={handleProposeSession}
      on:acceptInvite={handleAcceptInvite}
    />
    
    <ErrorDisplay error={wsError} on:dismiss={handleDismissError} />
  {/if}
</main>

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