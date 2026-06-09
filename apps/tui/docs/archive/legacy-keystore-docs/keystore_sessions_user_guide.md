# Keystore Sessions User Guide

## Overview

The FROST MPC CLI Node now supports automatic session type detection based on wallet names. This simplifies the workflow by using a single command for both creating new wallets (DKG) and signing with existing wallets.

## Quick Start

### 1. Creating a New Wallet (DKG Session)

To create a new distributed wallet, simply propose a session with a name that doesn't exist:

```bash
/propose company-wallet 3 2 alice,bob,charlie
```

The system will:
- Check if "company-wallet" exists
- Since it doesn't exist, start a DKG session
- After successful DKG, save the wallet as "company-wallet"

### 2. Signing with Existing Wallet

To sign transactions with an existing wallet, use the same `/propose` command with the wallet name:

```bash
/propose company-wallet 3 2 alice,bob,charlie
```

The system will:
- Find the existing "company-wallet"
- Verify that parameters match (3 participants, threshold 2)
- Start a signing session

### 3. Listing Available Wallets

To see all your wallets:

```bash
/wallets
```

Output:
```
═══════════════════════════════════════════════════
Your wallets:
═══════════════════════════════════════════════════
• company-wallet (2/3, ed25519) - 3 devices
  Created: 2024-01-20
• test-wallet (2/3, secp256k1) - 3 devices
  Created: 2024-01-15
═══════════════════════════════════════════════════

Use wallet name with /propose to start signing
```

## Session Types

### DKG Sessions
- **Purpose**: Create new distributed wallets
- **When**: Automatically triggered when wallet name doesn't exist
- **Result**: New wallet saved with session name

### Signing Sessions
- **Purpose**: Sign transactions using existing wallets
- **When**: Automatically triggered when wallet name exists
- **Requirement**: All participants must have the wallet

## Status Display

The session status now shows the type:

```
Session: company-wallet DKG (3 of 3, threshold 2)
```

or

```
Session: company-wallet Sign[company-wallet] (3 of 3, threshold 2)
```

## Error Handling

### Parameter Mismatch

If you try to use wrong parameters for an existing wallet:

```bash
/propose company-wallet 2 2 alice,bob
```

Error:
```
❌ Cannot proceed: Parameter mismatch
Wallet 'company-wallet' requires 3 participants (you specified: 2)
Correct usage: /propose company-wallet 3 2 alice,bob,charlie
```

### Missing Wallet on Participant

When a participant doesn't have the required wallet:

```
📥 Signing session invitation: company-wallet
⚠️ Wallet 'company-wallet' not found in local keystore
Options:
[1] Request wallet from participants (not implemented yet)
[2] Import wallet from backup (use /import_wallet)
[3] Join as observer (not implemented yet)
```

## Best Practices

1. **Meaningful Names**: Use descriptive wallet names like "company-prod", "team-testing"
2. **Consistent Naming**: All participants should use the same wallet name
3. **Check First**: Use `/wallets` to see available wallets before proposing
4. **Backup Wallets**: After DKG, backup your wallets for recovery

## Getting Help

Press `?` at any time to see a complete help popup with all available commands and keyboard shortcuts.

## Commands Reference

### Quick Keys (Normal Mode)
- `?` - Show complete help popup
- `i` - Enter input mode to type commands
- `o` - Accept pending session invitation  
- `Tab` - View pending signing requests
- `s` - Save log to file
- `q` - Quit application
- `↑/↓` - Scroll log

### Session Management
```bash
# Propose session (auto-detects DKG or signing)
/propose <wallet-name> <total> <threshold> <device1,device2,...>

# List all wallets
/wallets

# Accept session invitation (when you receive one)
Press 'o' key or type /accept <session-id>
```

### Wallet Management (Future)
```bash
# Export wallet for backup
/wallet export <wallet-name>

# Import wallet from backup
/wallet import <file-path>
```

## Examples

### Example 1: First Time Setup (DKG)
```
Alice: /propose team-wallet 3 2 alice,bob,charlie
System: No wallet 'team-wallet' found.
        Starting DKG to create new 2-of-3 wallet...

[All participants accept and complete DKG]

System: ✅ Successfully created wallet with session name 'team-wallet'!
        🔐 Wallet file: ed25519/team-wallet.dat
        🔑 Password is set to your device ID
```

### Example 2: Regular Signing
```
Bob: /propose team-wallet 3 2 alice,bob,charlie
System: Found wallet 'team-wallet' (2/3, ed25519)
        Starting signing session...

[All participants join]

Bob: /sign <transaction-hex>
[Threshold signatures collected]
System: ✅ Transaction signed successfully!
```

### Example 3: Adding New Device (Future)
```
New Device: /propose team-wallet 3 2 alice,bob,charlie,dave
System: ⚠️ Wallet 'team-wallet' not found in local keystore

[Request wallet share - to be implemented]
```

## Troubleshooting

### "Wallet not found" on signing session
- Ensure you have the wallet by running `/wallets`
- Check that the wallet name matches exactly (case-sensitive)
- If missing, wait for wallet sharing feature or import from backup

### "Parameter mismatch" error
- Check the correct parameters with `/wallets`
- Ensure you're using the right number of participants and threshold

### Session not starting
- Verify all device names are correct
- Check that all devices are online (`/list`)
- Ensure no typos in the wallet name