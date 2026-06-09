# Running a 3-Node MPC DKG Test Manually

Three separate terminals, one node per terminal. Commands assume
you're at the repo root (wherever that is on your machine — no
hardcoded paths).

For an automated smoke test instead, see `scripts/smoke-dkg.sh`
(repo root) and the cluster helpers in `apps/tui/scripts/`
(`build-starlab-client.sh`, `launch-3node-cluster.sh`,
`monitor-cluster.sh`, `health-check.sh`, `run-signal-server.sh`).
There is no README inside `apps/tui/scripts/` — earlier
drafts linked to one, but the scripts are self-documenting via
top-of-file comments.

## Build first

```bash
cargo build -p starlab-client --bin starlab-tui
```

The instructions below assume the debug binary is at
`./target/debug/starlab-tui` (the default workspace target
directory).

## Terminal 1 — MPC-1 (session creator)

```bash
RUST_LOG=info ./target/debug/starlab-tui --device-id mpc-1
```

Navigate with arrow keys:

1. Select **Create New Wallet** from the main menu → Enter.
2. Fill in the form: Name, Threshold (`2`), Total (`3`), Blockchain.
3. Submit; the app displays the session id for this ceremony.

## Terminal 2 — MPC-2 (joiner)

```bash
RUST_LOG=info ./target/debug/starlab-tui --device-id mpc-2
```

1. Select **Join Session** from the main menu.
2. Either pick the announced session from the list, or enter the
   session id from MPC-1.

## Terminal 3 — MPC-3 (joiner)

```bash
RUST_LOG=info ./target/debug/starlab-tui --device-id mpc-3
```

Same steps as MPC-2.

> Earlier drafts of this section used single-letter hotkeys like
> `Press n for "New Wallet"` / `Press d for "Discover Wallets"` /
> `Press j`. None of those bindings exist — the TUI navigates by
> arrow keys + Enter. See
> [`apps/tui/docs/KEYBOARD_NAVIGATION_GUIDE.md`](../../apps/tui/docs/KEYBOARD_NAVIGATION_GUIDE.md)
> for the authoritative keybind table.

## What should happen

1. MPC-1's Join Session list shows "👥 Participants (3/3):" once
   both joiners connect (real format at
   `apps/tui/src/elm/components/join_session.rs:329`).
2. All three nodes establish pairwise WebRTC connections (mesh).
3. The ceremony emits "🚀 Mesh ready! Starting real DKG protocol…"
   (real at `apps/tui/src/elm/command.rs:1268`) and DKG
   kicks off automatically.

Earlier drafts of this checklist claimed MPC-2/MPC-3 show a
"Connected to other participants" banner — that string doesn't
exist in `apps/tui/src/` (grep returns zero hits). The
joiner screens just render the same participant-list UI as
MPC-1 with progressively growing `Participants (N/3)` counts.

## Monitoring

Run with `RUST_LOG=info` (or `debug`) to see the per-peer
WebRTC lifecycle. Grep-friendly log lines from
`apps/tui/src/network/webrtc.rs`:

- `✅ WebRTC connection ESTABLISHED with <device_id>`
  (`webrtc.rs:370`) — per-peer success
- `✅ All N peer connections established, sending mesh_ready`
  (`webrtc.rs:496`) — mesh fully formed
- `🚀 Mesh ready! Starting real DKG protocol…`
  (`command.rs:1268`) — DKG auto-trigger

Earlier drafts said to look for "WebRTC CONNECTED" — that
uppercase string isn't emitted. The real log uses
"WebRTC connection ESTABLISHED" (mixed case).

## Signaling

By default the TUI connects to the production signal server
(`wss://xiongchenyu.dpdns.org`, per the `--signal-server` default
in `apps/tui/src/bin/starlab-tui.rs`). To run against a
local signal server instead, pass `--signal-server ws://localhost:9000`
and start the server in a fourth terminal:

```bash
cargo run -p starlab-signal-server
```
