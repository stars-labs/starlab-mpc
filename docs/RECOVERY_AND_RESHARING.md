# Recovery & Resharing — "What if a device is lost or stolen?"

The question every serious investor (and user) asks about a multi-device wallet.
This doc gives the **honest answer**, the **threat model**, and the **design** for
device recovery via share refresh/resharing — grounded in what FROST and this repo
actually provide. Read with [`MULTI_CURVE_DERIVATION.md`](MULTI_CURVE_DERIVATION.md)
(the seed-vs-share recovery model).

---

## TL;DR

- A `t`-of-`n` wallet **survives losing up to `n − t` devices** with no loss of funds.
- A **stolen device alone is useless** to an attacker as long as fewer than `t` shares are
  compromised — it cannot sign.
- To bring a **replacement device** online (or to neutralise a stolen one), a quorum of `t`
  holders runs a **share refresh / resharing** ceremony: the **group key (your address)
  stays the same**, every share is replaced with a fresh one, and a removed device's old
  share becomes **worthless**.
- The cryptographic primitive for this **already exists in `frost-core` (`keys::refresh`)**,
  including a **dealerless** variant matching our no-trusted-dealer model. The engine is
  **implemented and tested** (`frost-core/src/resharing.rs`); wiring it into a networked
  one-command client ceremony is the remaining engineering (tracked below).
- The everyday backup unit remains the **encrypted keystore + password** per device.

---

## 1. Threat model & guarantees

| Situation (2-of-3 example) | Funds safe? | Can still sign? | What to do |
|---|:--:|:--:|---|
| Lose **1** device (keystore gone) | ✅ | ✅ (other 2) | Refresh over the survivors (→ 2-of-2, same address), or restore the lost device from its keystore backup (§3). |
| Lose **2** devices, no keystore backups | ❌ | ❌ | Below threshold → unrecoverable. Choose `t<n` and back up keystores to avoid this. |
| **1** device stolen | ✅ | ✅ | Attacker has 1 < 2 shares → can't sign. **Refresh** to rotate that share to worthless (§3, §4). |
| **t** devices stolen/colluding | ❌ | (attacker can) | This is the threshold assumption being violated. Pick `t` for your trust model. |
| Keystore + password backed up | ✅ | ✅ | Restore the keystore on a new device. |

**Core property:** the private key is *never assembled anywhere*. Security holds as long as
fewer than `t` shares are in one adversary's hands. Recovery is about **re-distributing
shares**, not reconstructing a key.

> See [`MULTI_CURVE_DERIVATION.md`](MULTI_CURVE_DERIVATION.md) §3 for why the **root seed
> alone cannot rebuild a share** in dealerless DKG — backups are the keystore, not the seed.

---

## 2. The two recovery mechanisms

### (a) Keystore restore — the everyday path
Each device stores its share encrypted (PBKDF2/Argon2 + AES-256-GCM, `keystore.rs`). The
backup unit is **(keystore file, password)**. Restore = copy the keystore to the new device
and unlock. Simple, but the password is **unrecoverable by design** — store the two so
losing one doesn't lose both.

### (b) Refresh / resharing — the quorum path (no backup needed)
A threshold `t` of current holders collaboratively **issue fresh shares**. Properties:

- **Group public key — and therefore your on-chain address — is unchanged.** No funds move,
  no re-funding, no address rotation.
- **All shares are replaced.** Any share from before the refresh (e.g. on a stolen device)
  **no longer works** with the new set — it can't combine with refreshed shares to sign.
- **Threshold cannot be lowered** by a refresh (you can't weaken security this way).
- The participant set can **shrink** (remove a device), but a refresh **cannot add a
  brand-new device that never held a share** — frost-core's refresh requires every
  refreshing participant to bring its existing `KeyPackage`. Bringing a fresh replacement
  online is a keystore-restore or a fuller enrollment protocol (see §3).

> Verified in code: `frost-core/src/resharing.rs` proves group-key preservation, that an
> old share can't combine with refreshed shares, and that a participant can be removed —
> and that refresh **rejects** a participant with no prior share.

---

## 3. Recovery flows (operationally)

**Stolen/compromised device — proactive rotation (the strong case, fully supported):**
1. The remaining holders run a refresh; the stolen share is rotated to **worthless** while
   the address is unchanged. Optionally **remove** the compromised device (refresh over the
   subset, e.g. 2-of-3 → 2-of-2 over `{alice, bob}`).

**Lost device, quorum survives (2-of-3, lose carol):**
- **Keep it simple:** refresh over the survivors `{alice, bob}` → a 2-of-2 wallet, **same
  address**, carol's old share dead. You're back to a working wallet immediately.
- **Restore a third device:** bring carol's *replacement* online by **restoring carol's
  encrypted keystore backup** to it (the everyday path, §2a). A refresh can then rotate all
  three. *(Adding a never-seen device purely via refresh is not supported — see §2b.)*

**Below threshold, no backups:** unrecoverable — this is the security guarantee, not a bug.
Mitigate *in advance* by choosing `t < n` and/or keeping encrypted keystore backups.

---

## 4. Design: wiring resharing into the clients

The math is provided by `frost-core::keys::refresh` (v2.2). Two flows; **use the dealerless
DKG one** to match our no-trusted-dealer model:

```text
refresh_dkg_part_1(...)   // each participant: keep secret pkg, broadcast package   (like DKG round 1)
refresh_dkg_part2(...)    // each participant: produce per-recipient packages        (like DKG round 2)
refresh_dkg_shares(...)   // each participant: derive its refreshed KeyPackage + the
                          // (unchanged) group PublicKeyPackage                       (like DKG part 3)
```

(A trusted-dealer variant — `compute_refreshing_shares()` + `refresh_share()` — also exists
for simpler setups.)

**The engine is implemented and tested** in `frost-core/src/resharing.rs`:
`refresh(old_kps, old_pub, new_ids, threshold, …)` runs the three `refresh_dkg_*` steps
across the (possibly reduced) participant set and returns fresh key packages with the group
key preserved. Tests prove key-preservation, old-share invalidation, removal, and the
"can't refresh a shareless participant" guard.

What remains is the **networked client ceremony** around it, which mirrors the existing DKG
plumbing:

- **Transport & coordination:** identical to DKG — signal-server session announce/join +
  the WebRTC mesh. A new session type `reshare` (alongside `dkg`/`signing`).
- **Client engine:** drive `resharing::refresh` over the mesh (the per-participant round
  packages instead of the in-process loop). Output share replaces the on-disk keystore.
- **CLI surface (proposed):** `mpc-wallet-cli reshare --wallet-id <id> --keep alice,bob`
  (the surviving/retained set) + `session join` for the others — same UX shape as
  `wallet create`. *(Removing a device = omit it from `--keep`. Adding a fresh device is a
  separate keystore-restore/enrollment step, not a flag here — see §2b.)*
- **Safety:** assert the new `PublicKeyPackage` equals the old group key before committing;
  then securely erase the **old** keystore so the stale share can't linger.

**Status:** crypto engine + tests **done** (`resharing.rs`); the networked client ceremony
(session type, mesh driver, CLI/UX, old-share erase) is the remaining work. Tracked in the
resharing follow-up issue.

---

## 5. Proactive security (the bonus story)

Because refresh **invalidates all prior shares while keeping the address**, running it on a
schedule (say monthly) gives **proactive secret sharing**: an attacker must compromise `t`
shares *within a single epoch* — shares stolen in different epochs never combine. This is a
strong, investor-legible security posture that custodial single-key wallets simply cannot
offer.

---

## 6. Investor talking points

- *"Lose a laptop? Your money is fine — any two of your three devices keep signing; refresh
  down to those two (same address), or restore the lost device from its encrypted backup."*
- *"A stolen device is a dead end for the thief — one share can't sign, and we rotate it to
  worthless."*
- *"We can refresh the shares on a schedule so even a patient attacker collecting fragments
  over time never assembles a key. A single-key wallet can't do that."*
- **Honest caveat:** *"Lose more than `n−t` devices with no keystore backup and the funds are
  gone — that's the same threshold guarantee that keeps attackers out. Pick your `t`."*

---

## 7. Cross-references
- Seed vs share, recovery matrix: [`MULTI_CURVE_DERIVATION.md`](MULTI_CURVE_DERIVATION.md)
- Encrypted keystore: `packages/@mpc-wallet/frost-core/src/keystore.rs`
- Refresh primitive: `frost-core::keys::refresh` (upstream 2.2)
- DKG engine the reshare path would extend: `packages/@mpc-wallet/frost-core/src/{unified_dkg,curve_registry}.rs`
