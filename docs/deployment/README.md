# Deployment Guide

Production deployment paths that actually exist in this repo. Most of
this project's "operations" story is intentionally small — the signal
server is stateless, the wallet clients are single-binary apps, and
the production signaling path runs on a Cloudflare Worker.

## What ships to production

| Component | Artefact | Real deployment target |
|---|---|---|
| Signal server (edge) | Cloudflare Worker | `wrangler deploy` from `apps/signal-server/cloudflare-worker/` — see [CLOUDFLARE_DEPLOYMENT.md](CLOUDFLARE_DEPLOYMENT.md) |
| Signal server (self-host) | Native Rust binary from `apps/signal-server/server/` | systemd service behind an HTTPS terminator (nginx/caddy/Cloudflare Tunnel). Binds `0.0.0.0:9000`. Stateless, no DB, no Redis. |
| Browser extension | `bun run build` (Chrome MV3) / `bun run build:firefox` | Chrome Web Store / AMO distribution, or sideload via `.output/<browser>-mv3` |
| TUI wallet | `cargo build --release --bin starlab-tui` | End-user distribution — not a server deployment; single static binary. |
| Native desktop | `cargo build --release -p starlab-mpc-native` | End-user Slint desktop binary. |

## Cloudflare Worker signal server

The canonical production deployment. Stateless, globally distributed,
handled entirely by Cloudflare edge runtime.

```bash
cd apps/signal-server/cloudflare-worker
wrangler deploy        # deploys to the wrangler.toml-configured account
wrangler tail          # tail logs
```

Full instructions: [CLOUDFLARE_DEPLOYMENT.md](CLOUDFLARE_DEPLOYMENT.md).

## Self-hosted signal server

Verified against `apps/signal-server/server/src/main.rs` — the binary
reads zero environment variables, binds a hard-coded `0.0.0.0:9000`,
and holds all state in memory.

```bash
# Build
cargo build --release -p starlab-signal-server

# Run (foreground)
./target/release/starlab-signal-server
#   -> "Signal server listening on 0.0.0.0:9000"
```

### systemd unit template

```ini
[Unit]
Description=MPC Wallet signal server
After=network.target

[Service]
Type=simple
User=signal-server
ExecStart=/opt/starlab-mpc/starlab-signal-server
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

Terminate TLS in front of port 9000 with nginx, caddy, Cloudflare
Tunnel, or equivalent. The extension and TUI default to
`wss://xiongchenyu.dpdns.org`; override via chrome.storage.local
(extension) or `--signal-server` (TUI) during testing.

## Browser extension

```bash
cd apps/browser-extension
bun run build             # -> .output/chrome-mv3/ (default target)
bun run build:firefox     # -> .output/firefox-mv2/

# Package for web-store upload
cd .output/chrome-mv3 && zip -r ../../starlab-mpc-chrome.zip .
```

Install unpacked during development via `chrome://extensions` →
Developer Mode → "Load unpacked" → `.output/chrome-mv3`.

## Operator notes

### Kernel parameters for a busy self-hosted signal server

Applies to the native Rust signal server under load (thousands of
concurrent WebSocket connections). Not needed for the Cloudflare
Worker variant — that's Cloudflare's problem.

```bash
# /etc/sysctl.d/99-signal-server.conf
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.core.netdev_max_backlog = 65535
fs.file-max = 2097152
```

### Firewall

Self-hosted: open 9000/tcp inbound (or whatever port your TLS
terminator exposes). No other inbound ports are required — WebRTC
traffic goes peer-to-peer after signaling completes, so the signal
server does not proxy media.

### Observability

The signal server currently exposes no `/metrics`, `/health`, or
`/api/*` endpoints (verified: zero route handlers match). Operate
from `stdout`/`stderr` logs via systemd journal; add structured
logging by compiling with `RUST_LOG=starlab_signal_server=info`.

Future work: Prometheus-compatible `/metrics` endpoint, structured
log output (JSON).

## Troubleshooting

```bash
# Verify TLS+DNS to the public signal server
curl -v https://xiongchenyu.dpdns.org/
# The server is WebSocket-only, so expect "400 Bad Request" or similar
# on a plain HTTP GET — that still confirms reachability.

# Test a WebSocket upgrade
wscat -c wss://xiongchenyu.dpdns.org/

# Self-hosted: test local port
wscat -c ws://localhost:9000/

# Check connection latency
mtr xiongchenyu.dpdns.org
```

What is **not** supported today (deliberately — or because the repo
never shipped it):

- No Dockerfile or docker-compose.yml for any component (the old
  starlab-client Dockerfile + compose were pre-monorepo and got removed
  rather than carried as broken examples — see
  `apps/tui/docs/DEPLOYMENT_GUIDE.md` for the history).
- No Kubernetes manifests.
- No Redis / database layer — the signal server is stateless and
  holds session state in process memory.
- No Prometheus / Grafana scrape targets.
- No TURN server setup. The browser extension hard-codes Google's
  public STUN (`stun.l.google.com:19302` in
  `apps/browser-extension/src/entrypoints/offscreen/webrtc.ts:32`);
  the TUI currently passes an empty ICE-server list
  (`src/network/webrtc.rs:285`), so TUI-only peers only connect
  across directly-routable networks. If operators need reliable
  TUI-to-TUI over the public internet, add STUN config at the
  peer-connection construction sites — the extension's layout is
  the template.

## Navigation

- [← Back to Main Documentation](../README.md)
- [Testing Guide →](../testing/README.md)
- [Cloudflare-specific deployment →](CLOUDFLARE_DEPLOYMENT.md)
