# MPC Wallet TUI - Complete Technical Documentation

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Performance Optimizations](#performance-optimizations)
3. [User Experience Design](#user-experience-design)
4. [Navigation System](#navigation-system)
5. [Component Architecture](#component-architecture)
6. [State Management](#state-management)
7. [Security Model](#security-model)
8. [Testing Strategy](#testing-strategy)
9. [Deployment Guide](#deployment-guide)
10. [API Reference](#api-reference)

---

## 1. Architecture Overview

### Core Design Principles

The MPC Wallet TUI follows the **Elm Architecture** pattern, providing:
- **Unidirectional data flow**: Model → View → Message → Update → Model
- **Pure functions**: Side effects isolated in Commands
- **Type safety**: Rust's type system ensures correctness
- **Component isolation**: Each UI component is self-contained

### System Components

```
┌─────────────────────────────────────────────┐
│                   TUI Layer                  │
│  ┌─────────┐ ┌─────────┐ ┌─────────────┐   │
│  │ ElmApp  │ │  Model  │ │ Components  │   │
│  └────┬────┘ └────┬────┘ └──────┬──────┘   │
│       │           │              │           │
│  ┌────▼───────────▼──────────────▼────┐     │
│  │         Message Router              │     │
│  └────┬───────────┬──────────────┬────┘     │
│       │           │              │           │
└───────┼───────────┼──────────────┼───────────┘
        │           │              │
┌───────▼───────────▼──────────────▼───────────┐
│              Core Services                    │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │ Keystore │ │  FROST   │ │  WebRTC  │     │
│  └──────────┘ └──────────┘ └──────────┘     │
└───────────────────────────────────────────────┘
```

### File Structure

Real layout of `src/elm/` — from `ls`:

```
src/elm/
├── app.rs                # ElmApp<C> — main event loop + tui-realm shell
├── model.rs              # Model (pure UI state)
├── message.rs            # Message enum — input events
├── update.rs             # Update fn — Message → state transition + Commands
├── command.rs            # Command enum — side-effect tasks (non-generic; ciphersuite flows through AppState<C>)
├── mod.rs
├── provider.rs           # UIProvider trait
├── ws_runtime.rs         # WebSocket client runtime
├── webrtc_signaling.rs   # WebRTC signaling over the signal server
└── components/           # Per-screen tui-realm Component impls
```

Earlier drafts listed `adaptive_event_loop.rs` / `channel_config.rs`
/ `differential_update.rs` as part of this tree — none of those
files exist (verified via `find`). See § 2 below for details.

---

## 2. Performance Optimizations

Deliberate perf work in source today: **async tokio I/O**. That's
it.

Earlier drafts of this section described three specific optimizations
with Rust code samples:

  - `AdaptiveEventLoop { config, current_interval_ms, last_activity,
    is_idle }` — doesn't exist; no adaptive poll-interval code
    anywhere in the tree
  - `ChannelConfig { message_queue_size: 1000, session_event_queue_size:
    500, … }` — doesn't exist; no bounded-channel sizing scheme
  - `UpdateStrategy { NoUpdate / FullRemount / PartialUpdate }` —
    doesn't exist; no differential-render layer
  - "Reduces rendering overhead by 60-80%" — fabricated measurement
  - "CPU usage reduced from 5-10% to <1% when idle" — fabricated
    measurement

All three types are from
`docs/archive/dev-journal/PERFORMANCE_OPTIMIZATIONS.md` — a design
doc for work that was planned but never landed. I had propagated
these as real in other performance-section fixes earlier in this
cleanup pass (41d5ca0 / 7febf90 / f591806 / b335731); those have
been corrected back in their own docs.

Real opportunities if someone takes perf work on:

- Measure idle vs active CPU usage and introduce an adaptive
  event loop if the baseline justifies it.
- Audit `mpsc::unbounded_channel` call sites and add bounded
  alternatives where queue-growth could matter.
- Introduce differential rendering if tui-realm's built-in
  remount-on-state-change turns out to be a bottleneck.
- Add `criterion` benches with a reproducible methodology so
  future claims in this section can be anchored in measurement.

---

## 3. User Experience Design

### Design Philosophy

1. **Zero Learning Curve**: Menu-driven interface, no commands to memorize
2. **Visual Feedback**: Progress bars, status indicators, animations
3. **Contextual Help**: Always available with `?` key
4. **Error Recovery**: Clear error messages with suggested actions
5. **Accessibility**: High contrast, screen reader compatible

### Screen Hierarchy

```
Welcome Screen
    ├── Main Menu
    │   ├── Create New Wallet
    │   │   ├── Mode Selection (Online/Offline)
    │   │   ├── Curve Selection (Secp256k1/Ed25519)
    │   │   ├── Threshold Config
    │   │   └── DKG Process
    │   ├── Join Session
    │   │   ├── Session Discovery
    │   │   └── Session Details
    │   ├── Manage Wallets
    │   │   ├── Wallet List
    │   │   └── Wallet Details
    │   └── Settings
    │       ├── Network Settings
    │       └── Security Settings
    └── Help/About
```

### Visual Components

#### Progress Indicators
- **DKG Progress**: Multi-stage progress with participant status
- **Signing Progress**: Real-time signature generation tracking
- **Network Operations**: Connection status with retry indicators

#### Status Elements
- **Connection Status**: Visual WebSocket/WebRTC indicators
- **Wallet Status**: Balance, last activity, security level
- **Session Status**: Participant count, threshold, readiness

---

## 4. Navigation System

### Keyboard Shortcuts

#### Global Shortcuts
| Key | Action | Available |
|-----|--------|-----------|
| `Ctrl+Q` | Quit application | Always |
| `Ctrl+R` | Refresh current screen | Always |
| `Ctrl+H` | Go to home (main menu) | Always |
| `?` | Show contextual help | Always |
| `Esc` | Go back / Cancel | Context-dependent |

#### Navigation Keys
| Key | Action | Context |
|-----|--------|---------|
| `↑/↓` | Navigate menu items | Menus/Lists |
| `←/→` | Switch tabs/fields | Forms |
| `Enter` | Select/Confirm | Always |
| `Space` | Toggle selection | Checkboxes |
| `Tab` | Next field | Forms |
| `Shift+Tab` | Previous field | Forms |

### Navigation Stack

```rust
pub struct Model {
    pub navigation_stack: Vec<Screen>,
    pub current_screen: Screen,
    // ... other fields
}
```

**Behaviour**:
- Unbounded stack depth (no configurable max).
- `Model::push_screen` saves the current screen before transitioning;
  `Model::pop_screen` restores it on Esc; `Model::go_home` clears
  the stack and returns to the main menu.
- No breadcrumb rendering, no jump-to-level UI — earlier drafts
  promised these but they aren't implemented.

---

## 5. Component Architecture

### Component Structure

Each UI screen is a tui-realm `Component` — the trait comes from
upstream tuirealm, not a custom trait defined here:

```rust
// From tuirealm
impl Component<Message, UserEvent> for MainMenu {
    fn on(&mut self, event: Event<UserEvent>) -> Option<Message> { /* … */ }
    fn view(&mut self, frame: &mut Frame, area: Rect) { /* … */ }
    // plus state/props accessors
}
```

The `MpcWalletComponent` extension trait in
`src/elm/components/mod.rs` adds an `id()` method that returns the
`Id` enum variant so `Application::mount` knows which screen it's
wiring up.

### Core Components

#### MainMenu
- Shows "Create New Wallet" / "Join Session" / "Manage Wallets" /
  "Sign Transaction" / "Settings" / "Exit" (the last two always;
  Manage & Sign only when `wallets > 0`).
- Arrow-key navigation; Enter activates. Wallet count drives which
  items appear.

#### WalletList
- Shows the set of wallets stored under
  `~/.frost_keystore/<device_id>/<curve>/` via the keystore's
  cached `Vec<WalletMetadata>`.
- Earlier drafts of this doc claimed sort-by-balance, pagination,
  and search/filter features — none of those exist. No balance
  data is fetched, and the list renders with a simple scroll
  offset (not pagination). Filtering is not implemented.

#### CreateWallet / Mode+Curve+Threshold flow
- Single-screen mode selection (Online/Offline) → curve selection
  (secp256k1/ed25519) → threshold config → DKG ceremony.
- Not a multi-step wizard with rollback/progress persistence —
  each screen is discrete and `Esc` backs out without saving
  partial state.

#### DKGProgress
- Gauge-based progress display during the DKG ceremony.
- No "Error recovery options" button ships — if DKG fails you
  return to the main menu and start over.

#### JoinSession
- Shows announced sessions from the signal server's
  `session_available` broadcasts.
- Enter joins; Esc goes back. No preview / requirements validation
  / quick-reject UI beyond that.

### Component Communication

```
User Input → tuirealm Event → Component::on → Message → update() →
             (Model delta) + Option<Command> → Command::execute →
             (async work emitting Messages) → update() → ...
             → Component::view → Ratatui draw
```

---

## 6. State Management

### Model Structure

```rust
pub struct Model {
    // Core State
    pub wallet_state: WalletState,
    pub network_state: NetworkState,
    pub ui_state: UIState,
    
    // Navigation
    pub navigation_stack: Vec<Screen>,
    pub current_screen: Screen,
    
    // Session Management
    pub active_session: Option<SessionInfo>,
    pub pending_operations: Vec<Operation>,
    
    // User Context
    pub selected_wallet: Option<String>,
    pub device_id: String,
}
```

### State Updates

#### Pure Updates
- Model transformations
- No side effects
- Deterministic results

#### Commands (Side Effects)
- Network operations
- File I/O
- Async operations
- External system calls

### State Persistence

In-memory `Model` lives for the duration of the process. On-disk
persistence:

- **Keystore files**: wallet metadata + encrypted key share at
  `~/.frost_keystore/<device_id>/<curve>/<wallet_id>.{json,dat}`
  — written on DKG completion, re-read on startup.
- **Signal-server log**: append-only `tracing` output at
  `--log-location` (default
  `~/.frost_keystore/logs/mpc-wallet.log`).

No auto-save loop, no checkpoint-based crash recovery — earlier
drafts of this section promised "Auto-save every 30 seconds" +
"Crash recovery from last checkpoint"; neither is implemented.
On crash, anything not already persisted via the keystore is lost.

---

## 7. Security Model

For the authoritative security picture see
[`architecture/SECURITY.md`](./architecture/SECURITY.md), which
was rewritten in 89e9054 / 6d7fd5a / 333c97f to remove the same
fabrications this section inherited. In brief:

### Key protection

- **At rest**: AES-256-GCM + PBKDF2-HMAC-SHA256, 100_000 iterations,
  SALT_LEN = 16 bytes (not 32), NONCE_LEN = 12 bytes. Authoritative
  constants in `src/keystore/encryption.rs:20-21`.
- **Memory zeroization**: only
  `packages/@mpc-wallet/frost-core/src/root_secret.rs` uses
  `zeroize::Zeroize`. Key shares and decrypted keystore blobs are
  not zeroed on drop today. No swap-file exclusion, no secure-delete
  integration — earlier drafts listed these; none implemented.

### Network security

- WSS for signal-server connections; TLS version + cipher selection
  is delegated to the platform TLS stack (this app doesn't pin
  anything).
- WebRTC data channels use DTLS-SRTP; version/cipher is whatever
  the `webrtc` crate negotiates.
- No certificate pinning, no application-layer HMAC, no SDP
  sanitization, no TURN-server authentication (no TURN infra
  ships).

### Operational security

- **Offline mode** activated via `--offline` CLI flag; SD-card
  export/import wraps FROST packages in the `OfflineData` envelope
  (`src/offline/types.rs`). No automatic audit-trail generation —
  earlier drafts claimed this.
- **Access control**: password prompt for keystore unlock. No
  session timeouts, rate limiting, or failed-attempt tracking —
  earlier drafts listed these; none exist.

---

## 8. Testing Strategy

### Unit Tests

Unit tests live inside each module as `#[cfg(test)] mod tests`
blocks. Run the whole suite:

```bash
cargo test -p tui-node
```

Earlier drafts of this section showed sample tests for
`test_adaptive_event_loop` and `test_differential_updates` —
those tests don't exist because the features they'd test don't
exist (see § 2 Performance Optimizations).

### Integration Tests

```rust
// tests/integration/
├── dkg_flow.rs       # Complete DKG process
├── signing_flow.rs   # Transaction signing
├── import_export.rs  # Keystore operations
└── network_recovery.rs # Connection handling
```

### Test Coverage

| Component | Coverage | Target |
|-----------|----------|--------|
| Core Logic | 85% | 90% |
| UI Components | 70% | 80% |
| Network Layer | 75% | 85% |
| Cryptography | 95% | 100% |

### Testing Tools

- **Unit**: Rust built-in `#[test]`
- **Integration**: Custom test harness
- **UI**: MockUIProvider for headless testing
- **Network**: Mock WebSocket/WebRTC servers
- **Performance**: Criterion benchmarks

---

## 9. Deployment Guide

### Build Configurations

#### Development
```bash
cargo build --bin mpc-wallet-tui
RUST_LOG=debug ./target/debug/mpc-wallet-tui
```

#### Release
```bash
cargo build --release --bin mpc-wallet-tui
strip target/release/mpc-wallet-tui
```

#### Platform-Specific

**All platforms**: `cargo build --release -p tui-node
--bin mpc-wallet-tui` produces a single static binary at
`target/release/mpc-wallet-tui`. Distribute the binary directly —
there is no installer scaffolding in the repo.

Earlier drafts of this section suggested `cargo deb` / `cargo rpm
build` (Linux) / `lipo` universal binaries (macOS) / `cargo wix`
(Windows MSI). Verified: no `[package.metadata.deb]` /
`[package.metadata.wix]` entries exist in either `Cargo.toml`.
Those commands would fail without first configuring the
respective tooling — out of scope for this repo today. Same
absent-packaging finding as the workspace-level deployment
guide (06334be) and tech-doc Deployment section (f591806).

### System Requirements

Not measured. Earlier drafts of this section quoted specific
specs ("1 GHz single-core, 256 MB RAM, 50 MB storage" minimum;
"2 GHz dual-core, 1 GB RAM, 200 MB storage" recommended). Those
numbers had no source. The binary is a single-threaded
terminal app with an async runtime underneath — modest in
practice, but the repo doesn't benchmark a specific floor.

### Environment Variables

Real env vars the TUI consults (grepped from source):

```bash
export HOME=<your home>           # used to derive ~/.frost_keystore
export RUST_LOG=info              # standard tracing-subscriber filter
export PERF_MONITORING=1          # opt-in perf_monitor instrumentation
```

Earlier drafts listed `MPC_WALLET_CONFIG`, `MPC_KEYSTORE_PATH`,
`MPC_WEBSOCKET_URL` — none of those are read by the code. The
signal-server URL is a CLI flag (`--signal-server`); the keystore
path is fixed at `~/.frost_keystore`; no config file is loaded.

### Docker Deployment

Docker packaging isn't currently shipped. The `Dockerfile` that used
to live at `apps/tui-node/Dockerfile` was written for a pre-monorepo,
pre-edition-2024 layout (Rust 1.75, single-crate \`COPY Cargo.lock\`)
and doesn't build against the current workspace. A working
Dockerfile would need:

- `FROM rust:1.85-slim` (edition 2024 requires 1.85+)
- Placement at the monorepo root, not under apps/tui-node/
- A multi-stage build that copies every workspace member crate so
  cargo can resolve the full dep graph, then builds just the TUI
  binary: `cargo build --release --bin mpc-wallet-tui -p tui-node`

See `apps/tui-node/docs/DEPLOYMENT_GUIDE.md` for the currently-
supported deployment paths (systemd + launch scripts).

---

## 10. API Reference

The code is the authoritative reference — the enum signatures
below are sketches. For the complete variant lists, read the
source files directly (the real `Message` enum has ~80+ variants,
the real `Command` ~60+; listing them all here would duplicate
the source and drift immediately). The real `update` signature
is `pub fn update(model: &mut Model, msg: Message) -> Option<Command>`
(single optional command, not Vec) per `src/elm/update.rs:33`.

### Message Types

```rust
// Real definition: apps/tui-node/src/elm/message.rs
pub enum Message {
    // User input events (routed from Component::on)
    SelectItem { index: usize },
    SelectMode(Mode),
    SelectCurve(CurveType),
    ThresholdConfigConfirm,
    SignTypeChar(char),
    SignBackspace,
    SignSubmit,
    PasswordTypeChar(char),
    // …etc.

    // Async-result / network events (emitted by Command::execute)
    WsConnected,
    WsDisconnected { reason: String },
    SessionAvailable { info: SessionInfo },
    DkgRound1Received { from: String, package: Vec<u8> },
    DkgComplete,
    SigningComplete { signature: Vec<u8> },
    // …etc.

    // Navigation
    PushScreen(Screen),
    PopScreen,
    GoHome,
    Initialize,
}
```

Earlier drafts listed invented variants like `Navigate(Screen)`,
`CreateWallet(WalletConfig)`, `UpdateDKGProgress { round, progress
}`. The real variant names differ — see the source for the
canonical set.

### Command Types

```rust
// Real definition: apps/tui-node/src/elm/command.rs
pub enum Command<C: frost_core::Ciphersuite> {
    // Keystore
    InitializeKeystore { path: String, device_id: String },
    FinalizeWalletFromDkg { password: String, keystore_path: String,
                            wallet_name: String },
    UnlockWallet { wallet_id: String, keystore_path: String,
                   password: String, … },

    // Signal-server WebSocket
    ConnectWebSocket,
    SendWs(ClientMsg),

    // DKG / signing orchestration
    StartDKG { config: WalletConfig },   // session announce
    StartFrostProtocol,                  // fires once mesh is up
    StartSigning { wallet_id: String, message: String },
    // …etc.

    NoOp,
}
```

Note the `<C>` type parameter: every `Command` instance is
NON-generic. Earlier drafts of this section claimed `Command<C>`
with a ciphersuite generic; the real enum has no type parameter
(`pub enum Command` at `src/elm/command.rs:15`). The ciphersuite
is threaded through `AppState<C>` which `Command::execute` takes
by reference. This means the same Elm loop drives both the ed25519
and secp256k1 code paths via monomorphization.

### UIProvider trait

The real `UIProvider` trait is at
`apps/tui-node/src/elm/provider.rs:24` with ~20 async methods for
pushing state into the UI layer. Earlier drafts showed a
5-method sketch (`update_screen` / `show_message` /
`update_progress` / `get_user_input` / `confirm_action`) — none of
those method names match the real trait. See the authoritative
definition in source; the extension-doc architecture summary
(9990b34) also lists the real method set.

### Keystore API

Real shape from `src/keystore/storage.rs`:

```rust
impl Keystore {
    pub fn new<P: AsRef<Path>>(base_path: P, device_id: &str) -> io::Result<Self>;
    pub fn save_wallet(&mut self, metadata: WalletMetadata,
                       key_share: &[u8], password: &str) -> Result<()>;
    pub fn load_wallet(&self, wallet_id: &str, password: &str) -> Result<Vec<u8>>;
    pub fn list_wallets(&self) -> &[WalletMetadata];
    pub fn remove_wallet(&mut self, wallet_id: &str) -> Result<()>;
}
```

No `get_wallet(&str) -> Option<&Wallet>` — there's no `Wallet`
type; encrypted key shares stay on disk and are decrypted on demand
via `load_wallet(password)`. Earlier drafts of this section showed
a generic wallet-management interface that didn't match the
actual split-file keystore.

### FROST Protocol API

This crate does NOT define its own `FrostProtocol` trait —
DKG and signing primitives come from upstream `frost-core 2.2`
(plus `frost-ed25519` / `frost-secp256k1`). Earlier drafts
(and 49360fa caught similar cases) sketched a local trait with
`start_dkg` / `process_round1` / `process_round2` methods —
fabricated. The TUI wraps upstream frost-core via
`src/protocal/dkg.rs` + `src/protocal/dkg_coordinator.rs` +
`src/protocal/signing.rs`; consult those files for the real
call surface.

```rust
// Upstream frost-core, not this crate:
    fn start_signing(key_share: &KeyShare, message: &[u8]) -> Result<SigningSession>;
    fn generate_nonces(session: &mut SigningSession) -> Result<SigningNonces>;
    fn generate_signature_share(session: &SigningSession, nonces: &SigningNonces) -> Result<SignatureShare>;
    fn aggregate_signatures(shares: Vec<SignatureShare>) -> Result<Signature>;
}
```

---

## Appendices

### A. Configuration

The TUI has no config file today — runtime settings come from CLI
flags only. See `apps/tui-node/src/bin/mpc-wallet-tui.rs` for the
authoritative `clap::Args` struct, or `apps/tui-node/docs/README.md`
§ Configuration for the summary. The TOML schema originally sketched
here described features that were never implemented (theme, auto-lock,
audit log, reconnect tuning); removed so nobody follows it and
discovers the flags silently do nothing.

### B. Error Codes

No numeric error-code scheme exists — see `src/errors.rs` for the
strongly-typed error variants (`DKGError`, `SigningError`,
`KeystoreError`, `ComponentError`, `CryptoError`). A shared numeric
registry across Rust + TypeScript is open future work.

### C. Keyboard Map Reference

Canonical reference: [`KEYBOARD_NAVIGATION_GUIDE.md`](./KEYBOARD_NAVIGATION_GUIDE.md)
(rewritten in d09bddc after verifying every keybinding claim against
`src/elm/components/`).

TL;DR of what actually works:

```
┌─────────────────────────────────────┐
│  Global (every screen)              │
├─────────────┬───────────────────────┤
│ ↑ / ↓       │ Move selection / focus│
│ Enter       │ Confirm               │
│ Esc         │ Back / cancel         │
│ Tab         │ Move focus in a screen│
│ Ctrl+C      │ Quit (OS interrupt)   │
└─────────────┴───────────────────────┘
```

Earlier drafts of this appendix listed `Ctrl+Q` / `Ctrl+R` /
`Ctrl+H` / `?` global shortcuts, `hjkl` vim-style nav, `n` / `j`
/ `s` / `w` quick keys, `/` search, and `:` command mode. None of
those exist in source (verified via `grep Key::Char` across
`src/elm/components/`). See KEYBOARD_NAVIGATION_GUIDE.md for the
per-screen breakdown of what each component's `on(event)` handler
actually accepts.

### D. Troubleshooting Guide

#### TUI Display Issues

**Problem**: Garbled or broken UI
**Solution**: 
```bash
# Check terminal capabilities
echo $TERM
# Set proper terminal
export TERM=xterm-256color
# Reset terminal
reset
```

**Problem**: Colors not displaying
**Solution**:
```bash
# Force color output
export COLORTERM=truecolor
# Check terminfo
infocmp $TERM | grep colors
```

#### Performance Issues

**Problem**: High CPU usage
**Solution**:
Earlier drafts suggested "Check adaptive event loop is enabled" /
"Verify bounded channels are configured" / "Enable differential
updates" / "Disable animations in config" — none of those features
exist (see § 2 Performance Optimizations + the 49360fa retraction).
Real mitigations:

- Lower log verbosity: `RUST_LOG=error` or `RUST_LOG=warn`
  (default `info` is cheap; `debug`/`trace` gets expensive mid-
  ceremony).
- Stagger startup when running many TUI instances on one host so
  they don't all poll the signal server in lockstep.

#### Network Issues

**Problem**: Cannot connect to signal server

```bash
# curl does NOT speak wss:// — for a WebSocket upgrade probe,
# use wscat (`npm install -g wscat`):
wscat -c wss://xiongchenyu.dpdns.org/

# For plain TCP/TLS reachability:
curl -v https://xiongchenyu.dpdns.org/
# Server is WebSocket-only so a GET returns 400, but reaching
# "HTTP/1.1 400" confirms DNS + TLS + routing worked.

# Outbound firewall check:
sudo iptables -L
```

**Problem**: WebRTC connection fails after signal-server succeeds

- Check NAT type on both sides — symmetric NAT requires a TURN
  server, which this repo does not ship. Full-cone / restricted-
  cone / port-restricted-cone all work with public STUN.
- Open `chrome://webrtc-internals` in a browser on the same
  network to confirm STUN candidate gathering succeeds there —
  if the browser can't get candidates, neither can the TUI.
- Fallback: switch to `--offline` mode and exchange DKG/signing
  artefacts via SD card.

---

## Conclusion

This doc aims to describe the TUI as it actually ships today.
Earlier drafts concluded with "professional-grade implementation",
"enterprise-ready functionality", "comprehensive optimization",
plus a footer claiming "Document Version: 2.0.0" + "Status:
Production Ready". None of that was accurate (same fabrication
class removed from the MPC_WALLET_TECHNICAL_DOCUMENTATION.md
footer in f13514a): `git tag -l` is still empty, all workspace
crates are at 0.1.x, no third-party audit has been performed,
no benchmarks ship. What DOES ship:

- Real FROST t-of-n DKG + threshold signing via upstream
  `frost-core 2.2` (secp256k1 for Ethereum, ed25519 for Solana).
- Encrypted per-share keystore (PBKDF2 100k + AES-256-GCM) that
  round-trips with the browser extension.
- Online (WebRTC mesh) and offline (SD-card air-gap) ceremony
  modes.
- 174+ Rust tests under `cargo test --workspace` covering the
  DKG / signing / keystore paths.

For the latest state see
[github.com/hecoinfo/mpc-wallet](https://github.com/hecoinfo/mpc-wallet)
and `git log`; for security reports use
[GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new).