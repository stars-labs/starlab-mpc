# Keyboard Handling Guide for TUI Components

## Critical Rules to Prevent Keyboard Breakage

### 1. NEVER Use KeyModifiers::NONE
```rust
// ❌ WRONG - This breaks keyboard handling
Event::Keyboard(KeyEvent {
    code: Key::Enter,
    modifiers: KeyModifiers::NONE,  // NEVER DO THIS!
}) => { ... }

// ✅ CORRECT - Always use .. pattern
Event::Keyboard(KeyEvent {
    code: Key::Enter,
    ..  // This works with any modifiers
}) => { ... }
```

### 2. Return Correct Message Types

Each component must return the appropriate message variant from
`src/elm/message.rs`. Verified against that file:

| Component | Purpose | Correct Message |
|-----------|---------|-----------------|
| ModeSelection | Select Online/Offline mode | `Message::SelectMode(WalletMode)` |
| ThresholdConfig | Set threshold value | `Message::SetThreshold(u16)` |
| MainMenu | Select menu item | `Message::SelectItem { index: usize }` |
| JoinSession | Join a session | `Message::SelectItem { index: usize }` |
| WalletList | Pick wallet for unlock/sign | `Message::SelectItem { index: usize }` |

Earlier drafts of this table listed a `CurveSelection` component
returning `Message::SelectCurve(curve)`, and a
`ThresholdConfigConfirm` variant. Neither exists — `grep` for
either name returns zero hits. Curve selection is not a separate
screen; it's a field on the Create Wallet form, and threshold
confirmation is driven by `SetThreshold(u16)` + form submission
through the usual `SelectItem`/Enter flow.

### 3. Component ID Must Match
```rust
impl MpcWalletComponent for YourComponent {
    fn id(&self) -> Id {
        Id::YourComponentId  // Must match the Id enum
    }
}
```

### 4. Debug Logging is Essential
```rust
fn on(&mut self, event: Event<UserEvent>) -> Option<Message> {
    tracing::debug!("🎮 Component received event: {:?}", event);
    
    let result = match event {
        // ... handle events
    };
    
    if let Some(ref msg) = result {
        tracing::debug!("🎮 Component returning message: {:?}", msg);
    }
    
    result
}
```

## Common Component Patterns

### Navigation Components (Arrow Keys)
```rust
Event::Keyboard(KeyEvent { code: Key::Left, .. }) => {
    self.selected = 0;  // Update internal state
    Some(Message::ScrollUp)  // Trigger re-render
}
Event::Keyboard(KeyEvent { code: Key::Right, .. }) => {
    self.selected = 1;  // Update internal state
    Some(Message::ScrollDown)  // Trigger re-render
}
```

### List Components (Up/Down)
```rust
Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
    if self.selected > 0 {
        self.selected -= 1;
    }
    Some(Message::ScrollUp)
}
Event::Keyboard(KeyEvent { code: Key::Down, .. }) => {
    if self.selected < self.items.len() - 1 {
        self.selected += 1;
    }
    Some(Message::ScrollDown)
}
```

### Confirmation Components (Enter)
```rust
Event::Keyboard(KeyEvent { code: Key::Enter, .. }) => {
    // Return the appropriate message for your component's action
    Some(Message::YourSpecificAction)
}
```

### Universal Back Navigation (Escape)
```rust
Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
    Some(Message::NavigateBack)
}
```

## Testing Checklist

When modifying keyboard handling:

1. **Check for KeyModifiers::NONE**:
   ```bash
   grep -r "KeyModifiers::NONE" src/elm/components/
   ```

2. **Verify component IDs**:
   - Check that `fn id()` returns the correct `Id::` variant
   - Ensure the ID exists in the `Id` enum

3. **Test all keys**:
   - Arrow keys (Left/Right or Up/Down)
   - Enter key (performs action)
   - Escape key (goes back)
   - Tab key (if applicable)

4. **Check message handling**:
   - Verify the update function handles your component's messages
   - Ensure navigation flow is correct

## Debugging Tips

1. **Enable trace logging**:
   ```bash
   RUST_LOG=trace cargo run --bin starlab-tui
   ```

2. **Check component mounting**:
   - Verify component is mounted with proper subscriptions
   - Check that keyboard events are subscribed

3. **Verify event flow**:
   - Component receives event → Returns message → Update processes message → Screen changes

## Common Mistakes and Fixes

| Problem | Symptom | Fix |
|---------|---------|-----|
| KeyModifiers::NONE | Keys don't work | Use `..` pattern |
| Wrong message type | Enter doesn't proceed | Return correct Message variant |
| Wrong component ID | Component not found | Fix `fn id()` return value |
| Missing subscriptions | No events received | Subscriptions are attached at `Application::mount(...)` — search for the call site that mounts the affected component in `src/elm/app.rs` (there is no `subscription_manager.rs` file in this tree) |
| No debug logging | Hard to debug | Add tracing::debug calls |

## Prevention Script

Run this regularly to check for issues:
```bash
# Check for KeyModifiers::NONE
grep -r "KeyModifiers::NONE" src/elm/components/ && echo "❌ Found issues!" || echo "✅ Clean!"

# Check for missing debug logging
for file in src/elm/components/*.rs; do
    if grep -q "fn on(" "$file"; then
        grep -q "tracing::debug" "$file" || echo "Missing debug in $file"
    fi
done
```