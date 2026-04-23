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

Real layout — see `packages/@mpc-wallet/frost-core/src/`:

```
lib.rs                   # Re-exports + wiring
unified_dkg.rs           # Dual-curve DKG (ed25519 + secp256k1 from one root)
hd_derivation.rs         # BIP-32-style additive derivation (no extra DKG)
traits.rs                # FrostCurve trait (curve-agnostic ops)
ed25519.rs               # ed25519 impl (Solana address derivation)
secp256k1.rs             # secp256k1 impl (Ethereum address derivation)
keystore.rs              # AES-256-GCM + PBKDF2 encrypted key share I/O
root_secret.rs           # HKDF from root entropy → per-curve RNGs
errors.rs                # Typed error variants
```

The actual DKG + signing primitives come from upstream
`frost-core 2.2` / `frost-ed25519 2.2` / `frost-secp256k1 2.2` crates.
`unified_dkg` wraps them to run both curves simultaneously from a
single root secret. There is no custom `DKGSession` / `SigningSession`
struct in this crate — you interact with upstream types.

#### DKG Process Flow

FROST DKG (`dkg::part1` → `part2` → `part3`) runs in two wire rounds
plus a local finalize step. Once participants complete signaling and
bring up the WebRTC mesh:

```
Participant A          Participant B          Participant C
     │                      │                      │
     │── part1 (commitment + proof-of-knowledge) ───
     │─ broadcast to every peer ──►
     │
     │◄── part1 from B, from C
     │
     │── part2 (encrypted per-peer share, unicast) ─
     │─ A→B share ──►│   │◄── B→A share
     │─ A→C share ────────────────►│
     │
     │── part3 (local finalize: combine received shares
     │         to derive key_package + group public key) ──
     │                      │                      │
     └──────Complete────────┴──────Complete────────┘
```

Rounds don't traverse the signal server — they ride the peer-to-peer
WebRTC data channels established during setup (see the § Network
Architecture section).

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

Real service layer (`apps/browser-extension/src/services/`):

1. **AccountService** (`accountService.ts`): Wallet account list,
   address derivation per curve, active-account selection.
2. **NetworkService** (`networkService.ts`): EVM chain-ID / RPC
   endpoint registry (EIP-1193 provider side).
3. **KeystoreService** / **KeystoreManager** (`keystoreService.ts` /
   `keystoreManager.ts`): Encrypted share persistence, unlock flow.
4. **PermissionService** (`permissionService.ts`): Per-origin dApp
   connection state, request gating.
5. **WalletController** / **WalletClient** (`walletController.ts` /
   `walletClient.ts`): High-level orchestration + popup-facing API.

Separately, in the offscreen context:

6. **WebRTCManager** (`src/entrypoints/offscreen/webrtc.ts`): Full-mesh
   peer connection state + FROST state (`frostDkg`, `signingInfo`,
   `signingCommitments`, `signingShares`). See CLAUDE.md for the
   signing pipeline.

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

The TUI is built on the [tui-realm](https://github.com/veeso/tuirealm)
Elm architecture. The real entry struct is `ElmApp<C>`
(`apps/tui-node/src/elm/app.rs`), parameterized over a FROST ciphersuite:

```rust
pub struct ElmApp<C: frost_core::Ciphersuite> {
    model: Model,                                       // pure app state
    app: Application<Id, Message, UserEvent>,           // tui-realm app shell
    terminal: CrosstermTerminalAdapter,                 // render target
    message_tx: UnboundedSender<Message>,
    message_rx: UnboundedReceiver<Message>,
    app_state: Arc<Mutex<AppState<C>>>,                 // shared with non-Elm managers
    should_quit: bool,
}
```

Longer-lived business logic lives in `tui-node::core::*Manager`
types (`WalletManager`, `SessionManager`, `DkgManager`, `SigningManager`,
`OfflineManager`, `ConnectionManager`) — these are shared with the
native-node app via the `UICallback` trait. See CLAUDE.md for that
layering.

### 4. Native Desktop Application (`apps/native-node`)

Cross-platform desktop application with modern GUI.

#### UI Framework (Slint)

Real UI entry point: `apps/native-node/ui/main_enhanced.slint`. Uses
std-widgets (TabWidget, VerticalBox, HorizontalBox, GroupBox, LineEdit,
TextEdit, ListView, Button, ComboBox, ScrollView) — no custom
`HeaderBar` / `StatusBar` components. Sketch of the actual structure:

```slint
import { TabWidget, VerticalBox, HorizontalBox, GroupBox, /*...*/ }
    from "std-widgets.slint";

export component MainWindow inherits Window {
    // AppState globals (populated from Rust via UICallback)
    VerticalBox {
        // Header region (plain Text + HorizontalBox, not a widget)
        ...

        TabWidget {
            // Tab 1: Wallets
            VerticalBox { ... WalletSelector + forms }
            // Tab 2: Sessions / DKG
            VerticalBox { ... }
            // Tab 3: Signing
            VerticalBox { ... }
            // Tab 4: Network / Settings
            VerticalBox { ... }
        }
    }
}
```

Callbacks on MainWindow are wired to Rust closures via
`slint::invoke_from_event_loop` — see CLAUDE.md's Slint integration
section for the `Weak<MainWindow>` + Send-bridge pattern.

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

- **Key Derivation**: PBKDF2-HMAC-SHA256 with 100,000 iterations
  (constant `PBKDF2_ITERATIONS` in `apps/tui-node/src/keystore/encryption.rs`).
- **Encryption + Authentication**: AES-256-GCM. GCM provides
  confidentiality and authenticity in one pass — there is no separate
  HMAC layer; the GCM auth tag is the MAC.
- **Storage**: Local filesystem at `~/.frost_keystore/<device_id>/<curve>/<wallet_id>.dat`.

---

## Application Modules

### Browser Extension

Four runtime contexts (MV3), each rooted under `src/entrypoints/`.
Full flow diagram + entry-points table lives in CLAUDE.md; this
section summarizes the layering.

#### Background service worker (`src/entrypoints/background/`)

Orchestrates everything. Real managers:

| Class | File | Role |
|---|---|---|
| `StateManager` | `stateManager.ts` | Persistent state, cross-context broadcast, signing/DKG state listeners |
| `SessionManager` | `sessionManager.ts` | `createSigningSession`, `joinDkgSession` |
| `WebSocketManager` | `webSocketManager.ts` | Signal-server client, `maybeTriggerCeremony`, relay |
| `OffscreenManager` | `offscreenManager.ts` | Create / tear down offscreen document |
| `RpcHandler` | `rpcHandler.ts` | dApp EIP-1193 entry point |
| `KeepaliveController` | (in `index.ts` area) | Pings offscreen during active DKG/signing to prevent MV3 idle-death |

Dispatch table: `case MESSAGE_TYPES.*` blocks in
`src/entrypoints/background/messageHandlers.ts`. See the API Reference
section for the real message types.

#### Offscreen document (`src/entrypoints/offscreen/`)

Long-lived WebRTC + WASM host. Real class: `WebRTCManager`
(`webrtc.ts`), which holds all peer connections plus FROST state
(`frostDkg`, `signingInfo`, `signingCommitments` Map, `signingShares` Map).
WASM entry point: `loadKeystoreForSigning`, `initiateSigningCeremony`,
`_handleSigningCommitment`, `_handleSignatureShare`,
`_aggregateSignatureAndBroadcast`. See CLAUDE.md for the signing
pipeline end-to-end.

#### Content script + injected provider (`src/entrypoints/content/`)

Standard EIP-1193 pattern: the content script injects a
`window.ethereum` object into page context; RPC calls
(`eth_requestAccounts`, `eth_sendTransaction`, `personal_sign`, etc.)
cross into the content-script world and then into the background
service worker via `chrome.runtime.sendMessage`.

### Terminal UI

`apps/tui-node/` is structured around the Elm architecture in
`src/elm/` (Model/Update/View via tui-realm) plus the longer-lived
`src/core/` managers that are shared with native-node. Key modules:

| Path | What it holds |
|---|---|
| `src/elm/app.rs` | `ElmApp<C>` entry struct + main loop |
| `src/elm/model.rs` | `Model` — the single source of UI state |
| `src/elm/update.rs` | Update fn mapping `Message` → state transition + `Command` emissions |
| `src/elm/command.rs` | `Command<C>` enum — side effects to execute |
| `src/elm/components/` | Per-screen tui-realm `Component` impls |
| `src/elm/provider.rs` | `UIProvider` trait (abstract UI backend) |
| `src/core/*` | `*Manager` types — business logic reused by native-node |
| `src/protocal/` | Wire types (`signal.rs`, `dkg.rs`, `signing.rs`, `session_types.rs`) |
| `src/keystore/` | Encrypted share persistence |
| `src/offline/` | SD-card air-gap mode |

`UIProvider` is a TRAIT, not a struct:

```rust
// src/elm/provider.rs
pub trait UIProvider: Send + Sync { /* methods */ }
```

TUI and native-node implement this interface differently —
TUI drives it through tui-realm; native-node implements
`NativeUICallback` (`apps/native-node/src/ui_callback.rs`) to bridge
onto the Slint event loop.

### Native Desktop

`apps/native-node/` re-uses `tui_node::core::*Manager` types as the
business-logic backend and presents them through a Slint UI. Entry
points:

| Path | Role |
|---|---|
| `src/main.rs` | Tokio runtime + Slint event loop startup |
| `src/core_adapter.rs` | Bridges `CoreState` ↔ Slint AppState globals |
| `src/ui_callback.rs` | `NativeUICallback` — posts UI updates onto the Slint loop via `Weak<MainWindow>` + `slint::invoke_from_event_loop` |
| `ui/main_enhanced.slint` | Actual Slint UI compiled via `build.rs` |

The Send-bridge pattern for Slint's `!Send` `MainWindow` is described
in CLAUDE.md's "Native desktop node" section — future Slint bumps
should consult that for the gotcha list.

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

Full deployment guide lives in [`docs/deployment/README.md`](deployment/README.md)
— this section is a summary. For Cloudflare-Worker-specific steps see
[`docs/deployment/CLOUDFLARE_DEPLOYMENT.md`](deployment/CLOUDFLARE_DEPLOYMENT.md).

### What ships to production

- **Cloudflare Worker signal server**: canonical production path.
  `wrangler deploy` from `apps/signal-server/cloudflare-worker/`.
- **Self-hosted signal server**: `cargo build --release -p webrtc-signal-server`
  → systemd service behind an HTTPS terminator. Binds `0.0.0.0:9000`;
  reads zero env vars; stateless. No Dockerfile or docker-compose
  ships in-tree today.
- **Browser extension**: `bun run build:chrome` / `build:firefox`
  → web-store distribution.
- **TUI / native apps**: `cargo build --release` → single static
  binary. No platform installers (`.msi` / `.dmg` / `.AppImage`)
  ship today — earlier drafts of this section referenced WiX /
  create-dmg / linuxdeploy scaffolding that does not exist in the
  repo (verified: zero `.wxs` / `AppDir` / `create-dmg` references
  anywhere). Users build and run the raw binary.

### Infrastructure sizing

The standalone signal server is stateless and lightweight —
memory scales with active WebSocket count, not historic session
volume. Real-world sizing depends on concurrent-peer load and has
not been benchmarked; start small and scale vertically by
kernel-tuning (see `docs/deployment/README.md` § Operator notes
for the sysctl knobs). The public Cloudflare Worker deployment
offloads capacity planning entirely to Cloudflare's edge runtime.

### Monitoring and Observability

#### Metrics Collection

No Prometheus integration ships today — `prometheus` does not appear
in any workspace `Cargo.toml`, and the signal server exposes no
`/metrics` endpoint. Operators run off structured logs. Adding a
`/metrics` route on the self-hosted signal server (and matching
tracing-prometheus-bridge counters in the clients) is open future
work.

#### Logging Strategy

`tracing` / `tracing-subscriber` are the real logging stack. The
browser extension, TUI, and signal server all emit structured
`info!`/`debug!` events. Filter via the `RUST_LOG` env var:

```bash
RUST_LOG=tui_node=debug,webrtc=info mpc-wallet-tui --device-id alice
```

Most ceremony-relevant logs are at `info`; verbose mesh / FROST
internals are at `debug` or `trace`.

#### Health Checks

The self-hosted signal server has no `/health` route handler
(verified: `apps/signal-server/server/src/` has no route for it).
Liveness is inferred from whether a TCP/WebSocket upgrade succeeds:

```bash
wscat -c ws://localhost:9000/
# or, for the public Cloudflare Worker deployment:
wscat -c wss://xiongchenyu.dpdns.org/
```

Adding an HTTP `/health` endpoint that returns
`{status, version, uptime, active_connections}` is trivial in the
standalone server but not in the Worker variant (which exists as
stateless WS upgrade handling, no HTTP response scaffolding).

---

## API Reference

### Browser Extension: internal message types

Internal `chrome.runtime.sendMessage` types are consts in
`MESSAGE_TYPES` (see the actual enum in the extension source — grep
for `case MESSAGE_TYPES.` in `src/entrypoints/background/messageHandlers.ts`
for the full dispatch table). Key ones:

| Type | Purpose |
|---|---|
| `CREATE_DKG_WALLET` | Popup → background: kick off DKG for a new wallet |
| `JOIN_DKG_SESSION` | Popup → background: join a peer's announced DKG |
| `SAVE_DKG_WALLET` | Offscreen → background: persist the completed key share after DKG |
| `CREATE_SIGNING_SESSION` | Popup/dApp → background: start a signing ceremony |
| `ACCEPT_SESSION` / `DECLINE_SIGNING_SESSION` | Popup → background: join or refuse a signing session |
| `REQUEST_SIGNING` | Popup → background: user-initiated sign-message flow |
| `UNLOCK_KEYSTORE` | Popup → background: decrypt a wallet share with the password |
| `GET_STATE` / `GET_WEBRTC_STATE` | Popup → background: read app state for render |
| `LIST_DEVICES` | Popup → background: known peers on the signal server |
| `RELAY` | Background: forward a WebSocket relay payload |
| `FROM_OFFSCREEN` / `OFFSCREEN_READY` | Wrappers for background↔offscreen cross-context messages |

dApp-facing calls arrive as EIP-1193 RPC through the injected content
script (`window.ethereum.request(...)`), NOT as `sendMessage` types.
See `CLAUDE.md` § "Browser extension: threshold signing architecture"
for the end-to-end flow from `personal_sign` to aggregate signature.

### Terminal UI CLI Arguments

Authoritative definitions in `apps/tui-node/src/bin/mpc-wallet-tui.rs`:

```
mpc-wallet-tui [OPTIONS]

OPTIONS:
    --device-id <ID>           FROST participant identity.
                               Defaults to the machine hostname.
    --signal-server <URL>      WebSocket signal server URL.
                               Default: wss://xiongchenyu.dpdns.org
    --offline                  Run in offline (SD-card air-gap) mode.
    --log-location <PATH>      Log file path.
                               Default: ~/.frost_keystore/logs/mpc-wallet.log
    --log-level <LEVEL>        error | warn | info | debug | trace
                               Default: info
```

There is no `--config`, `--keystore`, or `--help`-queryable sub-command
arg — earlier drafts of this doc listed those. The keystore directory
is fixed at `~/.frost_keystore/` (see the Core Components section).

The TUI itself is a keyboard-driven Ratatui app, not a REPL with typed
commands. Keybindings are hardcoded in `src/elm/update.rs` and the
per-screen components; see `apps/tui-node/docs/KEYBOARD_NAVIGATION_GUIDE.md`.

### WebSocket signal protocol

Shape-compatible with the TUI's wire format. Top-level serde tag is
`type` (`snake_case`), each envelope has peer addressing + session
context as inline fields. Authoritative: `apps/tui-node/src/protocal/signal.rs`
and the TypeScript mirror in `packages/@mpc-wallet/types/src/session.ts`.

Message types actually used:

| Type | Direction | Purpose |
|---|---|---|
| `announce_session` | Client → server (broadcast) | Declare a new DKG or signing session |
| `session_available` | Server → clients | Server echoes an announce to every connected peer |
| `request_active_sessions` | Client → server | Cold-start replay — "what did I miss before connecting?" |
| `sessions_for_device` | Server → client | Reply to `request_active_sessions` |
| `session_status_update` | Client → server | Emitted on join — grows the participants list |
| `relay` | Client ↔ client (via server) | Opaque peer-to-peer envelope wrapping `websocket_msg_type`: WebRTCSignal (offer/answer/ICE), SessionProposal, SessionResponse, SigningDecline |

**DKG round packages and signature commitments do NOT go through the
signal server.** They ride the peer-to-peer WebRTC mesh that's
established once participants complete signaling. The signal server
sees only session announcements, relay envelopes, and ICE exchange.
Earlier drafts of this section claimed `DKG_ROUND1/2/3` and
`SIGNING_COMMITMENT/SHARE` as wire types — incorrect.

---

## Performance Characteristics

### Benchmarks

The repo does not yet ship `criterion` benches — no `benches/`
directory, no bench dev-dependencies. Earlier drafts of this doc
contained a specific DKG/signing numbers table (e.g. "DKG 3
participants: 1.2s / 15MB / 45KB") that had no reproducible source
and was removed. Contributing `criterion` benches + a reproducible
methodology is open work; until those exist, the authoritative
functional coverage is `cargo test --workspace` (≈170 tests) +
`bun test` in the extension (≈500 tests).

### Optimization present in code today

- **Adaptive event loop** (`apps/tui-node/src/elm/adaptive_event_loop.rs`):
  poll interval ramps 5ms→200ms based on UI activity to keep idle
  CPU below 1%.
- **Bounded channels** (`apps/tui-node/src/elm/channel_config.rs`):
  tokio mpsc channels use explicit capacity limits to prevent memory
  growth from queue buildup.
- **Deterministic session derivation** (`src/protocal/session_handler.rs`):
  session-id is a pure hash of wallet name, so re-generating the
  same wallet produces the same group key without re-running DKG.

There is **no** connection pool, message batcher, or state cache in
the code today — earlier drafts of this section sketched these as
aspirational patterns (`ConnectionPool`, `MessageBatcher`,
`StateCache`). They were removed because they described Rust types
that do not exist in the source.

### Scalability Considerations

The real cohort-size bottleneck is the WebRTC full-mesh degree
(n·(n-1)/2 peer connections), not the cryptography. FROST itself is
generic over `t`/`n`. No hard participant cap is enforced in code,
but production use has only been exercised at small cohorts (2-of-3,
3-of-5).

- **Signal server**: stateless + session-memory-only, so
  horizontally scaling it is a matter of running multiple instances
  behind a load balancer. The Cloudflare Worker variant does this
  automatically. There is no shared state store (no Redis) to
  coordinate — if an operator wants multi-instance with session
  sharing, the state-store layer would need to be added.
- **STUN/TURN**: clients rely on public STUN only. No TURN infra
  ships with this repo; symmetric-NAT peers may fail to connect.

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

No third-party security audit has been performed on this codebase.
Earlier drafts of this appendix listed a "2024-Q4 Internal: FROST
implementation: Passed" line that had no corresponding audit report
in the repo — fabricated and removed. Report vulnerabilities via
[GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new).

The FROST protocol implementation itself comes from the
[ZCash Foundation's audited `frost-*` crates](https://github.com/ZcashFoundation/frost)
(v2.2); this workspace's usage of those crates is NOT separately
audited.

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