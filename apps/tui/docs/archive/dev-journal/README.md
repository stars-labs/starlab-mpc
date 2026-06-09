# Dev-journal archive

Historical development-journal entries from the TUI's early phases —
preserved for context on *why* the code looks the way it does, not as
live documentation.

| File | Era | What it is |
|---|---|---|
| `PHASE1_COMPLETE.md` / `PHASE1_PROGRESS.md` / `PHASE1_SUMMARY.md` | Phase 1 | Early security-fix work: PBKDF2 iterations, panic elimination, `errors.rs` module introduction |
| `PHASE2_PLAN.md` / `PHASE2_PROGRESS.md` / `PHASE2_COMPLETE.md` | Phase 2 | Follow-on refactor pass — testing infra, clippy cleanup |
| `MONTH2_DOCUMENTATION_SUMMARY.md` | Month 2 | Docs retrospective, referencing the UX plan below |
| `UX_IMPROVEMENTS_PLAN.md` | Week 2 | ~900-line UX/keybinding/navigation plan from the original TUI rewrite |
| `REAL_DKG_IMPLEMENTATION_PROGRESS.md` | Pre-real-FROST | Progress notes from the stubbed→real FROST cutover |
| `COMPLETE_REFACTORING_SUMMARY.md` | Cross-phase | Meta-retrospective on the whole refactor arc |
| `FIX_TODOLIST.md` | Ongoing | Panic-audit todo list; every line-numbered `unwrap()` / `expect()` call it called out has since been fixed — the line numbers no longer resolve |
| `EVENT_SYSTEM_COMPLETE.md` / `EVENT_SYSTEM_FIX.md` | tui-realm integration | Keyboard routing switch-over from manual crossterm polling to `app.tick(PollStrategy::Once)` |
| `FIX_SUMMARY.md` / `FIX_THRESHOLD_SCREEN_STUCK.md` | Bug fixes | MPC-2 NaN-in-Gauge panic; ThresholdConfig stuck-on-DKGFailed |
| `WARNINGS_FIXED.md` | Clippy cleanup | "All warnings eliminated" snapshot after a clippy-fix pass |
| `LEGACY_REMOVED.md` | KISS simplification | Removed Argon2id support, legacy `.dat` migration, `create_wallet` single-chain fn |
| `TUI_SIMULATION_COMPLETE.md` | Offline E2E | Full TUI-key-sequence simulation with real FROST + Ethereum RLP |
| `HYBRID_MODE_COMPLETE.md` | Hybrid mode | Online+offline mixed-participant MPC E2E writeup |
| `KEYSTORE_E2E_COMPLETE.md` | Keystore E2E | Full DKG→persist→load→multi-sign test writeup |
| `FROST_DKG_IMPLEMENTATION_COMPLETE.md` / `REAL_FROST_IMPLEMENTATION.md` | Mock→real FROST | Cutover retrospective — stubbed DKG replaced with `frost-{secp256k1,ed25519}` crates |
| `REAL_DKG_IMPLEMENTATION.md` | Pre-cutover snapshot | Obsoleted by the COMPLETE doc above — documented the mock state that no longer exists (dated "As of 2025-09-15") |
| `PERFORMANCE_FIXES.md` / `PERFORMANCE_OPTIMIZATIONS.md` / `performance-analysis.md` | Week 1 perf | Adaptive event loop, bounded channels, group-address determinism fix |
| `OFFLINE_DKG_IMPLEMENTATION.md` | Design-spec, never built | Claimed "We have successfully implemented" `OfflineDKGProcessComponent` (offline_dkg_process.rs) + `SDCardManagerComponent` (sd_card_manager.rs). Verified neither file exists under `src/elm/components/`. Real offline mode works via `src/offline/` + the generic UI components (no dedicated per-phase wizard). Kept as a design-spec for the UX that was intended but never shipped |
| `KEYBOARD_EVENT_ARCHITECTURE_ANALYSIS.md` | Historical analysis | Post-mortem of a past UI-hang issue blaming empty `vec![]` subscription args to `app.mount`. Proposed a `mount_with_subscriptions` helper that was never implemented (grep: zero hits); the existing code still uses `mount(..., vec![])` at 10 call sites in `elm/app.rs` and the keyboard paths work today. Either the analysis's root-cause diagnosis was wrong or the real fix landed through a different mechanism (the current stack uses tuirealm 4's CrosstermTerminalAdapter). Archived as historical artefact |
| `KEYSTORE_INITIALIZATION.md` | Historical fix writeup | Describes adding auto-keystore-init on TUI startup, referencing the fix location as `src/handlers/keystore_commands.rs`. That file and the whole `handlers/` directory don't exist today — keystore auto-init really does happen now (in `src/bin/starlab-tui.rs:118` and `src/elm/update.rs:2486`), but not in the path this doc names. Also references a `/init_keystore <path>` command for a TUI that has no command palette. Archived so the real auto-init behaviour isn't masked by stale path claims |
| `ERROR_HANDLING_GUIDE.md` | Design spec, never built | 566-line design doc built around an N/K/D/S/V/C/F/U/I error-code taxonomy (N001 WebSocket failed, K001 Keystore Locked, D001 Insufficient Participants, etc.). Zero of those codes appear anywhere in `apps/tui/src/` (grep returned no hits). Also describes `ErrorTranslator` / `UserError` / `ErrorDialog` / `RecoveryAction` types that don't exist in source, and a "biometric unlock" option for the keystore (impossible in a terminal app). Same class of fabrication as the E001-E010 tables removed in 9e9cb19 — real errors are typed per-domain `thiserror` enums: `CoreError` (`src/core/mod.rs:21`), `KeystoreError` (`src/keystore/mod.rs:24`), `FrostKeystoreError` (`src/keystore/frost_keystore.rs:19`), `OfflineError` (`src/offline/mod.rs:24`), plus upstream `FrostError` from `packages/@starlab/core`. No umbrella `src/errors.rs` file exists (earlier draft of this retraction wrongly cited that path along with a `DkgError` / `SigningError` that aren't in starlab-client source — only in upstream frost-core's SigningError variants).|

Nothing in the active source, build scripts, or top-level docs
cross-references these files — verified zero hits outside this dir.
Consult the current `architecture/` and `guides/` docs for live
documentation.
