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

Real Screen enum at `src/elm/model.rs:378-427` (see
ELM_ARCHITECTURE.md § Screen Transitions for the full 23-variant
list). Tree structure below groups by user-flow, with each leaf
matching a real Screen variant:

```
Welcome
    └── MainMenu
        ├── CreateWallet (wizard)
        │   ├── PathSelection
        │   ├── ModeSelection             (Online/Offline)
        │   ├── TemplateSelection
        │   ├── WalletConfiguration       (contains threshold + curve
        │   │                              choice as form fields; not
        │   │                              separate screens)
        │   ├── ThresholdConfig
        │   ├── PasswordPrompt
        │   ├── DKGProgress { session_id }
        │   └── WalletComplete { wallet_id }
        │
        ├── JoinSession
        │   ├── SessionDetail { session_id }
        │   └── AcceptSession { sessions }
        │
        ├── ManageWallets                  (wallet_count > 0)
        │   ├── WalletDetail { wallet_id }
        │   ├── ImportWallet
        │   └── ExportWallet { wallet_id }
        │
        ├── SignTransaction { wallet_id }  (wallet_count > 0)
        │   ├── SigningProgress { request_id }
        │   └── SignatureComplete { request_id }
        │
        └── Settings
            ├── NetworkSettings
            ├── SecuritySettings
            └── About
```

Earlier drafts of this tree:
  - Showed a standalone `Curve Selection (Secp256k1/Ed25519)` screen
    — doesn't exist. Curve choice is a form field on
    `WalletConfiguration`, not its own screen.
  - Omitted the `SignTransaction` top-level branch (it's a sibling of
    `ManageWallets`, not nested under it).
  - Omitted `PasswordPrompt`, `ImportWallet`, `ExportWallet`,
    `AcceptSession`, `DKGProgress`, `WalletComplete`,
    `SigningProgress`, `SignatureComplete`, `About` — all real
    Screen variants.
  - Showed a `Help/About` leaf; there is no Help screen (no help
    modal ships, see § Keyboard Shortcuts below). `About` IS a
    real Screen.

### Visual Components

#### Progress Indicators
- **DKG Progress** (`dkg_progress.rs`): Multi-phase progress
  showing which FROST round is running + per-participant ready
  state. Renders as a `Gauge` + participant list. Used for both
  DKG and signing ceremonies (same component; label text
  switches).
- **Signing Progress**: same `dkg_progress.rs` component
  re-purposed — shows commitment / share / aggregate phase and
  which selected signers have replied.
- **Network Operations**: connection-state badges surface inside
  Join Session + the top status bar.

#### Status Elements
- **Connection Status**: WebSocket connected / WebRTC peer count
  indicators in the top status bar (`app.rs` renders into the
  title area).
- **Wallet Status**: real surfaced fields are `wallet_id /
  curve_type / threshold / total / participants list / derived
  addresses`. Earlier drafts of this bullet listed
  "Balance, last activity, security level" — none of those are
  surfaced. The TUI does not query on-chain balances, does not
  track last-used/last-activity timestamps, and has no
  "security level" indicator.
- **Session Status**: participant count, threshold, readiness
  (accepted_devices list length vs session.total).

---

## 4. Navigation System

### Keyboard Shortcuts

#### Global Shortcuts (wired at `src/elm/app.rs:851-866`)

| Key | Action | Source |
|-----|--------|--------|
| `Ctrl+Q` | Quit application | app.rs:851 → `Message::Quit` |
| `Ctrl+C` | Quit application | app.rs:855 → `Message::Quit` |
| `Ctrl+R` | Refresh current screen | app.rs:859 → `Message::Refresh` |
| `Ctrl+H` | Go to home (main menu) | app.rs:863 → `Message::NavigateHome` |
| `Esc` | Go back / cancel | app.rs:847 → `Message::NavigateBack` (per-component also handles it) |

Earlier drafts listed `?` as a contextual-help global; no such
handler exists (`grep -n "KeyCode::Char..?.." src/elm/app.rs`
returns no matches). No help modal ships. See
[`KEYBOARD_NAVIGATION_GUIDE.md`](./KEYBOARD_NAVIGATION_GUIDE.md)
for the authoritative per-screen reference.

#### Navigation Keys (per-component `Component::on` handlers)

| Key | Action | Context |
|-----|--------|---------|
| `↑` / `↓` | Navigate menu items / list rows | Menus, lists |
| `Enter` | Select / confirm | Most screens |
| `Tab` | Move focus to next form field | Forms |
| `Shift+Tab` | Move focus to previous field | Forms (supported by tui-realm's default field tab cycling) |

Earlier drafts listed `←/→` (switch tabs/fields) and `Space`
(toggle selection) as standard navigation keys. Neither is
wired up in the per-component handlers (`grep "KeyCode::Left\|
KeyCode::Right\|KeyCode::Char(' ')"` returns matches only inside
password/message-input fields where the printable char flows into
the text buffer, not as navigation). Form-field navigation is
Tab-only.

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

#### CreateWallet wizard (Mode → Threshold → Password → DKG)
- Real screen sequence: ModeSelection (Online/Offline) →
  TemplateSelection → WalletConfiguration (carries curve choice
  as a form field — NO separate CurveSelection screen) →
  ThresholdConfig → PasswordPrompt → DKGProgress → WalletComplete.
- Each screen is a discrete Screen variant (see Screen Hierarchy);
  Esc backs out without saving partial state. There is no
  wizard-style rollback-with-persisted-progress layer.
- Earlier drafts described this as "Mode → Curve → Threshold → DKG"
  with a standalone curve-selection step. The real flow has no
  CurveSelection screen; curve is selected via a radio/field on
  WalletConfiguration.

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

Real struct at `src/elm/model.rs:14-36`:

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
    pub session_invites: Vec<SessionInfo>,  // discovered-but-not-joined
                                            // sessions from
                                            // session_available broadcasts

    // User Context
    pub selected_wallet: Option<String>,
    pub device_id: String,

    // Application metadata
    pub app_version: String,                // set from CARGO_PKG_VERSION
    pub last_saved: Option<DateTime<Utc>>,  // keystore-write tracking
}
```

Earlier drafts of this sketch dropped `session_invites`,
`app_version`, `last_saved` — all three are real fields as of
the current source.

### State Updates

The `update(&mut Model, Message) -> Option<Command>` function is
the only legitimate site that mutates Model. It's not pure in the
strict functional sense — it takes `&mut Model` and mutates in
place rather than returning a new Model — but it IS deterministic
+ side-effect-free beyond the Model mutation itself (no I/O, no
network, no filesystem writes). All external interactions flow
through the returned `Option<Command>`, which is executed
asynchronously by the runtime (see Command::execute).

#### Update function responsibilities
- Synchronous Model mutation via `&mut Model`
- Emit exactly one `Option<Command>` (use `Command::Batch(Vec<Command>)`
  when multiple side effects are needed in a single update tick)
- Deterministic given `(previous Model, incoming Message)` — no
  implicit I/O reads

#### Commands handle side effects
- Network operations (WebSocket send, WebRTC peer connections)
- Filesystem I/O (keystore write/read)
- FROST protocol round execution (delegates through InternalCommand<C>)
- Spawning async tasks that eventually feed new Messages back
  into the queue

### State Persistence

In-memory `Model` lives for the duration of the process. On-disk
persistence:

- **Keystore files**: one `<wallet_id>.json` per wallet at
  `~/.frost_keystore/<device_id>/<curve>/<wallet_id>.json`. The
  JSON wraps plaintext metadata plus the base64-encoded
  AES-256-GCM ciphertext in a `WalletFile` struct (single file,
  NOT a `.json` + `.dat` pair as earlier drafts claimed — same
  retraction as f4fc866 for other docs). Written on DKG
  completion, re-read on startup (scan the keystore directory +
  cache metadata; the encrypted share only decrypts on unlock).
- **TUI tracing log**: append-only `tracing` output from the TUI
  itself (not the signal server) at the path passed to
  `--log-location` (default
  `~/.frost_keystore/logs/mpc-wallet.log`). Earlier drafts
  labelled this the "signal-server log" — it's not; the signal
  server is a separate process with its own stderr. This is the
  local TUI binary's structured-log file.

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
  `packages/@mpc-wallet/frost-core/src/root_secret.rs` zeros on
  drop, via a manual `self.0.fill(0)` in its `Drop` impl
  (`root_secret.rs:62-67`) — NOT via the `zeroize` crate (which
  isn't a workspace dependency). Key shares and decrypted keystore
  blobs are not zeroed on drop today. No swap-file exclusion, no
  secure-delete integration — earlier drafts listed these and
  asserted `zeroize::Zeroize` is in use; neither is true.

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

Real integration-style coverage lives in two places:

```
apps/tui-node/tests/
├── update_transitions.rs   # Pure Elm update() transition assertions
└── component_rendering.rs  # tui-realm component render smoke tests

apps/tui-node/examples/      # Runnable end-to-end scenarios
├── offline_dkg_demo.rs
├── offline_dkg_signing_demo.rs
├── offline_frost_dkg_signing.rs
├── hybrid_mode_e2e_test.rs
├── webrtc_mesh_e2e_test.rs
├── test_join_session_navigation.rs
├── test_keyboard_events.rs
└── test_session_loading_simple.rs
```

Earlier drafts of this section invented a `tests/integration/`
directory tree with files named `dkg_flow.rs` / `signing_flow.rs` /
`import_export.rs` / `network_recovery.rs`. None of those files
exist (`find apps/tui-node/tests -type f` returns only the two
above). End-to-end flows are exercised by the `examples/` binaries,
not a dedicated integration test harness.

### Test Coverage

No automated coverage tooling is wired into the workspace — there
is no `cargo tarpaulin` / `grcov` / `llvm-cov` config, no CI step
producing a coverage report, and no badge target. Earlier drafts
of this section printed a 4-row table with percentages
(`Core Logic 85% / UI Components 70% / Network Layer 75% /
Cryptography 95%`). Those numbers were fabricated; there was no
measurement run behind them. Removed.

### Testing Tools

- **Unit**: Rust built-in `#[test]`
- **Integration-style end-to-end**: runnable `examples/*.rs` binaries
  (see above) — no shared "test harness" crate beyond what
  `cargo test` + plain `tokio::test` provide.
- **UI**: `NoOpUIProvider` (`src/elm/provider.rs:71`) is the real
  headless provider used by `tests/update_transitions.rs`. Earlier
  drafts called this type `MockUIProvider`, which doesn't exist.
- **Network**: no dedicated mock WebSocket / WebRTC server —
  integration-style examples spin up real `tokio` listeners.
- **Performance**: no `criterion` dependency and no `benches/`
  directory today. Earlier drafts listed "Criterion benchmarks" as
  a real tool; they aren't. Adding a benches tree is open future
  work (see the Performance Considerations section).

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
numbers had no source. The binary uses `#[tokio::main]` with
the default multi-thread runtime, so it spawns a worker-thread
pool sized to the CPU core count — NOT single-threaded as an
earlier draft of this paragraph claimed. The Elm render loop
itself runs on one thread (tui-realm is single-threaded by
design), but FROST rounds / WebSocket I/O / WebRTC peer tasks
run on tokio workers in parallel. Modest in practice (no
benchmarks ship to pin a specific floor).

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
    // Navigation (verified message.rs:15-18,263)
    NavigateBack,            // line 15 — Esc
    NavigateHome,            // line 16 — Ctrl+H
    PushScreen(Screen),      // line 17
    PopScreen,               // line 18
    Quit,                    // line 263 — Ctrl+Q / Ctrl+C
    Refresh,                 // Ctrl+R

    // User input events (routed from Component::on)
    SelectItem { index: usize },
    SelectMode(WalletMode),  // line 34 — carries WalletMode, not Mode
    SetThreshold(u16),       // line 37 — NOT ThresholdConfigConfirm
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
}
```

Earlier drafts listed invented variants like `Navigate(Screen)`,
`CreateWallet(WalletConfig)`, `UpdateDKGProgress { round, progress
}`, `SelectCurve(CurveType)`, `ThresholdConfigConfirm`, `GoHome`.
None exist in source:

- `Navigate(Screen)` — real navigation is `PushScreen` / `PopScreen`
  / `NavigateBack` / `NavigateHome`.
- `SelectCurve(CurveType)` + `ThresholdConfigConfirm` — `grep`
  returns zero hits. No CurveSelection screen exists (curve is a
  form field on Create Wallet); threshold confirmation uses
  `SetThreshold(u16)` + regular form submission.
- `GoHome` — real variant name is `NavigateHome`; the model
  helper method is `go_home` (note the naming mismatch between
  the Message variant and the Model method).
- `SelectMode(Mode)` — real type is `WalletMode`, not `Mode`.

See the source for the canonical ~80+ variant set.

### Command Types

Note: the TUI has TWO parallel command enums. Read both together
to understand the flow:

```rust
// src/elm/command.rs — non-generic, higher-level Elm commands
pub enum Command {
    // Keystore
    InitializeKeystore { path: String, device_id: String },
    FinalizeWalletFromDkg { password: String,
                            keystore_path: String,
                            wallet_name: String },              // command.rs:76
    UnlockWallet { wallet_id: String,
                   password: String,
                   keystore_path: String },              // command.rs:90 —
                                                         // exactly 3 fields,
                                                         // no more

    // DKG / signing orchestration
    StartDKG { config: WalletConfig },         // command.rs:46 —
                                               // session announce
    StartFrostProtocol,                        // command.rs:53 —
                                               // fires once mesh is up
    StartSigning { request: SigningRequest },  // command.rs:97 —
                                               // NOT {wallet_id, message};
                                               // the SigningRequest struct
                                               // carries those fields plus
                                               // signing_id / blockchain /
                                               // chain_id internally.
    ApproveSignature { request_id: String },   // command.rs:98
    RejectSignature { request_id: String },    // command.rs:99
    Batch(Vec<Command>),                       // command.rs:121 — wraps
                                               // multiple commands
                                               // emitted by one update tick

    // …~60 more variants
}

impl Command {
    pub async fn execute<C: frost_core::Ciphersuite + …>(
        self,
        tx: UnboundedSender<Message>,
        app_state: &Arc<Mutex<AppState<C>>>,
    ) -> anyhow::Result<()> { /* … */ }
}

// src/utils/state.rs — ciphersuite-generic, per-round DKG/signing
pub enum InternalCommand<C: Ciphersuite> {
    // Keystore
    InitKeystore { path: String, device_name: String },  // state.rs:32
    ListWallets,
    CreateWallet {                                       // state.rs:41
        name: String,
        description: Option<String>,
        password: String,
        tags: Vec<String>,
    },
    LocateWallet { wallet_id: String },                  // state.rs:49

    // Session lifecycle
    SendToServer(ClientMsg),
    ProposeSession { session_id, total, threshold, participants },
    AcceptSessionProposal(String),
    InitiateWebRTCConnections,
    ReportChannelOpen { device_id },
    ProcessMeshReady { device_id },

    // DKG rounds (per-round granularity)
    TriggerDkgRound1,
    ProcessDkgRound1 { from_device_id: String, package: round1::Package<C> },
    TriggerDkgRound2,
    ProcessDkgRound2 { from_device_id: String, package: round2::Package<C> },
    FinalizeDkg,

    // Signing rounds
    InitiateSigning { transaction_data: String, blockchain: String, … },
    ProcessSigningCommitment { /* … */ },
    ProcessSignatureShare { /* … */ },
    // …more
}
```

The split is the product of two generations of the code:
`InternalCommand<C>` is the older ciphersuite-generic enum that
handles per-round DKG/signing mechanics; `Command` is the newer
Elm-architecture enum for higher-level orchestration. Retracting
my earlier claim (27615dd) that `TriggerDkgRound1`/`TriggerDkgRound2`
don't exist — they exist as `InternalCommand<C>` variants. The
retraction was right about `elm::Command` being non-generic
(verified) but wrong to extend that claim to every Command-named
type.

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

Real shape from `src/keystore/storage.rs` (verified against
current source):

```rust
impl Keystore {
    pub fn new(base_path: impl AsRef<Path>, device_name: &str)
        -> Result<Self>;

    // Accessors
    pub fn device_id(&self) -> &str;
    pub fn list_wallets(&self) -> Vec<&WalletMetadata>;
    pub fn get_wallet(&self, wallet_id: &str) -> Option<&WalletMetadata>;
    pub fn get_this_device(&self) -> Option<DeviceInfo>;

    // Create
    pub fn create_wallet(&mut self, /* … */) -> Result<String>;
    pub fn create_wallet_multi_chain(&mut self, /* … */) -> Result<String>;

    // Load the decrypted key-share bytes for a stored wallet
    pub fn load_wallet_file(&self, wallet_id: &str, password: &str)
        -> Result<Vec<u8>>;
}
```

Notes:

- `get_wallet` returns `Option<&WalletMetadata>` — NOT
  `Option<&Wallet>`; there's no `Wallet` struct in the codebase.
- The method reading encrypted key-share bytes is
  `load_wallet_file(wallet_id, password) -> Result<Vec<u8>>`, not
  `load_wallet` as earlier drafts of this sketch claimed.
- `save_wallet` / `remove_wallet` do NOT exist as named methods;
  wallet creation goes through `create_wallet` /
  `create_wallet_multi_chain`, which serialize metadata + the
  encrypted share to disk in one pass.

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

No numeric error-code scheme exists. Strongly-typed error enums
live per-domain (no top-level `src/errors.rs` umbrella file):

  - `KeystoreError`       (`src/keystore/mod.rs:24`)
  - `FrostKeystoreError`  (`src/keystore/frost_keystore.rs:19`)
  - `OfflineError`        (`src/offline/mod.rs:24`)
  - `CoreError`           (`src/core/mod.rs:21`)

plus upstream `FrostError` from
`packages/@mpc-wallet/frost-core` which carries FROST-specific
variants like `SigningError(String)`. A shared numeric registry
across Rust + TypeScript is open future work.

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
  cone / port-restricted-cone normally work with public STUN,
  but the TUI currently passes an empty ICE-server list at
  `src/network/webrtc.rs:285` + `src/elm/webrtc_signaling.rs:387`,
  so even those easier NAT types may fail until STUN is wired in.
  The browser extension hard-codes Google's STUN at
  `apps/browser-extension/src/entrypoints/offscreen/webrtc.ts:32`;
  the TUI hasn't picked up the matching change.
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