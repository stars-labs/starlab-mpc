# MPC Wallet TUI Node - Technical Architecture Documentation

## Executive Summary

The MPC Wallet TUI Node is a distributed system implementing threshold signature schemes using FROST (Flexible Round-Optimized Schnorr Threshold) protocol. The architecture follows an event-driven, state machine pattern with WebSocket/WebRTC communication for P2P coordination between nodes. This document provides comprehensive analysis of the state transitions, message flows, and critical architectural patterns.

## Table of Contents

1. [System Architecture Overview](#system-architecture-overview)
2. [State Machine Architecture](#state-machine-architecture)
3. [Message Flow Architecture](#message-flow-architecture)
4. [Critical Issues and Fixes](#critical-issues-and-fixes)
5. [Architectural Recommendations](#architectural-recommendations)

---

## System Architecture Overview

### Core Components

```mermaid
graph TB
    subgraph "External Layer"
        WS[WebSocket Server<br/>Signal Server]
        UI[TUI Provider]
    end
    
    subgraph "Application Core"
        AR[ElmApp&lt;C&gt;<br/>Event Loop]
        AS[AppState&lt;C&gt;<br/>Shared State]
        IC[InternalCommand&lt;C&gt;<br/>Message Queue]
    end
    
    subgraph "Protocol Layer"
        DKG[DKG Protocol]
        SIGN[Signing Protocol]
        MESH[Mesh Network]
    end
    
    subgraph "Network Layer"
        WSH[WebSocket Handler]
        WRH[WebRTC Handler]
        DC[Data Channels]
    end
    
    WS <--> WSH
    UI <--> AR
    AR --> IC
    IC --> AS
    AR --> WSH
    AR --> WRH
    WSH --> MESH
    WRH --> DC
    DC --> DKG
    DC --> SIGN
    AS --> DKG
    AS --> SIGN
```

### Key Design Patterns

1. **Event-Driven Architecture**: All state changes triggered by `InternalCommand` enum variants
2. **Actor Model**: Components communicate via message passing through `mpsc::UnboundedSender<InternalCommand<C>>`
3. **Shared State**: `Arc<Mutex<AppState<C>>>` provides thread-safe state access
4. **Command Pattern**: Each operation encapsulated as an `InternalCommand` variant

---

## State Machine Architecture

### 1. DKG State Machine

The Distributed Key Generation state machine manages the multi-party computation protocol for generating threshold keys.

```mermaid
stateDiagram-v2
    [*] --> Idle: Initial State
    
    Idle --> Round1InProgress: TriggerDkgRound1
    
    Round1InProgress --> Round1Complete: All packages received
    Round1InProgress --> Failed: Error/Timeout
    
    Round1Complete --> Round2InProgress: TriggerDkgRound2
    
    Round2InProgress --> Round2Complete: All packages received
    Round2InProgress --> Failed: Error/Timeout
    
    Round2Complete --> Finalizing: FinalizeDkg
    
    Finalizing --> Complete: Key generation successful
    Finalizing --> Failed: Finalization error
    
    Failed --> Idle: RetryDkg
    Complete --> [*]: Success
```

#### State Definitions

```rust
pub enum DkgState {
    Idle,                // No DKG in progress
    Round1InProgress,    // Generating and exchanging commitments
    Round1Complete,      // All Round 1 packages received
    Round2InProgress,    // Generating and exchanging shares
    Round2Complete,      // All Round 2 packages received
    Finalizing,          // Computing final key shares
    Complete,            // DKG successful, keys generated
    Failed(String),      // DKG failed with reason
}
```

#### State Transition Guards

- **Idle → Round1InProgress**: Requires:
  - `MeshStatus::Ready` (all P2P connections established)
  - `identifier_map.is_some()` (participant identifiers assigned)
  - `session.is_some()` (active session exists)
  - Session type is DKG
  - Participant count equals session total

- **Round1InProgress → Round1Complete**: Automatic when:
  - `received_dkg_packages.len() == session.participants.len()`

- **Round1Complete → Round2InProgress**: Triggered by:
  - `InternalCommand::TriggerDkgRound2`

- **Round2InProgress → Round2Complete**: Automatic when:
  - `received_dkg_round2_packages.len() == session.participants.len()`

### 2. Mesh Network State Machine

The mesh network manages P2P WebRTC connections between all participants.

```mermaid
stateDiagram-v2
    [*] --> Incomplete: Initial
    
    Incomplete --> WebRTCInitiated: InitiateWebRTCConnections
    
    WebRTCInitiated --> PartiallyReady: First channel opens
    
    PartiallyReady --> PartiallyReady: More channels open
    PartiallyReady --> Ready: All channels open & mesh ready signals received
    
    Ready --> Incomplete: Connection lost
    Incomplete --> WebRTCInitiated: Reconnection attempt
```

#### State Definitions

```rust
pub enum MeshStatus {
    Incomplete,                           // No connections established
    WebRTCInitiated,                      // Connection process started
    PartiallyReady {                      // Some connections established
        ready_devices: HashSet<String>,
        total_devices: usize,
    },
    Ready,                                // Full mesh network ready
}
```

#### Critical Mesh Readiness Logic

The mesh becomes ready when:
1. All required participants have joined (`participants.len() == session.total`)
2. All participants have accepted the session
3. WebRTC data channels are open to all other participants
4. All participants have sent `MeshReady` signals

### 3. Session State Lifecycle

Sessions manage the participant coordination and agreement protocol.

```mermaid
stateDiagram-v2
    [*] --> Created: ProposeSession
    
    Created --> Announced: AnnounceSession
    
    Announced --> Accepting: Participants join
    
    Accepting --> Active: Threshold met
    Accepting --> Expired: Timeout
    
    Active --> MeshBuilding: InitiateWebRTCConnections
    
    MeshBuilding --> Ready: Mesh complete
    
    Ready --> DKGActive: Start DKG
    
    DKGActive --> Complete: DKG success
    DKGActive --> Failed: DKG failure
    
    Complete --> [*]: Wallet created
    Failed --> [*]: Session terminated
```

#### Session State Synchronization

Sessions maintain synchronized state through:
- **SessionProposal**: Initial session configuration broadcast
- **SessionResponse**: Accept/reject responses from participants
- **SessionUpdate**: Real-time participant list synchronization
- **SessionJoinRequest**: Late join/rejoin requests

---

## Message Flow Architecture

### 1. External to Internal Message Flow

```mermaid
sequenceDiagram
    participant WS as WebSocket Server
    participant WSH as WebSocket Handler
    participant AR as ElmApp&lt;C&gt;
    participant IC as InternalCommand Queue
    participant H as Handler (DKG/Session/Mesh)
    participant AS as AppState&lt;C&gt;
    
    WS->>WSH: ServerMsg::Relay
    WSH->>WSH: Parse WebSocketMessage
    WSH->>IC: Send InternalCommand
    IC->>AR: Process command
    AR->>H: Delegate to handler
    H->>AS: Update state
    H->>IC: Trigger follow-up commands
```

### 2. Message Type Hierarchy

```
ServerMsg (WebSocket layer)
├── Devices: Online device list
├── Relay: P2P message relay
│   └── WebSocketMessage (Application layer)
│       ├── SessionProposal
│       ├── SessionResponse
│       ├── SessionUpdate
│       ├── SessionJoinRequest
│       └── WebRTCSignal
│           ├── Offer
│           ├── Answer
│           └── Candidate
├── SessionAvailable: Discovery announcement
├── SessionsForDevice: Active sessions query
└── SessionRemoved: Session termination

WebRTCMessage (Data channel layer)
├── ChannelOpen: Connection established
├── MeshReady: Participant ready signal
├── DkgRound1Package: Commitment exchange
├── DkgRound2Package: Share exchange
├── SigningRequest: Transaction signing
├── SigningCommitment: FROST Round 1
└── SignatureShare: FROST Round 2
```

### 3. Command Dispatch Pattern

```rust
// External message triggers internal command
WebSocketMessage::SessionProposal(proposal) => {
    InternalCommand::ProcessSessionProposal { proposal }
}

// Internal command updates state and triggers follow-up
InternalCommand::AcceptSessionProposal(id) => {
    1. Update session state
    2. Send SessionResponse via WebSocket
    3. Trigger InternalCommand::InitiateWebRTCConnections
}

// Follow-up command continues the flow
InternalCommand::InitiateWebRTCConnections => {
    1. Create P2P connections
    2. Open data channels
    3. Trigger InternalCommand::ReportChannelOpen per connection
}
```

### 4. Critical Message Flows

#### A. Session Creation and Join Flow

```mermaid
sequenceDiagram
    participant Creator as Device A (Creator)
    participant Server as Signal Server
    participant Joiner as Device B (Joiner)
    
    Creator->>Server: ProposeSession
    Server->>Server: Store session
    Server-->>Joiner: SessionAvailable broadcast
    
    Joiner->>Creator: SessionJoinRequest
    Creator->>Creator: Validate request
    Creator->>Joiner: SessionProposal (full details)
    
    Joiner->>Joiner: Accept proposal
    Joiner->>Creator: SessionResponse (accepted=true)
    
    Creator->>Creator: Update participant list
    Creator->>Server: SessionUpdate broadcast
    Server-->>Joiner: SessionUpdate (sync state)
    
    Note over Creator,Joiner: Both devices have synchronized session state
    
    Creator->>Joiner: InitiateWebRTCConnection
    Joiner->>Creator: WebRTC Answer
    
    Note over Creator,Joiner: P2P connection established
```

#### B. DKG Protocol Flow

```mermaid
sequenceDiagram
    participant D1 as Device 1
    participant D2 as Device 2
    participant D3 as Device 3
    
    Note over D1,D3: Mesh network established
    
    D1->>D1: Generate Round1 package
    D1->>D2: DkgRound1Package
    D1->>D3: DkgRound1Package
    
    D2->>D2: Generate Round1 package
    D2->>D1: DkgRound1Package
    D2->>D3: DkgRound1Package
    
    D3->>D3: Generate Round1 package
    D3->>D1: DkgRound1Package
    D3->>D2: DkgRound1Package
    
    Note over D1,D3: All Round1 packages received
    
    D1->>D1: Process & Generate Round2
    D1->>D2: DkgRound2Package
    D1->>D3: DkgRound2Package
    
    D2->>D2: Process & Generate Round2
    D2->>D1: DkgRound2Package
    D2->>D3: DkgRound2Package
    
    D3->>D3: Process & Generate Round2
    D3->>D1: DkgRound2Package
    D3->>D2: DkgRound2Package
    
    Note over D1,D3: All Round2 packages received
    
    D1->>D1: Finalize key generation
    D2->>D2: Finalize key generation
    D3->>D3: Finalize key generation
    
    Note over D1,D3: Each device has key share
```

---

## Critical Issues and Fixes

### Issue 1: Premature DKG Start Bug

**Problem**: DKG was starting with insufficient participants (2/3 instead of 3/3).

**Root Cause**: In `mesh_commands.rs`, the check was using current participant count instead of required total:
```rust
// BUG: Checking current count
if current_count < total_needed {
    // Buffer signal
}
```

**Fix Applied**: Check both participant count AND acceptance status:
```rust
// FIXED: Check both conditions
if current_count < total_needed {
    state_guard.pending_mesh_ready_signals.push(device_id.clone());
    return; // Don't proceed
}

if accepted_count < total_needed {
    state_guard.pending_mesh_ready_signals.push(device_id.clone());
    return; // Don't proceed
}
```

### Issue 2: Session State Desynchronization

**Problem**: Participants list and accepted_devices list becoming inconsistent.

**Root Cause**: Multiple code paths updating these lists independently:
- `SessionProposal` updates participants
- `SessionResponse` updates accepted_devices
- `SessionUpdate` should sync both but only updated accepted_devices

**Fix Applied**: Ensure both lists stay synchronized:
```rust
// In SessionUpdate handler
session.accepted_devices = update.accepted_devices.clone();
session.participants = update.accepted_devices.clone(); // Keep in sync
```

### Issue 3: Race Condition in WebRTC Initiation

**Problem**: Multiple simultaneous `InitiateWebRTCConnections` causing duplicate connections.

**Root Cause**: No debouncing or state tracking for ongoing initiation.

**Fix Applied**: Added initiation tracking:
```rust
pub struct AppState<C> {
    pub webrtc_initiation_in_progress: bool,
    pub webrtc_initiation_started_at: Option<Instant>,
    pub webrtc_offers_in_progress: HashMap<String, Instant>,
}

// Debounce logic
if state_guard.webrtc_initiation_in_progress {
    if started_at.elapsed() < Duration::from_millis(500) {
        return; // Skip duplicate
    }
}
```

### Issue 4: Auto-Join Security Vulnerability

**Problem**: Sessions automatically accepted without user consent.

**Root Cause**: Missing security checks in session acceptance flow.

**Fix Applied**: Require explicit user consent:
```rust
// Only auto-join if:
// 1. Rejoining existing session (disconnection recovery)
// 2. User explicitly requested join
let should_auto_join = is_rejoin || is_actively_joining;

if !should_auto_join {
    tracing::warn!("🔒 Security: Rejecting auto-join - requires consent");
    return;
}
```

---

## Architectural Recommendations

### 1. State Machine Determinism

**Issue**: Current state transitions have implicit dependencies and race conditions.

**Recommendation**: Implement formal state machine with explicit guards:

```rust
pub trait StateMachine {
    type State;
    type Event;
    type Guard;
    
    fn can_transition(&self, from: &Self::State, event: &Self::Event) -> bool;
    fn transition(&mut self, event: Self::Event) -> Result<Self::State>;
    fn guards(&self, state: &Self::State) -> Vec<Self::Guard>;
}

impl StateMachine for DkgStateMachine {
    fn can_transition(&self, from: &DkgState, event: &DkgEvent) -> bool {
        match (from, event) {
            (DkgState::Idle, DkgEvent::Start) => {
                self.mesh_ready() && 
                self.has_identifier_map() && 
                self.session_active()
            }
            // ... other transitions
        }
    }
}
```

### 2. Message Flow Validation

**Issue**: Messages processed without comprehensive validation.

**Recommendation**: Implement message validation pipeline:

```rust
pub trait MessageValidator {
    fn validate_session_state(&self, msg: &WebSocketMessage) -> Result<()>;
    fn validate_participant(&self, device_id: &str) -> Result<()>;
    fn validate_sequence(&self, msg: &WebSocketMessage) -> Result<()>;
}

// Use builder pattern for validation
MessageValidator::new()
    .require_session()
    .require_participant()
    .require_sequence_order()
    .validate(message)?;
```

### 3. Idempotent Command Processing

**Issue**: Commands can be processed multiple times causing state corruption.

**Recommendation**: Track processed commands:

```rust
pub struct CommandLog {
    processed: HashSet<CommandId>,
    in_flight: HashMap<CommandId, Instant>,
}

impl CommandLog {
    pub fn should_process(&mut self, cmd: &InternalCommand) -> bool {
        let id = cmd.id();
        if self.processed.contains(&id) {
            return false; // Already processed
        }
        if let Some(started) = self.in_flight.get(&id) {
            if started.elapsed() < Duration::from_secs(30) {
                return false; // Still processing
            }
        }
        self.in_flight.insert(id, Instant::now());
        true
    }
}
```

### 4. Mesh Network Health Monitoring

**Issue**: No proactive detection of network degradation.

**Recommendation**: Implement heartbeat and health checks:

```rust
pub struct MeshHealth {
    last_heartbeat: HashMap<String, Instant>,
    latency: HashMap<String, Duration>,
    packet_loss: HashMap<String, f32>,
}

impl MeshHealth {
    pub async fn monitor(&mut self) {
        loop {
            for (device, last) in &self.last_heartbeat {
                if last.elapsed() > Duration::from_secs(10) {
                    self.trigger_reconnection(device).await;
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
```

### 5. State Recovery and Persistence

**Issue**: State lost on restart, no recovery mechanism.

**Recommendation**: Implement state snapshots:

```rust
pub trait StateSnapshot {
    fn snapshot(&self) -> Result<Vec<u8>>;
    fn restore(data: &[u8]) -> Result<Self>;
}

impl<C: Ciphersuite> StateSnapshot for AppState<C> {
    fn snapshot(&self) -> Result<Vec<u8>> {
        // Serialize critical state
        let snapshot = StateData {
            session: self.session.clone(),
            dkg_state: self.dkg_state.clone(),
            identifier_map: self.identifier_map.clone(),
            // ... other persistent fields
        };
        bincode::serialize(&snapshot)
    }
}
```

### 6. Event Sourcing for Audit Trail

**Issue**: No audit trail of state transitions and decisions.

**Recommendation**: Implement event sourcing:

```rust
pub struct EventStore {
    events: Vec<StateEvent>,
}

pub enum StateEvent {
    SessionCreated { id: String, timestamp: u64 },
    ParticipantJoined { device: String, timestamp: u64 },
    DkgStarted { round: u8, timestamp: u64 },
    StateTransition { from: String, to: String, reason: String },
}

impl EventStore {
    pub fn append(&mut self, event: StateEvent) {
        self.events.push(event);
        self.persist_to_disk();
    }
    
    pub fn replay(&self) -> AppState {
        // Reconstruct state from events
    }
}
```

---

## Conclusion

The MPC Wallet TUI Node architecture demonstrates a sophisticated distributed system design with clear separation of concerns and event-driven state management. The identified issues primarily stem from:

1. **Race conditions** in concurrent state updates
2. **Implicit state dependencies** without formal guards
3. **Missing validation** in message processing
4. **Lack of idempotency** in command handling

The recommended improvements focus on:
- Formalizing state machines with explicit guards
- Implementing comprehensive validation pipelines
- Adding idempotent command processing
- Improving observability and monitoring
- Implementing state persistence and recovery

These enhancements would significantly improve the system's reliability, security, and maintainability while preserving the elegant event-driven architecture.

---

## Appendix A: Complete State Transition Table

Events below are `InternalCommand<C>` variants (defined in
`apps/tui-node/src/utils/state.rs:29`), which is the
ciphersuite-generic command enum distinct from the newer
`elm/command.rs::Command` (non-generic, used for the higher-level
Elm loop). Both enums coexist — InternalCommand handles the
per-round DKG / signing mechanics; Command handles higher-level
orchestration. See `src/utils/state.rs` vs `src/elm/command.rs`
for the full variant lists.

| Current State | Event | Guard Conditions | Next State | Side Effects |
|--------------|-------|------------------|------------|--------------|
| `DkgState::Idle` | `InternalCommand::TriggerDkgRound1` | `mesh_ready && identifier_map.is_some() && session.is_dkg()` | `Round1InProgress` | Generate Round1 package, broadcast to peers |
| `DkgState::Round1InProgress` | `ProcessDkgRound1` | Package from valid participant | `Round1InProgress` or `Round1Complete` | Store package, check if all received |
| `DkgState::Round1Complete` | `TriggerDkgRound2` | All Round1 packages processed | `Round2InProgress` | Generate Round2 packages, broadcast to peers |
| `DkgState::Round2InProgress` | `ProcessDkgRound2` | Package from valid participant | `Round2InProgress` or `Round2Complete` | Store package, check if all received |
| `DkgState::Round2Complete` | `FinalizeDkg` | All Round2 packages processed | `Finalizing` | Compute key shares |
| `DkgState::Finalizing` | (automatic) | Key computation successful | `Complete` | Store keys, generate addresses |
| `MeshStatus::Incomplete` | `InitiateWebRTCConnections` | Session active, participants available | `WebRTCInitiated` | Create peer connections |
| `MeshStatus::WebRTCInitiated` | `ReportChannelOpen` | Data channel established | `PartiallyReady` | Send ChannelOpen message |
| `MeshStatus::PartiallyReady` | `ProcessMeshReady` | All participants ready | `Ready` | Trigger DKG check |

## Appendix B: Critical File Structure

Real layout — verified against the current tree. Earlier drafts of
this appendix referenced an `app_runner.rs` / `handlers/` / `ui/tui.rs`
scheme that predates the Elm-architecture migration; none of those
paths exist today.

```
apps/tui-node/src/
├── bin/
│   └── mpc-wallet-tui.rs        # clap entry + keystore init + ElmApp bootstrap
├── elm/
│   ├── app.rs                   # ElmApp<C> — main event loop + tui-realm shell
│   ├── model.rs                 # Model (immutable state snapshot)
│   ├── update.rs                # Update fn: Message → state transition + Commands
│   ├── command.rs               # Command enum — side-effect tasks
│   │                             # (non-generic; ciphersuite-generic
│   │                             #  round orchestration uses
│   │                             #  InternalCommand<C> in utils/state.rs)
│   ├── message.rs               # Message enum — input events
│   ├── provider.rs              # UIProvider trait (abstract UI backend)
│   ├── ws_runtime.rs            # WebSocket client runtime
│   ├── webrtc_signaling.rs      # WebRTC signaling over the signal server
│   └── components/              # Per-screen tui-realm Component impls
├── core/                        # Long-lived managers reused by native-node
│                                # (WalletManager / SessionManager / DkgManager /
│                                #  SigningManager / OfflineManager / ConnectionManager)
├── protocal/                    # Wire types (note: intentional misspelling)
│   ├── dkg.rs                   # DKG protocol state machine
│   ├── dkg_coordinator.rs       # Round orchestration helper
│   ├── signing.rs               # Signing protocol state machine
│   ├── signal.rs                # Signal-server message types
│   └── session_types.rs
├── webrtc/                      # Mesh TEST HARNESS — not wired
│   │                            # into the Elm runtime; consumed by
│   │                            # examples/webrtc_mesh_e2e_test.rs
│   ├── mesh_manager.rs          # Simulated full-mesh manager
│   ├── connection_monitor.rs
│   ├── rejoin_coordinator.rs
│   └── mesh_simulator.rs
├── network/
│   ├── webrtc.rs                # Low-level WebRTC helpers — one of
│   │                            # the two REAL production
│   │                            # RTCPeerConnection construction sites
│   └── mod.rs
├── elm/webrtc_signaling.rs      # The other production RTCPeerConnection
│                                # site — the Elm-loop driver that
│                                # handles offer/answer/ICE exchange
├── keystore/
│   ├── storage.rs               # Keystore struct + `.json`/`.dat` I/O
│   ├── encryption.rs            # AES-256-GCM + PBKDF2 100k
│   ├── models.rs                # WalletFile / Metadata structs
│   ├── frost_keystore.rs        # FROST-specific keystore plumbing
│   └── extension_compat.rs      # Browser-extension format interop
├── offline/                     # SD-card air-gap mode
├── hybrid/                      # Online+offline mixed-participant mode
├── utils/
│   ├── state.rs                 # AppState<C> + InternalCommand<C> + DkgState
│   ├── erc20_encoder.rs         # ERC-20 transfer encoding
│   ├── eth_helper.rs            # EIP-191 personal_sign + ecrecover helpers
│   └── …                        # (curve_traits, device, performance, …)
└── lib.rs                       # Re-exports for native-node consumers
```

## Appendix C: Security Considerations (aspirational)

> **Scope note**: This appendix is a wishlist of hardening work,
> not a description of currently-implemented controls. See
> [`architecture/SECURITY.md`](./architecture/SECURITY.md) (rewritten
> in 89e9054 / 6d7fd5a / 333c97f) for the honest accounting of
> what actually ships today. Most items below don't exist in
> source yet — they're listed here so future contributors have a
> starting point rather than having to rederive the list.

### 1. Message Authentication (NOT implemented)
- Signing messages with per-device keys — the signal server is
  currently an unauthenticated relay; DTLS covers transport
  integrity but there's no app-layer MAC.
- Sequence numbers + timestamp validation for replay protection —
  FROST's own per-signing nonce generation prevents signature
  reuse; no additional message-level replay layer ships.

### 2. State Validation
- Validate all state transitions against business rules — partially
  in place via the `DkgState` + `MeshStatus` + `SigningState` enums
  that guard transitions in `utils/state.rs`. Rollback-on-invalid
  is NOT implemented (invalid state transitions currently log and
  continue).

### 3. Network Security
- End-to-end encryption for WebRTC channels — already implemented;
  WebRTC negotiates DTLS-SRTP by default.
- Certificate pinning for WebSocket connections — NOT implemented;
  we trust the system CA store.
- Rate limiting — NOT implemented.

### 4. Key Management (NOT implemented)
- HSM support — no PKCS#11 integration anywhere in source (same
  absent-HSM finding flagged across the cleanup pass: 7febf90 /
  6d7fd5a / f7e0bad / 0363ad2 / 0214b30 / c48fbf0).
- Key rotation — FROST share refresh in principle exists but this
  crate doesn't wire it up.
- Key backup — real path is keystore export (.json/.dat pair);
  "backup and recovery" in the fuller sense doesn't ship.

### 5. Audit and Compliance (NOT implemented)
- No structured audit log — only `tracing` output. Operators ship
  logs through their own pipeline.
- No tamper-evident logging.
- No compliance framework (SOC 2 / ISO 27001 / GDPR). See
  SECURITY.md § Compliance Framework (6d7fd5a) for the full "not
  certified" statement.