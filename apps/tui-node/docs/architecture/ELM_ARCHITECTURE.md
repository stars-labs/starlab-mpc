# Elm Architecture for MPC Wallet TUI

## Overview

This document describes the Elm Architecture implementation for the MPC Wallet Terminal User Interface using the `tui-realm` framework. The Elm Architecture provides a clean, functional approach to building interactive applications with predictable state management and clear separation of concerns.

## Core Concepts

### The Elm Architecture Pattern

The Elm Architecture consists of three main components:

1. **Model**: The application state
2. **Update**: A pure function that handles state transitions
3. **View**: A pure function that renders the UI based on the current state

Data flows unidirectionally:
```
User Input → Message → Update → New Model → View → UI
```

### Why Elm Architecture?

- **Predictability**: All state changes go through a single update function
- **Traceability**: Every state change can be logged and replayed
- **Testability**: Pure functions are easy to test
- **Maintainability**: Clear separation of concerns
- **Bug Prevention**: Centralized state management prevents inconsistencies

## Architecture Components

### 1. Model (`src/elm/model.rs`)

The Model represents the complete application state:

```rust
pub struct Model {
    // Core application state
    pub wallet_state: WalletState,
    pub network_state: NetworkState,
    pub ui_state: UIState,
    
    // Navigation
    pub navigation_stack: Vec<Screen>,
    pub current_screen: Screen,
    
    // Session management
    pub active_session: Option<SessionInfo>,
    pub pending_operations: Vec<Operation>,
    
    // User context
    pub selected_wallet: Option<WalletId>,
    pub device_id: String,
}

pub struct WalletState {
    pub wallets: Vec<Wallet>,
    pub keystore_initialized: bool,
    pub keystore_path: String,
}

pub struct NetworkState {
    pub connected: bool,
    pub peers: Vec<PeerId>,
    pub websocket_url: String,
    pub connection_status: ConnectionStatus,
}

pub struct UIState {
    pub focus: ComponentId,
    pub modal: Option<Modal>,
    pub notifications: Vec<Notification>,
    pub input_buffer: String,
    pub scroll_position: u16,
}
```

### 2. Messages (`src/elm/message.rs`)

Messages represent all possible events in the application. The
real `Message` enum has ~80+ variants — listing them exhaustively
here would duplicate source and drift. The sketch below shows the
shape + grouping; read `src/elm/message.rs` for the canonical list.

```rust
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    PushScreen(Screen),     // NOT `Navigate(Screen)` — earlier drafts
    PopScreen,              //  used that name; not a real variant
    GoHome,
    Initialize,

    // User input (routed from Component::on handlers)
    SelectItem { index: usize },
    SelectMode(Mode),
    SelectCurve(CurveType),
    ThresholdConfigConfirm,
    SignTypeChar(char),
    SignBackspace,
    SignSubmit,
    PasswordTypeChar(char),
    // …many more, one per per-screen component

    // Async-result / network events (emitted by Command::execute)
    WsConnected,
    WsDisconnected { reason: String },
    SessionAvailable { info: SessionInfo },
    DkgRound1Received { from: String, package: Vec<u8> },
    DkgKeyGenerated { group_key_hex: String, address: String },
    SigningComplete { signature: Vec<u8> },

    // Notifications + modals
    ShowNotification { text: String, kind: NotificationKind },
    // …etc.
}
```

Raw `KeyPressed(KeyEvent)` does not appear as a top-level variant
— tui-realm's own `Component::on(Event<UserEvent>)` translates
keystrokes into the specific `Message::<Action>` variants per
screen (see KEYBOARD_HANDLING_GUIDE.md). There is no
`Message::Quit` wired to `Ctrl+Q`; quit is a system interrupt.

### 3. Update Function (`src/elm/update.rs`)

The update function is a pure function that takes the current model and a message, returning a new model and optional commands:

```rust
// Real signature: src/elm/update.rs:33
pub fn update(model: &mut Model, msg: Message) -> Option<Command> {
    match msg {
        Message::PushScreen(screen) => {
            model.push_screen(screen.clone());
            match screen {
                Screen::ManageWallets => Some(Command::LoadWallets),
                _ => None,
            }
        }

        Message::PopScreen => {
            model.pop_screen();
            None
        }

        Message::ThresholdConfigConfirm => {
            // User pressed Enter on the threshold-config screen —
            // kick off wallet creation. The real handler builds a
            // WalletConfig from model.wallet_state and emits
            // Command::StartDKG, which in turn announces the
            // session on the signal server.
            Some(Command::StartDKG { config: /* … */ })
        }

        Message::MeshReady => {
            // WebRTC mesh finished forming; participants are all
            // connected. Fire StartFrostProtocol to run the FROST
            // part1/part2/part3 ceremony over the mesh.
            Some(Command::StartFrostProtocol)
        }

        Message::WsConnected => {
            model.network_state.connected = true;
            None
        }

        // …~80 more match arms
        _ => None,
    }
}
```

A few points that earlier drafts of this section got wrong:

- `Message::Navigate(Screen)` / `Message::NavigateBack` — not real
  variant names. Real variants are `PushScreen(Screen)` / `PopScreen`
  / `GoHome`, and `model.push_screen` / `model.pop_screen` /
  `model.go_home` are the helper methods on `Model` that mutate
  the navigation stack (defined at `src/elm/model.rs:57-76`).
- `Message::KeyPressed(KeyEvent)` with a global `Ctrl+Q → Quit`
  / `Esc → NavigateBack` match — none of that is in the real
  update function. Tui-realm dispatches keys to the active
  component's `on(event)` handler, which emits the appropriate
  typed Message variant. There is no Ctrl+Q keybinding
  (consistent with d09bddc's KEYBOARD_NAVIGATION_GUIDE rewrite).
- `Command::SendMessage(Message::…)` — not a real variant.
  Commands produce side effects (I/O, WebSocket send, keystore
  write, DKG round trigger) that eventually emit Messages back
  through a separate channel, rather than carrying a Message as
  a payload.
- `Command::StartDKG` is real, but the follow-on that actually
  runs the FROST rounds is `Command::StartFrostProtocol` (not
  `TriggerDkgRound1` as I claimed in an earlier fix to the
  tech doc's API Reference — retracting that here; the real name
  is `StartFrostProtocol`, fired once the WebRTC mesh is
  established).

### 4. Commands (`src/elm/command.rs`)

Commands represent side effects that need to be executed:

```rust
#[derive(Debug, Clone)]
pub enum Command {
    // Data loading
    LoadWallets,
    LoadSessions,
    LoadWalletDetails { id: WalletId },
    
    // Network operations
    ConnectWebSocket { url: String },
    SendNetworkMessage { to: PeerId, data: Vec<u8> },
    BroadcastMessage { data: Vec<u8> },
    
    // Cryptographic operations
    StartDKG { config: WalletConfig },
    StartSigning { request: SigningRequest },
    
    // Storage operations
    SaveWallet { wallet: Wallet },
    DeleteWallet { id: WalletId },
    ExportWallet { id: WalletId, path: PathBuf },
    ImportWallet { path: PathBuf },
    
    // UI operations
    SendMessage(Message),
    ShowNotification { text: String, kind: NotificationKind },
    RefreshUI,
    
    // System operations
    Quit,
    None,
}

impl Command {
    pub async fn execute(self, tx: Sender<Message>) -> Result<()> {
        match self {
            Command::LoadWallets => {
                let wallets = load_wallets_from_keystore().await?;
                tx.send(Message::WalletsLoaded { wallets }).await?;
            }
            Command::StartDKG { config } => {
                spawn_dkg_task(config, tx).await?;
            }
            // ... execute other commands
        }
        Ok(())
    }
}
```

## Component Architecture

### Component Hierarchy

```
Application
├── MainMenu
├── WalletManager
│   ├── WalletList
│   ├── WalletDetail
│   └── WalletActions
├── DKGWizard
│   ├── ModeSelection
│   ├── CurveSelection
│   ├── TemplateSelection
│   └── DKGProgress
├── SigningFlow
│   ├── TransactionInput
│   ├── SigningProgress
│   └── SignatureResult
└── Settings
    ├── NetworkSettings
    ├── SecuritySettings
    └── About
```

### Component Implementation

Each component implements the `Component` trait from tui-realm:

```rust
pub struct MainMenu {
    items: Vec<MenuItem>,
    selected: usize,
}

impl Component<Message, ()> for MainMenu {
    fn on(&mut self, event: Event<()>) -> Option<Message> {
        match event {
            Event::Key(KeyEvent { code: KeyCode::Up, .. }) => {
                self.selected = self.selected.saturating_sub(1);
                self.render(); // Update visual state
                None
            }
            Event::Key(KeyEvent { code: KeyCode::Down, .. }) => {
                self.selected = (self.selected + 1).min(self.items.len() - 1);
                self.render();
                None
            }
            Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => {
                // Return navigation message based on selection
                match self.selected {
                    0 => Some(Message::Navigate(Screen::CreateWallet)),
                    1 => Some(Message::Navigate(Screen::ManageWallets)),
                    2 => Some(Message::Navigate(Screen::JoinSession)),
                    3 => Some(Message::Navigate(Screen::Settings)),
                    4 => Some(Message::Quit),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl MockComponent for MainMenu {
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Render the component
        let items: Vec<ListItem> = self.items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == self.selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(item.label.clone()).style(style)
            })
            .collect();
            
        let list = List::new(items)
            .block(Block::default()
                .title("MPC Wallet")
                .borders(Borders::ALL));
                
        frame.render_widget(list, area);
    }
}
```

## Navigation System

### Navigation Stack

The navigation stack maintains history for proper back navigation:

```rust
pub struct NavigationStack {
    stack: Vec<Screen>,
    max_depth: usize,
}

impl NavigationStack {
    pub fn push(&mut self, screen: Screen) {
        if self.stack.len() >= self.max_depth {
            self.stack.remove(0); // Remove oldest
        }
        self.stack.push(screen);
    }
    
    pub fn pop(&mut self) -> Option<Screen> {
        self.stack.pop()
    }
    
    pub fn clear(&mut self) {
        self.stack.clear();
    }
    
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}
```

### Screen Transitions

Valid screen transitions are enforced through the type system:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Welcome,
    MainMenu,
    CreateWallet(CreateWalletState),
    ManageWallets,
    WalletDetail { id: WalletId },
    JoinSession,
    SessionDetail { id: SessionId },
    SignTransaction { wallet_id: WalletId },
    Settings,
}

impl Screen {
    pub fn can_navigate_to(&self, target: &Screen) -> bool {
        match (self, target) {
            // Define valid transitions
            (Screen::MainMenu, Screen::CreateWallet(_)) => true,
            (Screen::MainMenu, Screen::ManageWallets) => true,
            (Screen::ManageWallets, Screen::WalletDetail { .. }) => true,
            // ... other valid transitions
            _ => false,
        }
    }
}
```

## Event Flow

### Input Processing

1. Terminal input captured by crossterm
2. Converted to `Event` by tui-realm
3. Routed to focused component
4. Component returns `Message`
5. Message processed by `update` function
6. Model updated and command executed
7. View re-rendered with new model

### Message Processing Pipeline

```
Terminal Input
    ↓
Event Handler
    ↓
Component.on(event) → Option<Message>
    ↓
update(model, message) → (Model, Option<Command>)
    ↓
Command.execute() → Future<Message>
    ↓
update(model, message) → (Model, Option<Command>)
    ↓
View.render(model)
    ↓
Terminal Output
```

## State Management

### State Transitions

All state transitions are explicit and traceable:

```rust
pub struct StateTransition {
    pub from: Screen,
    pub to: Screen,
    pub trigger: Message,
    pub timestamp: DateTime<Utc>,
}

pub struct StateHistory {
    transitions: Vec<StateTransition>,
    max_history: usize,
}

impl StateHistory {
    pub fn record(&mut self, from: Screen, to: Screen, trigger: Message) {
        let transition = StateTransition {
            from,
            to,
            trigger,
            timestamp: Utc::now(),
        };
        
        self.transitions.push(transition);
        
        if self.transitions.len() > self.max_history {
            self.transitions.remove(0);
        }
    }
    
    pub fn recent(&self, count: usize) -> &[StateTransition] {
        let start = self.transitions.len().saturating_sub(count);
        &self.transitions[start..]
    }
}
```

### State Persistence

Critical state is persisted to enable recovery:

```rust
impl Model {
    pub fn save_state(&self) -> Result<()> {
        let state_file = self.get_state_file_path()?;
        let state_json = serde_json::to_string_pretty(&self.persistent_state())?;
        std::fs::write(state_file, state_json)?;
        Ok(())
    }
    
    pub fn load_state() -> Result<Self> {
        let state_file = Self::get_state_file_path()?;
        if state_file.exists() {
            let state_json = std::fs::read_to_string(state_file)?;
            let persistent = serde_json::from_str(&state_json)?;
            Ok(Self::from_persistent(persistent))
        } else {
            Ok(Self::default())
        }
    }
    
    fn persistent_state(&self) -> PersistentState {
        PersistentState {
            selected_wallet: self.selected_wallet.clone(),
            device_id: self.device_id.clone(),
            websocket_url: self.network_state.websocket_url.clone(),
            // ... other persistent fields
        }
    }
}
```

## Testing Strategy

### Unit Testing

Test individual components in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_navigation_back() {
        let mut model = Model::default();
        model.current_screen = Screen::MainMenu;
        
        // Navigate to wallet list
        let cmd = update(&mut model, Message::Navigate(Screen::ManageWallets));
        assert_eq!(model.current_screen, Screen::ManageWallets);
        assert_eq!(model.navigation_stack.len(), 1);
        
        // Navigate back
        let cmd = update(&mut model, Message::NavigateBack);
        assert_eq!(model.current_screen, Screen::MainMenu);
        assert_eq!(model.navigation_stack.len(), 0);
    }
    
    #[test]
    fn test_esc_key_navigation() {
        let mut model = Model::default();
        model.current_screen = Screen::ManageWallets;
        model.navigation_stack.push(Screen::MainMenu);
        
        // Press Esc
        let cmd = update(&mut model, Message::KeyPressed(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
        }));
        
        // Should return NavigateBack command
        assert!(matches!(cmd, Some(Command::SendMessage(Message::NavigateBack))));
    }
}
```

### Integration Testing

Test complete workflows:

```rust
#[tokio::test]
async fn test_wallet_creation_flow() {
    let (tx, mut rx) = mpsc::channel(100);
    let mut app = ElmApp::new(tx);
    
    // Start wallet creation
    app.update(Message::Navigate(Screen::CreateWallet));
    
    // Select mode
    app.update(Message::SelectMode(Mode::Online));
    
    // Select curve
    app.update(Message::SelectCurve(Curve::Secp256k1));
    
    // Select template
    app.update(Message::SelectTemplate(Template::TwoOfThree));
    
    // Start DKG
    app.update(Message::ConfirmWalletCreation);
    
    // Verify DKG started
    let cmd = rx.recv().await.unwrap();
    assert!(matches!(cmd, Command::StartDKG { .. }));
}
```

## Migration Strategy

### Phase 1: Parallel Implementation
- Implement Elm architecture alongside existing code
- Create adapter layer to bridge old and new systems
- Gradually migrate features

### Phase 2: Feature Migration
- Migrate one feature at a time
- Start with simple screens (main menu, settings)
- Progress to complex flows (DKG, signing)

### Phase 3: Legacy Removal
- Remove old UI code once feature is migrated
- Archive legacy documentation
- Update tests

### Phase 4: Optimization
- Profile and optimize performance
- Implement advanced features (undo/redo, time travel debugging)
- Add telemetry and analytics

## Benefits

### Immediate Benefits
1. **Fixed Navigation**: Esc key properly navigates back instead of exiting
2. **Consistent State**: All state changes go through update function
3. **Type Safety**: Messages replace string-based commands
4. **Better Testing**: Pure functions are easy to test

### Long-term Benefits
1. **Maintainability**: Clear architecture makes changes easier
2. **Debugging**: State transitions can be logged and replayed
3. **Extensibility**: New features fit naturally into the architecture
4. **Performance**: Efficient rendering with tui-realm
5. **User Experience**: Consistent and predictable interface behavior

## References

- [Elm Architecture Guide](https://guide.elm-lang.org/architecture/)
- [tui-realm Documentation](https://docs.rs/tuirealm/latest/tuirealm/)
- [Ratatui Elm Architecture](https://ratatui.rs/concepts/application-patterns/the-elm-architecture/)
- [Functional Reactive Programming](https://en.wikipedia.org/wiki/Functional_reactive_programming)