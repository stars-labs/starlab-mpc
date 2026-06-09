# Final Keystore Session Design

## Summary

After analyzing different approaches, the recommended solution is:

**Use session name as wallet identifier with auto-detection**

## How It Works

### 1. Creating New Wallet (DKG)
```bash
/propose my-wallet 3 2 device-1,device-2,device-3
# If "my-wallet" doesn't exist → Start DKG
# After DKG completes → Save as "my-wallet"
```

### 2. Signing with Existing Wallet
```bash
/propose my-wallet 3 2 device-1,device-2,device-3  
# If "my-wallet" exists and parameters match → Start signing session
# If parameters don't match → Show error with correct parameters
```

### 3. Listing Wallets
```bash
/wallets
# Shows all wallets with their parameters
# Users can see exact names to use with /propose
```

## Key Benefits

1. **Simplicity**: One command for both DKG and signing
2. **Natural naming**: Session name = Wallet name
3. **No memorization**: Users see wallet names with `/wallets`
4. **Auto-detection**: System determines session type automatically
5. **Error prevention**: Clear messages when parameters don't match

## Examples

### First Time Setup
```
User: /propose company-keys 3 2 alice,bob,charlie

System: No wallet 'company-keys' found.
        Starting DKG to create new 2-of-3 wallet...
        
[DKG Process]

System: ✓ Wallet 'company-keys' created successfully!
        You can now use: /propose company-keys 3 2 <devices>
        for future signing sessions.
```

### Regular Signing
```
User: /propose company-keys 3 2 alice,bob,charlie

System: Found wallet 'company-keys' (2-of-3, ed25519)
        Starting signing session...
        Waiting for participants to load wallet...
```

### Parameter Mismatch
```
User: /propose company-keys 2 2 alice,bob

System: ❌ Cannot proceed: Parameter mismatch
        
        Wallet 'company-keys' requires:
        - 3 participants (you specified: 2)  
        - Threshold: 2 ✓
        
        Correct usage:
        /propose company-keys 3 2 alice,bob,charlie
```

### Missing Wallet on Participant
```
On Bob's device:

System: 📥 Signing session invitation: company-keys
        ⚠️ Wallet 'company-keys' not found
        
        [1] Request wallet from alice, charlie
        [2] Import from backup
        [3] Join as observer
        
        Select option: _
```

## Implementation Priority

1. **Phase 1 (MVP)**:
   - Modify `/propose` to auto-detect based on wallet existence
   - Add `/wallets` command
   - Basic error messages

2. **Phase 2 (Recovery)**:
   - Wallet share request protocol
   - Import/export functionality
   - Recovery flows

3. **Phase 3 (Polish)**:
   - Better UI for wallet selection
   - Audit logging
   - Advanced error handling

This approach provides the best balance of simplicity and functionality while maintaining security.