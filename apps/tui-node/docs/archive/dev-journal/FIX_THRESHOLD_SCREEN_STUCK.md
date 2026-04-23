# Fix: ThresholdConfig Screen Stuck Issue

## Problem
When pressing Enter on the "MPC Threshold Parameters" (ThresholdConfig) screen:
- The DKG process would start
- It would fail (expected, since WebSocket isn't connected)
- **But the screen would remain stuck on ThresholdConfig**
- Pressing Enter again would just repeat the same process endlessly

## Root Causes Found

### 1. DKGFailed Message Handler Not Navigating Away
The `DKGFailed` message handler in `update.rs` was only showing an error modal but not navigating away from the ThresholdConfig screen:

```rust
// BEFORE (Broken):
Message::DKGFailed { error } => {
    error!("DKG failed: {}", error);
    model.ui_state.modal = Some(Modal::Error {
        title: "DKG Failed".to_string(),
        message: error,
    });
    None  // ← Screen stays on ThresholdConfig!
}
```

### 2. Modal Key Handling Issue
When the error modal appeared, pressing Enter would try to trigger `SelectItem` again instead of dismissing the modal, because the key handler wasn't checking for modal presence:

```rust
// BEFORE (Broken):
fn handle_key_event(&mut self, key: KeyEvent) -> Option<Message> {
    // No check for modal - Enter always triggers SelectItem
    KeyCode::Enter => Some(Message::SelectItem { ... })
}
```

## Fixes Applied

### 1. Navigate Back to Main Menu on DKG Failure
Updated `update.rs` to clear state and return to main menu when DKG fails:

```rust
// AFTER (Fixed):
Message::DKGFailed { error } => {
    error!("DKG failed: {}", error);
    
    // Show error modal
    model.ui_state.modal = Some(Modal::Error {
        title: "DKG Failed".to_string(),
        message: error,
    });
    
    // Clear wallet creation state
    model.wallet_state.creating_wallet = None;
    
    // Navigate back to main menu ← KEY FIX!
    model.navigation_stack.clear();
    model.current_screen = Screen::MainMenu;
    model.ui_state.focus = crate::elm::model::ComponentId::MainMenu;
    model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::MainMenu).or_insert(0);
    
    None
}
```

### 2. Handle Modal Keys Properly
Updated `app.rs` to check for modal presence first and dismiss it with Enter/Esc:

```rust
// AFTER (Fixed):
fn handle_key_event(&mut self, key: KeyEvent) -> Option<Message> {
    // Check if modal is open first - modal keys take priority
    if self.model.ui_state.modal.is_some() {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                return Some(Message::CloseModal); // ← Dismiss modal
            }
            _ => return None, // Ignore other keys
        }
    }
    
    // Normal key handling continues...
}
```

## Result
Now when you press Enter on ThresholdConfig screen:
1. DKG starts and shows informative messages
2. When it fails (due to missing WebSocket), an error modal appears
3. The screen navigates back to main menu
4. Pressing Enter on the error modal dismisses it
5. You're back at the main menu, not stuck on ThresholdConfig

## Files Modified
- `/apps/tui-node/src/elm/update.rs` - Added navigation back to main menu on DKG failure
- `/apps/tui-node/src/elm/app.rs` - Added modal-aware key handling

The user can now properly navigate through the wallet creation flow without getting stuck!