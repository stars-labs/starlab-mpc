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

Nothing in the active source, build scripts, or top-level docs
cross-references these files — verified zero hits outside this dir.
Consult the current `architecture/` and `guides/` docs for live
documentation.
