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
    pub session_invites: Vec<SessionInfo>, // discovered but not
                                           // accepted yet; populated
                                           // from session_available
                                           // broadcasts

    // User context
    pub selected_wallet: Option<String>,   // Wallet ID as plain String;
                                           // no newtype WalletId exists
    pub device_id: String,

    // Application metadata
    pub app_version: String,
    pub last_saved: Option<DateTime<Utc>>,
}

pub struct WalletState {
    pub wallets: Vec<WalletMetadata>,      // NOT Vec<Wallet> — no
                                           // `Wallet` type exists
    pub keystore_initialized: bool,
    pub keystore_path: String,
    pub keystore: Option<Arc<Keystore>>,
    // …plus password/DKG/signing draft fields, see source
}

pub struct NetworkState {
    pub connected: bool,
    pub peers: Vec<String>,                // Device IDs at Elm layer
                                           // are plain strings. The
                                           // `PeerId = u16` alias in
                                           // src/webrtc/mesh_manager.rs
                                           // belongs to the mesh
                                           // TEST-HARNESS library, not
                                           // the production runtime.
    pub websocket_url: String,
    pub connection_status: ConnectionStatus,
}

pub struct UIState {
    pub focus: ComponentId,                // Real enum at model.rs:476
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
    // Navigation (real variant names verified against message.rs:15-18,263)
    NavigateBack,            // message.rs:15 — Esc goes back one screen
    NavigateHome,            // message.rs:16 — Ctrl+H goes to main menu
    PushScreen(Screen),      // message.rs:17
    PopScreen,               // message.rs:18
    Quit,                    // message.rs:263 — Ctrl+Q / Ctrl+C emit this
    Refresh,                 // Ctrl+R

    // User input (routed from Component::on handlers)
    SelectItem { index: usize },
    SelectMode(WalletMode),  // message.rs:34 — carries WalletMode, not Mode
    SetThreshold(u16),       // message.rs:37 — NOT `ThresholdConfigConfirm`
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
screen (see KEYBOARD_HANDLING_GUIDE.md).

**Real global Ctrl-key bindings** (handled in `src/elm/app.rs:851-866`
BEFORE per-component dispatch):
  - `Ctrl+Q` / `Ctrl+C` → `Message::Quit`
  - `Ctrl+R` → `Message::Refresh`
  - `Ctrl+H` → `Message::NavigateHome`

Earlier drafts of this sketch had four specific errors:

  - Listed `GoHome` as the navigate-home variant; real name is
    `NavigateHome` (`message.rs:16`).
  - Listed `SelectCurve(CurveType)` and `ThresholdConfigConfirm` as
    real variants; neither exists (`grep -n 'SelectCurve\|ThresholdConfigConfirm'
    src/elm/message.rs` returns zero matches). Curve selection is
    a field on the Create Wallet form; threshold confirmation uses
    `SetThreshold(u16)` + regular SelectItem/Enter submission.
  - Claimed "There is no `Message::Quit` wired to `Ctrl+Q`; quit
    is a system interrupt". Wrong — `Message::Quit` is real at
    `message.rs:263`, and app.rs:851 wires Ctrl+Q to it. Quit is
    NOT a bare system interrupt.
  - Claimed "`Message::NavigateBack` is not a real variant name".
    Wrong — NavigateBack is real at `message.rs:15` and is the
    variant emitted on Esc.

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

A few points that earlier drafts of this section got wrong (now
corrected inline above; notes below trace the error history):

- `Message::Navigate(Screen)` was claimed as a variant; it isn't.
  **Real navigation variants**: `NavigateBack` / `NavigateHome` /
  `PushScreen(Screen)` / `PopScreen` (message.rs:15-18). The
  matching helper methods are `model.push_screen` / `model.pop_screen`
  (`model.rs:57,63`) and `model.go_home` (`model.rs:73` — note
  the method is `go_home` while the Message variant is
  `NavigateHome`; they're NOT the same identifier).
- Tui-realm dispatches keys to the active component's
  `on(event)` handler, which emits typed Message variants. In
  ADDITION to per-component routing there ARE four global Ctrl
  keybindings handled at `app.rs:851-866` BEFORE component
  dispatch (Ctrl+Q / Ctrl+C → Quit, Ctrl+R → Refresh,
  Ctrl+H → NavigateHome). An earlier draft of THIS note claimed
  "there is no Ctrl+Q keybinding" — that was wrong; Ctrl+Q fires
  `Message::Quit` at line 851.
- `Command::SendMessage(Message::…)` — not a real variant.
  Commands produce side effects (I/O, WebSocket send, keystore
  write, DKG round trigger) that eventually emit Messages back
  through a separate channel, rather than carrying a Message as
  a payload.
- Two parallel command enums drive DKG:
    - **Elm-loop level**: `Command::StartDKG { config }` at
      `command.rs:46` (announces the session) and
      `Command::StartFrostProtocol` at `command.rs:53` (kicks
      off the FROST ceremony once the mesh is ready).
    - **Protocol level**: `InternalCommand<C>::TriggerDkgRound1`
      / `TriggerDkgRound2` / `ProcessDkgRound1` / `ProcessDkgRound2`
      at `utils/state.rs:104,113,107,116` drive the per-round
      FROST state transitions.
  Elm-loop `Command` is non-generic; `InternalCommand<C>` is
  ciphersuite-generic. Both are real and distinct — see bb333ff
  for the original cross-reference fix.

### 4. Commands (`src/elm/command.rs`)

Commands represent side effects that need to be executed:

```rust
#[derive(Debug, Clone)]
pub enum Command {
    // Data loading
    LoadWallets,                              // command.rs:17
    LoadSessions,                             // command.rs:18
    LoadWalletDetails { wallet_id: String },  // command.rs:19

    // Network operations (real variant is ReconnectWebSocket —
    // there's intentionally no ConnectWebSocket, see the comment at
    // command.rs:23)
    ReconnectWebSocket,                       // command.rs:26
    SendNetworkMessage { to: String, data: Vec<u8> },   // :28
    BroadcastMessage { data: Vec<u8> },       // command.rs:29

    // DKG / signing operations
    StartDKG { config: WalletConfig },        // command.rs:46 — session announce
    StartFrostProtocol,                       // command.rs:53 — run FROST
                                              // part1/part2/part3 once mesh ready
    StartSigning { request: SigningRequest }, // command.rs:97

    // Storage operations
    SaveWallet { wallet_data: Vec<u8> },      // command.rs:36 — encrypted blob
    DeleteWallet { wallet_id: String },       // command.rs:37
    ExportWallet { wallet_id: String, path: PathBuf }, // command.rs:38
    ImportWallet { path: PathBuf },           // command.rs:39

    // …~60 more variants
}

impl Command {
    // Note: `execute` is generic over the ciphersuite, the enum itself
    // is not. This lets the same `Command` carry through both the
    // ed25519 and secp256k1 code paths — monomorphization happens on
    // the execute call, using the ciphersuite that's already pinned on
    // `AppState<C>`.
    pub async fn execute<C: frost_core::Ciphersuite + …>(
        self,
        tx: UnboundedSender<Message>,
        app_state: &Arc<Mutex<AppState<C>>>,
    ) -> anyhow::Result<()> {
        match self {
            Command::LoadWallets => { /* read ~/.frost_keystore, send back */ }
            Command::StartDKG { config } => { /* announce + await mesh */ }
            Command::StartFrostProtocol => { /* frost-core part1/2/3 */ }
            // ... execute other commands
        }
        Ok(())
    }
}
```

Earlier drafts of this section listed `Command::SendMessage(Message)`,
`Command::Quit`, `Command::None` — none are real variants. Commands
don't carry Messages as payloads; the `Option<Command>` return
from `update` either has a side-effect Command or doesn't. Quit
is handled at the **Message level** (`Message::Quit` at
`message.rs:263`, triggered by Ctrl+Q / Ctrl+C globals in
`app.rs:851-857`), not as a Command — the Elm loop sees the
Quit message and breaks out. An earlier retraction here claimed
"quit is a system interrupt" — wrong; quit flows through the
normal Elm update cycle.

`SaveWallet` takes `wallet_data: Vec<u8>` not a `Wallet` struct
(there is no such type; encrypted shares stay as bytes until
unlock). Earlier drafts also had two duplicate `StartDKG` entries
in the Command enum sketch — removed.

## Component Architecture

### Component Hierarchy

Real per-screen components are flat — each is a separate file under
`src/elm/components/`. Earlier drafts of this section showed nested
`WalletManager { WalletList, WalletDetail, WalletActions }` /
`DKGWizard { … }` / `SigningFlow { … }` / `Settings { NetworkSettings,
SecuritySettings, About }` grouping components. None of those
parent containers exist; each screen stands alone:

```
src/elm/components/
├── main_menu.rs
├── mode_selection.rs
├── threshold_config.rs
├── create_wallet.rs
├── dkg_progress.rs
├── sign_transaction.rs
├── signature_complete.rs
├── password_prompt.rs
├── wallet_list.rs
├── wallet_detail.rs
├── wallet_complete.rs
├── join_session.rs
├── notification.rs
└── modal.rs
```

### Component Implementation

Each screen is a tui-realm `Component<Message, UserEvent>` — note
the `UserEvent` second type parameter (not `()` as earlier drafts
showed; the real type is defined in `src/elm/components/mod.rs`):

```rust
use crate::elm::components::UserEvent;

pub struct MainMenu {
    items: Vec<MenuItem>,
    selected: usize,
    focused: bool,
    wallet_count: usize,
}

impl Component<Message, UserEvent> for MainMenu {
    fn on(&mut self, event: Event<UserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            Event::Keyboard(KeyEvent { code: Key::Down, .. }) => {
                self.selected = (self.selected + 1).min(self.items.len() - 1);
                None
            }
            Event::Keyboard(KeyEvent { code: Key::Enter, .. }) => {
                // Return a SelectItem message; the update function
                // translates the index into the appropriate action
                Some(Message::SelectItem { index: self.selected })
            }
            _ => None,
        }
    }

    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Render the menu into `area`; see main_menu.rs:152 onward.
    }
    // plus `state()` / `perform()` accessors — full Component trait
}
```

Earlier drafts used `Message::Navigate(Screen::...)` and
`Message::Quit` inside the Enter match arm. Real variants are
`Message::SelectItem { index }` + per-screen SelectMode /
SelectCurve / ThresholdConfigConfirm / etc. — the update function
is where the index-to-screen mapping happens, not the component.

Also: earlier drafts used `impl MockComponent for MainMenu` with a
`render(&mut self, …)` method. `MockComponent` is not the trait
name the real components implement — they implement
`tuirealm::component::Component` directly with its `view(&mut self,
frame, area)` method.

## Navigation System

### Navigation Stack

Navigation lives directly on `Model` (`src/elm/model.rs`), not on a
separate `NavigationStack` struct:

```rust
// From src/elm/model.rs
pub struct Model {
    pub navigation_stack: Vec<Screen>,   // unbounded
    pub current_screen: Screen,
    // …
}

impl Model {
    pub fn push_screen(&mut self, screen: Screen) {
        self.navigation_stack.push(self.current_screen.clone());
        self.current_screen = screen;
    }

    pub fn pop_screen(&mut self) -> bool {
        if let Some(prev) = self.navigation_stack.pop() {
            self.current_screen = prev;
            true
        } else {
            false
        }
    }

    pub fn go_home(&mut self) {
        self.navigation_stack.clear();
        self.current_screen = Screen::MainMenu;
    }
}
```

Earlier drafts of this section showed a dedicated
`NavigationStack { stack, max_depth }` struct with `push` / `pop` /
`depth` methods. No such struct exists — verified by grep. The
stack is unbounded (no `max_depth` cap; same finding as 3f87b38
for COMPLETE_TUI_DOCUMENTATION.md).

### Screen Transitions

Valid screen transitions are enforced through the type system:

```rust
// See src/elm/model.rs for the real enum — IDs are plain Strings
// throughout, NOT newtype WalletId / SessionId wrappers (neither
// type exists in source).
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Welcome,
    MainMenu,
    CreateWallet(CreateWalletState),
    ManageWallets,
    WalletDetail { wallet_id: String },
    JoinSession,
    SessionDetail { session_id: String },
    SignTransaction { wallet_id: String },
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
Terminal input (crossterm)
    ↓
tuirealm Event<UserEvent>
    ↓
Component::on(event) → Option<Message>
    ↓
update(&mut model, message) → Option<Command>
    ↓
Command::execute(self, tx, &app_state) → async side effect
    ↓ (emits new Messages onto tx)
update(&mut model, message) → Option<Command>
    ↓
Component::view(frame, area)
    ↓
Ratatui flushes to terminal
```

Real `update` signature takes `&mut Model` and returns just
`Option<Command>` — it does NOT return a new Model. Earlier
drafts of this pipeline arrow showed `update(model, message) →
(Model, Option<Command>)`, as if the function were pure in the
strict sense. It isn't: the Model is mutated in place for
performance, while still being the single point of state
transition.

## State Management

### State Transitions

All state transitions flow through the single `update(&mut Model,
Message) -> Option<Command>` function. The sequence of transitions
is not persisted anywhere — earlier drafts of this section showed
a `StateTransition { from, to, trigger, timestamp }` struct + a
`StateHistory { transitions, max_history }` recorder. No such types
exist in source (verified via grep). If transition tracing is
needed for debugging, the standard answer is `RUST_LOG=trace` which
emits tracing events per-message through `tracing::debug!` /
`tracing::info!` calls scattered through `update.rs`.

### State Persistence

`Model` itself is not persisted. Earlier drafts of this section
showed `Model::save_state() / load_state() / persistent_state()`
methods that serialize to JSON — none exist. Persistence surface:

- **Keystore files**: one JSON file per wallet, wrapping
  plaintext metadata and the base64-encoded AES-256-GCM
  ciphertext in a `WalletFile` struct
  (`src/keystore/models.rs:438-453`). On-disk path:
  `~/.frost_keystore/<device_id>/<curve>/<wallet_id>.json` —
  single file, no `.dat` sidecar despite what earlier drafts
  claimed. Written when `Command::SaveWallet` fires (real variant
  at `src/elm/command.rs:36`); re-read via
  `Keystore::load_wallet_file(wallet_id, password)` at
  `src/keystore/storage.rs:251` — earlier drafts called the
  re-read method `load_wallet` which does not exist.
- **Tracing log file**: append-only at `--log-location` (default
  `~/.frost_keystore/logs/mpc-wallet.log`).

That's the full durable state. On crash, anything in-memory on
`Model` / `AppState<C>` that hasn't been written through the
keystore is lost (same finding as 3f87b38 / 933db62 for the
COMPLETE_TUI_DOCUMENTATION.md State Persistence section).

## Testing Strategy

### Unit Testing

Test individual components in isolation:

See `apps/tui-node/tests/update_transitions.rs` for the real
pattern — pure `update(model, message)` assertions without any
network / TTY / async machinery. Example shape, using real
variant names:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_screen_pushes_previous_onto_stack() {
        let mut model = Model::new("alice".to_string());
        model.current_screen = Screen::MainMenu;

        // Push wallet-list screen
        update(&mut model, Message::PushScreen(Screen::ManageWallets));
        assert_eq!(model.current_screen, Screen::ManageWallets);
        assert_eq!(model.navigation_stack.len(), 1);

        // Pop back
        update(&mut model, Message::PopScreen);
        assert_eq!(model.current_screen, Screen::MainMenu);
        assert_eq!(model.navigation_stack.len(), 0);
    }
}
```

Real names: `PushScreen` / `PopScreen` (NOT `Navigate` /
`NavigateBack`), `Model::new(device_id)` constructor (NOT
`Model::default()`). There is no top-level `Message::KeyPressed`
match arm in update — keyboard events flow through tui-realm
component `on()` handlers first, which translate each press into
the right typed Message variant (see KEYBOARD_HANDLING_GUIDE.md).

### Integration Testing

No `ElmApp::new(tx)` constructor exists — real constructor is
`ElmApp::new(device_id, app_state)` where `app_state` is an
`Arc<Mutex<AppState<C>>>` (see `src/elm/app.rs:58`). A full
integration test harness spanning "drive the Elm app through a
DKG ceremony" is open work (see
`docs/testing/E2E_TEST_IMPLEMENTATION_PLAN.md` — the harness it
sketches is marked "plan only, not implemented" in 9197c38).

The current coverage for integration paths lives in:

- `apps/tui-node/tests/update_transitions.rs` — pure state-machine
  transitions
- `apps/tui-node/tests/component_rendering.rs` — ratatui snapshot
  tests
- `apps/tui-node/examples/hybrid_mode_e2e_test.rs` +
  `webrtc_mesh_e2e_test.rs` — full DKG + signing over the real
  frost-core wrapping

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