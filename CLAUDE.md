# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Rust (all workspace crates)
cargo build                              # Build all workspace members
cargo test                               # Run all Rust tests
cargo test -p starlab-core            # Test specific package
cargo test -p starlab-client                   # Test TUI node
cargo test test_name                     # Run single test by name
cargo run --example unified_dkg -p starlab-core  # Run example
cargo run --bin starlab-tui -p starlab-client                # Run TUI app
cargo check                              # Fast type check without codegen

# Browser extension (Bun + WASM)
bun install                              # Install JS dependencies (from repo root)
bun run build:wasm                       # Build WASM bindings — RUN FROM REPO ROOT
                                         #   (script only exists in root package.json;
                                         #    cd apps/browser-extension && bun run build:wasm fails)
cd apps/browser-extension
bun run dev                              # Dev server with hot reload
bun run build                            # Production build
bun test                                 # Run JS/TS tests
bun test path/to/test.ts                 # Run single test file
```

## Architecture

Rust monorepo (edition 2024) with a Bun-managed browser extension. Seven Cargo workspace members:

### Core Library: `packages/@starlab/core/`
Shared FROST cryptographic implementation used by all Rust targets. Key modules:
- `unified_dkg.rs` — Runs FROST DKG for ed25519 + secp256k1 simultaneously from a single root secret
- `hd_derivation.rs` — BIP-44 style HD key derivation using additive scalar offsets (no extra DKG rounds)
- `traits.rs` — `FrostCurve` trait abstracting over curve operations
- `ed25519.rs` / `secp256k1.rs` — Curve implementations (Solana addresses, Ethereum addresses)
- `keystore.rs` — Encrypted key share storage (PBKDF2 + AES-256-GCM)
- `root_secret.rs` — Root entropy → deterministic per-curve RNGs via HKDF

### Applications
- **`apps/tui/`** — Terminal UI (Ratatui) with Elm architecture (`src/elm/` for Model/Update/View). Exposes `lib.rs` (the Elm core + `core::*Manager` + `HeadlessRunner`) so the desktop app (`stars-labs/starlab-desktop`) can reuse the business logic cross-repo. Supports online (WebRTC mesh) and offline (SD card air-gap) DKG modes.
- **`apps/browser-extension/`** — Chrome/Firefox extension (WXT + Svelte 5). Manifest V3 with background worker, popup, offscreen document (WebRTC + WASM), content script.
- **`apps/signal-server/`** — WebRTC signaling: standard WebSocket server + Cloudflare Worker variant

### WASM & Blockchain
- **`packages/@starlab/core-wasm/`** — Thin `wasm-bindgen` wrapper around frost-core
- **`packages/@starlab/blockchain/`** — Multi-chain support. Only `solana-sdk` is pulled in directly; `bitcoin.rs` and `ethereum.rs` are hand-rolled over `sha2` / `sha3` / `bs58` primitives (ethers/bitcoin crate deps were removed when their dependent examples were disabled — see the Cargo.toml comment at line 27)

## Key Patterns

**FROST ciphersuite type names**: `frost_ed25519::Ed25519Sha512` and `frost_secp256k1::Secp256K1Sha256` (note capital K in Secp256K1).

**frost-core internal types**: `SigningShare::new()`, `VerifyingShare::new()`, `VerifyingKey::new()` are `pub(crate)`. To construct these from outside frost-core, use `serialize()` / `deserialize()` round-trips through `Field::serialize`/`Group::serialize`.

**UIProvider trait** (`apps/tui/src/elm/provider.rs`): Abstracts the TUI's Elm app loop over a UI backend. Separate from `UICallback` (see below).

**UICallback trait** (`apps/tui/src/core/mod.rs`): Event-push surface for the non-Elm managers in `starlab-client::core`. The TUI goes through the Elm loop; the desktop app (`starlab-desktop`, cross-repo) implements `UICallback` directly to push `UiEvent`s into an mpsc channel that its Iced `Subscription` turns into messages. Keep this trait + the `core::*Manager` types `pub` for that cross-repo consumer.

**Elm architecture** in TUI: State is `Model`, transitions via `Update`, rendering via `View`. Event-driven through `InternalCommand<C>` enum.

## Browser extension: threshold signing architecture

The extension is a standalone MPC client with TUI wire-protocol parity — any combination of extensions and TUI nodes can run DKG, threshold signing, and dApp `personal_sign`. Four runtime contexts (MV3):

1. **Popup** (`src/entrypoints/popup/App.svelte`) — Svelte 5 legacy reactivity (NOT runes). Lives only while the browser-action panel is open. Talks to background via `chrome.runtime.connect({name: "popup"})`.
2. **Background SW** (`src/entrypoints/background/`) — Orchestrates. Owns `StateManager`, `SessionManager`, `WebSocketManager` (signal server), `OffscreenManager`. MV3 service workers terminate after ~30s idle; `KeepaliveController` pings during active DKG/signing states to prevent death.
3. **Offscreen** (`src/entrypoints/offscreen/`) — Long-lived WebRTC + WASM host. Loads `@starlab/core-wasm` (FROST); holds `WebRTCManager` with all FROST state (`frostDkg`, `signingInfo`, `signingCommitments` Map, `signingShares` Map). Background↔offscreen communicate via `chrome.runtime.sendMessage` wrapped in `{type: "fromBackground"|"fromOffscreen", payload}`.
4. **Content + injected** — Injects an EIP-1193 provider into page context. The provider is scoped to `window.starlabEthereum` only (never `window.ethereum`, to coexist with other wallet extensions); dApps discover it via EIP-6963 `eip6963:announceProvider` events. `window.starlabEthereum.request({method: 'personal_sign', ...})` → content script → `background.rpcHandler.handleSignMessageRequest`.

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

- `announce_session` / `session_available` — session-discovery broadcasts. Flat `session_type: "dkg" | "signing"` string; signing sessions carry top-level `wallet_name`, `group_public_key`, `blockchain`, `signing_message_hex` siblings. See `packages/@starlab/types/src/session.ts`. Parser in `src/utils/session-parse.ts` synthesizes `accepted_devices: []` (TUI omits it) so downstream can always index.
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

## GUI products live in their own repos

This repo is the **headless engine + terminal clients**: the crypto packages
(`frost-core` / `core-wasm` / `blockchain` / `types`), the CLI (`starlab-cli`,
the conformance oracle + headless automation), the TUI (`starlab-client`), and the
signal server. The consumer GUI products are separate:

- **Desktop app** — `stars-labs/starlab-desktop` (Iced, MIT). It consumes
  `starlab-client` (and its `core::*Manager` + `CoreState` + `HeadlessRunner`) as a
  cross-repo dependency, so `starlab-client` MUST keep that surface `pub`. The
  SD-card air-gap convention is shared: export/import dirs are
  `starlab_export` / `starlab_import` (see `starlab-client`'s `offline_manager.rs`)
  — a mismatch breaks desktop↔TUI air-gap interop.
- **Browser extension** — being split into `stars-labs/starlab-wallet`
  (FROST + YubiKey + sandbox execution); consumes `@starlab/core-wasm`.

When changing `starlab-client::core` (`WalletManager`, `SessionManager`, `DkgManager`,
`SigningManager`, `OfflineManager`, `ConnectionManager`, `CoreState`) or
`HeadlessRunner`, remember the desktop app depends on it across repos.

## Dependencies

FROST: `frost-core` 2.2.0, `frost-ed25519` 2.2.0, `frost-secp256k1` 2.2.0 (ZCash implementations).
Crypto: `sha2`, `sha3`, `k256`, `aes-gcm`, `argon2`, `pbkdf2` (keystore KDF — used in both `starlab-client::keystore::encryption` and `frost-core::keystore`), `hkdf` (root-secret expansion in frost-core), `hmac` (both HKDF and BIP-32-style HD derivation in frost-core's `hd_derivation.rs`). No direct `ed25519-dalek` — ed25519 curve ops go through `frost-ed25519` which pulls `curve25519-dalek` transitively.
Dev environment: Nix flake (`nix develop`) provides all system deps including graphics libs.

## Workspace Layout

```
Cargo.toml              # Workspace root, resolver = "2"
package.json            # Bun monorepo (browser extension)
flake.nix               # Nix dev environment (Linux + macOS)
apps/
  starlab-client/             # Rust binary + library (lib reused by starlab-desktop)
  cli/             # Headless CLI (crate starlab-cli) — conformance oracle
  signal-server/        # server/ + cloudflare-worker/
  browser-extension/    # WXT + Svelte (splitting out → stars-labs/starlab-wallet)
packages/@starlab/
  frost-core/           # Core crypto library
  core-wasm/            # WASM bindings
  blockchain/           # Chain integrations
  types/                # Shared TypeScript types (Bun workspace only, not in Cargo)
```
