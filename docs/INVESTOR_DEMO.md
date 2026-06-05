# Investor Demo Runbook — multi-device MPC wallet

Goal: in front of investors, several people on **their own devices** jointly
create a wallet (no single person ever holds the private key), sign together,
do it air-gapped, and show multi-chain addresses — **without anything blowing
up live**.

The whole strategy is three layers:

1. **Rehearse + pre-flight** — prove the stack works in seconds before anyone watches.
2. **Run the live multi-device demo** — the impressive part.
3. **Fallback ladder** — if the live run wobbles, drop one rung and keep going. The
   bottom rung cannot fail (it's self-contained crypto).

---

## 0. Golden rules (read these or it WILL bite you)

- **Every node needs a UNIQUE `--device-id`.** Two people picking the same id (or
  letting it default to the same hostname) collide on the signal server and the
  mesh silently breaks. Pre-assign names: `alice`, `bob`, `carol`, … Hand them out.
- **Run `preflight.sh` 10 minutes before.** Green = the cryptography + WebRTC +
  network path are healthy. Red = you found out in private, not on stage.
- **Decide the signal server up front** and put it on every device:
  - Live: `--signal-server wss://panda.qzz.io` (needs internet on all devices).
  - Local backup: one laptop runs the server; everyone uses `--signal-server
    ws://<that-laptop-LAN-ip>:9000` (needs same Wi-Fi, no internet). Set this up
    *before* the meeting so you can switch in 10 seconds.
- **Keep one laptop ready to run `demo-local.sh`** — the single-machine fallback.

---

## 1. Setup (each device, done beforehand)

```bash
# once per device
git clone <repo> && cd mpc-wallet
nix develop                      # or have the toolchain installed
cargo build --release -p tui-node

# launch the wallet (pick the id you were assigned!)
cargo run --release --bin mpc-wallet-tui -p tui-node -- \
  --device-id alice \
  --signal-server wss://panda.qzz.io
```

Roles for a 2-of-3 demo: **alice, bob, carol**. (2-of-3 = any two can sign; no one
alone can. Good story.)

---

## 2. Pre-flight (T-10 minutes)

On any one machine:

```bash
scripts/demo/preflight.sh
# or against the server you'll actually use:
SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh
```

It runs full DKG (2-of-2/2-of-3/3-of-5) and threshold signing **end to end** in a
few seconds each, then checks the signal server is reachable. All ✅ → go.
Any ❌ → fix it or fall back (§5). This is the single most important step — it's
the answer to "出事怎么办": you make sure there's no 出事, in private, first.

> Why it's trustworthy: `preflight` uses `mpc-wallet-cli simulate`, which spins a
> real N-node FROST ceremony (real crypto, real WebRTC over loopback, embedded
> server) in one process. It's the same code path the real clients use.

---

## 3. The live demo — 4 scenarios

### Scenario 1 — Online DKG (create a shared wallet)
1. **alice**: Create Wallet → 2-of-3 → set a name → set password → it announces a session.
2. **bob**, **carol**: a "session available" notification appears → Join → enter their own password.
3. When all 3 are in, DKG runs (a few seconds).
4. **Punchline:** all three screens show the **same wallet address**, yet **no
   device ever held the private key** — each holds only a share. That's MPC.

### Scenario 2 — Threshold signing (sign together)
1. **alice**: open the wallet → Sign a message/tx.
2. **bob**: gets a signing request → Approve (enter password).
3. A valid signature is produced from alice + bob's shares.
4. **Punchline:** carol didn't participate and wasn't needed (2-of-3). Optionally
   show that **alice alone cannot** produce a signature — the threshold is enforced
   by math, not policy.

### Scenario 3 — Offline / air-gap (SD card)
1. Switch to **Offline mode** in the TUI (air-gapped DKG/signing).
2. Each participant exports their round package to an SD card / USB.
3. Physically move the card between machines; import each round.
4. **Punchline:** the keys are generated and used with the machines **never
   connected to any network** — the cold-storage / high-security story.

### Scenario 4 — Multiple wallets + multi-chain addresses
1. Open alice's wallet detail → show the **ETH / BTC / Solana** addresses derived
   from the one share set.
2. Create a **second** wallet (different threshold, e.g. 3-of-5) to show it's not a
   one-shot.
3. **Punchline:** one MPC key set → addresses on every major chain; unlimited
   wallets.

---

## 4. What you're actually claiming (for Q&A)

- **No single point of compromise:** the private key never exists in one place —
  not on a device, not on a server, not even momentarily during signing.
- **Threshold enforced by cryptography:** k-of-n is FROST math; you can lose up to
  n−k devices and still sign, and an attacker needs k shares.
- **Works offline:** full air-gapped ceremony via removable media.
- **Multi-chain:** one share set → ETH/BTC/Solana (and more) addresses.

---

## 5. Fallback ladder (when something wobbles live)

Drop one rung at a time. Each is more reliable and less network-dependent than the
last; the bottom one is bulletproof.

| Rung | Trigger | Action |
|---|---|---|
| **0. Live** | normal | Multi-device via `wss://panda.qzz.io`. |
| **1. Local server** | internet flaky / panda unreachable | One laptop: `MPC_SIGNAL_BIND=0.0.0.0:9000 cargo run --release -p webrtc-signal-server`. Everyone restarts their TUI with `--signal-server ws://<laptop-LAN-ip>:9000`. Same demo, no internet. |
| **2. One laptop, visual** | a participant's device misbehaves | `scripts/demo/demo-local.sh` → local server + 3 TUI nodes in a tmux grid on ONE machine. Still looks multi-party. |
| **3. Nuclear (cannot fail)** | everything is on fire | `NUCLEAR=1 scripts/demo/demo-local.sh` → `mpc-wallet-cli simulate --nodes 3 --threshold 2 --sign "…"`. Full DKG + signing + verification in ~5s, self-contained, prints the group key + a verified signature. "Here is the cryptography working, right now." |

Rehearse rungs 2 and 3 so switching is muscle memory.

---

## 6. Troubleshooting (fast)

| Symptom | Cause | Fix |
|---|---|---|
| A node never joins / mesh stalls | duplicate `--device-id` | give every node a unique id; restart. |
| "Waiting for participants" forever | not everyone connected to the SAME server | confirm identical `--signal-server` on all; check internet / use rung 1. |
| Join shows nothing | joiner connected after the announce | the joiner can re-query sessions (open the Join screen); or recreate the wallet with everyone connected first. |
| Wrong/odd address shown | stale build | rebuild release; addresses are pinned by tests (ETH/BTC/SOL goldens). |
| Signing hangs after approve | cold start race (fixed) — ensure latest build | rebuild; if reproducing, drop to rung 2/3. |

---

## 7. The 30-second "出事了" decision tree

1. Did **pre-flight** pass earlier? If no — you shouldn't have started; go to rung 3.
2. Live run stalls > ~30s? → **rung 1** (local server), have everyone reconnect.
3. Still stalling, or a device is the problem? → **rung 2** (one-laptop tmux).
4. Anything still wrong? → **rung 3** (nuclear simulate). It will work. Narrate the
   crypto while you reset.

Never debug live for more than ~30s. Drop a rung, keep the story moving, fix later.
