# Cloudflare Workers deployment

Canonical production deployment path for the signal server. The
Worker runs Rust compiled to WASM via the `worker` crate, backed by a
single Durable Object class (`Devices`) that holds session state.

## Prerequisites

- Cloudflare account with Workers + Durable Objects enabled (Durable
  Objects require a paid Workers plan or the free tier with limits).
- `wrangler` CLI installed — `npm install -g wrangler` (or `bun add -g wrangler`).
- `worker-build` — the build command in `wrangler.toml` invokes it;
  `cargo install worker-build` if it isn't on your PATH yet.

## Deployment steps

```bash
cd apps/signal-server/cloudflare-worker

# Log in once per workstation
wrangler login

# Edit wrangler.toml to use YOUR account_id + your own domain route
# (the committed account_id + `xiongchenyu.dpdns.org` route are
# bound to the upstream maintainer's account — the Worker won't
# deploy under them unless you own those credentials).

# Deploy
wrangler deploy

# Tail logs
wrangler tail
```

## wrangler.toml (as shipped)

```toml
name = "starlab-signal-server-cloudflare-worker"
main = "build/worker/shim.mjs"
compatibility_date = "2025-05-08"
account_id = "…"                                     # your account ID
workers_dev = true                                    # keeps .workers.dev fallback URL

routes = [
  { pattern = "xiongchenyu.dpdns.org", custom_domain = true }
]

[build]
command = "worker-build --release"

[durable_objects]
bindings = [
  { name = "Devices", class_name = "Devices" }
]

[[migrations]]
tag = "v2"
renamed_classes = [{ from = "Peers", to = "Devices" }]
```

The `renamed_classes` migration covers the historical
`Peers` → `Devices` rename; deploying from a fresh account won't
need a new migration tag unless you change the Durable Object class
name.

## Runtime architecture

Single Durable Object class `Devices` (src/lib.rs), which:

- Accepts WebSocket upgrades on the Worker's route
- Tracks connected `device_id`s and active sessions in DO storage
- Parses incoming `ClientMsg` / emits `ServerMsg` envelopes (see
  `apps/signal-server/server/src/lib.rs` for the authoritative enum
  definitions — both variants share that type crate)

### Message types handled

| Client → Server | Purpose |
|---|---|
| `Register` | Associate this WebSocket with a device_id |
| `ListDevices` | Request a snapshot of connected devices |
| `Relay` | Forward payload to a specific device |
| `AnnounceSession` | Declare a new DKG or signing session |
| `RequestActiveSessions` | Cold-start replay of known sessions |
| `SessionStatusUpdate` | Emitted on join — grows participants list |
| `QueryMyActiveSessions` | "What sessions am I in?" rejoin query |

| Server → Clients | Purpose |
|---|---|
| `Devices` | Connected-devices snapshot |
| `Relay` | Forwarded relay payload |
| `Error` | Signal-level error (parse failure, etc.) |
| `SessionAvailable` | Broadcast of a new session |
| `SessionListRequest` | Server asks a device to re-announce |
| `SessionsForDevice` | Reply to QueryMyActiveSessions |
| `SessionRemoved` | Sent when a session creator disconnects |

### Session lifecycle semantics

- Sessions are bound to their creator's WebSocket. When the creator
  disconnects, all participants receive `SessionRemoved` and the
  session is dropped — no orphans survive.
- Devices can rejoin with `QueryMyActiveSessions` to repopulate their
  view of in-flight sessions after a reconnect, without needing
  client-side persistence.

## Local development

```bash
# Run the Worker locally (uses miniflare under the hood)
wrangler dev

# Tail production logs
wrangler tail

# Formatted / filtered output
wrangler tail --format json | jq '.logs[].message'
```

## What to check after deploy

1. A client can WebSocket-upgrade against the deployed URL
   (`wscat -c wss://your-worker.workers.dev/` — replace with your
   route). First message should be a `Register` envelope.
2. `ListDevices` returns a list including the new connection.
3. `wrangler tail` shows the registration + subsequent relay traffic.
4. No `Error` envelopes come back in response to well-formed messages.
