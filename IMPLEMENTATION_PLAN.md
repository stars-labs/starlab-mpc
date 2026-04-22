# Phase A — DKG → Usable Wallet (Persist, Display, Navigate)

**Purpose**: After `Message::DKGKeyGenerated` fires, turn the in-memory `KeyPackage` into a persisted wallet the user can see on every restart, with all chain addresses visible on a dedicated completion screen. No signing yet — that's Phase C.

**Non-goals this phase**:
- Running ed25519 + secp256k1 concurrently (follow-up #2 stays deferred)
- FROST signing round1/round2 (Phase C)
- Recovering from partial-failure DKG (use the existing `DkgState::Failed` surface; don't re-architect)

**Remove this file** once all 5 stages are `Complete`.

---

## Stage 1: Collect a wallet password from the user

**Goal**: Both the creator path (`CreateWallet → ThresholdConfig → …`) and the joiner path (`JoinSession → accept`) land on a `PasswordPrompt` screen that captures a password before DKG starts. Password lives in `Model.wallet_state.pending_password: Option<String>` — cleared after encryption.

**Rationale for password-per-device**: `keystore::create_wallet_multi_chain` encrypts each node's share locally with AES-256-GCM (PBKDF2). Each node needs its own password; they do NOT need to match across nodes (the group public key is cleartext, only the share is secret).

**Success Criteria**:
- New `Screen::PasswordPrompt { purpose: PasswordPurpose }` where `PasswordPurpose` is `CreateWallet` or `JoinWallet(session_id)`
- Navigation: creator flow goes `…Config → PasswordPrompt → DKGProgress`; joiner flow goes `AcceptSession → PasswordPrompt → DKGProgress`
- Password validation: min 8 chars, confirm field matches, non-empty — show inline error, don't advance until valid
- Escape from PasswordPrompt cancels the wallet-creation flow and returns to MainMenu (no DKG triggered, no session announced)

**Files touched** (estimate):
- `apps/tui-node/src/elm/model.rs` — add `pending_password: Option<String>` to `WalletState`; new `Screen::PasswordPrompt`; new `ComponentId::PasswordPrompt`
- `apps/tui-node/src/elm/message.rs` — `Message::SubmitPassword { value }`, `Message::PasswordValidationError`
- `apps/tui-node/src/elm/update.rs` — handler for `SubmitPassword`, transitions
- `apps/tui-node/src/elm/components/` — new `password_prompt.rs` (input + confirm + error label, reuse existing input-field pattern from `create_wallet.rs`)
- `apps/tui-node/src/elm/app.rs` — mount branch for `Screen::PasswordPrompt`

**Tests**:
- Manual e2e: start 3 nodes, create wallet with password "test1234", verify all 3 prompt for password independently and advance to DKGProgress only after submit
- Unit: `update()` with `Message::SubmitPassword { value: "short" }` returns `Message::PasswordValidationError` and does NOT transition screen
- Unit: `update()` with valid password sets `pending_password = Some(…)` and pushes `Screen::DKGProgress`

**Status**: Complete (delivered across 1.1 → 1.3-rework; see commits dd76d5e, e2c5a4e, c1ffa05, fa7632d, 5f274cb).
Notes for future reference:
- `PasswordPurpose` enum was not needed — creator vs. joiner is inferred at `Message::SubmitPassword` from `active_session` (populated on the joiner path by `AcceptSession`) vs. `creating_wallet` (populated on the creator path by `ThresholdConfig`).
- Draft input state (password/confirm/focus/error) lives on `Model.wallet_state`, not inside the component, because `app.rs::handle_key_event` bypasses tuirealm's per-component `on()` and routes keys through Messages.
- Draft is wiped on every exit (Esc / go_home / PopScreen / successful submit). The component renders bullets from lengths — no cleartext in the component.
- Error surface is inline on the screen (not a modal); any typing clears the stale error.

---

## Stage 2: Persist DKG result to keystore

**Goal**: A new `Command::FinalizeWalletFromDkg` that reads the DKG output from `AppState<C>` and writes an encrypted wallet file using the pending password. Emits `Message::DKGFinalized { wallet_id, group_pubkey_hex, addresses }` on success, `Message::DKGFailed` on error.

**Success Criteria**:
- Command reads `app_state.lock()` → pulls `key_package`, `public_key_package`, `blockchain_addresses`, `current_wallet_id`
- Serializes `key_package` via `frost-core` serialize → passes bytes to `Keystore::create_wallet_multi_chain` (exists at `keystore/storage.rs:123`)
- Wallet file written to `{keystore_path}/{device_id}/{curve_type}/{wallet_id}.json` — follows existing v2 format (PBKDF2 + AES-GCM)
- Password cleared from `model.wallet_state.pending_password` after Command dispatches (security hygiene: don't keep it sitting in memory)
- Metadata includes: `wallet_id`, `curve_type` (now the real "secp256k1" not "unified" — depends on Stage 5 but we can pass `C::curve_type()` directly here)

**Files touched**:
- `apps/tui-node/src/elm/command.rs` — new `Command::FinalizeWalletFromDkg` variant + executor
- `apps/tui-node/src/elm/message.rs` — `Message::DKGFinalized { wallet_id, group_pubkey_hex, addresses: Vec<(String, String)> }` (chain_id, address pairs)
- `apps/tui-node/src/protocal/dkg.rs` — expose `key_package().serialize()` helper if needed (likely not; AppState already stores it)
- `apps/tui-node/src/core/wallet_manager.rs` — delete the stub `save_dkg_result` (lines 229-249) or leave it as-is and ignore; Command calls `Keystore` directly

**Tests**:
- Integration: run 3-node DKG end-to-end, verify a file appears at `{keystore}/mpc-1/secp256k1/wallet-xxxxxxxx.json` on each node
- Negative: force `Keystore::create_wallet_multi_chain` to fail (e.g. pre-create the wallet file so the "already exists" branch triggers), verify `Message::DKGFailed` fires with the right error text
- Unit: mock `AppState` with populated `key_package`, confirm `Command::FinalizeWalletFromDkg` dispatches `DKGFinalized` with the expected `group_pubkey_hex`

**Status**: Complete (commits cf7022c, af45b48, 9668c02).
Notes for future reference:
- `Keystore::create_wallet_multi_chain` ignores its `_blockchains: Vec<BlockchainInfo>` parameter — addresses are re-derived on demand from `metadata.group_public_key + curve_type`, so the wallet metadata file itself only carries the group key. The `Message::DKGFinalized` carries addresses for UI convenience (Stage 3 will consume them).
- Writable `Keystore` is constructed locally in the Command rather than mutating the `Arc<Keystore>` in `AppState` — post-write, we rehydrate the shared Arc by calling `Keystore::new` again, which rescans the directory. `Arc::get_mut` doesn't work because the Model holds a second clone.
- `participant_index` must use the canonical (sorted-lexicographic) ordering of `session.participants`, matching `protocal::dkg::canonical_identifier`. The wire ordering puts each node's self-id at the end, which meant every node stored `participant_index = 3` until the canonical-sort fix.

---

## Stage 3: WalletComplete screen — show group key + all chain addresses

**Goal**: `Screen::WalletComplete { wallet_id }` actually renders. Shows the wallet name, group verifying key (copy-to-clipboard), and every blockchain address that was derived. Has a "Done" button that navigates home.

**Success Criteria**:
- `app.rs` has a `Screen::WalletComplete { ref wallet_id } => { … }` branch that mounts a new component
- New `WalletCompleteComponent` renders:
  - Wallet ID at top
  - Group verifying key in monospace, prefixed `Group PubKey:`, clipboard icon → `Copy` action
  - Addresses list, one row per chain: `[icon] Ethereum   0x1234…abcd   [Copy]`
  - Rows pulled from `model.wallet_state.wallets.iter().find(|w| w.id == wallet_id).addresses` (requires Stage 2 to have populated these)
  - "Done" button at bottom → `Message::Navigate(Screen::MainMenu)` via `model.go_home()`
- Keyboard: `Tab`/`Shift-Tab` moves focus between rows, `Enter` copies focused row, `Esc` = Done
- Works even if only secp256k1 chains succeeded (ed25519 warnings tolerable in this phase)

**Files touched**:
- `apps/tui-node/src/elm/components/wallet_complete.rs` — new component (~200 lines, pattern off `dkg_progress.rs`)
- `apps/tui-node/src/elm/components/mod.rs` — `pub mod wallet_complete; pub use wallet_complete::WalletCompleteComponent;`
- `apps/tui-node/src/elm/app.rs` — add mount branch, add `Id::WalletComplete` to the `Id` enum if it has one
- `apps/tui-node/src/elm/model.rs` — ensure `WalletMetadata` (or whatever backs the list) has the `addresses: Vec<BlockchainInfo>` field populated from Stage 2's keystore write

**Tests**:
- Manual: after DKG + finalize, all 3 nodes land on WalletComplete showing the same group key and the same address list (one per secp256k1 chain)
- Accessibility smoke: Tab cycles focus, Copy actually writes to system clipboard (via `arboard` — already a dep)

**Status**: Not Started

---

## Stage 4: Wire up the end-to-end flow

**Goal**: The `DKGKeyGenerated → FinalizeWalletFromDkg → DKGFinalized → WalletComplete` chain fires automatically with no user interaction. User sees: 100% progress → ~1.5s pause → WalletComplete with addresses → press Done → MainMenu showing new wallet.

**Success Criteria**:
- `Message::DKGKeyGenerated` handler (update.rs):
  1. Sets `dkg_round = Complete` (already done)
  2. Pushes success notification (already done)
  3. Returns `Command::Batch([SendMessage(ForceRemount), ScheduleMessage { delay_ms: 1500, Box::new(SendMessage(FinalizeWalletFromDkg)) }])`
- `Message::DKGFinalized` handler:
  1. Clears `pending_password` (belt-and-suspenders; Stage 2 also clears)
  2. Clears `creating_wallet` and `dkg_in_progress`
  3. Navigates to `Screen::WalletComplete { wallet_id }`
  4. Returns `Command::LoadWallets` so the MainMenu count updates
- `Message::DKGFailed` handler navigates back to ThresholdConfig or ManageWallets with the error modal so user can retry (existing logic, verify it handles the new failure paths)
- MainMenu's "Sign Transaction" menu item becomes visible once `model.wallet_state.wallets.len() > 0` (existing logic; Stage 2 must write the file, Stage 4's `LoadWallets` must re-read it)

**Files touched**:
- `apps/tui-node/src/elm/update.rs` — `Message::DKGKeyGenerated` + new `Message::DKGFinalized` handlers
- Whatever `Message::Navigate` path sits behind `Screen::WalletComplete`

**Tests**:
- E2E: run 3 nodes, complete DKG, verify all 3 land on WalletComplete without user action
- E2E: press Done on one node, verify MainMenu shows the new wallet with count incremented
- E2E restart: kill and restart `mpc-1`, verify wallet is still listed (Stage 2's file must be readable by `Keystore::list_wallets`)

**Status**: Not Started

---

## Stage 5: Replace `"unified"` curve label with the real curve name

**Goal**: Stop pretending the session runs both curves. Every place that currently hardcodes `"unified"` gets the actual curve from `C::curve_type()` (secp256k1 for the TUI). Removes a whole class of subtle bugs (the one that caused `Unknown curve type: unified` warnings in follow-up #1 was only the most visible).

**Success Criteria**:
- `elm/update.rs:171` `curve_type: "unified".to_string()` → `C::curve_type().to_string()` (or propagate from signals — see note)
- `elm/command.rs:118, 360, 392` — same replacements
- `SessionInfo::curve_type` field now always reflects the curve actually running at the protocol layer
- No more `warn!("Could not generate X address: Unknown curve type: unified")` lines in logs after Stage 5 ships
- Removed or deprecated: `protocal::dkg::handle_trigger_dkg_round1_dynamic` (dead code once we don't branch on "unified")

**Note on threading**: `Model::update` doesn't see `C` directly (it's plain data, no generic). Options:
  (a) Store curve label on the Model at startup (`model.curve_type: String` set by `ElmApp::new<C>` when constructing the Model)
  (b) Pass it through via a constant like `const CURVE: &str = "secp256k1"` in the main binary — ugly but honest
  
  Recommendation: (a). One-line change to model init, every update handler reads `model.curve_type` instead of hardcoding.

**Files touched**:
- `apps/tui-node/src/elm/model.rs` — add `curve_type: &'static str` field (default secp256k1)
- `apps/tui-node/src/elm/update.rs`, `apps/tui-node/src/elm/command.rs` — replace literal `"unified"` (3 sites)
- `apps/tui-node/src/protocal/dkg.rs` — delete `handle_trigger_dkg_round1_dynamic` (now unused)

**Tests**:
- Grep: `grep -rn '"unified"' apps/tui-node/src/` returns 0 results
- Log check: 3-node DKG run produces no `Unknown curve type` warnings

**Status**: Not Started

---

## Execution order

Stages 1 → 2 → 3 → 4 → 5. Stage 4 depends on 1-3 all landing. Stage 5 is independent of 1-4 technically, but better last so we don't churn the curve-type code while also adding new screens.

Ship each stage as its own commit. Each commit compiles + passes `cargo check -p tui-node` + doesn't break the existing 3-node DKG flow.

After Stage 5 is complete and merged, **delete this file**.
