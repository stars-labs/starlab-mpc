# MPC Wallet — User Guide (four UIs, multi-device, multi-tenant, multi-chain)

This guide explains how to use all **four clients** to create threshold wallets
across multiple devices and tenants, sign with them, and use the supported
chains.

- **CLI** — `mpc-wallet-cli` (headless, scriptable; LLM/automation/servers)
- **TUI** — `mpc-wallet-tui` (terminal UI; online + offline/air-gap)
- **Native** — desktop GUI (Slint)
- **Extension** — Chrome/Firefox browser wallet (dApp provider + popup)

All four share the same FROST cryptography and the same wire protocol, so **any
mix of them can participate in one ceremony** (e.g. 1 extension + 2 CLI).

---

## 1. Core concepts (read first)

- **Threshold wallet (k-of-n):** the private key is split into `n` shares (one
  per participant/device); any `k` of them can sign, fewer cannot. The full
  private key is **never** assembled anywhere — not on a device, not on a server,
  not even while signing.
- **DKG (Distributed Key Generation):** the one-time ceremony that creates the
  wallet. **All `n` participants must be online together** for DKG. Afterward,
  only `k` are needed to sign.
- **Device id:** a name identifying each participant. **Must be unique within a
  room** — two participants with the same id collide on the signal server and the
  mesh silently breaks. Use a scheme like `alice` / `cli-a` / `acme-signer-1`.
- **Room (tenant):** the hosted signal server is multi-tenant; every connection
  carries a `?room=<id>` and each room is fully isolated. **The room is
  mandatory and must be strong** (≥16 chars; use a UUID). All participants of one
  wallet must use the **same** room. (Details in §4.)
- **Curve & chain:** a wallet is created on **one curve**, which determines which
  chains it can address (§3). One wallet → addresses on every chain of its curve.

---

## 2. The four UIs at a glance

| | CLI | TUI | Native | Extension |
|---|---|---|---|---|
| Form | JSONL / one-shot commands | terminal UI | desktop window | browser popup + dApp |
| Best for | automation, servers, scripts | power users, **offline/air-gap** | desktop users | everyday web3 / dApps |
| Curves | secp256k1 + ed25519 | secp256k1 + ed25519 | secp256k1 + ed25519 (`MPC_CURVE`, one per launch) | secp256k1 + ed25519 |
| Signal server | `--signal-server` | `--signal-server` | `MPC_SIGNAL_SERVER` env | Settings ⚙ |
| Room | `--room` | `--room` | `MPC_ROOM` env | Settings ⚙ ("Signal server room") |
| Device id | `--device-id` | `--device-id` (default: hostname) | `MPC_DEVICE_ID` env | auto |
| Keystore | `--keystore` | `~/.frost_keystore` | `~/.frost_keystore` | browser storage |

> Native is configured via env vars and runs **one curve per launch**
> (`MPC_CURVE=secp256k1` default, or `ed25519`) — like the CLI `serve`. Launch it
> again with the other curve to use the other chain family.

---

## 3. Curves and chains

A wallet is created on one curve. That curve fixes the set of chains it can use:

| Curve | Chains | Address style |
|---|---|---|
| **secp256k1** | Ethereum, BSC, Polygon, Avalanche (+ EVM), **Bitcoin** | `0x…` (EVM) / `bc1…` (BTC P2WPKH) |
| **ed25519** | Solana, Sui, Aptos, NEAR | base58 (Solana) / chain-specific |

One secp256k1 wallet gives you the **same** signing key across all EVM chains
plus a Bitcoin address; one ed25519 wallet covers Solana/Sui/Aptos/NEAR. To use
both families, create **two** wallets (one per curve).

You pick the curve indirectly by choosing the **chain/blockchain** when creating
the wallet (e.g. choosing "ethereum" ⇒ secp256k1; "solana" ⇒ ed25519).

---

## 4. The room model (multi-tenant) — set this up first

The hosted signal server (`wss://panda.qzz.io`) routes each `?room=<id>` to an
isolated instance. Rules:

1. **A room is required and must be strong** (≥16 chars of `[A-Za-z0-9_-]`). A
   missing/weak room is rejected (you'll see the client stuck "Offline" / a
   `400` in logs).
2. **Generate one strong room id and share it** with every participant of the
   wallet — out of band (chat, etc.). It behaves like a meeting link.
   ```bash
   uuidgen        # e.g. 7f3a9c2e-4b1d-4e8a-9c2f-001122334455
   ```
3. **Different rooms are fully isolated** — separate participants, sessions, and
   relays. This is the tenant boundary: give each customer/cohort its own room.
4. A **local standalone server** (for dev/air-gapped LAN) needs **no** room.

> Multiple wallets can share a room, but for clean isolation use **one room per
> tenant** (and optionally per wallet). Never reuse another tenant's room id.

---

## 5. Per-UI setup

### CLI
```bash
cargo build --release -p mpc-wallet-cli      # binary: target/release/mpc-wallet-cli
# common flags: --device-id --keystore --signal-server --room
#   passwords: --password-file <f>  (preferred)  | --password-env VAR | --password <p>
```

### TUI
```bash
cargo run --release --bin mpc-wallet-tui -p tui-node -- \
  --device-id alice --signal-server wss://panda.qzz.io --room "$ROOM"
# add --offline for air-gapped (SD-card) mode (no network)
```

### Native (desktop)
```bash
MPC_DEVICE_ID=alice MPC_ROOM="$ROOM" \
  MPC_SIGNAL_SERVER=wss://panda.qzz.io \
  MPC_CURVE=secp256k1 \                            # or ed25519 for Solana/Sui/Aptos/NEAR
  cargo run --release -p mpc-wallet-native        # needs `nix develop` for graphics
```

### Extension
1. Build: from repo root `bun run build:wasm`, then `cd apps/browser-extension && bun run build`.
2. `chrome://extensions` → Developer mode → **Load unpacked** → `apps/browser-extension/.output/chrome-mv3`.
3. Open the popup → **gear ⚙** → **"Signal server room"** → **Generate** (or paste the shared room) → **Save**.
4. Close & reopen the popup → the badge should go **Connected** (it stays Offline / shows a `400` until a room is set).

---

## 6. Creating a multi-device wallet (DKG)

Decide up front: **curve/chain**, **threshold k**, **total n**, the **shared
room**, and a **unique device id per participant**. Example: a **2-of-3 Ethereum**
wallet shared by `alice`, `bob`, `carol`.

> All `n` participants must be online (same room) for DKG. One **creates**, the
> others **join**.

### CLI (creator + joiner)
Creator (`alice`):
```bash
mpc-wallet-cli serve --device-id alice --keystore /tmp/ks-alice \
  --signal-server wss://panda.qzz.io --room "$ROOM"
# then on its stdin (JSONL):
{"cmd":"connect"}
{"id":1,"cmd":"create_wallet","name":"team","threshold":2,"total":3,"curve":"secp256k1","password":"…"}
# → emits {"event":"session_announced","session_id":"dkg_…"} — share that id
```
Joiners (`bob`, `carol`):
```bash
mpc-wallet-cli serve --device-id bob --keystore /tmp/ks-bob \
  --signal-server wss://panda.qzz.io --room "$ROOM"
{"cmd":"connect"}
{"cmd":"list_sessions"}                 # discovers the announced session
{"id":2,"cmd":"join_session","session_id":"dkg_…","password":"…"}
```
One-shot equivalents also exist: `mpc-wallet-cli wallet create …` (creator,
blocks until done) and `mpc-wallet-cli session join …`.

When all three have joined, DKG runs (seconds). Every participant ends with the
**same group address** — verify they match.

### TUI
1. Each participant launches the TUI with a **unique `--device-id`**, the **same
   `--signal-server` + `--room`**.
2. Creator: **Create Wallet** → choose chain (→ curve), threshold, total, set a
   password → it announces a session.
3. Joiners: a session appears under **Join Session** → select it → enter their
   password.
4. When the group is complete, DKG runs; all screens show the same address.

### Native
Launch with `MPC_DEVICE_ID`/`MPC_ROOM`/`MPC_SIGNAL_SERVER`/`MPC_CURVE` set; use
the window's Create/Join controls. The launch `MPC_CURVE` fixes the wallet's
curve (secp256k1 ⇒ EVM/BTC, ed25519 ⇒ Solana-family).

### Extension
1. Set the room (§5). 2. **Create wallet** (popup) → pick threshold/total → set a
password. 3. Co-signers (any client) join the announced session; the extension
also shows incoming sessions to join.

---

## 7. Signing

Signing needs **k** participants (not all n). One initiates; `k−1` others
approve. The result is a single signature that verifies against the group key.

### CLI
Initiator:
```bash
{"id":10,"cmd":"sign","wallet_id":"wallet-dkg_…","message":"hello","encoding":"utf8","password":"…"}
#   encoding: "utf8" (default) or "hex"
```
Approver (a co-signer):
```bash
# it receives {"event":"signing_request","session_id":"sign_…",…}
{"cmd":"approve_signing","session_id":"sign_…","password":"…"}
```
The initiator emits `{"event":"signature_complete","signature":"0x…"}`.
Or one-shot: `mpc-wallet-cli sign --wallet-id … --message … --room … …`.
An auto-approving co-signer (gated by an allowlist/budget) can run
`serve --auto-approve --approve-wallet <id> --approve-password-file <f>`.

### TUI
Open the wallet → **Sign** a message/tx → a co-signer gets a signing request →
**Approve** with their password → signature appears.

### Extension
- **From the popup:** open the wallet → **Sign Message** → confirm.
- **From a dApp:** the extension injects an EIP-1193 provider at
  `window.starlabEthereum` (discovered via EIP-6963; it never overrides
  `window.ethereum`). A dApp calls
  `window.starlabEthereum.request({method:"personal_sign", …})`; the extension
  prompts you to approve, runs the threshold ceremony with the co-signers, and
  returns the signature.
- Co-signers (any client) approve their part for the ceremony to complete.

> secp256k1 signs an EIP-191 hash of the message; ed25519 signs the raw bytes.
> Either way the produced signature verifies against the wallet's group key.

---

## 8. Using the chains

After DKG, one wallet exposes addresses for **every chain of its curve**:

- **secp256k1 wallet:** the same `0x…` address on Ethereum/BSC/Polygon/Avalanche,
  plus a Bitcoin `bc1…` (P2WPKH) address. View them in the TUI wallet detail, the
  extension account view, or via the CLI (`list_wallets` reports the primary
  address; multi-chain addresses derive from the one group key).
- **ed25519 wallet:** Solana (base58) + Sui/Aptos/NEAR addresses.

To transact on a specific chain you sign the chain's payload (e.g. an Ethereum
tx, a Solana message) through the signing flow in §7; broadcasting to the chain's
RPC is done by your dApp/tooling (this wallet produces the signature).

To cover both curve families, create two wallets (one secp256k1, one ed25519).

---

## 9. Multi-tenant operation

Each tenant (customer, cohort, team) gets **its own room**:

- Generate a distinct strong room id per tenant; distribute it only to that
  tenant's participants.
- Device-id convention within a tenant: `<tenant>-<role>-<n>` (e.g.
  `acme-signer-1`) — globally unique and traceable.
- Keystores: one directory per `(tenant, device)`; never share across tenants.
- Isolation is enforced by the server (different room ⇒ different Durable Object
  instance ⇒ no cross-tenant visibility). Even if someone learns a room id and
  joins, they cannot steal keys — only enrolled participants hold usable shares.
- For the strongest isolation / hosting, see `docs/MULTI_TENANT.md` (one
  Durable-Object-per-room is the deployed model; auth-derived rooms are the
  roadmap for adversarial tenants).

---

## 10. End-to-end example: 1 extension + 2 CLI (2-of-3)

A worked, step-by-step version (with the known caveats — e.g. rehearse the live
extension↔CLI run) lives in **`docs/INTEROP_EXT_CLI.md`**. Summary:

1. `ROOM=$(uuidgen)` — share it with all three.
2. Two CLI nodes: `serve --device-id cli-a/cli-b --signal-server wss://panda.qzz.io --room "$ROOM"`, each `{"cmd":"connect"}`.
3. Extension: ⚙ → set the same room → it connects.
4. One creates a 2-of-3 wallet; the others join; DKG completes → same address everywhere.
5. Any two sign (CLI `sign`+`approve_signing`, or the extension's approve).

---

## 11. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Client stays **Offline**; `400` in logs | no room / weak room on the hosted server | set a strong `--room` / `MPC_ROOM` / Settings room (≥16 chars) |
| A participant never joins; mesh stalls | duplicate `--device-id` in the room | give every participant a unique id; restart |
| Joiner sees no session | joined after the announce | run `list_sessions` (CLI) / reopen Join screen (TUI); or recreate with everyone connected first |
| Extension keeps asking to unlock | (fixed) it no longer auto-prompts | unlock on demand via the header 🔓 button |
| Stale session/keys in the extension | leftover dev state | Service Worker console → `chrome.storage.local.clear()` → reload |
| Wrong/odd address | stale build | rebuild (addresses are pinned by tests: ETH=G→`0x7e5f4552…`, BTC=G→`bc1qw508d6…`) |
| Want both EVM and Solana | one wallet = one curve | create two wallets (secp256k1 + ed25519) |

---

## 12. Quick reference

```bash
ROOM=$(uuidgen)                         # one shared strong room per wallet/tenant

# CLI
mpc-wallet-cli serve   --device-id <id> --keystore <dir> --signal-server wss://panda.qzz.io --room "$ROOM"
mpc-wallet-cli wallet  create --name w --threshold 2 --total 3 --curve secp256k1 --room "$ROOM" --device-id <id> --password-file <f>
mpc-wallet-cli sign    --wallet-id <id> --message hi --room "$ROOM" --device-id <id> --password-file <f>

# Self-contained end-to-end smoke test (real multi-process DKG + signing over a local server):
scripts/demo/ceremony.sh --nodes 3 --threshold 2 --sign hi   # add --signal wss://… --room "$ROOM" to test a hosted server

# TUI
mpc-wallet-tui --device-id <id> --signal-server wss://panda.qzz.io --room "$ROOM" [--offline]

# Native (MPC_CURVE=secp256k1|ed25519, one curve per launch)
MPC_DEVICE_ID=<id> MPC_ROOM="$ROOM" MPC_SIGNAL_SERVER=wss://panda.qzz.io MPC_CURVE=secp256k1 cargo run --release -p mpc-wallet-native

# Extension: ⚙ Settings → Signal server room → Generate/paste "$ROOM" → Save → reopen popup
```

Related docs: `docs/MULTI_TENANT.md` (isolation), `docs/INTEROP_EXT_CLI.md`
(ext↔CLI interop + caveats), `docs/INVESTOR_GUIDE.md` (demo guide + fallbacks).
