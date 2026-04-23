# Running a 3-Node MPC DKG Test Manually

Three separate terminals, one node per terminal. Commands assume
you're at the repo root (wherever that is on your machine — no
hardcoded paths).

For an automated smoke test instead, see `scripts/smoke-dkg.sh`
(repo root) and the cluster helpers in `apps/tui-node/scripts/`
(`build-tui-node.sh`, `launch-3node-cluster.sh`,
`monitor-cluster.sh`, `health-check.sh`, `run-signal-server.sh`).
There is no README inside `apps/tui-node/scripts/` — earlier
drafts linked to one, but the scripts are self-documenting via
top-of-file comments.

## Build first

```bash
cargo build -p tui-node --bin mpc-wallet-tui
```

The instructions below assume the debug binary is at
`./target/debug/mpc-wallet-tui` (the default workspace target
directory).

## Terminal 1 — MPC-1 (session creator)

```bash
RUST_LOG=info ./target/debug/mpc-wallet-tui --device-id mpc-1
```

Navigate with arrow keys:

1. Select **Create New Wallet** from the main menu → Enter.
2. Fill in the form: Name, Threshold (`2`), Total (`3`), Blockchain.
3. Submit; the app displays the session id for this ceremony.

## Terminal 2 — MPC-2 (joiner)

```bash
RUST_LOG=info ./target/debug/mpc-wallet-tui --device-id mpc-2
```

1. Select **Join Session** from the main menu.
2. Either pick the announced session from the list, or enter the
   session id from MPC-1.

## Terminal 3 — MPC-3 (joiner)

```bash
RUST_LOG=info ./target/debug/mpc-wallet-tui --device-id mpc-3
```

Same steps as MPC-2.

> Earlier drafts of this section used single-letter hotkeys like
> `Press n for "New Wallet"` / `Press d for "Discover Wallets"` /
> `Press j`. None of those bindings exist — the TUI navigates by
> arrow keys + Enter. See
> [`apps/tui-node/docs/KEYBOARD_NAVIGATION_GUIDE.md`](../../apps/tui-node/docs/KEYBOARD_NAVIGATION_GUIDE.md)
> for the authoritative keybind table.

## What should happen

1. MPC-1 should show "Participants (3/3)" once both joiners connect
2. MPC-2 and MPC-3 should show "Connected to other participants"
3. All nodes should establish WebRTC connections (mesh network)
4. DKG should start automatically once the mesh is ready

## Monitoring

- Watch the logs in each terminal for connection status
- Look for "WebRTC CONNECTED" messages
- Check for "Mesh ready" notifications

## Signaling

By default the TUI connects to the production signal server
(`wss://xiongchenyu.dpdns.org`, per the `--signal-server` default
in `apps/tui-node/src/bin/mpc-wallet-tui.rs`). To run against a
local signal server instead, pass `--signal-server ws://localhost:9000`
and start the server in a fourth terminal:

```bash
cargo run -p webrtc-signal-server
```
