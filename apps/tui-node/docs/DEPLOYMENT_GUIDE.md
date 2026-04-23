# TUI Node Deployment Guide

Covers running the standalone Rust signal server + one or more TUI
nodes. For the browser extension + native-node + Cloudflare-Worker
paths, see the workspace-level `docs/deployment/README.md` +
`docs/deployment/CLOUDFLARE_DEPLOYMENT.md`.

## Quick Start

Scripts referenced below live at **`apps/tui-node/scripts/`** (not
`./scripts/` at the repo root — that's a different set for
workspace-wide build/test/clean).

```bash
# From the repo root
cd apps/tui-node

# Build the signal server binary (release mode)
./scripts/build-signal-server.sh

# Build the TUI binary
./scripts/build-tui-node.sh

# Launch signal server + 3 TUI nodes locally
./scripts/launch-3node-cluster.sh

# Health check (verifies signal server is reachable + each node is up)
./scripts/health-check.sh [--verbose]

# Continuous monitoring
./scripts/monitor-cluster.sh
```

## Deployment methods

### Method 1: direct invocation

Start signal server:

```bash
# The signal server binds 0.0.0.0:9000 unconditionally — no env var
# controls this today (verified: signal-server/server/src/main.rs
# reads zero env vars). If you need a different port, patch main.rs.
cargo run --release -p webrtc-signal-server
```

Or use the wrapper script:

```bash
./scripts/run-signal-server.sh
```

Start TUI nodes in separate terminals:

```bash
# Terminal 1
cargo run -p tui-node --bin mpc-wallet-tui -- \
  --signal-server ws://localhost:9000 --device-id mpc-1

# Terminal 2
cargo run -p tui-node --bin mpc-wallet-tui -- \
  --signal-server ws://localhost:9000 --device-id mpc-2

# Terminal 3
cargo run -p tui-node --bin mpc-wallet-tui -- \
  --signal-server ws://localhost:9000 --device-id mpc-3
```

Omit `--signal-server` to use the default
`wss://xiongchenyu.dpdns.org` (the Cloudflare Worker deployment
bound to that domain).

### Method 2: systemd

The repo does NOT ship pre-built systemd unit files — earlier
drafts of this guide referenced `systemd/mpc-signal-server.service`
+ `mpc-wallet-cluster.target` + `mpc-wallet-node@.service` that
don't exist (verified: no `systemd/` dir, no `.service` files in
the tree).

For a self-hosted production deployment, write your own unit files
adapted from the template in `docs/deployment/README.md` § "systemd
unit template". The binary paths are whatever you place under
`/opt/mpc-wallet/` or similar after `cargo build --release`.

## Configuration

### Signal server

Reads zero environment variables. Binds `0.0.0.0:9000` as hardcoded
in `apps/signal-server/server/src/main.rs:35`. To change the bind
address, edit that line (or add a CLI flag + wire it through).

### TUI node

Accepts these CLI flags (authoritative:
`apps/tui-node/src/bin/mpc-wallet-tui.rs`):

| Flag                      | Default                           |
|---------------------------|-----------------------------------|
| `--device-id <ID>`        | hostname                          |
| `--signal-server <URL>`   | `wss://xiongchenyu.dpdns.org`     |
| `--offline`               | (off)                             |
| `--log-location <PATH>`   | `~/.frost_keystore/logs/mpc-wallet.log` |
| `--log-level <LEVEL>`     | `info`                            |

Environment: only `HOME` (to compute the keystore path), `RUST_LOG`
(tracing-subscriber directive), and `PERF_MONITORING` (enables
the `perf_monitor` instrumentation) are consulted.

There is no `DATA_DIR` env var — the keystore location is fixed at
`~/.frost_keystore` (see the keystore-directory fix in 22ae959).

## Health checks and monitoring

Real helper scripts in `scripts/` are signal-server-focused:

```bash
# Continuous signal-server health monitor (polls the WS upgrade
# endpoint on a loop — see scripts/signal-server-monitor.sh)
./scripts/signal-server-monitor.sh

# Debug runner that starts the signal server with verbose logging
./scripts/signal-server-debug.sh
```

Earlier drafts of this section referenced `./scripts/health-check.sh
--verbose` and `MONITOR_INTERVAL=30 ./scripts/monitor-cluster.sh`.
Neither file exists — `ls scripts/` shows only
`build-all.sh / clean-all.sh / README.md / signal-server-debug.sh /
signal-server-monitor.sh / smoke-dkg.sh / test-all.sh`. Node-level
health monitoring (per-node process state + log tailing) is not
automated today and would need custom tooling.

There is no `/health` HTTP endpoint on the signal server (earlier
drafts of this guide showed `curl -v http://localhost:9000/health`
— that returns 400 or connection-closed because the server only
accepts WebSocket upgrades). Real reachability probe:

```bash
# WebSocket upgrade attempt — success = TCP/TLS reach + server alive
wscat -c ws://localhost:9000/
```

## Testing the deployment

```bash
# Single node against a running signal server
cargo run -p tui-node --bin mpc-wallet-tui -- \
  --signal-server ws://localhost:9000 --device-id test-node

# Smoke DKG test (runs the whole workspace test suite)
./scripts/smoke-dkg.sh
```

Earlier drafts of this section referenced
`./scripts/launch-3node-cluster.sh` for a "Full 3-node cluster"
launch. That script does not exist. Multi-node local testing is
done by running three `mpc-wallet-tui` processes manually (in
separate terminals) with distinct `--device-id` values against one
signal server. The `examples/webrtc_mesh_e2e_test.rs` binary
exercises 3-peer mesh behaviour in-process for smoke coverage
without needing three real TUI instances.

## Resource requirements

Not yet benchmarked — earlier drafts of this guide quoted specific
figures (`Signal Server: ~50MB RAM / ~100-200MB per TUI node /
100+ concurrent nodes`) without a source. Real sizing depends on
concurrent-peer load + the number of active DKG/signing
ceremonies; start small and scale vertically. WebRTC full-mesh
degree is `n·(n-1)/2` peer connections — keep cohorts small or
provision accordingly.

## Not currently supported

- **Docker deployment**: the `Dockerfile` + `docker-compose.yml`
  that used to live at `apps/tui-node/` were written for a
  pre-monorepo, pre-edition-2024 layout (Rust 1.75, single-crate
  `COPY Cargo.lock`). They were removed rather than carried as
  broken examples. Reintroducing Docker deployment would need a
  Dockerfile at the workspace root with `FROM rust:1.85+` and a
  proper multi-stage build covering every workspace member crate.
- **systemd units**: see Method 2 above — write your own from the
  workspace-level template.
- **Kubernetes manifests / Helm charts**: not shipped; see
  `docs/deployment/README.md` § "Not supported" for the full
  absent-infra list.
- **Prometheus `/metrics`**: not implemented. Operational visibility
  is stdout/stderr via `tracing` + the `--log-location` file.

## Security notes

- Run the signal server as a non-root user. Because the binary
  binds a privileged port below 1024 only if you terminate TLS
  in front (see `docs/deployment/README.md` for nginx / caddy /
  Cloudflare Tunnel options), port 9000 is fine for an
  unprivileged user.
- Firewall 9000/tcp inbound (or whatever port your TLS proxy
  exposes). WebRTC traffic is peer-to-peer after signaling, so
  the signal server doesn't proxy media/data.
- Keystore files under `~/.frost_keystore/<device_id>/` are
  PBKDF2-HMAC-SHA256 (100k) + AES-256-GCM encrypted. The
  password is what gates them — don't store the password next
  to the `.dat` file.
