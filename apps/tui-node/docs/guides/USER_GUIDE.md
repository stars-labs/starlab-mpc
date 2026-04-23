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

Access your wallets from the main menu:

```
┌─────────────────────────────────────────────────────┐
│ Your Wallets                                        │
├─────────────────────────────────────────────────────┤
│ > company-treasury (2/3)                   Ethereum │
│   Balance: 5.432 ETH                                │
│   Created: 2024-01-20                               │
│   Last used: 2 hours ago                            │
│                                                     │
│   defi-operations (3/5)                     Solana  │
│   Balance: 1,234.56 SOL                             │
│   Created: 2024-01-15                               │
│   Last used: 1 day ago                              │
│                                                     │
│ [Enter: Details] [E: Export] [B: Backup] [D: Delete]│
└─────────────────────────────────────────────────────┘
```

### Wallet Details

Selecting a wallet shows comprehensive information:

```
┌─────────────────────────────────────────────────────┐
│ Wallet Details: company-treasury                    │
├─────────────────────────────────────────────────────┤
│ Configuration:                                      │
│ • Threshold: 2 of 3                                 │
│ • Blockchain: Ethereum (secp256k1)                 │
│ • Created: 2024-01-20 14:30:00                     │
│                                                     │
│ Address:                                            │
│ 0x742d35Cc6634C0532925a3b844Bc9e7595f7A          │
│                                                     │
│ Participants:                                       │
│ 1. alice (You) - Key Share #1                      │
│ 2. bob - Key Share #2                              │
│ 3. charlie - Key Share #3                          │
│                                                     │
│ Recent Activity:                                    │
│ • 2024-01-20 16:45 - Signed transaction (2 of 3)   │
│ • 2024-01-20 15:30 - Wallet created                │
│                                                     │
│ [Sign Transaction] [Export] [Back]                  │
└─────────────────────────────────────────────────────┘
```

## Signing Transactions

### Initiating a Signing Session

From wallet details or main menu:

```
┌─────────────────────────────────────────────────────┐
│ Sign Transaction - company-treasury                 │
├─────────────────────────────────────────────────────┤
│ Transaction Type: [Ethereum Transfer] ▼             │
│                                                     │
│ To Address:                                         │
│ [0x9876543210987654321098765432109876543210___]   │
│                                                     │
│ Amount: [1.5___] ETH                                │
│                                                     │
│ Gas Settings:                                       │
│ • Max Fee: [50] gwei                                │
│ • Priority: [2] gwei                                │
│                                                     │
│ Message/Note (optional):                            │
│ [Q1 2024 contractor payment_________________]      │
│                                                     │
│ Required Signers: 2 of 3                            │
│ Available: alice ✓, bob ✓, charlie ✗              │
│                                                     │
│ [Initiate Signing] [Cancel]                         │
└─────────────────────────────────────────────────────┘
```

### Signing Process

During the signing process:

```
┌─────────────────────────────────────────────────────┐
│ Signing in Progress - Transaction #4521             │
├─────────────────────────────────────────────────────┤
│ Transaction Summary:                                │
│ • From: company-treasury (2-of-3)                  │
│ • To: 0x987...3210                                 │
│ • Amount: 1.5 ETH                                  │
│ • Note: Q1 2024 contractor payment                 │
│                                                     │
│ Signature Collection:                               │
│ • alice    ✅ Signed at 10:45:23                   │
│ • bob      ⏳ Reviewing transaction...             │
│ • charlie  ⬜ Not participating                    │
│                                                     │
│ Progress: 1 of 2 signatures collected              │
│ ▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░ 50%                          │
│                                                     │
│ Status: Waiting for 1 more signature...            │
│                                                     │
│ [View Details] [Cancel]                            │
└─────────────────────────────────────────────────────┘
```

### Transaction Completion

Once threshold is reached:

```
┌─────────────────────────────────────────────────────┐
│ ✅ Transaction Signed Successfully!                 │
├─────────────────────────────────────────────────────┤
│ Transaction Hash:                                   │
│ 0xf3d4e2c1b0a9e8d7c6b5a4f3e2d1c0b9a8e7d6c5b4a3   │
│                                                     │
│ Signatures collected: 2 of 2 required               │
│ • alice - Signed at 10:45:23                       │
│ • bob - Signed at 10:46:15                         │
│                                                     │
│ Transaction Status: Broadcasting...                 │
│                                                     │
│ [View on Etherscan] [Copy TX Hash] [Done]         │
└─────────────────────────────────────────────────────┘
```

## Offline Operations

### Enabling Offline Mode

For air-gapped security, switch to offline mode:

```
┌─────────────────────────────────────────────────────┐
│ 🔒 Offline Mode Settings                            │
├─────────────────────────────────────────────────────┤
│ Current Status: Online                              │
│                                                     │
│ ⚠️  Switching to offline mode will:                │
│ • Disable all network connections                  │
│ • Require SD card for data transfer                │
│ • Enable air-gapped signing workflow               │
│                                                     │
│ SD Card Mount Point: [/mnt/sdcard___]              │
│                                                     │
│ ☑ Verify SD card is formatted and empty            │
│ ☑ I understand the offline workflow                │
│                                                     │
│ [Switch to Offline] [Cancel]                       │
└─────────────────────────────────────────────────────┘
```

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

### Multi-Wallet Signing Queue

Manage multiple signing requests:

```
┌─────────────────────────────────────────────────────┐
│ Pending Signatures (3)                      🔔      │
├─────────────────────────────────────────────────────┤
│ Priority | Wallet           | Details      | Time   │
├─────────────────────────────────────────────────────┤
│ HIGH     │ company-treasury | 50 ETH       | 2 min  │
│          │ To: 0xABC...123  | Payroll      |        │
│          │ [Sign] [Details] [Skip]         |        │
├─────────────────────────────────────────────────────┤
│ MEDIUM   │ defi-operations  | Compound     | 15 min │
│          │ Supply 1000 USDC | Lending      |        │
│          │ [Sign] [Details] [Skip]         |        │
├─────────────────────────────────────────────────────┤
│ LOW      │ test-wallet      | 0.1 ETH      | 1 hour │
│          │ To: 0xDEF...456  | Test TX      |        │
│          │ [Sign] [Details] [Skip]         |        │
├─────────────────────────────────────────────────────┤
│ [Sign All Compatible] [Settings] [Close]           │
└─────────────────────────────────────────────────────┘
```

### Backup and Recovery

Comprehensive backup interface:

```
┌─────────────────────────────────────────────────────┐
│ Backup & Recovery Center                            │
├─────────────────────────────────────────────────────┤
│ Backup Options:                                     │
│                                                     │
│ > Full Backup (Recommended)                         │
│   Includes all wallets and settings                │
│   Size: ~2.3 MB                                     │
│                                                     │
│   Individual Wallet Backup                          │
│   Select specific wallets to backup                 │
│                                                     │
│   Export for Hardware Security Module               │
│   Compatible with Ledger, Trezor (Beta)            │
│                                                     │
│ Recovery Options:                                   │
│                                                     │
│   Restore from Backup File                         │
│   Import from another device                       │
│   Recover from mnemonic (Limited)                  │
│                                                     │
│ [Select Option] [Help] [Cancel]                    │
└─────────────────────────────────────────────────────┘
```

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

Press `?` at any time for context-sensitive help:

```
┌─────────────────────────────────────────────────────┐
│ Help - Current Context: Wallet List                 │
├─────────────────────────────────────────────────────┤
│ Available Actions:                                  │
│                                                     │
│ Navigation:                                         │
│ • ↑/↓ - Move between wallets                       │
│ • Enter - View wallet details                      │
│ • → - Quick actions menu                           │
│                                                     │
│ Shortcuts:                                          │
│ • S - Start signing session                        │
│ • C - Create new wallet                            │
│ • E - Export selected wallet                       │
│ • D - Delete wallet (requires confirmation)        │
│ • R - Refresh wallet balances                      │
│                                                     │
│ Global:                                             │
│ • ? - This help screen                             │
│ • Esc - Return to main menu                       │
│ • q - Quit application                             │
│                                                     │
│ [Close]                                            │
└─────────────────────────────────────────────────────┘
```

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

| Shortcut | Context | Action |
|----------|---------|--------|
| ? | Global | Show help |
| q | Global | Quit application |
| Esc | Global | Go back/Cancel |
| Tab | Global | Next element |
| Shift+Tab | Global | Previous element |
| Enter | Global | Select/Confirm |
| ↑↓←→ | Global | Navigate |
| / | Main Menu | Quick search |
| n | Wallet List | New wallet |
| s | Wallet View | Start signing |
| e | Any List | Export selected |
| r | Any List | Refresh |
| o | Notifications | Open/Accept |
| Space | Checkboxes | Toggle selection |

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

| Code | Description | Solution |
|------|-------------|----------|
| E001 | Network timeout | Check connection, retry |
| E002 | Invalid threshold | Threshold must be ≤ participants |
| E003 | Keystore locked | Unlock with password |
| E004 | Session expired | Create new session |
| E005 | Signature invalid | Verify key shares |
| E006 | Insufficient peers | Wait for more participants |
| E007 | SD card not found | Check mount point |
| E008 | Backup corrupted | Use alternate backup |
| E009 | Version mismatch | Update all clients |
| E010 | Permission denied | Check file permissions |