# FROST MPC TUI Wallet - User Guide

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [User Interface Overview](#user-interface-overview)
4. [Creating Your First Wallet](#creating-your-first-wallet)
5. [Managing Wallets](#managing-wallets)
6. [Signing Transactions](#signing-transactions)
7. [Offline Operations](#offline-operations)
8. [Advanced Features](#advanced-features)
9. [Troubleshooting](#troubleshooting)

## Introduction

The FROST MPC TUI Wallet provides enterprise-grade multi-party computation through an intuitive terminal interface. Unlike traditional CLI tools that require memorizing commands, our TUI offers a complete menu-driven experience accessible to users of all technical levels.

### Key Concepts

- **MPC (Multi-Party Computation)**: Cryptographic technique where multiple parties jointly compute functions over their inputs while keeping those inputs private
- **Threshold Signatures**: Require a minimum number of participants (threshold) out of the total to create a valid signature
- **DKG (Distributed Key Generation)**: Process where participants jointly generate a key that no single party fully controls
- **TUI (Terminal User Interface)**: Visual interface in the terminal with menus, windows, and interactive elements

## Getting Started

### First Launch

When you start the wallet for the first time:

```bash
mpc-wallet-tui --device-id <your-unique-id>
```

You'll see the main interface:

```
┌─────────────────────────────────────────────────────┐
│ MPC Wallet TUI v0.1.0 - Device: alice              │
├─────────────────────────────────────────────────────┤
│ Main Menu:                                          │
│ > Create New Wallet                                 │
│   Import Wallet                                     │
│   Join Session                                      │
│   Settings                                          │
│   Help                                              │
│   Exit                                              │
├─────────────────────────────────────────────────────┤
│ Status: Connected to signal server                  │
│ Network: Online | Wallets: 0 | Sessions: 0         │
└─────────────────────────────────────────────────────┘
```

### Navigation Basics

The interface is designed for keyboard navigation:

- **Arrow Keys (↑↓)**: Move between menu items
- **Arrow Keys (←→)**: Switch between tabs/panels
- **Enter**: Select the highlighted option
- **Escape**: Go back or cancel current operation
- **Tab**: Move to next interactive element
- **?**: Show context-sensitive help
- **q**: Quit (with confirmation)

## User Interface Overview

### Main Screen Layout

```
┌──────────────────────────────────────────────────────────┐
│ [1] Title Bar - Shows wallet version and device ID      │
├──────────────────────────────────────────────────────────┤
│ [2] Menu/Content Area - Main interaction space          │
│                                                          │
│     • In menu mode: Shows available options             │
│     • In session: Shows participant status              │
│     • In wallet view: Shows wallet details              │
│                                                          │
├──────────────────────────────────────────────────────────┤
│ [3] Activity Log - Real-time updates and messages       │
│                                                          │
│     [2024-01-20 10:15:23] Connected to signal server    │
│     [2024-01-20 10:15:24] Discovered 2 online devices   │
│                                                          │
├──────────────────────────────────────────────────────────┤
│ [4] Status Bar - Connection, mode, and quick stats      │
└──────────────────────────────────────────────────────────┘
```

### Visual Indicators

The TUI uses colors and symbols to convey information:

- 🟢 **Green**: Connected, ready, successful operations
- 🟡 **Yellow**: Pending, waiting, in-progress
- 🔴 **Red**: Disconnected, errors, warnings
- 🔵 **Blue**: Information, neutral states
- 🔒 **Lock**: Encrypted or secure operations
- 📡 **Antenna**: Network operations
- 💾 **Disk**: Storage operations

## Creating Your First Wallet

### Step 1: Initiate Wallet Creation

From the main menu, select "Create New Wallet":

```
┌─────────────────────────────────────────────────────┐
│ Create New Wallet                                   │
├─────────────────────────────────────────────────────┤
│ Wallet Name: [company-treasury___]                  │
│                                                     │
│ Participants:    [3] ▼                              │
│ Threshold:       [2] ▼                              │
│ Blockchain:      [Ethereum (secp256k1)] ▼          │
│                                                     │
│ Participants to invite:                             │
│ ☐ bob (online)                                      │
│ ☐ charlie (online)                                  │
│ ☐ dave (offline)                                    │
│                                                     │
│ [Create] [Cancel]                                   │
└─────────────────────────────────────────────────────┘
```

### Step 2: Configure Parameters

1. **Wallet Name**: Choose a descriptive name (e.g., "company-treasury", "defi-operations")
2. **Participants**: Total number of key holders
3. **Threshold**: Minimum signatures required (must be ≤ participants)
4. **Blockchain**: Select target blockchain (determines curve type)

### Step 3: DKG Process

Once initiated, the DKG process begins:

```
┌─────────────────────────────────────────────────────┐
│ DKG in Progress - company-treasury (2 of 3)        │
├─────────────────────────────────────────────────────┤
│ Stage: Key Generation Round 1                       │
│                                                     │
│ Participants:                                       │
│ • alice    [████████████████████] Ready            │
│ • bob      [████████████░░░░░░░░] Generating...    │
│ • charlie  [████████████████████] Ready            │
│                                                     │
│ Progress: Round 1 of 2                              │
│ ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░ 67%                │
│                                                     │
│ Status: Waiting for all participants...             │
└─────────────────────────────────────────────────────┘
```

### Step 4: Wallet Created

Upon successful completion:

```
┌─────────────────────────────────────────────────────┐
│ ✅ Wallet Created Successfully!                     │
├─────────────────────────────────────────────────────┤
│ Wallet: company-treasury                            │
│ Type: 2-of-3 Ethereum Wallet                       │
│ Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f7A │
│                                                     │
│ Your key share has been encrypted and saved.       │
│ Location: ~/.frost_keystore/company-treasury.json  │
│                                                     │
│ ⚠️  Important: Back up your keystore file!         │
│                                                     │
│ [View Wallet] [Export Backup] [Done]               │
└─────────────────────────────────────────────────────┘
```

## Managing Wallets

### Wallet List View

The "Manage Wallets" menu item opens the WalletList component
(`src/elm/components/wallet_list.rs`). It shows the `WalletMetadata`
fields the keystore holds — threshold, total participants, curve,
creation timestamp — for each wallet in `~/.frost_keystore/`.

Earlier drafts of this section showed per-wallet `Balance` rows
(5.432 ETH, 1,234.56 SOL) and a `Last used: 2 hours ago` indicator.
The TUI does **not** query blockchain RPCs or track "last used"
timestamps — no balance data, no activity heatmap. The list shows
only what's stored in the keystore metadata `.json` files.

The real shortcuts on this screen are the tui-realm navigation
keys (`↑` / `↓` / `Enter` for details / `Esc` to go back).
Single-letter shortcuts like `E` / `B` / `D` for Export / Backup /
Delete are not implemented globally — operations happen through
the wallet-detail screen.

### Wallet Details

Selecting a wallet opens the WalletDetail component
(`src/elm/components/wallet_detail.rs`), which shows:

- Wallet name (session_id)
- Threshold + total participants
- Curve (secp256k1 / ed25519)
- Creation timestamp
- Derived addresses per blockchain (Ethereum / Solana) — these are
  computed on-demand from the stored `group_public_key` (see
  `WalletMetadata::derive_ethereum_address` /
  `derive_solana_address` in `src/keystore/models.rs`)
- Participant list (device IDs from the DKG session)

The screen does **not** show "Recent Activity" or a signing
history — earlier drafts of this section listed entries like
"2024-01-20 16:45 - Signed transaction (2 of 3)", which would
require an activity log that doesn't exist (same scope note as
the absent audit log in SECURITY.md, fixed in 6d7fd5a).

Available actions from this screen lead to the Sign Message flow
(see below) or go back to the wallet list.

## Signing Messages

### Scope of what the TUI signs

The TUI produces **EIP-191 `personal_sign` signatures** over a
user-supplied message string (see the docstring of
`src/elm/components/sign_transaction.rs`: "Phase C scope:
message-only field"). It does NOT:

- build Ethereum transactions (no to-address / amount / gas-price /
  gas-limit / nonce fields)
- broadcast transactions to any chain (no RPC integration)
- open Etherscan / block explorer links
- track transaction hashes on-chain

Earlier drafts of this guide showed a "Sign Transaction" form with
Transaction Type / To Address / Amount / Max Fee / Priority Fee /
Broadcasting Status / View-on-Etherscan button. None of that UI
exists. To actually send a signed transaction on-chain, take the
raw signature output from the TUI and hand it to an external tool
(wallet front-end, ethers/web3 script, etc.) that constructs and
broadcasts the transaction.

### Initiating a signing session

From the wallet list or wallet detail screen, select "Sign". The
real Sign screen is a single-field message input:

```
┌── 🖊️  Sign with <wallet_id> ────────────┐
│ Group key: <short>...                 │
│                                        │
│ ┌ Message to sign ─────────────────┐   │
│ │ <user text>_                     │   │
│ └──────────────────────────────────┘   │
│                                        │
│ <error, if any>                        │
│                                        │
│ Enter = Sign    Esc = Cancel           │
└────────────────────────────────────────┘
```

### Signing process

Once the user submits the message, the TUI kicks off a FROST
signing ceremony. Progress is shown in the `DKGProgress`-style
gauge (the same component handles both DKG and signing ceremonies
— name is historical). Participants are fetched from the active
session; each must have their key share unlocked before they can
contribute a signature share.

### Completion

On successful aggregation, the TUI shows the raw signature hex
via the `SignatureComplete` component. Copy it out for use in
whatever external tool will broadcast the actual transaction. No
on-chain activity happens inside the TUI.

## Offline Operations

### Enabling Offline Mode

Offline mode is a **startup-time** decision, not a runtime toggle:
launch with the `--offline` CLI flag.

```bash
mpc-wallet-tui --device-id alice --offline
```

Earlier drafts of this guide showed an in-app "Switch to Offline"
settings screen with SD-card mount-point configuration. No such
screen exists. The `--offline` flag tells the TUI to skip WebSocket
signaling + peer discovery; file paths for SD-card export/import
are chosen via file dialog / entered path when the specific action
runs (not configured globally).

### Offline Signing Workflow

In offline mode, the UI guides you through each step:

```
┌─────────────────────────────────────────────────────┐
│ 🔒 Offline Signing - Step 1: Import Request         │
├─────────────────────────────────────────────────────┤
│ Insert SD card with signing request                │
│                                                     │
│ Expected file: /mnt/sdcard/signing_request.json    │
│                                                     │
│ Status: ⏳ Waiting for SD card...                  │
│                                                     │
│ Detected files:                                     │
│ • No SD card detected                               │
│                                                     │
│ Instructions:                                       │
│ 1. Insert SD card from coordinator                 │
│ 2. Wait for auto-detection                         │
│ 3. Review and approve request                      │
│                                                     │
│ [Refresh] [Manual Import] [Cancel]                 │
└─────────────────────────────────────────────────────┘
```

### Offline Data Review

Before signing offline:

```
┌─────────────────────────────────────────────────────┐
│ 🔒 Review Offline Signing Request                   │
├─────────────────────────────────────────────────────┤
│ Request ID: sig_20240120_4521                      │
│ Created: 2024-01-20 10:30:00                       │
│ Expires: 2024-01-20 11:30:00                       │
│                                                     │
│ Transaction Details:                                │
│ • Wallet: company-treasury (2-of-3)                │
│ • Type: Ethereum Transfer                           │
│ • To: 0x987...3210                                 │
│ • Amount: 1.5 ETH                                  │
│ • Gas: 50 gwei max                                 │
│                                                     │
│ Required Participants: 2 of 3                      │
│ • alice (You)                                       │
│ • bob                                               │
│ • charlie                                           │
│                                                     │
│ ⚠️  Verify details match expected transaction       │
│                                                     │
│ [Approve & Sign] [Reject] [Export Details]         │
└─────────────────────────────────────────────────────┘
```

## Advanced Features

### Session Discovery

View and join available sessions:

```
┌─────────────────────────────────────────────────────┐
│ Available Sessions                                  │
├─────────────────────────────────────────────────────┤
│ > team-wallet (DKG)                        2 of 3   │
│   Proposer: bob                                     │
│   Participants: bob, charlie                        │
│   Status: Waiting for 1 more participant           │
│   Created: 5 minutes ago                            │
│                                                     │
│   monthly-bills (Signing)                   3 of 5  │
│   Wallet: operations-wallet                         │
│   Proposer: alice                                   │
│   Status: Collecting signatures (2/3)              │
│   Created: 2 minutes ago                            │
│                                                     │
│ [Enter: Join] [R: Refresh] [F: Filter]             │
└─────────────────────────────────────────────────────┘
```

### Pending signing requests

Pending signing sessions appear in the main menu under "Signing
Requests". One-at-a-time review / approve / decline — no
prioritized queue, no "HIGH/MEDIUM/LOW" labels, no batch-sign
action. Earlier drafts of this guide showed a Priority/Wallet/
Details multi-row table with a `[Sign All Compatible]` button;
that UI is not implemented.

### Backup and Recovery

The TUI has two import/export surfaces:

- **Export wallet**: from the wallet-detail screen, write a
  `.json`+`.dat` pair to a chosen path using the keystore's
  encrypted format (also the extension-compatible format — the
  browser extension can import the same file).
- **Import wallet**: reverse of the above; read a `.json`+`.dat`
  pair plus the password to unlock it.

There is **no** "Backup & Recovery Center" screen with Full Backup /
HSM / Mnemonic options. Earlier drafts promised:

- "Export for Hardware Security Module — Compatible with Ledger,
  Trezor (Beta)" — no HSM integration exists.
- "Recover from mnemonic (Limited)" — FROST-generated keys don't
  have a mnemonic. The key is distributed; each participant holds
  a share, not a BIP-39 seed.

For offline backups, the working path is: export each wallet
(`.json`+`.dat` pair), encrypt/store it elsewhere (ideally
geographically distributed), and keep the password safe. Losing
the password renders a `.dat` file useless.

## Troubleshooting

### Common Issues and Solutions

#### Connection Problems

```
┌─────────────────────────────────────────────────────┐
│ ⚠️  Connection Troubleshooting                      │
├─────────────────────────────────────────────────────┤
│ Issue: Cannot connect to signal server              │
│                                                     │
│ Diagnostics:                                        │
│ • Network: ✅ Connected                             │
│ • DNS: ✅ Resolved xiongchenyu.dpdns.org           │
│ • Server: ❌ Connection refused                     │
│                                                     │
│ Possible solutions:                                 │
│ 1. Check firewall settings (port 443)              │
│ 2. Verify proxy configuration                      │
│ 3. Try alternative server                          │
│                                                     │
│ [Retry] [Change Server] [Offline Mode]             │
└─────────────────────────────────────────────────────┘
```

#### Signing Failures

```
┌─────────────────────────────────────────────────────┐
│ ❌ Signing Failed                                   │
├─────────────────────────────────────────────────────┤
│ Error: Insufficient signatures collected            │
│                                                     │
│ Required: 2 signatures                              │
│ Collected: 1 signature                              │
│                                                     │
│ Details:                                            │
│ • alice: ✅ Signed                                  │
│ • bob: ⏱️  Timeout (no response for 10 min)       │
│ • charlie: ❌ Rejected (invalid transaction)       │
│                                                     │
│ Options:                                            │
│ • Wait for bob to come online                      │
│ • Request signature from backup participant        │
│ • Cancel and create new signing session            │
│                                                     │
│ [Retry] [Contact Participants] [Cancel]            │
└─────────────────────────────────────────────────────┘
```

### Getting Help

There is no context-sensitive `?` help screen (verified: zero
keybinding hits for `?` in source). Earlier drafts of this
section showed a Help overlay with per-context shortcuts; not
implemented.

For keybinding reference, see the Keyboard Shortcuts appendix
below — but note that's a reference list, not a feature of the
running TUI. The [`KEYBOARD_NAVIGATION_GUIDE.md`](../KEYBOARD_NAVIGATION_GUIDE.md)
sibling doc has the more detailed per-screen walkthrough.

## Best Practices

### Security Recommendations

1. **Device ID Security**
   - Use unique, non-identifying device IDs
   - Never share device IDs publicly
   - Rotate device IDs periodically for sensitive operations

2. **Network Security**
   - Always verify signal server certificates
   - Use VPN for additional privacy
   - Consider offline mode for high-value transactions

3. **Keystore Management**
   - Regular encrypted backups
   - Store backups in multiple secure locations
   - Test recovery procedures periodically

### Operational Guidelines

1. **Session Management**
   - Close completed sessions promptly
   - Review participant lists before starting
   - Set appropriate session timeouts

2. **Transaction Verification**
   - Always double-check addresses
   - Verify amounts and gas settings
   - Use test transactions for new setups

3. **Backup Strategy**
   - Backup after every wallet creation
   - Maintain offline copies
   - Document recovery procedures

## Appendix

### Keyboard Shortcuts Reference

Per-screen keybindings are handled by the individual tui-realm
`Component` impls in `src/elm/components/`. The authoritative list
is [`KEYBOARD_NAVIGATION_GUIDE.md`](../KEYBOARD_NAVIGATION_GUIDE.md);
below is just the core keys that work globally.

| Shortcut   | Context | Action              |
|------------|---------|---------------------|
| `↑` / `↓`  | Global  | Navigate menu items |
| `Enter`    | Global  | Select / confirm    |
| `Esc`      | Global  | Go back / cancel    |
| `Tab`      | Global  | Move focus          |
| `q`        | Global  | Quit application    |

Earlier drafts of this table listed additional shortcuts (`?`
global help, `/` quick search, `r` refresh balances, `e` export,
`o` accept notification) — none of those are wired up in the
current code.

### Status Indicators

| Symbol | Meaning |
|--------|---------|
| 🟢 | Online/Connected/Ready |
| 🟡 | Pending/In Progress |
| 🔴 | Offline/Error/Failed |
| 🔵 | Information/Neutral |
| ⏳ | Waiting/Loading |
| ✅ | Completed/Success |
| ❌ | Failed/Rejected |
| 🔒 | Encrypted/Secure |
| 📡 | Network Activity |
| 💾 | Storage Operation |
| 🔔 | Notification/Alert |

### Error Codes

The TUI does not currently surface numeric `E001`-style error codes;
errors are shown as descriptive messages derived from the
`DKGError` / `SigningError` / `KeystoreError` variants in
`src/errors.rs`. Grep the source by error message or error-type name
when debugging.