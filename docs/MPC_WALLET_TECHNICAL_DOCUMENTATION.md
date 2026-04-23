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

The MPC (Multi-Party Computation) Wallet is a distributed cryptographic wallet system that enables secure key generation and transaction signing across multiple parties without any single party having access to the complete private key. Built on the FROST (Flexible Round-Optimized Schnorr Threshold) signature scheme via the ZCash Foundation's `frost-core 2.2` crates, the system splits signing authority across `t`-of-`n` participants — compromise of fewer than `t` key shares cannot produce a signature.

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

The MPC Wallet follows a **distributed, peer-to-peer architecture**:

1. **No single participant holds a complete key**: shares are distributed
   via FROST DKG; only a valid `t`-of-`n` subset can sign.
2. **Peer-to-peer ceremony**: DKG and signing run over WebRTC data
   channels; the signal server is a stateless relay, blind to payload
   content once the mesh is up.
3. **Multiple curves**: concrete support for secp256k1 (Ethereum) and
   ed25519 (Solana). The code is not curve-agnostic beyond those two
   — the `FrostCurve` trait abstracts over them but every address
   derivation, encoding, and chain-integration path is written
   per-curve. ("Protocol agnostic" has been removed from the earlier
   draft because each new chain needs meaningful integration work.)
4. **Modular repo layout**: shared `frost-core` crate + three UI
   frontends (TUI, native, extension) that reuse it.

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
├── apps/
│   ├── browser-extension/          # WXT + Svelte 5, MV3
│   │   ├── src/entrypoints/        # background / popup / offscreen / content
│   │   ├── src/components/         # Svelte components
│   │   ├── src/services/           # AccountService, KeystoreService, etc.
│   │   ├── tests/                  # Bun test suite
│   │   └── wxt.config.ts
│   │
│   ├── native-node/                # Slint 1.x desktop GUI
│   │   ├── src/main.rs             # entry (tokio + Slint event loop)
│   │   ├── src/core_adapter.rs     # bridges CoreState ↔ Slint AppState globals
│   │   ├── src/ui_callback.rs      # NativeUICallback (Send-bridge onto Slint loop)
│   │   └── ui/main_enhanced.slint  # Slint UI, compiled by build.rs
│   │
│   ├── tui-node/                   # Ratatui Elm-architecture TUI
│   │   ├── src/bin/                # mpc-wallet-tui binary entry
│   │   ├── src/elm/                # Model / Update / View / Command,
│   │   │                           # per-screen components, and the
│   │   │                           # real runtime WebRTC driver at
│   │   │                           # src/elm/webrtc_signaling.rs
│   │   ├── src/core/               # *Manager types (reused by native-node)
│   │   ├── src/protocal/           # Wire types (signal.rs / dkg.rs / signing.rs)
│   │   ├── src/keystore/           # Encrypted share persistence
│   │   ├── src/webrtc/             # Mesh TEST HARNESS — not wired
│   │   │                           # into the Elm runtime; consumed
│   │   │                           # by examples/webrtc_mesh_e2e_test.rs
│   │   ├── src/network/            # Low-level WebSocket + webrtc helpers
│   │   │                           # (src/network/webrtc.rs is the other
│   │   │                           # production RTCPeerConnection site)
│   │   ├── src/offline/            # SD-card air-gap mode
│   │   ├── src/hybrid/             # Online+offline mixed-participant mode
│   │   └── src/utils/
│   │
│   └── signal-server/
│       ├── server/                 # Standalone tokio + tokio-tungstenite
│       └── cloudflare-worker/      # Rust-over-WASM via `worker` crate
│
├── packages/@mpc-wallet/
│   ├── frost-core/                 # FROST wrapper: unified_dkg, hd_derivation,
│   │                               # traits, ed25519, secp256k1, keystore, root_secret
│   ├── core-wasm/                  # wasm-bindgen wrapper around frost-core
│   ├── blockchain/                 # Ethereum/Solana/Bitcoin encoding
│   └── types/                      # Shared TypeScript types (Bun workspace only)
│
├── scripts/
├── docs/
├── Cargo.toml                      # Rust workspace (edition 2024, resolver 2)
└── package.json                    # Bun workspace root
```

Note: the directory name `src/protocal/` is intentionally misspelled
in-tree; rename would be a broad refactor and is not blocking.

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

Real main menu comes from `src/elm/components/main_menu.rs:55-114`
and is rendered as a Ratatui `List` of emoji-prefixed entries. Items
vary with `wallet_count`:

- **Always**: `🆕 Create New Wallet`, `🔗 Join Session`, `⚙️ Settings`,
  `🚪 Exit`
- **Added once `wallet_count > 0`**: `💼 Manage Wallets` (plus DKG-
  progress and signing flows which live inside sub-screens, not as
  top-level menu items).

Earlier drafts printed a numbered-hotkey layout
(`[1] Wallet / [2] DKG / [3] Sign / [4] Session / [5] Network /
[6] Settings / [Q] Quit`) with a right-hand pane showing
`Current Wallet: mpc_wallet_01 / Address: 0x742d35Cc6634C053... /
Balance: 1.234 ETH / Connected Peers: 2/3 / Session Status: Active`.
None of that is real:

- Menu navigation is arrow-key-driven; there are no number hotkeys.
- No wallet-summary side pane exists. Wallet details live inside
  the `WalletDetail` component (`src/elm/components/wallet_detail.rs`),
  reachable through Manage Wallets → pick wallet.
- The TUI does NOT query on-chain balances — the displayed
  `1.234 ETH` figure was fabricated, not a live or placeholder
  feed.
- There is no dedicated `Connected Peers` header bar; per-session
  peer status surfaces inside the DKG-progress / signing screens.

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

**Round 3: Local finalize** (no wire traffic)
```
Each participant Pi:
1. Verify received shares using commitments
2. Compute final share xi = Σ(fji)
3. Run frost-core::dkg::part3 to produce KeyPackage + VerifyingKey
```

Round 3 is local — there is NO broadcast step. Each participant
derives the shared `VerifyingKey` (group public key) independently
from the accumulated round1 + round2 packages; if participants
agree on those, they agree on the VerifyingKey. Earlier drafts of
this section showed a "Broadcast yi for group public key
computation" step — that's a Feldman-VSS notation that doesn't
apply to the FROST pipeline as implemented by the ZCash crates.

### Key Storage and Security

#### Keystore Format

Real on-disk layout is **one JSON file per wallet** at
`~/.frost_keystore/<device_id>/<curve>/<wallet_id>.json` —
there is NO wrapping `{ "wallets": [...] }` container. The
serialized shape is the `WalletFile` struct
(`apps/tui-node/src/keystore/models.rs:438-453`):

```json
{
  "version": "2.0",
  "encrypted": true,
  "algorithm": "AES-256-GCM-Argon2id",
  "data": "<base64 ciphertext of the FROST key-share blob>",
  "metadata": {
    "wallet_id": "...",
    "curve_type": "secp256k1",
    "threshold": 2,
    "total_participants": 3,
    "group_public_key": "hex",
    "created_at": <unix-timestamp-u64>,
    "devices": [ /* DeviceInfo list */ ],
    "blockchains": [ /* BlockchainInfo list */ ]
  }
}
```

Earlier drafts of this section showed a `{ "wallets": [{ id,
name, blockchain, public_key, address, key_shares: { encrypted,
algorithm, data }, metadata: { created_at, last_used } }] }`
shape. That structure never shipped — there's no wallet array,
no `last_used` timestamp, no flat `blockchain/address`
properties (the real `metadata` nests blockchain addresses in
a `blockchains` Vec, and derives them on demand from
`group_public_key` + `curve_type`). Same finding as f4fc866 for
the broader keystore-layout retraction.

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
| `src/elm/command.rs` | `Command` enum — side effects to execute (non-generic; the concrete ciphersuite is threaded through `AppState<C>` which `Command::execute` takes by reference) |
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

Two deployable variants — standalone tokio server + Cloudflare Worker —
share wire-type definitions in
`apps/signal-server/server/src/lib.rs`.

#### Wire-protocol envelope types

```rust
// Client → Server
pub enum ClientMsg {
    Register { device_id: String },
    ListDevices,
    Relay { to: String, data: serde_json::Value },
    AnnounceSession { session_info: serde_json::Value },
    RequestActiveSessions,
    SessionStatusUpdate { session_info: serde_json::Value },
    QueryMyActiveSessions,
}

// Server → Clients
pub enum ServerMsg {
    Devices { devices: Vec<String> },
    Relay { from: String, data: serde_json::Value },
    Error { error: String },
    SessionAvailable { session_info: serde_json::Value },
    SessionListRequest { from: String },
    SessionsForDevice { sessions: Vec<serde_json::Value> },
    SessionRemoved { session_id: String, reason: String },
}
```

Serde tag is `type` + `snake_case`. Session state is held in-process
(`session_manager.rs`) — no database, no Redis. Sessions disappear
from the registry when their creator disconnects.

#### Standalone Rust server (`apps/signal-server/server/`)

Tokio + `tokio-tungstenite`. Binds `0.0.0.0:9000`, no env vars, no
HTTP health endpoint. Run via `cargo run -p webrtc-signal-server`.

#### Cloudflare Worker variant (`apps/signal-server/cloudflare-worker/`)

Rust-over-WASM via the `worker` crate, not TypeScript. The Worker
receives the WebSocket upgrade, routes messages through the same
`ClientMsg` / `ServerMsg` enum types, and leverages Cloudflare
Durable Objects for session state. See
[`docs/deployment/CLOUDFLARE_DEPLOYMENT.md`](deployment/CLOUDFLARE_DEPLOYMENT.md)
for deployment specifics.

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

- **Key Generation**: `rand_core::OsRng` + `rand_chacha::ChaCha20Rng`
  seeded from a root entropy pool (see
  `frost-core/src/root_secret.rs`). Upstream ZCash FROST crates
  consume the RNG for commitments and shares.
- **Share Distribution**: DKG round-2 packages are exchanged over
  DTLS-encrypted WebRTC data channels (not plaintext over the signal
  server).
- **Signature Generation**: FROST threshold enforced by the `frost-*`
  crates' `aggregate` function — fewer than `t` shares cannot
  reconstruct a valid signature.
- **Verification**: Every aggregated signature verifies against the
  group public key before being returned.

#### 2. Network Security

- **TLS over WebSocket**: All signaling uses `wss://`; plain `ws://`
  is supported only for localhost testing.
- **WebRTC DTLS-SRTP**: Peer-to-peer data channels are DTLS-encrypted
  end-to-end; the signal server is blind to payload content once
  the mesh is up.
- **No application-layer HMAC**: Earlier drafts claimed "HMAC on
  all messages" + "Nonce-based replay protection" — neither exists.
  The signal server is an unauthenticated relay (DTLS-level integrity
  only); applications wanting stronger guarantees would need to
  re-sign over the wire data themselves.

#### 3. Application Security

- **Input validation**: serde envelope parsing rejects malformed
  messages at the boundary.
- **Memory zeroization**: Only `frost-core/src/root_secret.rs`
  zeros sensitive material on drop today, and it does so via a
  manual `self.0.fill(0)` inside `impl Drop for RootSecret` at
  `root_secret.rs:62-67` — NOT via the `zeroize` crate's `Zeroize`
  trait (the `zeroize` crate isn't a workspace dependency;
  `grep -rn zeroize` returns a single hit inside a code comment).
  Key shares, decrypted keystore blobs, session passwords, and the
  signing commitments / shares residing in `AppState` are **not**
  zeroed on drop today — open hardening work. Earlier drafts of
  this bullet asserted `zeroize::Zeroize` is actively used; it
  isn't.
- **Audit Logging**: There is no built-in audit log. Operational
  observability is via `tracing` / `RUST_LOG` output as described
  in the Monitoring section above. Earlier drafts claimed
  "Comprehensive activity logging" — not accurate.

### Security Assumptions

1. **Honest Majority**: At least t participants are honest
2. **Secure Channels**: TLS/DTLS provide confidentiality
3. **Random Oracle**: Hash functions behave as random oracles
4. **Discrete Log**: ECDLP is computationally hard

---

## Development Guide

### Prerequisites

- **OS**: Linux, macOS, or Windows (WSL2 for dev on Windows is fine;
  native Windows builds work via MSVC toolchain)
- **Rust**: 1.85+ (edition 2024 requirement from the workspace `Cargo.toml`)
- **Bun**: latest stable
- **System libs**: Slint's native UI needs the platform's graphics
  stack. On NixOS you can run `nix develop` for a pre-provisioned
  shell (see `flake.nix` at the repo root).

#### Tooling install

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown  # for core-wasm

# Bun
curl -fsSL https://bun.sh/install | bash
```

`wasm-pack` is a devDependency of `packages/@mpc-wallet/core-wasm`
so `bun install` at the repo root pulls it in — no separate
`cargo install wasm-pack` needed. There is no watch-script
infrastructure; earlier drafts of this section recommended
`cargo install cargo-watch`, but no watch targets exist in the
workspace.

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

# Coverage: no `cargo tarpaulin` / llvm-cov config ships today —
# earlier drafts of this doc showed a tarpaulin command that
# doesn't correspond to anything in the tree.
```

#### Integration Tests

```bash
# Browser extension tests (Bun, not npm/Vitest — see docs/testing/TESTING.md)
cd apps/browser-extension
bun test                    # full suite
bun run test:integration    # just tests/integration
bun run test:webrtc         # just tests/entrypoints/offscreen/webrtc.*
```

No automated full-mesh E2E harness exists yet — see
`docs/testing/E2E_TEST_IMPLEMENTATION_PLAN.md` for the open plan.

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
- **Browser extension**: `bun run build` (defaults to Chrome MV3) or
  `bun run build:firefox` → web-store distribution.
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

Async I/O — the whole stack is tokio-based and non-blocking. That
is the extent of deliberate optimization work in this codebase.

Earlier drafts of this section (including my 41d5ca0 commit) listed
three supposedly-real optimizations:

  - `apps/tui-node/src/elm/adaptive_event_loop.rs` — adaptive
    poll-interval ramp
  - `apps/tui-node/src/elm/channel_config.rs` — bounded mpsc channels
  - `apps/tui-node/src/protocal/session_handler.rs` — deterministic
    session derivation

**None of those files exist** (`find apps/tui-node/src -name
"adaptive_event_loop*" -o -name "channel_config*" -o -name
"session_handler*"` returns empty, and no types named
`AdaptiveEventLoop` / `ChannelConfig` / `UpdateStrategy` are
defined anywhere in the workspace). Those names were carried
forward from `docs/archive/dev-journal/PERFORMANCE_OPTIMIZATIONS.md`
— work that was planned but never landed — and I repeated the
error in 41d5ca0 while fixing unrelated fabrications elsewhere.
Corrected now.

There is also **no** connection pool, message batcher, state cache,
or `ResourceManager` in the code — earlier drafts sketched these
as aspirational Rust types (`ConnectionPool`, `MessageBatcher`,
`StateCache`) that don't exist. Removed.

Real opportunities if someone wants to take perf work on:

- Measure idle vs active CPU and add an adaptive event loop if
  warranted.
- Audit `mpsc::unbounded_channel` call sites and add bounded
  alternatives where queue-growth could matter.
- Add `criterion` benches so future optimizations have a baseline.

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
- **STUN/TURN**: the browser extension hard-codes Google's public
  STUN (`stun.l.google.com:19302`); the TUI currently passes an
  empty ICE-server list, so TUI-only peer meshes only form across
  directly-routable networks. No TURN infra ships with this repo —
  symmetric-NAT peers are unreachable in any configuration.
  Adding STUN to the TUI is a straightforward hand-edit at
  `apps/tui-node/src/network/webrtc.rs:285` (and the matching
  `src/elm/webrtc_signaling.rs:387`).

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
# Check signal-server connectivity — the server is WebSocket-only,
# so curl just verifies DNS + TLS + that something is listening on 443.
curl -v https://xiongchenyu.dpdns.org/

# Probe STUN with wscat / stuntman / anything that does a BINDING
# Request — Google's public STUN works out of the box, any
# reachability failure here means your outbound UDP path is broken.

# Enable debug logging
RUST_LOG=debug cargo run -p tui-node --bin mpc-wallet-tui
```

#### 2. WebRTC Connection Issues

**Symptom**: Peers cannot establish direct connection

**Debugging Steps**: use `chrome://webrtc-internals/` (or
`about:webrtc` in Firefox) to inspect per-connection ICE gathering,
SDP exchange, and data-channel state in real time. This surface
makes STUN/TURN traffic observable — application code can't monitor
STUN itself, since it's UDP (chrome.webRequest only sees HTTP-family
schemes, not `stun:` / `turn:` URIs).

For programmatic introspection, the standard `RTCPeerConnection`
events are:

```javascript
pc.addEventListener('icegatheringstatechange', () => {
  console.log('ICE gathering:', pc.iceGatheringState);
});
pc.addEventListener('iceconnectionstatechange', () => {
  console.log('ICE connection:', pc.iceConnectionState);
});
pc.addEventListener('connectionstatechange', () => {
  console.log('PeerConnection:', pc.connectionState);
});
```

Earlier drafts of this section listed a
`chrome.webRequest.onBeforeRequest` filter on `stun:*` / `turn:*`
URLs — that filter is invalid: `chrome.webRequest` only matches
http/https/ws/wss/file/ftp, and STUN/TURN are UDP-level.

#### 3. Signature Verification Failures

**Symptom**: Generated signatures fail verification

**Diagnostic steps**: run FROST's own `aggregate` + `verify` paths
from the upstream ZCash crates — `frost_core::aggregate` already
verifies internally against the group public key before returning
a signature. A post-aggregate verification failure therefore almost
always means:

- wrong message bytes (hash mismatch — check the signing-payload
  construction; `signing_message_hex` must be what you believed)
- wrong group public key (probably stale keystore metadata —
  inspect the `.json` sidecar in `~/.frost_keystore/<device_id>/<curve>/`)
- stale share from a previous DKG run (participants all need the
  same group key — rerun DKG if in doubt)

There are no `compute_group_public_key` / signature.r / signature.s
helpers to call manually — the earlier draft of this section
referenced a custom helper that doesn't exist.

### Debug Tools

No custom `ProtocolAnalyzer` struct ships — earlier drafts of this
section described a `ProtocolAnalyzer` counting `Round1`/`Round2`/`Round3`
messages in a trace, which is not backed by any real type in source.
The practical tools are:

- `RUST_LOG=tui_node=debug` (or a finer scope like `tui_node::protocal::dkg=trace`)
  + the session log at `~/.frost_keystore/logs/mpc-wallet.log`
- `chrome://webrtc-internals` for the browser side
- `wscat -c wss://xiongchenyu.dpdns.org/` + `Register` / `ListDevices`
  ClientMsg payloads for manual signal-server probes

#### Network Diagnostics

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
crates and as descriptive strings in the browser extension. Real
Rust types live per-domain: `KeystoreError` in
`apps/tui-node/src/keystore/mod.rs:24`, `FrostKeystoreError` in
`src/keystore/frost_keystore.rs:19`, `OfflineError` in
`src/offline/mod.rs:24`, `CoreError` in `src/core/mod.rs:21`; plus
upstream `FrostError` from `packages/@mpc-wallet/frost-core` with
`SigningError` / other variants. No top-level `src/errors.rs`
umbrella file exists.

Future work: a shared error-code registry across Rust + TypeScript
so operator-facing logs carry machine-grep-able identifiers.

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

#### WebRTC debugging

`chrome://webrtc-internals/` is the canonical inspection surface —
it exposes active peer connections, ICE candidate gathering,
SDP exchange, data-channel state, and per-stream stats. Firefox
has `about:webrtc` for the same purpose.

Earlier drafts of this section listed three speculative Chrome
launch flags (`--enable-webrtc-stun-origin`,
`--enforce-webrtc-ip-permission-check`,
`--webrtc-max-cpu-consumption-percentage=50`). Not all of these
correspond to stable Chrome switches, and none are required for
this project — left as an open "flag-level tuning" TODO for
operators who actually need it.

### E. References

1. **FROST Paper**: Komlo, C., & Goldberg, I. (2020). "FROST: Flexible Round-Optimized Schnorr Threshold Signatures"
2. **MPC Book**: Evans, D., Kolesnikov, V., & Rosulek, M. (2018). "A Pragmatic Introduction to Secure Multi-Party Computation"
3. **WebRTC Security**: Rescorla, E. (2013). "WebRTC Security Architecture"
4. **Threshold Cryptography**: Gennaro, R., & Goldfeder, S. (2018). "Fast Multiparty Threshold ECDSA"

---

## Conclusion

This repo implements a t-of-n FROST threshold wallet with three
frontends (browser extension, Slint desktop, Ratatui TUI) that share
a common `frost-core` backend and interoperate over a WebRTC mesh
established by a small signal server. It's early-stage development
software — no tagged release, no third-party security audit, no
hardware-wallet integration, no benchmarks. For the latest state see
the repository at [github.com/hecoinfo/mpc-wallet](https://github.com/hecoinfo/mpc-wallet).

Open contributions tracked in the repo rather than in this doc —
check `git log` and the `CLAUDE.md` file at the workspace root for
the live architecture notes.