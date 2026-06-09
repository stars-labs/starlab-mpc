# Keystore Session UX Flow

## Overview

This document describes the user experience flow for keystore-based sessions, focusing on clarity and error recovery.

## 1. Session Proposal Flow

### 1.1 Proposing a DKG Session (Creating New Wallet)

```
User Input:
> /propose dkg-session-1 3 2 device-1,device-2,device-3

System Response:
📋 Proposing DKG session: dkg-session-1
   Type: Distributed Key Generation (New Wallet)
   Participants: 3 (threshold: 2)
   Waiting for responses...

   device-1: ✓ Ready
   device-2: ✓ Ready  
   device-3: ⏳ Waiting...

Session ready! Type /start to begin DKG process.
```

### 1.2 Proposing a Signing Session (Using Existing Wallet)

```
User Input:
> /propose sign-tx-1 3 2 device-1,device-2,device-3 --wallet prod_2of3

System Response:
📋 Proposing signing session: sign-tx-1
   Type: Transaction Signing
   Wallet: prod_2of3 (ed25519)
   Participants: 3 (threshold: 2)
   
   Validating wallet compatibility...
   ✓ Wallet found: prod_2of3
   ✓ Threshold matches: 2
   ✓ Participants match: 3
   
   Waiting for participant responses...
   
   device-1: ✓ Ready (has wallet)
   device-2: ⚠️ Missing wallet
   device-3: ✓ Ready (has wallet)

⚠️ Warning: device-2 does not have the required wallet.
They will be prompted to recover or import it.
```

## 2. Session Acceptance Flow

### 2.1 Accepting with Valid Keystore

```
Notification:
╔══════════════════════════════════════════════════════╗
║ 📥 New Session Invitation                            ║
║                                                      ║
║ Session: sign-tx-1                                   ║
║ Type: Transaction Signing                            ║
║ Proposer: device-1                                   ║
║ Wallet: prod_2of3                                    ║
║                                                      ║
║ ✓ Wallet found in keystore                          ║
║ ✓ Your identifier: 2                                ║
║ ✓ Ready to participate                              ║
║                                                      ║
║ [Accept] [Decline] [View Details]                    ║
╚══════════════════════════════════════════════════════╝

User Action: Press 'o' or click [Accept]

System Response:
✓ Joined session: sign-tx-1
Loading wallet: prod_2of3...
✓ Wallet loaded successfully
Status: Waiting for other participants...
```

### 2.2 Accepting with Missing Keystore

```
Notification:
╔══════════════════════════════════════════════════════╗
║ 📥 New Session Invitation                            ║
║                                                      ║
║ Session: sign-tx-1                                   ║
║ Type: Transaction Signing                            ║
║ Proposer: device-1                                   ║
║ Wallet Required: prod_2of3                           ║
║                                                      ║
║ ⚠️ Wallet NOT found in keystore                     ║
║                                                      ║
║ To participate in signing, you need this wallet.    ║
║ Choose an option:                                    ║
║                                                      ║
║ [1] Request wallet from other participants          ║
║ [2] Import wallet from backup                       ║
║ [3] Join as observer (view-only)                    ║
║ [4] Decline invitation                              ║
╚══════════════════════════════════════════════════════╝

User Action: Press '1'

System Response:
📤 Requesting wallet share from session participants...

Approval Status:
device-1: ✓ Approved
device-3: ⏳ Pending...

⏳ Waiting for threshold approvals (1/2)...
```

## 3. Wallet Recovery Flow

### 3.1 Share Request Approval (On Other Devices)

```
Notification on device-1:
╔══════════════════════════════════════════════════════╗
║ 🔑 Wallet Share Request                              ║
║                                                      ║
║ device-2 is requesting access to wallet: prod_2of3  ║
║                                                      ║
║ Reason: Missing keystore                             ║
║ Session: sign-tx-1                                   ║
║                                                      ║
║ ⚠️ Sharing will allow device-2 to:                  ║
║ • Participate in this signing session                ║
║ • Sign future transactions with this wallet         ║
║                                                      ║
║ [Approve] [Deny] [View Device Info]                  ║
╚══════════════════════════════════════════════════════╝

User Action: Click [Approve]

System Response:
✓ Share request approved
⏳ Waiting for threshold approvals...
✓ Threshold reached (2/2)
📤 Encrypting and sending wallet share...
✓ Share sent successfully
```

### 3.2 Receiving Wallet Share

```
On device-2 (requesting device):
✓ Share request approved by threshold participants!
📥 Receiving encrypted wallet shares...

Received from:
• device-1: ✓ Valid share
• device-3: ✓ Valid share

🔐 Reconstructing wallet...
✓ Wallet reconstruction successful
✓ Validating against group public key... OK
✓ Saving to keystore...

╔══════════════════════════════════════════════════════╗
║ ✅ Wallet Successfully Imported                      ║
║                                                      ║
║ Wallet: prod_2of3                                   ║
║ Type: ed25519 (Solana)                             ║
║ Your Identifier: 2                                  ║
║ Threshold: 2 of 3                                   ║
║                                                      ║
║ You can now participate in the signing session.     ║
╚══════════════════════════════════════════════════════╝

Joining session: sign-tx-1
✓ Successfully joined as active participant
```

## 4. Import from Backup Flow

### 4.1 Import Dialog

```
User selected: [2] Import wallet from backup

╔══════════════════════════════════════════════════════╗
║ 📁 Import Wallet from Backup                         ║
║                                                      ║
║ Drag and drop your backup file here, or:            ║
║                                                      ║
║ [Browse Files] [Paste JSON]                          ║
║                                                      ║
║ Supported formats:                                   ║
║ • .dat (Encrypted FROST wallet)                     ║
║ • .json (Exported key share)                        ║
║ • .frostkey (Full backup)                           ║
╚══════════════════════════════════════════════════════╝

User Action: Selects file 'prod_2of3_backup.dat'

System Response:
📄 File: prod_2of3_backup.dat
🔐 Encrypted wallet detected

Enter decryption password: ****

🔓 Decrypting...
✓ Decryption successful

Validating wallet...
✓ Wallet ID matches required: prod_2of3
✓ Group public key matches session
✓ Valid key share for identifier: 2

Import this wallet? [Yes] [No]

User Action: [Yes]

✓ Wallet imported successfully
✓ Joined session: sign-tx-1
```

## 5. Status Display

### 5.1 Session Status with Mixed Readiness

```
╔══════════════════════════════════════════════════════╗
║ Session Status: sign-tx-1                            ║
╠══════════════════════════════════════════════════════╣
║ Type: Transaction Signing                            ║
║ Wallet: prod_2of3                                    ║
║ Threshold: 2 of 3                                    ║
║                                                      ║
║ Participants:                                        ║
║ • device-1 ✅ Ready (Proposer)                      ║
║ • device-2 🔄 Importing wallet...                   ║
║ • device-3 ✅ Ready                                 ║
║                                                      ║
║ Status: ⏳ Waiting for all participants             ║
╚══════════════════════════════════════════════════════╝
```

### 5.2 Ready to Sign

```
╔══════════════════════════════════════════════════════╗
║ Session Status: sign-tx-1                            ║
╠══════════════════════════════════════════════════════╣
║ Type: Transaction Signing                            ║
║ Wallet: prod_2of3                                    ║
║ Threshold: 2 of 3                                    ║
║                                                      ║
║ Participants:                                        ║
║ • device-1 ✅ Ready (Proposer)                      ║
║ • device-2 ✅ Ready                                 ║
║ • device-3 ✅ Ready                                 ║
║                                                      ║
║ Status: ✅ Ready to sign                            ║
║                                                      ║
║ Commands:                                            ║
║ • /sign <transaction_hex> - Initiate signing        ║
║ • /session info - View detailed information         ║
║ • /leave - Leave session                            ║
╚══════════════════════════════════════════════════════╝
```

## 6. Error States

### 6.1 Wallet Mismatch

```
╔══════════════════════════════════════════════════════╗
║ ❌ Session Incompatible                              ║
║                                                      ║
║ Cannot join session: sign-tx-1                      ║
║                                                      ║
║ Required wallet: prod_2of3 (2 of 3)                 ║
║ Your wallet: prod_2of3 (3 of 5)                     ║
║                                                      ║
║ Error: Threshold mismatch                            ║
║                                                      ║
║ This appears to be a different wallet with the      ║
║ same name. Please ensure you have the correct       ║
║ wallet for this session.                            ║
║                                                      ║
║ [Import Correct Wallet] [Cancel]                     ║
╚══════════════════════════════════════════════════════╝
```

### 6.2 Recovery Failure

```
╔══════════════════════════════════════════════════════╗
║ ❌ Wallet Recovery Failed                            ║
║                                                      ║
║ Unable to recover wallet: prod_2of3                 ║
║                                                      ║
║ Reason: Insufficient approvals (1/2)                ║
║                                                      ║
║ Approvals:                                           ║
║ • device-1: ✅ Approved                             ║
║ • device-3: ❌ Denied - "Unknown device"            ║
║                                                      ║
║ Options:                                             ║
║ [1] Request from different participants             ║
║ [2] Import from backup                              ║
║ [3] Join as observer                                ║
║ [4] Leave session                                   ║
╚══════════════════════════════════════════════════════╝
```

## 7. Command Reference

### New Commands

```bash
# Propose signing session with existing wallet
/propose <session_id> <total> <threshold> <devices> --wallet <wallet_id>

# List available wallets for signing
/wallets list

# Export wallet for backup
/wallet export <wallet_id> [--password <password>]

# Import wallet from backup  
/wallet import <file_path>

# Request wallet from session participants
/wallet request <wallet_id> --session <session_id>

# Approve/deny wallet share request
/wallet share approve <request_id>
/wallet share deny <request_id> [--reason "reason"]

# Check session compatibility
/session check <session_id>
```

## 8. Best Practices

1. **Always backup wallets** after successful DKG
2. **Use descriptive wallet IDs** that include threshold info (e.g., "prod_2of3")
3. **Verify session parameters** before accepting
4. **Keep audit logs** of all wallet sharing events
5. **Rotate wallets periodically** instead of sharing to many devices
