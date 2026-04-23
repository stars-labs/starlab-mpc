# End-to-End Test Implementation Plan

## Status: Plan only — not yet implemented

Everything below is a design proposal for future work. No
`TestRunner` struct exists in the tree (verified via grep —
zero hits for `struct TestRunner` / `impl TestRunner`), and the
per-priority checklists that mark items as "DONE" with ✅
overstate the current state. Treat the rest of this document as
a starting point for someone who wants to build the harness,
not as a reference for tests that already exist.

Existing functional coverage that does real MPC flows (without
a dedicated E2E harness): see the examples at
`apps/tui-node/examples/hybrid_mode_e2e_test.rs` +
`webrtc_mesh_e2e_test.rs` + the Bun test suite under
`apps/browser-extension/tests/entrypoints/background/`. Those are
the real de facto E2E baselines today.

## Overview
Comprehensive test suite for MPC Wallet using real WebSocket signal server (wss://xiongchenyu.dpdns.org)

## Priority 1: Core Functionality Tests (Week 1)

### Basic DKG Flow Tests
```rust
// test_2of2_dkg_happy_path.rs
#[tokio::test]
async fn test_2of2_dkg_with_real_websocket() {
    let mut node_a = TestRunner::new("alice").await;
    let mut node_b = TestRunner::new("bob").await;
    
    node_a.start().await.unwrap();
    node_b.start().await.unwrap();
    
    // Create and join session
    let session_id = node_a.create_session(2, 2, vec!["alice", "bob"]).await.unwrap();
    node_b.join_session(&session_id).await.unwrap();
    
    // Wait for mesh formation
    wait_for_mesh_ready(&[&node_a, &node_b]).await;
    
    // Execute DKG
    node_a.start_dkg().await.unwrap();
    wait_for_dkg_complete(&[&node_a, &node_b]).await;
    
    // Verify addresses match
    assert_eq!(node_a.get_address().await, node_b.get_address().await);
}
```

### Implementation Tasks:
1. ⬜ Create `TestRunner` infrastructure (earlier drafts marked
   this ✅ DONE; the struct doesn't actually exist in source)
2. ⬜ Add `wait_for_mesh_ready()` helper
3. ⬜ Add `wait_for_dkg_complete()` helper
4. ⬜ Implement address verification
5. ⬜ Add timeout handling

## Priority 2: Network Resilience Tests (Week 2)

### Disconnect/Reconnect Scenarios
```rust
// test_participant_disconnect.rs
#[tokio::test]
async fn test_disconnect_during_dkg() {
    // Setup 3-node session
    let nodes = setup_nodes(&["alice", "bob", "charlie"], 2, 3).await;
    
    // Start DKG
    nodes[0].start_dkg().await.unwrap();
    sleep(Duration::from_millis(500)).await;
    
    // Disconnect charlie
    nodes[2].disconnect().await;
    
    // Verify DKG continues with threshold participants
    wait_for_dkg_complete(&nodes[0..2]).await;
    
    // Reconnect charlie
    nodes[2].reconnect().await.unwrap();
    
    // Verify charlie can still participate in signing
    let signature = sign_with_nodes(&nodes, "test message").await;
    assert!(verify_signature(signature));
}
```

### Implementation Tasks:
1. ⬜ Implement controlled disconnect/reconnect
2. ⬜ Add network partition simulation
3. ⬜ Create message loss injection
4. ⬜ Add latency simulation
5. ⬜ Implement automatic recovery verification

## Priority 3: Security Tests (Week 3)

### Attack Scenario Tests
```rust
// test_malicious_participant.rs
#[tokio::test]
async fn test_invalid_dkg_data() {
    let mut honest_nodes = setup_nodes(&["alice", "bob"], 2, 3).await;
    let mut malicious = MaliciousNode::new("mallory").await;
    
    // Mallory sends invalid DKG round 1 data
    malicious.send_invalid_round1_data().await;
    
    // Honest nodes should detect and exclude
    let result = execute_dkg_with_validation(&honest_nodes).await;
    assert!(result.excluded_participants.contains("mallory"));
    
    // DKG completes without malicious participant
    assert_eq!(result.final_threshold, 2);
    assert_eq!(result.final_participants, 2);
}
```

### Implementation Tasks:
1. ⬜ Create MaliciousNode test helper
2. ⬜ Implement various attack vectors
3. ⬜ Add replay attack simulation
4. ⬜ Test invalid signature requests
5. ⬜ Verify proper error handling

## Priority 4: Concurrent Operations (Week 4)

### Multiple Sessions Test
```rust
// test_concurrent_sessions.rs
#[tokio::test]
async fn test_multiple_simultaneous_sessions() {
    let alice = TestRunner::new("alice").await;
    
    // Alice participates in 3 different sessions
    let session1 = alice.create_session(2, 3, vec!["alice", "bob", "charlie"]).await;
    let session2 = alice.join_session("existing-session-123").await;
    let session3 = alice.create_session(3, 5, vec!["alice", "dave", "eve", "frank", "grace"]).await;
    
    // Run DKG on all simultaneously
    let handles = vec![
        alice.start_dkg_for_session(session1),
        alice.start_dkg_for_session(session2),
        alice.start_dkg_for_session(session3),
    ];
    
    // All should complete independently
    for handle in handles {
        assert!(handle.await.is_ok());
    }
    
    // Verify proper isolation
    assert_ne!(alice.get_key_for_session(session1), alice.get_key_for_session(session2));
}
```

## Priority 5: Performance Tests (Week 5)

### Stress Testing
```rust
// test_high_load.rs
#[tokio::test]
async fn test_100_concurrent_signatures() {
    let nodes = setup_nodes(&["alice", "bob", "charlie"], 2, 3).await;
    complete_dkg(&nodes).await;
    
    // Submit 100 concurrent signing requests
    let mut handles = vec![];
    for i in 0..100 {
        let msg = format!("message-{}", i);
        handles.push(sign_async(&nodes, msg));
    }
    
    // Measure performance
    let start = Instant::now();
    let results: Vec<_> = futures::future::join_all(handles).await;
    let duration = start.elapsed();
    
    // Verify all signatures
    for (i, sig) in results.iter().enumerate() {
        assert!(verify_signature(sig, &format!("message-{}", i)));
    }
    
    // Performance assertion
    let throughput = 100.0 / duration.as_secs_f64();
    assert!(throughput > 10.0, "Should achieve >10 signatures/second");
}
```

## Test Infrastructure Requirements

### 1. Enhanced TestRunner
```rust
impl TestRunner {
    // Network control
    async fn disconnect(&mut self);
    async fn reconnect(&mut self) -> Result<()>;
    async fn add_latency(&mut self, ms: u32);
    async fn add_packet_loss(&mut self, percent: f32);
    
    // State verification
    async fn get_mesh_status(&self) -> MeshStatus;
    async fn get_dkg_progress(&self) -> DkgProgress;
    async fn verify_key_share(&self) -> bool;
    
    // Multi-session support
    async fn create_session_context(&mut self, id: String) -> SessionContext;
    async fn switch_session(&mut self, id: String);
}
```

### 2. Test Helpers
```rust
// Waiting helpers with timeout
async fn wait_for_condition<F>(condition: F, timeout: Duration) 
where F: Fn() -> bool;

// Batch operations
async fn setup_nodes(ids: &[&str], threshold: u16, total: u16) -> Vec<TestRunner>;
async fn complete_dkg(nodes: &[TestRunner]) -> DkgResult;
async fn sign_with_nodes(nodes: &[TestRunner], message: &str) -> Signature;

// Network simulation
async fn simulate_network_partition(group1: &[TestRunner], group2: &[TestRunner]);
async fn inject_message_corruption(node: &TestRunner, rate: f32);
```

### 3. Verification Utilities
```rust
// State verification
fn verify_mesh_topology(nodes: &[TestRunner]) -> bool;
fn verify_key_shares_consistency(nodes: &[TestRunner]) -> bool;
fn verify_threshold_property(shares: &[KeyShare], threshold: u16) -> bool;

// Protocol verification
fn verify_dkg_transcript(transcript: &DkgTranscript) -> bool;
fn verify_signing_round(round: &SigningRound) -> bool;
```

## Implementation Schedule

### Week 1: Core Infrastructure
- [ ] Enhance TestRunner with network control
- [ ] Implement waiting helpers
- [ ] Create batch operation utilities
- [ ] Add basic verification functions

### Week 2: Basic Tests
- [ ] 2-of-2 DKG happy path
- [ ] 2-of-3 with threshold
- [ ] 3-of-5 large group
- [ ] Basic signing tests

### Week 3: Resilience Tests
- [ ] Disconnect/reconnect scenarios
- [ ] Network partition handling
- [ ] Message loss tolerance
- [ ] Latency tolerance

### Week 4: Security Tests
- [ ] Malicious participant detection
- [ ] Replay attack prevention
- [ ] Invalid message handling
- [ ] Byzantine fault tolerance

### Week 5: Advanced Tests
- [ ] Concurrent operations
- [ ] Performance benchmarks
- [ ] Cross-platform interop
- [ ] State persistence

### Week 6: Integration & CI/CD
- [ ] Integrate with GitHub Actions
- [ ] Create test reports
- [ ] Performance dashboards
- [ ] Failure analysis tools

## Test Execution Strategy

### Local Development
```bash
# Run specific test category
cargo test --test e2e_basic_dkg
cargo test --test e2e_network_resilience
cargo test --test e2e_security

# Run with detailed output
RUST_LOG=debug cargo test -- --nocapture

# Run performance tests
cargo test --release --test e2e_performance
```

### CI/CD Pipeline
```yaml
name: E2E Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        test-suite: [basic, resilience, security, performance]
    steps:
      - uses: actions/checkout@v2
      - name: Run E2E Tests
        run: |
          cargo test --test e2e_${{ matrix.test-suite }}
        env:
          SIGNAL_SERVER: wss://xiongchenyu.dpdns.org
          TEST_TIMEOUT: 300
```

## Success Metrics

### Coverage Goals
- 90% code coverage for business logic
- 80% coverage for network handlers
- 100% coverage for security-critical paths

### Performance Targets
- DKG completion: <5 seconds for 3 nodes
- Signature generation: >10 signatures/second
- WebRTC mesh formation: <2 seconds for 5 nodes
- Recovery from disconnect: <3 seconds

### Reliability Targets
- 99.9% success rate under normal conditions
- 95% success rate with 10% packet loss
- 90% success rate with 500ms latency
- Graceful failure for all error conditions

## Documentation Updates Completed

The docs-architect has created/updated:
1. ✅ **README.md** - Complete setup and usage guide
2. ✅ **ARCHITECTURE.md** - Detailed system design with diagrams
3. ✅ **TESTING.md** - Comprehensive testing guide
4. ✅ **API.md** - Complete API reference
5. ✅ **DEPLOYMENT.md** - Production deployment guide
6. ✅ **CLAUDE.md** - Updated with latest changes

All documentation reflects the recent Elm-architecture migration
(the pre-migration entry type was named `AppRunner`; it's now
`ElmApp<C>` — see `apps/tui-node/src/elm/app.rs:25`) and provides
clear guidance for developers.