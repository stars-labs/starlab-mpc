# WebRTC Mesh Network E2E Test Design

## Overview

This document outlines the comprehensive E2E test for WebRTC mesh network establishment, offline mode simulation, and participant rejoin functionality. The test simulates real-world network conditions including disconnections, reconnections, and degraded connectivity.

## Architecture

```
Initial State (Full Mesh):
    Alice (P1)
    ╱     ╲
   ╱       ╲
Bob (P2)━━━Charlie (P3)

Network Partition:
    Alice (P1)
    ╱     ❌
   ╱       
Bob (P2)━━━Charlie (P3)

After Rejoin:
    Alice (P1)
    ╱     ╲
   ╱       ╲
Bob (P2)━━━Charlie (P3)
```

## Test Scenarios

### 🌐 Scenario 1: WebRTC Mesh Establishment

**Setup:**
- 3 participants: Alice, Bob, Charlie
- Threshold: 2-of-3
- Full mesh topology required

**Flow:**
1. **Signaling Phase**
   - All participants connect to signaling server
   - Exchange SDP offers/answers
   - ICE candidate gathering

2. **Mesh Formation**
   - P1 ↔ P2 connection established
   - P1 ↔ P3 connection established  
   - P2 ↔ P3 connection established
   - Verify full mesh connectivity

3. **Data Channel Setup**
   - Create reliable ordered channels
   - Create unreliable unordered channels (for performance)
   - Test message routing

### 📡 Scenario 2: Connection Quality Degradation

**Network Conditions:**
- Latency: 50ms → 500ms → 50ms
- Packet loss: 0% → 10% → 30% → 0%
- Bandwidth: 10Mbps → 1Mbps → 10Mbps

**Expected Behavior:**
- Messages queued during high latency
- Retransmission on packet loss
- Graceful degradation of service
- Recovery when conditions improve

### 🔌 Scenario 3: Participant Disconnection

**Test Cases:**

#### Case A: Planned Disconnect
1. Charlie announces departure
2. Cleanup connections gracefully
3. Alice & Bob continue (still meet threshold)
4. Complete signing with 2/3

#### Case B: Sudden Disconnect (Network Failure)
1. Charlie loses network suddenly
2. WebRTC detects via heartbeat timeout
3. Mesh reconfigures to P1 ↔ P2
4. Continue operations without P3

#### Case C: Below Threshold
1. Both Bob and Charlie disconnect
2. Alice alone (below 2/3 threshold)
3. Operations suspended
4. Wait for participants to return

### 🔄 Scenario 4: Participant Rejoin

**Rejoin Flow:**

1. **Detection Phase**
   - Participant comes back online
   - Connects to signaling server
   - Announces availability

2. **Authentication**
   - Verify participant identity
   - Check session validity
   - Validate key material

3. **Mesh Reintegration**
   - Re-establish WebRTC connections
   - Sync missed messages
   - Update participant status

4. **State Recovery**
   - Catch up on missed rounds
   - Receive any pending data
   - Resume participation

### 🎯 Scenario 5: DKG with Disconnections

**Test Flow:**
1. Start DKG with 3 participants
2. During Round 1: Charlie disconnects
3. Alice & Bob complete Round 1
4. Charlie rejoins before Round 2
5. All complete Round 2 together
6. Verify consistent key generation

### ✍️ Scenario 6: Signing with Rejoin

**Test Flow:**
1. Start signing with Alice & Bob (Charlie offline)
2. Generate commitments
3. Charlie rejoins mid-signing
4. Decision: Continue without or restart with Charlie
5. Complete signature

### 🌊 Scenario 7: Network Partition (Split Brain)

**Partition Scenarios:**

#### Scenario A: 2-1 Split
- Group 1: Alice + Bob (can sign)
- Group 2: Charlie alone (cannot sign)
- Resolution: Group 1 continues

#### Scenario B: 1-1-1 Split
- All participants isolated
- No group meets threshold
- Wait for network healing

### 📊 Scenario 8: Stress Testing

**Load Testing:**
- 100 messages/second
- Large message sizes (1MB)
- Rapid connect/disconnect cycles
- Concurrent operations

## Implementation Components

### 1. WebRTC Mesh Manager

Real struct at `apps/tui-node/src/webrtc/mesh_manager.rs:136`
with 6 fields — HashMaps live under `Arc<Mutex<...>>` per the
interior-mutability pattern (same as `ConnectionMonitor` +
`RejoinCoordinator` below). Real method list at :153-319
includes four send-path methods (`send_message`,
`broadcast_message`, `simulate_network_failure`,
`get_mesh_stats`) that earlier sketches omitted.

```rust
// struct WebRTCMeshManager (:136)
pub struct WebRTCMeshManager {
    pub local_peer: PeerId,
    pub connections: Arc<Mutex<HashMap<PeerId, RTCPeerConnection>>>,
    pub data_channels: Arc<Mutex<HashMap<PeerId, RTCDataChannel>>>,
    pub connection_states: Arc<Mutex<HashMap<PeerId, ConnectionState>>>,
    pub mesh_topology: Arc<Mutex<MeshTopology>>,
    pub message_buffer: Arc<Mutex<HashMap<PeerId, Vec<Vec<u8>>>>>,
}

// impl WebRTCMeshManager — real public methods (line numbers in comments):
impl WebRTCMeshManager {
    pub fn new(local_peer: PeerId,
               total_peers: usize,
               threshold: usize) -> Self;                       // :153
    pub async fn establish_mesh(&mut self,
        peers: Vec<PeerId>) -> Result<(), String>;              // :165
    pub async fn handle_peer_disconnect(&mut self,
        peer: PeerId);                                          // :228
    pub async fn handle_peer_rejoin(&mut self,
        peer: PeerId) -> Result<(), String>;                    // :242
    pub fn get_connected_peers(&self) -> Vec<PeerId>;           // :262
    pub fn is_threshold_met(&self) -> bool;                     // :271
    pub fn send_message(&self, to: PeerId,
        message: Vec<u8>) -> Result<(), String>;                // :276
    pub fn broadcast_message(&self,
        message: Vec<u8>) -> Result<(), String>;                // :296
    pub fn simulate_network_failure(&mut self);                 // :307
    pub fn get_mesh_stats(&self) -> MeshStats;                  // :319
}
```

Earlier sketches of this block had three classes of drift:

  - Bare `HashMap<PeerId, RTCPeerConnection>` fields without
    `Arc<Mutex<...>>` wrapping.
  - Missing the `message_buffer` field and the four send-path
    methods.
  - `establish_mesh(&mut self) -> Result<()>` (no `peers` arg,
    no `String` error type).

### 2. Connection Monitor

Real struct at `apps/tui-node/src/webrtc/connection_monitor.rs:78`
has 5 fields (three of them `Arc<Mutex<...>>`-wrapped for interior
mutability, same pattern as RejoinCoordinator below). `ConnectionQuality`
at `:12` carries RTT / packet-loss / bandwidth / last-heartbeat:

```rust
// struct ConnectionMonitor (:78)
pub struct ConnectionMonitor {
    pub heartbeat_interval: Duration,
    pub timeout_threshold: Duration,
    pub quality_metrics: Arc<Mutex<HashMap<PeerId, ConnectionQuality>>>,
    pub heartbeat_sequences: Arc<Mutex<HashMap<PeerId, u64>>>,
    pub pending_heartbeats: Arc<Mutex<HashMap<(PeerId, u64), Instant>>>,
}

// struct ConnectionQuality (:12)
pub struct ConnectionQuality {
    latency_ms: u32,
    packet_loss_rate: f32,
    bandwidth_kbps: u32,
    last_heartbeat: Instant,
}
```

Earlier sketches showed `quality_metrics: HashMap<PeerId,
ConnectionQuality>` (bare HashMap, no Arc<Mutex<...>>) and
omitted `heartbeat_sequences` / `pending_heartbeats` — same
interior-mutability pattern as `RejoinCoordinator`; mutating
methods all take `&self`, not `&mut self`.

### 3. Rejoin Coordinator

Real struct + impl at `apps/tui-node/src/webrtc/rejoin_coordinator.rs`.
Note: all the internal HashMaps are `Arc<Mutex<...>>`-wrapped
(not bare HashMaps as earlier sketches implied), and all the
mutating methods take `&self` (not `&mut self`) because the
interior mutability lives behind the mutex guards:

```rust
// struct RejoinCoordinator (:103)
pub struct RejoinCoordinator {
    pub session_id: String,
    pub expected_participants: Vec<PeerId>,
    pub threshold: usize,
    pub pending_rejoins: Arc<Mutex<HashMap<PeerId, RejoinRequest>>>,
    pub authenticated_peers: Arc<Mutex<HashMap<PeerId, String>>>,
    pub message_buffer: Arc<Mutex<MessageBuffer>>,
    pub current_round: Arc<Mutex<u8>>,
    // ... plus a few more history/stat fields
}

// impl RejoinCoordinator — real public methods (line numbers in comments):
impl RejoinCoordinator {
    pub fn new(session_id: String,
               participants: Vec<PeerId>,
               threshold: usize) -> Self;                 // :127

    pub async fn handle_rejoin_request(&self,
        request: RejoinRequest) -> RejoinResponse;        // :147

    pub async fn validate_rejoin(&self,
        request: &RejoinRequest) -> bool;                 // :196

    pub async fn sync_participant(&self, peer_id: PeerId); // :235

    pub fn record_message(&self, from: PeerId, round: u8,
                          msg_type: &str, data: Vec<u8>); // :248
    pub fn advance_round(&self);                          // :286
    pub fn get_rejoin_stats(&self) -> RejoinStats;        // :305
}
```

Earlier sketches had three signature errors:

  - `handle_rejoin_request(&mut self, ...)` — real is `&self`
    (and returns `RejoinResponse`, not `()`).
  - `sync_participant(&mut self, peer: PeerId)` — real is `&self,
    peer_id: PeerId`.
  - `validate_rejoin(&self, peer: PeerId) -> bool` — real takes
    `request: &RejoinRequest`, not a bare PeerId.

## Test Metrics

### Connection Metrics
- Time to establish full mesh
- Connection success rate
- Reconnection time after failure
- Message delivery rate

### Performance Metrics
- Message latency (p50, p95, p99)
- Throughput (messages/second)
- CPU usage during mesh operations
- Memory usage with message buffering

### Reliability Metrics
- Mean time between failures (MTBF)
- Mean time to recovery (MTTR)
- Success rate under network stress
- Data consistency after rejoin

## Expected Output

> **Scope note**: the specific millisecond / percentage numbers in
> the illustrative output below (`45ms` / `52ms` / `145ms average`
> / `2.3s rejoin` / `15ms p50 / 45ms p95` / `99.5% uptime`) are
> **stylistic placeholders**, NOT observed benchmark results. No
> `criterion` harness exists in the workspace to produce p50/p95
> latency stats, no uptime tracker, no throughput benchmark. The
> `examples/webrtc_mesh_e2e_test.rs` runner prints phase
> progression and a final pass/fail summary but does NOT publish
> these performance aggregates. Read the numbers as shape-of-
> output, not benchmark-of-record.

```
🚀 WebRTC Mesh Network E2E Test
================================

Phase 1: Mesh Establishment
✅ Alice connected to signaling server
✅ Bob connected to signaling server
✅ Charlie connected to signaling server
✅ P1 ↔ P2 WebRTC connection established
✅ P1 ↔ P3 WebRTC connection established
✅ P2 ↔ P3 WebRTC connection established
✅ Full mesh topology achieved

Phase 2: Connection Quality (simulated conditions)
✅ Normal
⚠️ Degraded (simulated high latency / packet loss)
✅ Recovered: Back to normal

Phase 3: Disconnection Handling
✅ Charlie disconnected (planned)
✅ Mesh reconfigured: P1 ↔ P2
✅ Threshold still met (2/3)
⚠️ Bob disconnected (sudden)
❌ Below threshold - operations suspended

Phase 4: Rejoin Process
✅ Bob rejoin initiated
✅ Authentication successful (placeholder length-check; see
   § Security in WEBRTC_MESH_IMPLEMENTATION.md for the honest
   scope of authenticate_peer)
✅ WebRTC reconnection complete
✅ State synchronized
✅ Charlie rejoin initiated
✅ Full mesh restored

Phase 5: DKG with Disconnections
✅ Round 1 started with 3 participants
⚠️ Charlie disconnected during Round 1
✅ Alice & Bob completed Round 1
✅ Charlie rejoined
✅ All completed Round 2
✅ Consistent keys generated

Phase 6: Signing with Rejoin
✅ Signing started (2/3)
✅ Charlie rejoined mid-signing
✅ Signature completed successfully

Summary:
========
Total Tests: <n>
Passed: <n>
Warnings: <n>
Failed: <n>
```

Earlier drafts of this block included a "Performance" trailer with
concrete timing figures (`Mesh establishment: 145ms average /
Rejoin time: 2.3s average / Message latency: 15ms p50, 45ms p95 /
Reliability: 99.5% uptime`). All fabricated — removed. Adding real
performance harness + publishing these stats is open future work.

## Security Considerations

### Authentication
- Peer identity verification
- Session token validation
- Prevent man-in-the-middle

### Message Integrity
- Message authentication codes
- Sequence numbers
- Replay attack prevention

### State Consistency
- Merkle tree for state verification
- Consensus on rejoin
- Rollback mechanisms

## Success Criteria

1. **Mesh Establishment**: < 1 second for 3 participants
2. **Disconnection Detection**: < 5 seconds
3. **Rejoin Time**: < 10 seconds
4. **Message Delivery**: > 99% reliability
5. **Threshold Operations**: Continue with minimum participants
6. **State Consistency**: 100% after rejoin
7. **Stress Tolerance**: Handle 100 msg/sec