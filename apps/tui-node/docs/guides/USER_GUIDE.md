# MPC Wallet TUI — User Guide

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [User Interface Overview](#user-interface-overview)
4. [Creating Your First Wallet](#creating-your-first-wallet)
5. [Managing Wallets](#managing-wallets)
6. [Signing Messages](#signing-messages)
7. [Offline Operations](#offline-operations)
8. [Troubleshooting](#troubleshooting)

## Introduction

The MPC Wallet TUI is a terminal-based client for the FROST
threshold-signature scheme. It's menu-driven (not REPL / typed
commands), uses Ratatui via the tui-realm Elm-architecture
framework, and interoperates with the browser extension + native
desktop app over the same wire protocol and keystore format.

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
│ > 🆕 Create New Wallet                              │
│   🔗 Join Session                                   │
│   💼 Manage Wallets      (only when wallets > 0)   │
│   ✍️  Sign Transaction   (only when wallets > 0)   │
│   ⚙️  Settings                                      │
│   🚪 Exit                                           │
└─────────────────────────────────────────────────────┘
```

The real menu item list (see
`apps/tui-node/src/elm/components/main_menu.rs:55-114`) omits an
"Import Wallet" entry that earlier drafts of this guide listed —
wallet import happens via the Manage Wallets screen after creating
or discovering at least one wallet. There is also no standalone
"Help" item (no help modal ships, see Keyboard Shortcuts below).

### Navigation Basics

The interface is keyboard-only:

- **↑ / ↓**: Move between menu items
- **Enter**: Select the highlighted option
- **Esc**: Go back or cancel current operation
- **Tab**: Move focus within a screen (e.g., between input fields)
- **Ctrl+Q** / **Ctrl+C**: Quit (see § Appendix → Keyboard
  Shortcuts for all four Ctrl globals). Earlier drafts of this
  bullet listed plain lowercase `q` as a quit key — that's NOT
  wired up; the `Ctrl` modifier is required.

Earlier drafts also mentioned `←→` for "tab/panel switching" and
`?` for context-sensitive help. There are no tab panels in the
current layout (each screen is a single view), and no `?` help
overlay is wired up (zero hits in source for a help keybinding).

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

From the main menu, select "Create New Wallet". The real
`CreateWalletComponent` (`src/elm/components/create_wallet.rs`) is
a step-driven form — user progresses through Mode / Template /
Parameters + Curve screens rather than filling a single form page
with all fields at once. There is no per-peer "Participants to
invite" checkbox list — peer discovery is driven by the session
announcement + Join Session flow on each participant's side, not
a push-invite from the creator.

Earlier drafts of this section showed a single-page form with
fields for `Participants` + `Threshold` + `Blockchain` dropdowns
PLUS a `Participants to invite: ☐ bob ☐ charlie ☐ dave` checkbox
list and a `[Create] [Cancel]` button row. None of those UI
elements exist — the ratatui components in `create_wallet.rs`
don't draw dropdowns or checkboxes, and peer invitation isn't a
creator-side feature (each co-signer joins from their own node).

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

On successful completion the `WalletComplete` component
(`src/elm/components/wallet_complete.rs`) shows the new wallet's
metadata + derived address(es).

The keystore file is written at:

```
~/.frost_keystore/<device_id>/<curve>/<wallet_id>.json
```

Path is partitioned by `device_id` and curve
(`secp256k1` / `ed25519`) — NOT a flat top-level
`<name>.json` file as earlier drafts of this section claimed.
Unified DKG mints both curves from the same ceremony so each
participant ends up with a pair of files (one per curve).

Earlier drafts of this section also showed an ASCII mock with
`[View Wallet] [Export Backup] [Done]` action buttons. Real
ratatui components don't render button rows like that — the
screen has a fixed completion banner and you navigate with
Enter/Esc.

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

Before signing offline, the user is shown the decoded
`OfflineData` envelope (see `src/offline/types.rs:12`) — session
id, request type (`signing_request` / `commitments` / etc.),
created/expires timestamps, and the raw message bytes being
signed.

Earlier drafts of this section showed a rich "Review" modal with
a `Transaction Details` block (To / Amount / Gas) and
`[Approve & Sign] [Reject] [Export Details]` action buttons.
That UI does not ship — the TUI signs raw messages (EIP-191
`personal_sign` shape), not transactions with to-address /
amount / gas fields. See § "Signing Messages → Scope of what the
TUI signs" above for the honest accounting. For offline signing,
the user sees the message bytes + session identity and then
approves/rejects via Enter/Esc; there is no transaction-decoding
step.

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
  single `<wallet_id>.json` file to a chosen path. It's a
  `WalletFile` JSON wrapping plaintext metadata + the base64-
  encoded encrypted share (same format the browser extension
  produces, so extension ↔ TUI keystore import is a direct
  round-trip).
- **Import wallet**: reverse of the above; read a `.json` file
  and the password to unlock it.

Earlier drafts of this section described a `.json`+`.dat` pair
per wallet. No `.dat` file is produced — everything lives in the
one JSON (retraction previously landed in f4fc866 for the broader
keystore layout but missed this spot).

There is **no** "Backup & Recovery Center" screen with Full Backup /
HSM / Mnemonic options. Earlier drafts promised:

- "Export for Hardware Security Module — Compatible with Ledger,
  Trezor (Beta)" — no HSM integration exists.
- "Recover from mnemonic (Limited)" — FROST-generated keys don't
  have a mnemonic. The key is distributed; each participant holds
  a share, not a BIP-39 seed.

For offline backups, the working path is: export each wallet
(single `<wallet_id>.json` file — the `WalletFile` JSON already
wraps AES-256-GCM ciphertext internally), store the file
elsewhere (ideally geographically distributed), and keep the
password safe. Losing the password renders the exported JSON
undecryptable. (Earlier drafts of this bullet described a
`.json`+`.dat` pair and spoke of a `.dat` file becoming useless
— no `.dat` file is produced; the ciphertext is base64-embedded
in the JSON's `data` field.)

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
   - Choose unique device IDs that don't leak real-world identity
     (the device_id shows up as the `from` field on signal-server
     relay frames, so it's visible to every participant in your
     sessions).
   - Keep the device_id consistent across sessions for a given
     physical device — the keystore tree is partitioned by
     `<device_id>/<curve>/`, so changing the flag re-homes which
     directory the TUI reads/writes (earlier drafts of this bullet
     suggested "Rotate device IDs periodically"; rotating would
     orphan existing wallets under the old device_id path).

2. **Network Security**
   - Connections to the signal server use `wss://` (TLS via the
     system CA store; no certificate pinning is implemented in the
     TUI today — that's open hardening work).
   - Offline mode (`--offline`) is the stronger answer for
     high-value ceremonies: no signaling, no metadata leakage,
     the mesh never forms over public networks.

3. **Keystore Management**
   - Regular encrypted backups — export each
     `<wallet_id>.json` (it's already AES-256-GCM encrypted, but
     store in multiple places anyway).
   - Losing the password renders the `data` ciphertext
     undecryptable; there is no recovery path without the
     password.
   - Test the unlock flow periodically so you catch password-
     management issues before they matter.

### Operational Guidelines

1. **Session Management**
   - Close completed sessions promptly (creator disconnecting
     removes the session from the signal server's registry)
   - Review participant lists before starting — the session
     `participants` list is persisted into `WalletMetadata` and
     used for cold-start rejoin
   - Earlier drafts suggested "Set appropriate session timeouts" —
     there is no session-timeout mechanism (see SECURITY.md and
     ARCHITECTURE.md); sessions persist until the creator disconnects.

2. **Signing Verification**
   - Double-check the hex message bytes you paste into the Sign
     Message screen — the TUI signs raw bytes as-given (EIP-191
     personal_sign shape); if you wanted to sign an Ethereum
     transaction, you must RLP-encode + keccak256-hash it
     externally first and hand the resulting hex to the TUI
   - After the ceremony, verify the signature against the wallet's
     group public key via `frost-core::VerifyingKey::verify` (or
     `ecrecover` for secp256k1 + EIP-191 shape)
   - Earlier drafts said "Verify amounts and gas settings" and
     "Use test transactions" — the TUI does NOT decode or display
     transaction amount / gas / to-address fields (it only sees
     bytes); those checks happen in whatever tool constructs the
     transaction before handoff

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
| `Ctrl+Q`   | Global  | Quit application    |
| `Ctrl+C`   | Global  | Quit application    |
| `Ctrl+R`   | Global  | Refresh             |
| `Ctrl+H`   | Global  | Navigate to home    |

Earlier drafts of this table listed additional shortcuts (`?`
global help, `/` quick search, `r` refresh balances, `e` export,
`o` accept notification, plain `q` quit) — none of those are
wired up in the current code. Quit requires the Ctrl modifier
(see `src/elm/app.rs:851`): plain lowercase `q` is NOT a quit
key. The four Ctrl-modified globals all live in `app.rs:851-866`
before per-component dispatch.

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
per-domain typed error enums: `KeystoreError`
(`src/keystore/mod.rs:24`), `FrostKeystoreError`
(`src/keystore/frost_keystore.rs:19`), `OfflineError`
(`src/offline/mod.rs:24`), `CoreError` (`src/core/mod.rs:21`),
plus upstream `FrostError` from `packages/@mpc-wallet/frost-core`.
Grep the source by error message or enum name when debugging.
There is no top-level `src/errors.rs` umbrella file despite what
earlier drafts of this section claimed.