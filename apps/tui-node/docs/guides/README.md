# TUI Node User Guides

User-facing walkthroughs and how-to documentation for the MPC
Wallet TUI.

## Contents

- `USER_GUIDE.md` — comprehensive walkthrough of TUI features and
  operations
- `offline-mode.md` — instructions for using the TUI in offline
  mode (SD-card air-gap exchanges)

Earlier drafts of this README also listed
`keystore_sessions_user_guide.md` — that file moved to
[`../archive/legacy-keystore-docs/`](../archive/legacy-keystore-docs/)
as part of the keystore-docs archival pass.

## Quick Start

The TUI node provides a terminal-based interface for:

1. **Creating / Joining Sessions** — participate in DKG ceremonies
2. **Managing Keystores** — import, export, manage encrypted shares
3. **Signing Messages** — EIP-191 `personal_sign` over threshold
   FROST (the TUI does not build or broadcast transactions — see
   `USER_GUIDE.md` § Signing Messages for scope)
4. **Network Operations** — monitor WebRTC / WebSocket connectivity

## Navigation

- Keyboard navigation only (see
  [`../KEYBOARD_NAVIGATION_GUIDE.md`](../KEYBOARD_NAVIGATION_GUIDE.md)
  for the real per-screen keybinding reference — rewritten in
  d09bddc after verifying each binding against source)
- Follow on-screen prompts for each operation
- `Esc` universally goes back / cancels

## Related Documentation

- [Technical architecture](../architecture/)
- [Historical UI/UX wireframes (pre-componentization)](../archive/legacy-ui/)
- [Dev-journal archive](../archive/dev-journal/) — for historical
  context on past bug-fix passes. Earlier drafts of this README
  had an inlined "Known Issues & Recent Fixes (2025-09-02)" list
  of 11 DKG / UI / keyboard fixes; those are historical and have
  been moved out of the live user guide. `git log` is the
  authoritative record for fix history.
