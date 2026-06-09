# Multi-Curve Key Derivation & Recovery Model

**Status:** Authoritative design note (matches shipped code as of this commit)
**Scope:** how one root secret yields per-curve threshold wallets, and — critically — **what can and cannot be recovered**, and from what.
**Code:** `packages/@starlab/core/src/{root_secret.rs, unified_dkg.rs, curve_registry.rs, keystore.rs}`

---

## TL;DR (read this first)

1. **One seed → many keys, not one key across curves.** A 32-byte `RootSecret`
   deterministically seeds DKG randomness *per curve* and *per account*. ed25519
   and secp256k1 live in different scalar fields, so they are genuinely
   **separate keys** — there is no single key shared across curves.
2. **The seed alone does NOT recover your share.** In our dealerless (Pedersen)
   DKG the final signing share is a function of **every** participant's
   contributions, not just your local randomness. Backing up the 32-byte seed is
   therefore **not** a complete recovery story.
3. **The practical backup is the encrypted keystore.** `keystore.rs`
   (PBKDF2/Argon2 + AES-256-GCM) holds your finalized share. That file + its
   password is what actually restores a node.

If you remember nothing else: **back up the keystore file, not the seed.**

---

## 1. What the root secret is (and is not)

`RootSecret` is 32 bytes of CSPRNG entropy (`RootSecret::generate`). It seeds
**this node's** DKG randomness through a versioned, domain-separated HKDF:

```text
info := "frost-dkg" "/" VERSION "/" CURVE "/" ACCOUNT      ; e.g. frost-dkg/v1/secp256k1/0
seed  = HKDF-SHA256(ikm = root, salt = ∅, info)            ; 32 bytes
rng   = ChaCha20Rng::from_seed(seed)                       ; fed into frost dkg::part1
```

(See `root_secret.rs` for the grammar, the version constant, and the byte-locked
regression tests. The domain-separation rules are documented there in full.)

So the root seed governs the **per-node, per-curve, per-account randomness that
goes into round 1** of the DKG. That is *all* it governs. It does **not** encode
the other participants' contributions, which is exactly why it can't, by itself,
reproduce your finalized share — see §3.

### One seed, many keys (the framing that matters)

| | |
|---|---|
| Same root, different **curve** (`ed25519` vs `secp256k1`) | different scalar field → **independent keys**; one cannot be derived from the other |
| Same root, different **account** (`…/0` vs `…/1`) | domain-separated → **independent wallets** on the same curve |
| Same root, same `(curve, account)`, **fresh DKG run** | different **group key** each run (DKG randomness from *all* nodes differs run-to-run) |

This is *derivation convenience* (one entropy source to manage), **not** key
unification. There is no master key from which all curves' keys descend in a way
that would let one curve's compromise leak another.

> **Aside — HD child keys are different.** `hd_derivation.rs` derives BIP-44-style
> *child* keys from an already-finalized share via additive scalar offsets (no
> extra DKG). That is a separate mechanism layered *on top of* a finished wallet;
> it does not change the recovery model below.

---

## 2. How a wallet is actually produced (dealerless DKG)

`unified_dkg.rs` / `curve_registry.rs` run a standard FROST Pedersen DKG — **no
trusted dealer**:

```
part1(seed_i)            → round-1 secret_i (local)      + round-1 package_i (broadcast)
part2(secret_i, {pkg_j}) → round-2 secret_i (local)      + round-2 packages_i→j (point-to-point)
part3(secret_i, …)       → key_package_i  (YOUR SHARE)   + public_key_package (GROUP KEY)
```

The finalized `key_package_i` (your share) and the `group_public_key` are
computed from **the full transcript**: your round-1/round-2 secrets *and* every
other participant's round-1 and round-2 packages. Change any participant's
contribution and your share changes.

**Consequence:** `share_i ≠ f(root_i)`. It is `f(root_i, transcript_from_everyone)`.
Your local seed is one input among `n`.

---

## 3. Recovery matrix — what restores what

| You still have… | Recovers your share? | Recovers the group key? | Notes |
|---|---|:--:|---|
| **Root seed only** | ❌ | ❌ | Seeds only *your* round-1 randomness; the share depends on all `n` transcripts. A seed backup is **not** a wallet backup. |
| **Root seed + full DKG transcript** | ✅ (recomputable) | ✅ | If every round-1/round-2 package from that ceremony is persisted, `part2`/`part3` are deterministic given your seed → share is reproducible. Requires storing the whole transcript; we do not do this by default. |
| **Encrypted keystore + password** | ✅ | ✅ | The **shipped, recommended** path. `keystore.rs` stores the finalized share encrypted (PBKDF2/Argon2 + AES-256-GCM). Restore = decrypt. |
| **Threshold quorum of *other* holders** | ✅ (re-issue) | ✅ (unchanged) | A `t`-of-`n` quorum can **reshare** to a replacement device. No secret is ever reconstructed in one place; the group key is preserved. (Resharing protocol — see "Open items".) |
| **Nothing (lost keystore, sub-threshold devices)** | ❌ | — | Below threshold and without a keystore/transcript, funds are unrecoverable. This is the security property, not a bug. |

### Why "seed-only" is the dangerous misconception

A user who believes "I wrote down my 32-byte seed, I'm safe" is **wrong** under
dealerless DKG. They have backed up one of `n` round-1 inputs. Restoring requires
either the keystore, the full transcript, or a surviving quorum. Communicate this
explicitly in any backup UX.

### Password caveat

The keystore password is **unrecoverable** by design — there is no reset. Lose
the password and the keystore is opaque ciphertext. So the real backup unit is
**(keystore file, password)**, stored such that losing either does not lose both.

---

## 4. Threshold geometry of recovery

- **`t`-of-`n`**: any `t` shares can sign; any `t` *holders* can reshare to a new
  device. Losing up to `n − t` devices is survivable **if** the remaining holders
  cooperate or their keystores survive.
- **`n`-of-`n`** (e.g. 2-of-2): every device is load-bearing. Lose one keystore
  with no transcript and no resharing quorum → unrecoverable. Choose `t < n` if
  device-loss recovery matters.

---

## 5. Open items (tracked)

- **Resharing protocol.** The "threshold quorum re-issues a lost share" row is
  the intended recovery path but the resharing ceremony itself is not yet a
  first-class flow in all clients — track separately.
- **Optional transcript retention.** We deliberately do *not* persist full DKG
  transcripts (extra secret-adjacent material at rest). If transcript-based
  recovery is ever wanted, it needs an explicit, encrypted, opt-in store — not a
  silent default.

---

## 6. Cross-references

- Derivation grammar, versioning, domain separation, salt decision:
  `packages/@starlab/core/src/root_secret.rs` (module docs + tests).
- N-curve DKG engine: `packages/@starlab/core/src/curve_registry.rs`.
- Encrypted share storage: `packages/@starlab/core/src/keystore.rs`.
- Which chains can actually *verify* the resulting signatures:
  [`SIGNATURE_CHAIN_COMPATIBILITY.md`](SIGNATURE_CHAIN_COMPATIBILITY.md).
