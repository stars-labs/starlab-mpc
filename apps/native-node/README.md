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
| Wallet import/export    | ✅       | ✅          | ⚠ stub — calls back `show_message("coming soon")` |
| Threshold signing       | ✅       | ✅          | ❌ no signing flow wired |
| SD-card air-gap mode    | ✅       | ❌          | ⚠ core wired, no file-picker UI |
| Keystore persistence    | ✅       | ✅          | ⚠ inherits from TUI's `Keystore` but no UI to unlock/lock |
| EIP-1193 dApp injection | ❌       | ✅          | ❌ (desktop app — no in-browser context) |

`tui-node/src/core/` already exposes `WalletManager`,
`SessionManager`, `DkgManager`, `OfflineManager`,
`ConnectionManager`, and `CoreState`. Feature gaps above are
mostly "the callback path into the core exists but the desktop
UI surface isn't hooked up".

## Next steps (in recommended order)

1. **Wire `import_wallet` / `export_wallet` to `WalletManager`**.
   The current `core_adapter.rs` stubs them with
   `show_message("coming soon")`. Needs a file-dialog integration
   (`rfd` crate) + a password-prompt modal in the Slint UI.

2. **Add a `SigningManager` to `tui-node::core`** mirroring the
   browser extension's `webrtc.ts` signing ceremony. Expose it via
   a new `SigningInfo` type in `tui-node::core` and new
   `update_signing_*` methods on the `UICallback` trait.

3. **Extend `main_enhanced.slint`** with a signing modal
   (transaction preview, approve/reject) matching the extension's
   popup and the TUI's `SignTransactionComponent`.

4. **Delete `CoreAdapter::initialize_demo`** — it seeds fake
   wallets/sessions at startup and will mislead users.

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
│  OfflineManager, ConnectionManager, CoreState            │
│                                                           │
│  All real FROST / WebRTC / keystore logic lives here.    │
└──────────────────────────────────────────────────────────┘
```
