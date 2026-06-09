# frost-mpc — Native Desktop Node

**Iced**-based desktop GUI (MIT-licensed) that reuses the `tui-node` library
as its business-logic backend. Intended as a third client alongside the
Terminal UI and the browser extension, with the same FROST threshold
signing primitives.

> Migrated from Slint to Iced (Slint's licensing is unsuitable for a
> proprietary product; Iced is MIT). Iced's Elm architecture
> (`State` / `Message` / `update` / `view`) maps directly onto the
> `tui-node` Elm core, and the async `UICallback` events now flow through
> an `mpsc` channel + an Iced `Subscription` instead of Slint's
> `Weak<MainWindow>` + `invoke_from_event_loop`.

## Build

```bash
cargo build -p frost-mpc-native
cargo run -p frost-mpc-native
```

The UI is plain Rust — no markup files, no `build.rs` codegen. Screens are
`view` functions returning `Element<Message>`.

## Feature-parity status (vs TUI + browser extension)

| Feature                 | TUI node | Browser ext | Native node |
|-------------------------|----------|-------------|-------------|
| WebSocket signal server | ✅       | ✅          | ✅          |
| Session create/join     | ✅       | ✅          | ✅          |
| WebRTC mesh             | ✅       | ✅          | ✅ (core reused) |
| DKG ceremony            | ✅       | ✅          | ✅ (core reused) |
| Wallet import/export    | ✅       | ✅          | ✅ `rfd` file dialog + Settings password field (for encrypted keystores) |
| Threshold signing       | ✅       | ✅          | ⚠ full approve/reject modal + SigningManager in `tui-node::core`; signature is a placeholder until FROST rounds plug into `protocal::signing` |
| SD-card air-gap mode    | ✅       | ❌          | ⚠ `rfd` folder-picker wired for export/import/clear (`frost_mpc_export` / `frost_mpc_import`, matching the TUI); emits placeholder JSON artefacts until FROST hookup lands |
| Keystore persistence    | ✅       | ✅          | ⚠ inherits from TUI's `Keystore` but no UI to unlock/lock |
| EIP-1193 dApp injection | ❌       | ✅          | ❌ (desktop app — no in-browser context) |

`tui-node/src/core/` exposes `WalletManager`, `SessionManager`,
`DkgManager`, `SigningManager`, `OfflineManager`, `ConnectionManager`,
and `CoreState`. Feature gaps above are mostly "the callback path into
the core exists but the desktop UI surface isn't fully hooked up".

## Next steps (in recommended order)

1. **Hook the SigningManager into real FROST rounds.** The Iced approve/reject
   modal and `SigningManager` skeleton exist — `SigningManager::approve`
   fast-forwards state through Commitment → Share → Aggregating → Complete
   with a placeholder all-zero signature. Plugging in
   `protocal::signing::{handle_start_signing, process_signing_round1,
   process_signing_round2}` requires either (a) extracting a
   ciphersuite-generic backend shared between the elm `Message` loop and the
   core SigningManager, or (b) bridging the existing elm-coupled functions
   via an internal channel.

2. **Wire SD-card export/import to real FROST artefacts.** The
   `rfd::FileDialog::pick_folder` hooks + `OfflineManager` export/import/clear
   API surface are wired end-to-end; the artefacts written are placeholder
   JSON — they'll carry real FROST round-1 / round-2 / signature-share bytes
   once step #1 lands. The gap is content, not plumbing.

3. **Port the two stubbed screens.** The per-participant DKG round table
   (`DkgParticipants`) and the pending-SD-operations list (`SdOperations`)
   are `// TODO(iced)` stubs that currently drop their data; the DKG progress
   bar + status line still convey state. Also `request_confirmation`
   auto-confirms — route it through a real Iced modal.

## Architecture (Iced)

```
┌──────────────────────────────────────────────────────────┐
│        view(&State) -> Element<Message>  (pure Rust)     │
│   widgets emit Message; update(&mut State, Message)      │
└────────────────────┬──────────────────────┬──────────────┘
                     │                      │ Subscription::run(ui_bridge)
            user Message              external UiEvent → Message::Ui
                     │                      │ (reads the mpsc receiver)
┌────────────────────┴──────────────────────┴──────────────┐
│   src/ui_callback.rs            src/main.rs (State)       │
│   NativeUICallback sends         owns the model; holds    │
│   UiEvent into an mpsc channel   the channel sender/recv  │
└───────────────────────────────────┬──────────────────────┘
                                    ↓
┌───────────────────────────────────────────────────────────┐
│           src/core_adapter.rs (CoreAdapter)              │
│   Thin wrapper around tui-node::core::* managers          │
└───────────────────────────────────┬───────────────────────┘
                                    ↓
┌──────────────────────────────────────────────────────────┐
│        tui-node::core (shared with the TUI binary)       │
│  WalletManager, SessionManager, DkgManager,              │
│  SigningManager, OfflineManager, ConnectionManager,      │
│  CoreState — all real FROST / WebRTC / keystore logic.   │
└──────────────────────────────────────────────────────────┘
```
