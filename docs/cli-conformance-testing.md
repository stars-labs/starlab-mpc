# CLI-Driven Conformance & Cross-Client Parity Testing

**Status:** Design proposal (no implementation yet)
**Owner:** MPC wallet team
**Scope:** `apps/cli-node` as the test oracle and automated peer for the whole MPC stack
**Related:** `apps/tui-node`, `apps/native-node`, `apps/browser-extension`, `apps/signal-server`

---

## 1. Motivation

The MPC wallet ships **four independent clients** that must interoperate over one wire
protocol:

| Client | Language | Core | UI |
|---|---|---|---|
| TUI node (`apps/tui-node`) | Rust | `tui_node::elm` + `tui_node::core` | Ratatui |
| Native node (`apps/native-node`) | Rust | reuses `tui_node::core` + `HeadlessRunner` | Slint |
| CLI node (`apps/cli-node`) | Rust | reuses `tui_node::elm::HeadlessRunner` | JSONL stdin/stdout |
| Browser extension (`apps/browser-extension`) | TypeScript + WASM | independent reimpl of FROST glue over `@mpc-wallet/core-wasm` | Svelte 5 |

Three of the four (TUI, native, CLI) share the **same Rust Elm core**. The extension is a
**separate implementation** of the same ceremony, sharing only the FROST primitives (via
WASM) and the wire protocol. That asymmetry is the central testing problem:

- A bug in the shared Rust core is reproducible from any of the three Rust clients — and
  the CLI is the cheapest place to catch it.
- A bug in the **wire protocol contract** (field names, tags, ordering, optional vs
  required) only shows up at the boundary between *different* implementations — i.e.
  Rust-core ↔ extension. Nothing in the Rust-only test suite exercises that boundary.
- A bug in a **client's UI/orchestration layer** (the extension's `webSocketManager`
  trigger logic, the native node's Slint bridge, the TUI's Elm wiring) is invisible to a
  unit test of the core; it only appears when the client is driven end-to-end.

The CLI node was built to be terminal-free, scriptable, and deterministic. It already runs
a full DKG + threshold signing in-process in seconds (`simulate`, `tests/e2e_dkg.rs`).
This document proposes promoting it from "a way to test the core" to **the conformance
oracle and automated counterparty for every other client.**

### Goals

1. **Exhaustively exercise every MPC flow** the product supports (DKG variants, signing
   variants, persistence, error/edge cases, security policy) using the CLI alone — fast,
   deterministic, CI-friendly.
2. **Use the CLI as a golden oracle**: pin protocol traces and cryptographic outputs from
   the CLI, then assert every other client reproduces them bit-for-bit.
3. **Use the CLI as an automated peer**: let one or more CLI nodes stand in as N−1
   participants so a single human/GUI/extension client can be driven through a real
   ceremony in an automated test.
4. **Reverse-check the GUIs**: when a flow that passes CLI↔CLI fails CLI↔(TUI|native|
   extension), the diff localizes the bug to that client's non-shared layer.

### Non-goals

- Replacing the human-in-the-loop smoke tests entirely (3-browser live FROST pairing
  still has value as a final gate).
- Testing blockchain RPC / on-chain submission (out of scope; this is about the MPC
  ceremony and address/signature derivation).
- Performance/load testing.

---

## 2. What "conformance" means here

Two clients **conform** if, given the same inputs (participant set, threshold, root
entropy where applicable, message to sign), they produce:

1. **The same group public key** after DKG (FROST DKG is deterministic given the same
   round packages, but the packages depend on per-node randomness — so equality is
   asserted *across the cluster in one run*, not across runs; see §5.4).
2. **The same derived addresses** for every supported chain from a given group key.
3. **Verifiable signatures** over the same message under the same group key (signature
   bytes differ by nonce, but all must verify against the group key — this is the
   invariant we assert, not byte-equality).
4. **A wire-message sequence** that is shape-compatible: same `type` tags, same field
   names/casing, same required/optional discipline, same ordering constraints.

(1)–(3) are *cryptographic* conformance; (4) is *protocol* conformance. The harness tests
both, with different oracles.

---

## 3. Flow catalog

The complete set of flows the harness must cover. Each is a row in the L1 matrix (§5.1)
and, where a GUI exposes it, a cross-client case (§5.3).

### 3.1 DKG (key generation)

| ID | Flow | Notes |
|---|---|---|
| DKG-1 | 2-of-2 | minimal threshold; both must sign |
| DKG-2 | 2-of-3 | canonical; primary CI case |
| DKG-3 | 3-of-5 | larger participant set |
| DKG-4 | t-of-n parametric | property-style sweep over small (t,n) with t≤n, n≤7 |
| DKG-5 | ed25519 ciphersuite | currently CLI runner is secp256k1-fixed — see §7 gap |
| DKG-6 | secp256k1 ciphersuite | default |
| DKG-7 | both curves from one root secret | `unified_dkg` path |

### 3.2 Signing

| ID | Flow | Notes |
|---|---|---|
| SIG-1 | sign with exactly threshold signers | the common case |
| SIG-2 | sign with more than threshold available (quorum subset) | aggregator picks a quorum |
| SIG-3 | sign secp256k1 → verify against group key | `verify_secp256k1` |
| SIG-4 | sign ed25519 → verify | needs ed25519 runner (§7) |
| SIG-5 | dApp `personal_sign`-shaped message (Ethereum) | extension-originated in cross-client |
| SIG-6 | raw hex message vs utf8 message | `encoding` field |
| SIG-7 | co-signer decline (explicit rejection) | extension `SigningDecline` path |
| SIG-8 | auto-approve policy gates signing | CLI `--auto-approve` + allowlist + budget |

### 3.3 Persistence / lifecycle

| ID | Flow | Notes | Layer |
|---|---|---|---|
| LIFE-1 | DKG → keystore written → reload → list shows wallet | cold-start replay; pure keystore round-trip | **L1** ✅ |
| LIFE-2 | reload → sign with persisted share (no re-DKG) | the bug that bit the headless sign path | **L3** (see note) |
| LIFE-3 | wallet label/name round-trips through keystore | `WalletMetadata.label` / `display_name()` | L1 |
| LIFE-4 | session announced before our connect is still discoverable | `request_active_sessions` replay | **L1** ✅ (found+fixed a parity gap — see note) |

> **LIFE-2 is an L3 (process-isolation) test, not L1.** A faithful cold-restart
> *re-signing* requires the previous node to be truly gone. In-process, the CLI
> `HeadlessRunner` has no shutdown that aborts its spawned tasks — `Message::Quit`
> only breaks the Elm loop, so the old node's WebSocket/reconnect tasks **and its
> WebRTC ICE agents stay alive**. On the same host those ghost ICE agents answer
> the fresh node's connectivity checks with a mismatched DTLS fingerprint, so the
> new signing mesh's data channel never opens (observed: the post-reload
> `SIGN_COMMIT` retries exhaust; the channel only opens ~100s later). Pointing the
> reloaded nodes at a fresh signal server fixes WS discovery but not the ICE
> interference. Real process death (the L3 `serve`-subprocess harness, where the OS
> reclaims sockets/tasks on SIGKILL) is the only faithful way to test LIFE-2 —
> which is also exactly the production condition (a restart kills the process).
> LIFE-1 (the persistence half — the share is on disk and reloads with the right
> group key) is fully covered in L1.

> **LIFE-4 surfaced a real cross-client parity gap (now fixed).** The browser
> extension fires `requestActiveSessions()` automatically ~2s after the WebSocket
> opens, so it always discovers sessions announced before it connected. The Rust
> core only fired the equivalent `LoadSessions` replay when the user was on the
> Join-Session *screen* — so a headless node (native/CLI) never auto-replayed and
> would silently miss a pre-announced session. Fix: a `HeadlessRefreshSessions`
> message (→ `Command::LoadSessions`) plus a `HeadlessRunner::refresh_sessions()`
> helper, and the CLI `list_sessions` command now triggers a real server replay
> (previously it only answered from the local cache). The LIFE-4 test asserts the
> explicit replay works and *records* (without yet asserting) that auto-on-connect
> replay is still extension-only — a candidate follow-up if headless auto-replay is
> wanted.

### 3.4 Error & edge cases

| ID | Flow | Expected behavior |
|---|---|---|
| ERR-1 | wrong password on unlock | clean error, no panic, no partial state — **L1** ✅ (`WalletUnlockFailed`, "Invalid password") |
| ERR-2 | round2 package arrives before local part2 | buffered + re-fed (regression #20) |
| ERR-3 | joiner receives real total/threshold (not hardcoded) | regression #19 |
| ERR-4 | signing requested for unknown wallet id | error event, no hang |
| ERR-5 | peer drops mid-DKG | timeout → error, no deadlock |
| ERR-6 | duplicate session announcement | idempotent; not double-joined |
| ERR-7 | malformed JSONL request to `serve` | `Error{code:"bad_request"}`, loop continues |

### 3.5 Security / policy

| ID | Flow | Expected behavior |
|---|---|---|
| SEC-1 | auto-approve OFF by default | no silent approvals |
| SEC-2 | auto-approve respects allowlist | off-allowlist wallet refused |
| SEC-3 | auto-approve respects budget | N+1th approval refused |
| SEC-4 | password never on argv in test harness | always `--password-file`/`--password-env` |
| SEC-5 | password never appears in any emitted event or trace | grep assertion over JSONL + trace |

---

## 4. Architecture: the CLI as oracle + peer

```
                         ┌─────────────────────────────────────────────┐
                         │              Shared Rust Elm core            │
                         │     tui_node::elm  +  tui_node::core         │
                         └───────▲──────────────▲──────────────▲────────┘
                                 │              │              │
                    ┌────────────┴──┐   ┌───────┴──────┐  ┌────┴─────────┐
                    │   CLI node    │   │   TUI node   │  │  native node │
                    │ (JSONL/stdio) │   │  (Ratatui)   │  │   (Slint)    │
                    └───────▲───────┘   └──────▲───────┘  └──────▲───────┘
                            │                  │ PTY             │ in-proc
       oracle + automated   │                  │                 │ HeadlessRunner
       peer                 │   ┌──────────────┴─────────────────┘
                            │   │
                            ▼   ▼              wire protocol (snake_case, tagged)
                    ┌───────────────────┐    ◄──────────────────────────────────►
                    │   signal server   │              ┌─────────────────────┐
                    │ (embedded/loopback│◄────────────►│  browser extension  │
                    │   or real WS)     │   WS + WebRTC│  (TS + WASM, indep.) │
                    └───────────────────┘              └─────────────────────┘
```

The CLI plays three roles:

- **Oracle** — run a flow CLI-only, capture `(group_key, addresses, signature, wire
  trace)`, freeze as a golden fixture.
- **Peer** — run as N−1 participants so the client-under-test only has to supply 1
  participant, yet a real ceremony completes.
- **Driver** — its JSONL `serve` surface is itself the thing a test script speaks to when
  the CLI *is* the client under test.

---

## 5. Test layers

Four layers, increasing in fidelity and cost. CI runs L1–L2 on every push; L3–L4 on a
schedule / pre-release (they need browsers and PTYs).

### 5.1 L1 — CLI↔CLI flow matrix (fast, in-process, always-on)

**What:** Every row of the §3 catalog driven entirely by CLI nodes in one process, over an
**embedded signal server on a loopback ephemeral port**, with real WebRTC over loopback.

**How:** Extend the existing `simulate` / `tests/e2e_dkg.rs` machinery:

- `run_simulation(opts)` already does DKG for N nodes and asserts all agree on the group
  key. Generalize `SimulateOpts` to cover every DKG/signing/persistence/error case as a
  parametric table test.
- Keep the in-process `dkg_cluster(opts) -> Cluster` that holds runners alive, so signing
  and reload cases can reuse a live cluster.
- Each catalog ID becomes a `#[test]` (fast cases) or `#[ignore]` e2e (cases needing the
  full WebRTC mesh, run with `--ignored` in CI).

**Catches:** every shared-core bug — DKG races, round buffering (#20), joiner param
plumbing (#19), persistence/reload, auto-approve policy. This is the regression net for
the Rust core.

**Cost:** seconds. Runs on every push.

### 5.2 L2 — Protocol golden fixtures (the oracle, deterministic)

**What:** For each flow, capture the **exact sequence of wire messages** (the
`relay`/`announce_session`/`session_available`/… frames as serialized JSON) plus the
**JSONL event stream** the CLI emits, and freeze them as golden files under
`apps/cli-node/tests/fixtures/`.

**How:** Add a `--trace <path>` option to `serve`/`simulate` (see §7) that tees every wire
frame sent/received and every `CliEvent` emitted, with **volatile fields redacted/
normalized** (nonces, ephemeral keys, timestamps, random session-id suffixes → stable
placeholders). The golden is the normalized trace.

A fixture pins two things:
1. **Shape** — the set and order of `type`-tagged frames for a flow (e.g. DKG round1
   broadcast → round1 ingest ×(n−1) → round2 → round2 ingest → finalize).
2. **Event contract** — the `CliEvent` sequence and their fields (`Ready`, `Ack`,
   `DkgProgress`, `DkgComplete{group_public_key, addresses}`, `SignatureComplete`, …).

**Catches:** accidental protocol drift inside the Rust stack (a serde rename, a field made
optional, a reordered broadcast). These fixtures become the **specification** the
extension is tested against in L4.

**Cost:** ms. Pure comparison. Runs on every push. Regenerated deliberately with a
`BLESS=1`-style env when the protocol intentionally changes (reviewed in the diff).

### 5.3 L3 — Cross-client interop (CLI as automated peer)

**What:** Drive a real ceremony where the CLI supplies N−1 peers and the **client under
test supplies the last peer**, over a **real signal server** (embedded for Rust clients;
a real `apps/signal-server` instance for the extension).

Three sub-harnesses, one per non-CLI client:

#### L3a — CLI ↔ native (in-process)
Native node already embeds `HeadlessRunner` via `core_adapter.rs`. Spin up the native core
adapter + (n−1) CLI runners in one test binary on a loopback signal server. Assert the
native node reaches `DkgComplete` with the same group key the CLI cluster agrees on, and
that addresses derived by the native path equal the CLI's.

> **Reverse-check:** if L1 DKG-2 passes but L3a DKG-2 fails, the bug is in native's
> CoreAdapter / Slint bridge / `model_wallets` address derivation — *not* the core.

#### L3b — CLI ↔ TUI (via PTY)
Drive the real TUI binary under a pseudo-terminal (e.g. `portable-pty`/`expect`-style),
scripting keystrokes for "create wallet 2-of-2", while a CLI peer joins. Assert the TUI's
rendered final state (parsed from the screen, or better, from a TUI debug/JSON side-channel
if we add one) shows the same group key/address. Lower fidelity (screen-scraping) but
catches Elm wiring bugs the headless runner skips because it bypasses the View.

> **Reverse-check:** L1 passes, L3b fails → bug is in TUI View/Update wiring or the
> keystroke→Message mapping, not the core.

#### L3c — CLI ↔ extension (via Playwright + real signal server)
The highest-value, highest-cost harness — it's the only one that crosses the
**Rust-core ↔ independent-TS-impl** boundary.

- Start a real `apps/signal-server` instance on a known port.
- Start (n−1) CLI `serve` nodes pointed at it, pre-seeded with keystores (or co-running a
  DKG).
- Launch the extension in a Playwright-driven Chromium with the signal URL configured.
- Script the extension UI: create/join wallet, approve signing.
- Assert: the extension and the CLI peers **agree on the group key**, the extension's
  displayed address matches the CLI-derived address, and a signature produced with the
  extension as a co-signer **verifies against the group key**.

> **Reverse-check & bug localization:** This is where wire-protocol mismatches surface.
> If the extension can't even join (session never reaches threshold), the bug is in
> `session-parse.ts` / `announce_session` field handling. If it joins but DKG stalls, it's
> in the extension's `add_round1_package`/`can_start_round2` n−1 accounting. If DKG
> completes but the address differs, it's address derivation in the extension vs
> `blockchain_config`. Each failure mode maps to a specific extension module.

**Catches:** the entire class of "the extension and the Rust clients disagree" bugs that
*no other layer can see*.

**Cost:** seconds–minutes; needs a browser + a running server. Scheduled / pre-release.

### 5.4 L4 — Differential oracle (golden diff across clients)

**What:** Tie L2 and L3 together. The L2 CLI golden fixtures are the **reference**; L4
asserts each client reproduces them.

For each flow and each client:

- **Group key:** within a single interop run, all participants (CLI peers + client under
  test) must report the *same* group public key. (We can't compare to a frozen golden key
  across runs because DKG randomness differs run-to-run — so the invariant is
  *intra-run agreement*, captured by every node emitting its computed key and the harness
  asserting they're all equal.)
- **Address:** for a given group key, the derived address per chain is deterministic.
  Assert every client derives byte-identical addresses → **this can be a cross-run golden**
  (pin a known group key → known addresses table; reuse the yubiwallet-style vectors).
- **Signature:** not byte-equal across clients (nonces differ), so the invariant is
  *verifies against the group key*. Each client's produced signature is run through
  `verify_secp256k1`/ed25519 verify; all must pass.
- **Wire trace:** the extension's emitted frames (captured via Playwright network
  inspection / a CDP hook) are normalized the same way as L2 and diffed against the CLI
  golden **shape** (not values). Drift = protocol bug.

**Address golden table** (cross-run deterministic — derived from a fixed group key):

| Curve | Group key (hex) | Chain | Expected address |
|---|---|---|---|
| secp256k1 | *(pinned)* | Ethereum | *(pinned, lower-case)* |
| secp256k1 | *(pinned)* | Bitcoin P2WPKH | *(pinned bc1q…)* |
| ed25519 | *(pinned)* | Solana | *(pinned base58)* |

These reuse the same derivation paths as the per-chain tests already landed in
`crypto-rust-tools/yubiwallet/tests/chains.rs`, keeping one source of truth for address
ground-truth across both repos.

---

## 6. Per-client bug classes each layer targets

| Bug class | Lives in | Caught by |
|---|---|---|
| DKG round race / buffering | shared Rust core | L1 |
| Joiner param plumbing (#19) | shared Rust core | L1 |
| Persistence / reload / sign-after-reload | shared Rust core | L1 (LIFE-*) |
| Auto-approve policy | CLI + core | L1 (SEC-*) |
| Protocol serde drift (rename/optional/order) | Rust wire layer | L2 golden |
| Native CoreAdapter / Slint bridge / address derivation | native only | L3a (vs L1) |
| TUI Elm View/Update wiring | TUI only | L3b (vs L1) |
| Extension session-parse / field casing | extension only | L3c, L4 trace diff |
| Extension n−1 round accounting | extension only | L3c |
| Extension address derivation | extension only | L4 address golden |
| Cross-impl signature validity | extension vs core | L4 verify |

The "(vs L1)" entries are the **reverse-check**: a flow green at L1 but red at L3 isolates
the bug to the client's non-shared layer by construction.

---

## 7. Required CLI enhancements

Small, additive changes to `apps/cli-node` to support the harness. None change existing
behavior by default.

1. **`session_announced` event.** When a `serve` node *creates* a session (DKG or
   signing), emit `CliEvent::SessionAnnounced { correlates, session_id }` as soon as the
   announcement goes out. Today a driver that creates a session has no deterministic way
   to learn the generated `session_id` to hand to the other peers; it has to scrape it
   from `SessionAvailable`. An explicit creator-side event removes that race. *(Bridge
   already tracks `announced_sessions`; this surfaces it.)*

2. **`--trace <path>` on `serve` and `simulate`.** Tee every wire frame (sent + received)
   and every emitted `CliEvent` to a JSONL trace file, with volatile fields normalized
   (see §5.2). Off by default; stdout protocol stream is unchanged.

3. **ed25519 runner.** `spawn_secp256k1` is curve-fixed. Add `spawn_ed25519` (or
   parameterize `HeadlessRunner` over the ciphersuite) so DKG-5 / SIG-4 / Solana address
   goldens can run. The Elm core is already generic over `FrostCurve`; this is wiring, not
   new crypto.

4. **Deterministic test entropy hook (test-only).** An opt-in seed (behind a
   `#[cfg(test)]` / hidden flag) so a run can be made reproducible for trace stability
   where we *want* byte-equality. Must never be reachable in a release build / real use.

5. **`--peers-ready` / barrier nicety (optional).** A way for `simulate` to expose when
   the cluster is fully connected, so L3 harnesses know when to launch the client under
   test. Can be derived from existing `Connection` events; flagged as optional.

All five are backward-compatible and independently shippable.

---

## 8. Repository layout

```
apps/cli-node/
  src/...                         # existing
  tests/
    e2e_dkg.rs                    # existing — extend into the L1 matrix
    conformance_matrix.rs         # NEW — L1 parametric table over the §3 catalog
    fixtures/                     # NEW — L2 golden traces (normalized JSONL)
      dkg_2of3.trace.jsonl
      sign_2of2_secp256k1.trace.jsonl
      ...
    interop_native.rs             # NEW — L3a (in-process CLI↔native)
docs/
  cli-conformance-testing.md      # this document

apps/browser-extension/
  tests/interop/                  # NEW — L3c Playwright harness (CLI peers + real server)

scripts/
  conformance/                    # NEW — orchestration: start server, spawn CLI peers,
                                  #       run PTY (TUI) / Playwright (extension) drivers
```

TUI PTY driver (L3b) lives under `apps/tui-node/tests/` or `scripts/conformance/` depending
on whether `portable-pty` becomes a dev-dependency of the crate or an external script.

---

## 9. CI integration

| Stage | Layers | Trigger | Approx time |
|---|---|---|---|
| `rust-fast` | L1 (non-ignored) + L2 golden diff | every push | <1 min |
| `rust-e2e` | L1 `--ignored` (full WebRTC mesh) + L3a | every push (or PR) | ~1–3 min |
| `interop-tui` | L3b (PTY) | nightly + pre-release | ~min |
| `interop-ext` | L3c + L4 (Playwright + real signal server) | nightly + pre-release | ~min |
| `human-smoke` | 3-browser live FROST | manual, release gate | manual |

The existing `.github/workflows/ci.yml` (rust job + extension bun job) is extended:
`rust-fast`/`rust-e2e` slot into the rust job; `interop-ext` becomes a new job that
`bun install`s, builds the extension, boots `apps/signal-server`, and runs Playwright.

Golden regeneration is a deliberate, reviewed action (`BLESS=1 cargo test -p
mpc-wallet-cli`), so a protocol change shows up as a fixture diff in the PR.

---

## 10. Phased delivery

Each phase is independently mergeable and leaves the tree green.

- **Phase 1 — L1 matrix.** Turn the §3 catalog into parametric CLI↔CLI tests on top of the
  existing `simulate`/`e2e_dkg` machinery. Highest ROI; pure Rust; no new infra. Add
  `session_announced` event (#7.1) along the way.
- **Phase 2 — L2 golden traces.** Add `--trace` (#7.2), normalize, freeze fixtures, wire
  the golden diff into `rust-fast`. Establishes the protocol spec artifact.
- **Phase 3 — ed25519 runner (#7.3).** Unlocks DKG-5/SIG-4 and the Solana address golden.
- **Phase 4 — L3a (CLI↔native).** In-process; reuses Phase-1 machinery + native's
  `core_adapter`. First reverse-check harness.
- **Phase 5 — L3c + L4 (CLI↔extension).** The crown jewel: real server + Playwright +
  differential oracle. Highest cost, catches the cross-impl bug class nothing else can.
- **Phase 6 — L3b (CLI↔TUI via PTY).** Lower priority (the TUI shares the core that L1
  already covers; only the View/keystroke wiring is unique), but closes the matrix.

---

## 11. Open questions

1. **TUI observability.** L3b needs to read the TUI's final state. Screen-scraping is
   brittle; a small `--emit-json-state` debug side-channel on the TUI would make it
   robust. Worth it, or accept scraping?
2. **Trace normalization fidelity.** How aggressively to normalize? Too little → flaky
   goldens (nonces differ every run); too much → the golden stops catching real drift.
   Proposal: normalize only provably-volatile fields (nonces, ephemeral keys, the random
   suffix of session ids, timestamps) and keep everything else literal.
3. **Extension wire capture.** Playwright network inspection vs a CDP hook vs an
   instrumented build that tees frames. The instrumented build is most reliable but
   diverges from the shipped artifact. Lean toward CDP network capture of WS frames.
4. **Embedded vs real signal server for L3c.** The extension needs a real WS endpoint.
   Embedded `webrtc_signal_server::run(listener)` should work if the extension can be
   pointed at an arbitrary `ws://127.0.0.1:<port>` — confirm the extension's signal URL
   is fully configurable at runtime (not baked at build).
5. **ed25519 across the whole matrix** vs only the address/verify goldens — full parity
   doubles the matrix. Proposal: full DKG/sign coverage for secp256k1, targeted coverage
   for ed25519 (one DKG, one sign, the Solana address golden).

---

## 12. Summary

The CLI node is already a terminal-free driver of the shared Rust core. This design turns
it into the project's **conformance oracle** (frozen protocol + address goldens), its
**automated counterparty** (N−1 peers so a single GUI/extension client can be driven
through a real ceremony), and the **reference for a reverse-check**: a flow that passes
CLI↔CLI but fails CLI↔client localizes the bug to that client's non-shared layer. Four
layers — in-process matrix, golden traces, cross-client interop, differential oracle —
give fast always-on regression coverage of the core plus deep, scheduled coverage of the
one boundary nothing else tests: the Rust core against the independent TypeScript/WASM
extension.
