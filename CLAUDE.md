# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Rust (all workspace crates)
cargo build                              # Build all workspace members
cargo test                               # Run all Rust tests
cargo test -p mpc-wallet-frost-core      # Test specific package
cargo test -p tui-node                   # Test TUI node
cargo test test_name                     # Run single test by name
cargo run --example unified_dkg -p mpc-wallet-frost-core  # Run example
cargo run --bin mpc-wallet-tui -p tui-node                # Run TUI app
cargo check                              # Fast type check without codegen

# Browser extension (Bun + WASM)
bun install                              # Install JS dependencies
bun run build:wasm                       # Build WASM bindings (required first)
bun run dev                              # Dev server with hot reload
bun run build                            # Production build
bun test                                 # Run JS/TS tests
bun test path/to/test.ts                 # Run single test file
```

## Architecture

Rust monorepo (edition 2024) with a Bun-managed browser extension. Seven Cargo workspace members:

### Core Library: `packages/@mpc-wallet/frost-core/`
Shared FROST cryptographic implementation used by all Rust targets. Key modules:
- `unified_dkg.rs` — Runs FROST DKG for ed25519 + secp256k1 simultaneously from a single root secret
- `hd_derivation.rs` — BIP-44 style HD key derivation using additive scalar offsets (no extra DKG rounds)
- `traits.rs` — `FrostCurve` trait abstracting over curve operations
- `ed25519.rs` / `secp256k1.rs` — Curve implementations (Solana addresses, Ethereum addresses)
- `keystore.rs` — Encrypted key share storage (PBKDF2 + AES-256-GCM)
- `root_secret.rs` — Root entropy → deterministic per-curve RNGs via HKDF

### Applications
- **`apps/tui-node/`** — Terminal UI (Ratatui) with Elm architecture (`src/elm/` for Model/Update/View). Exposes `lib.rs` so native-node can reuse business logic. Supports online (WebRTC mesh) and offline (SD card air-gap) DKG modes.
- **`apps/native-node/`** — Desktop GUI (Slint 1.x) reusing `tui-node::core::{*Manager, CoreState}` for business logic. Native UI events are bridged through the `UICallback` trait (impl: `NativeUICallback` in `src/ui_callback.rs`) which posts closures onto the Slint event loop via `Weak<MainWindow>` + `slint::invoke_from_event_loop` (closures must be `Send`; `MainWindow` is `!Send`). File dialogs use the `rfd` crate. See `apps/native-node/README.md` for feature-parity status.
- **`apps/browser-extension/`** — Chrome/Firefox extension (WXT + Svelte 5). Manifest V3 with background worker, popup, offscreen document (WebRTC + WASM), content script.
- **`apps/signal-server/`** — WebRTC signaling: standard WebSocket server + Cloudflare Worker variant

### WASM & Blockchain
- **`packages/@mpc-wallet/core-wasm/`** — Thin `wasm-bindgen` wrapper around frost-core
- **`packages/@mpc-wallet/blockchain/`** — Multi-chain support (solana-sdk, ethers, bitcoin crate)

## Key Patterns

**FROST ciphersuite type names**: `frost_ed25519::Ed25519Sha512` and `frost_secp256k1::Secp256K1Sha256` (note capital K in Secp256K1).

**frost-core internal types**: `SigningShare::new()`, `VerifyingShare::new()`, `VerifyingKey::new()` are `pub(crate)`. To construct these from outside frost-core, use `serialize()` / `deserialize()` round-trips through `Field::serialize`/`Group::serialize`.

**UIProvider trait** (`apps/tui-node/src/elm/provider.rs`): Abstracts the TUI's Elm app loop over a UI backend. Separate from `UICallback` (see below).

**UICallback trait** (`apps/tui-node/src/core/mod.rs`): Event-push surface for the non-Elm managers in `tui-node::core`. Both TUI and native-node consume `core::*Manager` types — TUI goes through the Elm loop, native-node implements `UICallback` directly (`NativeUICallback` in `apps/native-node/src/ui_callback.rs`) to push state onto Slint globals.

**Elm architecture** in TUI: State is `Model`, transitions via `Update`, rendering via `View`. Event-driven through `InternalCommand<C>` enum.

## Browser extension: threshold signing architecture

The extension is a standalone MPC client with TUI wire-protocol parity — any combination of extensions and TUI nodes can run DKG, threshold signing, and dApp `personal_sign`. Four runtime contexts (MV3):

1. **Popup** (`src/entrypoints/popup/App.svelte`) — Svelte 5 legacy reactivity (NOT runes). Lives only while the browser-action panel is open. Talks to background via `chrome.runtime.connect({name: "popup"})`.
2. **Background SW** (`src/entrypoints/background/`) — Orchestrates. Owns `StateManager`, `SessionManager`, `WebSocketManager` (signal server), `OffscreenManager`. MV3 service workers terminate after ~30s idle; `KeepaliveController` pings during active DKG/signing states to prevent death.
3. **Offscreen** (`src/entrypoints/offscreen/`) — Long-lived WebRTC + WASM host. Loads `@mpc-wallet/core-wasm` (FROST); holds `WebRTCManager` with all FROST state (`frostDkg`, `signingInfo`, `signingCommitments` Map, `signingShares` Map). Background↔offscreen communicate via `chrome.runtime.sendMessage` wrapped in `{type: "fromBackground"|"fromOffscreen", payload}`.
4. **Content + injected** — Injects an EIP-1193 provider into page context. `window.ethereum.personal_sign` → content script → `background.rpcHandler.handleSignMessageRequest`.

### Signing pipeline (end-to-end)

Each arrow crosses a runtime boundary unless noted:

```
dApp .personal_sign OR popup "Sign Message"
   → background.rpcHandler / background.sessionManager
   → sessionManager.createSigningSession builds SessionInfo
   → wsClient.announceSession broadcasts on signal server
   → signal server → session_available to ALL connected peers
   → co-signer extensions: webSocketManager handles session_available
      → signingNotifier.maybeNotify fires chrome.notifications
      → popup "sessionAvailable" broadcast triggers auto-modal (Ext-3b)
   → user clicks Join → joinDkgSession → wsClient.sendSessionStatusUpdate
   → server re-broadcasts session_available with grown participants list
   → webSocketManager.maybeTriggerCeremony sees participants.length >= threshold
      → sessionReadyForSigning event sent to offscreen
   → offscreen: loadKeystoreForSigning + initiateSigningCeremony
      → frostDkg.signing_commit() → broadcast SigningCommitment over WebRTC mesh
   → each peer: _handleSigningCommitment → frostDkg.add_signing_commitment
   → threshold commitments received → _generateAndSendSignatureShare
      → frostDkg.sign(messageHex) → broadcast SignatureShare
   → each peer: _handleSignatureShare → frostDkg.add_signature_share
   → threshold shares received → _aggregateSignatureAndBroadcast (aggregator only)
      → frostDkg.aggregate_signature(messageHex) → broadcast AggregatedSignature
   → all peers: onSigningComplete callback fires
   → offscreen → background "signingComplete" event
   → stateManager stashes appState.lastSignature + resolves RpcHandler pending
      promise (for dApp flow) + broadcasts signingCompleted to popup
   → popup renders SignatureComplete banner (Ext-2e)
```

### Wire protocol (extension ↔ signal server)

Shape-compatible with TUI (see TUI's `command.rs`). Top-level serde tag `type`, `snake_case`.

- `announce_session` / `session_available` — session-discovery broadcasts. Flat `session_type: "dkg" | "signing"` string; signing sessions carry top-level `wallet_name`, `group_public_key`, `blockchain`, `signing_message_hex` siblings. See `packages/@mpc-wallet/types/src/session.ts`. Parser in `src/utils/session-parse.ts` synthesizes `accepted_devices: []` (TUI omits it) so downstream can always index.
- `request_active_sessions` / `sessions_for_device` — cold-start replay. Extension fires `requestActiveSessions()` 2s after WS open so sessions announced before our connect aren't missed.
- `session_status_update` — outbound only; emitted on join.
- `relay` (generic peer-to-peer, wraps `websocket_msg_type`) — used for WebRTCSignal, SessionProposal, SessionResponse, and `SigningDecline` (Ext-3c, explicit rejection without joining the mesh).

### MV3 gotchas (have bitten us)

- **Session-ephemeral state**: `pendingKeystoreJson`, `sessionInfo`, `dkgState` MUST reset on SW wake (`StateManager.loadPersistedState`). A stale `pending_sign_*` state causes DKG password prompt to misroute to UnlockWallet (fixed in 615da01).
- **Offscreen idle termination**: offscreen document idles out ~30s. `KeepaliveController` (background) pings it every 25s during Initializing/Round1/Round2/Finalizing DKG states. Wire via `stateManager.addDkgStateListener`.
- **chrome.action.openPopup**: Chromium-only; fallback to `chrome.tabs.create({url: popup.html})` for Firefox.
- **navigator.clipboard** works in popup context but NOT in background SW or offscreen.

### Key entry points for signing work

| Layer | File | Key methods |
|---|---|---|
| Popup UI | `entrypoints/popup/App.svelte` | `buildSignPreview`, `confirmSignPreview`, `reviewSigningInvite`, `declineSigningInvite` |
| Background RPC | `entrypoints/background/rpcHandler.ts` | `handleSignMessageRequest`, `approveDappSignature`, `handleSignatureComplete` |
| Background session | `entrypoints/background/sessionManager.ts` | `createSigningSession`, `joinDkgSession` |
| Background trigger | `entrypoints/background/webSocketManager.ts` | `maybeTriggerCeremony`, `relayToPeer` |
| Background state | `entrypoints/background/stateManager.ts` | `case "signingComplete"`, `case "signingProgress"` |
| Offscreen | `entrypoints/offscreen/webrtc.ts` | `loadKeystoreForSigning`, `initiateSigningCeremony`, `_handleSigningCommitment`, `_handleSignatureShare`, `_aggregateSignatureAndBroadcast` |

WASM FROST methods actually called for signing: `signing_commit()` (returns hex), `add_signing_commitment(idx, hex)`, `sign(msgHex)` (returns hex), `add_signature_share(idx, hex)`, `aggregate_signature(msgHex)` (returns hex). Participant indices are 1-based; compute as `participants.indexOf(peerId) + 1`. Both `signing_commit()` and `sign()` auto-register the local side of their output (our commitment, then our share) into the WASM instance's internal maps — do NOT call `add_signing_commitment` / `add_signature_share` for our own index, those are peer-only. This keeps the contract uniform across every `add_*` method (peer-only) while satisfying frost-core's requirement that the signer's own commitment + share appear in the signing_package / aggregate input.

DKG is analogous: `generate_round1()` returns our round-1 package as hex; `add_round1_package(idx, hex)` is called for peer packages only. `can_start_round2()` returns true once all n-1 peer packages are ingested (matches frost-core's `dkg::part2` contract which wants exactly n-1). Same for round 2.

### Testing

Signal-server live smoke tests need 3 browser instances — no bun harness exercises full FROST+WebRTC pairing. Unit coverage via `tests/entrypoints/background/` (regression suites: `dkgAutoTrigger`, `signingAutoTrigger`, `signingNotification`, `dappSignatureApproval`, `signingDecline`).

## Native desktop node: Slint + tui-node::core

`apps/native-node/` is a third MPC client that reuses `tui-node::core::{WalletManager, SessionManager, DkgManager, OfflineManager, ConnectionManager, SigningManager, CoreState}` as its business-logic backend. Architecture:

```
ui/*.slint (Slint 1.x)  <---callbacks--->  src/main.rs  <--->  src/core_adapter.rs  --->  tui_node::core::*Manager
     |                                                              |
     | AppState globals                                              | UICallback events
     |                                                               ↓
     |<-------------------- NativeUICallback (src/ui_callback.rs)   |
                          dispatches via Weak<MainWindow> +
                          slint::invoke_from_event_loop
                          (closures must be Send;
                           MainWindow is !Send → upgrade INSIDE closure)
```

**Send bridge pattern**. `MainWindow` contains `Cell<u32>` / `UnsafeCell<...>` and is therefore `!Send`. `invoke_from_event_loop` requires `Send + 'static` closures, so `NativeUICallback` holds `Weak<MainWindow>` (which IS Send), clones it into each callback closure, and upgrades inside. Pre-compute all values (e.g. `Vec<SlintWalletInfo>`) before the closure so only owned data crosses the Send boundary.

**File dialogs**: `rfd` crate. Always spawn via `tokio::task::spawn_blocking(|| rfd::FileDialog::new()...)` — the dialog thread blocks; running it on the main tokio executor freezes the Slint event loop.

**Slint 1.x gotchas** (all fixed during the rehabilitation pass, documented so future Slint bumps have a cheat-sheet):
- `vertical-alignment` is only valid on `Text` — remove from `Rectangle` / `VerticalBox` / `HorizontalBox`.
- `linear-gradient(...)` requires `@` prefix; stops take explicit percentages.
- Strings have no `.length` / `.substring` / `.slice` — compute on the Rust side and push through struct properties.
- `Rectangle` doesn't accept `padding` or `margin-*` — move to an inner layout element.
- `Image.rotation-angle` removed (renamed `rotation`); `animate { loop: true }` unsupported.
- `GridBox { Row { ... } }` doesn't accept `if`/`for` children — use `HorizontalBox`.

**Feature-parity status** (see `apps/native-node/README.md` for the matrix): DKG + sessions + WebRTC + keystore import/export with password + signing UX (approve/reject modal) + SD-card export/import/clear all wired end-to-end. The single remaining gap: `SigningManager::approve` + SD-card artefact emission both delegate to stubs because `tui_node::protocal::{signing, dkg}` operates on `AppState<C>` via the Elm `Message` loop. Extracting a ciphersuite-generic backend shared between the Elm loop and `core::*Manager` is the one-PR remaining for full parity — UI surface + CoreAdapter wiring are all in place.

## Dependencies

FROST: `frost-core` 2.2.0, `frost-ed25519` 2.2.0, `frost-secp256k1` 2.2.0 (ZCash implementations).
Crypto: `sha2`, `sha3`, `hmac`, `hkdf`, `aes-gcm`, `argon2`, `k256`, `ed25519-dalek`.
Dev environment: Nix flake (`nix develop`) provides all system deps including graphics libs.

## Workspace Layout

```
Cargo.toml              # Workspace root, resolver = "2"
package.json            # Bun monorepo (browser extension)
flake.nix               # Nix dev environment (Linux + macOS)
apps/
  tui-node/             # Rust binary + library
  native-node/          # Rust binary (Slint GUI)
  signal-server/        # server/ + cloudflare-worker/
  browser-extension/    # WXT + Svelte + TailwindCSS
packages/@mpc-wallet/
  frost-core/           # Core crypto library
  core-wasm/            # WASM bindings
  blockchain/           # Chain integrations
```
