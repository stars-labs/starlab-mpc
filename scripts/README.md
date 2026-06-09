# Test harness for starlab-tui

## Three feedback loops, fastest first

| Loop | Command | Typical wall-clock | Scope |
|---|---|---|---|
| Compile | `cargo check -p starlab-client` | 5–10 s (warm) | Type errors, trait bounds |
| Unit / snapshot tests | `cargo test -p starlab-client --tests` | 15 s (first run after change), sub-second after | Pure `update()` state machine + component rendering |
| E2E smoke | `bash scripts/smoke-dkg.sh` | 30–60 s | Real 3-node FROST DKG over WebRTC |

Run the fastest loop that can disprove your hypothesis. Don't reach for smoke-dkg until compile + unit are green.

## `scripts/smoke-dkg.sh`

```
./scripts/smoke-dkg.sh [--manual | --tmux] [--timeout SECS]
```

- `--manual` (default) — you open 3 terminals, run `target/debug/starlab-tui --device-id mpc-{1,2,3}`, drive the UI. The script tails the logs and prints `PASS` / `FAIL` plus per-device log excerpts when all three converge on the same group verifying key.
- `--tmux` — the script spawns a 3-pane `mpc-smoke` tmux session with one TUI per pane. You still drive the UI (keystroke automation is a TODO — see source). Attach with `tmux attach -t mpc-smoke`; clean up with `tmux kill-session -t mpc-smoke`.

Exit 0 = all 3 nodes agree on the group key. Exit 1 = timeout or mismatch.

## `apps/tui/tests/update_transitions.rs`

Integration tests on the pure `elm::update::update(Model, Message)` function. No TTY, no network. Guards the DKG state-machine transitions from silent regression — `StartDKGProtocol → Round1`, first `ProcessDKGRound2 → Round2`, `DKGKeyGenerated → Complete` + `dkg_in_progress = false` + notification.

Run with:
```
cargo test -p starlab-client --test update_transitions
```

## `apps/tui/tests/component_rendering.rs`

`ratatui::backend::TestBackend` snapshot tests for `DKGProgressComponent`. Renders into an in-memory 120×40 buffer, flattens the cells into a string, asserts on substrings like `"Round 1"`, `"100%"`, `"DKG complete"`.

Uses contains-style assertions rather than exact buffer equality — layout tweaks don't invalidate them.

Run with:
```
cargo test -p starlab-client --test component_rendering
```

## When editing the Elm/DKG code

1. Make the change.
2. `cargo test -p starlab-client --tests` — covers rendering + state transitions. If this breaks, you broke an invariant the tests encode.
3. If you change the DKG protocol layer (anything under `src/protocal/` or `src/network/webrtc.rs`), also run `bash scripts/smoke-dkg.sh`.
4. If you change `DKGProgressComponent`'s rendering or the `DKGRound` enum, add/adjust tests in `component_rendering.rs` so the new behaviour is pinned.
