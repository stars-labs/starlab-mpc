# MPC Wallet TUI - UX Improvements Implementation Plan

## Overview

This document outlines the comprehensive UX improvements for the MPC Wallet TUI, focusing on making the interface more intuitive, responsive, and user-friendly.

## Week 2: UX Improvements Tasks

### 1. Navigation Consistency & Keyboard Shortcuts ⏳

#### Current Issues
- Inconsistent navigation between screens
- Some shortcuts work globally, others only in specific contexts
- No visual indication of available shortcuts
- Vim-style navigation incomplete

#### Implementation Plan

##### A. Standardize Navigation Model
```rust
pub struct NavigationConfig {
    pub vim_mode: bool,
    pub wrap_around: bool,
    pub quick_keys: HashMap<char, Action>,
    pub global_shortcuts: HashMap<KeyCombo, Command>,
}
```

##### B. Create Consistent Navigation Handler
```rust
pub trait NavigationHandler {
    fn handle_navigation(&mut self, key: KeyEvent) -> Option<Message> {
        match key {
            // Universal navigation
            KeyEvent::Up | KeyEvent::Char('k') => Some(Message::NavigateUp),
            KeyEvent::Down | KeyEvent::Char('j') => Some(Message::NavigateDown),
            KeyEvent::Left | KeyEvent::Char('h') => Some(Message::NavigateLeft),
            KeyEvent::Right | KeyEvent::Char('l') => Some(Message::NavigateRight),
            
            // Universal actions
            KeyEvent::Enter => Some(Message::Select),
            KeyEvent::Esc => Some(Message::Back),
            KeyEvent::Tab => Some(Message::NextField),
            
            // Quick actions
            KeyEvent::Char('n') => Some(Message::QuickNew),
            KeyEvent::Char('j') => Some(Message::QuickJoin),
            KeyEvent::Char('w') => Some(Message::QuickWallets),
            
            _ => None
        }
    }
}
```

##### C. Visual Shortcut Hints
```
┌─ Main Menu ─────────────────────── [?] Help ─┐
│                                               │
│  > Create New Wallet            [n]           │
│    Join Session                 [j]           │
│    Manage Wallets               [w]           │
│                                               │
│ ─────────────────────────────────────────────│
│ [↑↓] Navigate  [Enter] Select  [Esc] Exit    │
└───────────────────────────────────────────────┘
```

#### Files to Modify
- `src/elm/components/base.rs` - Add NavigationHandler trait
- `src/elm/components/*.rs` - Implement consistent navigation
- `src/elm/app.rs` - Centralize shortcut handling
- `src/ui/shortcut_bar.rs` - Create visual hint component

---

### 2. Loading States & Progress Indicators ⏳

#### Current Issues
- No feedback during async operations
- Users unsure if action was registered
- No indication of operation duration
- Missing operation cancellation

#### Implementation Plan

##### A. Loading State Enum
```rust
pub enum LoadingState {
    Idle,
    Loading {
        message: String,
        progress: Option<f32>,
        started_at: Instant,
        cancelable: bool,
    },
    Success {
        message: String,
        duration: Duration,
    },
    Error {
        message: String,
        recoverable: bool,
    },
}
```

##### B. Progress Components

**Spinner Component:**
```
⠋ Connecting to network...
⠙ Loading wallets... (2.3s)
⠹ Processing transaction...
```

**Progress Bar Component:**
```
DKG Progress - Round 2 of 3
[████████████░░░░░░░░░░] 55% 
Estimated time: 12s remaining
[Cancel] Press Esc to abort
```

**Multi-Stage Progress:**
```
Creating Wallet
├─ ✓ Validation complete
├─ ⟳ Generating keys... 
├─ ○ Distributing shares
└─ ○ Finalizing

Overall: 40% complete
```

##### C. Implementation Components
```rust
pub struct ProgressManager {
    operations: HashMap<OperationId, ProgressState>,
    ui_update_tx: Sender<UIUpdate>,
}

impl ProgressManager {
    pub fn start_operation(&mut self, id: OperationId, description: &str) {
        self.operations.insert(id, ProgressState::new(description));
        self.update_ui();
    }
    
    pub fn update_progress(&mut self, id: OperationId, progress: f32) {
        if let Some(op) = self.operations.get_mut(&id) {
            op.progress = progress;
            op.estimate_completion();
            self.update_ui();
        }
    }
}
```

#### Files to Create
- `src/elm/components/spinner.rs` - Animated spinner
- `src/elm/components/progress_bar.rs` - Progress bar with ETA
- `src/elm/components/multi_progress.rs` - Multi-stage progress
- `src/elm/loading_manager.rs` - Centralized loading state

---

### 3. User-Friendly Error Messages ⏳

#### Current Issues
- Technical error messages confuse users
- No suggested recovery actions
- Errors disappear too quickly
- No error history/log

#### Implementation Plan

##### A. Error Classification
```rust
pub enum ErrorCategory {
    Network(NetworkError),
    Validation(ValidationError),
    Crypto(CryptoError),
    Storage(StorageError),
    User(UserError),
}

pub struct UserFriendlyError {
    pub title: String,
    pub description: String,
    pub technical_details: Option<String>,
    pub recovery_actions: Vec<RecoveryAction>,
    pub error_code: String,
}

pub enum RecoveryAction {
    Retry,
    Configure { setting: String },
    ContactSupport,
    CheckNetwork,
    UpdateSoftware,
}
```

##### B. Error Message Templates

**Network Error:**
```
┌─ Connection Failed ──────────────────[E001]─┐
│                                              │
│  ⚠ Unable to connect to the network         │
│                                              │
│  The WebSocket server is not responding.    │
│  This might be due to:                      │
│  • Network connectivity issues              │
│  • Firewall blocking the connection         │
│  • Server maintenance                       │
│                                              │
│  What you can do:                           │
│  [R] Retry connection                       │
│  [S] Change server settings                 │
│  [D] Show technical details                 │
│                                              │
│  [Dismiss]                     [Get Help]   │
└──────────────────────────────────────────────┘
```

**Validation Error:**
```
┌─ Invalid Configuration ─────────────────────┐
│                                              │
│  ⚠ Threshold must be less than participants │
│                                              │
│  You set:                                   │
│  • Participants: 2                          │
│  • Threshold: 3 ← This is invalid           │
│                                              │
│  The threshold is the minimum number of     │
│  participants needed to sign. It must be    │
│  less than or equal to total participants.  │
│                                              │
│  [Fix Now]                      [Learn More]│
└──────────────────────────────────────────────┘
```

##### C. Error History & Logging
```rust
pub struct ErrorLog {
    entries: VecDeque<ErrorEntry>,
    max_entries: usize,
}

pub struct ErrorEntry {
    timestamp: DateTime<Utc>,
    error: UserFriendlyError,
    context: HashMap<String, String>,
    resolved: bool,
}
```

#### Files to Create
- `src/elm/error_translator.rs` - Convert technical to user-friendly
- `src/elm/components/error_dialog.rs` - Error display component
- `src/elm/error_log.rs` - Error history management
- `src/elm/recovery_actions.rs` - Automated recovery logic

---

### 4. Contextual Help System ⏳

#### Current Issues
- No in-app help available
- Users must refer to external docs
- No context-sensitive guidance
- Missing tooltips/hints

#### Implementation Plan

##### A. Help System Architecture
```rust
pub struct HelpSystem {
    help_database: HelpDatabase,
    current_context: HelpContext,
    search_index: SearchIndex,
}

pub struct HelpContext {
    screen: Screen,
    focused_component: ComponentId,
    user_action: Option<UserAction>,
}

pub struct HelpEntry {
    id: String,
    title: String,
    content: String,
    related: Vec<String>,
    shortcuts: Vec<KeyBinding>,
    examples: Vec<Example>,
}
```

##### B. Help Overlay Design

**Quick Help (?):**
```
┌─ Quick Help ─────────────────────── [Esc] ─┐
│                                             │
│  Current Screen: Create Wallet              │
│                                             │
│  Available Actions:                         │
│  ├─ [Tab]    Next field                     │
│  ├─ [↑↓]     Adjust values                  │
│  ├─ [Enter]  Proceed to next step           │
│  └─ [Esc]    Cancel and return              │
│                                             │
│  What is a threshold?                       │
│  The minimum number of participants needed  │
│  to authorize a transaction. For example,   │
│  2-of-3 means any 2 participants can sign.  │
│                                             │
│  [F1] Full Help  [/] Search  [→] Next Tip  │
└─────────────────────────────────────────────┘
```

**Contextual Tooltips:**
```
Threshold: [2] ←→
           ▲
     ┌─────┴──────────────────────┐
     │ Min signatures required     │
     │ Must be ≤ total participants│
     │ Common: 2-of-3, 3-of-5      │
     └─────────────────────────────┘
```

##### C. Interactive Tutorial Mode
```rust
pub struct Tutorial {
    steps: Vec<TutorialStep>,
    current_step: usize,
    completion_state: HashMap<String, bool>,
}

pub struct TutorialStep {
    title: String,
    instruction: String,
    highlight_component: ComponentId,
    validation: Box<dyn Fn(&Model) -> bool>,
    hint: String,
}
```

**Tutorial Overlay:**
```
┌─ Tutorial: Creating Your First Wallet (1/5) ─┐
│                                               │
│  👋 Welcome! Let's create your first wallet. │
│                                               │
│  Step 1: Choose Online Mode                  │
│  ┌─────────────────────┐                     │
│  │ ▶ Online    ←────── │ Select this        │
│  │   Offline           │                     │
│  └─────────────────────┘                     │
│                                               │
│  Online mode allows real-time coordination   │
│  with other participants over the network.   │
│                                               │
│  [Skip Tutorial]          [Next Step →]      │
└───────────────────────────────────────────────┘
```

#### Files to Create
- `src/elm/help_system.rs` - Core help system
- `src/elm/components/help_overlay.rs` - Help display
- `src/elm/components/tooltip.rs` - Contextual tooltips
- `src/elm/tutorial.rs` - Interactive tutorial system
- `assets/help_content.toml` - Help content database

---

## Implementation Priority & Timeline

### Phase 1: Foundation (Days 1-2)
1. ✅ Create comprehensive documentation
2. ⏳ Implement NavigationHandler trait
3. ⏳ Standardize keyboard shortcuts across all components

### Phase 2: Visual Feedback (Days 3-4)
1. ⏳ Add loading spinners and progress bars
2. ⏳ Implement operation progress tracking
3. ⏳ Create visual shortcut hints bar

### Phase 3: Error Handling (Days 5-6)
1. ⏳ Build error translation system
2. ⏳ Create user-friendly error dialogs
3. ⏳ Implement recovery action system

### Phase 4: Help System (Days 7-8)
1. ⏳ Build contextual help overlay
2. ⏳ Add tooltips to all input fields
3. ⏳ Create interactive tutorial

### Phase 5: Polish & Testing (Days 9-10)
1. ⏳ User testing and feedback
2. ⏳ Performance optimization
3. ⏳ Documentation updates

## Success Metrics

### Quantitative
- Navigation consistency: 100% of screens follow same pattern
- Loading feedback: <100ms to show loading state
- Error clarity: 0 technical jargon in user-facing errors
- Help coverage: 100% of features documented in-app

### Qualitative
- Users can navigate without documentation
- Clear understanding of operation progress
- Errors provide actionable recovery steps
- Help is discoverable and contextual

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_navigation_consistency() {
    // Verify all components implement NavigationHandler
}

#[test]
fn test_error_translation() {
    // Verify technical errors become user-friendly
}

#[test]
fn test_help_context() {
    // Verify help content matches current screen
}
```

### Integration Tests
- Full user journey with navigation
- Error recovery flows
- Help system search and display
- Tutorial completion

### User Testing
- 5 new users complete wallet creation
- 5 experienced users test advanced features
- Accessibility testing with screen readers
- Performance testing on slow terminals

## Risk Mitigation

### Risk: Breaking existing workflows
**Mitigation**: Add compatibility mode for old shortcuts

### Risk: Performance impact from UI updates
**Mitigation**: Use differential rendering, lazy loading

### Risk: Help system adds complexity
**Mitigation**: Make help optional, load on-demand

### Risk: Too many visual elements
**Mitigation**: Progressive disclosure, clean default view

## Rollout Plan

1. **Alpha**: Internal testing with feature flags
2. **Beta**: Opt-in for adventurous users
3. **RC**: Default for new installations
4. **Stable**: Gradual rollout to all users

## Future Enhancements

### Version 2.1
- Customizable themes
- Macro recording
- Command palette (Ctrl+P)

### Version 2.2
- Mouse support (optional)
- Split panes
- Plugin system

### Version 3.0
- Web-based TUI (xterm.js)
- Mobile terminal support
- Voice commands

---

## Conclusion

These UX improvements will transform the MPC Wallet TUI from a functional tool into a delightful user experience. By focusing on consistency, feedback, clarity, and help, we ensure both new and experienced users can efficiently manage their wallets.

The implementation is designed to be incremental, allowing us to ship improvements continuously while maintaining stability.

---

*Document Version: 1.0*  
*Last Updated: 2025*  
*Status: In Development*