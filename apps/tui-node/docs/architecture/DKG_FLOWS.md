# FROST MPC TUI Wallet - DKG Flows

## Table of Contents

1. [Overview](#overview)
2. [Online DKG Flow](#online-dkg-flow)
3. [Offline DKG Flow](#offline-dkg-flow)
4. [Hybrid DKG Flow](#hybrid-dkg-flow)
5. [Recovery Procedures](#recovery-procedures)
6. [Security Considerations](#security-considerations)
7. [Troubleshooting](#troubleshooting)

## Overview

Distributed Key Generation (DKG) is the foundational process for creating MPC wallets. The FROST protocol enables multiple parties to jointly generate a key pair where no single party ever has access to the complete private key. This document details both online and offline DKG procedures.

### Key Concepts

- **Threshold (t)**: Minimum number of participants needed to sign
- **Participants (n)**: Total number of key share holders
- **Key Shares**: Individual pieces of the distributed private key
- **Verification Shares**: Public commitments used to verify operations

### DKG Properties

1. **Distributed Trust**: No single point of failure
2. **Verifiable**: All participants can verify correct execution
3. **Robust**: Can complete even if some parties fail (up to n-t failures)
4. **Secure**: Threshold of parties required to reconstruct private key

## Online DKG Flow

The online DKG process uses WebRTC mesh networking for real-time coordination between participants.

### Prerequisites

- All participants online simultaneously for the duration of the
  DKG ceremony
- WebRTC-routable network: STUN is enough for most home NATs
  (full-cone / restricted-cone / port-restricted-cone). Symmetric-NAT
  peers may fail to connect because no TURN server ships with this
  repo. If a participant is behind symmetric NAT, either run the
  ceremony in offline mode over SD card, or stand up your own TURN
  and point the clients at it.

Earlier drafts of this section listed "Synchronized system clocks
(±5 minutes tolerance)" as a prerequisite — FROST DKG is not
time-sensitive; remove that from your checklist.

### Step-by-Step Process

> **Scope note (UI mocks)**: The ASCII-box "View" mocks in this
> section are illustrative sketches, not literal renders of the
> ratatui UI. Specifically, the following elements **are not**
> drawn by the real components:
>
>   - Dropdown widgets (`▼` arrows for Blockchain / Participants /
>     Threshold)
>   - "Available Participants" checkbox lists with
>     per-participant IP addresses (peers are discovered via the
>     Join Session flow, not a coordinator-side invite; no IPs
>     are surfaced in the TUI)
>   - Button rows like `[Start DKG] [Test Connection] [Cancel]`,
>     `[Accept & Join] [Decline] [View Details]`,
>     `[View Technical Details] [Pause]`, `[Complete DKG]
>     [View Shares]`, `[View Wallet] [Create Backup] [Done]` —
>     ratatui components don't render button widgets; navigation
>     is keyboard-driven (arrow keys, Enter, Esc)
>   - "Network Quality: Latency 12ms / Packet Loss 0.0% /
>     Encryption DTLS 1.3" telemetry — the TUI does NOT measure
>     per-peer latency/loss; that would need WebRTC stats
>     collection that isn't wired up
>   - "Next Steps" cards with "Test wallet with small transaction"
>     (TUI doesn't construct or broadcast transactions; see
>     guides/USER_GUIDE.md § Signing Messages → Scope)
>
> The narrative flow (Session Initiation → Invitation → Mesh →
> DKG Round 1/2 → Finalization) matches the real ceremony. Read
> the mocks for structural intent; don't try to match screenshots
> against them.

#### 1. Session Initiation

**Coordinator's View:**
```
┌─────────────────────────────────────────────────────┐
│ Create New Wallet - Online DKG                      │
├─────────────────────────────────────────────────────┤
│ Wallet Configuration:                               │
│                                                     │
│ Name: [treasury-wallet_______________]              │
│ Blockchain: [Ethereum (secp256k1)] ▼               │
│ Participants: [3] ▼                                 │
│ Threshold: [2] ▼                                    │
│                                                     │
│ Available Participants (3 online):                  │
│ ☑ alice (coordinator - you)                         │
│ ☑ bob (online - 192.168.1.10)                      │
│ ☑ charlie (online - 192.168.1.11)                  │
│ ☐ dave (offline)                                   │
│                                                     │
│ (No pre-flight NAT/bandwidth check is run by the TUI.
│ The Network Check panel in earlier drafts of this mockup
│ claimed a "Symmetric NAT (WebRTC compatible)" status —
│ backwards: symmetric NAT is the HARDEST case for WebRTC
│ without TURN. Reality is that DKG is attempted directly
│ over the peer mesh once signaling completes; failures
│ surface as peer-connection timeouts, not a pre-flight
│ "bandwidth insufficient" gate.)                     │
│                                                     │
│ [Start DKG] [Test Connection] [Cancel]             │
└─────────────────────────────────────────────────────┘
```

#### 2. Participant Invitation

**Participant's View:**
```
┌─────────────────────────────────────────────────────┐
│ 🔔 DKG Session Invitation                           │
├─────────────────────────────────────────────────────┤
│ Coordinator: alice                                  │
│ Wallet Name: treasury-wallet                        │
│ Type: 2-of-3 Ethereum Wallet                       │
│                                                     │
│ Your Role: Participant #2                           │
│ Other Participants:                                 │
│ • alice (Coordinator)                               │
│ • charlie (Pending)                                 │
│                                                     │
│ Session Details:                                    │
│ • Created: 2024-01-20 10:30:15                     │
│ • Expires: 2024-01-20 10:45:15 (15 min)           │
│ • Protocol: FROST-secp256k1                        │
│                                                     │
│ ⚠️  Joining will start key generation immediately  │
│                                                     │
│ [Accept & Join] [Decline] [View Details]           │
└─────────────────────────────────────────────────────┘
```

#### 3. WebRTC Mesh Formation

**Connection Status Display:**
```
┌─────────────────────────────────────────────────────┐
│ Establishing Secure Connections                     │
├─────────────────────────────────────────────────────┤
│ Building P2P mesh network...                        │
│                                                     │
│ Connections:                                        │
│ • You → bob     [████████████░░░░] Connecting...   │
│ • You → charlie [████████████████] Connected       │
│ • bob → charlie [████████████████] Connected       │
│                                                     │
│ Network Quality:                                    │
│ • Latency: 12ms average                            │
│ • Packet Loss: 0.0%                                │
│ • Encryption: DTLS 1.3                             │
│                                                     │
│ Status: Waiting for all connections...             │
│                                                     │
│ [Details] [Abort]                                  │
└─────────────────────────────────────────────────────┘
```

#### 4. DKG Protocol Execution

**Round 1 - Commitment Generation:**
```
┌─────────────────────────────────────────────────────┐
│ DKG Progress - Round 1 of 2                         │
├─────────────────────────────────────────────────────┤
│ Generating cryptographic commitments...             │
│                                                     │
│ Local Operations:                                   │
│ ✅ Generated secret polynomial                      │
│ ✅ Computed Feldman commitments                     │
│ ✅ Created proof of knowledge                       │
│                                                     │
│ Broadcast Status:                                   │
│ • To bob:     ✅ Sent (confirmed)                  │
│ • To charlie: ✅ Sent (confirmed)                  │
│                                                     │
│ Received Commitments:                               │
│ • From bob:     ✅ Valid                           │
│ • From charlie: ⏳ Waiting...                      │
│                                                     │
│ Round Progress: ▓▓▓▓▓▓▓▓▓▓░░░░░ 66%               │
│                                                     │
│ [View Technical Details] [Pause]                   │
└─────────────────────────────────────────────────────┘
```

**Round 2 - Share Distribution:**
```
┌─────────────────────────────────────────────────────┐
│ DKG Progress - Round 2 of 2                         │
├─────────────────────────────────────────────────────┤
│ Distributing encrypted shares...                    │
│                                                     │
│ Share Generation:                                   │
│ ✅ Computed shares for each participant             │
│ ✅ Encrypted with participant public keys           │
│ ✅ Generated zero-knowledge proofs                  │
│                                                     │
│ Distribution Status:                                │
│ • To bob:     ✅ Delivered & Acknowledged          │
│ • To charlie: ✅ Delivered & Acknowledged          │
│                                                     │
│ Share Verification:                                 │
│ • From bob:     ✅ Valid share received            │
│ • From charlie: ✅ Valid share received            │
│                                                     │
│ Final Verification:                                 │
│ ✅ All shares consistent with commitments           │
│ ✅ Threshold parameters verified                    │
│                                                     │
│ [Complete DKG] [View Shares]                       │
└─────────────────────────────────────────────────────┘
```

#### 5. Wallet Finalization

**Success Screen:**
```
┌─────────────────────────────────────────────────────┐
│ ✅ Wallet Created Successfully!                     │
├─────────────────────────────────────────────────────┤
│ Wallet Details:                                     │
│ • Name: treasury-wallet                            │
│ • Type: 2-of-3 Ethereum Wallet                     │
│ • Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f │
│                                                     │
│ Your Key Share:                                     │
│ • Share Index: 2                                    │
│ • Public Share: 0x04a8b3...                        │
│ • Status: Encrypted and saved                      │
│                                                     │
│ Other Participants:                                 │
│ • alice: Share 1 ✅                                 │
│ • charlie: Share 3 ✅                               │
│                                                     │
│ Next Steps:                                         │
│ 1. Test wallet with small transaction              │
│ 2. Create secure backup                            │
│ 3. Document participant contacts                   │
│                                                     │
│ [View Wallet] [Create Backup] [Done]               │
└─────────────────────────────────────────────────────┘
```

### Online DKG Sequence Diagram

```
Alice (Coordinator)     Bob (Participant)      Charlie (Participant)
        |                       |                       |
        |---- Create Session -->|                       |
        |                       |                       |
        |<--- Accept -------->  |                       |
        |                       |                       |
        |------ Invite -------->|------- Invite ------->|
        |                       |                       |
        |<---- Accept ----------|<----- Accept ---------|
        |                       |                       |
        |==== WebRTC Setup =====|===== WebRTC Setup ====|
        |                       |                       |
        |---- Round 1 Comm ---->|---- Round 1 Comm ---->|
        |<--- Round 1 Comm -----|<--- Round 1 Comm -----|
        |                       |                       |
        |---- Round 2 Share --->|---- Round 2 Share --->|
        |<--- Round 2 Share ----|<--- Round 2 Share ----|
        |                       |                       |
        |===== Verify ==========|====== Verify =========|
        |                       |                       |
        |---- Complete -------->|---- Complete -------->|
```

## Offline DKG Flow

The offline DKG process enables key generation without network connectivity, using removable media for data exchange.

### Prerequisites

- Dedicated, air-gapped machines for each participant
- Removable media (SD cards, USB drives)
- Secure physical channel for media exchange
- Trusted coordinator for orchestration

### Step-by-Step Process

> **Scope note (mocks)**: the ASCII screens below are
> illustrative, not literal renders. Specifically absent from
> the real TUI:
>
>   - An in-app "Enable Offline Mode" toggle screen — offline
>     mode is a startup-time `--offline` CLI flag decision
>     (see guides/offline-mode.md § Enabling Offline Mode),
>     not a runtime confirmation dialog.
>   - The "☑ System clock synchronized" checklist item —
>     FROST DKG is not time-sensitive; same retraction as §
>     Prerequisites above.
>   - Network-interface auto-disable ("WiFi/Ethernet/Bluetooth:
>     Will be disabled") — the TUI doesn't touch OS-level
>     network interfaces; `--offline` just skips the signaling
>     code path. Operators disable interfaces themselves.
>   - Button rows like `[Enable Offline Mode] [Cancel]` —
>     ratatui components don't render button widgets; navigation
>     is keyboard-driven (arrow keys + Enter + Esc).
>   - "Format: JSON (signed)" lines in the per-round artefact
>     descriptions — there is no application-level per-file
>     signature layer on offline bundles (same retraction as
>     OFFLINE_DKG_GUIDE.md § Data Formats); integrity comes from
>     the physical chain of custody of the SD cards plus
>     frost-core's cryptographic rejection of malformed
>     commitment/share bytes during `part2` / `part3`.
>   - Specific file-size figures like "2.3 KB" — illustrative
>     only; real sizes depend on ciphersuite + participant
>     count and aren't published anywhere authoritatively.
>
> The narrative flow (mode decision → parameter exchange →
> round 1 commitments → round 2 shares → local finalize) is
> correct; only the specific widgets/checklists should be read
> as design-intent pseudo-screens.

#### 1. Offline Mode Activation

**Each Participant:**
```
┌─────────────────────────────────────────────────────┐
│ 🔒 Enable Offline Mode                              │
├─────────────────────────────────────────────────────┤
│ Current Status: Online                              │
│                                                     │
│ Offline Mode Checklist:                            │
│ ☑ Network interfaces will be disabled              │
│ ☑ SD card mounted at: /mnt/secure-sd              │
│ ☑ System clock synchronized                        │
│ ☑ Temporary files cleared                          │
│                                                     │
│ Security Verification:                              │
│ • WiFi: Will be disabled                           │
│ • Ethernet: Will be disabled                       │
│ • Bluetooth: Will be disabled                      │
│ • USB: Restricted to storage only                  │
│                                                     │
│ ⚠️  This action cannot be undone without restart   │
│                                                     │
│ [Enable Offline Mode] [Cancel]                     │
└─────────────────────────────────────────────────────┘
```

#### 2. DKG Parameters Exchange

**Coordinator Creates DKG Package:**
```
┌─────────────────────────────────────────────────────┐
│ 🔒 Create Offline DKG Package                       │
├─────────────────────────────────────────────────────┤
│ DKG Configuration:                                  │
│                                                     │
│ Wallet Name: cold-storage                           │
│ Participants: 3                                     │
│ Threshold: 2                                        │
│ Blockchain: Bitcoin (secp256k1)                    │
│                                                     │
│ Participant Information:                            │
│ 1. alice-airgap (Coordinator)                      │
│ 2. bob-airgap                                      │
│ 3. charlie-airgap                                  │
│                                                     │
│ Package Contents:                                   │
│ • DKG parameters                                    │
│ • Participant identifiers                           │
│ • Session metadata                                  │
│ • Expiration: 48 hours                             │
│                                                     │
│ Export Location: /mnt/secure-sd/dkg-init.json      │
│                                                     │
│ [Generate Package] [Cancel]                         │
└─────────────────────────────────────────────────────┘
```

**Participants Import Package:**
```
┌─────────────────────────────────────────────────────┐
│ 🔒 Import DKG Package                               │
├─────────────────────────────────────────────────────┤
│ SD Card Status: Mounted                             │
│ Found DKG package: dkg-init.json                    │
│                                                     │
│ Package Details:                                    │
│ • Created by: alice-airgap                         │
│ • Created at: 2024-01-20 10:00:00                 │
│ • Expires at: 2024-01-22 10:00:00                 │
│ • Signature: ✅ Valid                              │
│                                                     │
│ DKG Parameters:                                     │
│ • Wallet: cold-storage                             │
│ • Your Role: Participant #2 (bob-airgap)          │
│ • Threshold: 2 of 3                                │
│                                                     │
│ [Import & Continue] [Reject] [View Raw]            │
└─────────────────────────────────────────────────────┘
```

#### 3. Round 1 - Commitment Generation

**Each Participant Generates Commitments:**
```
┌─────────────────────────────────────────────────────┐
│ 🔒 Generate DKG Commitments (Offline)               │
├─────────────────────────────────────────────────────┤
│ Round 1 - Local Generation                          │
│                                                     │
│ Operations:                                         │
│ ✅ Generated random polynomial                      │
│ ✅ Computed commitment values                       │
│ ✅ Created cryptographic proofs                     │
│ ✅ Self-verification passed                         │
│                                                     │
│ Commitment Data:                                    │
│ • Size: 2.3 KB                                      │
│ • Format: JSON (signed)                             │
│ • Includes: Public commitments only                │
│                                                     │
│ Ready to export to SD card:                        │
│ /mnt/secure-sd/round1/bob-commitments.json        │
│                                                     │
│ Instructions:                                       │
│ 1. Export your commitments                         │
│ 2. Deliver SD card to coordinator                  │
│ 3. Wait for aggregated commitments                 │
│                                                     │
│ [Export Commitments] [Regenerate]                  │
└─────────────────────────────────────────────────────┘
```

**Coordinator Aggregates Commitments:**
```
┌─────────────────────────────────────────────────────┐
│ 🔒 Aggregate Round 1 Commitments                    │
├─────────────────────────────────────────────────────┤
│ Commitment Collection Status:                       │
│                                                     │
│ Received Commitments:                               │
│ ✅ alice-airgap: alice-commitments.json           │
│ ✅ bob-airgap: bob-commitments.json               │
│ ⏳ charlie-airgap: Waiting...                      │
│                                                     │
│ Verification Results:                               │
│ • alice: ✅ Valid signature & proofs               │
│ • bob: ✅ Valid signature & proofs                 │
│                                                     │
│ [Refresh] [Import from SD] [Verify All]            │
│                                                     │
│ Once all commitments received:                      │
│ [Create Round 1 Package]                           │
└─────────────────────────────────────────────────────┘
```

#### 4. Round 2 - Share Distribution

**Participants Generate Shares:**
```
┌─────────────────────────────────────────────────────┐
│ 🔒 Generate Secret Shares (Offline)                 │
├─────────────────────────────────────────────────────┤
│ Round 2 - Share Generation                          │
│                                                     │
│ Imported Round 1 Package: ✅                        │
│ All commitments verified: ✅                        │
│                                                     │
│ Share Generation:                                   │
│ • For alice-airgap: ✅ Encrypted                   │
│ • For charlie-airgap: ✅ Encrypted                 │
│ • Self share: ✅ Stored locally                    │
│                                                     │
│ Export Package Contents:                            │
│ • Encrypted shares for others                      │
│ • Zero-knowledge proofs                            │
│ • Share commitments                                │
│                                                     │
│ Ready to export:                                    │
│ /mnt/secure-sd/round2/bob-shares.json             │
│                                                     │
│ [Export Shares] [Verify] [Back]                    │
└─────────────────────────────────────────────────────┘
```

**Share Verification:**
```
┌─────────────────────────────────────────────────────┐
│ 🔒 Verify Received Shares                           │
├─────────────────────────────────────────────────────┤
│ Share Import Status:                                │
│                                                     │
│ Received Shares:                                    │
│ • From alice-airgap: ✅ Valid                      │
│ • From charlie-airgap: ✅ Valid                    │
│                                                     │
│ Verification Steps:                                 │
│ ✅ Decrypted shares successfully                    │
│ ✅ Shares match commitment values                   │
│ ✅ Polynomial consistency verified                  │
│ ✅ Zero-knowledge proofs valid                      │
│                                                     │
│ Key Reconstruction Test:                            │
│ ✅ Successfully computed public key                 │
│ ✅ Address derivation successful                    │
│                                                     │
│ Your Key Share: Securely stored                    │
│                                                     │
│ [Complete DKG] [Export Summary]                    │
└─────────────────────────────────────────────────────┘
```

#### 5. Final Verification

**All Participants Confirm:**
```
┌─────────────────────────────────────────────────────┐
│ 🔒 Offline DKG Complete                             │
├─────────────────────────────────────────────────────┤
│ ✅ Cold Storage Wallet Created                      │
│                                                     │
│ Wallet Summary:                                     │
│ • Name: cold-storage                               │
│ • Type: 2-of-3 Bitcoin Wallet                      │
│ • Address: bc1qxy2kgdygjrsqtzq2n0yrf24...         │
│                                                     │
│ Security Verification:                              │
│ ✅ No network activity detected                     │
│ ✅ All operations performed offline                 │
│ ✅ Key material never exposed                       │
│ ✅ Shares encrypted at rest                         │
│                                                     │
│ Backup Reminder:                                    │
│ ⚠️  Create encrypted backup immediately            │
│ ⚠️  Store backup in separate location              │
│ ⚠️  Test recovery procedure                        │
│                                                     │
│ [Create Backup] [View Details] [Exit]              │
└─────────────────────────────────────────────────────┘
```

### Offline DKG Data Flow

```
Coordinator                 Participant 1              Participant 2
     |                           |                           |
     |-- DKG Parameters -------->|                           |
     |         (SD Card)         |-- DKG Parameters -------->|
     |                           |      (SD Card)            |
     |                           |                           |
     |<-- Round 1 Commitments ---|<-- Round 1 Commitments ---|
     |      (SD Card)            |      (SD Card)           |
     |                           |                           |
     |-- Aggregated Commitments->|-- Aggregated Commitments->|
     |      (SD Card)            |      (SD Card)           |
     |                           |                           |
     |<-- Round 2 Shares --------|<-- Round 2 Shares --------|
     |      (SD Card)            |      (SD Card)           |
     |                           |                           |
     |-- Share Packages -------->|-- Share Packages -------->|
     |      (SD Card)            |      (SD Card)           |
     |                           |                           |
     |==== Local Verify =========|==== Local Verify =========|
```

## Hybrid DKG Flow

The hybrid approach combines online coordination with offline key generation for enhanced security.

### Use Cases

1. **High-Value Wallets**: Online coordination, offline key generation
2. **Geographically Distributed Teams**: Mixed online/offline participants
3. **Regulatory Compliance**: Audit trail with air-gapped security

### Process Overview

```
┌─────────────────────────────────────────────────────┐
│ Hybrid DKG Configuration                            │
├─────────────────────────────────────────────────────┤
│ Coordination: Online (WebRTC)                       │
│ Key Generation: Offline (Air-gapped)                │
│                                                     │
│ Participants:                                       │
│ • alice: Online coordination + Offline keygen      │
│ • bob: Fully offline (SD card only)               │
│ • charlie: Online coordination + Offline keygen    │
│                                                     │
│ Workflow:                                           │
│ 1. Online: Establish session parameters            │
│ 2. Offline: Generate commitments                   │
│ 3. Online: Exchange commitments                    │
│ 4. Offline: Generate shares                        │
│ 5. Online: Exchange encrypted shares               │
│ 6. Offline: Verify and store                       │
│                                                     │
│ [Configure Details] [Start Hybrid DKG]             │
└─────────────────────────────────────────────────────┘
```

## Recovery Procedures

### Lost Key Share Recovery

If a participant loses their key share, the only currently-shipping
recovery path is restoring from backup:

- **Restore from backup**: Decrypt an exported keystore file —
  a single `<wallet_id>.json` per wallet (from
  `~/.frost_keystore/<device_id>/<curve>/`, or the extension-format
  export of the same shape) — with the original password. Works
  provided the participant kept a copy. The extension/TUI
  round-trip is test-covered (`extension_compat.rs`). Earlier
  drafts of this bullet described a `.json`+`.dat` pair; no
  `.dat` file is ever written (see f4fc866 for the broader
  keystore-layout retraction).

Earlier drafts of this section offered two more recovery methods
that do NOT exist in source today:

- **"Threshold Recovery: generate a new 2-of-3 wallet, Bob gets a new
  share"** — this is cryptographically incoherent. Re-running DKG
  produces a completely new group public key (and therefore a
  completely different on-chain address); it does not reissue a
  lost share for an existing key. A new DKG == a new wallet.
- **"Share Refresh Protocol"** — FROST supports proactive share
  refresh in principle (updating shares so the same group key is
  preserved while old shares become useless), but this crate does
  not implement it. Adding refresh is open work, tracked as a
  future item.

If a share is lost and no backup exists, the participant is out of
the threshold. If the remaining participants still hit the threshold
`t`, the wallet can still sign; if they don't, the funds under that
group key are permanently inaccessible — standard threshold-signature
failure mode.

### Emergency access

The TUI displays no presence / last-seen / timezone information for
participants. Earlier drafts of this section showed a panel with
"Alice: Last seen 2 hours ago" / "Time-Locked Recovery" / "Social
Recovery Protocol: 3 of 4 trustees online" — none of those features
exist. The only emergency options today are:

1. Gather threshold participants (possibly out-of-band) and sign the
   required transaction through the normal signing flow.
2. If threshold participation is impossible, the funds are
   inaccessible. Plan for this by keeping encrypted backups of every
   share in safe places ahead of time.

## Security Considerations

### DKG Security Model

```
┌─────────────────────────────────────────────────────┐
│ Security Properties                                 │
├─────────────────────────────────────────────────────┤
│ ✅ Guaranteed Properties:                           │
│ • No single party has complete key                 │
│ • Threshold parties required for signing           │
│ • Verifiable correct execution                     │
│ • Robust against t-1 malicious parties            │
│                                                     │
│ ⚠️  Assumptions:                                    │
│ • Secure communication channels                    │
│ • Honest majority during DKG                      │
│ • Secure local storage                            │
│ • Trusted execution environment                    │
│                                                     │
│ 🔒 Best Practices:                                  │
│ • Verify participant identities                    │
│ • Use offline DKG for high-value                  │
│ • Regular key share backups                       │
└─────────────────────────────────────────────────────┘

(Earlier drafts of this box also listed "Periodic share refresh"
as a best practice, contradicting the Recovery section above which
correctly notes that FROST share refresh is not implemented in
this crate today. Removed — add it back once
`frost-core::refresh` is actually wired up.)
```

### Attack Vectors and Mitigations

| Attack Vector | Impact | Mitigation |
|--------------|--------|------------|
| Malicious participant during DKG | Key compromise | Requires ≥t malicious parties |
| Network eavesdropping | Metadata leak | TLS/DTLS encryption |
| Commitment manipulation | Protocol failure | Cryptographic verification |
| Denial of service | DKG failure | Timeout and retry mechanisms |
| Key share theft | Partial compromise | Encrypted storage (AES-256-GCM + PBKDF2). No HSM integration — earlier drafts claimed HSM support; none exists. |
| Replay attacks | Double signing | FROST nonces are randomly generated per-signing; no separate nonce-tracking or explicit session-id validation layer is applied on top of the protocol. |

## Troubleshooting

### Common DKG Issues

#### "Timeout during Round 1"
```
┌─────────────────────────────────────────────────────┐
│ ⚠️  DKG Timeout Detected                            │
├─────────────────────────────────────────────────────┤
│ Issue: Round 1 timeout (300s exceeded)              │
│ Missing: charlie's commitments                      │
│                                                     │
│ Diagnostics:                                        │
│ • Network: ✅ Connected                             │
│ • Charlie status: 🔴 Disconnected (180s ago)      │
│ • Partial data: 2 of 3 commitments received       │
│                                                     │
│ Options:                                            │
│ 1. Wait for Charlie (extend timeout)               │
│ 2. Restart with available participants             │
│ 3. Switch to offline DKG                          │
│                                                     │
│ [Extend 5 min] [Restart] [Go Offline]             │
└─────────────────────────────────────────────────────┘
```

#### "Verification Failed"
```
┌─────────────────────────────────────────────────────┐
│ ❌ Share Verification Failed                        │
├─────────────────────────────────────────────────────┤
│ Error: Invalid share from participant 'bob'         │
│                                                     │
│ Details:                                            │
│ • Share doesn't match commitment                   │
│ • Polynomial evaluation incorrect                  │
│ • Possible corruption or attack                    │
│                                                     │
│ Automatic Actions Taken:                            │
│ ✅ Notified other participants                      │
│ ✅ Logged incident for audit                        │
│ ✅ Excluded bob from current round                  │
│                                                     │
│ Next Steps:                                         │
│ • Contact bob to verify software                  │
│ • Restart DKG without bob                         │
│ • Consider alternative participant                 │
│                                                     │
│ [View Technical Details] [Restart] [Abort]         │
└─────────────────────────────────────────────────────┘
```

### DKG Best Practices

1. **Pre-DKG Checklist**
   - Verify all participant identities
   - Test network connections
   - Clear previous failed attempts (old session data
     on disk)
   - Earlier drafts listed "Synchronize clocks" here —
     contradicts § Prerequisites retraction above; FROST
     DKG is not time-sensitive.

2. **During DKG**
   - Monitor progress actively
   - Keep stable network connection
   - Don't interrupt the process
   - Capture `tracing` output via `RUST_LOG=info` /
     `--log-location` if you want a post-hoc trace.
     Earlier drafts said "Save all logs for audit" — no
     audit-log emission ships (see SECURITY.md § Audit
     logs); `tracing` is diagnostic, not a
     tamper-evident audit trail.

3. **Post-DKG**
   - Create immediate backup (export each
     `<wallet_id>.json` to offline storage)
   - Document participant info
   - Earlier drafts listed "Test with small transaction"
     — the TUI doesn't construct or broadcast
     transactions (see guides/USER_GUIDE.md § Signing
     Messages → Scope). You can sign a test EIP-191
     message through the normal Sign flow to verify the
     key works, but "small transaction" would need an
     external wallet tool.
   - Earlier drafts also listed "Schedule regular health
     checks" — no health-check tooling ships.

4. **Security Hygiene**
   - Use dedicated devices for high-value wallets
   - Implement proper access controls (OS-level —
     keystore files are mode-600 by default via user
     umask)
   - Practice recovery procedures
   - Earlier drafts listed "Regular security audits" —
     no built-in audit tooling ships; regular external
     review is of course good practice but not a
     feature this codebase provides.