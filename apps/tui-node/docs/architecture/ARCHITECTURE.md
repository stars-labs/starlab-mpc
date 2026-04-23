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

No `curve_type`, `wallets: HashMap<…>`, `pending_operations: VecDeque<…>`,
`network_status`, or `offline_mode` fields — those were listed in
earlier drafts of this doc. Curve is per-wallet (lives in the
keystore's `WalletMetadata`), wallets live in the keystore, signing
requests queue through `pending_signing_requests`, and offline-mode
is set at startup via the `--offline` CLI flag (no runtime toggle
field).

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
produce state transitions plus a `Vec<Command<C>>` of side effects
(`src/elm/command.rs`). There is no standalone `UIEvent` enum —
that was a fabrication in earlier drafts of this doc.

### Rendering Pipeline

1. **Poll tui-realm**: `app.tick(PollStrategy::Once)` pulls
   pending crossterm events.
2. **Route to component**: Active component's `Component::on` handles
   the event and optionally returns a `Message`.
3. **Update model**: `update(&mut model, Message) -> Vec<Command<C>>`
   mutates pure state and emits side effects.
4. **Execute commands**: `Command<C>::execute` runs async tasks
   (WebSocket send, keystore I/O, DKG rounds, etc.) that eventually
   feed messages back into the queue.
5. **Draw**: tui-realm calls `Component::view` on the active
   screen, Ratatui flushes to the terminal.

## Network Layer

### WebSocket client

The signal-server WebSocket client lives in `src/elm/ws_runtime.rs`
(and accompanying `src/network/` helpers). No named `WebSocketClient`
struct — earlier drafts of this doc claimed a specific public
struct with `reconnect_strategy` / `message_handler` fields that
don't exist. The real flow:

- Connection bootstrapped by `Command::ConnectWebSocket`
- Inbound messages (`ServerMsg` envelopes — see
  `apps/signal-server/server/src/lib.rs` for the enum) decode
  back to `Message` variants
- Outbound messages (`ClientMsg`) emit via the `Command::SendWs*`
  variants

### WebRTC Mesh

`src/webrtc/mesh_manager.rs` holds the real full-mesh manager. Peer
connections live on `AppState<C>.device_connections` (see the state
section) — an `Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>`
— alongside `data_channels`, `device_statuses`, and `pending_ice_candidates`
tables.

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
// src/keystore/storage.rs
pub struct Keystore {
    base_path: PathBuf,        // ~/.frost_keystore
    device_id: String,
    wallet_cache: Vec<WalletMetadata>,
}
```

Earlier drafts of this section listed a `Keystore { encryption_key,
wallets: HashMap, metadata }` struct and a wrapper `EncryptedWallet`
type — neither exist. The wallets themselves live as split files on
disk (see the Storage System section below); the `Keystore` just
maintains the metadata index and dispatches reads/writes.

**Encryption scheme** (see `src/keystore/encryption.rs`):

- Key derivation: PBKDF2-HMAC-SHA256, 100_000 iterations
  (`PBKDF2_ITERATIONS` constant)
- Encryption + authentication: AES-256-GCM (the GCM tag is the MAC —
  no separate HMAC layer, matching the tech-doc fix in 8016f1a)
- Wire format on disk for each `.dat` blob:
  `salt(16 B) | nonce(12 B) | ciphertext | gcm_tag(16 B)`
- Backup format: round-trips with the browser extension's keystore
  (`src/keystore/extension_compat.rs` handles the import/export
  wrapping)

## Storage System

### Directory Structure

The structure is partitioned by device_id and curve (see
`src/keystore/storage.rs`):

```
~/.frost_keystore/
├── index.json                    # Wallet index (device_id × curve → wallet list)
├── device_id                     # This node's device_id
└── <device_id>/
    ├── ed25519/
    │   ├── <wallet_id>.json      # Wallet metadata (threshold, participants, etc.)
    │   └── <wallet_id>.dat       # Encrypted FROST key share (AES-256-GCM)
    └── secp256k1/
        ├── <wallet_id>.json
        └── <wallet_id>.dat
```

The TUI currently has no config-file, session-history, log-archive, or
automated-backup functionality — all runtime config goes through CLI
flags (see `apps/tui-node/src/bin/mpc-wallet-tui.rs`), and logs stream
to the path passed via `--log-location`.

### Data Persistence

There is no `StorageBackend` trait or pluggable-backend abstraction —
the `Keystore` struct talks to the filesystem directly via
`std::fs`. Earlier drafts of this section sketched a trait with
`FileSystemBackend / MemoryBackend / RemoteBackend` implementations;
none exist in source (grep: zero hits).

Real persistence surface is a handful of `Keystore` methods:

- `Keystore::new(base_path, device_id)` — opens (or creates)
  the per-device directory tree
- `Keystore::save_wallet(metadata, key_share, password)` —
  writes the `.json` + encrypted `.dat` pair
- `Keystore::load_wallet(wallet_id, password)` — reads both
  and decrypts the share
- `Keystore::list_wallets()` — scans the cached metadata
- `Keystore::remove_wallet(wallet_id)` — deletes both files

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
  `frost-core/src/root_secret.rs` uses `zeroize::Zeroize`; key
  shares and decrypted keystore blobs are not zeroed on drop
  (open hardening work, matching the d854239 fix elsewhere)

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

**None of those files exist** in `apps/tui-node/src/` (verified
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
`packages/@mpc-wallet/blockchain/src/*.rs` (see 5fd8378 for the
multi-L2 support precedent).

### Custom UI Themes (proposal)

No theming layer exists. Colors and borders are hardcoded in the
per-screen components (e.g. `main_menu.rs` uses `Color::Cyan` for
high-priority items). A `Theme { colors, borders, symbols }`
struct is a reasonable design but not in source.

### Protocol Extensions (proposal)

Hypothetical work, none underway:

- Additional curves beyond ed25519 / secp256k1 (would need
  `FrostCurve` impl in `packages/@mpc-wallet/frost-core`)
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

Note the plain `String` wallet IDs — no `WalletId` newtype exists
in source. Same for `SessionId`, `PeerId`, etc. (earlier drafts of
this doc used those invented type names; they don't reflect the
real string-typed IDs in `src/elm/model.rs`).

## Development Guidelines

### Module Organization

See the TUI file-structure appendix in
[`../MPC_WALLET_TUI_ARCHITECTURE.md`](../MPC_WALLET_TUI_ARCHITECTURE.md)
§ Appendix B for the authoritative layout (fixed in commit 4345c59
to drop the earlier `app_runner.rs` / `handlers/` / `ui/tui.rs`
tree that predated the Elm-architecture migration). In short:

- `src/bin/mpc-wallet-tui.rs` — CLI entry
- `src/elm/` — Elm-architecture app shell + per-screen components
- `src/core/` — long-lived managers reused by native-node
- `src/protocal/` — wire types + DKG/signing state machines
  (note: intentional misspelling)
- `src/webrtc/mesh_manager.rs` — full-mesh peer manager
- `src/network/` — WebSocket client helpers
- `src/keystore/` — encrypted share I/O
- `src/offline/` and `src/hybrid/` — air-gap + mixed-mode
- `src/utils/` — AppState, erc20_encoder, eth_helper, …
- `src/lib.rs` — re-exports consumed by native-node

### Error Handling

The real error-type landscape uses `thiserror`-derived per-domain
enums:

```rust
// src/errors.rs (+ per-module error types)
pub enum DkgError { /* ... */ }
pub enum SigningError { /* ... */ }
pub enum KeystoreError { /* ... */ }
pub enum ComponentError { /* ... */ }
pub enum CryptoError { /* ... */ }
```

Earlier drafts of this section sketched a `WalletError` umbrella
enum with `Network`, `Crypto`, `Storage`, `InvalidOperation`
variants. That enum doesn't exist — the actual scheme uses the
per-domain types above (see tech-doc § Error Codes for the same
note, removed in 9e9cb19).

### Testing Strategy

1. **Unit Tests**: Individual component testing
2. **Integration Tests**: Multi-component interaction
3. **Protocol Tests**: FROST protocol compliance
4. **UI Tests**: Terminal UI behavior
5. **Security Tests**: Penetration testing scenarios

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