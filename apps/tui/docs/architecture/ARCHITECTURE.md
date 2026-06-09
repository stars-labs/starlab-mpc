# FROST MPC TUI Wallet - Architecture

## Table of Contents

1. [System Overview](#system-overview)
2. [Core Components](#core-components)
3. [TUI Architecture](#tui-architecture)
4. [Network Layer](#network-layer)
5. [Cryptographic Core](#cryptographic-core)
6. [Storage System](#storage-system)
7. [Security Architecture](#security-architecture)
8. [Performance Considerations](#performance-considerations)
9. [Extension Points](#extension-points)

## System Overview

The MPC Wallet TUI is a modular, event-driven Ratatui application that wraps the FROST threshold-signature protocol (via `frost-core 2.2`) in a keyboard-driven terminal interface. The architecture aims for clear separation between the tui-realm Elm loop, the protocol state machines in `src/protocal/`, and the shared `*Manager` business-logic types in `src/core/` that are reused by native-node.

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Terminal UI Layer                       │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   Ratatui   │  │ UI Provider  │  │  Event Handler   │  │
│  │  Framework  │  │  Interface   │  │     System       │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                    Business Logic Layer                      │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   Session   │  │    Wallet    │  │   Transaction    │  │
│  │  Manager    │  │   Manager    │  │     Engine       │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                      Network Layer                           │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  WebSocket  │  │    WebRTC    │  │    Offline       │  │
│  │   Client    │  │     Mesh     │  │    Handler       │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                   Cryptographic Core                         │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │    FROST    │  │   Keystore   │  │   Threshold      │  │
│  │   Protocol  │  │  Encryption  │  │    Signing       │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Separation of Concerns**: Clear boundaries between UI, business logic, and cryptography
2. **Event-Driven Architecture**: Asynchronous message passing between components
3. **Security by Design**: Cryptographic operations isolated in secure modules
4. **User-Centric Interface**: TUI designed for ease of use without sacrificing functionality
5. **Network Resilience**: Support for both online and offline operations

## Core Components

### Application Entry (`elm/app.rs`)

The real entry struct is `ElmApp<C>`, not a named `AppRunner`.
Earlier drafts of this doc referenced an `AppRunner` type that
never existed in source.

```rust
// src/elm/app.rs
pub struct ElmApp<C: frost_core::Ciphersuite> {
    model: Model,                                // pure UI state
    app: Application<Id, Message, UserEvent>,    // tui-realm shell
    terminal: CrosstermTerminalAdapter,
    message_tx: UnboundedSender<Message>,
    message_rx: UnboundedReceiver<Message>,
    app_state: Arc<Mutex<AppState<C>>>,          // shared with non-Elm managers
    should_quit: bool,
}
```

See [`ELM_ARCHITECTURE.md`](./ELM_ARCHITECTURE.md) for the
Model/Update/View breakdown.

### UI Provider System (`elm/provider.rs`)

Trait abstracting UI backends so non-Elm managers (the `core::*Manager`
types reused by native-node) can push state without knowing whether
they're driving a Ratatui TUI, a Slint GUI, or a test harness:

```rust
#[async_trait]
pub trait UIProvider: Send + Sync {
    // Connection + device list
    async fn set_connection_status(&self, connected: bool);
    async fn set_device_id(&self, device_id: String);
    async fn update_device_list(&self, devices: Vec<String>);

    // Session / DKG / signing updates
    async fn update_session_status(&self, status: String);
    async fn add_session_invite(&self, invite: SessionInfo);
    async fn update_dkg_status(&self, status: String);
    async fn add_signing_request(&self, request: PendingSigningRequest);
    async fn set_signature_result(&self, signing_id: String, signature: Vec<u8>);

    // Wallet list + logs + mesh status + error/progress
    async fn update_wallet_list(&self, wallets: Vec<WalletDisplayInfo>);
    async fn add_log(&self, message: String);
    async fn update_mesh_status(&self, ready: usize, total: usize);
    async fn show_error(&self, error: String);
    async fn set_busy(&self, busy: bool);
    // …etc., see provider.rs for the full surface
}
```

**Real implementations:**
- `NoOpUIProvider` (`elm/provider.rs`) — no-op for tests / headless
- The TUI itself drives UI updates through the tui-realm Elm loop
  rather than implementing `UIProvider` directly. (Earlier drafts
  of this doc listed `TuiProvider` / `CliProvider` / `TestProvider`
  implementations that don't exist in source — removed.)

### State Management (`utils/appstate_compat.rs`)

`AppState<C: Ciphersuite>` is the shared state container — a
thread-safe (`Arc<Mutex<AppState<C>>>`) blob holding the pieces
that the Elm `Model` doesn't own: peer connections, ICE candidates,
DKG/signing FROST state, etc.

Key fields (abbreviated — full struct in `utils/appstate_compat.rs`):

```rust
pub struct AppState<C: Ciphersuite> {
    // Identity + network
    pub device_id: String,
    pub signal_server_url: String,
    pub devices: Vec<String>,

    // Session
    pub session: Option<SessionInfo>,
    pub invites: Vec<SessionInfo>,
    pub available_sessions: Vec<SessionAnnouncement>,

    // Keystore + blockchain surface
    pub keystore: Option<Arc<Keystore>>,
    pub blockchain_addresses: Vec<BlockchainInfo>,
    pub current_wallet_id: Option<String>,

    // WebRTC mesh (per-peer tables)
    pub device_connections: Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>,
    pub data_channels: HashMap<String, Arc<RTCDataChannel>>,
    pub device_statuses: HashMap<String, RTCPeerConnectionState>,
    pub pending_ice_candidates: HashMap<String, Vec<RTCIceCandidateInit>>,

    // DKG state machine + packages
    pub mesh_status: MeshStatus,
    pub dkg_state: DkgState,
    pub dkg_round1_packages: BTreeMap<Identifier<C>, round1::Package<C>>,
    pub dkg_round2_packages: BTreeMap<Identifier<C>, round2::Package<C>>,
    pub key_package: Option<KeyPackage<C>>,
    pub group_public_key: Option<VerifyingKey<C>>,

    // Signing state machine + FROST intermediates
    pub signing_state: SigningState<C>,
    pub pending_signing_requests: Vec<PendingSigningRequest>,
    pub frost_commitments: BTreeMap<Identifier<C>, SigningCommitments<C>>,
    pub frost_signature_shares: BTreeMap<Identifier<C>, SignatureShare<C>>,
    pub frost_nonces: Option<SigningNonces<C>>,
    pub signing_message: Option<Vec<u8>>,

    pub log: Vec<String>,
}
```

**What's NOT here** (earlier drafts of this doc listed these —
verified absent via grep): `curve_type`, `wallets: HashMap<…>`,
`pending_operations: VecDeque<…>`, `network_status`. Curve is
per-wallet (lives in the keystore's `WalletMetadata` + is pinned
on `Model.wallet_state.curve_type`); wallets live in the
keystore, not a HashMap on AppState; signing requests queue
through `pending_signing_requests`.

**What IS here but worth calling out**: `pub offline_mode: bool`
(appstate_compat.rs:77) + `pub dkg_in_progress: bool`
(appstate_compat.rs:73). An earlier draft of THIS retraction
claimed `offline_mode` was a fake field — it's real. The
`offline_mode` flag is set at startup from the `--offline` CLI
arg and is read at the signaling-setup + SD-card-export branch
points. There is no in-app toggle to flip it mid-run.

## TUI Architecture

### tui-realm Integration

The TUI is built on [tui-realm](https://github.com/veeso/tuirealm)
(which itself wraps Ratatui), using its Elm-architecture event
routing. There is no named `TuiManager` struct — the terminal +
application machinery lives on `ElmApp<C>` (see Core Components
above).

### UI Components

The real per-screen components (under `src/elm/components/`):

```
src/elm/components/
├── main_menu.rs            # root navigation
├── mode_selection.rs       # online / offline selection
├── threshold_config.rs     # t-of-n picker
├── join_session.rs         # browse + join announced sessions
├── wallet_list.rs          # ManageWallets screen
├── wallet_detail.rs
├── wallet_complete.rs      # DKG completion
├── create_wallet.rs
├── password_prompt.rs      # unlock / import password flow
├── dkg_progress.rs         # DKG progress gauge
├── sign_transaction.rs
├── signature_complete.rs   # EIP-191 result display
├── notification.rs         # toast-style messages
└── modal.rs                # modal dialog scaffolding
```

Each component is a tui-realm `Component` impl that routes input
events through `Component::on(Event) -> Option<Message>`. The `Id`
enum (one variant per component) is what `Application::mount` /
`Application::active` reference.

### Event System

User input arrives as `tuirealm::Event<UserEvent>`. Each component
translates events into `Message` variants (`src/elm/message.rs`),
which go through the `update` function (`src/elm/update.rs`) to
produce state transitions plus an optional `Command` side effect
(`src/elm/command.rs`). Real signature verified at
`src/elm/update.rs:33`:

```rust
pub fn update(model: &mut Model, msg: Message) -> Option<Command>
```

`Command` is **non-generic** (see `src/elm/command.rs:15`) and
`update` returns `Option<Command>`, not `Vec<Command<C>>`. Earlier
drafts of this doc (27615dd / 6612d58 trace) had the function
return a vector of ciphersuite-generic commands — neither is
accurate. Per-round DKG / signing orchestration that IS
ciphersuite-generic lives in `InternalCommand<C>`
(`src/utils/state.rs:104,113`); the Elm command and the
protocol-round command are two separate enums.

There is also no standalone `UIEvent` enum — that was a
fabrication in earlier drafts of this doc.

### Rendering Pipeline

1. **Poll tui-realm**: `app.tick(PollStrategy::Once)` pulls
   pending crossterm events.
2. **Route to component**: Active component's `Component::on` handles
   the event and optionally returns a `Message`.
3. **Update model**: `update(&mut model, Message) -> Option<Command>`
   mutates pure state and optionally emits one side effect.
4. **Execute commands**: the returned `Command` runs as an async
   task (WebSocket send, keystore I/O, etc.) that eventually feeds
   a new `Message` back into the queue.
5. **Draw**: tui-realm calls `Component::view` on the active
   screen, Ratatui flushes to the terminal.

## Network Layer

### WebSocket client

The signal-server WebSocket client lives in `src/elm/ws_runtime.rs`
(and accompanying `src/network/` helpers). No named `WebSocketClient`
struct — earlier drafts of this doc claimed a specific public
struct with `reconnect_strategy` / `message_handler` fields that
don't exist. The real flow:

- Connection bootstrapped by `Command::ReconnectWebSocket`
  (`command.rs:26`). There is intentionally no separate
  `ConnectWebSocket` variant — the comment at `command.rs:23`
  explicitly notes that `ReconnectWebSocket` covers the
  first-connect path too. Earlier drafts of THIS bullet cited
  `Command::ConnectWebSocket`; the variant doesn't exist
  (grep returns zero hits).
- Inbound messages (`ServerMsg` envelopes — see
  `apps/signal-server/server/src/lib.rs` for the enum) decode
  back to `Message` variants.
- Outbound messages are `ClientMsg` envelopes. The Elm loop
  emits them through specific Command variants
  (`SendNetworkMessage`, `BroadcastMessage`, `StartDKG`,
  `StartSigning`, etc.) that internally wrap the ClientMsg
  before pushing into the WS sink. There is no umbrella
  `Command::SendWs*` family — earlier drafts of this bullet
  claimed one.

### WebRTC Mesh

The runtime mesh lives across `src/network/webrtc.rs` (low-level
helpers) + `src/elm/webrtc_signaling.rs` (Elm-loop WebRTC driver).
Real peer connections are created at these two sites; they each
call `RTCConfiguration { ice_servers: vec![], .. }` today (see the
STUN gap noted under § Security Architecture and the follow-up
work in the root README Roadmap).

Peer connections live on `AppState<C>.device_connections` (see the
State Management section) — an
`Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>` — alongside
`data_channels`, `device_statuses`, and `pending_ice_candidates`
tables.

Earlier drafts of this section cited `src/webrtc/mesh_manager.rs`
as the main production mesh manager. That module + its sibling
`connection_monitor.rs` / `rejoin_coordinator.rs` /
`mesh_simulator.rs` are **not wired into the Elm runtime** — they
exist as a standalone test-harness library consumed by
`apps/tui/examples/webrtc_mesh_e2e_test.rs` (the integration
example that exercises full-mesh form / disconnect / rejoin
scenarios in-process). The production runtime doesn't import
`WebRTCMeshManager` — it builds RTCPeerConnection objects
directly from the webrtc crate in the two files above.

**Mesh formation**:
1. Signal server relays session announcements + discovery
2. Per-peer SDP offer/answer exchanged through `Relay` envelopes
3. ICE candidates exchanged over `Relay` during gathering
4. Data channels open per-peer; `MeshStatus::Ready` fires once all
   peers are connected; DKG/signing ceremony starts

### Offline Data Transfer

`src/offline/` (`types.rs`, `export.rs`, `import.rs`, `session.rs`)
implements the SD-card air-gap mode. No named `OfflineHandler`
struct — the export/import functions work over JSON bundles
read/written to whatever path the user selects. Coordinator and
participants exchange a handful of round-specific files; the full
procedure is in [`../guides/offline-mode.md`](../guides/offline-mode.md)
and [`../OFFLINE_DKG_GUIDE.md`](../OFFLINE_DKG_GUIDE.md).

## Cryptographic Core

### FROST Protocol Implementation

This crate does NOT define its own `FrostProtocol<C>` type — all
DKG and signing primitives come from the upstream ZCash Foundation
`frost-core 2.2` crate family. The TUI wraps them in:

- `src/protocal/dkg.rs` — DKG orchestration (state machine driving
  `dkg::part1` → `part2` → `part3`)
- `src/protocal/signing.rs` — signing orchestration
  (`round1::commit`, `round2::sign`, `aggregate`)
- `src/protocal/dkg_coordinator.rs` — round-level helpers
- FROST state for an in-flight ceremony lives on `AppState<C>`:
  `dkg_round1_packages`, `dkg_round2_packages`, `key_package`,
  `group_public_key`, `frost_commitments`, `frost_signature_shares`,
  `frost_nonces`.

**Protocol rounds** (as orchestrated by the Rust types here, not
the underlying FROST math):

1. DKG round 1: broadcast `part1` Package to all peers
2. DKG round 2: unicast per-peer `part2` Package to each recipient
3. DKG finalize: local `part3` to compute `KeyPackage` + group key
4. Signing round 1: broadcast `SigningCommitments`
5. Signing round 2: compute + broadcast `SignatureShare`
6. Aggregate: combine shares into the final `Signature` (verified
   automatically inside `frost_core::aggregate`)

### Keystore Architecture

Secure storage for cryptographic materials. Real types live in
`src/keystore/`:

```rust
// src/keystore/storage.rs:17
pub struct Keystore {
    base_path: PathBuf,        // ~/.frost_keystore
    device_id: String,         // derived from device_name passed to new()
    device_name: String,       // same value as device_id today
    wallet_cache: Vec<WalletMetadata>,
}
```

Earlier drafts of this section listed a `Keystore { encryption_key,
wallets: HashMap, metadata }` struct and a wrapper `EncryptedWallet`
type — neither exist. The wallets themselves live as one JSON
file per wallet on disk (partitioned by device_id + curve — see
the § Storage System section below for the directory tree); the
`Keystore` struct just caches `WalletMetadata` and dispatches
reads/writes to those files. An earlier wording called this a
"split files" layout which was ambiguous — to be clear: each
wallet is a SINGLE file; "split" here means split ACROSS the
`<device_id>/<curve>/` directory tree, not split between
`.json` and `.dat` (no `.dat` file exists).

**Encryption scheme** (see `src/keystore/encryption.rs`):

- Key derivation: PBKDF2-HMAC-SHA256, 100_000 iterations
  (`PBKDF2_ITERATIONS` constant) — or Argon2id, selectable per
  wallet via the `algorithm` field in `WalletFile`
- Encryption + authentication: AES-256-GCM (the GCM tag is the MAC —
  no separate HMAC layer, matching the tech-doc fix in 8016f1a)
- On-disk shape: a single JSON file per wallet wrapping plaintext
  metadata AND the base64-encoded encrypted-share blob inside a
  `WalletFile` struct (see `src/keystore/models.rs:438-453`).
  Earlier drafts of this doc claimed a `salt(16) | nonce(12) |
  ciphertext | gcm_tag(16)` "wire format on disk for each `.dat`
  blob"; there is **no** separate `.dat` file on disk — the
  ciphertext is base64-encoded inside the JSON's `data` field, and
  the framing (salt + nonce) is internal to the
  `encrypt_data_with_method` helper in `encryption.rs` rather than
  being a visible on-disk layout.
- Backup format: round-trips with the browser extension's keystore
  (`src/keystore/extension_compat.rs` handles the import/export
  wrapping)

## Storage System

### Directory Structure

Partitioned by device_id and curve (see `src/keystore/storage.rs`,
`save_wallet_file_v2_with_method` at lines 216-247):

```
~/.frost_keystore/
├── index.json                    # Legacy wallet index (migration-only)
├── device_id                     # This node's device_id
└── <device_id>/
    ├── ed25519/
    │   └── <wallet_id>.json      # WalletFile (plaintext metadata +
    │                             # base64 encrypted share in `data`)
    └── secp256k1/
        └── <wallet_id>.json      # same format
```

Earlier drafts of this diagram showed two files per wallet —
`<wallet_id>.json` for metadata + `<wallet_id>.dat` for the raw
encrypted share. That split never shipped; everything is in the
single `.json` document, and the `.dat` convention was fabricated.

The TUI currently has no config-file, session-history, log-archive, or
automated-backup functionality — all runtime config goes through CLI
flags (see `apps/tui/src/bin/starlab-tui.rs`), and logs stream
to the path passed via `--log-location`.

### Data Persistence

There is no `StorageBackend` trait or pluggable-backend abstraction —
the `Keystore` struct talks to the filesystem directly via
`std::fs`. Earlier drafts of this section sketched a trait with
`FileSystemBackend / MemoryBackend / RemoteBackend` implementations;
none exist in source (grep: zero hits).

Real persistence surface is a handful of `Keystore` methods
(verified against `src/keystore/storage.rs`):

- `Keystore::new(base_path, device_name) -> Result<Self>` —
  opens (or creates) the per-device directory tree
- `Keystore::create_wallet(…)` / `create_wallet_multi_chain(…)` —
  serialize metadata + the encrypted share to disk in one pass,
  returning the new wallet_id
- `Keystore::load_wallet_file(wallet_id, password) -> Result<Vec<u8>>` —
  reads the single `<wallet_id>.json` file, extracts the base64
  ciphertext from the `data` field of the `WalletFile` wrapper,
  and decrypts it back to raw bytes (no separate `.dat` file —
  everything lives in the JSON)
- `Keystore::list_wallets() -> Vec<&WalletMetadata>` — scans the
  cached metadata
- `Keystore::get_wallet(wallet_id) -> Option<&WalletMetadata>` —
  metadata-only lookup (no decryption)

Earlier drafts of this list invented `save_wallet` / `load_wallet`
/ `remove_wallet` method names that don't exist — wallet writes
go through `create_wallet` / `create_wallet_multi_chain`, reads
through `load_wallet_file`. No delete method is exposed on the
public API today.

## Security Architecture

### Threat Model

1. **Network Adversary**: Can observe and modify network traffic
2. **Compromised Participant**: One or more malicious participants
3. **Local Malware**: Malicious software on user's machine
4. **Physical Access**: Attacker with device access

### Security Measures

#### Cryptographic Security
- FROST protocol (via upstream `frost-core 2.2`) provides
  `t`-of-`n` threshold security
- No single party ever holds the complete private key
- Signatures require threshold participation; `aggregate` verifies
  the result against the group public key before returning

#### Network Security
- TLS for all WebSocket connections (`wss://`)
- DTLS for WebRTC data channels — peer-to-peer traffic is
  end-to-end encrypted, the signal server is blind to payload
  content once the mesh is up
- **No** certificate pinning — earlier drafts claimed this;
  verified absent from source. The signal-server URL is just a
  standard `wss://` endpoint trusted via the system CA store

#### Local Security
- Keystore encryption at rest (PBKDF2 + AES-256-GCM, 100k iterations)
- Secure random via `rand_core::OsRng` + `rand_chacha::ChaCha20Rng`
- **No** systematic memory zeroization — only
  `frost-core/src/root_secret.rs` zeros on drop, and it does so
  via a manual `self.0.fill(0)` inside `impl Drop`
  (`root_secret.rs:62-67`), NOT via the `zeroize` crate (which is
  not a workspace dependency). Key shares and decrypted keystore
  blobs are not zeroed on drop (open hardening work, matching the
  d854239 fix elsewhere)

#### Operational Security
- Offline mode for air-gapped signing (`--offline` CLI flag)
- **No** session timeouts, audit logs, or operation history —
  earlier drafts claimed these; none exist in source. See the
  `SECURITY.md` sibling for the full honest surface

### Security Boundaries

```
┌─────────────────────────────────────┐
│         Untrusted Zone              │
│  - Network Communication            │
│  - Signal Server                    │
│  - Other Participants               │
├─────────────────────────────────────┤
│      Trust Boundary                 │
├─────────────────────────────────────┤
│         Trusted Zone                │
│  - Local Keystore                   │
│  - FROST Protocol Core              │
│  - UI Event Handler                 │
└─────────────────────────────────────┘
```

## Performance Considerations

### Real optimizations in source

- **Async I/O**: All networking is tokio-based and non-blocking.

That's it, concretely. Earlier drafts of this section (including
my own 7febf90 commit) listed three additional optimizations:

  - `adaptive_event_loop.rs` — adaptive poll-interval ramp
  - `channel_config.rs` — bounded mpsc channels
  - `protocal/session_handler.rs` — deterministic session derivation

**None of those files exist** in `apps/tui/src/` (verified
by `ls`/`find`). No type named `AdaptiveEventLoop`,
`ChannelConfig`, or `UpdateStrategy` is defined anywhere in the
workspace. Those claims originated in the archived
`docs/archive/dev-journal/PERFORMANCE_OPTIMIZATIONS.md` dev-journal
which described work that was planned but never landed — and I
propagated them as real while fixing other docs. Correcting now.

Also still removed from even earlier drafts: connection-pooling,
message-batching, a `ResourceManager` struct, and specific
benchmark targets (< 5 s DKG / < 2 s signing / < 50 ms UI /
< 500 MB peak memory).

Real opportunities for perf work:

- Measure actual idle vs active CPU usage and decide if an
  adaptive poll loop is warranted.
- Audit `mpsc::unbounded_channel` call sites and switch to bounded
  channels where queue growth could matter.
- Add `criterion` benches for DKG / signing / keystore paths so
  future optimizations have a reproducible baseline.

## Extension Points

This section sketches FUTURE extension surfaces. None of the
traits / types below exist in current source — they're design
proposals listed here so future contributors have a starting
point rather than having to rederive the shape. For what
actually ships, see the Module Organization section earlier in
this doc + the parent [`./README.md`](./README.md).

### Plugin System (proposal)

A potential `WalletPlugin` trait could let third parties add new
chains without modifying core crates:

```rust
// PROPOSAL — not implemented
pub trait WalletPlugin {
    fn name(&self) -> &str;
    fn supported_chains(&self) -> Vec<Blockchain>;
    fn create_transaction(&self, params: TxParams) -> Result<Transaction>;
    fn verify_address(&self, address: &str) -> Result<bool>;
}
```

Today, adding a chain requires hand-editing
`packages/@starlab/blockchain/src/*.rs` (see 5fd8378 for the
multi-L2 support precedent).

### Custom UI Themes (proposal)

No theming layer exists. Colors and borders are hardcoded in the
per-screen components (e.g. `main_menu.rs` uses `Color::Cyan` for
high-priority items). A `Theme { colors, borders, symbols }`
struct is a reasonable design but not in source.

### Protocol Extensions (proposal)

Hypothetical work, none underway:

- Additional curves beyond ed25519 / secp256k1 (would need
  `FrostCurve` impl in `packages/@starlab/core`)
- Custom threshold schemes (FROST share refresh, accountable
  threshold signatures, etc.)
- Hardware-wallet co-signing

### Integration APIs (proposal)

No REST API exists. The TUI is a local binary that talks to a
signal server over WebSocket; there's no HTTP surface for
external automation. A possible future API trait:

```rust
// PROPOSAL — not implemented
pub trait ExternalAPI {
    fn create_wallet(&self, params: WalletParams) -> Result<String /* wallet_id */>;
    fn sign_transaction(&self, wallet_id: &str, tx: Transaction) -> Result<Signature>;
    fn get_wallet_info(&self, wallet_id: &str) -> Result<WalletInfo>;
}
```

Note the plain `String` wallet IDs — no `WalletId` or `SessionId`
newtype exists in source; both are plain strings in
`src/elm/model.rs`. Exception: `PeerId` IS a real type, but it's
a `u16` alias (`pub type PeerId = u16;` in
`src/webrtc/mesh_manager.rs:9`) used inside the mesh **test-harness**
library for compact peer addressing — distinct from the String
`device_id` the Elm layer uses for cross-context identity. The
production runtime uses Strings throughout; the u16 PeerId is a
test-harness concept.

## Development Guidelines

### Module Organization

See the TUI file-structure appendix in
[`../MPC_WALLET_TUI_ARCHITECTURE.md`](../MPC_WALLET_TUI_ARCHITECTURE.md)
§ Appendix B for the authoritative layout (fixed in commit 4345c59
to drop the earlier `app_runner.rs` / `handlers/` / `ui/tui.rs`
tree that predated the Elm-architecture migration). In short:

- `src/bin/starlab-tui.rs` — CLI entry
- `src/elm/` — Elm-architecture app shell + per-screen components
- `src/core/` — long-lived managers reused by native-node
- `src/protocal/` — wire types + DKG/signing state machines
  (note: intentional misspelling)
- `src/webrtc/` — standalone mesh **test harness** (not wired
  into the Elm runtime; used by `examples/webrtc_mesh_e2e_test.rs`).
  Real production RTCPeerConnection creation happens in
  `src/network/webrtc.rs` + `src/elm/webrtc_signaling.rs`.
- `src/network/` — WebSocket client helpers
- `src/keystore/` — encrypted share I/O
- `src/offline/` and `src/hybrid/` — air-gap + mixed-mode
- `src/utils/` — AppState, erc20_encoder, eth_helper, …
- `src/lib.rs` — re-exports consumed by native-node

### Error Handling

The real error-type landscape uses `thiserror`-derived per-domain
enums. Verified live types (no top-level `src/errors.rs` umbrella
file exists):

```rust
// src/keystore/mod.rs:24
pub enum KeystoreError { /* variants */ }
// src/keystore/frost_keystore.rs:19
pub enum FrostKeystoreError { /* variants */ }
// src/offline/mod.rs:24
pub enum OfflineError { /* variants */ }
// src/core/mod.rs:21
pub enum CoreError { /* variants */ }

// Upstream, from packages/@starlab/core:
pub enum FrostError {
    SigningError(String),
    /* plus other crypto-operation variants */
}
```

Earlier drafts of this section referenced `src/errors.rs` as a
central errors module + `DkgError` / `SigningError` /
`ComponentError` / `CryptoError` as local enums — none of those
exist in starlab-client (`src/errors.rs` never landed despite being
planned in the archived PHASE1 docs). The error story ended up
per-domain rather than centralised; "SigningError" specifically
is a variant of upstream `FrostError`, not a local enum.

Earlier drafts also sketched a `WalletError` umbrella enum with
`Network`, `Crypto`, `Storage`, `InvalidOperation` variants.
That enum doesn't exist — see tech-doc § Error Codes for the
consolidated story.

### Testing Strategy

1. **Unit Tests**: Individual component testing — ✅ real,
   via inline `#[cfg(test)]` modules + `cargo test --lib`.
2. **Integration Tests**: Multi-component interaction — ✅ real,
   `apps/tui/tests/update_transitions.rs` (88 tests over
   the Elm update function) + `component_rendering.rs` (13 tests
   over ratatui render output).
3. **Protocol Tests**: FROST protocol compliance — ⚠ upstream.
   The FROST primitives themselves are tested in the ZCash
   `frost-core 2.2` crate; this workspace doesn't duplicate
   those tests. Integration-style DKG/signing coverage comes
   through the `examples/` binaries rather than a dedicated
   protocol-test harness here.
4. **UI Tests**: Terminal UI behavior — ✅ real, via
   `tests/component_rendering.rs` using
   `ratatui::backend::TestBackend` snapshot assertions.
5. **Security Tests**: Penetration testing scenarios — ❌ no
   such harness ships (zero `pen_test` / `fuzz` / `proptest`
   dev-deps). Earlier drafts listed this as if implemented.
   Future work; no committed timeline.

### Future Enhancements

1. **Hardware Security Module Support**
   - Integration with HSMs for key storage
   - PKCS#11 interface support

2. **Multi-Protocol Support**
   - Additional threshold signature schemes
   - Post-quantum cryptography preparation

3. **Enterprise Features**
   - LDAP/Active Directory integration
   - Compliance reporting
   - Advanced audit trails

4. **Cloud Integration**
   - Encrypted cloud backup
   - Multi-device synchronization
   - Remote signing capabilities