# MPC Wallet TUI - Error Handling & Recovery Guide

## Error Philosophy

Errors should be:
1. **Understandable** - Clear, non-technical language
2. **Actionable** - Provide recovery steps
3. **Contextual** - Relevant to current operation
4. **Traceable** - Include error codes for support

## Error Code System

### Code Format: `[Category][Number]`

| Prefix | Category | Description |
|--------|----------|-------------|
| `N` | Network | Connection, WebSocket, WebRTC issues |
| `K` | Keystore | Storage, encryption, key management |
| `D` | DKG | Distributed key generation errors |
| `S` | Signing | Transaction signing failures |
| `V` | Validation | Input validation, configuration |
| `C` | Crypto | Cryptographic operations |
| `F` | File | File system operations |
| `U` | User | User input, authentication |
| `I` | Internal | System errors, panics |

## Common Errors & Solutions

### Network Errors (N-Series)

#### N001: WebSocket Connection Failed
```
Error: Cannot connect to signaling server
User Message: "Unable to connect to the coordination server"

Causes:
- Network offline
- Firewall blocking
- Server down
- Invalid URL

Recovery:
1. Check internet connection
2. Verify firewall settings
3. Try alternative server
4. Check server status page
```

#### N002: WebRTC Connection Failed
```
Error: Failed to establish peer connection
User Message: "Cannot connect to other participants"

Causes:
- NAT/firewall issues
- No STUN/TURN servers
- Peer offline

Recovery:
1. Enable UPnP
2. Configure port forwarding
3. Use TURN server
4. Retry connection
```

#### N003: Connection Timeout
```
Error: Operation timed out after 30s
User Message: "Connection took too long"

Causes:
- Slow network
- Server overloaded
- Packet loss

Recovery:
1. Retry with longer timeout
2. Check network speed
3. Try different time
4. Use offline mode
```

### Keystore Errors (K-Series)

#### K001: Keystore Locked
```
Error: Keystore is encrypted and locked
User Message: "Please unlock your keystore"

Causes:
- Not authenticated
- Session expired
- Auto-locked

Recovery:
1. Enter password
2. Use biometric unlock
3. Reset if forgotten
```

#### K002: Corrupt Keystore
```
Error: Keystore data is corrupted
User Message: "Wallet data appears damaged"

Causes:
- Incomplete write
- Disk corruption
- Version mismatch

Recovery:
1. Restore from backup
2. Use recovery tool
3. Import from seed
4. Contact support
```

#### K003: Keystore Full
```
Error: Maximum wallet limit reached
User Message: "You've reached the maximum number of wallets (50)"

Causes:
- Too many wallets
- Storage limit

Recovery:
1. Delete unused wallets
2. Archive old wallets
3. Upgrade storage plan
```

### DKG Errors (D-Series)

#### D001: Insufficient Participants
```
Error: Not enough participants for DKG
User Message: "Waiting for more participants to join"

Current: 2 participants
Required: 3 participants

Recovery:
1. Wait for others
2. Invite participants
3. Reduce requirement
4. Use different session
```

#### D002: DKG Round Failed
```
Error: Round 2 verification failed
User Message: "Key generation step failed"

Causes:
- Participant dropped
- Invalid share
- Network issue

Recovery:
1. Restart round
2. Remove failed peer
3. Start new session
```

#### D003: DKG Timeout
```
Error: DKG did not complete in time
User Message: "Key generation took too long"

Elapsed: 5 minutes
Timeout: 5 minutes

Recovery:
1. Increase timeout
2. Check all participants ready
3. Use faster network
4. Try offline mode
```

### Signing Errors (S-Series)

#### S001: Insufficient Signers
```
Error: Not enough signatures collected
User Message: "Need more participants to sign"

Current: 1 signature
Required: 2 signatures

Recovery:
1. Wait for others
2. Request signatures
3. Check participant availability
```

#### S002: Invalid Signature
```
Error: Signature verification failed
User Message: "A signature could not be verified"

Causes:
- Wrong key
- Corrupted data
- Version mismatch

Recovery:
1. Re-request signature
2. Verify key material
3. Check compatibility
```

#### S003: Transaction Rejected
```
Error: Transaction was rejected
User Message: "Transaction was not approved"

Reason: User rejected
By: Participant "Alice"

Recovery:
1. Review transaction
2. Address concerns
3. Create new request
```

### Validation Errors (V-Series)

#### V001: Invalid Threshold
```
Error: Threshold > participants
User Message: "Invalid configuration"

Your settings:
- Participants: 3
- Threshold: 5 ❌

Rules:
- Threshold ≤ Participants
- Threshold ≥ 1

Recovery:
1. Adjust threshold to 3 or less
2. Add more participants
```

#### V002: Invalid Address
```
Error: Address checksum failed
User Message: "The address appears invalid"

Address: 0x123...abc
Issue: Invalid checksum

Recovery:
1. Copy address again
2. Verify source
3. Use address book
```

#### V003: Invalid Password
```
Error: Password requirements not met
User Message: "Password is too weak"

Requirements:
✓ At least 8 characters
✗ Include uppercase
✗ Include number
✓ Include special character

Recovery:
1. Add uppercase letter
2. Add number
3. Use password generator
```

## Error Handling Implementation

### Error Translation Layer

```rust
pub trait ErrorTranslator {
    fn to_user_friendly(&self) -> UserError {
        match self {
            // Network errors
            NetworkError::ConnectionFailed(e) => UserError {
                code: "N001",
                title: "Connection Failed",
                message: "Unable to connect to the network",
                details: Some(e.to_string()),
                actions: vec![
                    Action::Retry,
                    Action::CheckNetwork,
                    Action::ChangeServer,
                ],
            },
            
            // Keystore errors
            KeystoreError::Locked => UserError {
                code: "K001",
                title: "Keystore Locked",
                message: "Please enter your password",
                details: None,
                actions: vec![
                    Action::Unlock,
                    Action::Reset,
                ],
            },
            
            // ... more translations
        }
    }
}
```

### Error Display Component

```rust
pub struct ErrorDialog {
    error: UserError,
    selected_action: usize,
    show_details: bool,
}

impl ErrorDialog {
    pub fn render(&self) -> Frame {
        // Render user-friendly error dialog
        // Show title, message, actions
        // Hide technical details by default
    }
    
    pub fn handle_input(&mut self, key: KeyEvent) {
        match key {
            KeyEvent::Up => self.previous_action(),
            KeyEvent::Down => self.next_action(),
            KeyEvent::Enter => self.execute_action(),
            KeyEvent::Char('d') => self.toggle_details(),
            KeyEvent::Esc => self.dismiss(),
        }
    }
}
```

### Error Recovery Actions

```rust
pub enum RecoveryAction {
    Retry {
        max_attempts: u32,
        backoff: Duration,
    },
    
    Reconnect {
        server: Option<String>,
    },
    
    Unlock {
        prompt: String,
    },
    
    ImportBackup {
        default_path: PathBuf,
    },
    
    ContactSupport {
        error_report: ErrorReport,
    },
    
    UseOfflineMode,
    
    ShowDocumentation {
        topic: String,
    },
}
```

## Error Logging

### Log Levels

| Level | Usage | Example |
|-------|-------|---------|
| `ERROR` | Unrecoverable errors | Panic, critical failure |
| `WARN` | Recoverable errors | Retry needed, degraded |
| `INFO` | User actions | Button clicked, navigation |
| `DEBUG` | Development info | State changes, flow |
| `TRACE` | Detailed debugging | Every function call |

### Log Format

```
[2025-01-07 10:23:45] [ERROR] [N001] WebSocket connection failed
  Context: Connecting to wss://signal.example.com
  User: alice-node
  Session: 7f3a2b1c
  Details: Connection refused (ECONNREFUSED)
  Stack: src/network/websocket.rs:142
```

### Error Reporting

```rust
pub struct ErrorReport {
    pub timestamp: DateTime<Utc>,
    pub error_code: String,
    pub user_message: String,
    pub technical_details: String,
    pub context: HashMap<String, String>,
    pub stack_trace: Option<String>,
    pub system_info: SystemInfo,
}

impl ErrorReport {
    pub fn generate_report(&self) -> String {
        // Generate user-friendly report
        // Redact sensitive information
        // Include system details
    }
    
    pub fn send_to_support(&self) -> Result<()> {
        // Optionally send to support
        // With user permission only
    }
}
```

## Testing Error Scenarios

### Unit Tests

```rust
#[test]
fn test_error_translation() {
    let error = NetworkError::ConnectionFailed("timeout".into());
    let user_error = error.to_user_friendly();
    
    assert_eq!(user_error.code, "N001");
    assert!(!user_error.message.contains("timeout")); // No technical jargon
    assert!(user_error.actions.contains(&Action::Retry));
}
```

### Integration Tests

```rust
#[test]
async fn test_error_recovery() {
    // Simulate network failure
    let mut app = create_test_app();
    app.disconnect_network();
    
    // Trigger operation requiring network
    let result = app.create_wallet().await;
    
    // Verify error handling
    assert!(matches!(result, Err(UserError { code: "N001", .. })));
    
    // Test recovery action
    app.connect_network();
    let retry_result = app.retry_last_operation().await;
    assert!(retry_result.is_ok());
}
```

### Error Injection Testing

```rust
pub struct ErrorInjector {
    pub failures: HashMap<String, ErrorConfig>,
}

pub struct ErrorConfig {
    pub fail_after: u32,  // Succeed N times, then fail
    pub error_type: ErrorType,
    pub recovery: RecoveryBehavior,
}

// Use in tests
let injector = ErrorInjector::new()
    .add_failure("websocket", ErrorConfig {
        fail_after: 3,
        error_type: ErrorType::NetworkTimeout,
        recovery: RecoveryBehavior::RetryWithBackoff,
    });
```

## Best Practices

### DO:
- ✅ Provide context about what was happening
- ✅ Suggest concrete next steps
- ✅ Include error codes for support
- ✅ Log technical details separately
- ✅ Allow retry for transient errors
- ✅ Show progress during recovery

### DON'T:
- ❌ Show stack traces to users
- ❌ Use technical jargon
- ❌ Blame the user
- ❌ Hide errors silently
- ❌ Retry infinitely without backoff
- ❌ Lose user data on error

## Support Integration

### Error Code Database

Maintain a database of all error codes with:
- Full description
- Common causes
- Step-by-step solutions
- Related documentation
- Support escalation path

### Support Tools

The TUI binary does not currently expose `--debug-report`,
`--analyze-errors`, or `--inject-error` flags — the support-tooling
story is "run with `--log-level debug` and send the log file". See
`apps/tui-node/src/bin/mpc-wallet-tui.rs` for the real flag list. A
dedicated debug-report bundler is tracked as future work.

## Monitoring & Analytics

### Error Metrics

Track:
- Error frequency by code
- Recovery success rate
- Time to resolution
- User actions taken
- Support tickets created

### Dashboards

```
Error Dashboard
├─ Top Errors (Last 24h)
│  ├─ N001: 45 occurrences (78% recovered)
│  ├─ K001: 23 occurrences (100% recovered)
│  └─ D002: 12 occurrences (41% recovered)
├─ Error Trends
│  └─ Graph showing error rate over time
├─ Recovery Actions
│  ├─ Retry: 67% success
│  ├─ Reconnect: 45% success
│  └─ Support: 12 tickets
└─ User Impact
   ├─ Affected users: 89
   └─ Lost operations: 3
```

---

## Conclusion

Effective error handling transforms frustrating failures into manageable situations. By providing clear, actionable error messages with recovery options, we ensure users can resolve issues independently while maintaining confidence in the system.

Remember: Every error is an opportunity to help the user succeed.

---

*Document Version: 1.0*  
*Last Updated: 2025*  
*Error Code Registry: [Link to full registry]*