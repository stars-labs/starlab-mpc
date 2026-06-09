# TUI Node Fix Todo List

## Priority Levels
- 🔴 **CRITICAL**: Security/Stability - Fix immediately (can cause crashes or security vulnerabilities)
- 🟠 **HIGH**: Functionality - Fix soon (affects core functionality)
- 🟡 **MEDIUM**: Quality - Fix this week (code quality, performance)
- 🟢 **LOW**: Polish - Fix when possible (documentation, style)

---

## 🔴 CRITICAL - Security & Stability Issues

### 1. Fix Panic-Prone Cryptographic Code
- [ ] **File**: `src/keystore/encryption.rs`
  - [ ] Line 65: Replace `Params::new(4096, 3, 1, Some(KEY_LEN)).unwrap()` with proper error handling
  - [ ] Line 72: Fix `salt.as_bytes().try_into().unwrap()`
  - [ ] Line 144: Fix `let binding = password_hash.hash.unwrap()`
  - [ ] Line 151: Fix `salt.as_bytes().try_into().unwrap()`

- [ ] **File**: `src/protocal/dkg.rs`
  - [ ] Line 70: Replace `Identifier::<C>::try_from(my_index).expect("Invalid identifier")`
  - [ ] Line 81: Fix `Identifier::<C>::try_from(i).expect("Invalid identifier")`
  - [ ] Line 84-85: Fix multiple `expect()` calls in round1
  - [ ] Line 91: Fix `expect()` in round2

- [ ] **File**: `src/protocal/signing.rs`
  - [ ] Line 88: Fix `Identifier::try_from(participant_index).expect("Invalid identifier")`
  - [ ] Line 102: Fix `Identifier::try_from(i).expect("Invalid identifier")`
  - [ ] Line 143: Fix multiple `expect()` calls

### 2. Update Cryptographic Security Parameters
- [ ] **File**: `src/keystore/encryption.rs`
  - [ ] Line 65: Change PBKDF2 iterations from 4096 to 100,000+ (industry standard)
  - [ ] Document why specific iteration count was chosen
  - [ ] Make iterations configurable via environment variable

- [ ] **File**: `src/keystore/frost_keystore.rs`
  - [ ] Line: Align PBKDF2 iterations (currently 262144) with encryption.rs
  - [ ] Add configuration for consistent security parameters

### 3. Fix Component ID Conflicts
- [ ] **File**: `src/elm/app.rs`
  - [ ] Lines 170-200: Create unique IDs for each screen component
    - [ ] Change ModeSelection to use `Id::ModeSelection`
    - [ ] Change CurveSelection to use `Id::CurveSelection`
    - [ ] Change ThresholdConfig to use `Id::ThresholdConfig`
    - [ ] Change JoinSession to use `Id::JoinSession`
  - [ ] Update `src/elm/components/mod.rs` to add new IDs

### 4. Fix Unbounded Data Structures
- [ ] **File**: `src/elm/model.rs`
  - [ ] Add max size limit for notifications (e.g., 50)
  - [ ] Add max depth for navigation_stack (e.g., 20)
  - [ ] Implement cleanup when limits exceeded

---

## 🟠 HIGH - Core Functionality Issues

### 5. Fix TODOs in Critical Code Paths
- [ ] **File**: `src/handlers/signing_commands.rs`
  - [ ] Line 58: `TODO: Fix TypeId comparison for curve validation`
  - [ ] Line 71: `TODO: Store nonces properly`
  - [ ] Implement proper nonce storage mechanism

- [ ] **File**: `src/keystore/frost_keystore.rs`
  - [ ] Line 273: `TODO: Implement wallet deletion in keystore`
  - [ ] Implement proper wallet deletion with cleanup

- [ ] **File**: `src/handlers/dkg_commands.rs`
  - [ ] Line 127: `TODO: Implement proper rejoin logic`
  - [ ] Line 315: `TODO: Consider adding persistence for connection state`

- [ ] **File**: `src/protocal/signal.rs`
  - [ ] Line 198: `TODO: Add proper error handling`
  - [ ] Line 254: `TODO: Consider adding retry logic`

- [ ] **File**: `src/webrtc/connection_monitor.rs`
  - [ ] Line 45: `TODO: Add reconnection logic`

- [ ] **File**: `src/webrtc/rejoin_coordinator.rs`
  - [ ] Line 112: `TODO: Add metrics for rejoin success/failure`

### 6. Implement Real Tests
- [ ] **File**: `src/keystore/frost_keystore.rs`
  - [ ] Line 391-396: Replace fake test with real implementation
  - [ ] Add test for save_key_package
  - [ ] Add test for load_key_package
  - [ ] Add test for encryption/decryption

- [ ] **File**: `tests/` (new tests needed)
  - [ ] Add integration test for complete DKG flow
  - [ ] Add test for signing with error scenarios
  - [ ] Add test for keystore import/export
  - [ ] Add test for Elm message flow

### 7. Fix Architectural Violations
- [ ] **File**: `src/elm/app.rs`
  - [ ] Lines 254-264: Remove direct tokio::spawn from update
  - [ ] Implement proper command executor pattern
  - [ ] Return futures from commands instead of spawning

- [ ] **File**: `src/elm/update.rs`
  - [ ] Refactor to return Effects instead of Commands
  - [ ] Implement proper async handling

---

## 🟡 MEDIUM - Code Quality Issues

### 8. Fix All Clippy Warnings (35+)
- [ ] **uninlined_format_args** (15 occurrences)
  - [ ] Change `format!("text {}", var)` to `format!("text {var}")`
  - [ ] Files: Throughout codebase

- [ ] **needless_else** (9 occurrences)
  - [ ] Remove empty else branches
  - [ ] Files: Various

- [ ] **new_without_default** (3 occurrences)
  - [ ] Implement Default trait where appropriate
  - [ ] Files: Component structs

- [ ] **type_complexity** warnings
  - [ ] Simplify complex type signatures
  - [ ] Use type aliases where appropriate

### 9. Remove Dead Code
- [ ] **File**: `src/handlers/dkg_commands.rs`
  - [ ] Remove unused imports
  - [ ] Remove commented out code

- [ ] **File**: `src/elm/update.rs`
  - [ ] Remove unused Message variants
  - [ ] Clean up unreachable match arms

- [ ] **File**: `examples/`
  - [ ] Remove or fix broken examples
  - [ ] Update examples to use new event system

### 10. Fix Input Validation
- [ ] **File**: `src/utils/erc20_encoder.rs`
  - [ ] Line 64-68: Add checksum validation for Ethereum addresses
  - [ ] Add address format validation

- [ ] **File**: `src/utils/solana_encoder.rs`
  - [ ] Add proper Solana address validation
  - [ ] Validate transaction parameters

### 11. Fix Memory Management
- [ ] **File**: `src/elm/app.rs`
  - [ ] Line 98: Implement component reuse instead of unmount_all
  - [ ] Cache components when possible
  - [ ] Add memory profiling tests

### 12. Improve Error Handling
- [ ] Create structured error types
  - [ ] `DKGError` enum for DKG failures
  - [ ] `KeystoreError` enum for keystore operations
  - [ ] `SigningError` enum for signing failures
  
- [ ] Replace string errors with typed errors
- [ ] Add error recovery mechanisms

---

## 🟢 LOW - Documentation & Polish

### 13. Documentation Updates
- [ ] **File**: `README.md`
  - [ ] Update with new event system documentation
  - [ ] Add security considerations section
  - [ ] Document configuration parameters

- [ ] **File**: All public functions
  - [ ] Add comprehensive doc comments
  - [ ] Include examples where appropriate
  - [ ] Document error conditions

### 14. Configuration Management
- [ ] Create `config.rs` module
  - [ ] Move all hardcoded values to configuration
  - [ ] Support environment variables
  - [ ] Add validation for config values

### 15. Performance Optimization
- [ ] Profile application for bottlenecks
- [ ] Optimize component rendering
- [ ] Add benchmarks for critical paths

### 16. Dependency Cleanup
- [ ] **File**: `Cargo.toml`
  - [ ] Remove unused dependencies
  - [ ] Consolidate similar dependencies
  - [ ] Update to latest stable versions
  - [ ] Consider switching from Rust 2024 to 2021 edition

---

## Implementation Order

### Phase 1: Critical Security (Week 1)
1. Fix all panic-prone unwrap/expect (Items 1)
2. Update security parameters (Item 2)
3. Fix component ID conflicts (Item 3)
4. Fix unbounded data structures (Item 4)

### Phase 2: Core Functionality (Week 2)
5. Fix critical TODOs (Item 5)
6. Implement real tests (Item 6)
7. Fix architectural violations (Item 7)

### Phase 3: Code Quality (Week 3)
8. Fix clippy warnings (Item 8)
9. Remove dead code (Item 9)
10. Fix input validation (Item 10)
11. Fix memory management (Item 11)
12. Improve error handling (Item 12)

### Phase 4: Polish (Week 4)
13. Update documentation (Item 13)
14. Configuration management (Item 14)
15. Performance optimization (Item 15)
16. Dependency cleanup (Item 16)

---

## Testing Strategy

After each phase:
1. Run full test suite: `cargo test --all`
2. Run clippy: `cargo clippy --all-targets`
3. Check for new warnings: `cargo build --all-targets 2>&1 | grep warning`
4. Run security audit: `cargo audit`
5. Test manual workflows in TUI

---

## Success Metrics

- [ ] Zero panics in production code paths
- [ ] Zero clippy warnings
- [ ] Zero TODOs in critical code
- [ ] 80%+ test coverage for security-critical code
- [ ] All components have unique IDs
- [ ] PBKDF2 iterations ≥ 100,000
- [ ] Proper error types throughout
- [ ] Complete documentation for public APIs