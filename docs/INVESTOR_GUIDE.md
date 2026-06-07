# Investor Demo Guide — multi-device MPC wallet

> 中文 / Chinese: [INVESTOR_GUIDE.zh.md](./INVESTOR_GUIDE.zh.md)
> Rehearse with the companion checklist: [`DEMO_REHEARSAL_CHECKLIST.md`](./DEMO_REHEARSAL_CHECKLIST.md).

**Goal:** in front of investors, prove this is **real threshold cryptography across real,
separate machines** — several people jointly create a wallet (no single person ever holds
the private key), sign together, and have it verified by a tool that isn't ours — **without
anything blowing up live.**

There are **two ways to run the demo**; pick by audience, or do both:

- **Track A — Raw CLI, independently verifiable** (§3). The headline. Every command is
  raw and visible; the final signature is verified by the investor's **own** crypto library
  (Node.js / Python / OpenSSL) and the key is a **real Solana address**. This is the
  unfakeable version — favour it with skeptics.
- **Track B — TUI multi-device** (§4). The polished visual: people on their own laptops
  drive a terminal UI, with air-gap and multi-chain scenarios. Same cryptography, more
  showmanship, less independently verifiable.

The whole strategy is three layers: **rehearse + pre-flight** (prove it works in private
first), **run the live demo**, and a **fallback ladder** whose bottom rung cannot fail.

---

## 0. Golden rules (read these or it WILL bite you)

- **Every node needs a UNIQUE `--device-id`.** Two people picking the same id (or letting it
  default to the same hostname) collide on the signal server and the mesh silently breaks.
  Pre-assign names: `alice`, `bob`, `carol`, … Hand them out.
- **Run `preflight.sh` 10 minutes before** (§2). Green = the cryptography + WebRTC + network
  path are healthy. Red = you found out in private, not on stage.
- **One shared room, generated once.** The hosted server requires a strong `--room` (≥16
  chars). Generate it ONCE, send the exact same value to every participant, and pass it on
  every device. Different rooms can't see each other; a bare URL with no room is rejected.
  (This is also the tenant-isolation boundary — keep each cohort's room private.) The
  local-server fallback (§6 rung 1) does **not** need a room.
- **Decide the signal server up front** and put it on every device:
  - Live: `--signal-server wss://panda.qzz.io --room "$ROOM"` (needs internet).
  - Local backup: one laptop runs the server; everyone uses `--signal-server
    ws://<that-laptop-LAN-ip>:9000` (needs same Wi-Fi, no internet). Set this up *before*
    the meeting so you can switch in 10 seconds.
- **Demo on ed25519.** A FROST-ed25519 threshold signature is a *completely standard*
  RFC-8032 Ed25519 signature, so the investor can verify it with any off-the-shelf library
  (§3.3). (On secp256k1 our signature is RFC-9591, which no off-the-shelf tool checks — so
  we deliberately demo on ed25519.)
- **Never type a real password on a slide.** Use a throwaway like `demo`.
- **Keep one laptop ready for the fallback ladder** (§6).

---

## 1. Setup & prerequisites (each device, done beforehand)

```bash
# once per device
git clone <repo> && cd mpc-wallet
nix develop                      # or have the toolchain installed

# Track A (raw CLI): build the CLI
cargo build --release -p mpc-wallet-cli      # binary: ./target/release/mpc-wallet-cli
# Track B (TUI): build the TUI
cargo build --release -p tui-node
```

- **Internet access** for the live path (`wss://panda.qzz.io`). No internet? See §6 rung 1.
- **One verifier tool** on the investor's machine for Track A: **Node.js** (easiest), or
  Python with `cryptography`/`PyNaCl`, or `openssl`. All shown in §3.3.
- If you'll show the on-chain beat (§3.4), `@solana/web3.js` must be installed (`bun install`
  at repo root).

**The shared room** — one person generates it and sends the exact value to everyone:

```bash
ROOM=$(uuidgen | tr -d -)      # e.g. 7f3a9c2e4b1d4e8a9c2f001122334455
echo "$ROOM"                   # send this exact string to bob and carol
```

> The hosted server (`panda.qzz.io`) **requires a strong `--room`** (≥16 chars; see #31).
> All participants of one wallet must pass the **same** room. A bare `wss://panda.qzz.io`
> (no room) is rejected.

Roles for a 2-of-3 demo: **alice, bob, carol** — any two can sign, no one alone can. Good
story.

---

## 2. Pre-flight (T-10 minutes)

On any one machine:

```bash
scripts/demo/preflight.sh
# or against the server you'll actually use:
SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh
```

It runs full DKG (2-of-2 / 2-of-3 / 3-of-5) and threshold signing **end to end** in a few
seconds each, then checks the signal server is reachable. All ✅ → go. Any ❌ → fix it or
fall back (§6). This is the single most important step — it's the answer to "出事怎么办":
you make sure there's no 出事, in private, first.

> Why it's trustworthy: `preflight` uses `mpc-wallet-cli simulate`, which spins a real
> N-node FROST ceremony (real crypto, real WebRTC over loopback, embedded server) in one
> process. It's the same code path the real clients use.

---

## 3. Track A — Raw CLI, independently verifiable (the headline)

The strongest demo: three separate processes (ideally three laptops), each with its own
keystore file, run a real DKG + threshold signing, and a tool **the investor trusts** —
not ours — confirms the signature.

> **What it answers** — the skeptic's objections:
>
> | Investor's doubt | How the demo answers it |
> |---|---|
> | "It's one program faking multiple parties." | **Three separate processes**, each with its **own keystore file**, each printing its result independently. |
> | "Your tool says it's valid — maybe your tool lies." | Verified by **Node.js / Python / OpenSSL built-in crypto** — code we didn't write — and the key is a **real Solana address**. |
> | "Maybe one machine secretly holds the whole key." | **No machine can sign alone.** In a 2-of-3 wallet, one device signing alone just **times out** (§3.5). |
> | "It's pre-recorded / canned." | The investor **picks the message** on the spot. The signature changes; it still verifies. |
> | "The key was generated once and hard-coded." | A **fresh wallet is created live** via DKG; the key differs every run and depends on all three machines' randomness. |

> **Protocol transport.** Each node runs `mpc-wallet-cli serve`, which speaks
> **newline-delimited JSON**: you type a command object on stdin, it prints event objects on
> stdout. Investors literally see the wire protocol — nothing hidden.

### 3.1 Start the three nodes

Each person runs **one** command (substitute their name). Keep these terminals open and
visible.

```bash
# alice
./target/release/mpc-wallet-cli serve --curve ed25519 \
  --device-id alice --keystore ~/.frost_alice \
  --signal-server wss://panda.qzz.io --room "$ROOM"

# bob
./target/release/mpc-wallet-cli serve --curve ed25519 \
  --device-id bob --keystore ~/.frost_bob \
  --signal-server wss://panda.qzz.io --room "$ROOM"

# carol
./target/release/mpc-wallet-cli serve --curve ed25519 \
  --device-id carol --keystore ~/.frost_carol \
  --signal-server wss://panda.qzz.io --room "$ROOM"
```

Each prints:
```json
{"event":"ready","protocol":1,"device_id":"alice","curve":"ed25519"}
{"event":"connection","connected":true}
```

### 3.2 Create the wallet, then sign (the live ceremony)

Everything below is typed into a node's terminal (its stdin). Type the JSON and press Enter.

**alice creates a 2-of-3 wallet (distributed key generation):**
```json
{"cmd":"create_wallet","threshold":2,"total":3,"password":"demo"}
```
alice prints the session id — **read it out**:
```json
{"event":"session_announced","session_id":"dkg_8f1c…"}
```

**bob and carol join that session** (use the id alice just announced):
```json
{"cmd":"join_session","session_id":"dkg_8f1c…","password":"demo"}
```
After a few seconds, **all three** terminals independently print the same result:
```json
{"event":"dkg_complete","wallet_id":"…","address":"<Solana base58 address>","group_public_key":"<64 hex chars>"}
```

> 🎤 **Talking point:** "Three separate machines just generated a shared wallet. None holds
> the whole private key — each holds one *share*. All three print the **same** public key
> and the **same** Solana address, independently."
>
> Show the three keystore files exist and differ:
> ```bash
> ls -la ~/.frost_alice ~/.frost_bob ~/.frost_carol   # three separate shares on disk
> ```

**Sign a message the investor chooses.** Ask for a phrase; put it in **alice's** terminal:
```json
{"cmd":"sign","wallet_id":"<wallet_id from dkg_complete>","message":"we closed the round","encoding":"utf8","password":"demo"}
```
**bob** sees an approval request and consents (use the `sign_…` id bob prints):
```json
{"event":"signing_request","session_id":"sign_3a2e…","wallet":"…"}
```
```json
{"cmd":"approve_signing","session_id":"sign_3a2e…","password":"demo"}
```
alice prints the finished signature:
```json
{"event":"signature_complete","signature":"0x<128 hex chars>","message_hash":"…"}
```

> 🎤 **Talking point:** "Two of the three devices just co-signed the investor's exact
> message. Let's prove it's real — with a tool none of us wrote."

### 3.3 The money shot: independent verification

Hand the investor **three values** from the run:

- **GK** — `group_public_key` (64 hex chars) from `dkg_complete`
- **SIG** — `signature` from `signature_complete`, **drop the leading `0x`** (128 hex chars)
- **MSG** — the exact message string you signed (e.g. `we closed the round`)

The investor runs **any one** of these on **their own machine**:

**Node.js (built-in `crypto`, no install)**
```bash
node -e '
const crypto=require("crypto");
const GK="PASTE_GK", SIG="PASTE_SIG_NO_0x", MSG="we closed the round";
const der=Buffer.concat([Buffer.from("302a300506032b6570032100","hex"),Buffer.from(GK,"hex")]);
const pub=crypto.createPublicKey({key:der,format:"der",type:"spki"});
console.log("VERIFIED:", crypto.verify(null, Buffer.from(MSG), pub, Buffer.from(SIG,"hex")));
'
# → VERIFIED: true
```

**Python (`cryptography`)**
```bash
python3 -c '
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
GK="PASTE_GK"; SIG="PASTE_SIG_NO_0x"; MSG=b"we closed the round"
Ed25519PublicKey.from_public_bytes(bytes.fromhex(GK)).verify(bytes.fromhex(SIG), MSG)
print("VERIFIED: True")   # raises + prints nothing if invalid
'
```

**Python (`PyNaCl`)**
```bash
python3 -c '
from nacl.signing import VerifyKey
GK="PASTE_GK"; SIG="PASTE_SIG_NO_0x"; MSG=b"we closed the round"
VerifyKey(bytes.fromhex(GK)).verify(MSG, bytes.fromhex(SIG)); print("VERIFIED: True")
'
```

**OpenSSL**
```bash
# portable hex→binary helper (works without xxd):
hex2bin(){ python3 -c "import sys,binascii;open(sys.argv[2],'wb').write(binascii.unhexlify(sys.argv[1]))" "$1" "$2"; }

hex2bin "302a300506032b6570032100PASTE_GK" pub.der     # 12-byte SPKI prefix + key
openssl pkey -pubin -inform DER -in pub.der -out pub.pem
printf '%s' "we closed the round" > msg.bin
hex2bin "PASTE_SIG_NO_0x" sig.bin
openssl pkeyutl -verify -pubin -inkey pub.pem -rawin -in msg.bin -sigfile sig.bin
# → Signature Verified Successfully
```

**Bonus: it's a real Solana address.** The `address` from `dkg_complete` is the base58 of
that same 32-byte key — a valid Solana account. Paste it into
<https://explorer.solana.com> to show it's a real, well-formed account this wallet controls.

> 🎤 **Closing line:** "Your own crypto library — not ours — just confirmed that a signature,
> over the message *you* chose, is valid under a key that is a real Solana address, produced
> by two of three independent machines that each held only a fragment. That is threshold
> MPC, live."

### 3.4 Level up: a REAL transaction on the Solana blockchain

The strongest version: the MPC wallet signs an actual Solana transfer and it lands on-chain,
visible in a public block explorer. Division of labour (this is what makes it credible): the
**standard `@solana/web3.js` library** builds and submits the transaction; our **raw
`mpc-wallet-cli` only signs**. Helper: `scripts/demo/solana_onchain.mjs`.

> **Always derive the address from `group_public_key`** (`solana_onchain.mjs address
> <groupKeyHex>`), not from the `dkg_complete` event's `address` field — that field is
> currently unreliable for ed25519 (tracked separately).

**Pre-demo (do this BEFORE you're on stage — the live faucet is rate-limited).** Pre-fund
the address ahead of time:
```bash
node scripts/demo/solana_onchain.mjs address <groupKeyHex>     # -> the Solana address
# then fund it via the web faucet (https://faucet.solana.com, has a captcha)
# or transfer ~0.01 SOL from any funded devnet wallet. Confirm it's funded:
node scripts/demo/solana_onchain.mjs airdrop <groupKeyHex> 1   # works only if not rate-limited
```

**On stage — two ways to present (pick by whether you pre-funded):**

(i) **Fund-independent proof** (cannot be rate-limited — recommended safe default):
```bash
node scripts/demo/solana_onchain.mjs prepare <groupKeyHex> self 1000   # prints MESSAGE hex
# MPC-sign that message (2-of-3): in alice's serve terminal
#   {"cmd":"sign","wallet_id":"…","message":"<MESSAGE hex>","encoding":"hex","password":"demo"}
#   bob: {"cmd":"approve_signing","session_id":"sign_…","password":"demo"}
node scripts/demo/solana_onchain.mjs verify <signatureHex>             # -> web3.js tx.verifySignatures(): true
```
> 🎤 "The standard Solana library just confirmed our threshold signature is valid for a real
> Solana transaction — no trust in our code required."

(ii) **Full on-chain** (if the address is pre-funded): same `prepare` + MPC-sign, then
```bash
node scripts/demo/solana_onchain.mjs finalize <signatureHex>          # submits; prints the explorer URL
```
Open the printed `https://explorer.solana.com/tx/…?cluster=devnet` link on the projector.
> 🎤 "That transaction was just settled on a public blockchain — authorized by two of three
> machines that never assembled the private key."

> `prepare → sign → finalize` must complete within ~60s (blockhash lifetime), so have the
> MPC nodes already running.

### 3.5 "One device cannot sign alone" (the most visceral proof)

1. In **alice's** terminal, start a sign **but have nobody approve**:
   ```json
   {"cmd":"sign","wallet_id":"…","message":"alice alone","encoding":"utf8","password":"demo"}
   ```
2. Wait. With threshold 2 and only alice participating, the ceremony **cannot complete** — it
   times out with no signature.
3. Now repeat with bob approving → it completes.

> 🎤 "One machine, by itself, is powerless. The threshold is enforced by mathematics, not by
> policy."

---

## 4. Track B — TUI multi-device (the polished visual)

Each participant launches the terminal UI on their own laptop:

```bash
cargo run --release --bin mpc-wallet-tui -p tui-node -- \
  --device-id alice \
  --signal-server wss://panda.qzz.io \
  --room "$ROOM"
```

### Scenario 1 — Online DKG (create a shared wallet)
1. **alice**: Create Wallet → 2-of-3 → set a name → set password → it announces a session.
2. **bob**, **carol**: a "session available" notification appears → Join → enter their own password.
3. When all 3 are in, DKG runs (a few seconds).
4. **Punchline:** all three screens show the **same wallet address**, yet **no device ever
   held the private key** — each holds only a share. That's MPC.

### Scenario 2 — Threshold signing (sign together)
1. **alice**: open the wallet → Sign a message/tx.
2. **bob**: gets a signing request → Approve (enter password).
3. A valid signature is produced from alice + bob's shares.
4. **Punchline:** carol didn't participate and wasn't needed (2-of-3). Optionally show that
   **alice alone cannot** produce a signature — the threshold is enforced by math, not policy.

### Scenario 3 — Offline / air-gap (SD card)
1. Switch to **Offline mode** in the TUI (air-gapped DKG/signing).
2. Each participant exports their round package to an SD card / USB.
3. Physically move the card between machines; import each round.
4. **Punchline:** the keys are generated and used with the machines **never connected to any
   network** — the cold-storage / high-security story.

### Scenario 4 — Multiple wallets + multi-chain addresses
1. Open alice's wallet detail → show the **ETH / BTC / Solana** addresses derived from the
   one share set.
2. Create a **second** wallet (different threshold, e.g. 3-of-5) to show it's not a one-shot.
3. **Punchline:** one MPC key set → addresses on every major chain; unlimited wallets.

---

## 5. Recovery & rotation: "a lost device doesn't lose the wallet"

The question every investor asks about a multi-device wallet. The cryptographic engine for
**share refresh / resharing** is shipped and exercisable from one command — it proves the
recovery story without any network setup:

```bash
# Rotate all shares (proactive security) — same wallet, fresh shares:
mpc-wallet-cli reshare-simulate --nodes 3 --threshold 2 --curve ed25519

# Remove a lost/stolen device (2-of-3 → keep only devices 1 & 2):
mpc-wallet-cli reshare-simulate --nodes 3 --threshold 2 --curve ed25519 --keep 1,2
```

Both print the same shape:
```json
{ "kept": [1,2], "group_public_key": "06833fdf…badb6ac8",
  "key_preserved": true, "refreshed_quorum_signs": true, "old_share_rejected": true, "ok": true }
```

What to point at:
- **`group_public_key` is identical** before/after → **the address never changes**; no funds
  move, no re-funding.
- **`refreshed_quorum_signs: true`** → the wallet keeps working with the new shares.
- **`old_share_rejected: true`** → every pre-refresh share is now **dead** — a stolen
  device's fragment can no longer combine to sign.

> 🎤 "Lose a laptop? Refresh down to the survivors — same address, the wallet keeps working,
> and the lost device's share is now worthless. We can also rotate on a schedule, so an
> attacker collecting fragments over months never assembles a key. A single-key custodial
> wallet can't do any of that."

> **Scope:** `reshare-simulate` runs the real refresh **in one process** (like the nuclear
> fallback) — it proves the cryptography. The **networked** multi-device reshare ceremony
> (over the WebRTC mesh, like DKG) also ships — initiate with `mpc-wallet-cli reshare
> --wallet-id <W>` and have retained signers `session join` (or `serve --auto-approve`). See
> `docs/RECOVERY_AND_RESHARING.md` for the full threat model + talking points.

---

## 6. Fallback ladder (when something wobbles live)

Drop one rung at a time. Each is more reliable and less network-dependent than the last; the
bottom one is bulletproof. Rehearse rungs 2–3 so switching is muscle memory.

| Rung | Trigger | Action |
|---|---|---|
| **0. Live** | normal | Multi-device via `wss://panda.qzz.io` + a shared `--room`. |
| **1. Local / LAN server** | internet flaky / panda unreachable | One laptop: `MPC_SIGNAL_BIND=0.0.0.0:9000 cargo run --release -p webrtc-signal-server`. Everyone restarts with `--signal-server ws://<laptop-LAN-ip>:9000` (a local server needs **no** room). Same demo, no internet. |
| **2. One laptop, visual** (TUI) | a participant's device misbehaves | `scripts/demo/demo-local.sh` → local server + 3 TUI nodes in a tmux grid on ONE machine. Still looks multi-party. |
| **3. Nuclear (cannot fail)** | everything is on fire | `./target/release/mpc-wallet-cli simulate --nodes 3 --threshold 2 --curve ed25519 --sign "we closed the round"` → full DKG + signing + a verifiable signature in ~3s, self-contained. Prints the group key + a verified signature; then verify with §3.3. "Here is the cryptography working, right now." |

---

## 7. Troubleshooting (fast)

| Symptom | Cause / fix |
|---|---|
| `WebSocket … 400` / connection rejected | Missing or weak `--room` (hosted server requires ≥16 chars). Generate with `uuidgen \| tr -d -`; everyone uses the **same** value. |
| A node never joins / "Waiting for participants" forever | Duplicate `--device-id`, OR not everyone on the **same room + same server**. Give every node a unique id; confirm identical `--signal-server` and `--room`; check internet / use rung 1. |
| bob/carol never see the session | They connected after alice announced — they just send `join_session` with the id alice printed (late joins work). |
| Verifier says **false** | Wrong `MSG` bytes (must be the **exact** message signed), `0x` not stripped from `SIG`, or a GK/SIG transcription typo. Re-copy. |
| Signature verifies but address looks odd | `address` is base58 (Solana); `group_public_key` is hex — same key, two encodings. |
| Wrong/odd address in the TUI | stale build; rebuild release (addresses are pinned by ETH/BTC/SOL golden tests). |
| Signing hangs after approve | cold-start race (fixed) — rebuild to latest; if reproducing, drop to rung 2/3. |

---

## 8. The 30-second "出事了" decision tree

1. Did **pre-flight** pass earlier? If no — you shouldn't have started; go to rung 3.
2. Live run stalls > ~30s? → **rung 1** (local/LAN server), have everyone reconnect.
3. Still stalling, or a device is the problem? → **rung 2** (one-laptop TUI) or **rung 3**.
4. Anything still wrong? → **rung 3** (nuclear simulate). It will work. Narrate the crypto
   while you reset.

Never debug live for more than ~30s. Drop a rung, keep the story moving, fix later.

---

## 9. What you're claiming — and what's under the hood (Q&A)

**The claims:**
- **No single point of compromise:** the private key never exists in one place — not on a
  device, not on a server, not even momentarily during signing.
- **Threshold enforced by cryptography:** k-of-n is FROST math; you can lose up to n−k
  devices and still sign, and an attacker needs k shares.
- **Works offline:** full air-gapped ceremony via removable media.
- **Multi-chain:** one share set → ETH/BTC/Solana (and more) addresses.

**Under the hood (for the technical investor):**
- **DKG (key generation):** FROST distributed key generation, Pedersen variant — **no
  trusted dealer**. Each device contributes randomness; the private key is never assembled
  anywhere. Each device ends with a *share*; the group public key is public.
- **Signing:** FROST threshold Schnorr. `t` of `n` devices each produce a partial signature;
  these aggregate into **one ordinary signature** that verifies under the group key with a
  standard verifier. No device ever sees another's share.
- **Curve:** ed25519 (RFC 8032) — the group key is a normal Ed25519 public key (a Solana
  address) and the signature is a normal Ed25519 signature, hence the independent
  verification in §3.3. The same software also runs secp256k1 (Ethereum/Bitcoin family); we
  demo ed25519 specifically because it's verifiable with off-the-shelf tools.
- **Transport:** a signal server for discovery + WebRTC for the encrypted peer-to-peer mesh
  the shares travel over. The signal server never sees key material.
- **Recovery / custody:** because there's no dealer, the encrypted keystore per device is the
  backup unit. (See `docs/MULTI_CURVE_DERIVATION.md`.)

---

## 10. Quick reference card (print this)

```
SHARED ROOM:  ROOM=$(uuidgen | tr -d -)        # same value on every device
SERVER:       wss://panda.qzz.io               # or LAN: ws://<laptop-ip>:9000 (no room)
ROLES:        alice / bob / carol   (2-of-3)   # UNIQUE --device-id each
PASSWORD:     demo                             # throwaway, never a real one

PRE-FLIGHT:   SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh     # all ✅ → go

— Track A (raw CLI) —
START (each):  mpc-wallet-cli serve --curve ed25519 --device-id <name> \
                 --keystore ~/.frost_<name> --signal-server wss://panda.qzz.io --room "$ROOM"
alice:         {"cmd":"create_wallet","threshold":2,"total":3,"password":"demo"}
bob,carol:     {"cmd":"join_session","session_id":"<dkg_…>","password":"demo"}
alice:         {"cmd":"sign","wallet_id":"<…>","message":"<investor's words>","encoding":"utf8","password":"demo"}
bob:           {"cmd":"approve_signing","session_id":"<sign_…>","password":"demo"}
VERIFY:        node -e '…'   # GK + SIG(no 0x) + MSG → VERIFIED: true

FALLBACK:      0 live → 1 LAN server → 2 one-laptop TUI → 3 nuclear simulate
NUCLEAR:       mpc-wallet-cli simulate --nodes 3 --threshold 2 --curve ed25519 --sign "we closed the round"
```
