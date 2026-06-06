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
| DKG-5 | ed25519 ciphersuite | **L1** ✅ (`spawn_ed25519`; 2-of-2 & 2-of-3 agree) |
| DKG-6 | secp256k1 ciphersuite | default |
| DKG-7 | both curves from one root secret | `unified_dkg` path |

### 3.2 Signing

| ID | Flow | Notes |
|---|---|---|
| SIG-1 | sign with exactly threshold signers | the common case |
| SIG-2 | sign with more than threshold available (quorum subset) | aggregator picks a quorum |
| SIG-3 | sign secp256k1 → verify against group key | `verify_secp256k1` |
| SIG-4 | sign ed25519 → verify | **L1** ✅ (raw-bytes sign; 2-of-2 & 2-of-3 verify) |
| SIG-5 | dApp `personal_sign`-shaped message (Ethereum) | extension-originated in cross-client |
| SIG-6 | raw hex message vs utf8 message | `encoding` field — **L1** ✅ (hex-decode path → EIP-191 → verify) |
| SIG-7 | co-signer decline (explicit rejection) | extension `SigningDecline` path |
| SIG-8 | auto-approve policy gates signing | CLI `--auto-approve` + allowlist + budget — **L3** ✅ (found+fixed a bridge dedup bug) |

### 3.3 Persistence / lifecycle

| ID | Flow | Notes | Layer |
|---|---|---|---|
| LIFE-1 | DKG → keystore written → reload → list shows wallet | cold-start replay; pure keystore round-trip | **L1** ✅ |
| LIFE-2 | reload → sign with persisted share (no re-DKG) | found+fixed 2 real races | **L3** ✅ (see note) |
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

> **🐞→✅ LIFE-2 found and FIXED two real cold-start signing races.** The L3 test
> (`tests/l3_serve_process.rs::sign_after_process_restart_verifies`) does a DKG
> across two `serve` processes, kills both, respawns on the same keystores, and
> signs — now green and back in the CI gate. Getting there exposed **two** genuine
> product bugs (not test artifacts), in sequence:
>
> **Bug A — the WebRTC offer was dropped.** Relay handling (peer offer/answer/ICE
> + `participant_update`) lived only inside the StartDKG/JoinDKG driver loops, so
> it was alive only while/after a DKG had run *this session*. A cold-started
> signer (load keystore → sign, no DKG) had no such loop, so the initiator's
> offer hit the inbound broadcast with no subscriber and was dropped; the mesh
> never formed. Fixed with an **always-on relay handler** (`spawn_relay_handler_task`)
> subscribed once at connect, with the Relay arms removed from the driver loops to
> keep single-handling. (It locks `app_state` only for `from=="server"` frames —
> never for the high-volume peer ICE candidates — so it doesn't contend with the
> FROST ceremony; an earlier version that locked per-candidate tripled large-mesh
> runtime.)
>
> **Bug B — the first SIGN_COMMIT was dropped.** With the mesh fixed, the
> initiator's `SIGN_COMMIT` then raced ~250 ms ahead of the co-signer's
> `JoinSigning` (which rebuilds the signing session from keystore metadata) and
> was dropped ("no active session"), stalling the ceremony. Fixed with a
> **pre-session commit buffer** (`AppState::pending_pre_session_commitments`):
> commits arriving before the session is established are held raw (they can't be
> keyed by `Identifier` yet) and re-fed via `drain_pre_session_commitments` once
> `handle_start_signing` has set up the session.
>
> Both affect any client that restarts then signs (TUI/native/extension share the
> staggered initiator-vs-joiner ordering). Verified: LIFE-2 green; DKG (L3 +
> in-process secp/ed25519), signing (secp/ed25519/hex), and the full 9-group
> matrix all stay green (~81 s). Original failure analysis, kept for the record:
>
> 1. After restart the initiator unlocks its persisted share and runs
>    `StartSigning` (cold path: rebuilds the signing session from wallet
>    metadata, announces it, and **immediately broadcasts its WebRTC offer**).
> 2. The co-signer only discovers the signing session, approves, unlocks, and
>    then runs `JoinSigning` — and **only then starts its WebRTC signaling
>    subscriber**, ~0.7s *after* the initiator's offer was relayed.
> 3. The offer is delivered over the broadcast channel with **no subscriber yet**,
>    so it is dropped; the initiator never re-offers; the signing data channel
>    never opens; `SIGN_COMMIT` retries exhaust and the ceremony stalls.
>
> Warm flows (DKG, and same-run signing) work because both peers initiate WebRTC
> near-simultaneously, so neither offer predates the other's subscriber. This
> affects **all clients** that restart then sign (TUI/native/extension share the
> staggered initiator-vs-joiner ordering). Candidate fixes: run the signaling
> subscriber whenever connected (decoupled from `InitiateWebRTC`); buffer WebRTC
> signals that arrive before the peer connection exists; or have the initiator
> re-send its offer until the channel opens. The reproduction is `#[ignore]`'d
> and **excluded from the CI gate** (CI names the DKG test explicitly) until the
> fix lands, at which point it flips red→green. This is the harness working as
> designed: CLI-as-oracle surfaced a cross-client bug nothing else exercised.

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
| ERR-4 | signing requested for unknown wallet id | error event, no hang — **L1** ✅ (`WalletUnlockFailed`, fast lane) |
| ERR-5 | peer drops mid-DKG | timeout → error, no deadlock |
| ERR-6 | duplicate session announcement | idempotent; not double-joined |
| ERR-7 | malformed JSONL request to `serve` | `Error{code:"bad_request"}`, loop continues — **L3** ✅ (real process; survives + answers next cmd) |

### 3.5 Security / policy

| ID | Flow | Expected behavior |
|---|---|---|
| SEC-1 | auto-approve OFF by default | no silent approvals |
| SEC-2 | auto-approve respects allowlist | off-allowlist wallet refused |
| SEC-3 | auto-approve respects budget | N+1th approval refused |
| SEC-4 | password never on argv in test harness | always `--password-file`/`--password-env` |
| SEC-5 | password never appears in any emitted event or trace | **L3** ✅ (scans all stdout events for a sentinel password) |

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

**Status:** the **event-contract** half (2) is landed — `src/trace.rs` provides
`normalize_event` (redacts volatile crypto/id fields → self-describing `"<key>"`
placeholders, collapses `participants` to `["<device>"]`, keeps every structural/
enum/numeric field literal) and `tests/event_contract.rs` pins all 13 `CliEvent`
variants against `tests/fixtures/event_contract.golden.jsonl`. Deterministic and
offline, so it runs in the fast CI lane; `BLESS=1` regenerates after a reviewed
change. The **wire-frame** half (1) is also landed — `tests/wire_trace.rs` captures the
on-the-wire `announce_session`/`session_available`/`relay` frames via a test-only
recording WebSocket proxy (rather than a `--trace` tee, which would have needed WS-layer
instrumentation) and pins the normalized type vocabulary + session-discovery frame shapes.
The same normal form is what the L4 oracle will diff the extension against.

> **Observation (not a bug) from the signing wire golden:** the signing
> `announce_session` carries `blockchain: "secp256k1"` — i.e. the *curve*, not a
> chain like `ethereum`. This is a limitation, not a correctness defect: the
> headless/TUI sign API (`HeadlessSign{wallet_id, message, encoding, password}`)
> has no per-chain context — a secp256k1 wallet is multi-chain, and the message is
> hashed by curve (EIP-191 for secp256k1), so the signature verifies regardless.
> An extension co-signer parsing this sees the curve where it might expect a chain
> name; worth aligning when the extension-interop layer lands (give the sign API a
> `chain` field, or have `session-parse.ts` tolerate a curve value). The golden
> pins the current shape so the change is visible if/when it's made.

### 5.3 L3 — Cross-client interop (CLI as automated peer)

**What:** Drive a real ceremony where the CLI supplies N−1 peers and the **client under
test supplies the last peer**, over a **real signal server** (embedded for Rust clients;
a real `apps/signal-server` instance for the extension).

Three sub-harnesses, one per non-CLI client:

#### L3·0 — CLI ↔ CLI across processes (foundation) ✅

Before any cross-*client* interop, the substrate is two real `mpc-wallet-cli
serve` **binaries** driven over JSONL, with the signal server in-process. This
is the first thing to exercise the compiled bin's stdin/stdout surface (the lib
tests never do) and the first place teardown is real OS process death — which is
why faithful cold-restart signing (LIFE-2) belongs here, not in the in-process
simulate. Landed: `tests/l3_serve_process.rs` runs a 2-of-2 DKG across two
`serve` processes and asserts they agree on the group key (~3s); it also
cross-checks the creator's `session_announced` id against the peer's
`session_available` id. The `ServeProc` JSONL driver it introduces (spawn,
send, `wait_for(event)`) is the reusable harness the cross-client layers extend.

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

| Curve | Group key | Chain | Expected address |
|---|---|---|---|
| secp256k1 | generator G | Ethereum | `0x7e5f4552091a69125d5dfcb7b8c2659029395bdf` ✅ |
| secp256k1 | generator G | Bitcoin P2WPKH | `bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4` ✅ |
| ed25519 | all-zero key | Solana | `11111111111111111111111111111111` ✅ |

These reuse the same derivation paths as the per-chain tests already landed in
`crypto-rust-tools/yubiwallet/tests/chains.rs`, keeping one source of truth for address
ground-truth across both repos.

**Status / 🐞 found a serious bug.** The deterministic address goldens are landed as
bridge unit tests (`golden_ethereum_address_for_generator_g`,
`golden_solana_address_for_zero_key`) — secp256k1 generator G → the canonical
`0x7e5f4552…` (privkey=1) and the all-zero ed25519 key → the Solana System Program id,
both externally verifiable against the yubiwallet vectors. **Writing them immediately
caught a real bug:** `generate_address_for_chain` (used by the bridge/CLI for the address
shown to users) derived Ethereum addresses by stripping the first byte of the **compressed**
33-byte FROST group key and keccak-hashing the remaining 32 bytes — i.e. hashing only the
X coordinate — instead of decompressing to `X‖Y` first. That produced addresses that **do
not correspond to the signing key** (G gave `0x51cbf46…` instead of `0x7e5f4552…`), and it
**disagreed with the keystore's own `derive_ethereum_address`** (which already decompressed
correctly). Fixed by decompressing via `k256::PublicKey::from_sec1_bytes` →
`to_encoded_point(false)` before hashing (robust to compressed *or* uncompressed input);
the golden now matches the canonical vector and the two derivations agree. This is the
address-oracle catching a wrong-receive-address bug that affected every secp256k1 wallet's
displayed address.

The **Bitcoin P2WPKH golden** is now landed too (`golden_bitcoin_p2wpkh_for_generator_g`),
and it surfaced a second gap: `generate_address_for_chain` had **no `bitcoin` arm at all**
— "bitcoin" is a registered chain, but the oracle returned "address generation not
implemented", so BTC MPC wallets could not display an address. Implemented P2WPKH = bech32
segwit-v0 of `hash160(compressed pubkey)` (P2WPKH mandates the compressed key, which is
exactly FROST's serialization), pinned against the BIP-173 worked example
(G → `bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4`). All three address goldens now hold.

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

3. **ed25519 runner.** ✅ **Done.** Added `spawn_ed25519` alongside `spawn_secp256k1`,
   and made `HeadlessRunner::new` seed `curve_type` from `C` (it previously relied on the
   default being secp256k1 — which silently mis-curved an ed25519 runner). `simulate`
   accepts `curve=ed25519` and switches the spawn fn; DKG-5 (2-of-2 & 2-of-3 ed25519)
   passes. SIG-4 (ed25519 sign+verify) is also done — `verify_signature` dispatches to
   `verify_ed25519`/`verify_secp256k1` by curve. Still open: the Solana address golden.

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

> **Status (live).** Phases 1–3 are done and a substantial L3 foundation is in place;
> the harness has already found and fixed **5 real bugs** (see §13). Remaining work is the
> heavier cross-client infra (Phases 5–6) and the wire-frame half of L2.

- **Phase 1 — L1 matrix.** ✅ **Done.** `tests/conformance_matrix.rs` covers DKG
  (2-of-2/2-of-3/3-of-3/3-of-5/2-of-4, secp256k1 + ed25519), signing (threshold + quorum
  subset, secp256k1 + ed25519 + hex), LIFE-1/3/4, ERR-1/4. `session_announced` (#7.1)
  added. Wired into the CI e2e job.
- **Phase 2 — L2 goldens.** ✅ **Done.** Event-contract golden + normalizer
  (`src/trace.rs`, `tests/event_contract.rs`) and the ETH/BTC/SOL address goldens
  (`bridge.rs`) are in the fast lane. The **wire-frame golden** (#7.2) is also done —
  `tests/wire_trace.rs` captures the real signal-server protocol via a test-only recording
  WebSocket proxy (no core instrumentation needed: clients are pointed at the proxy via
  `SimulateOpts.signal_url`), and pins the normalized type vocabulary + `announce_session`/
  `session_available` shapes in `tests/fixtures/dkg_wire_protocol.golden.txt`. Stable
  across runs; `BLESS=1` regenerates. This is the wire contract the L4 oracle diffs the
  extension against.
- **Phase 3 — ed25519 runner (#7.3).** ✅ **Done.** `spawn_ed25519`; DKG-5 + SIG-4 pass;
  Solana address golden landed.
- **L3 foundation + tests.** ✅ **Done (beyond the original plan).**
  `tests/l3_serve_process.rs` drives real `serve` subprocesses over JSONL: cross-process
  DKG, **LIFE-2 cold-restart signing** (the two-race fix), ERR-7 (malformed input), SEC-5
  (no password leak), SIG-8 (auto-approve). Reusable `ServeProc` harness.
- **Phase 4 — L3a (CLI↔native).** ⏸ **Deprioritized.** Native reuses the *same*
  `HeadlessRunner`/core as the CLI, so an in-process CLI↔native test would mostly re-cover
  CLI↔CLI. Low marginal value vs the extension; do only if native grows client-specific
  logic.
- **Phase 5 — L3c + L4 (CLI↔extension).** ⬜ **Pending — the real remaining prize.** Real
  server + Playwright + differential oracle against the address/event goldens. The only
  layer that crosses the Rust-core ↔ independent-TS/WASM boundary. Needs dedicated
  scaffolding (browser automation, extension build in CI).
- **Phase 6 — L3b (CLI↔TUI via PTY).** ⬜ **Pending, low priority.** TUI shares the L1-
  covered core; only View/keystroke wiring is unique.

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

---

## 13. Bugs found (and fixed) by building this harness

The point of a conformance oracle is to find bugs; here is the running tally. Every one was
surfaced by writing a test that exercised a real path, verifiable against external ground
truth or a cross-client invariant — exactly the failure classes §1 predicted.

| # | Bug | Surfaced by | Fix |
|---|---|---|---|
| 1 | Headless/CLI nodes never replayed sessions announced before they connected (the extension auto-replays). | LIFE-4 | `HeadlessRefreshSessions` → `LoadSessions`; CLI `list_sessions` now replays. |
| 2 | Ethereum address derived by hashing the compressed key's X coordinate, not `keccak(X‖Y)` — every secp256k1 wallet showed a wrong address that didn't match its signing key, and disagreed with the keystore's own derivation. | ETH address golden (vs generator-G vector) | Decompress via `k256` before keccak. |
| 3 | Bitcoin had no address derivation at all (registered chain → "not implemented"). | BTC address golden (vs BIP-173 vector) | Implement P2WPKH (bech32 segwit-v0 of `hash160`). |
| 4 | Cold-start signing stalled — (A) the WebRTC offer was dropped (relay handling lived only in DKG driver loops), then (B) the first `SIGN_COMMIT` raced ahead of the co-signer's session setup and was dropped. | LIFE-2 (L3 cold restart) | (A) always-on relay handler; (B) pre-session commit buffer. |
| 5 | The warm signing path reuses the DKG session id; the CLI bridge deduped discovered sessions by id alone, so it never emitted a `signing_request` for a signing session reusing a DKG id — silently breaking discovery for every CLI co-signer. | SIG-8 (auto-approve e2e) | Dedup per `(kind, id)`. |

Bugs 1, 2, 4, 5 are in the **shared Rust core** (so they affected TUI/native/CLI alike);
2 and 3 are also exactly the cross-impl correctness class the L4 address oracle targets.
None had any prior test.

### Cross-impl crypto: verified correct (by inspection)

A static check of the extension's independent crypto answered the most important L4
question without a browser:

- **Address derivation.** The extension's WASM (`@mpc-wallet/core-wasm`) derives Ethereum
  addresses via `frost-core::Secp256k1Curve::get_eth_address`, which **decompresses the key
  correctly** (`k256::from_sec1_bytes → to_encoded_point(false) → keccak(X‖Y)[12..]`). So
  bug #2 was specific to the divergent `tui_node::blockchain_config` impl (now fixed to
  match); the extension was already correct. Pin this with the same golden vectors when the
  browser harness lands.
- **FROST compatibility.** `core-wasm` builds on the same `frost_secp256k1` / `frost_ed25519`
  crates as the CLI/TUI/native, so DKG packages and signatures are cross-compatible *by
  construction* — the risk isn't the crypto, it's the protocol glue.

What remains for **L3c** is therefore the extension's **TypeScript orchestration layer**
(`webSocketManager`, `session-parse.ts`, the offscreen WebRTC/WASM host) driven end-to-end
in a real browser against CLI peers — exactly the layer the wire-frame goldens (§5.2) now
specify the contract for. That's a dedicated browser-automation effort (Playwright + MV3
extension load + WASM build), not an autonomous-loop increment.
