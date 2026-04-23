# Event System Fix - Proper tui-realm Integration

## Problem
The keyboard events (arrow keys and Enter) were not working on the ThresholdConfig and CurveSelection screens because the app was manually handling keyboard events instead of properly using tui-realm's event routing system.

## Root Cause
1. The app was directly polling crossterm events and trying to handle them manually
2. Events were not being forwarded to the active components through tui-realm's event system
3. Components had proper `Component::on()` implementations but weren't receiving events

## Solution Implemented

### 1. Proper Event Listener Configuration
- Configured `EventListenerCfg` with crossterm input listener in app initialization
- This allows tui-realm to automatically poll for keyboard events

### 2. Event Routing Through tui-realm
- Replaced manual event polling with `app.tick(PollStrategy::Once)`
- This method:
  - Polls for crossterm events automatically
  - Routes events to the active component
  - Collects messages returned by components

### 3. Component Event Handling
- Components already had proper `Component<Message, UserEvent>` implementations
- Their `on()` methods handle keyboard events and return appropriate Messages
- Messages are processed through the Elm update function

### 4. Global Shortcuts
- Kept a minimal `process_global_shortcuts()` for truly global keys (Ctrl+Q, Ctrl+R, Ctrl+H)
- Component-specific keys are handled by the components themselves

## Code Changes

### app.rs - Main Event Loop
```rust
// Before: Manual event polling
if crossterm::event::poll(Duration::from_millis(10))? {
    match crossterm::event::read() {
        Ok(event) => self.handle_terminal_event(event).await?,
        // ...
    }
}

// After: tui-realm event system
match self.app.tick(PollStrategy::Once) {
    Ok(messages) => {
        for msg in messages {
            self.process_message(msg).await;
        }
    }
    // ...
}
```

### Component Event Handling (unchanged, already correct)
```rust
impl Component<Message, UserEvent> for ThresholdConfigComponent {
    fn on(&mut self, event: Event<UserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                // Handle up arrow
                Some(Message::ThresholdConfigUp)
            }
            // ... other keys
        }
    }
}
```

## Benefits
1. **Proper Architecture**: Uses tui-realm as intended
2. **Cleaner Code**: Removed workaround code and screen-specific handling
3. **Maintainability**: Each component handles its own events
4. **Consistency**: All components work the same way
5. **KISS Principle**: Simpler, more maintainable solution

## Testing
Run the TUI and verify:
1. Arrow keys work on all screens
2. Enter key selects items
3. Esc key navigates back
4. Threshold Config: Up/Down adjust values, Left/Right switch fields
5. Curve Selection: Left/Right switch between curves

## Files Modified
- `apps/tui-node/src/elm/app.rs` - Main application event loop
- Components already had correct implementations, no changes needed