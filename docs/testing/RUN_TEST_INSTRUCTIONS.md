# Running a 3-Node MPC DKG Test Manually

Three separate terminals, one node per terminal. Commands assume
you're at the repo root (wherever that is on your machine — no
hardcoded paths).

For an automated smoke test instead, see `scripts/smoke-dkg.sh`
and `apps/tui-node/scripts/README.md`.

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

1. Press `n` for "New Wallet"
2. Select `2 of 3 (secp256k1)`
3. Note the session code that appears

## Terminal 2 — MPC-2 (joiner)

```bash
RUST_LOG=info ./target/debug/mpc-wallet-tui --device-id mpc-2
```

1. Press `d` for "Discover Wallets"
2. Press `j` to join the discovered session

## Terminal 3 — MPC-3 (joiner)

```bash
RUST_LOG=info ./target/debug/mpc-wallet-tui --device-id mpc-3
```

1. Press `d` for "Discover Wallets"
2. Press `j` to join the discovered session

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
