# Keystore Session Error Recovery Design

## Overview

This document describes the error recovery mechanisms for keystore-based sessions in the FROST MPC CLI Node. It covers scenarios where participants may be missing keystores, need to recover from failures, or transition between different session types.

## Session Types

### 1. DKG Session
- **Purpose**: Generate new distributed keys
- **Proposal**: `/propose <session_id> <total> <threshold> <device1,device2,...>`
- **Result**: Creates new keystore entries on all participants

### 2. Signing Session  
- **Purpose**: Sign transactions using existing distributed keys
- **Proposal**: `/propose <session_id> <total> <threshold> <device1,device2,...> --wallet <wallet_id>`
- **Requirement**: All participants must have the specified wallet in their keystore

## Error Scenarios and Recovery

### Scenario 1: Missing Keystore on Session Join

When a participant receives a signing session proposal but doesn't have the required wallet:

```
State: KEYSTORE_MISSING
Actions Available:
1. Request Keystore Share
2. Join as Observer
3. Decline Session
```

#### Recovery Flow:

1. **Automatic Detection**
   ```
   Session Proposal Received: test-signing
   Type: Signing (wallet: prod_2of3)
   Status: ⚠️ Wallet not found in local keystore
   
   Options:
   [1] Request wallet share from participants
   [2] Import wallet from backup
   [3] Join as observer (no signing capability)
   [4] Decline and exit
   ```

2. **Request Wallet Share**
   - Send `KEYSTORE_SHARE_REQUEST` to session participants
   - Existing participants vote to approve/deny
   - If approved, initiate secure key share transfer
   - Verify share integrity using group public key

3. **Import from Backup**
   - Display import dialog
   - Accept encrypted `.dat` file or recovery JSON
   - Validate against session's group public key
   - Add to local keystore if valid

### Scenario 2: Keystore Corruption/Loss

When a participant's keystore is corrupted or lost:

```
State: KEYSTORE_CORRUPTED
Recovery Options:
1. Restore from automated backup
2. Re-derive from other participants
3. Emergency DKG with remaining participants
```

#### Recovery Flow:

1. **Check Local Backups**
   ```
   /keystore recover
   
   Found backups:
   1. Auto-backup from 2024-01-15 (2 days ago)
   2. Manual export from 2024-01-01
   
   Select backup to restore: _
   ```

2. **Re-derivation Protocol**
   - Requires `t+1` participants to reconstruct share
   - Uses Shamir's secret sharing properties
   - Validates against group public key
   - Only available if other participants are online

3. **Emergency DKG**
   - If share cannot be recovered
   - Remaining participants perform new DKG
   - Creates new wallet with same threshold
   - Old wallet marked as deprecated

### Scenario 3: Partial Session Failure

When some participants fail during a signing session:

```
State: PARTIAL_FAILURE
Participants: 3/5 online (threshold: 3)
Can Continue: Yes
Missing: device-4, device-5
```

#### Recovery Flow:

1. **Automatic Participant Check**
   ```
   /session status
   
   Session: prod-signing
   Required: 3 signatures
   Available: device-1 ✓, device-2 ✓, device-3 ✓
   Missing: device-4 ✗, device-5 ✗
   
   Status: Can proceed with available participants
   ```

2. **Flexible Signing**
   - Proceed with any `t` participants
   - Track which devices contributed
   - Log for audit purposes

### Scenario 4: Session-Keystore Mismatch

When session parameters don't match keystore:

```
State: PARAMETER_MISMATCH
Issue: Session expects 3-of-5, keystore has 2-of-3
Resolution: Cannot proceed - wrong wallet
```

#### Recovery Flow:

1. **Validation on Join**
   ```
   Validating session parameters...
   ❌ Threshold mismatch: session=3, wallet=2
   ❌ Participant count mismatch: session=5, wallet=3
   
   This wallet cannot be used for this session.
   Options:
   [1] Propose new session with correct parameters
   [2] Select different wallet
   [3] Exit
   ```

## Backup and Export Mechanisms

### Automatic Backups

1. **Trigger Points**
   - After successful DKG completion
   - After first successful signing
   - Weekly automated backup
   - Before any keystore modification

2. **Backup Location**
   ```
   ~/.frost_keystore/
   ├── backups/
   │   ├── auto/
   │   │   ├── 2024-01-15_wallet_2of3.bak
   │   │   └── 2024-01-15_index.bak
   │   └── manual/
   │       └── export_2024-01-01.zip
   ```

### Manual Export

```
/keystore export <wallet_id> [--format json|encrypted]

Exporting wallet: prod_2of3
Include metadata? [Y/n]: Y
Encrypt with password? [Y/n]: Y
Enter password: ****
Confirm password: ****

✓ Exported to: ~/frost_exports/prod_2of3_2024-01-15.frostkey
```

### Import Process

```
/keystore import <file_path>

Importing: ~/frost_exports/prod_2of3_2024-01-15.frostkey
File type: Encrypted FROST keystore
Enter decryption password: ****

Wallet found: prod_2of3
- Curve: ed25519
- Threshold: 2 of 3
- Your identifier: 1
- Group public key: 0x1234...

Import this wallet? [Y/n]: Y
✓ Wallet imported successfully
```

## Protocol Extensions

### New Message Types

1. **SessionProposal** (extended)
   ```json
   {
     "type": "session_proposal",
     "session_id": "test-signing",
     "session_type": "signing",  // "dkg" or "signing"
     "wallet_id": "prod_2of3",    // null for DKG
     "total": 3,
     "threshold": 2,
     "participants": ["device-1", "device-2", "device-3"],
     "group_public_key": "0x1234..."  // for validation
   }
   ```

2. **ParticipantStatus**
   ```json
   {
     "type": "participant_status",
     "session_id": "test-signing",
     "device_id": "device-2",
     "status": "ready",  // "ready", "missing_keystore", "error"
     "keystore_status": {
       "has_wallet": true,
       "wallet_valid": true,
       "identifier": 2
     }
   }
   ```

3. **KeystoreShareRequest**
   ```json
   {
     "type": "keystore_share_request",
     "session_id": "test-signing",
     "requesting_device": "device-4",
     "wallet_id": "prod_2of3",
     "reason": "missing_keystore"
   }
   ```

4. **KeystoreShareResponse**
   ```json
   {
     "type": "keystore_share_response",
     "session_id": "test-signing",
     "approved": true,
     "approving_devices": ["device-1", "device-2"],
     "encrypted_share": "...",  // encrypted with requesting device's public key
     "share_proof": "..."       // proof of valid share
   }
   ```

## Implementation Priorities

1. **Phase 1: Basic Keystore Sessions**
   - Extend session proposal for signing type
   - Add wallet validation on join
   - Show clear error states

2. **Phase 2: Recovery Mechanisms**
   - Implement keystore export/import
   - Add backup automation
   - Basic share request protocol

3. **Phase 3: Advanced Recovery**
   - Share re-derivation
   - Emergency DKG
   - Audit logging

## Security Considerations

1. **Share Transfer Security**
   - Always encrypt shares in transit
   - Use device public keys for encryption
   - Require threshold approval for share requests
   - Validate shares against group public key

2. **Backup Security**
   - Encrypted backups by default
   - Use device ID as default password (user can override)
   - Store backup metadata separately
   - Regular backup rotation

3. **Access Control**
   - Share requests require majority approval
   - Time-limited share transfer windows
   - Audit log all keystore operations
   - Rate limit recovery attempts

## User Experience Guidelines

1. **Clear Status Communication**
   - Always show session type clearly
   - Indicate keystore requirements upfront
   - Provide actionable error messages

2. **Progressive Disclosure**
   - Simple happy path for normal operations
   - Advanced options available when needed
   - Guided recovery workflows

3. **Safety Checks**
   - Confirm before any destructive operations
   - Validate all imports against session parameters
   - Prevent accidental overwrites

## Testing Scenarios

1. **Happy Path**
   - All participants have keystore
   - Session completes successfully

2. **Missing Keystore**
   - One participant missing
   - Test share request flow
   - Test import flow

3. **Recovery Testing**
   - Simulate corrupted keystore
   - Test backup restoration
   - Test emergency DKG

4. **Edge Cases**
   - Network failures during share transfer
   - Partial backups
   - Version mismatches