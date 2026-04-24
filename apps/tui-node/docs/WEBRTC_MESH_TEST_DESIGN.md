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
```rust
struct WebRTCMeshManager {
    local_peer: PeerId,
    connections: HashMap<PeerId, RTCPeerConnection>,
    data_channels: HashMap<PeerId, RTCDataChannel>,
    connection_states: HashMap<PeerId, ConnectionState>,
    mesh_topology: MeshTopology,
}

impl WebRTCMeshManager {
    async fn establish_mesh(&mut self) -> Result<()>;
    async fn handle_peer_disconnect(&mut self, peer: PeerId);
    async fn handle_peer_rejoin(&mut self, peer: PeerId);
    fn get_connected_peers(&self) -> Vec<PeerId>;
    fn is_threshold_met(&self) -> bool;
}
```

### 2. Connection Monitor
```rust
struct ConnectionMonitor {
    heartbeat_interval: Duration,
    timeout_threshold: Duration,
    quality_metrics: HashMap<PeerId, ConnectionQuality>,
}

struct ConnectionQuality {
    latency_ms: u32,
    packet_loss_rate: f32,
    bandwidth_kbps: u32,
    last_heartbeat: Instant,
}
```

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

```
🚀 WebRTC Mesh Network E2E Test
================================

Phase 1: Mesh Establishment
✅ Alice connected to signaling server
✅ Bob connected to signaling server
✅ Charlie connected to signaling server
✅ P1 ↔ P2 WebRTC connection established (45ms)
✅ P1 ↔ P3 WebRTC connection established (52ms)
✅ P2 ↔ P3 WebRTC connection established (48ms)
✅ Full mesh topology achieved

Phase 2: Connection Quality
✅ Normal latency: 50ms average
⚠️ Degraded: 500ms latency, 10% packet loss
✅ Recovered: Back to normal

Phase 3: Disconnection Handling
✅ Charlie disconnected (planned)
✅ Mesh reconfigured: P1 ↔ P2
✅ Threshold still met (2/3)
⚠️ Bob disconnected (sudden)
❌ Below threshold - operations suspended

Phase 4: Rejoin Process
✅ Bob rejoin initiated
✅ Authentication successful
✅ WebRTC reconnection complete (2.3s)
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
Total Tests: 25
Passed: 23
Warnings: 2
Failed: 0

Performance:
- Mesh establishment: 145ms average
- Rejoin time: 2.3s average
- Message latency: 15ms p50, 45ms p95
- Reliability: 99.5% uptime
```

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