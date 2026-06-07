# Networked Reshare Ceremony — Design

**Status:** Design proposal (pre-implementation)
**Issue:** #45 (the networked half — the engine + `reshare-simulate` CLI already landed)
**Scope:** turn share refresh/resharing from an in-process engine into a real,
networked ceremony over the WebRTC mesh, drivable from the CLI (and reusable by
TUI/native/extension), so a quorum can **rotate shares** or **remove a device**
with the wallet's address unchanged.

---

## 1. Goal & non-goals

**Goal.** A `reshare` ceremony that, given an existing wallet and a chosen
*retained* participant set, has those participants collaboratively produce
**fresh shares** over the mesh such that:

1. the **group public key (address) is unchanged**,
2. every retained device ends with a **new** encrypted keystore share,
3. any **pre-reshare share is dead** (can't sign with the refreshed group),
4. a **removed** device is dropped (its old share is dead; it isn't in the new set).

**Non-goals (this design).**
- **Adding a brand-new device** that never held a share — frost-core refresh needs
  each participant's old `KeyPackage`; onboarding a fresh device is keystore-restore
  or a separate enrollment protocol (out of scope; see `RECOVERY_AND_RESHARING.md`).
- Lowering the threshold (frost forbids it; the refresh preserves `min_signers`).
- Changing the curve.

---

## 2. Background: the DKG ceremony we mirror

Reshare is "DKG-shaped rounds over an *existing* wallet". The existing DKG path
(the template):

- **Session.** `SessionType::DKG` in `protocal/signal.rs`; announced on the signal
  server via `announce_session`; peers `join`. `SessionInfo { session_id, proposer_id,
  total, threshold, participants, session_type, curve_type, … }`.
- **Identifiers.** `protocal/dkg.rs::canonical_identifier(participants, device_id)` =
  **sort participants, position+1**. Every node derives the same id set from the same
  participant list.
- **Driver.** `Command::StartDKG` (creator) / `Command::JoinDKG` (joiner) set up the
  session; the mesh forms (WebRTC); then rounds flow as messages:
  `ProcessDKGRound1{from_device, package_bytes}` → `ProcessDKGRound2{…}` →
  `DKGFinalized{wallet_id, group_pubkey_hex, curve_type, addresses}`.
  `protocal/dkg.rs` holds the per-round accumulators on `AppState` and calls
  frost `dkg::part1/2/3`.
- **Persistence.** On finalize, the share is written via
  `keystore::storage::create_wallet*` (encrypted; `WalletMetadata` carries
  `threshold`, `total_participants`, `participants`).
- **Signing (the closest existing "existing-wallet" ceremony).** `Command::StartSigning`
  shows the pattern reshare needs: load the wallet (`state.public_key_package`,
  `state.keystore`), warm/cold session setup, announce with the group key so joiners
  cross-check. Reshare copies this "operate on an already-loaded wallet" shape.

---

## 3. The one hard problem: identifier preservation

`canonical_identifier` derives a node's FROST identifier from its **position in the
sorted participant list**. That is fine for DKG (the set is fixed for the whole
ceremony) but **breaks for reshare-with-removal**:

> Original `{alice,bob,carol}` → sorted ids alice=1, bob=2, carol=3.
> Remove **bob** → naive recompute over `{alice,carol}` → alice=1, **carol=2**.
> But carol's **old `KeyPackage` carries identifier 3**. frost-core
> `refresh_dkg_shares(old_key_package, …)` would then mismatch → wrong/failed refresh.

**Rule (load-bearing):** reshare participants MUST keep the identifier they had in
the **original** wallet. Derive identifiers from the wallet's **original full
participant list** (persisted in `WalletMetadata.participants` at DKG time), filtered
to the retained set — **never** recompute `canonical_identifier` over the reduced set.

Consequences:
- The retained ids may be **non-contiguous** (e.g. `{1,3}` after removing 2). The
  refresh round packages are keyed by these real ids; `max_signers` = retained count
  (2), but the identifiers are `{1,3}`.
- **Validation needed:** confirm frost-core `refresh_dkg_part_1(id, max_signers,
  min_signers)` + `part2` + `refresh_dkg_shares` accept non-contiguous identifiers
  with `max_signers < max(id)`. The current `resharing.rs` tests only cover `{1,2,3}`
  and `{1,2}` (contiguous). **Add an engine test for `keep={1,3}` before building the
  network layer** (cheap, de-risks the whole feature). If frost rejects it, fall back
  to the trusted-dealer refresh (`compute_refreshing_shares` over the retained ids) or
  document removal as "remove highest-index only".

`resharing.rs::refresh(old_kps, old_pub, new_ids, threshold, …)` already takes the ids
explicitly (doesn't recompute), so the engine is ready; the **network layer** is what
must pass original ids.

---

## 4. New protocol surface

### 4.1 Session type
```rust
// protocal/signal.rs
pub enum SessionType {
    DKG,
    Signing { wallet_name, curve_type, blockchain, group_public_key },
    Reshare {
        wallet_name: String,
        curve_type: String,
        group_public_key: String,   // the OLD group key; joiners verify it matches
                                     // their loaded wallet before participating.
    },
}
```
`SessionInfo.participants` = the **retained** device set (the new signer set). The
announce carries `group_public_key` so a joiner refuses if it doesn't own that wallet.

### 4.2 Messages (`elm/message.rs`)
Mirror the DKG trio:
```rust
StartReshareProtocol,                                   // mesh ready → begin
ProcessReshareRound1 { from_device, package_bytes },    // refresh_dkg_part_1 pkg
ProcessReshareRound2 { from_device, package_bytes },    // refresh_dkg_part2 pkg (per-recipient)
ReshareFinalized { wallet_id, group_pubkey_hex, curve_type },
// headless entry points (CLI/serve):
HeadlessReshare { wallet_id, keep: Vec<String>, password },
```

### 4.3 Commands (`elm/command.rs`)
```rust
Command::StartReshare { wallet_id, keep: Vec<String> },  // creator: load wallet, announce
Command::JoinReshare  { session_id, … },                 // joiner: load wallet, join
```

### 4.4 Wire compatibility
`Reshare` is a new `SessionType` tag → extends the existing `announce_session` /
`session_available` shape (serde-tagged). Older clients ignore unknown sessions.
The CLI bridge (`bridge.rs`) gains a `reshare_request` event (like `signing_request`)
so co-signers can auto-approve via policy.

---

## 5. End-to-end flow

```
creator (alice)                         joiners (bob, [carol])
──────────────                          ──────────────────────
reshare --wallet-id W --keep alice,bob
  load wallet W (unlock keystore)
  read old PublicKeyPackage + my KeyPackage + original participants
  build SessionInfo{type:Reshare, participants=keep, group_public_key=old}
  announce_session ───────────────────► session_available (Reshare, W's group key)
                                          each joiner: load wallet W, verify group key
                                          matches; if not in `keep`, ignore
                                        ◄─ join (session_status_update)
  mesh reaches |keep| participants → StartReshareProtocol on every node
  ── WebRTC mesh ────────────────────────────────────────────────
  each node, using its ORIGINAL identifier (from W's metadata ∩ keep):
    refresh_dkg_part_1(id, max=|keep|, min=threshold) → broadcast R1 pkg
    collect |keep|-1 R1 pkgs → refresh_dkg_part2 → send per-recipient R2 pkgs
    collect R2 pkgs + old KeyPackage + old PublicKeyPackage →
        refresh_dkg_shares → (new KeyPackage, new PublicKeyPackage)
    ASSERT new group key == old group key   (abort + keep old share if not)
    atomically overwrite keystore share for W; securely erase the old share
  emit ReshareFinalized
```

Removed devices (not in `keep`) simply never join; their share is untouched on
disk but is now **cryptographically dead** against the refreshed group. (UX note:
tell the user to delete the removed device's keystore; we can't reach it.)

---

## 6. The mesh driver (`protocal/reshare.rs`)

New module mirroring `protocal/dkg.rs`, reusing the **frost-core refresh primitives**
directly (the streaming/mesh version of `resharing::refresh`):

- `start_reshare<C>(app_state, session)` — compute my original identifier; call
  `refresh_dkg_part_1`; stash the round-1 secret on `AppState`; broadcast my R1 pkg.
- `process_reshare_round1<C>(app_state, from, bytes)` — accumulate peers' R1 pkgs;
  when `|keep|-1` collected, call `refresh_dkg_part2`; stash R2 secret; send each
  peer its R2 pkg (point-to-point over the mesh, like DKG round 2).
- `process_reshare_round2<C>(app_state, from, bytes)` — accumulate R2 pkgs; when
  complete, call `refresh_dkg_shares(r2_secret, others_r1, recv_r2, old_pub,
  old_key_pkg)`; **assert group key unchanged**; persist; emit `ReshareFinalized`.

AppState gains reshare accumulators (mirror the DKG ones): `reshare_round1_secret`,
`reshare_round1_packages`, `reshare_round2_secret`, `reshare_round2_packages`, plus
the loaded `old_key_package` / `old_public_key_package` (already present as the
wallet's `key_package`/`public_key_package` once unlocked).

Curve dispatch: same `C: Ciphersuite` generic pattern the DKG/signing drivers use
(ed25519 vs secp256k1 chosen from the wallet's `curve_type`).

---

## 7. Keystore: load, swap, erase

- **Load** (creator + joiners): unlock W exactly as signing does (decrypt with the
  user password → `key_package` + `public_key_package` on AppState).
- **Write** the refreshed share: reuse `keystore::storage::create_wallet*` to
  overwrite W's encrypted file with the new `KeyPackage` (same `wallet_id`, same
  `group_public_key`, same address, same metadata except a bumped `reshare_epoch`/
  timestamp). **Atomic:** write to a temp file then rename, so a crash mid-write
  can't corrupt the only share.
- **Erase old:** after a verified swap, the previous share bytes must not linger.
  At minimum overwrite+remove the prior file; ideally zeroize in-memory copies
  (`zeroize` already used in `root_secret.rs`).
- **Abort safety:** if `refresh_dkg_shares` fails or the group-key assertion fails,
  **keep the old share** and report an error — never leave the wallet without a
  usable share.

---

## 8. Safety invariants (must all hold, asserted in code + tests)

1. **Group key preserved:** `new_pub.verifying_key() == old_pub.verifying_key()` —
   abort otherwise (a mismatch means a protocol/identifier bug; do not persist).
2. **Threshold preserved:** `min_signers` unchanged.
3. **Old share death:** an old `KeyPackage` cannot combine with refreshed shares to
   verify (covered by the engine test; add a cross-process variant).
4. **Atomic persistence:** never a window with no valid share on disk.
5. **Quorum required:** the ceremony needs ≥ `threshold` retained participants online
   (you can't reshare with fewer than the threshold — same liveness as signing).

---

## 9. CLI / serve surface

- One-shot: `mpc-wallet-cli reshare --wallet-id W --keep alice,bob --password-file f
  --room R --signal-server …` (creator); co-signers run `serve` and approve the
  `reshare_request` (auto-approve policy reused) or `session join`.
- `reshare-simulate` (already shipped) stays as the in-process/CI check.
- Bridge events: `reshare_request` (discovered), `reshare_progress`,
  `reshare_complete{wallet_id, group_public_key}`.

---

## 10. Edge cases & failures

| Case | Handling |
|---|---|
| Joiner doesn't own W (group key mismatch) | refuse to join; log; don't contribute. |
| Fewer than `threshold` retained online | ceremony stalls → timeout → error; old shares intact. |
| Non-contiguous retained ids ({1,3}) | must work (see §3 validation); else fall back. |
| Crash mid-write | atomic temp+rename → old or new share, never neither. |
| Group-key assertion fails | abort, keep old share, loud error. |
| A removed device is offline | fine — it's excluded by construction; warn user to wipe it. |
| Mixed-version peers (no Reshare type) | they ignore the session; ceremony only proceeds if all retained peers understand it. |

---

## 11. Test plan

- **Engine (frost-core, fast):** extend `resharing.rs` — add `keep={1,3}`
  (non-contiguous) and a 3→2 removal where the removed id is the middle one. (Gate
  the whole feature on this passing.)
- **L1 in-process (cli):** extend `reshare-simulate` coverage in CI (already green
  for same-set + tail removal).
- **L3 cross-process (`tests/l3_serve_process.rs` style):** DKG across 2–3 `serve`
  processes → kill/keep → run a real `reshare` over the mesh → assert all retained
  nodes agree on the unchanged group key, a refreshed quorum signs, and an old share
  (captured pre-reshare) fails. This is the real proof; mirrors the LIFE-2 harness.
- **Wire golden (L2):** pin the `Reshare` `announce_session` shape in `wire_trace`.

---

## 12. Phased delivery (each PR independently green)

> **Status (live):** phases 1–4a are **done and merged**, all unit-tested. The
> remaining networked wiring (4b) + CLI/L3 (4c) is tracked in **issue #56** — it
> needs a real multi-node/multi-process environment to validate end to end.

1. ✅ **Engine hardening** (PR #52) — `resharing.rs`: non-contiguous-id +
   middle-removal tests. frost accepts non-contiguous ids → no fallback needed.
2. ✅ **Protocol types** (PR #53) — `SessionType::Reshare`,
   `CliEvent::ReshareRequest`, discovery plumbing + serde tests. No behavior yet.
3. ✅ **Driver core** (PR #54) — `protocal/reshare.rs` (`reshare_part1/part2/
   finalize`, group-key-preserved assertion) + AppState accumulators; multi-
   AppState in-process test.
4. **CLI/serve wiring** — split:
   - ✅ **4a** (PR #55) — `Keystore::update_wallet_share`: atomic
     (temp+fsync+rename) share swap; address/label preserved.
   - ⬜ **4b** (#56) — async mesh transport (`WebRTCMessage` reshare rounds +
     device/webrtc routing) + `StartReshare`/`JoinReshare` command+message
     handlers + mesh-ready trigger.
   - ⬜ **4c** (#56) — `mpc-wallet-cli reshare` one-shot + `serve` approve path +
     **L3 cross-process e2e test** ("the button works").
5. ⬜ **Other clients (later)** — native/TUI reuse the core; extension implements
   the same three messages over its WASM `refresh_*` (separate effort, like its DKG).

Phases 1–4 land the headless, testable feature end to end; phase 5 is per-client UI.

---

## 13. Open questions / risks

1. **Non-contiguous identifiers** (§3) — the single biggest unknown; validate in
   phase 1 before anything else.
2. **frost-core refresh exposes the streaming pieces we need?** `refresh_dkg_part_1/
   part2/refresh_dkg_shares` are public (used by `resharing.rs`), so yes — the driver
   just sequences them over the mesh.
3. **Old-share erase guarantees** — best-effort on disk; document that a copied/backed-
   up old keystore remains a latent (but sub-threshold) share until the epoch's quorum
   is itself rotated. This is inherent to refresh, not a bug.
4. **Concurrent ceremonies** — disallow reshare while a sign/DKG for the same wallet
   is in flight (single in-flight ceremony per wallet on AppState).
5. **Extension parity** — out of scope here; tracked separately so a reshare done by
   CLIs is later honored by an extension co-signer.
