# Real FROST DKG Implementation Status

## Current State (As of 2025-09-15)

The TUI application currently has a **mock DKG implementation** that doesn't actually perform distributed key generation. When you press Enter on the ThresholdConfig screen, it:

1. Shows fake progress updates
2. Returns hardcoded "mock_public_key" 
3. Generates fake addresses
4. Completes instantly without any real cryptography
5. **Does NOT coordinate with other nodes**
6. **Does NOT generate real threshold keys**

## What Real DKG Requires

For a real 2-of-3 (or any threshold) FROST DKG to work:

### Online Mode Requirements

1. **Multiple Nodes Running**: You need exactly `total_participants` nodes running simultaneously
2. **WebSocket Connection**: All nodes must connect to the signal server (wss://auto-life.tech)
3. **Session Creation**: One node creates a DKG session with specific parameters
4. **Session Joining**: Other nodes join using the session ID
5. **WebRTC Mesh**: Nodes establish peer-to-peer connections for secure communication
6. **FROST Protocol Execution**:
   - Round 1: All participants generate and broadcast commitments
   - Round 2: All participants generate and send encrypted shares to each other
   - Round 3: All participants verify shares and compute the group public key

### Current Implementation Updates

The command handler (`src/elm/command.rs`) has been updated to:
- Recognize that multiple participants are needed
- Show informative messages about the requirements
- Fail with proper error messages instead of fake success
- Explain the steps needed for real DKG

## How to Test Real DKG (When Implemented)

### For 2-of-3 Setup:

1. **Terminal 1** (Participant 1):
```bash
cd apps/tui-node
cargo run -- --device-id alice
# Select: Create Wallet > Online Mode > [Curve] > 3 participants, 2 threshold
# Note the session ID displayed
```

2. **Terminal 2** (Participant 2):
```bash
cd apps/tui-node
cargo run -- --device-id bob
# Select: Join Session
# Enter the session ID from Terminal 1
```

3. **Terminal 3** (Participant 3):
```bash
cd apps/tui-node
cargo run -- --device-id charlie
# Select: Join Session
# Enter the session ID from Terminal 1
```

Once all 3 participants have joined, the DKG will automatically start and execute the FROST protocol.

## Implementation Components

### Already Exists:
- ✅ FROST cryptographic library (`packages/@mpc-wallet/frost-core`)
- ✅ Real DKG protocol code (`src/protocal/dkg.rs`)
- ✅ WebSocket/WebRTC infrastructure (`src/webrtc/`, `src/session/`)
- ✅ Session management types (`src/protocal/session_types.rs`)
- ✅ Keystore for saving results (`src/keystore/`)

### Still Needed:
- ❌ WebSocket connection to signal server on startup
- ❌ Session creation and announcement via WebSocket
- ❌ Session discovery and joining mechanism
- ❌ WebRTC peer connection establishment
- ❌ Message routing between DKG protocol and WebRTC
- ❌ Proper state management for multi-round protocol
- ❌ Error recovery and timeout handling

## Code Structure

```
DKG Flow:
1. UI (ThresholdConfig) → SelectItem
2. Update Handler → StartDKG Command
3. Command Handler → Check online/offline mode
4. Online Mode:
   a. Create/join session via WebSocket
   b. Wait for all participants
   c. Execute protocal/dkg.rs functions
   d. Exchange messages via WebRTC
   e. Save keystore on completion
5. Return to main menu with new wallet

Key Files:
- src/elm/command.rs - Command execution (updated)
- src/protocal/dkg.rs - Real FROST DKG implementation
- src/session/mod.rs - Session management
- src/webrtc/mesh_manager.rs - P2P communication
- src/handlers/session_handler.rs - Session coordination
```

## Testing the Current State

To see the current (non-functional) behavior:

```bash
cargo run
# Create Wallet > Online > Ed25519 > Enter
# You'll see error messages explaining that real DKG needs multiple participants
```

## Next Steps for Full Implementation

1. **Connect to WebSocket on startup** - Establish connection to wss://auto-life.tech
2. **Implement session creation** - When starting DKG, create a session on the server
3. **Implement session joining** - Allow other nodes to discover and join sessions
4. **Wire up WebRTC** - Establish P2P connections between participants
5. **Connect DKG to messaging** - Route DKG protocol messages through WebRTC
6. **Handle protocol completion** - Save keys, derive addresses, update UI

## Security Considerations

- The current mock implementation is **completely insecure**
- Real DKG requires **secure channels** (TLS for WebSocket, DTLS for WebRTC)
- Each participant must **verify** all received data
- Keys must be **encrypted at rest** in the keystore
- The group public key must be **derived from the DKG output**, not from session IDs

## References

- FROST Paper: https://eprint.iacr.org/2020/852
- frost-core documentation: https://docs.rs/frost-core/
- Example implementation: packages/@mpc-wallet/frost-core/examples/dkg.rs