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
**Location:** `/src/entrypoints/background/index.ts`

**Responsibilities:**
- Central message router for all communication
- WebSocket client management for signaling server
- Account and network services
- Offscreen document lifecycle management
- RPC request handling for blockchain operations

**Key Services:**
- `AccountService`: Manages wallet accounts and addresses
- `NetworkService`: Handles blockchain network configurations
- `WalletClientService`: Provides blockchain client functionality
- `WebSocketClient`: Manages connection to signaling server

### 2. Popup Page (UI)
**Location:** `/src/entrypoints/popup/App.svelte`

**Responsibilities:**
- User interface for wallet operations
- Display connection status and peer information
- Session management UI for MPC operations
- Crypto operations (signing, address generation)

**Features:**
- MPC-based distributed key generation (DKG)
- Multi-chain support (Ethereum/Solana)
- Threshold message signing via MPC protocol
- Real-time peer discovery and session management
- WebRTC connection status monitoring

### 3. Offscreen Page (WebRTC Handler)
**Location:** `/src/entrypoints/offscreen/index.ts`

**Responsibilities:**
- WebRTC connection management
- P2P communication handling
- MPC session coordination
- DOM-dependent operations

**Key Components:**
- `WebRTCManager`: Handles peer-to-peer connections
- Session proposal and acceptance logic
- Data channel management for MPC communication
- ICE candidate exchange and connection establishment

### 4. Content Script (Web Integration)
**Location:** `/src/entrypoints/content/index.ts`

**Responsibilities:**
- Injects wallet API into web pages
- Provides `window.ethereum` compatibility
- Proxies JSON-RPC requests to background script
- Manages web page wallet interactions

## Message System

The extension uses a comprehensive type-safe message system defined in `/src/types/messages.ts`. Understanding the message flow directions is crucial for proper implementation.

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
**File:** `/src/types/messages.ts` - `BackgroundMessage`

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
**File:** `/src/types/messages.ts` - `PopupMessage`

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
**File:** `/src/types/messages.ts` - `BackgroundToOffscreenMessage`

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
**File:** `/src/types/messages.ts` - Uses same `OffscreenMessage` type

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
- Node.js 18+ and npm/yarn
- Chrome/Chromium browser for testing
- Rust toolchain for WASM compilation

### Development Setup
```bash
# Clone the repository
git clone <repository-url>
cd mpc-wallet

# Install dependencies
npm install

# Build WASM modules
npm run build:wasm

# Start development server
npm run dev

# Build for production
npm run build
```

### Extension Installation
1. Build the extension: `npm run build`
2. Open Chrome and navigate to `chrome://extensions/`
3. Enable "Developer mode"
4. Click "Load unpacked" and select the `dist` folder

## Development

### Understanding Message Flow in Development

When developing new features, follow these patterns:

1. **Define Message Types**: Add to `/src/types/messages.ts`
2. **Popup Actions**: Send messages via `chrome.runtime.sendMessage()`
3. **Background Processing**: Handle in `chrome.runtime.onMessage.addListener()`
4. **Offscreen Operations**: Forward via `safelySendOffscreenMessage()`
5. **State Updates**: Broadcast to all components via `broadcastToPopupPorts()`

### Debugging Message Flow

1. **Background Console**: Check Service Worker console for message routing
2. **Popup Console**: Monitor UI-triggered messages
3. **Offscreen Console**: Debug WebRTC and crypto operations
4. **Message Validation**: Use TypeScript for compile-time message validation

### Project Structure
```
src/
├── entrypoints/
│   ├── background/     # Service worker
│   ├── content/        # Content scripts
│   ├── offscreen/      # Offscreen document
│   └── popup/          # Extension popup UI
├── types/              # TypeScript type definitions
├── services/           # Business logic services
└── components/         # Svelte UI components
```

### Key Files
- `src/types/messages.ts`: Message type definitions
- `src/types/appstate.ts`: Application state types
- `src/entrypoints/background/index.ts`: Main background script
- `src/entrypoints/offscreen/webrtc.ts`: WebRTC management
- `src/entrypoints/popup/App.svelte`: Main UI component

### Testing
```bash
# Run type checking
npm run type-check

# Run linting
npm run lint

# Run tests
npm run test
```

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
