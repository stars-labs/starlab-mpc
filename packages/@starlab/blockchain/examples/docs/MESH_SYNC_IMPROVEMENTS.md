# Mesh Cluster Synchronization Improvements

> **Note**: As of 2026-04, the reference examples `eth_dkg.rs` and
> `solana_dkg.rs` have been renamed to `.rs.disabled` because they
> don't compile against current `bincode` (workspace dropped the
> dep), `rlp`/`ethers-core` (U256 / H160 `Encodable` trait bounds
> changed), and `solana-sdk` (Transaction no longer implements
> `SerializableTransaction`) API versions. The code is preserved
> in-tree as a reference for the multi-party signing flow this
> document describes; rename back to `.rs` + refresh the three
> problematic imports if you want to actually run it.

## Problem Solved
Both the Solana DKG (`solana_dkg.rs`) and Ethereum DKG (`eth_dkg.rs`) examples required all nodes to be started **simultaneously**, which was impractical for real-world usage. Nodes would fail with "Connection refused" errors if other nodes weren't already running.

## Solution: Wait-for-All Mesh Discovery

### Key Features

1. **Deterministic Synchronization**
   - Nodes wait indefinitely for ALL expected peers to join
   - No timeouts or race conditions
   - Guaranteed mesh formation before DKG begins

2. **Staggered Startup Support**
   - Nodes can be started at any time in any order
   - Each node waits patiently for the complete cluster
   - No more "Connection refused" spam in logs

3. **Discovery Phase**
   - New `Discovery` state in the node state machine
   - Periodic ping/pong messaging to find available peers
   - Real-time status updates showing discovered peers

4. **Robust Mesh Formation**
   - Only proceeds when ALL expected nodes are discovered
   - Peer-based communication (not index-based broadcasting)
   - Better error handling and logging

5. **Cross-Platform Support**
   - Works with both sync (Solana) and async (Ethereum) networking
   - Consistent behavior across different blockchain examples

### New Message Types
```rust
// Discovery messages
Ping(PingMessage),
Pong(PongMessage),
Ready(ReadyMessage),
```

### New CLI Option
```bash
--wait-for-all true   # Wait for all nodes before proceeding (default: true)
```

## Usage Examples

### Before (Required simultaneous startup)
```bash
# All terminals needed to be started at the same time
cargo run --example solana_dkg -- --index 1 --total 3 --threshold 2 &
cargo run --example solana_dkg -- --index 2 --total 3 --threshold 2 &
cargo run --example solana_dkg -- --index 3 --total 3 --threshold 2 &

# Same issue with ETH DKG
cargo run --example eth_dkg -- --index 1 --total 3 --threshold 2 &
cargo run --example eth_dkg -- --index 2 --total 3 --threshold 2 &
cargo run --example eth_dkg -- --index 3 --total 3 --threshold 2 &
```

### After (Any order, any timing)
```bash
# Solana DKG - Start nodes whenever you want - they'll wait for each other
cargo run --example solana_dkg -- --index 1 --total 3 --threshold 2 &
# ... wait 30 seconds if you want
cargo run --example solana_dkg -- --index 2 --total 3 --threshold 2 &
# ... wait another minute if you want  
cargo run --example solana_dkg -- --index 3 --total 3 --threshold 2 &
# DKG starts automatically when all 3 are discovered

# Ethereum DKG - Same flexible behavior
cargo run --example eth_dkg -- --index 1 --total 3 --threshold 2 &
# ... staggered timing supported
cargo run --example eth_dkg -- --index 2 --total 3 --threshold 2 &
cargo run --example eth_dkg -- --index 3 --total 3 --threshold 2 &
```

## Demo Scripts

### Solana DKG
- `demo-mesh-sync.sh` - Interactive demo showing staggered Solana DKG startup
- `test-mesh-discovery.sh` - Test script for Solana mesh discovery

### Ethereum DKG  
- `demo-eth-mesh-sync.sh` - Interactive demo showing staggered Ethereum DKG startup

## Technical Implementation

### State Machine Updates
```rust
enum NodeState {
    Initial,
    Discovery,     // New state for peer discovery
    DkgProcess,    // Only reached when all peers found
    // ... rest unchanged
}
```

### Discovery Algorithm
1. **Ping Phase**: Continuously ping all expected peer addresses
2. **Listen Phase**: Accept incoming pings and respond with pongs  
3. **Collection Phase**: Build set of discovered peers in real-time
4. **Completion**: Proceed to DKG only when ALL expected nodes found
5. **No Timeouts**: Nodes wait indefinitely for complete mesh formation

### Sample Output
```
Node 1 starting peer discovery, waiting for all 3 nodes...
Node 1 discovered 1/3 nodes: {1}
Node 1 discovered 2/3 nodes: {1, 2}
Node 1 discovery complete! Found all 3 nodes: {1, 2, 3}
Node 1 discovered all 3 peers: {1, 2, 3}
Node 1 in DKG_PROCESS state
```

### Network Improvements
- **Solana DKG**: Synchronous TCP with non-blocking discovery
- **Ethereum DKG**: Asynchronous Tokio TCP with timeout handling
- Intelligent peer detection (only ping undiscovered nodes)
- Real-time progress reporting
- Peer-filtered broadcasting (no wasted messages)
- Graceful connection handling

## Benefits

✅ **Guaranteed synchronization**: No race conditions or partial cluster starts  
✅ **Flexible startup**: Start nodes in any order at any time  
✅ **Deterministic behavior**: Always waits for complete mesh formation  
✅ **Better UX**: Clear progress reporting and status updates  
✅ **Production ready**: No timeouts to tune or failure modes to handle  
✅ **Fault tolerance**: Robust against temporary network issues  
✅ **Scalability**: Works with any cluster size (1 to N nodes)  
✅ **Cross-platform**: Works with both sync and async networking models  

## Backward Compatibility

The changes are fully backward compatible. The new `--wait-for-all true` is the default behavior, providing improved robustness while maintaining the same CLI interface for both Solana and Ethereum DKG examples.