# MPC Wallet — Native Desktop Node

Slint-based desktop GUI that reuses the `tui-node` library as its
business-logic backend. Intended as a third client alongside the
Terminal UI and the browser extension, with the same FROST threshold
signing primitives.

## Build

```bash
cargo build -p mpc-wallet-native
cargo run -p mpc-wallet-native
```

The Slint UI is compiled at build time from `ui/main_enhanced.slint`
via `build.rs`; changes to `.slint` files trigger a rebuild
automatically.

## Feature-parity status (vs TUI + browser extension)

| Feature                 | TUI node | Browser ext | Native node |
|-------------------------|----------|-------------|-------------|
| WebSocket signal server | ✅       | ✅          | ✅          |
| Session create/join     | ✅       | ✅          | ✅          |
| WebRTC mesh             | ✅       | ✅          | ✅ (core reused) |
| DKG ceremony            | ✅       | ✅          | ✅ (core reused) |
| Wallet import/export    | ✅       | ✅          | ✅ `rfd` file dialog + Settings password field (for encrypted keystores) |
| Threshold signing       | ✅       | ✅          | ⚠ full approve/reject modal + SigningManager in `tui-node::core`; signature is a placeholder until FROST rounds plug into `protocal::signing` |
| SD-card air-gap mode    | ✅       | ❌          | ⚠ `rfd` folder-picker wired for export/import/clear; emits placeholder JSON artefacts until FROST hookup lands |
| Keystore persistence    | ✅       | ✅          | ⚠ inherits from TUI's `Keystore` but no UI to unlock/lock |
| EIP-1193 dApp injection | ❌       | ✅          | ❌ (desktop app — no in-browser context) |

`tui-node/src/core/` exposes `WalletManager`, `SessionManager`,
`DkgManager`, `SigningManager`, `OfflineManager`,
`ConnectionManager`, and `CoreState` (seven public items total;
earlier drafts of this sentence listed six, dropping
`SigningManager` — the real `core/signing_manager.rs` has been
shipping since the native-node rehabilitation pass). Feature
gaps above are mostly "the callback path into the core exists
but the desktop UI surface isn't hooked up".

## Next steps (in recommended order)

1. **Hook the SigningManager into real FROST rounds.** The Slint
   modal, approve/reject wiring, and `SigningManager` skeleton
   all exist — `SigningManager::approve` fast-forwards state
   through Commitment → Share → Aggregating → Complete with a
   placeholder all-zero signature. Plugging in
   `protocal::signing::{handle_start_signing,
   process_signing_round1, process_signing_round2}` requires
   either (a) extracting a ciphersuite-generic backend that's
   shared between the elm `Message` loop and the core
   SigningManager, or (b) bridging the existing elm-coupled
   functions via an internal channel. See the TODO at the top
   of `src/core/signing_manager.rs`.

2. **Wire SD-card export/import to real FROST artefacts.** The
   `rfd::FileDialog::pick_folder` hooks + `OfflineManager`
   export/import/clear API surface are already wired end-to-end
   (see status-table row 6), but the artefacts written are
   placeholder JSON — they'll carry real FROST round-1 / round-2
   / signature-share bytes once step #1 above (SigningManager
   FROST hookup) lands. Earlier drafts of this step said the
   folder-picker still needed hooking up; it doesn't. The gap is
   content, not plumbing.

3. **Password-prompt UX polish.** The Settings tab has a
   persistent password field that feeds `import_wallet` /
   `export_wallet`. A proper UX would clear it after each use
   (plain-text password lingering in the field is a foot-gun)
   and optionally prompt in a modal per operation. Low priority
   since the field is `input-type: password` (masked) and not
   persisted.

## Known Slint 1.x gotchas (from the rehabilitation pass)

These caught the build for a long time; documented here so the
next migration has a cheat-sheet:

- `vertical-alignment` is only valid on `Text` — remove from
  `Rectangle` / `VerticalBox` / `HorizontalBox`.
- `linear-gradient(...)` requires the `@` prefix; stops now take
  explicit percentages (`@linear-gradient(90deg, #A 0%, #B 100%)`).
- Strings have no `.length`, `.substring`, or `.slice`. Compute
  display-shortened strings on the Rust side and push them
  through the struct property.
- `Rectangle` doesn't accept `padding` or `margin-*` — move
  padding to an inner layout element (`VerticalBox`,
  `HorizontalBox`) or use explicit `x`/`y` offsets.
- `Image.rotation-angle` was renamed to `rotation`; `animate {
  loop: true }` isn't supported — use a `Timer`-driven state
  toggle if animation is needed.
- `GridBox { Row { ... } }` doesn't accept `if`/`for` children;
  use `HorizontalBox` for conditional layouts.
- `Text` elements don't accept padding; use parent layout
  `spacing`.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│              ui/*.slint (Slint 1.x UI)                   │
│                                                           │
│  AppState globals  ←→  callbacks (create_wallet, etc.)   │
└────────────────────┬──────────────────────┬──────────────┘
                     │                      │
                Rust ↓ upgrades        Rust ↑ fires
                                       callbacks
                     │                      │
┌────────────────────┴──────────────────────┴──────────────┐
│         src/ui_callback.rs   src/main.rs                 │
│   (NativeUICallback          (wires on_* → adapter)      │
│    dispatches to event loop                              │
│    via Weak<MainWindow>)                                 │
└───────────────────────────────────┬──────────────────────┘
                                    │
                                    ↓
┌───────────────────────────────────────────────────────────┐
│           src/core_adapter.rs (CoreAdapter)              │
│                                                           │
│  Thin wrapper around tui-node::core::* managers          │
└───────────────────────────────────┬───────────────────────┘
                                    │
                                    ↓
┌──────────────────────────────────────────────────────────┐
│        tui-node::core (shared with TUI binary)           │
│                                                           │
│  WalletManager, SessionManager, DkgManager,              │
│  SigningManager, OfflineManager, ConnectionManager,      │
│  CoreState                                               │
│                                                           │
│  All real FROST / WebRTC / keystore logic lives here.    │
└──────────────────────────────────────────────────────────┘
```
