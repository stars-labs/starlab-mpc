# Hybrid Mode E2E Test Design

## Overview

This document outlines the hybrid operational mode where some MPC participants operate online (via WebSocket/WebRTC) while others remain offline (air-gapped with SD card exchange). This reflects real-world scenarios where high-security keys are kept offline while convenience signers operate online.

## Architecture

```
┌─────────────────┐        WebSocket         ┌─────────────────┐
│   Online Node   │◄────────────────────────►│   Online Node   │
│   (Alice - P1)  │                          │   (Bob - P2)    │
└────────┬────────┘        WebRTC            └────────┬────────┘
         │         ◄──────────────────────────►        │
         │                                              │
         │              SD Card Exchange               │
         └──────────────────────────────────────────────┘
                               │
                               ▼
                    ┌─────────────────┐
                    │  Offline Node   │
                    │ (Charlie - P3)  │
                    │  (Air-gapped)   │
                    └─────────────────┘
```

## Test Scenarios

> **Scope note**: Scenarios 2–5 below are structured as
> "transaction signing" (ETH Transfer / SOL Transfer / SPL Token /
> Emergency Signing) with transaction-shaped setup fields
> (Type / Amount / To / Program). The shipped TUI signs **raw
> bytes** (EIP-191 `personal_sign` shape over a hex-encoded
> message), not Ethereum-transaction structs with decoded
> Type/Amount/To fields. An external tool must serialize the
> transaction first and hand the hex to the TUI.
>
> Scenario 1 (Hybrid DKG) matches what the hybrid layer at
> `src/hybrid/` actually implements. For signing, read the
> scenario setup as "what an external wallet would hand to the
> TUI for signing after RLP-encoding + keccak-hashing the
> transaction" rather than as literal TUI operations. The hybrid
> DKG flow + SD-card exchange mechanics are accurate.
>
> See `apps/tui/docs/guides/USER_GUIDE.md § Signing Messages
> → Scope` and the "Phase C scope: message-only field" comment
> in `src/elm/components/sign_transaction.rs` for the honest
> signing-surface picture.

### 🌐 Scenario 1: Hybrid DKG (2 Online + 1 Offline)

**Setup:**
- Alice (P1): Online coordinator
- Bob (P2): Online participant  
- Charlie (P3): Offline participant
- Threshold: 2-of-3
- Curves: Both secp256k1 (Ethereum) and ed25519 (Solana)

**DKG Flow:**

1. **Round 1 - Commitment Generation**
   - Alice & Bob: Exchange commitments via WebRTC
   - Charlie: Generates commitment offline, exports to SD card
   - Alice: Collects Charlie's commitment from SD card

2. **Round 2 - Share Distribution**
   - Alice & Bob: Exchange shares via encrypted WebRTC
   - Charlie: Receives aggregated data via SD card
   - Charlie: Generates shares, exports to SD card
   - Alice & Bob: Import Charlie's shares from SD card

3. **Round 3 - Finalization**
   - All parties finalize locally
   - Group public keys verified across all participants

### 💰 Scenario 2: Hybrid Ethereum Transaction Signing

**Transaction:** 
- Type: ETH Transfer
- Amount: 2.5 ETH
- To: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7

**Signing Participants:** Alice (online) + Charlie (offline)

**Flow:**
1. Alice initiates transaction online
2. Alice generates commitment, broadcasts via WebSocket
3. Charlie receives transaction via SD card
4. Charlie generates commitment offline, exports to SD card
5. Alice imports Charlie's commitment
6. Both generate signature shares
7. Alice aggregates and broadcasts

### ☀️ Scenario 3: Hybrid Solana Transaction Signing

**Transaction:**
- Type: SOL Transfer
- Amount: 100 SOL
- To: 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM

**Signing Participants:** Bob (online) + Charlie (offline)

**Flow:**
1. Bob creates Solana transaction
2. Bob's commitment sent via WebSocket
3. SD card exchange for Charlie
4. Signature aggregation
5. Transaction submission to Solana

### 🪙 Scenario 4: SPL Token Transfer (Solana)

**Transaction:**
- Token: USDC (SPL)
- Amount: 500 USDC
- Program: TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA

**Signing Participants:** Alice (online) + Bob (online)
- Charlie remains offline but could participate if needed

### 🔄 Scenario 5: Emergency Signing (All Offline)

**Situation:** Network compromise detected, all nodes switch to offline mode

**Flow:**
1. All nodes disconnect from network
2. Transaction created offline
3. SD card round-robin for commitments
4. SD card round-robin for shares
5. Final signature assembly offline

## Implementation Components

> **Scope note (partial retraction)**: the three code blocks below
> mix real types with fabricated ones. The *real* hybrid layer
> lives in `apps/tui/src/hybrid/` (two files: `transport.rs` +
> `coordinator.rs`), but the sketches below invented wrapper types
> that don't exist. Verifications:
>
> **Real types** (keep using these names):
>   - `HybridCoordinator`  `src/hybrid/coordinator.rs:41`
>   - `ParticipantInfo`    `src/hybrid/coordinator.rs:18`
>   - `OnlineTransport`    `src/hybrid/transport.rs:9`
>   - `OfflineTransport`   `src/hybrid/transport.rs:84`
>   - `HybridMessage`      (re-exported from `src/hybrid/mod.rs:7`)
>   - `SolanaTransactionBuilder`  `src/utils/solana_encoder.rs:53`
>
> **Fabricated types** (grep returns only this doc):
>   - `NetworkSimulator` / `WebSocketHub` / `WebRTCMesh` (§ 1 block)
>     — no such structs in source. `WebRTCMeshManager` IS real at
>     `src/webrtc/mesh_manager.rs:136` but that's the in-process
>     mesh simulator library, not a "WebRTCMesh" in hybrid/.
>   - `SolanaTransaction` (§ 2 block) — real name is
>     `SolanaTransactionBuilder`, and its method list differs:
>     grep for the actual constructors in `solana_encoder.rs`.
>   - `MessageQueue` struct — doesn't exist. Hybrid messaging rides
>     the `Vec<HybridMessage>` return of
>     `HybridCoordinator::receive_messages(participant_id)`
>     (`coordinator.rs:145`).
>   - `coordinate_dkg` / `coordinate_signing` / `bridge_online_offline`
>     method names on `HybridCoordinator` — none exist. Real methods
>     are `register_participant` / `send_message` /
>     `broadcast_message` / `receive_messages` /
>     `perform_sd_card_exchange` / `advance_round`
>     / `simulate_network_failure` / `restore_network`.
>
> Treat the three sketches below as design-intent notation rather
> than a literal API reference.

### 1. Network Simulator
```rust
struct NetworkSimulator {
    online_nodes: HashMap<ParticipantId, OnlineNode>,
    offline_nodes: HashMap<ParticipantId, OfflineNode>,
    websocket_hub: WebSocketHub,
    webrtc_mesh: WebRTCMesh,
    sd_card: MockSDCard,
}
```

### 2. Solana Transaction Builder
```rust
struct SolanaTransaction {
    instructions: Vec<Instruction>,
    recent_blockhash: Hash,
    fee_payer: Pubkey,
}

impl SolanaTransaction {
    fn transfer_sol(from: &Pubkey, to: &Pubkey, lamports: u64) -> Self;
    fn transfer_spl_token(token: &Pubkey, from: &Pubkey, to: &Pubkey, amount: u64) -> Self;
    fn create_associated_token_account(wallet: &Pubkey, mint: &Pubkey) -> Self;
}
```

### 3. Hybrid Coordinator
```rust
struct HybridCoordinator {
    online_transport: OnlineTransport,
    offline_transport: OfflineTransport,
    message_queue: MessageQueue,
}

impl HybridCoordinator {
    async fn coordinate_dkg(&mut self) -> Result<GroupKey>;
    async fn coordinate_signing(&mut self, tx: Transaction) -> Result<Signature>;
    fn bridge_online_offline(&mut self) -> Result<()>;
}
```

## Test Execution Plan

### Phase 1: Setup
1. Initialize 3 participants with mixed online/offline status
2. Establish WebSocket connections for online nodes
3. Setup WebRTC data channels
4. Initialize SD card simulation for offline node

### Phase 2: Hybrid DKG
1. Execute DKG with online nodes communicating via WebRTC
2. Bridge offline node via SD card exchanges
3. Verify all nodes derive same group keys
4. Save keystores for all participants

### Phase 3: Ethereum Signing
1. Create ETH transaction
2. Sign with Alice (online) + Charlie (offline)
3. Verify signature
4. Test with different participant combinations

### Phase 4: Solana Signing
1. Create SOL transfer transaction
2. Sign with Bob (online) + Charlie (offline)
3. Create SPL token transfer
4. Sign with Alice + Bob (both online)
5. Verify ed25519 signatures

### Phase 5: Stress Testing
1. Simulate network failures
2. Test offline fallback
3. Verify signature consistency
4. Test concurrent transactions

## Security Considerations

> **Scope note**: the bullets below mix real properties with
> aspirational hardening items. Each is labelled with its shipped
> status.

### Online Nodes
- **TLS for WebSocket** — ✅ real, `wss://` via the platform TLS
  stack. NOT pinned to a specific TLS version (earlier drafts
  claimed "TLS 1.3"; the code delegates version negotiation to
  tokio-tungstenite's defaults).
- **DTLS for WebRTC** — ✅ real, negotiated by the webrtc crate.
- **Authenticated channels** — ⚠ transport-level only (DTLS
  certificate fingerprint exchange); no app-level authentication
  on signal-server relays.
- **Rate limiting** — ❌ not implemented. The signal server
  doesn't rate-limit announcements or relays; adding that is open
  hardening work.

### Offline Node
- **Air-gap enforcement** — ⚠ enforced at the operator level
  (launching with `--offline` skips network bring-up; there's no
  runtime interface-blocker beyond the flag).
- **Per-wallet encryption at rest** — ✅ AES-256-GCM + PBKDF2 or
  Argon2id password KDF (not "SD card encryption" as a
  filesystem-level feature; the files ON the SD card are already
  the encrypted `<wallet_id>.json` or the `OfflineData` envelopes).
- **Physical security** — out-of-scope for the software; operator-
  controlled.
- **Audit logging** — ❌ no structured audit log ships. `tracing`
  output at `--log-location` is the only observability path.

### Bridge Security
- **Verification of imported data** — ⚠ partial. `frost-core`'s
  `dkg::part2` / `part3` + `aggregate` reject malformed commitments
  / shares cryptographically, which IS the real tamper-detection
  line. No signatures on the outer JSON envelopes themselves.
- **Time-based validity windows** — ✅ real; `OfflineData.expires_at`
  at `src/offline/types.rs:12` gates import against wall-clock
  expiry.
- **One-way data flow enforcement** — ❌ not mechanically enforced
  by the TUI; it's a procedural recommendation (export to SD card,
  import on air-gapped node, re-export back). Earlier drafts
  described this as a code-level feature.
- **Sanitization of SD card data** — ❌ no sanitization layer.
  Imported JSON is deserialized via serde; malformed structures
  error out at parse time, but there's no allowlist / fuzzer-
  resistance layer.

## Success Criteria

1. **DKG Success**: All nodes derive identical group keys
2. **Signing Success**: Valid signatures from any 2-of-3 combination
3. **Hybrid Operation**: Seamless online/offline coordination
4. **Multi-Chain**: Both Ethereum and Solana transactions work
5. **Security**: No key material leakage between online/offline
6. **Performance targets** (not measured — no benchmark harness
   ships; see the Performance Considerations section in the main
   ARCHITECTURE.md for context): aim for online-only completion
   well under typical human-interaction cadence, and hybrid
   completion dominated by SD-card handoff rather than compute.
   Earlier drafts of this bullet listed specific numbers
   (`< 5 seconds for online, < 30 seconds for hybrid`); those
   numbers had no source and have been removed.

## Expected Output

```
🚀 Hybrid Mode E2E Test
========================

Phase 1: Setup
✅ Alice (P1): Online - WebSocket connected
✅ Bob (P2): Online - WebRTC ready
✅ Charlie (P3): Offline - SD card initialized

Phase 2: Hybrid DKG
✅ Online nodes exchanged via WebRTC
✅ Offline node bridged via SD card
✅ Group keys match across all nodes
  Ethereum: 0x1234...
  Solana: 9WzDX...

Phase 3: Ethereum Transactions
✅ ETH transfer signed (Alice + Charlie)
✅ ERC20 transfer signed (Bob + Charlie)
✅ Signatures verified

Phase 4: Solana Transactions
✅ SOL transfer signed (Bob + Charlie)
✅ SPL token transfer signed (Alice + Bob)
✅ Ed25519 signatures valid

Phase 5: Stress Tests
✅ Network failure handled
✅ Offline fallback successful
✅ Concurrent signing works

Summary: All tests passed!
```

## Implementation Files

Real layout (verified against `find apps/tui -name '*.rs'`):

```
apps/tui/
├── src/
│   ├── hybrid/
│   │   ├── mod.rs
│   │   ├── coordinator.rs      # HybridCoordinator + ParticipantInfo
│   │   └── transport.rs        # OnlineTransport + OfflineTransport
│   │                           # + HybridMessage (ONE file, not two)
│   └── utils/
│       └── solana_encoder.rs   # SolanaTransactionBuilder, SPL token
│                               # encoding — there is NO `src/solana/`
│                               # directory; everything is here.
└── examples/
    └── hybrid_mode_e2e_test.rs
```

Earlier drafts of this tree invented four files that don't exist:
`src/hybrid/online_transport.rs`, `src/hybrid/offline_transport.rs`
(both `OnlineTransport` and `OfflineTransport` live together in
`transport.rs`); and the trio `src/solana/{mod,transaction,spl_token}.rs`
(the whole `src/solana/` directory never existed — Solana encoding
is a single file inside `src/utils/`).