# MPC Wallet Extension

A Multi-Party Computation (MPC) wallet browser extension built with WXT, Svelte, and Rust/WebAssembly. This extension enables secure distributed key generation and signing operations across multiple parties using WebRTC for peer-to-peer communication.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Components](#components)
- [Message System](#message-system)
- [Message Flow Patterns](#message-flow-patterns)
- [WebSocket Communication](#websocket-communication)
- [WebRTC Management](#webrtc-management)
- [Installation](#installation)
- [Development](#development)
- [Usage](#usage)
- [API Reference](#api-reference)

## Architecture Overview

The MPC Wallet Extension follows a Chrome Extension Manifest V3 architecture with four main contexts that communicate via strongly-typed messages:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Popup Page    │    │ Background Page │    │ Offscreen Page  │
│                 │    │                 │    │                 │
│ - UI Components │    │ - Service Worker│    │ - WebRTC Manager│
│ - State Display │◄──►│ - Message Router│◄──►│ - DOM Access    │
│ - User Actions  │    │ - WebSocket     │    │ - Crypto Ops    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │ Content Script  │
                    │                 │
                    │ - Web Page Hook │◄── Web Page
                    │ - JSON-RPC Proxy│    (window.ethereum)
                    └─────────────────┘
```

## Components

### 1. Background Page (Service Worker)
**Location:** `src/entrypoints/background/index.ts`

**Responsibilities:**
- Central message router for all cross-context communication
- WebSocket client management for the signaling server
- Account / keystore / network services
- Offscreen document lifecycle management
- RPC request handling for dApp-facing EIP-1193 traffic

**Key classes in `src/entrypoints/background/`:**
- `StateManager` — persistent state + cross-context broadcast
- `SessionManager` — `proposeSession` / `acceptSession` / DKG
  & signing session lifecycle
- `WebSocketManager` — signal-server connection, session relay,
  ceremony trigger hooks
- `WebSocketClient` — low-level WS transport under WebSocketManager
- `OffscreenManager` — create/tear down the offscreen document
- `RpcHandler` — dApp EIP-1193 entry point
- `KeepaliveController` — pings offscreen during active ceremonies
  to keep MV3 from idle-killing it

**Key services in `src/services/`** (default-exported classes,
consumed across contexts):
- `AccountService` (`accountService.ts`)
- `NetworkService` (`networkService.ts`)
- `WalletClientService` (`walletClient.ts`)
- `WalletController` (`walletController.ts`)
- `KeystoreService` / `KeystoreManager`
  (`keystoreService.ts` / `keystoreManager.ts`)
- `PermissionService` (`permissionService.ts`)

### 2. Popup Page (UI)
**Location:** `src/entrypoints/popup/App.svelte` (Svelte 5 legacy
reactivity, NOT runes — see the top-of-repo `CLAUDE.md`)

**Responsibilities:**
- User interface for wallet operations
- Display connection status + peer list
- Session management UI for DKG / signing
- Crypto operations (signing, address display)

**Features:**
- MPC-based distributed key generation (DKG)
- Multi-chain support (Ethereum / Solana; additional L2s share
  Ethereum's secp256k1 address)
- Threshold message signing via FROST
- Real-time peer discovery + session invite management
- WebRTC connection-state panel

### 3. Offscreen Page (WebRTC Handler)
**Location:** `src/entrypoints/offscreen/index.ts` (routing entry)
+ `src/entrypoints/offscreen/webrtc.ts` (main `WebRTCManager` host).

**Responsibilities:**
- WebRTC peer-connection lifecycle + data channels
- P2P DKG / signing ceremony execution
- FROST-WASM host (loads `@mpc-wallet/core-wasm` → `frostDkg`)
- DOM-dependent operations that can't run in the MV3 service
  worker context

**Key components:**
- `WebRTCManager` — peer connections + FROST state
  (`frostDkg`, `signingInfo`, `signingCommitments`,
  `signingShares`). See CLAUDE.md for the signing pipeline.

### 4. Content Script (Web Integration)
**Location:** `src/entrypoints/content/index.ts` (injects the
provider) + `src/entrypoints/injected/index.ts` (the provider
itself — runs in page context).

**Responsibilities:**
- Injects an EIP-1193 provider into the page as
  `window.starlabEthereum` (not `window.ethereum` — the
  extension coexists with other wallets via EIP-6963 discovery;
  see `docs/implementation/EIP-6963-IMPLEMENTATION.md`)
- Proxies JSON-RPC requests (eth_requestAccounts,
  eth_sendTransaction, personal_sign, etc.) to the background
  via `chrome.runtime.sendMessage`

## Message System

The extension uses a type-safe message system defined in
`packages/@mpc-wallet/types/src/messages.ts` (shared workspace
package — NOT under the extension's own `src/types/`).
Understanding the message-flow directions is crucial for proper
implementation.

### Message Flow Architecture

```
Popup ◄──────────► Background ◄──────────► Offscreen
  │                     │                      │
  │                     │                      │
  │                     ▼                      │
  │               WebSocket Server             │
  │                     │                      │
  │                     │                      ▼
  └─────────────────────┼──────────── WebRTC Peer Network
                        │
                        ▼
                 Content Script
                        │
                        ▼
                   Web Page API
```

### Core Message Types and Flow Directions

#### 1. Popup → Background Messages
**File:** `packages/@mpc-wallet/types/src/messages.ts` (shared workspace package, not under the extension's own `src/`) — real type name is `PopupToBackgroundMessage` (the alias `BackgroundMessage` shown below is `@deprecated`; both resolve to the same union)

```typescript
// User initiates actions from popup UI
export type BackgroundMessage = BaseMessage & (
    // Core wallet operations
    | { type: 'getState' }                    // Request current app state
    | { type: 'listdevices' }                   // Request peer discovery
    
    // Session management
    | { type: 'proposeSession'; session_id: string; total: number; threshold: number; participants: string[] }
    | { type: 'acceptSession'; session_id: string; accepted: boolean }

    // Management operations
    | { type: 'createOffscreen' }             // Request offscreen creation
    | { type: 'getOffscreenStatus' }          // Check offscreen status
    | { type: 'offscreenReady' }              // Signal offscreen is ready

    // Communication
    | { type: 'relay'; to: string; data: WebSocketMessagePayload }
    | { type: 'fromOffscreen'; payload: OffscreenMessage }

    // RPC operations (for blockchain interactions)
    | { type: string; payload: JsonRpcRequest; action?: string; method?: string; params?: unknown[] }
);

// Example usage in popup:
chrome.runtime.sendMessage({ 
    type: 'proposeSession', 
    session_id: 'session-123',
    total: 3,
    threshold: 2,
    participants: ['peer1', 'peer2', 'peer3']
});
```

#### 2. Background → Popup Messages
**File:** `packages/@mpc-wallet/types/src/messages.ts` — real type name is `BackgroundToPopupMessage` (`PopupMessage` below is the deprecated alias)

```typescript
// Background responds and broadcasts state updates
export type PopupMessage = BaseMessage & (
    | { type: 'wsStatus'; connected: boolean; reason?: string }
    | { type: 'wsMessage'; message: ServerMsg }
    | { type: 'deviceList'; devices: string[] }
    | { type: 'wsError'; error: string }
    | { type: 'fromOffscreen'; payload: OffscreenMessage }
    | { type: 'sessionUpdate'; sessionInfo: SessionInfo | null; invites: SessionInfo[] }
    | { type: 'meshStatusUpdate'; status: MeshStatus }
    | { type: 'dkgStateUpdate'; state: DkgState }
    | { type: 'webrtcConnectionUpdate'; deviceId: string; connected: boolean }
    | { type: 'proposeSession'; session_id: string; total: number; threshold: number; participants: string[] }
    | { type: 'acceptSession'; session_id: string; accepted: boolean }
    | InitialStateMessage  // Full state on popup connection
);

// InitialStateMessage contains complete app state
export interface InitialStateMessage extends AppState {
    type: 'initialState';
    deviceId: string;
    connecteddevices: string[];
    wsConnected: boolean;
    sessionInfo: SessionInfo | null;
    invites: SessionInfo[];
    meshStatus: { type: number };
    dkgState: number;
    webrtcConnections: Record<string, boolean>;
}

// Example usage in background:
broadcastToPopupPorts({
    type: 'sessionUpdate',
    sessionInfo: currentSession,
    invites: pendingInvites
});
```

#### 3. Background → Offscreen Messages
**File:** `packages/@mpc-wallet/types/src/messages.ts` — real type name is `BackgroundToOffscreenMessage` (matches the doc below) wrapped by `BackgroundToOffscreenWrapper`

```typescript
// Background forwards operations to offscreen
export type BackgroundToOffscreenMessage = {
    type: 'fromBackground';
    payload: OffscreenMessage;
}

// Where OffscreenMessage includes:
export type OffscreenMessage = BaseMessage & (
    | { type: 'relayViaWs'; to: string; data: WebRTCSignal }
    | { type: 'init'; deviceId: string; wsUrl: string }
    | { type: 'relayMessage'; fromdeviceId: string; data: WebSocketMessagePayload }
    | { type: 'meshStatusUpdate'; status: MeshStatus }
    | { type: 'dkgStateUpdate'; state: DkgState }
    | { type: 'webrtcConnectionUpdate'; deviceId: string; connected: boolean }
    | { type: 'sessionUpdate'; sessionInfo: SessionInfo | null; invites: SessionInfo[] }
    | { type: 'webrtcMessage'; fromdeviceId: string; message: DataChannelMessage }
);

// Example usage in background:
safelySendOffscreenMessage({
    type: 'fromBackground',
    payload: {
        type: 'init',
        deviceId: 'mpc-2',
        wsUrl: 'wss://xiongchenyu.dpdns.org'
    }
});
```

#### 4. Offscreen → Background Messages
**File:** `packages/@mpc-wallet/types/src/messages.ts` — payload uses the same `BackgroundToOffscreenMessage` union, wrapped as `OffscreenToBackgroundWrapper`

```typescript
// Offscreen reports status and forwards peer messages
// Sent as: { type: 'fromOffscreen', payload: OffscreenMessage }

// Example usage in offscreen:
chrome.runtime.sendMessage({
    type: 'fromOffscreen',
    payload: {
        type: 'webrtcConnectionUpdate',
        deviceId: 'peer-123',
        connected: true
    }
});
```

### Message Constants

```typescript
export const MESSAGE_TYPES = {
    GET_STATE: "getState",
    LIST_devices: "listdevices",
    PROPOSE_SESSION: "proposeSession",
    ACCEPT_SESSION: "acceptSession",
    RELAY: "relay",
    FROM_OFFSCREEN: "fromOffscreen",
    OFFSCREEN_READY: "offscreenReady",
    CREATE_OFFSCREEN: "createOffscreen",
    GET_OFFSCREEN_STATUS: "getOffscreenStatus",
    
    // Legacy support
    ACCOUNT_MANAGEMENT: "ACCOUNT_MANAGEMENT",
    NETWORK_MANAGEMENT: "NETWORK_MANAGEMENT",
    UI_REQUEST: "UI_REQUEST",
} as const;
```

### Message Validation Helpers

```typescript
// Runtime message validation functions
export function validateMessage(msg: unknown): msg is BackgroundMessage;
export function validateSessionProposal(msg: BackgroundMessage): boolean;
export function validateSessionAcceptance(msg: BackgroundMessage): boolean;

// Message type checking
export function isRpcMessage(msg: BackgroundMessage): boolean;
export function isAccountManagement(msg: BackgroundMessage): boolean;
export function isNetworkManagement(msg: BackgroundMessage): boolean;
export function isUIRequest(msg: BackgroundMessage): boolean;
```

## Message Flow Patterns

### 1. Extension Initialization Flow
```
Background Script Startup
    │
    ├─► Initialize Services (Account, Network, Wallet)
    │
    ├─► Connect to WebSocket Server
    │   │
    │   └─► Register Peer ID: "mpc-2"
    │
    ├─► Create Offscreen Document
    │   │
    │   ├─► Offscreen sends: { type: 'offscreenReady' }
    │   │
    │   └─► Background sends: { type: 'fromBackground', payload: { type: 'init', deviceId, wsUrl } }
    │
    └─► Setup Popup Port Listeners
        │
        └─► On popup connect: Send InitialStateMessage with full app state
```

### 2. User Initiates Session Proposal Flow
```
Popup UI (User clicks "Propose Session")
    │
    ├─► Send: { type: 'proposeSession', session_id, total, threshold, participants }
    │
    ▼
Background Script
    │
    ├─► Validate using validateSessionProposal()
    │
    ├─► Create WebSocket message: { websocket_msg_type: 'SessionProposal', ... }
    │
    ├─► Send to each participant via WebSocket
    │   │
    │   └─► wsClient.relayMessage(deviceId, proposalData)
    │
    └─► Broadcast to popup: { type: 'sessionUpdate', sessionInfo, invites }

Meanwhile, for receiving devices:
WebSocket Server → Background Script (Receiving Peer)
    │
    ├─► Process session proposal in handleSessionProposal()
    │
    ├─► Add to appState.invites
    │
    └─► Broadcast: { type: 'sessionUpdate', invites: [...] }
        │
        └─► Popup shows invitation UI
```

### 3. Session Acceptance and WebRTC Setup Flow
```
Popup UI (User clicks "Accept Invitation")
    │
    ├─► Send: { type: 'acceptSession', session_id, accepted: true }
    │
    ▼
Background Script
    │
    ├─► Validate using validateSessionAcceptance()
    │
    ├─► Move session from invites to sessionInfo
    │
    ├─► Send acceptance via WebSocket to other participants
    │
    ├─► Forward to offscreen: { type: 'fromBackground', payload: { type: 'sessionUpdate', ... } }
    │
    ▼
Offscreen Document
    │
    ├─► Initialize WebRTC connections for all participants
    │
    ├─► Create RTCPeerConnection for each peer
    │
    ├─► Exchange ICE candidates via Background ↔ WebSocket ↔ devices
    │
    ├─► Establish data channels
    │
    └─► Report status: { type: 'fromOffscreen', payload: { type: 'webrtcConnectionUpdate', deviceId, connected } }
        │
        ▼
    Background Script
        │
        ├─► Update appState.webrtcConnections[deviceId] = connected
        │
        └─► Broadcast: { type: 'webrtcConnectionUpdate', deviceId, connected }
            │
            ▼
        Popup UI updates connection status indicators
```

### 4. WebRTC Signaling Message Flow
```
Peer A (Offscreen) - Generate ICE candidate
    │
    ├─► Create: { Candidate: { candidate, sdpMid, sdpMLineIndex } }
    │
    ├─► Send: { type: 'fromOffscreen', payload: { type: 'relayViaWs', to: 'peer-B', data: signal } }
    │
    ▼
Background Script (Peer A)
    │
    ├─► Extract signal from OffscreenMessage
    │
    ├─► Forward via WebSocket: wsClient.relayMessage('peer-B', signal)
    │
    ▼
WebSocket Server
    │
    ├─► Relay to Peer B as ServerMsg
    │
    ▼
Background Script (Peer B)
    │
    ├─► Receive in handleRelayMessage()
    │
    ├─► Extract WebRTC signal: { websocket_msg_type, ...webrtcSignalData } = data
    │
    ├─► Forward: { type: 'fromBackground', payload: { type: 'relayViaWs', to: fromdeviceId, data: webrtcSignalData } }
    │
    ▼
Offscreen Document (Peer B)
    │
    ├─► Process ICE candidate
    │
    └─► Add to appropriate RTCPeerConnection
```

### 5. State Synchronization Patterns
```
Any State Change in Background
    │
    ├─► Update appState object
    │
    ├─► Broadcast to all popup ports via broadcastToPopupPorts()
    │   │
    │   ├─► Port-based communication (persistent connection)
    │   │
    │   └─► Popup receives via port.onMessage.addListener()
    │
    └─► Forward relevant updates to offscreen via safelySendOffscreenMessage()
        │
        └─► Wrapped in BackgroundToOffscreenMessage format

Popup State Updates:
    │
    ├─► Receive via port messages (not runtime.sendMessage)
    │
    ├─► Update Svelte reactive variables
    │
    └─► UI re-renders automatically
```

### 6. Error Handling Flow
```
Error in Any Component
    │
    ▼
Background Script (Error Handler)
    │
    ├─► Log error details
    │
    ├─► Update error state in appState
    │
    ├─► Broadcast error: { type: 'wsError', error: errorMessage }
    │
    ├─► Attempt recovery:
    │   │
    │   ├─► WebSocket reconnection
    │   ├─► Offscreen document recreation
    │   └─► Connection state reset
    │
    └─► Update UI with recovery status

Popup Error Display:
    │
    ├─► Show error message in UI
    │
    ├─► Provide user actions for recovery
    │
    └─► Clear error state when resolved
```

## WebSocket Communication

### Server Connection
**Location:** Background Page (`/src/entrypoints/background/websocket.ts`)

The WebSocket client connects to a signaling server for peer discovery and WebRTC signaling:

```typescript
const WEBSOCKET_URL = "wss://xiongchenyu.dpdns.org";
wsClient = new WebSocketClient(WEBSOCKET_URL);
```

### Message Types
- **Registration**: devices register with their unique ID
- **Peer Discovery**: List available devices for MPC sessions
- **Relay**: Forward WebRTC signaling data between devices
- **Session Management**: Coordinate MPC session proposals

### Connection Management
- Automatic reconnection with exponential backoff
- Connection state monitoring and UI updates
- Error handling and recovery mechanisms

## WebRTC Management

### Peer Connection Setup
**Location:** Offscreen Page (`/src/entrypoints/offscreen/webrtc.ts`)

The WebRTC manager handles:
- **Peer Connection Creation**: RTCPeerConnection instances for each participant
- **Data Channel Setup**: Reliable data channels for MPC communication
- **ICE Handling**: STUN/TURN server configuration and candidate exchange
- **Connection State Monitoring**: Track connection health and handle failures

### Session Management
```typescript
// Session Proposal
webRTCManager.proposeSession(sessionId, total, threshold, participants);

// Session Acceptance
webRTCManager.acceptSession(sessionId);

// Mesh Status Tracking
enum MeshStatusType {
    Incomplete,
    PartiallyReady,
    Ready
}
```

### Security Features
- **Origin Validation**: Verify message sources
- **Encrypted Channels**: Secure WebRTC data transmission
- **Isolated Contexts**: Separate WebRTC operations in offscreen context

## Installation

### Prerequisites
- Bun runtime (`curl -fsSL https://bun.sh/install | bash`)
- Rust toolchain with `wasm32-unknown-unknown` target (for
  `core-wasm` build)
- Chromium-based browser (Chrome / Brave / Edge) or Firefox for
  testing

### Development Setup
```bash
git clone https://github.com/hecoinfo/mpc-wallet.git
cd mpc-wallet

# Install JS deps (root-level Bun workspace)
bun install

# Build WASM bindings (run from repo root — the build:wasm
# script lives only in root package.json)
bun run build:wasm

# Start extension dev server with hot reload
cd apps/browser-extension
bun run dev
```

### Extension Installation (Chrome / Chromium)

```bash
# From apps/browser-extension/
bun run build          # default target: Chrome MV3 -> .output/chrome-mv3/
bun run build:firefox  # .output/firefox-mv2/
bun run build:edge     # .output/edge-mv3/
```

1. Open `chrome://extensions/`
2. Enable "Developer mode"
3. Click "Load unpacked" and select
   `apps/browser-extension/.output/chrome-mv3/`

## Development

### Understanding Message Flow in Development

When developing new features, follow these patterns:

1. **Define Message Types**: Add to
   `packages/@mpc-wallet/types/src/messages.ts` (shared workspace
   package — see the Message System section above).
2. **Popup Actions**: Send messages via
   `chrome.runtime.sendMessage()`.
3. **Background Processing**: Handle in
   `chrome.runtime.onMessage.addListener()` (see
   `src/entrypoints/background/messageHandlers.ts` — one big
   `case MESSAGE_TYPES.<Name>:` switch).
4. **Offscreen Operations**: Forward via the offscreen manager
   in `src/entrypoints/background/offscreenManager.ts`.
5. **State Updates**: Broadcast to all popup ports via
   `StateManager.broadcastToPopupPorts()`.

### Debugging Message Flow

1. **Background Console**: Check Service Worker console for message routing
2. **Popup Console**: Monitor UI-triggered messages
3. **Offscreen Console**: Debug WebRTC and crypto operations
4. **Message Validation**: Use TypeScript for compile-time message validation

### Project Structure
```
apps/browser-extension/
├── src/
│   ├── entrypoints/
│   │   ├── background/     # Service worker + managers
│   │   ├── content/        # Content script (EIP-1193 injection)
│   │   ├── offscreen/      # Offscreen doc (WebRTC + WASM FROST)
│   │   └── popup/          # Svelte 5 popup UI
│   ├── services/           # Per-domain services
│   ├── components/         # Svelte components
│   ├── utils/
│   └── config/             # signal-server.ts + other config
├── tests/                  # Bun test suite
├── public/                 # Static assets (WASM, icons, etc.)
└── wxt.config.ts           # WXT framework config

packages/@mpc-wallet/types/src/
├── messages.ts             # All cross-context message types
├── appstate.ts             # AppState + chain / curve types
├── session.ts              # SessionInfo types
└── ...
```

### Key Files
- `packages/@mpc-wallet/types/src/messages.ts` — all message
  type definitions (shared workspace package — types aren't
  under the extension's own `src/`)
- `packages/@mpc-wallet/types/src/appstate.ts` — application
  state types + SupportedChain
- `src/entrypoints/background/index.ts` — SW entry + router
- `src/entrypoints/background/messageHandlers.ts` — MESSAGE_TYPES
  dispatch table (the authoritative handler switch)
- `src/entrypoints/offscreen/webrtc.ts` — WebRTC + FROST/WASM host
- `src/entrypoints/popup/App.svelte` — main UI

### Testing

From inside `apps/browser-extension/` (all via Bun's built-in
test runner — see `docs/testing/TESTING.md`):

```bash
bun run check            # svelte-check type-check pass
bun test                 # full test suite
bun run test:watch       # watch mode
bun run test:coverage    # coverage report
bun run test:unit        # tests/services + tests/config only
bun run test:integration # tests/integration only
bun run test:webrtc      # tests/entrypoints/offscreen/webrtc.*
```

There is no `npm run lint` — lint is handled by `tsc --noEmit`
(via `bun run check`) and the svelte compiler; no dedicated
ESLint setup ships.

## Usage

### Basic Wallet Operations
1. **Generate Wallet**: Click "Show Wallet Address" to create/display address
2. **Sign Messages**: Enter message and click "Sign Message"
3. **Chain Support**: Switch between Ethereum (secp256k1) and Solana (ed25519)

### MPC Session Management
1. **Peer Discovery**: Click "List devices" to find available participants
2. **Create Session**: Click "Propose Session" with 3+ devices
3. **Join Session**: Accept incoming session invitations
4. **Monitor Status**: View connection and session state in real-time

### Advanced Features
- **Network Switching**: Change blockchain networks
- **Account Management**: Import/export private keys
- **Connection Diagnostics**: Debug WebRTC and WebSocket issues

## API Reference

### Background Script API
```typescript
// Account Management
handleAccountManagement(action: string, payload: any)

// Network Management
handleNetworkManagement(action: string, payload: any)

// RPC Handling
handleRpcRequest(request: JsonRpcRequest)
```

### WebRTC Manager API
```typescript
// Session Management
proposeSession(sessionId: string, total: number, threshold: number, participants: string[])
acceptSession(sessionId: string)
resetSession()

// Communication
sendWebRTCAppMessage(todeviceId: string, message: WebRTCAppMessage)
```

### WebSocket Client API
```typescript
// Connection Management
connect()
disconnect()
register(deviceId: string)

// Communication
relayMessage(to: string, data: any)
listdevices()
```

## Error Handling and Recovery

### Offscreen Document Management
- Background script ensures offscreen document exists before forwarding messages
- Creation is protected against concurrent attempts
- Ready signal confirms initialization before use

### WebSocket Reconnection
- Automatic reconnection with exponential backoff
- State synchronization on reconnection
- UI reflects connection status changes

### WebRTC Connection Recovery
- ICE connection state monitoring
- Automatic cleanup of failed connections
- Session reset capabilities for stuck states

## Security Considerations

1. **Message Validation**: All messages are strongly typed and validated
2. **Origin Checking**: Content scripts verify message sources
3. **Isolated Contexts**: WebRTC operations isolated to offscreen context
4. **Secure Communication**: All external communication via WebSocket/WebRTC
5. **Private Key Security**: Keys stored securely in extension storage

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes with proper TypeScript typing
4. Add tests for new functionality
5. Submit a pull request

## License

[Add your license information here]

## Support

For issues and questions:
- Create an issue in the repository
- Check the console logs for debugging information
- Use the built-in diagnostic tools in the popup UI
