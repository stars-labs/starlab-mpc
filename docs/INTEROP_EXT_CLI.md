# Interop: 1 browser extension + 2 CLI — code review & operations guide

This document does two things:

1. **Reviews the current code logic** for running a mixed ceremony (browser
   extension ↔ CLI nodes), and flags what works vs what will bite you.
2. **Gives detailed operating procedures** for the target topology
   (1 extension + 2 CLI), then for **multi-device** and **multi-tenant** setups.

> TL;DR: the wire protocol and the cryptography are compatible **by
> construction**, but there is **one confirmed blocker** — the extension and the
> Rust core derive FROST participant identifiers differently (array order vs
> sorted order). Fix that (or use the ordering workaround in §3.4) before
> relying on a live ext↔CLI ceremony. Live ext↔CLI has **never been run in an
> automated test** (only mock-based comparisons exist), so rehearse first.

---

## 1. What the three clients share

| | Browser extension | CLI (`mpc-wallet-cli`) | TUI / native |
|---|---|---|---|
| FROST crypto | `@mpc-wallet/core-wasm` (Rust→WASM) | `tui_node` core (Rust) | `tui_node` core |
| Same FROST crates? | **yes** (`frost_secp256k1`/`frost_ed25519` via frost-core) | yes | yes |
| Wire protocol | hand-written TS, modeled on TUI | Rust core | Rust core |
| Transport | WebSocket (signal) + WebRTC (mesh) | same | same |

The extension is an **independent reimplementation** of the protocol glue over
the *same* FROST primitives. So DKG packages and signatures are
cryptographically interchangeable; the risk is entirely in the glue (identifiers,
wire shapes, session orchestration).

---

## 2. Code-logic review

### 2.1 Wire protocol — ✅ compatible
- Extension `announceSession` emits `{type:"announce_session", session_info:{…}}`
  (`src/entrypoints/background/websocket.ts:113`), and `session-parse.ts` is
  explicitly modeled on the core's `parse_session_info` (flat lowercase
  `session_type`, synthesises `accepted_devices`).
- The CLI's wire contract is pinned by goldens
  (`apps/cli-node/tests/fixtures/dkg_wire_protocol.golden.txt`,
  `signing_wire_protocol.golden.txt`). The extension's emitted shape matches
  those fields. When the live ext↔CLI harness lands, diff the extension's frames
  against those goldens to keep them aligned.

### 2.2 Cryptography / addresses — ✅ correct on the extension side
- The extension's WASM eth-address derivation
  (`frost-core::Secp256k1Curve::get_eth_address`) decompresses the key correctly
  (`k256 from_sec1_bytes → to_encoded_point(false) → keccak(X‖Y)[12..]`). The
  ETH-address bug that was fixed recently was in `tui_node::blockchain_config`
  (the CLI/TUI bridge), **not** in the extension.
- Same FROST crates ⇒ DKG/signing artifacts are cross-compatible.

### 2.3 🛑 FROST identifier derivation — **MISMATCH (blocker)**
This is the one that will break a live ext↔CLI ceremony.

- **Core** (`apps/tui-node/src/protocal/dkg.rs:44`): `canonical_identifier` SORTS
  the participant device-ids, then uses `position + 1`:
  ```rust
  let mut sorted = participants.iter().collect::<Vec<_>>();
  sorted.sort();
  let idx = sorted.iter().position(|p| *p == device_id)?;   // 1-based id
  ```
- **Extension** (`apps/browser-extension/src/entrypoints/offscreen/webrtc.ts:619,
  752, 1299`): uses `participants.indexOf(peerId) + 1` on the array **as
  received** — and nothing sorts `participants` anywhere in the extension
  (`session-parse.ts` only filters to strings).

Consequence: unless the `participants` array is already in sorted (lexicographic
device-id) order, the extension assigns each peer a **different FROST identifier**
than the Rust nodes do. FROST DKG/signing then fails or, worse, mismatches
silently. The server grows the participant list in **join order**, so it is
generally *not* sorted.

It "accidentally works" only when devices happen to join in lexicographic order
of their device-ids.

**Fix (recommended):** sort `participants` once where the session is established
in the extension (e.g. in `session-parse.ts` when building the parsed session, or
in `webrtc.ts` before computing any identifier), so every `indexOf` matches the
core's sorted order. Small, localized change — but it MUST match the core's
`sort()` semantics (plain lexicographic byte/string sort).

### 2.4 Session discovery & DKG trigger — ⚠ works, with caveats
- Signal server (`apps/signal-server/server/src/lib.rs`) is a **flat shared
  bus**: `announce_session` is broadcast to *all* connected devices (l.273), and
  `RequestActiveSessions` returns *all* stored sessions (l.284). No participant
  filtering, no rooms. (See §5 multi-tenant.)
- Duplicate `device_id` registration is **rejected** (l.101) — so two clients
  with the same id can't both connect (good, but plan unique ids).
- Extension DKG trigger (`webSocketManager.ts:462 maybeTriggerCeremony`): fires
  DKG when `participants.length === total` (all N joined) and only if this device
  is in `participants`. Signing fires at `>= threshold`.
- The participant list grows via `session_status_update` (server appends joiners,
  l.306+). All clients must converge on the same final participant set/order
  (see §2.3).

### 2.5 Signing announce carries `blockchain = curve` — ⚠ cosmetic
The headless/TUI signing announce sets `blockchain` to the curve
("secp256k1"), not a chain ("ethereum"), because the sign API has no per-chain
context. Signatures verify regardless; an extension co-signer parsing this sees
the curve where it might display a chain. Align when convenient (see
`docs/cli-conformance-testing.md` §5.2).

### 2.6 Test coverage — ⚠ ext↔CLI is unverified live
CLI↔CLI (incl. cross-process) is covered by the conformance suite. The only
ext↔CLI tests today are **mock-WASM** (`apps/browser-extension/tests/
cli-chrome-comparison.test.ts`) — they do not run a real joint ceremony. So a
live 1-ext + 2-CLI run is **first-time integration**: rehearse it before a demo.

---

## 3. Procedure: 1 browser extension + 2 CLI

Target: a **2-of-3** wallet shared by `cli-a`, `cli-b`, and the extension
(`ext-1`); afterwards any two of them can sign.

### 3.1 Prerequisites
- A signal server reachable by all three (live `wss://panda.qzz.io`, or a local
  one — see §4.2).
- **A shared strong room** (hosted server requires it, #31): generate once with
  `uuidgen` and use the SAME value for all three (`ROOM=$(uuidgen)`). A local
  standalone server (§4.2) needs no room.
- **Unique device-ids**: `cli-a`, `cli-b`, `ext-1`. Never reuse.
- Isolated keystore dir per CLI node.
- **Apply the §2.3 fix first**, or use the ordering workaround in §3.4.

### 3.2 Start the two CLI nodes (JSONL `serve`)
```bash
ROOM=$(uuidgen)   # the ONE shared room for all three; the extension uses the same
# terminal 1
mpc-wallet-cli serve --device-id cli-a --keystore /tmp/ks-a --signal-server wss://panda.qzz.io --room "$ROOM"
# terminal 2
mpc-wallet-cli serve --device-id cli-b --keystore /tmp/ks-b --signal-server wss://panda.qzz.io --room "$ROOM"
```
For the extension, set its signal server to `wss://panda.qzz.io/?room=<the same ROOM>`
(or set the room in its settings).
Each prints a `ready` event, then accepts newline-delimited JSON commands on
stdin and emits events on stdout. Connect both:
```json
{"cmd":"connect"}
```
(They emit `{"event":"connection","connected":true}`.)

### 3.3 Run the 3-party DKG
Pick **one** creator. Two equivalent options:

**Option A — a CLI creates, extension + other CLI join:**
1. `cli-a` stdin:
   ```json
   {"id":1,"cmd":"create_wallet","name":"shared","threshold":2,"total":3,"password":"pw-a"}
   ```
   `cli-a` emits `{"event":"session_announced","session_id":"dkg_…"}` — note the id.
2. `cli-b` stdin (using that id):
   ```json
   {"id":2,"cmd":"join_session","session_id":"dkg_…","password":"pw-b"}
   ```
3. **Extension**: open the popup → it shows the available session (auto-discovered
   via `session_available`) → **Join** → enter its password.
4. When all three are in (`participants.length === 3 === total`), DKG runs over
   the WebRTC mesh. All three end with the **same group public key / address**.

**Option B — the extension creates, both CLIs join:**
1. Extension popup → Create Wallet → 2-of-3 → note the session id it shows.
2. Each CLI: `{"cmd":"join_session","session_id":"…","password":"…"}`.

> Discovery note: a CLI that connected *after* the announce can still find the
> session — send `{"cmd":"list_sessions"}` (it triggers a server replay; the
> session arrives as a `session_available` event).

### 3.4 Ordering workaround (until §2.3 is fixed)
The FROST identifiers come out consistent only if every client sees the same
**sorted** participant order. Until the extension sorts, make the natural order
*be* sorted: choose device-ids whose **join order equals lexicographic order**.
E.g. name them `n1-cli-a`, `n2-cli-b`, `n3-ext` and have them join in that order.
Fragile — prefer the real fix.

### 3.5 Threshold signing (2-of-3)
1. Initiator (say `cli-a`):
   ```json
   {"id":10,"cmd":"sign","wallet_id":"wallet-dkg_…","message":"hello","encoding":"utf8","password":"pw-a"}
   ```
2. A second signer approves: the **extension** shows a signing request → Approve;
   or `cli-b`: `{"cmd":"approve_signing","session_id":"sign_…","password":"pw-b"}`.
3. Initiator emits `{"event":"signature_complete","signature":"0x…"}`. The third
   party isn't needed (2-of-3).

### 3.6 Pre-flight (always)
Before any live ext↔CLI run, prove the Rust stack is healthy:
```bash
scripts/demo/preflight.sh
```
It does full DKG + signing CLI↔CLI in seconds. It does **not** exercise the
extension — so additionally do one rehearsal of the actual 1-ext+2-CLI flow.

---

## 4. Multi-device operation

Same as §3, but the three nodes are on three machines.

### 4.1 Rules
- **Network:** every device must reach the *same* signal server. For
  `wss://panda.qzz.io` that means internet on each. WebRTC then forms a direct
  mesh between devices (NAT/firewalls permitting; loopback/LAN is easiest).
- **Unique device-ids** across *all* devices (the server rejects duplicates).
- **Isolated keystores** per device (default `~/.frost_keystore`; fine since each
  machine is separate).
- All devices must agree on `total`/`threshold` and converge on the same
  participant set (see §2.3 — the sort fix matters even more across devices,
  since join order is now timing-dependent).

### 4.2 Local signal server (no internet / most reliable)
On one machine:
```bash
MPC_SIGNAL_BIND=0.0.0.0:9000 mpc-wallet-cli ...   # or: cargo run -p webrtc-signal-server
```
Point every node at `ws://<that-machine-LAN-ip>:9000`. Removes the internet
dependency; ideal for a controlled demo room. (See the fallback ladder in
`docs/INVESTOR_GUIDE.md`.)

### 4.3 WebRTC reachability
The mesh needs the peers to connect directly. On the same LAN this is fine. Across
the open internet behind symmetric NATs you may need a TURN relay — out of scope
here, but flag it: a pure local-LAN demo avoids the issue entirely.

---

## 5. Multi-tenant operation

"Multi-tenant" = multiple independent groups (e.g. different investor cohorts)
using the system without seeing each other.

### 5.1 The current isolation model — ⚠ none at the server
The signal server is a **single flat namespace**:
- One global `device_id → connection` map; ids must be globally unique.
- `Devices` roster is broadcast to **everyone** on every (de)register (l.110).
- `announce_session` → broadcast to **all** connected devices (l.273).
- `RequestActiveSessions` → returns **all** sessions to any requester (l.284).

So on a shared server, every tenant **sees every other tenant's** device ids and
session announcements. Isolation today exists only at the *ceremony* level:
- you only join a session whose `session_id` you know, and
- DKG identifiers come from the participant list — a stranger joining a session
  they weren't enrolled in won't hold a valid share.

That's enough to keep funds safe, but it leaks metadata (who's online, what
sessions exist) and is noisy.

### 5.2 Recommended tenant isolation (pick per need)

**Option 1 — one signal-server instance per tenant (recommended, zero code).**
Run a separate `webrtc-signal-server` (different port/host) per tenant; give each
tenant its own URL. Full isolation: no cross-tenant visibility, device-id
collisions only matter within a tenant. Cheapest correct answer today.
```bash
MPC_SIGNAL_BIND=0.0.0.0:9001 webrtc-signal-server   # tenant A
MPC_SIGNAL_BIND=0.0.0.0:9002 webrtc-signal-server   # tenant B
```

**Option 2 — namespacing on a shared server (small code change).** Add a
`tenant`/`room` field to `Register` and scope the `devices` map, the `Devices`
broadcast, `announce_session` fan-out, and `RequestActiveSessions` to the same
room. ~localized change in `lib.rs` (the four broadcast sites in §5.1). Gives
logical multi-tenancy on one process; needed if you want a single hosted
endpoint.

**Option 3 — Cloudflare Worker variant** (`apps/signal-server/cloudflare-worker`)
with a Durable Object per tenant/room — the scalable hosted form of Option 2.

### 5.3 Practical conventions regardless of option
- **Device-id scheme:** `"<tenant>-<role>-<n>"` (e.g. `acme-cli-a`) so ids are
  globally unique and human-traceable. Prevents the duplicate-id collision.
- **Session naming:** include the tenant in the wallet name; never share session
  ids across tenants.
- **Keystores:** one directory per (tenant, device); never share a keystore dir
  between tenants on the same machine.

---

## 6. Action items (priority order)

1. **[blocker] Sort participants in the extension** before deriving FROST
   identifiers, to match the core's `canonical_identifier` (§2.3). Without this,
   live ext↔CLI DKG/signing is unreliable.
2. **[demo] Rehearse** a real 1-ext + 2-CLI DKG + sign once end-to-end — it has
   never run in an automated test (§2.6).
3. **[multi-tenant] Choose an isolation model** (§5.2). For investor demos,
   Option 1 (one server per cohort) is the safe default now.
4. **[polish] Align `blockchain` field** in the signing announce (§2.5).
5. **[coverage] L3c harness** (browser-driven ext↔CLI, diff vs the wire/address
   goldens) would turn items 1–2 into automated regression guards — see
   `docs/cli-conformance-testing.md` §5.3.
