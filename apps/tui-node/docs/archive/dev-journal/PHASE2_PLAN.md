# Phase 2 Implementation Plan

## Critical TODOs Analysis

### 1. 🔴 CRITICAL - Signing Module TypeId Comparison
**Files**: `src/handlers/signing_commands.rs` (lines 68, 313)
- **Issue**: TypeId comparison for curve validation is commented out
- **Impact**: Cannot validate if wallet curve matches request
- **Solution**: Use trait-based approach instead of TypeId

### 2. 🔴 CRITICAL - Nonces Storage in Signing
**Files**: `src/handlers/signing_commands.rs` (lines 1013, 1115)
- **Issue**: SigningCommitments and SigningNonces are different types
- **Impact**: Cannot complete signing without proper nonce storage
- **Solution**: Generate and store SigningNonces alongside commitments

### 3. 🟡 IMPORTANT - Participant Index Calculation
**File**: `src/handlers/extension_commands.rs` (line 149)
- **Issue**: Hardcoded participant index as 1
- **Impact**: Wrong index breaks multi-party operations
- **Solution**: Calculate from session participants list

### 4. 🟡 IMPORTANT - Threshold from Session
**File**: `src/session/event_handler.rs` (line 279)
- **Issue**: Hardcoded threshold as 2
- **Impact**: Incorrect threshold validation
- **Solution**: Get from session info

### 5. 🟢 MINOR - Wallet Deletion
**File**: `src/elm/command.rs` (line 182)
- **Issue**: Not implemented
- **Impact**: Cannot delete wallets
- **Solution**: Implement keystore deletion

### 6. 🟢 MINOR - Last Used Tracking
**File**: `src/keystore/extension_compat.rs` (line 255)
- **Issue**: No last_used tracking
- **Impact**: Cannot track wallet usage
- **Solution**: Add timestamp tracking

## Test Files to Fix

### Files with `assert!(true)` fake tests:
1. `src/keystore/encryption.rs`
2. `src/keystore/storage.rs`
3. `src/keystore/extension_compat.rs`

## Implementation Order

1. **Fix Critical Signing Issues** (TypeId + Nonces)
2. **Fix Participant Index Calculation**
3. **Fix Threshold Retrieval**
4. **Implement Real Tests**
5. **Fix Minor TODOs**
6. **Remove Dead Code**

## Safety Principles

1. **No Breaking Changes**: Preserve all existing interfaces
2. **Backward Compatibility**: Maintain all current functionality
3. **Test Before Change**: Understand current behavior first
4. **Incremental Updates**: Small, verifiable changes
5. **Compile After Each Fix**: Ensure no regressions