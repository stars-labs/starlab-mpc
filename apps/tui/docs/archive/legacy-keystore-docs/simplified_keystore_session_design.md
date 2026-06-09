# Simplified Keystore Session Design

## Problem
Users don't know wallet IDs and typing them is error-prone. We need a simpler approach.

## Simplified Approach

### Option 1: Single Command with Interactive Selection

```
User: /propose my-signing 3 2 device-1,device-2,device-3

System detects this could be either DKG or signing, shows:

┌─────────────────────────────────────────────┐
│ Select session type:                        │
│                                             │
│ [1] Create new wallet (DKG)                │
│ [2] Sign with existing wallet              │
│                                             │
│ Press 1 or 2:                              │
└─────────────────────────────────────────────┘

If user selects [2]:

┌─────────────────────────────────────────────┐
│ Select wallet for signing:                  │
│                                             │
│ [1] prod_wallet (2 of 3, ed25519)         │
│     Created: 2024-01-10                    │
│     Last used: Yesterday                   │
│                                             │
│ [2] test_wallet (2 of 3, ed25519)         │
│     Created: 2024-01-05                    │
│     Last used: 1 week ago                  │
│                                             │
│ [3] ethereum_keys (3 of 5, secp256k1)     │
│     Created: 2023-12-20                    │
│     Last used: Never                       │
│                                             │
│ Select wallet (1-3) or 'c' to cancel:      │
└─────────────────────────────────────────────┘
```

### Option 2: Separate Commands (Clearer Intent)

```
# For DKG (creating new wallet)
/newwallet my-session 3 2 device-1,device-2,device-3

# For signing (shows wallet selector automatically)  
/sign my-session 3 2 device-1,device-2,device-3

When user types /sign:

┌─────────────────────────────────────────────┐
│ Select wallet for signing session:          │
│                                             │
│ [1] prod_wallet (2/3) ✓ Compatible         │
│ [2] test_wallet (2/3) ✓ Compatible         │  
│ [3] ethereum_keys (3/5) ✗ Wrong threshold  │
│                                             │
│ Only showing compatible wallets.            │
│ Select (1-2):                              │
└─────────────────────────────────────────────┘
```

### Option 3: Auto-Match by Session Name (Simplest)

```
Convention: Session name matches wallet name

User: /propose prod_wallet 3 2 device-1,device-2,device-3

System checks:
- Does wallet "prod_wallet" exist? → Yes → Signing session
- Does wallet "prod_wallet" exist? → No → DKG session

Benefits:
- No extra selection needed
- Natural naming convention
- Session name = Wallet name makes sense
```

## Recommended Solution: Hybrid Approach

### 1. For New Users (No Wallets Yet)

```
User: /propose my-first-wallet 3 2 device-1,device-2,device-3

System: No wallet found with name 'my-first-wallet'
        Creating new DKG session...
        
Result: After DKG completes, wallet is saved as 'my-first-wallet'
```

### 2. For Existing Wallets (Auto-Detection)

```
User: /propose prod_wallet 3 2 device-1,device-2,device-3

System: Found existing wallet 'prod_wallet' (2 of 3)
        ✓ Threshold matches
        ✓ Participant count matches
        Creating signing session...
```

### 3. For Mismatches (Show Options)

```
User: /propose prod_wallet 3 2 device-1,device-2,device-3

System: Found wallet 'prod_wallet' but parameters don't match
        Wallet: 2 of 3, Session: 3 of 2
        
        What would you like to do?
        [1] Create new wallet 'prod_wallet_v2'
        [2] Select different wallet
        [3] Cancel
```

### 4. Quick Wallet List Command

```
User: /wallets

System: Your wallets:
        • prod_wallet (2/3, ed25519) - 3 devices
        • test_wallet (2/3, ed25519) - 3 devices  
        • ethereum_keys (3/5, secp256k1) - 5 devices
        
        Use wallet name with /propose to start signing
```

## Implementation Benefits

1. **No wallet IDs to remember** - Use meaningful names
2. **Auto-detection** - System figures out DKG vs signing
3. **Natural workflow** - Name your session = name your wallet
4. **Backwards compatible** - Old DKG flow still works
5. **Error prevention** - System validates before starting

## Updated Protocol Flow

### Session Proposal Message

```json
{
  "type": "session_proposal",
  "session_id": "prod_wallet",  // This IS the wallet name
  "total": 3,
  "threshold": 2,
  "participants": ["device-1", "device-2", "device-3"],
  "session_intent": "auto",  // "auto", "dkg", or "signing"
  "detected_wallet": {
    "exists": true,
    "compatible": true,
    "wallet_info": { ... }
  }
}
```

### Participant Response  

```json
{
  "type": "participant_response",
  "session_id": "prod_wallet",
  "device_id": "device-2",
  "wallet_status": {
    "has_wallet": true,
    "wallet_matches": true
  },
  "ready": true
}
```

## Migration Path

1. **Phase 1**: Add auto-detection to existing `/propose` command
2. **Phase 2**: Add `/wallets` command for listing
3. **Phase 3**: Deprecate manual wallet_id parameter
4. **Phase 4**: (Optional) Add `/sign` as alias for signing sessions

## Error Messages (Clear and Actionable)

### No Wallet Found
```
No wallet named 'prod_wallet' found.
→ Creating new wallet with DKG...
```

### Wrong Parameters
```
Wallet 'prod_wallet' exists but has different parameters:
  Your session: 3 participants, threshold 2
  Existing wallet: 5 participants, threshold 3
  
These must match for signing. Try:
→ /propose prod_wallet 5 3 device-1,device-2,device-3,device-4,device-5
```

### Multiple Matches (Rare)
```
Multiple wallets found for pattern 'prod':
  [1] prod_wallet (2/3)
  [2] production_keys (3/5)
  
Please use exact name or select: _
```