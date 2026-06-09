# Multi-tenant isolation — decision (#31)

**Decision:** **Option 3 — Cloudflare Worker, one Durable Object instance per
room — is implemented** and is the production multi-tenant path (panda.qzz.io).
Option 1 (separate standalone-server instances) remains fine for local/dev or
air-gapped setups. Option 2 (room namespacing inside the standalone Rust server)
stays deferred — only needed if the *standalone* server must be multi-tenant.

> **Status:** ✅ Option 3 shipped (`apps/signal-server/cloudflare-worker`,
> see §"Option 3 — implemented"). Clients select a tenant purely by URL
> (`wss://panda.qzz.io/?room=<tenant>`); no client code change.

---

## Current state (the problem)

`apps/signal-server/server/src/lib.rs` is a **single flat namespace** — verified
by code review:

- One global `devices: HashMap<device_id, sender>`; `device_id` must be globally
  unique (duplicate `Register` is rejected, l.101).
- `Devices` roster is broadcast to **everyone** on every (de)register (l.110-114).
- `announce_session` → `session_available` broadcast to **all** connected
  devices (l.273).
- `RequestActiveSessions` → returns **all** stored sessions to any requester
  (l.284).
- `Relay { to }` → delivered to a specific device by id (l.219).

So on a shared server, every tenant **sees every other tenant's** device ids and
session announcements. Funds stay safe (you can only contribute a usable share to
a session you were enrolled in, and identifiers come from the participant set),
but it leaks metadata (who's online, what ceremonies exist) and is noisy. Not
acceptable for separate investor cohorts / customers on one endpoint.

---

## Options

| | Isolation | Code change | Ops | When |
|---|---|---|---|---|
| **1. Instance per tenant** | Full (separate process/port/host) | **none** | run N servers | **now / demos / few tenants** |
| **2. Room namespacing** | Full, logical, on one process | moderate, localized to `lib.rs` + `Register` | one server | single hosted endpoint, many tenants |
| **3. Cloudflare Worker, DO per room** | Full, serverless, autoscaling | port Option 2 logic into the Worker (`apps/signal-server/cloudflare-worker`) | managed edge | hosted product at scale |

### Why Option 1 now
- Zero code, ships today, full isolation (different process ⇒ no cross-tenant
  visibility at all). Device-id collisions only matter *within* a tenant.
- Perfect for investor demos: give each cohort its own URL.
```bash
MPC_SIGNAL_BIND=0.0.0.0:9001 starlab-signal-server   # tenant / cohort A
MPC_SIGNAL_BIND=0.0.0.0:9002 starlab-signal-server   # tenant / cohort B
```
Clients: `--signal-server ws://<host>:9001` (CLI/TUI) or the extension's signal
setting.

### Conventions (apply regardless of option)
- **Device-id scheme:** `"<tenant>-<role>-<n>"` (e.g. `acme-cli-a`) → globally
  unique, human-traceable, collision-proof (ties into #29 hygiene).
- **Keystores:** one directory per `(tenant, device)`; never shared across tenants.
- **Sessions:** include the tenant in the wallet name; never reuse session ids
  across tenants.

---

## Option 2 implementation plan (when a single endpoint is needed)

Localized change in `apps/signal-server/server/src/lib.rs` (+ the `ClientMsg`
type in the shared crate). A "room" is the tenant boundary.

1. **Register carries a room.** Add `room: Option<String>` to
   `ClientMsg::Register` (default `"default"` for backward compat). Track the
   socket's room alongside its `device_id`.
2. **Scope the device map by room.** Either
   `HashMap<Room, HashMap<DeviceId, Sender>>` or key by `(room, device_id)`.
   Duplicate-id rejection becomes per-room (two tenants may both have `cli-a`).
3. **Scope the four fan-out sites to the sender's room:**
   - `Devices` roster broadcast (l.110) → only same-room devices.
   - `session_available` on announce (l.273) → only same-room devices.
   - `RequestActiveSessions` reply (l.284) → only sessions whose room matches.
   - `SessionListRequest` broadcast (l.299) → only same-room devices.
4. **Scope sessions.** Store `room` in `StoredSession`; key the sessions map by
   `(room, session_id)` (or filter by room on read). Cleanup unchanged.
5. **Relay** (l.219): resolve `to` within the sender's room only; `to == "*"`
   broadcasts within the room. Cross-room relay is refused.
6. **Clients pass the room:** add `--room`/`MPC_ROOM` to CLI/TUI and a room field
   to the extension's connect. Absent ⇒ `"default"`.

**Acceptance / test:** an L1-style test — two rooms on one embedded server; a
node in room A never receives room B's `session_available`/`Devices`, and a relay
A→(B's device) is refused. Reuse the `starlab_signal_server::run` in-process
harness already used by `simulate`/e2e.

**Effort:** ~half a day; risk low (additive, default-room keeps existing clients
working). Not started — gated on the "single hosted endpoint" requirement.

---

## Option 3 — implemented (Cloudflare Worker, DO per room)

The production worker (`apps/signal-server/cloudflare-worker/src/lib.rs`) routes
each connection to a Durable Object instance **named after its room**:

```rust
let room = extract_room(&req);            // ?room=<tenant>, sanitized, default "global"
let id = env.durable_object("Devices")?.id_from_name(&room)?;
id.get_stub()?.fetch_with_request(req).await
```

Why this is full isolation, by construction:
- `id_from_name(room)` is a deterministic 1:1 room→instance map. Different rooms ⇒
  different DO instances ⇒ **separate `state.storage()` and separate live
  WebSocket sets**. The existing device/session/relay logic is unchanged — each
  instance simply only ever sees its own room's traffic. No per-message filtering,
  no shared map to leak across tenants.
- The room name **is** the tenant boundary, so it is **mandatory and must be
  strong** — sanitized to `[A-Za-z0-9_-]` and **≥ 16 chars**; a missing/weak room
  is **rejected (HTTP 400)**, with **no `"global"` fallback**. This is a
  deliberate **breaking change** (not backward compatible): it removes the shared
  bucket two tenants could collide on. Solves the *tenant-name collision* problem —
  you can't pick a guessable name like `acme`; use a high-entropy id (UUID).
  Adversarial join-with-known-id is still possible (the room is a bearer
  capability); the real fix for that is auth-derived rooms (Option B in §"Options",
  still deferred) — but even an intruder can't steal keys (FROST identifiers come
  from the enrolled participant set).
- **No new DO binding and no migration** — same `Devices` class, just more
  instances. `wrangler.toml` is untouched.

### Client usage (no code change)
Put the tenant in the signal URL:
```
CLI/TUI : --signal-server 'wss://panda.qzz.io/?room=<uuid>'
extension: set the signal server to wss://panda.qzz.io/?room=<uuid>
(no room) : REJECTED (HTTP 400) — a strong ?room is required
```
Generate a room with `uuidgen` / `python3 -c 'import uuid;print(uuid.uuid4())'`
and share the **same** URL with all ceremony participants. Use a `<role>-<n>`
device-id convention within a room.

### Verification
- `sanitize_room` unit tests (host): `cargo test -p starlab-signal-server-cloudflare-worker` (5 pass).
- Edge build: `cargo check --target wasm32-unknown-unknown` (or `worker-build --release`).
- End-to-end isolation (manual / miniflare): connect device `a` to `?room=x` and
  device `b` to `?room=y`; `a`'s `request_active_sessions` and the `devices`
  roster must never include `b` or `b`'s announced sessions, and a relay `a→b` is
  undeliverable (different instance). Automating this needs `wrangler dev`
  (miniflare); tracked alongside #33's harness work.

## Recommendation summary

- **Hosted multi-tenant endpoint (production):** ✅ **Option 3 (shipped)** — one
  Cloudflare DO instance per `?room=<tenant>`. Pick a room per tenant in the URL.
- **Local / dev / air-gapped:** Option 1 — one standalone instance per tenant
  (the CF worker isn't involved there).
- **Option 2** (room namespacing in the standalone Rust server) stays deferred —
  only if the *standalone* server itself must be multi-tenant. Plan above.
- Adopt the `<tenant>-<role>-<n>` device-id convention within each room — it's
  free and prevents the collision class (#29).

Tracking: issue #31. Option 3 implemented; Option 2 remains optional/deferred.
