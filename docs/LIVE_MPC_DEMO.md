# Live MPC Demo — Raw CLI, Independently Verifiable

**Audience:** the person running the demo, and the (skeptical) investors watching.
**Goal:** prove, in front of a sceptic, that this is **real threshold cryptography across
real, separate machines** — not a script pretending. Every command is raw and visible;
the final signature is verified by a **tool that is not ours** (Node.js / Python / OpenSSL),
and the public key is a **real Solana address**.

> **The one idea that makes this unfakeable:** we run the ceremony on the **ed25519**
> curve. A FROST‑ed25519 threshold signature is a *completely standard* RFC‑8032
> Ed25519 signature. So the investor can verify it with their own laptop's crypto
> library, with zero knowledge of our code. If a universally‑trusted library says
> "valid", it's valid. (On secp256k1 our signature is RFC‑9591, which no off‑the‑shelf
> tool checks — so we deliberately demo on ed25519.)

---

## 0. What this demo proves (and the sceptic's objections it answers)

| Investor's doubt | How the demo answers it |
|---|---|
| "It's one program faking multiple parties." | **Three separate processes** (ideally three laptops), each with its **own keystore file**, each printing its result independently. |
| "Your tool says it's valid — maybe your tool lies." | The signature is verified by **Node.js / Python / OpenSSL built‑in crypto** — code we didn't write — and the key is a **real Solana address**. |
| "Maybe one machine secretly holds the whole key." | **No machine can sign alone.** With a 2‑of‑3 wallet, one device trying to sign just **times out**. Two must cooperate. (Live proof in §6.) |
| "It's pre‑recorded / canned." | The investor **picks the message** to sign, on the spot. The signature changes; it still verifies. |
| "The key was generated once and hard‑coded." | A **fresh wallet is created live** via distributed key generation (DKG); the key is different every run and depends on all three machines' randomness. |

---

## 1. Roles & prerequisites

- **Three participants** — call them **alice**, **bob**, **carol**. Ideally three
  separate laptops; three terminals on one machine also works (less convincing, same crypto).
- Each machine needs the `mpc-wallet-cli` binary:
  ```bash
  cargo build --release -p mpc-wallet-cli
  # binary: ./target/release/mpc-wallet-cli
  ```
- Internet access (the demo uses the hosted signal server `wss://panda.qzz.io`).
  - *No internet?* See §7 — one laptop runs the server on the LAN.
- One verifier tool on the investor's machine: **Node.js** (easiest), or Python with
  `cryptography`/`PyNaCl`, or `openssl`. All shown in §5.

> **Protocol transport.** Each node runs `mpc-wallet-cli serve`, which speaks **newline‑
> delimited JSON**: you type a command object on stdin, it prints event objects on stdout.
> Investors literally see the wire protocol — nothing hidden.

---

## 2. One‑time setup: the shared room

The hosted server is multi‑tenant. Every participant of one wallet must use the **same
strong room id** (≥16 chars). One person generates it and shares the exact value:

```bash
ROOM=$(uuidgen | tr -d -)      # e.g. 7f3a9c2e4b1d4e8a9c2f001122334455
echo "$ROOM"                   # send this exact string to bob and carol
```

Pick a shared password for the demo too (e.g. `demo`). In a real deployment each device
has its own; for the demo a shared one keeps it simple. **Never type a real password on a
slide.**

---

## 3. Start the three nodes

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

> **Device ids must be unique.** Two nodes with the same `--device-id` collide on the
> server and the mesh won't form. alice / bob / carol — all different.

---

## 4. Create the wallet, then sign (the live ceremony)

Everything below is typed into a node's terminal (its stdin). Type the JSON and press Enter.

### 4a. alice creates a 2‑of‑3 wallet (distributed key generation)

In **alice's** terminal:
```json
{"cmd":"create_wallet","threshold":2,"total":3,"password":"demo"}
```

alice immediately prints the session id — **read it out**:
```json
{"event":"session_announced","session_id":"dkg_8f1c…"}
```

### 4b. bob and carol join that session

In **bob's** and **carol's** terminals (use the id alice just announced):
```json
{"cmd":"join_session","session_id":"dkg_8f1c…","password":"demo"}
```

After a few seconds, **all three** terminals independently print the same result:
```json
{"event":"dkg_complete","wallet_id":"…","address":"<Solana base58 address>","group_public_key":"<64 hex chars>"}
```

> 🎤 **Talking point:** "Three separate machines just generated a shared wallet. None of
> them holds the whole private key — each holds one *share*. Notice all three print the
> **same** public key and the **same** Solana address, independently."
>
> Show the investor the three keystore files exist and differ:
> ```bash
> ls -la ~/.frost_alice ~/.frost_bob ~/.frost_carol   # three separate shares on disk
> ```

### 4c. Sign a message the investor chooses

Ask the investor for a phrase. Put it in **alice's** terminal (`message` = their words):
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

> 🎤 **Talking point:** "Two of the three devices just co‑signed the investor's exact
> message. Let's prove it's real — with a tool none of us wrote."

---

## 5. The money shot: independent verification

Hand the investor **three values** from the run:

- **GK** — `group_public_key` (64 hex chars) from `dkg_complete`
- **SIG** — `signature` from `signature_complete`, **drop the leading `0x`** (128 hex chars)
- **MSG** — the exact message string you signed (e.g. `we closed the round`)

The investor runs **any one** of these on **their own machine**:

### Node.js (built‑in `crypto`, no install)
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

### Python (`cryptography`)
```bash
python3 -c '
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
GK="PASTE_GK"; SIG="PASTE_SIG_NO_0x"; MSG=b"we closed the round"
Ed25519PublicKey.from_public_bytes(bytes.fromhex(GK)).verify(bytes.fromhex(SIG), MSG)
print("VERIFIED: True")   # raises + prints nothing if invalid
'
```

### Python (`PyNaCl`)
```bash
python3 -c '
from nacl.signing import VerifyKey
GK="PASTE_GK"; SIG="PASTE_SIG_NO_0x"; MSG=b"we closed the round"
VerifyKey(bytes.fromhex(GK)).verify(MSG, bytes.fromhex(SIG)); print("VERIFIED: True")
'
```

### OpenSSL
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

### Bonus: it's a real Solana address
The `address` from `dkg_complete` is the base58 of that same 32‑byte key — a valid Solana
account. Paste it into <https://explorer.solana.com> to show it's a real, well‑formed
account this wallet controls.

> 🎤 **Closing line:** "Your own crypto library — not ours — just confirmed that a
> signature, over the message *you* chose, is valid under a key that is a real Solana
> address, produced by two of three independent machines that each held only a fragment.
> That is threshold MPC, live."

---

## 6. Optional but powerful: "one device cannot sign alone"

This is the most visceral proof that the key is genuinely split.

1. In **alice's** terminal, start a sign **but have nobody approve**:
   ```json
   {"cmd":"sign","wallet_id":"…","message":"alice alone","encoding":"utf8","password":"demo"}
   ```
2. Wait. With threshold 2 and only alice participating, the ceremony **cannot complete**
   — it times out with no signature.
3. Now repeat with bob approving → it completes.

> 🎤 "One machine, by itself, is powerless. The threshold is enforced by mathematics, not
> by policy."

---

## 7. Fallback ladder (if the network misbehaves)

Have these ready; you should never be stuck.

| Rung | When | What to do |
|---|---|---|
| **0. Live** | normal | `wss://panda.qzz.io` + a shared `--room` (the steps above). |
| **1. LAN server** | internet flaky | One laptop runs the server: `MPC_SIGNAL_BIND=0.0.0.0:9000 cargo run --release -p webrtc-signal-server`. Everyone uses `--signal-server ws://<that-laptop-LAN-ip>:9000` (a local server needs **no** room). Same demo, no internet. |
| **2. Nuclear (cannot fail)** | everything is on fire | On one machine: `./target/release/mpc-wallet-cli simulate --nodes 3 --threshold 2 --curve ed25519 --sign "we closed the round"`. Full DKG + signing + a verifiable signature in ~3s, self‑contained. Then verify with §5. Less impressive (one machine) but the **crypto is identical and still independently verifiable**. |

**Pre‑flight (run 10 minutes before, on each machine):**
```bash
SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh
# proves the local crypto stack AND a real ceremony through the live server. Green = go.
```

---

## 8. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `WebSocket … 400` / connection rejected | Missing or weak `--room` (the hosted server requires ≥16 chars). Generate with `uuidgen \| tr -d -`; everyone uses the **same** value. |
| "Waiting for participants" forever | Not everyone is on the **same room** and **same server**; or duplicate `--device-id`. Check all three. |
| bob/carol never see the session | They connected after alice announced — they just need to send `join_session` with the id alice printed. (Joins also work if they connect late.) |
| Verifier says **false** | Wrong `MSG` bytes (must be the **exact** message signed), or the `0x` wasn't stripped from `SIG`, or GK/SIG were transcribed with a typo. Re‑copy. |
| Signature verifies but address looks odd | The `address` is base58 (Solana); the `group_public_key` is hex. They're the same key in two encodings. |

---

## 9. Under the hood (for the technical investor who asks)

- **DKG (key generation):** FROST distributed key generation, Pedersen variant — **no
  trusted dealer**. Each device contributes randomness; the private key is never
  assembled anywhere. Each device ends with a *share*; the group public key is public.
- **Signing:** FROST threshold Schnorr. `t` of `n` devices each produce a partial
  signature; these aggregate into **one ordinary signature** that verifies under the group
  key with a standard verifier. No device ever sees another's share.
- **Curve:** ed25519 (RFC 8032). The group key is a normal Ed25519 public key (a Solana
  address); the signature is a normal Ed25519 signature — hence the independent
  verification in §5. The same software also runs secp256k1 (Ethereum/Bitcoin family); we
  demo ed25519 specifically because it is verifiable with off‑the‑shelf tools.
- **Transport:** a signal server for discovery + WebRTC for the encrypted peer‑to‑peer
  mesh the shares travel over. The signal server never sees key material.
- **Recovery / custody note:** because there's no dealer, the seed of one device alone
  cannot reconstruct its share — the encrypted keystore per device is the backup unit.
  (See `docs/MULTI_CURVE_DERIVATION.md`.)

---

## 10. Quick reference card (print this)

```
SETUP   ROOM=$(uuidgen | tr -d -)               # share this exact value
NODE    mpc-wallet-cli serve --curve ed25519 --device-id <name> \
        --keystore ~/.frost_<name> --signal-server wss://panda.qzz.io --room "$ROOM"
CREATE  (alice) {"cmd":"create_wallet","threshold":2,"total":3,"password":"demo"}
JOIN    (bob,carol) {"cmd":"join_session","session_id":"dkg_…","password":"demo"}
SIGN    (alice) {"cmd":"sign","wallet_id":"…","message":"<investor's words>","encoding":"utf8","password":"demo"}
APPROVE (bob)   {"cmd":"approve_signing","session_id":"sign_…","password":"demo"}
VERIFY  node -e '…'   # §5 — investor runs it; → VERIFIED: true
```
