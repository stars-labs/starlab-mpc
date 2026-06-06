# WebRTC Signal Server — Cloudflare Worker

Rust-over-WASM Cloudflare Worker backing the signal-server side of
the MPC Wallet. Uses the `Devices` Durable Object class to hold
connected-device + session-announcement state.

## Multi-tenant rooms (Option 3, #31)

Every WebSocket connection is routed to a Durable Object instance
**named after its room**: the `fetch` handler reads `?room=<id>` from the
URL and calls `Devices.id_from_name(room)`. Each room is a separate DO
instance with its **own storage and its own set of connections**, so
tenants are **fully isolated** — a device in room A never sees room B's
devices, session announcements, or relays, with no per-message filtering.

**The room name IS the tenant boundary, so it is mandatory and must be
strong.** There is intentionally **no backward-compatible default** (no
`"global"` bucket): a connection with a missing or weak room is
**rejected with HTTP 400**. A room must sanitize to **≥ 16 chars** of
`[A-Za-z0-9_-]` (≤ 64). This stops two unrelated tenants from colliding
on a guessable name like `acme`/`test` — use a high-entropy id (UUID /
128-bit token).

```
wss://panda.qzz.io/?room=7f3a9c2e-4b1d-4e8a-9c2f-001122334455   # ok (UUID)
wss://panda.qzz.io/?room=acme    # ✗ rejected (too short / guessable)
wss://panda.qzz.io               # ✗ rejected (no room)
```

Clients must include a strong room in the signal URL
(`--signal-server 'wss://panda.qzz.io/?room=<uuid>'` for CLI/TUI, or the
extension's signal setting). Generate one with `uuidgen` /
`python3 -c 'import uuid;print(uuid.uuid4())'`; share the **same** URL
with all participants of a ceremony. Recommended device-id convention
within a room: `<role>-<n>`. No new DO binding or migration is required
(same `Devices` class, more instances). See `docs/MULTI_TENANT.md`.

> ⚠️ Breaking: the previous behavior (no room → shared `global`) is gone.
> Any client still connecting to a bare `wss://panda.qzz.io` will be
> rejected and must add `?room=<uuid>`.

## Features

- **WebSocket-based signaling** for WebRTC device discovery + P2P
  relay
- **Durable Object** persistence (`Devices` class) for consistent
  device + session tracking across Worker invocations
- Compatible with Cloudflare's free plan (uses SQLite-backed
  Durable Objects via the `new_sqlite_classes` migration flag)

## Protocol

The wire protocol is shared with the standalone Rust signal server
under `../server/` — authoritative enum definitions live at
`apps/signal-server/server/src/lib.rs` (`ClientMsg` and `ServerMsg`).
Full message-type matrix + session-discovery semantics are in
[`docs/deployment/CLOUDFLARE_DEPLOYMENT.md`](../../../docs/deployment/CLOUDFLARE_DEPLOYMENT.md)
(workspace-level, rewritten in commit 1841904). Abbreviated
mini-reference:

### Client → Server

- `{ "type": "register", "device_id": "<id>" }`
- `{ "type": "list_devices" }`
- `{ "type": "relay", "to": "<id>", "data": <any JSON> }`
- `{ "type": "announce_session", "session_info": { … } }`
- `{ "type": "request_active_sessions" }`
- `{ "type": "session_status_update", "session_info": { … } }`
- `{ "type": "query_my_active_sessions" }`

### Server → Client

- `{ "type": "devices", "devices": [ … ] }`
- `{ "type": "relay", "from": "<id>", "data": <any JSON> }`
- `{ "type": "error", "error": "<message>" }`
- `{ "type": "session_available", "session_info": { … } }`
- `{ "type": "sessions_for_device", "sessions": [ … ] }`
- `{ "type": "session_list_request", "from": "<id>" }`
- `{ "type": "session_removed", "session_id": "<id>", "reason": "<text>" }`

## Project Structure

- `src/lib.rs` — Worker entry + `Devices` Durable Object impl
- `wrangler.toml` — Worker + Durable Object configuration
- `Cargo.toml` — `cdylib` + `rlib` crate-type, builds via
  `worker-build --release`

## Deploying to Cloudflare

```bash
# 1. Install wrangler + worker-build
npm install -g wrangler          # or: bun add -g wrangler
cargo install worker-build

# 2. Log in once per machine
wrangler login

# 3. Edit wrangler.toml with YOUR account_id + routes
#    (the committed config is bound to the upstream
#    maintainer's `xiongchenyu.dpdns.org` route)

# 4. Deploy — wrangler deploy handles the build via the
#    `[build] command = "worker-build --release"` entry in
#    wrangler.toml; no separate `wrangler build` step.
wrangler deploy
```

Older `wrangler publish` is deprecated in current Wrangler
versions — use `wrangler deploy`.

## Durable Object migration notes

The `Devices` Durable Object class was renamed from an earlier
`Peers` name; the committed `wrangler.toml` has:

```toml
[[migrations]]
tag = "v2"
renamed_classes = [{ from = "Peers", to = "Devices" }]
```

A fresh-account deployment that has never shipped `Peers` can use a
simpler migration:

```toml
[[migrations]]
tag = "v1"
new_sqlite_classes = ["Devices"]
```

(`new_sqlite_classes` is the free-plan-compatible route.)

## References

- Canonical deployment reference:
  [`docs/deployment/CLOUDFLARE_DEPLOYMENT.md`](../../../docs/deployment/CLOUDFLARE_DEPLOYMENT.md)
  + [`apps/signal-server/docs/deployment/cloudflare-deployment.md`](../docs/deployment/cloudflare-deployment.md)
  (the in-crate quick-reference companion — earlier draft mislabeled
  this link as `docs/signal-server/docs/...` which doesn't exist;
  the link target `../docs/...` correctly resolves to the
  sibling `apps/signal-server/docs/` subtree)
- [Cloudflare Durable Objects docs](https://developers.cloudflare.com/durable-objects/)
- [worker-rs](https://github.com/cloudflare/workers-rs) — the Rust
  SDK this crate uses
