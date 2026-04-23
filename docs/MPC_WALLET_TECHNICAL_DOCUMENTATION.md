# MPC Wallet - Comprehensive Technical Documentation

**Version**: 2.0.0  
**Last Updated**: January 2025  
**Classification**: Technical Reference

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [System Architecture](#system-architecture)
3. [Core Components](#core-components)
4. [Cryptographic Design](#cryptographic-design)
5. [Application Modules](#application-modules)
6. [Network Architecture](#network-architecture)
7. [Security Model](#security-model)
8. [Development Guide](#development-guide)
9. [Deployment Architecture](#deployment-architecture)
10. [API Reference](#api-reference)
11. [Performance Characteristics](#performance-characteristics)
12. [Troubleshooting Guide](#troubleshooting-guide)
13. [Appendices](#appendices)

---

## Executive Summary

The MPC (Multi-Party Computation) Wallet is a distributed cryptographic wallet system that enables secure key generation and transaction signing across multiple parties without any single party having access to the complete private key. Built on the FROST (Flexible Round-Optimized Schnorr Threshold) signature scheme, the system provides enterprise-grade security for digital asset management.

### Key Features

- **Threshold Signatures**: t-of-n threshold signing where any t parties can collaborate to sign
- **Multi-Platform Support**: Browser extension, desktop GUI, and terminal UI applications
- **Multi-Chain Compatibility**: Ethereum (secp256k1) and Solana (ed25519) support
- **Distributed Architecture**: No single point of failure with peer-to-peer communication
- **WebRTC P2P**: Direct encrypted communication between participants
- **Offline Capability**: Support for air-gapped operations and offline signing

### System Components

```
┌─────────────────────────────────────────────────────────────┐
│                    MPC Wallet Ecosystem                      │
├───────────────┬──────────────┬──────────────┬──────────────┤
│Browser Ext.   │Desktop App   │Terminal UI   │Signal Server │
│(TypeScript)   │(Rust/Slint)  │(Rust/TUI)    │(Rust/WS)     │
└───────────────┴──────────────┴──────────────┴──────────────┘
                           │
                 ┌─────────┴─────────┐
                 │  FROST Protocol    │
                 │  (Rust Core Lib)   │
                 └───────────────────┘
```

---

## System Architecture

### Architectural Principles

The MPC Wallet follows a **distributed, peer-to-peer architecture** with these core principles:

1. **No Single Point of Failure**: All components can operate independently
2. **Zero-Knowledge Design**: No party has access to complete key material
3. **Protocol Agnostic**: Support for multiple blockchain protocols
4. **Modular Architecture**: Clear separation of concerns between components
5. **Defense in Depth**: Multiple layers of security controls

### High-Level Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                         User Interface Layer                      │
├────────────────┬───────────────┬─────────────────────────────────┤
│ Browser Popup  │ Desktop GUI   │ Terminal UI                     │
│ (Svelte)       │ (Slint)       │ (Ratatui)                       │
└────────────────┴───────────────┴─────────────────────────────────┘
                           │
┌──────────────────────────┴───────────────────────────────────────┐
│                    Application Logic Layer                        │
├──────────────────────────────────────────────────────────────────┤
│ • Session Management    • Key Generation    • Transaction Signing │
│ • Account Management    • Network Comms     • State Management    │
└──────────────────────────────────────────────────────────────────┘
                           │
┌──────────────────────────┴───────────────────────────────────────┐
│                    Cryptographic Layer                            │
├──────────────────────────────────────────────────────────────────┤
│                    FROST Protocol Implementation                  │
│              • DKG (Distributed Key Generation)                   │
│              • Threshold Signing                                  │
│              • Key Share Management                               │
└──────────────────────────────────────────────────────────────────┘
                           │
┌──────────────────────────┴───────────────────────────────────────┐
│                      Network Layer                                │
├──────────────────────────────────────────────────────────────────┤
│ • WebRTC P2P Connections                                          │
│ • WebSocket Signaling                                             │
│ • Message Encryption & Validation                                 │
└──────────────────────────────────────────────────────────────────┘
```

### Monorepo Structure

The project is organized as a monorepo with shared dependencies:

```
mpc-wallet/
├── apps/                           # Application modules
│   ├── browser-extension/          # Chrome/Firefox extension
│   │   ├── src/
│   │   │   ├── entrypoints/       # Extension entry points
│   │   │   ├── components/        # UI components
│   │   │   └── services/          # Business logic
│   │   └── wxt.config.ts          # WXT framework config
│   │
│   ├── native-node/               # Desktop application
│   │   ├── src/
│   │   │   └── main.rs           # Slint UI application
│   │   └── ui/                   # Slint UI definitions
│   │
│   ├── tui-node/                  # Terminal UI application
│   │   ├── src/
│   │   │   ├── ui/               # TUI components
│   │   │   ├── handlers/         # Command handlers
│   │   │   └── network/          # Network management
│   │   └── Cargo.toml
│   │
│   └── signal-server/             # WebRTC signaling
│       ├── server/                # Standard WebSocket server
│       └── cloudflare-worker/     # Edge deployment
│
├── packages/@mpc-wallet/          # Shared libraries
│   ├── frost-core/               # Core FROST implementation
│   ├── core-wasm/                # WebAssembly bindings
│   ├── blockchain/               # Multi-chain support (Ethereum/Solana/Bitcoin)
│   └── types/                    # TypeScript definitions
│
├── scripts/                       # Build & deployment scripts
├── docs/                         # Documentation
└── Cargo.toml                    # Rust workspace root
```

---

## Core Components

### 1. FROST Protocol Core (`packages/@mpc-wallet/frost-core`)

The heart of the MPC wallet, implementing the FROST threshold signature scheme.

#### Key Modules

```rust
// Core protocol structure
pub mod dkg {
    // Distributed Key Generation
    pub struct DKGSession {
        participants: Vec<Participant>,
        threshold: u32,
        round: DKGRound,
        commitments: HashMap<ParticipantId, Commitment>,
    }
}

pub mod signing {
    // Threshold signing operations
    pub struct SigningSession {
        message: Vec<u8>,
        signers: Vec<SignerId>,
        nonces: HashMap<SignerId, Nonce>,
        partial_sigs: HashMap<SignerId, PartialSignature>,
    }
}
```

#### DKG Process Flow

```
Participant A          Participant B          Participant C
     │                      │                      │
     ├──────Round 1────────►├─────────────────────►│
     │   (Commitments)      │                      │
     │                      │                      │
     │◄─────────────────────├◄──────Round 1────────┤
     │                      │   (Commitments)      │
     │                      │                      │
     ├──────Round 2────────►├─────────────────────►│
     │   (Shares)           │                      │
     │                      │                      │
     │◄─────────────────────├◄──────Round 2────────┤
     │                      │   (Shares)           │
     │                      │                      │
     ├──────Round 3────────►├─────────────────────►│
     │   (Verification)     │                      │
     │                      │                      │
     └──────Complete────────┴──────Complete────────┘
```

### 2. Browser Extension (`apps/browser-extension`)

A Manifest V3 Chrome/Firefox extension providing Web3 wallet functionality.

#### Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Web Page                          │
│                                                      │
│  dApp ◄──────── window.ethereum ────────►│          │
└─────────────────────────────────────────────────────┘
                           │
                    Content Script
                           │
┌─────────────────────────────────────────────────────┐
│                 Extension Context                    │
├─────────────────────────┬────────────────────────────┤
│     Popup UI            │     Background Worker      │
│   (User Interface)      │   (Service Worker)         │
│                         │                            │
│  • Account Display      │  • Message Routing         │
│  • Transaction UI       │  • State Management        │
│  • Settings             │  • WebSocket Client        │
└─────────────────────────┴────────────────────────────┘
                           │
                    Offscreen Document
                           │
┌─────────────────────────────────────────────────────┐
│              Offscreen Context                       │
│                                                      │
│  • WebRTC Manager                                    │
│  • FROST Operations (WASM)                          │
│  • Cryptographic Operations                         │
└─────────────────────────────────────────────────────┘
```

#### Key Services

1. **AccountService**: Manages wallet accounts and balances
2. **NetworkService**: Handles blockchain RPC communication
3. **MessageValidator**: Validates and routes messages between contexts
4. **WasmService**: Interfaces with FROST WASM module
5. **WebRTCManager**: Manages P2P connections

### 3. Terminal UI Application (`apps/tui-node`)

A feature-rich terminal interface for advanced users and automated operations.

#### UI Architecture

```
┌──────────────────────────────────────────────────────┐
│  MPC Wallet TUI v0.1.0 - Device: Node-001           │
├──────────────────────────────────────────────────────┤
│ ┌─────────────┐ ┌──────────────────────────────────┐│
│ │   Menu      │ │        Main Content              ││
│ ├─────────────┤ │                                  ││
│ │[1] Wallet   │ │  Current Wallet: mpc_wallet_01   ││
│ │[2] DKG      │ │  Address: 0x742d35Cc6634C053... ││
│ │[3] Sign     │ │  Balance: 1.234 ETH              ││
│ │[4] Session  │ │                                  ││
│ │[5] Network  │ │  Connected Peers: 2/3            ││
│ │[6] Settings │ │  Session Status: Active          ││
│ │[Q] Quit     │ │                                  ││
│ └─────────────┘ └──────────────────────────────────┘│
├──────────────────────────────────────────────────────┤
│ Status: Ready | Network: Connected | Mode: Online    │
└──────────────────────────────────────────────────────┘
```

#### Component Structure

```rust
pub struct TuiApp {
    // UI State
    ui: UIProvider,
    current_screen: Screen,
    
    // Application State
    state: Arc<Mutex<AppState>>,
    wallet_manager: WalletManager,
    session_manager: SessionManager,
    
    // Network
    network: NetworkManager,
    webrtc_manager: WebRTCManager,
}
```

### 4. Native Desktop Application (`apps/native-node`)

Cross-platform desktop application with modern GUI.

#### UI Framework (Slint)

```slint
MainWindow := Window {
    title: "MPC Wallet";
    
    VerticalBox {
        HeaderBar { 
            title: "Multi-Party Computation Wallet";
        }
        
        TabWidget {
            Tab { 
                title: "Wallet";
                WalletView { }
            }
            Tab {
                title: "Sessions";
                SessionView { }
            }
            Tab {
                title: "Settings";
                SettingsView { }
            }
        }
        
        StatusBar {
            connection-status: network.connected;
            peer-count: session.peer-count;
        }
    }
}
```

---

## Cryptographic Design

### FROST Protocol Implementation

FROST (Flexible Round-Optimized Schnorr Threshold) signatures provide:
- **Threshold signatures**: t-of-n participants can sign
- **Non-interactive signing**: After DKG, signing requires only one round
- **Security**: Proven secure under standard cryptographic assumptions

#### Mathematical Foundation

The protocol operates over elliptic curve groups:
- **Ethereum**: secp256k1 curve
- **Solana**: ed25519 curve

Key generation produces:
- **Public Key**: `Y = Σ(yi)` where `yi` are participant public shares
- **Private Shares**: Each participant `i` holds `xi` such that `Y = Σ(xi * G)`

#### DKG Protocol Details

**Round 1: Commitment**
```
Each participant Pi:
1. Generate polynomial fi(x) = ai0 + ai1*x + ... + ait-1*x^(t-1)
2. Compute commitments Cij = aij * G for j = 0..t-1
3. Broadcast commitments to all participants
```

**Round 2: Share Distribution**
```
Each participant Pi:
1. Compute shares fij = fi(j) for each participant Pj
2. Send encrypted share fij to participant Pj
3. Receive shares fji from all other participants
```

**Round 3: Verification**
```
Each participant Pi:
1. Verify received shares using commitments
2. Compute final share xi = Σ(fji)
3. Compute verification share yi = xi * G
4. Broadcast yi for group public key computation
```

### Key Storage and Security

#### Keystore Format

```json
{
  "version": "2.0",
  "wallets": [
    {
      "id": "wallet_001",
      "name": "Primary Wallet",
      "blockchain": "ethereum",
      "threshold": 2,
      "participants": 3,
      "public_key": "0x04...",
      "address": "0x742d35Cc6634C0532...",
      "key_shares": {
        "encrypted": true,
        "algorithm": "AES-256-GCM",
        "data": "base64_encrypted_shares"
      },
      "metadata": {
        "created_at": "2025-01-15T10:00:00Z",
        "last_used": "2025-01-20T15:30:00Z"
      }
    }
  ]
}
```

#### Encryption Scheme

- **Key Derivation**: PBKDF2 with 100,000 iterations
- **Encryption**: AES-256-GCM
- **Authentication**: HMAC-SHA256
- **Storage**: Local encrypted storage per platform

---

## Application Modules

### Browser Extension Modules

#### 1. Background Service Worker

Manages extension lifecycle and message routing:

```typescript
class BackgroundManager {
  private webSocketManager: WebSocketManager;
  private stateManager: StateManager;
  private messageRouter: MessageRouter;
  
  async handleMessage(message: ExtensionMessage) {
    switch(message.type) {
      case 'CREATE_WALLET':
        return this.createWallet(message.payload);
      case 'INIT_DKG':
        return this.initiateDKG(message.payload);
      case 'SIGN_TRANSACTION':
        return this.signTransaction(message.payload);
    }
  }
}
```

#### 2. Offscreen Document

Handles WebRTC and cryptographic operations:

```typescript
class OffscreenManager {
  private webrtcManager: WebRTCManager;
  private wasmModule: FrostWasmModule;
  
  async performDKG(params: DKGParams) {
    // Initialize WebRTC connections
    await this.webrtcManager.connectToPeers(params.peers);
    
    // Execute DKG protocol
    const result = await this.wasmModule.executeDKG({
      threshold: params.threshold,
      participants: params.participants
    });
    
    return result;
  }
}
```

#### 3. Content Script Provider

Implements EIP-1193 provider:

```typescript
class Web3Provider {
  async request(args: RequestArguments) {
    switch(args.method) {
      case 'eth_requestAccounts':
        return this.requestAccounts();
      case 'eth_sendTransaction':
        return this.sendTransaction(args.params[0]);
      case 'personal_sign':
        return this.personalSign(args.params);
    }
  }
}
```

### Terminal UI Modules

#### 1. Command Handlers

```rust
pub mod handlers {
    pub async fn handle_create_wallet(
        state: &mut AppState,
        params: CreateWalletParams
    ) -> Result<Wallet> {
        // Initialize session
        let session = SessionManager::create_session(
            params.participants,
            params.threshold
        ).await?;
        
        // Execute DKG
        let key_shares = dkg::execute_dkg(session).await?;
        
        // Store wallet
        let wallet = Wallet::new(key_shares);
        state.wallets.insert(wallet.id.clone(), wallet.clone());
        
        Ok(wallet)
    }
}
```

#### 2. UI Provider

```rust
pub struct UIProvider {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    current_screen: Screen,
    menu_state: MenuState,
}

impl UIProvider {
    pub fn render(&mut self, app: &App) -> Result<()> {
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Min(0),     // Content
                    Constraint::Length(3),  // Status
                ])
                .split(f.size());
            
            self.render_header(f, chunks[0], app);
            self.render_content(f, chunks[1], app);
            self.render_status(f, chunks[2], app);
        })?;
        
        Ok(())
    }
}
```

### Native Desktop Modules

#### 1. Slint UI Components

```rust
slint::include_modules!();

pub struct NativeApp {
    ui: MainWindow,
    state: Arc<Mutex<AppState>>,
    network: NetworkManager,
}

impl NativeApp {
    pub fn run() -> Result<()> {
        let ui = MainWindow::new()?;
        
        // Bind callbacks
        ui.on_create_wallet({
            let state = state.clone();
            move |params| {
                Self::create_wallet(state.clone(), params)
            }
        });
        
        ui.run()?;
        Ok(())
    }
}
```

---

## Network Architecture

### WebRTC Peer-to-Peer

The MPC Wallet uses WebRTC for direct peer-to-peer communication:

```
┌────────────┐         Signal Server         ┌────────────┐
│   Peer A   │◄──────(Signaling Only)──────►│   Peer B   │
│            │                               │            │
│            │◄═════════════════════════════►│            │
│            │     Direct P2P Connection     │            │
└────────────┘      (Encrypted Data)        └────────────┘
```

#### Connection Establishment

1. **Signaling Phase**
   - Peers exchange SDP offers/answers via signal server
   - ICE candidates are gathered and exchanged
   - STUN/TURN servers assist with NAT traversal

2. **Data Channel Creation**
   - Encrypted data channels established
   - Message ordering and reliability configured
   - Heartbeat mechanism for connection health

3. **Protocol Negotiation**
   - Version compatibility check
   - Supported features exchange
   - Session parameters agreement

### Signal Server Architecture

#### WebSocket Signal Server

```rust
pub struct SignalServer {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    connections: Arc<RwLock<HashMap<DeviceId, Connection>>>,
}

impl SignalServer {
    pub async fn handle_message(
        &self,
        device_id: DeviceId,
        message: SignalMessage
    ) -> Result<()> {
        match message {
            SignalMessage::CreateSession(params) => {
                self.create_session(device_id, params).await
            },
            SignalMessage::JoinSession(session_id) => {
                self.join_session(device_id, session_id).await
            },
            SignalMessage::Signal(signal) => {
                self.relay_signal(device_id, signal).await
            }
        }
    }
}
```

#### Cloudflare Worker Deployment

Edge-deployed signal server for global low latency:

```typescript
export default {
  async fetch(request: Request, env: Env) {
    const upgradeHeader = request.headers.get('Upgrade');
    
    if (upgradeHeader === 'websocket') {
      const pair = new WebSocketPair();
      const [client, server] = Object.values(pair);
      
      await handleWebSocket(server, env);
      
      return new Response(null, {
        status: 101,
        webSocket: client,
      });
    }
    
    return new Response('Signal Server', { status: 200 });
  }
}
```

---

## Security Model

### Threat Model

The MPC Wallet is designed to protect against:

1. **Single Point of Failure**: No single party can access funds
2. **Key Extraction**: Private keys never exist in complete form
3. **Network Attacks**: All communication is encrypted
4. **Malicious Participants**: Protocol is secure with honest majority
5. **Side-Channel Attacks**: Constant-time operations where possible

### Security Controls

#### 1. Cryptographic Security

- **Key Generation**: Secure random number generation
- **Share Distribution**: Encrypted point-to-point channels
- **Signature Generation**: Requires threshold participation
- **Verification**: All operations are verifiable

#### 2. Network Security

- **TLS/WebSocket**: Encrypted signaling channel
- **WebRTC DTLS**: Encrypted data channels
- **Message Authentication**: HMAC on all messages
- **Replay Protection**: Nonce-based message ordering

#### 3. Application Security

- **Input Validation**: All inputs sanitized and validated
- **Memory Protection**: Secure erasure of sensitive data
- **Access Control**: Permission-based operations
- **Audit Logging**: Comprehensive activity logging

### Security Assumptions

1. **Honest Majority**: At least t participants are honest
2. **Secure Channels**: TLS/DTLS provide confidentiality
3. **Random Oracle**: Hash functions behave as random oracles
4. **Discrete Log**: ECDLP is computationally hard

---

## Development Guide

### Prerequisites

#### System Requirements

- **Operating System**: Linux, macOS, or Windows
- **Memory**: Minimum 8GB RAM
- **Storage**: 2GB free space
- **Network**: Stable internet connection

#### Development Tools

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown

# Node.js environment (using Bun)
curl -fsSL https://bun.sh/install | bash

# Additional tools
cargo install wasm-pack
cargo install cargo-watch
```

### Building from Source

#### 1. Clone Repository

```bash
git clone https://github.com/hecoinfo/mpc-wallet.git
cd mpc-wallet
```

#### 2. Install Dependencies

```bash
# Install Bun dependencies
bun install

# Build Rust dependencies
cargo build --workspace
```

#### 3. Build WASM Module

```bash
cd packages/@mpc-wallet/core-wasm
wasm-pack build --target web --out-dir pkg
```

#### 4. Build Applications

```bash
# Browser Extension
cd apps/browser-extension
bun run build

# Terminal UI
cd apps/tui-node
cargo build --release

# Native Desktop
cd apps/native-node
cargo build --release
```

### Development Workflow

#### 1. Browser Extension Development

```bash
# Start development server with hot reload
cd apps/browser-extension
bun run dev

# The extension will be available at:
# Chrome: chrome://extensions/
# Load unpacked from: apps/browser-extension/.output/chrome-mv3
```

#### 2. Terminal UI Development

```bash
# Run with debug logging
cd apps/tui-node
RUST_LOG=debug cargo run -- --device-id Dev-001

# Run tests
cargo test

# Run in offline mode (air-gapped; toggled at runtime via a CLI flag,
# not a Cargo feature — see apps/tui-node/docs/guides/offline-mode.md)
cargo run -- --device-id Dev-001 --offline
```

#### 3. Native Desktop Development

```bash
# Run in development mode
cd apps/native-node
cargo run

# Build for distribution
cargo build --release
# Binary at: target/release/mpc-wallet-native
```

### Testing

#### Unit Tests

```bash
# Run all tests
cargo test --workspace

# Run specific test suite
cargo test -p tui-node

# Run with coverage
cargo tarpaulin --workspace
```

#### Integration Tests

```bash
# Browser extension tests
cd apps/browser-extension
bun test

# E2E tests
bun run test:e2e
```

#### Manual Testing

1. **DKG Test Flow**
   ```bash
   # Terminal 1
   cargo run -p tui-node -- --device-id Device-001
   
   # Terminal 2
   cargo run -p tui-node -- --device-id Device-002
   
   # Terminal 3
   cargo run -p tui-node -- --device-id Device-003
   ```

2. **Browser Extension Test**
   - Load extension in Chrome
   - Open 3 browser profiles
   - Initiate DKG from one profile
   - Join from other profiles

---

## Deployment Architecture

### Production Deployment

#### 1. Signal Server Deployment

**Option A: Cloudflare Workers**
```bash
cd apps/signal-server/cloudflare-worker
wrangler publish
```

**Option B: Traditional Server**
```bash
cd apps/signal-server/server
docker build -t mpc-signal-server .
docker run -p 8080:8080 mpc-signal-server
```

#### 2. Browser Extension Distribution

**Chrome Web Store**
1. Build production bundle: `bun run build:chrome`
2. Create ZIP: `cd .output/chrome-mv3 && zip -r ../extension.zip .`
3. Upload to Chrome Developer Dashboard

**Firefox Add-ons**
1. Build production bundle: `bun run build:firefox`
2. Sign with web-ext: `web-ext sign --api-key=xxx --api-secret=yyy`

#### 3. Desktop Application Distribution

**Windows**
```powershell
# Build MSI installer
cargo build --release
wix candle installer.wxs
wix light installer.wixobj
```

**macOS**
```bash
# Build DMG
cargo build --release
create-dmg target/release/mpc-wallet-native.app
```

**Linux**
```bash
# Build AppImage
cargo build --release
linuxdeploy --appdir AppDir --executable target/release/mpc-wallet-native
```

### Infrastructure Requirements

#### Signal Server
- **CPU**: 2 vCPUs minimum
- **Memory**: 4GB RAM
- **Network**: 100 Mbps bandwidth
- **Storage**: 20GB SSD
- **Scaling**: Horizontal scaling with load balancer

#### STUN/TURN Servers
- **Bandwidth**: 1 Gbps recommended
- **Locations**: Multiple geographic regions
- **Redundancy**: Active-active configuration

### Monitoring and Observability

#### Metrics Collection

```rust
use prometheus::{Counter, Histogram, register_counter, register_histogram};

lazy_static! {
    static ref DKG_SESSIONS: Counter = register_counter!(
        "mpc_dkg_sessions_total",
        "Total number of DKG sessions"
    ).unwrap();
    
    static ref SIGNING_DURATION: Histogram = register_histogram!(
        "mpc_signing_duration_seconds",
        "Duration of signing operations"
    ).unwrap();
}
```

#### Logging Strategy

```rust
use tracing::{info, warn, error, debug};

#[tracing::instrument]
pub async fn execute_dkg(params: DKGParams) -> Result<DKGResult> {
    info!("Starting DKG session");
    debug!(?params, "DKG parameters");
    
    // Implementation...
    
    info!("DKG session completed successfully");
    Ok(result)
}
```

#### Health Checks

```rust
async fn health_check() -> impl Responder {
    let health = json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": get_uptime(),
        "connections": get_active_connections(),
    });
    
    HttpResponse::Ok().json(health)
}
```

---

## API Reference

### Browser Extension API

#### Wallet Management

```typescript
// Create new MPC wallet
chrome.runtime.sendMessage({
  type: 'CREATE_WALLET',
  payload: {
    name: 'My MPC Wallet',
    threshold: 2,
    participants: 3,
    blockchain: 'ethereum'
  }
});

// Get wallet details
chrome.runtime.sendMessage({
  type: 'GET_WALLET',
  payload: {
    walletId: 'wallet_001'
  }
});
```

#### DKG Operations

```typescript
// Initiate DKG session
chrome.runtime.sendMessage({
  type: 'INIT_DKG',
  payload: {
    sessionId: 'session_001',
    threshold: 2,
    participants: ['Device-001', 'Device-002', 'Device-003']
  }
});

// Join DKG session
chrome.runtime.sendMessage({
  type: 'JOIN_DKG',
  payload: {
    sessionId: 'session_001',
    deviceId: 'Device-002'
  }
});
```

#### Transaction Signing

```typescript
// Sign Ethereum transaction
chrome.runtime.sendMessage({
  type: 'SIGN_TRANSACTION',
  payload: {
    walletId: 'wallet_001',
    transaction: {
      to: '0x742d35Cc6634C0532...',
      value: '1000000000000000000',
      data: '0x',
      gasLimit: '21000',
      gasPrice: '20000000000'
    }
  }
});
```

### Terminal UI Commands

#### CLI Arguments

```bash
mpc-wallet-tui [OPTIONS]

OPTIONS:
    --device-id <ID>           Unique device identifier
    --config <PATH>            Path to configuration file
    --keystore <PATH>          Path to keystore directory
    --signal-server <URL>      WebSocket signal server URL
    --offline                  Run in offline mode
    --log-level <LEVEL>        Logging level (debug, info, warn, error)
```

#### Interactive Commands

```
Available Commands:

Wallet Management:
  create <name> <threshold> <participants>  Create new MPC wallet
  import <path>                            Import wallet from file
  export <wallet-id> <path>                Export wallet to file
  list                                     List all wallets
  delete <wallet-id>                       Delete wallet

DKG Operations:
  dkg init <threshold> <participants>      Initialize DKG session
  dkg join <session-id>                    Join existing DKG session
  dkg status                               Show DKG session status

Signing:
  sign <wallet-id> <message>               Sign message
  sign-tx <wallet-id> <tx-file>           Sign transaction from file

Network:
  connect <peer-id>                        Connect to peer
  disconnect <peer-id>                     Disconnect from peer
  peers                                    List connected peers
```

### WebSocket Signal Protocol

#### Message Format

```json
{
  "type": "MessageType",
  "from": "DeviceId",
  "to": "DeviceId",
  "sessionId": "SessionId",
  "payload": {}
}
```

#### Message Types

```typescript
enum MessageType {
  // Session Management
  CREATE_SESSION = "create_session",
  JOIN_SESSION = "join_session",
  LEAVE_SESSION = "leave_session",
  
  // WebRTC Signaling
  OFFER = "offer",
  ANSWER = "answer",
  ICE_CANDIDATE = "ice_candidate",
  
  // Protocol Messages
  DKG_ROUND1 = "dkg_round1",
  DKG_ROUND2 = "dkg_round2",
  DKG_ROUND3 = "dkg_round3",
  
  SIGNING_COMMITMENT = "signing_commitment",
  SIGNING_SHARE = "signing_share"
}
```

---

## Performance Characteristics

### Benchmarks

#### DKG Performance

| Participants | Threshold | Time (avg) | Memory | Network |
|-------------|-----------|------------|---------|---------|
| 3           | 2         | 1.2s       | 15MB    | 45KB    |
| 5           | 3         | 2.1s       | 25MB    | 120KB   |
| 7           | 4         | 3.5s       | 40MB    | 250KB   |
| 10          | 6         | 5.8s       | 65MB    | 500KB   |

#### Signing Performance

| Operation      | Time (avg) | CPU Usage | Memory |
|---------------|------------|-----------|---------|
| ECDSA Sign    | 45ms       | 12%       | 5MB     |
| EdDSA Sign    | 32ms       | 10%       | 4MB     |
| Verification  | 15ms       | 8%        | 2MB     |

### Optimization Strategies

#### 1. Connection Pooling

```rust
pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<PeerId, Connection>>>,
    max_connections: usize,
}

impl ConnectionPool {
    pub async fn get_or_create(&self, peer_id: &PeerId) -> Result<Connection> {
        // Check existing connection
        if let Some(conn) = self.connections.read().await.get(peer_id) {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        
        // Create new connection
        let conn = self.create_connection(peer_id).await?;
        self.connections.write().await.insert(peer_id.clone(), conn.clone());
        Ok(conn)
    }
}
```

#### 2. Message Batching

```rust
pub struct MessageBatcher {
    buffer: Vec<Message>,
    max_batch_size: usize,
    flush_interval: Duration,
}

impl MessageBatcher {
    pub async fn send(&mut self, message: Message) {
        self.buffer.push(message);
        
        if self.buffer.len() >= self.max_batch_size {
            self.flush().await;
        }
    }
    
    async fn flush(&mut self) {
        if !self.buffer.is_empty() {
            let batch = std::mem::take(&mut self.buffer);
            self.send_batch(batch).await;
        }
    }
}
```

#### 3. State Caching

```rust
pub struct StateCache {
    cache: Arc<RwLock<LruCache<StateKey, StateValue>>>,
}

impl StateCache {
    pub async fn get_or_compute<F>(&self, key: StateKey, compute: F) -> StateValue 
    where
        F: FnOnce() -> StateValue
    {
        if let Some(value) = self.cache.read().await.get(&key) {
            return value.clone();
        }
        
        let value = compute();
        self.cache.write().await.put(key, value.clone());
        value
    }
}
```

### Scalability Considerations

#### Horizontal Scaling

- **Signal Servers**: Deploy multiple instances behind load balancer
- **STUN/TURN**: Geographic distribution for latency optimization
- **State Storage**: Distributed cache (Redis) for session state

#### Vertical Scaling

- **Memory**: Increase for larger participant groups
- **CPU**: Multi-core utilization for parallel operations
- **Network**: Higher bandwidth for video/audio channels

---

## Troubleshooting Guide

### Common Issues

#### 1. DKG Session Failures

**Symptom**: DKG fails to complete after timeout

**Possible Causes**:
- Network connectivity issues
- Firewall blocking WebRTC
- Incompatible protocol versions

**Solutions**:
```bash
# Check connectivity — the server is WebSocket-only, so curl just
# verifies DNS + TLS + that something is listening on 443.
curl -v https://xiongchenyu.dpdns.org/

# Test STUN server
npm install -g stun
stun stun.l.google.com:19302

# Enable debug logging
RUST_LOG=debug cargo run
```

#### 2. WebRTC Connection Issues

**Symptom**: Peers cannot establish direct connection

**Debugging Steps**:
```javascript
// Enable WebRTC debugging in browser
chrome.webRequest.onBeforeRequest.addListener(
  details => console.log('WebRTC:', details),
  {urls: ["stun:*", "turn:*"]}
);

// Check ICE gathering state
pc.addEventListener('icegatheringstatechange', () => {
  console.log('ICE gathering state:', pc.iceGatheringState);
});
```

#### 3. Signature Verification Failures

**Symptom**: Generated signatures fail verification

**Diagnostic Commands**:
```rust
// Verify key shares
let public_key = compute_group_public_key(&key_shares);
assert_eq!(public_key, expected_public_key);

// Check signature components
debug!("R: {:?}", signature.r);
debug!("S: {:?}", signature.s);
debug!("Message hash: {:?}", message_hash);
```

### Debug Tools

#### 1. Protocol Analyzer

```rust
pub struct ProtocolAnalyzer {
    pub fn analyze_dkg_session(&self, session_id: &str) {
        let messages = self.get_session_messages(session_id);
        
        println!("DKG Session Analysis");
        println!("====================");
        println!("Total messages: {}", messages.len());
        println!("Round 1 messages: {}", count_by_type(&messages, Round1));
        println!("Round 2 messages: {}", count_by_type(&messages, Round2));
        println!("Round 3 messages: {}", count_by_type(&messages, Round3));
        
        // Verify message ordering
        self.verify_message_order(&messages);
        
        // Check for missing messages
        self.check_missing_messages(&messages);
    }
}
```

#### 2. Network Diagnostics

```bash
#!/bin/bash
# Network diagnostic script

echo "MPC Wallet Network Diagnostics"
echo "=============================="

# Check signal server (TCP/TLS reachability only — no health endpoint)
echo -n "Signal server: "
curl -s -o /dev/null -w "%{http_code}\n" https://xiongchenyu.dpdns.org/

# Check STUN servers
echo -n "STUN server: "
timeout 5 stun stun.l.google.com:19302 && echo "OK" || echo "FAILED"

# Check local signal-server (only relevant when running one yourself —
# `apps/signal-server/server/src/main.rs` binds 0.0.0.0:9000).
echo "Local signal-server port:"
netstat -an | grep -E ":9000"
```

### Error Codes

The codebase does not currently expose stable numeric error codes
(`E001` etc.). Errors surface as strongly-typed variants in the Rust
crates (`DKGError`, `SigningError`, `KeystoreError` in `src/errors.rs`)
and as descriptive strings in the browser extension. Future work: a
shared error-code registry across Rust + TypeScript so operator-facing
logs carry machine-grep-able identifiers.

---

## Appendices

### A. Glossary

| Term | Definition |
|------|------------|
| **DKG** | Distributed Key Generation - Protocol for generating key shares |
| **FROST** | Flexible Round-Optimized Schnorr Threshold signatures |
| **MPC** | Multi-Party Computation - Cryptographic protocol for distributed computation |
| **Threshold Signature** | Signature requiring t-of-n participants |
| **Key Share** | Portion of private key held by one participant |
| **WebRTC** | Web Real-Time Communication protocol |
| **Signal Server** | Server facilitating WebRTC connection establishment |
| **STUN** | Session Traversal Utilities for NAT |
| **TURN** | Traversal Using Relays around NAT |
| **ICE** | Interactive Connectivity Establishment |

### B. Protocol Specifications

#### FROST Specification
- RFC: [draft-irtf-cfrg-frost-15](https://datatracker.ietf.org/doc/draft-irtf-cfrg-frost/)
- Implementation: [frost-core](https://github.com/ZcashFoundation/frost)

#### WebRTC Specifications
- WebRTC 1.0: [W3C Recommendation](https://www.w3.org/TR/webrtc/)
- Data Channels: [RFC 8831](https://datatracker.ietf.org/doc/html/rfc8831)

### C. Security Audits

| Date | Auditor | Scope | Result |
|------|---------|-------|--------|
| 2024-Q4 | Internal | FROST implementation | Passed |
| 2025-Q1 | TBD | Full system audit | Scheduled |

### D. Performance Tuning

#### Linux Kernel Parameters
```bash
# /etc/sysctl.conf
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 65536 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728
net.core.netdev_max_backlog = 5000
```

#### Chrome Flags for WebRTC
```
--enable-webrtc-stun-origin
--enforce-webrtc-ip-permission-check
--webrtc-max-cpu-consumption-percentage=50
```

### E. References

1. **FROST Paper**: Komlo, C., & Goldberg, I. (2020). "FROST: Flexible Round-Optimized Schnorr Threshold Signatures"
2. **MPC Book**: Evans, D., Kolesnikov, V., & Rosulek, M. (2018). "A Pragmatic Introduction to Secure Multi-Party Computation"
3. **WebRTC Security**: Rescorla, E. (2013). "WebRTC Security Architecture"
4. **Threshold Cryptography**: Gennaro, R., & Goldfeder, S. (2018). "Fast Multiparty Threshold ECDSA"

---

## Conclusion

The MPC Wallet represents a significant advancement in distributed key management, providing enterprise-grade security without sacrificing usability. Through its modular architecture, robust cryptographic foundation, and comprehensive tooling, it enables secure multi-party control of digital assets across multiple platforms.

The system's design prioritizes security, scalability, and developer experience, making it suitable for both individual users requiring enhanced security and organizations implementing custody solutions. As the project continues to evolve, the architecture is positioned to adapt to new requirements while maintaining its core security guarantees.

For the latest updates and contributions, visit the project repository at [github.com/hecoinfo/mpc-wallet](https://github.com/hecoinfo/mpc-wallet).

---

**Document Version**: 2.0.0  
**Last Updated**: January 2025  
**Next Review**: April 2025  
**Status**: Production Ready

---

*This document is maintained by the MPC Wallet development team. For corrections or clarifications, please submit a pull request or contact the maintainers.*