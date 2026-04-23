# Signal and WebRTC Message Types

This document defines the JSON message types and protocol flow for negotiating and creating an MPC wallet using a signaling server. Nodes communicate via a CLI application that supports both Ed25519 (Solana) and Secp256k1 (Ethereum) cryptographic curves. The signaling server coordinates device discovery and WebRTC connection setup; all MPC protocol messages are exchanged over WebRTC.

---

## Protocol Overview

1. **Node Registration:**  
   Each node connects to the signaling server via WebSocket and registers with a unique `device_id`.

2. **Discovery:**  
   Nodes query the signaling server for available devices.

3. **Session Negotiation & Mesh Formation:**
   Nodes coordinate session parameters (e.g., total participants,
   threshold, session ID) and build the peer mesh themselves. The
   signaling server tracks registered `device_id`s and session
   announcements in memory (standalone server) or Durable Object
   storage (Cloudflare Worker variant), and relays opaque peer-to-peer
   envelopes — but it does not inspect or intermediate DKG/signing
   protocol messages.

4. **Signaling Exchange:**  
   Nodes exchange WebRTC signaling data (SDP offers/answers, ICE candidates) via the signaling server to establish direct device-to-device connections.

5. **MPC Wallet Creation:**  
   Once WebRTC connections are established, nodes exchange MPC protocol messages (commitments, shares, etc.) directly.

6. **Threshold Signing:**  
   After wallet creation, nodes can participate in threshold signing processes using the FROST protocol.

---

## WebSocket (Signaling Server) Message Types

### 1. Registration

**Client → Server**
```json
{ "type": "register", "device_id": "<device_id>" }
```
Registers the client with the signaling server using a unique `device_id`.

---

### 2. Device Discovery

**Client → Server**
```json
{ "type": "list_devices" }
```
Requests a list of currently registered devices.

**Server → Client**
```json
{ "type": "devices", "devices": ["device1", "device2", ...] }
```
Returns the list of available devices.

---

### 3. Signaling Relay

**Client → Server**
```json
{ "type": "relay", "to": "<device_id>", "data": { ... } }
```
Sends signaling data (SDP offer/answer, ICE candidate, etc.) to another device via the server.

**Server → Client**
```json
{ "type": "relay", "from": "<device_id>", "data": { ... } }
```
Relays signaling data from another device.

---

### 4. Error

**Server → Client**
```json
{ "type": "error", "error": "<description>" }
```
Sent if an error occurs (e.g., unknown device).

### 5. Session discovery

The signal server also knows about session announcements — this
enables cold-start rejoin + peer discovery without needing every
participant to be online simultaneously at announce time.
Authoritative enum: `ClientMsg` / `ServerMsg` in
`apps/signal-server/server/src/lib.rs`.

**Client → Server**
```json
{ "type": "announce_session", "session_info": { ... } }
{ "type": "request_active_sessions" }
{ "type": "session_status_update", "session_info": { ... } }
{ "type": "query_my_active_sessions" }
```

**Server → Clients**
```json
{ "type": "session_available", "session_info": { ... } }
{ "type": "sessions_for_device", "sessions": [ ... ] }
{ "type": "session_list_request", "from": "<device_id>" }
{ "type": "session_removed", "session_id": "<id>", "reason": "<text>" }
```

The `session_info` payload is a JSON blob whose shape is defined
by the sender (browser extension, TUI, native-node all share it
via `packages/@mpc-wallet/types/src/session.ts` on the TS side
and ad-hoc on the Rust side). The signal server treats the inner
payload as opaque.

---

## WebRTC (Device-to-Device) Message Types

Once a direct WebRTC connection is established, nodes exchange application-level messages for the MPC protocol.

> **Scope note (wire-format retraction)**: every JSON example in
> this section uses `{"type": "snake_case_name", "payload": {...}}`
> shape. **That is NOT the real on-wire format** — the examples
> are stylistic, not literal. The real enums are:
>
>   - `WebRTCMessage<C>` — `apps/tui-node/src/protocal/signal.rs:199`
>     tagged `#[serde(tag = "webrtc_msg_type")]`, NO `rename_all`,
>     so the tag value is the **PascalCase** variant name, and
>     variant fields serialize **flat** as sibling properties
>     (NO `"payload"` wrapper).
>   - `WebSocketMessage` — `signal.rs:87` tagged
>     `#[serde(tag = "websocket_msg_type")]`, same PascalCase +
>     flat-fields rule.
>
> A real `DkgRound1Package` message over the data channel has the
> shape (verified against `src/network/webrtc.rs:78-82` and the
> TypeScript mirror at `packages/@mpc-wallet/types/src/webrtc.ts:26`):
>
> ```json
> {
>   "webrtc_msg_type": "DkgRound1Package",
>   "package": "<serialized frost-core round1 Package>"
> }
> ```
>
> — NOT `{"type": "dkg_round1_package", "payload": {"package": ...}}`.
>
> The narrative sections (protocol ordering, role of each message,
> which side sends what) are correct; only the JSON shape below
> should be read as illustrative pseudo-JSON rather than copyable
> on-wire literals.

### 1. Session Management

> **Correction**: `SessionProposal` and `SessionResponse` live on the
> `WebSocketMessage` enum (`signal.rs:91,93`), NOT on `WebRTCMessage`.
> They're relayed **through the signal server** wrapped in a
> `ClientMsg::Relay` envelope — not sent over the peer-to-peer
> data channel. Earlier drafts of this section placed them under
> "WebRTC (Device-to-Device) Message Types"; that's wrong. They
> appear here because the mesh hasn't formed yet when session
> negotiation happens — peers can't reach each other over WebRTC
> before the SDP/ICE exchange completes.

Real shape on the wire (inside a `Relay` envelope, `data` field):

```json
{
  "websocket_msg_type": "SessionProposal",
  "session_id": "<id>",
  "total": 3,
  "threshold": 2,
  "participants": ["device1", "device2", "device3"]
}
```

And for the response:

```json
{
  "websocket_msg_type": "SessionResponse",
  "session_id": "<id>",
  "accepted": true
}
```

---

### 2. Mesh Formation

```json
{
  "type": "channel_open",
  "payload": {
    "device_id": "<device_id>"
  }
}
```
Notifies other devices when a data channel is opened.

```json
{
  "type": "mesh_ready",
  "payload": {
    "session_id": "<id>",
    "device_id": "<device_id>"
  }
}
```
Indicates a device has established connections to all other participants.

---

### 3. Distributed Key Generation (DKG)

Authoritative enum: `WebRTCMessage<C>` in
`apps/tui-node/src/protocal/signal.rs:199`. The enum is tagged
`#[serde(tag = "webrtc_msg_type")]` with no `rename_all`, so on
the wire the discriminator value is the **PascalCase** variant
name (`DkgRound1Package`, not `dkg_round1_package`). Earlier
drafts of this note claimed snake_case; verify against
`signal.rs:199` + the TS mirror at
`packages/@mpc-wallet/types/src/webrtc.ts:26`.

```json
{
  "type": "dkg_round1_package",
  "payload": {
    "package": "<frost-core::keys::dkg::round1::Package>"
  }
}
```
Sends DKG round 1 package (commitments) to other devices.

```json
{
  "type": "dkg_round2_package",
  "payload": {
    "package": "<frost-core::keys::dkg::round2::Package>"
  }
}
```
Sends DKG round 2 package (encrypted shares) to other devices.

DKG finalization is entirely local — there is no `dkg_complete`
wire message. Each participant runs `dkg::part3` on its received
packages to produce its own `KeyPackage` + the shared
`VerifyingKey` (group public key). Participants can cross-check
the resulting group key out-of-band if needed. Earlier drafts of
this doc listed a `dkg_complete` message broadcasting the
group_pubkey — no such type exists in `WebRTCMessage`.

---

### 4. Threshold Signing

```json
{
  "type": "signing_request",
  "payload": {
    "signing_id": "<id>",
    "transaction_data": "<hex-data>",
    "required_signers": 2
  }
}
```
Initiates a signing request for specified transaction data.

```json
{
  "type": "signing_acceptance",
  "payload": {
    "signing_id": "<id>",
    "accepted": true
  }
}
```
Responds to a signing request.

```json
{
  "type": "signer_selection",
  "payload": {
    "signing_id": "<id>",
    "selected_signers": ["<frost-identifier-1>", "<frost-identifier-2>"]
  }
}
```
Announces which participants will be involved in signing.

```json
{
  "type": "signing_commitment",
  "payload": {
    "signing_id": "<id>",
    "sender_identifier": "<frost-identifier>",
    "commitment": "<serialized-commitment>"
  }
}
```
Sends a FROST round 1 commitment for signing.

```json
{
  "type": "signature_share",
  "payload": {
    "signing_id": "<id>",
    "sender_identifier": "<frost-identifier>",
    "share": "<serialized-share>"
  }
}
```
Sends a FROST round 2 signature share.

```json
{
  "type": "aggregated_signature",
  "payload": {
    "signing_id": "<id>",
    "signature": "<hex-encoded-signature>"
  }
}
```
Broadcasts the final aggregated signature.

---

## Protocol Flow

### 1. Registration & Discovery

- Each node connects to the signaling server and registers with a unique `device_id`.
- Nodes may request a list of available devices.

### 2. Session Negotiation & Mesh Formation

- One node (initiator) proposes a session with specific parameters (session ID, total participants, threshold).
- Other nodes accept the session proposal.
- All nodes establish WebRTC connections with each other through signaling exchange.
- Each node tracks the status of its connections and reports readiness when all connections are established.

### 3. Distributed Key Generation (DKG)

- When all nodes report mesh readiness, the DKG process begins automatically:
  - **Round 1:** Each node sends commitments to all other nodes.
  - **Round 2:** Each node sends encrypted shares to all other nodes.
  - **Finalization:** Nodes verify shares and compute their final key shares.
- After successful DKG, each node has:
  - A key package with its signing share
  - The group public key for the distributed wallet

### 4. Threshold Signing

- Any node can initiate a signing request by specifying transaction data.
- Other nodes can accept the request.
- Once enough nodes accept (meeting or exceeding the threshold):
  - The initiator selects which accepted nodes will participate (exactly threshold number).
  - Selected signers exchange FROST round 1 commitments.
  - Selected signers exchange FROST round 2 signature shares.
  - One node aggregates the shares into a final signature and broadcasts it to all participants.

### 5. Completion

- The final signature can be used as appropriate for the blockchain (Ethereum or Solana).
- The session and WebRTC connections remain active for future signing operations.

---

## Message Flow Examples

> **Scope note**: same wire-format caveat as the § WebRTC Message
> Types scope note above — every JSON literal below uses the
> stylistic `{"type": snake_case, "payload": {...}}` shape which
> is **not** the on-wire format. Real envelopes are
> `{"webrtc_msg_type": "PascalCaseName", ...flat fields...}` (or
> `"websocket_msg_type"` for session-layer messages routed through
> the signal server). Sequencing / role-of-each-message is
> correct; only the JSON is illustrative.

### Session Creation

1. Device `mpc-1` sends a `SessionProposal` to `mpc-2` / `mpc-3`
   carrying `session_id / total / threshold / participants`.
2. Each peer responds with a `SessionResponse` whose `accepted`
   field signals acceptance.

### Mesh Formation

1. Each data-channel open triggers a `ChannelOpen` notification
   whose only field is the remote `device_id`.
2. Once a device has observed `ChannelOpen` from every required
   peer, it emits `MeshReady { session_id, device_id }` so the
   coordinator can count mesh-ready signals.

### DKG Process

1. After mesh readiness, each participant broadcasts its
   `DkgRound1Package { package: <frost-core round1::Package>
   }` over the data channel.
2. After all Round-1 packages are received, each participant
   broadcasts its `DkgRound2Package { package: <frost-core
   round2::Package> }`. DKG then finalises locally on each peer
   via `dkg::part3` — there is no dedicated completion message.

### Signing Process

1. `mpc-1` broadcasts `SigningRequest` carrying `signing_id /
   transaction_data (hex) / required_signers / blockchain /
   chain_id` (last two fields required by the real variant at
   `signal.rs:230` — earlier drafts of this example dropped them).
2. Each co-signer replies with `SigningAcceptance { signing_id,
   accepted: bool }`.
3. The coordinator publishes the chosen signer set via
   `SignerSelection { signing_id, selected_signers: Vec<Identifier<C>> }`.
4. Selected signers exchange `SigningCommitment` then
   `SignatureShare`; the aggregator broadcasts `AggregatedSignature
   { signing_id, signature: Vec<u8> }` once threshold shares are in.

---

## Mesh Formation Details

Establishing a complete WebRTC mesh involves:

1. **Connection Establishment:**
   - Each node attempts to establish WebRTC connections to all other participants
   - Connections use SDP offers/answers and ICE candidates exchanged via the signaling server

2. **Data Channel Tracking:**
   - Nodes track when a data channel is successfully opened with each device
   - A `channel_open` message is sent when each data channel opens

3. **Readiness Notification:**
   - When a node has established data channels to all other participants, it broadcasts a `mesh_ready` message
   - Each node tracks which devices have reported mesh readiness
   - The mesh is considered fully ready when all nodes have reported readiness

4. **Automatic Recovery:**
   - If connections fail, nodes automatically attempt reconnection with backoff
   - (Earlier drafts mentioned a manual `/mesh_ready` slash command;
     no slash-command system exists in the TUI — mesh readiness is
     computed automatically from accumulated `MeshReady` events.)

---

## Troubleshooting

- **Device Discovery Issues:** The TUI is keyboard-driven; there
  is no `/list` slash command. Return to the main menu and re-enter
  Join Session to refresh the session list (which triggers a
  `request_active_sessions` round-trip to the signal server).
- **WebRTC Connection Failures:**
  - Ensure you're not behind a restrictive firewall blocking WebRTC.
  - Check the logs for ICE connectivity errors.
  - Try restarting the affected nodes.
- **DKG Failures:**
  - Verify that all nodes are using the same cryptographic curve.
  - Check that the mesh is fully ready before DKG begins.
  - Examine the logs for cryptographic errors in package processing.
- **Signing Issues:**
  - Ensure DKG completed successfully for all nodes.
  - Verify that enough participants have accepted the signing request.
  - Check that selected signers can communicate with each other.

---

## Implementation Notes

- The protocol uses FROST (Flexible Round-Optimized Schnorr Threshold signatures) for threshold signing.
- The implementation supports both Ed25519 (for Solana) and Secp256k1 (for Ethereum) curves.
- WebRTC is used for secure device-to-device communication without requiring a central server after initial connection setup.
- The signaling server is stateless and only facilitates connection establishment, not the cryptographic protocol.