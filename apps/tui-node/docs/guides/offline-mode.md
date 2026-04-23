# FROST MPC Offline Mode

## Overview

The offline mode enables air-gapped threshold signing where nodes operate without network connectivity. Signature shares are transferred via SD cards or other removable media, providing maximum security for high-value operations.

## Architecture

### Offline Workflow

1. **Transaction Preparation** (Online Coordinator)
   - Prepare transaction to be signed
   - Generate signing commitments request
   - Export to SD card

2. **Commitment Generation** (Offline Signers)
   - Import signing request from SD card
   - Generate signing commitments
   - Export commitments to SD card

3. **Commitment Aggregation** (Online Coordinator)
   - Collect commitments from all signers via SD cards
   - Generate signing package with aggregated commitments
   - Export signing package to SD cards

4. **Signature Generation** (Offline Signers)
   - Import signing package from SD card
   - Generate signature share
   - Export signature share to SD card

5. **Signature Aggregation** (Online Coordinator)
   - Collect signature shares from threshold number of signers
   - Aggregate into final signature
   - Broadcast transaction

### Data Formats

All data exchanged via SD card uses JSON format with the following structure:

```json
{
  "version": "1.0",
  "type": "signing_request|commitments|signing_package|signature_share",
  "session_id": "unique-session-identifier",
  "created_at": "2025-06-27T12:00:00Z",
  "expires_at": "2025-06-27T13:00:00Z",
  "data": {
    // Type-specific data
  }
}
```

#### Signing Request
```json
{
  "type": "signing_request",
  "data": {
    "wallet_id": "wallet_2of3",
    "transaction": {
      "type": "ethereum|solana",
      "payload": "base64-encoded-transaction",
      "hash": "transaction-hash-hex"
    },
    "message": "Human readable description",
    "required_signers": ["device-1", "device-2", "device-3"],
    "threshold": 2
  }
}
```

#### Commitments Response
```json
{
  "type": "commitments",
  "data": {
    "session_id": "original-session-id",
    "device_id": "device-1",
    "identifier": "frost-identifier-hex",
    "hiding_nonce_commitment": "commitment-hex",
    "binding_nonce_commitment": "commitment-hex"
  }
}
```

#### Signing Package
```json
{
  "type": "signing_package",
  "data": {
    "session_id": "original-session-id",
    "message": "message-to-sign-hex",
    "commitments": {
      "device-1": {
        "identifier": "frost-identifier-hex",
        "hiding": "commitment-hex",
        "binding": "commitment-hex"
      },
      "device-2": { ... }
    }
  }
}
```

#### Signature Share
```json
{
  "type": "signature_share",
  "data": {
    "session_id": "original-session-id",
    "device_id": "device-1",
    "identifier": "frost-identifier-hex",
    "signature_share": "share-hex"
  }
}
```

### Security Considerations

1. **Air Gap Enforcement**
   - Offline nodes must have network interfaces disabled
   - No automatic network fallback
   - Clear visual indicators of offline status

2. **Data Validation**
   - Verify session IDs match expected values
   - Check expiration timestamps
   - Validate all cryptographic materials

3. **SD Card Handling**
   - Use dedicated SD cards for transfers
   - Clear cards after use
   - Verify card integrity before reading

## Implementation

### CLI Commands

#### Offline Mode Initialization
```bash
# Start node in offline mode
mpc-wallet-tui --offline --device-id offline-signer-1

# Or toggle offline mode in running node
/offline on
/offline off
```

#### Export Commands
```bash
# Export signing request (coordinator)
/export_signing_request <session_id> <transaction_data> /mnt/sdcard/signing_request.json

# Export commitments (signer)
/export_commitments <session_id> /mnt/sdcard/commitments_device1.json

# Export signing package (coordinator)
/export_signing_package <session_id> /mnt/sdcard/signing_package.json

# Export signature share (signer)
/export_signature_share <session_id> /mnt/sdcard/signature_device1.json
```

#### Import Commands
```bash
# Import signing request (signer)
/import_signing_request /mnt/sdcard/signing_request.json

# Import commitments (coordinator)
/import_commitments /mnt/sdcard/commitments_*.json

# Import signing package (signer)
/import_signing_package /mnt/sdcard/signing_package.json

# Import signature shares (coordinator)
/import_signature_shares /mnt/sdcard/signature_*.json
```

### UI Enhancements

1. **Status Bar**
   - Show "OFFLINE MODE" prominently
   - Display pending imports/exports
   - Show SD card status

2. **Import/Export Queue**
   - List files awaiting import
   - Show export status
   - Progress indicators

3. **Session Management**
   - Track offline signing sessions
   - Show session expiration
   - Display missing components

## Usage Example

### Coordinator (Online)
```bash
# 1. Create signing request
/create_signing_request wallet_2of3 "Send 1 ETH to 0x..."
# Session ID: signing_abc123

# 2. Export to SD card
/export_signing_request signing_abc123 /mnt/sdcard/request.json

# 3. After collecting commitments from SD cards
/import_commitments /mnt/sdcard/commitments_*.json

# 4. Export signing package
/export_signing_package signing_abc123 /mnt/sdcard/package.json

# 5. After collecting signature shares
/import_signature_shares /mnt/sdcard/sig_*.json

# 6. Transaction is automatically aggregated and can be broadcast
```

### Signer (Offline)
```bash
# 1. Import signing request
/import_signing_request /mnt/sdcard/request.json

# 2. Review and approve
/review_signing_request signing_abc123

# 3. Export commitments
/export_commitments signing_abc123 /mnt/sdcard/commitments_device1.json

# 4. Later, import signing package
/import_signing_package /mnt/sdcard/package.json

# 5. Generate and export signature
/export_signature_share signing_abc123 /mnt/sdcard/sig_device1.json
```

## File Organization

SD card structure:
```
/sdcard/
├── requests/
│   └── signing_abc123_request.json
├── commitments/
│   ├── signing_abc123_device1.json
│   ├── signing_abc123_device2.json
│   └── signing_abc123_device3.json
├── packages/
│   └── signing_abc123_package.json
└── signatures/
    ├── signing_abc123_device1.json
    └── signing_abc123_device2.json
```