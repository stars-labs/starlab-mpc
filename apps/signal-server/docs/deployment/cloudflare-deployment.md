# Cloudflare Worker Deployment Guide

For the canonical deployment-reference version of this doc (with
the real as-shipped `wrangler.toml`, Durable Object binding +
`Peers → Devices` class-rename migration, etc.), see
[`docs/deployment/CLOUDFLARE_DEPLOYMENT.md`](../../../../docs/deployment/CLOUDFLARE_DEPLOYMENT.md)
at the repo root. This file is the in-crate quick-reference.

## Prerequisites

1. Install Wrangler CLI (npm, bun, or cargo — any works):
```bash
npm install -g wrangler
# or: bun add -g wrangler
```

2. Install `worker-build` (used by `wrangler deploy`'s build step
   to compile the Rust→WASM Worker):
```bash
cargo install worker-build
```

3. Login to Cloudflare:
```bash
wrangler login
```

## Deploy

```bash
cd apps/signal-server/cloudflare-worker
wrangler deploy
```

`wrangler deploy` handles building internally via the
`[build] command = "worker-build --release"` entry in
`wrangler.toml` — no separate `wrangler build` step is needed.

Earlier drafts of this guide showed `wrangler publish`, which is
deprecated in newer Wrangler versions; `wrangler deploy` is the
current command.

## Configuration

Edit `wrangler.toml` with your own `account_id` + `routes` before
deploying. The committed config is bound to the upstream
maintainer's `xiongchenyu.dpdns.org` route and account. See the
workspace deploy guide above for a full wrangler.toml walkthrough.

## Features Added

The Cloudflare Worker supports the full session-discovery
wire protocol. Authoritative enum definitions are in
`apps/signal-server/server/src/lib.rs` (shared between the
standalone server and the Worker).

### Client → server messages
- **AnnounceSession**: creator declares a new DKG or signing
  session; saved to Durable Object storage with a `session:`
  prefix.
- **RequestActiveSessions**: new/reconnecting nodes ask for all
  known sessions (cold-start replay).
- **SessionStatusUpdate**: grows the participants list as
  peers join.
- **QueryMyActiveSessions**: "what sessions am I in?" rejoin
  query.
- **Register / ListDevices / Relay**: basic signaling primitives.

### Server → client messages
- **SessionAvailable**: broadcast echo of an AnnounceSession
  to every connected peer.
- **SessionsForDevice**: reply to RequestActiveSessions /
  QueryMyActiveSessions.
- **SessionListRequest**: server asks a specific device to
  re-announce its sessions (note: server→client direction,
  not client→server as earlier drafts of this doc claimed).
- **SessionRemoved**: notifies participants when a creator
  disconnects.
- **Devices / Relay / Error**: basic signaling counterparts.

### Storage
- Sessions are stored with `session:` prefix in Durable Object storage
- Sessions persist across reconnections
- Sessions are cleaned up when the creator disconnects

### How It Works

1. **Creator starts session**: Sends `AnnounceSession` message
2. **Worker stores session**: Saves to Durable Object storage
3. **Worker broadcasts**: Sends `SessionAvailable` to all connected devices
4. **New node connects**: Sends `RequestActiveSessions` 
5. **Worker responds**: Returns all stored sessions
6. **Discovery complete**: New node sees available sessions

## Testing

After deployment, test with:
```bash
cargo run --bin mpc-wallet-tui -p tui-node -- --device-id mpc-1
# Create a session

cargo run --bin mpc-wallet-tui -p tui-node -- --device-id mpc-2
# Should see the session from mpc-1
```

## Monitoring

View logs in Cloudflare dashboard:
1. Go to Workers & Pages
2. Select your worker
3. View Logs tab

## Rollback

If issues occur, you can rollback:
```bash
wrangler rollback
```