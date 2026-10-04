# EVM → threshold ECDSA (cggmp24)

Decision (plan C, 2026-09-27): ed25519 chains and Bitcoin (Taproot) stay on
FROST; EVM accounts move to threshold ECDSA via `cggmp24` (MIT/Apache-2.0,
Kudelski-audited, 0.7.0-alpha). FROST Schnorr signatures cannot be verified by an EOA (`ecrecover`);
Ethereum signing and transaction construction therefore use the ECDSA key.

Fixed design points (from the cggmp24 0.7 docs/source):
- A wallet holds three keys: FROST ed25519, FROST secp256k1-tr (BIP-340),
  and a cggmp24 secp256k1 ECDSA key. The Ethereum account comes from the
  ECDSA key only; Ethereum addresses change (not launched → no migration).
- Full interactive signing only — **no presignatures** (v0.7 forbids
  presignature + raw-hash and presignature + HD: CVE-2025-66017 class).
- HD: one derivation scheme for every curve. Our public additive-offset
  derivation (`hd_derivation.rs`, chain code from the group key) is plugged
  into cggmp24 via its `HdWallet` trait, so `accounts.rs` stays the single
  source of truth and addresses stay public-only derivable. Path stays
  `m/44'/60'/0'/0/n`.
- Aux info (Paillier moduli, safe primes) runs before/with keygen. Safe-prime
  generation is slow (minutes in WASM): every client pre-generates primes in
  the background and caches them encrypted; aux info is reusable per signer
  group.
- Transport: cggmp24 is network-agnostic. Drive it through round_based's
  sync `state-machine` API in `starlab-core`, so native (TUI/CLI/desktop) and
  WASM (extension) share one driver; messages ride our existing WebRTC data
  channels (DTLS-encrypted p2p) as a new frame kind carrying
  (session id, protocol, round, sender index, p2p|broadcast, payload).
- Execution ID = hash(session id ‖ protocol ‖ sorted participants), never
  reused.
- Known limitation: cggmp24 has **no key refresh** — the ECDSA key can't be
  reshared / a lost device can't be dropped without a new key + moving funds.
  `resharing.rs` keeps covering the FROST keys only. Documented, not hidden.

## Stage 1: Core ECDSA engine (`starlab-core::ecdsa`)
**Goal**: aux-info, threshold keygen and HD signing via cggmp24 behind a
transport-agnostic state-machine driver; key share serialization.
**Success Criteria**: builds native + `wasm32-unknown-unknown` (num-bigint
backend); no presignature API exposed.
**Tests**: in-process 2-of-3 and 3-of-3 keygen + sign over a simulated
network (message shuffling/duplication tolerated as the protocol allows);
the signature `ecrecover`s (k256) to the account's Ethereum address from
`accounts.rs`; public-only child address == signing child address for
accounts 0..3; execution-id separation (replayed message from another
session rejected); share serde round-trip.
**Status**: Complete — `starlab-core::ecdsa` (`EcdsaCeremony` driver, CBOR
wire format v1, `StarlabHd`, `EcdsaKeyShare`, `pregenerate_primes`).
Findings that shape later stages:
- Safe-prime generation (num-bigint) measured at ~94 s per party natively
  (release, 3 runs: 98/75/110 s) and ~5.4 min in wasm/node (299/347 s).
  Background pre-generation is mandatory; the extension must do it in a
  worker well before a DKG.
- Aux info costs ~18 s CPU per party natively (single thread); signing
  ~3 s per signer. Run ceremonies off the UI/async thread
  (`EcdsaCeremony` is `!Send`: create and drive it on one thread/worker).
- `accounts.rs` and chain configuration map Ethereum to
  `secp256k1-ecdsa`; the FROST secp256k1 key is Bitcoin-only.
- cggmp24 0.7.0-alpha.3 must be built with `cggmp24-keygen` /
  `paillier-zk` pinned to `=0.7.0-alpha.3` (alpha.4 moved to generic-ec
  0.5 and breaks the build) — pinned in `starlab-core/Cargo.toml`.

## Stage 2: Shared engine (`starlab-client`) + CLI oracle + wire protocol
**Goal**: the shared engine (Elm core + `HeadlessRunner`, run by CLI and
TUI) creates the ECDSA key in every secp256k1/unified DKG and signs Ethereum
accounts with it; keystore holds the ECDSA share; the CLI proves it end to
end over the signal server + WebRTC.
**Success Criteria**: CLI e2e (in-process + L3 serve processes) green in CI.
**Tests**: real-network e2e: 2-of-3 DKG incl. ECDSA → EIP-191 sign by every
signer pair (consecutive signings, different initiators) → ecrecover ==
account-0 address from accounts.rs; raw 32-byte prehash sign; 3-of-3 signing
twice; 2-of-3 with all 3 online (non-signer gets the signature); ECDSA share
export → import round trip; existing FROST e2e (Bitcoin BIP-340, reshare,
timeout, ed25519) unchanged.
**Status**: Complete (integrated in draft PR #135 targeting
`feat/ecdsa-cggmp24`). Native gates are green; coordinate the engine, desktop
and extension release together; all acceptance gates are complete.

What it does:
- **DKG**: after the FROST ceremonies of a secp256k1 (or unified) DKG, the
  same participants run ECDSA aux info then keygen; keygen index = position
  in the sorted participant list (FROST's canonical order); execution ids per
  `starlab_core::ecdsa::execution_id(session_id, protocol, sorted ids)`.
  `DKGFinalized` fires only once the ECDSA share is persisted too; its
  `addresses` list the ECDSA Ethereum address first (then Bitcoin / Solana).
  ed25519-only DKGs have no ECDSA key.
- **Threads**: each ceremony runs on its own OS thread (`protocal::ecdsa::
  worker`), fed by a std channel, emitting over tokio channels — never on the
  runtime. Frames that arrive before the local ceremony starts are buffered
  (per node, capped) and replayed; frames of other executions are dropped.
- **Primes**: `PrimeSupply` generates in the background when a secp256k1
  runner / the TUI starts; a DKG takes the ready set (waiting with a status
  toast if needed) and the generator starts the next. Memory only. Test-only
  injection: `PrimeSupply::insecure_test_fixed`, reached via the hidden
  `starlab-cli serve --insecure-test-primes <file>` flag and
  `SimulateOpts.insecure_test_primes` (both named insecure; tests feed the
  fixture primes from `packages/@starlab/core/src/ecdsa/testdata`).
- **Keystore**: `secp256k1-ecdsa/<wallet_id>.json`, same encrypted v2 format
  and password (plaintext = `EcdsaKeyShare` serde JSON); listed next to the
  FROST entries (so wallet export writes all curve files, import is per file);
  `Keystore::{save_ecdsa_share, load_ecdsa_share}`.
- **Accounts**: `curve_for_chain("ethereum") = secp256k1-ecdsa`; the FROST
  secp256k1 (Taproot) key is Bitcoin-only.
- **Signing**: an Ethereum account child `{root}-ethereum-{n}` signs with the
  root's ECDSA share at `m/44'/60'/0'/0/n` (no child is materialized), in a
  fresh `sign_<uuid>` session, with exactly `threshold` signers fixed by the
  proposer (itself + first `threshold-1` READY senders — the #121 rule).
  Output: 65 bytes `r ‖ s ‖ v`, low-s, `v = 27 + recovery_id`
  (personal_sign convention; a typed tx takes `y_parity = v - 27`).
  Payloads: EIP-191 hash of the message (default) or, with encoding
  `prehash`, a given 32-byte digest signed as-is (e.g. an EIP-1559 sighash).
  No tx builder in the CLI yet → no real EIP-1559 tx test in this stage.
  The signing announce carries `curve_type: "secp256k1-ecdsa"`,
  `blockchain: "ethereum"`, `wallet_name: "{root}-ethereum-{n}"`,
  `group_public_key` = the ECDSA root key, `signing_message_hex` = the hash.

### Wire format (data channel, for the extension in Stage 4)
All frames are `SimpleMessage` texts on the existing WebRTC data channel
(`{"webrtc_msg_type":"SimpleMessage","text":"…"}`); the sender is the
channel's peer. They are ceremony frames: remembered and resent on reconnect
(#117), de-duplicated on receipt (each carries a digest or session id, so
distinct frames never collide — no SIGN_SET-style exemption needed).

```text
ECDSA:<id>:<index>:<count>:<base64 chunk>
ECDSA_SIGN_READY:<base64 JSON {"session_id":"sign_…"}>
ECDSA_SIGN_SET:<base64 JSON {"session_id":"sign_…","signers":["dev-a","dev-b"]}>
ECDSA_SIGN_DONE:<base64 JSON {"session_id":"sign_…","signature":"<hex r‖s‖v>"}>
```

- `ECDSA:` carries one `starlab_core::ecdsa` driver payload (35-byte header:
  wire version 1, protocol 1 aux-info / 2 keygen / 3 signing, kind
  0 broadcast / 1 p2p, 32-byte execution id; then CBOR), split into chunks
  of ≤ 16 KiB of payload (aux-info messages are ~200 KiB). `<id>` = lowercase
  hex of the first 8 bytes of SHA-256(payload); `<index>` 0-based and
  `<count>` ≥ 1 (≤ 64), decimal; standard base64 with padding. Receivers
  reassemble per (sender, id) and check the digest, then feed the payload
  with the sender's keygen index (position in the sorted participant list).
  A broadcast payload goes to every other party of the ceremony (signing:
  the other signers), a p2p payload to that party only.
- Signing control: every joined node sends READY to the proposer
  (`session.proposer_id`); the proposer fixes the set (itself + first
  `threshold-1` READY senders), sends SET to every participant, and all
  listed nodes start the cggmp24 signing (keygen indices of the set). A node
  outside the set waits; the proposer sends it DONE with the signature, which
  it accepts only if it `ecrecover`s to the account address.
- Timeouts: signing uses the FROST signing timeout (120 s,
  `STARLAB_SIGNING_TIMEOUT_MS`); DKG ceremony 10 min after primes.

Findings for Stages 3–4:
- Aux info measured ~19–20 s per party on the e2e host (3 parties, parallel
  threads, debug build with optimized bigint crates); a signing ~3 s.
- The TUI wallet list shows the ECDSA entry as its own row (curve
  `secp256k1-ecdsa`), like unified wallets show one row per curve — Stage 3
  should group rows by wallet id.
- The desktop app builds on `spawn_secp256k1` (primes start in the runner);
  `spawn_secp256k1_with_primes` exists for injection.
- Legacy FROST `…-ethereum-n` child files, if any, are ignored (Ethereum
  always resolves to the ECDSA root share); not launched → no migration.

## Stage 3: TUI + desktop (engine clients)
**Goal**: the engine side (ECDSA DKG + signing, keystore, primes) landed in
Stage 2; this stage is the UI: TUI screens/labels (group wallet rows by id,
ECDSA signature label, prime-generation status), desktop lock bump + its
Ethereum flows on the new engine.
**Success Criteria**: TUI + desktop (`starlab-desktop`, lock bump) create a
wallet and sign an Ethereum message/tx together.
**Tests**: unit + L3 serve-process tests; live TUI+desktop run.
**Status**: Complete (not merged) — engine PR #133 (into
`feat/ecdsa-cggmp24`), desktop draft PR stars-labs/starlab-desktop#23
(`feat/ecdsa-engine`), with final desktop follow-up #24. All three engine
dependencies and the committed lock pin integrated Git revision
`fab8e16634ce4c03d71b00ff9f92bcb058ecc9d4`. Locked tests, release, strict
Clippy and three-node signing/recovery pass without local overrides. Merge
only together with the engine; final cross-product gates are complete.

Live run (2026-10-04, local signal server, desktop on Xvfb): TUI (real
primes) creates a 2-of-3 wallet, the desktop (real primes) and a CLI `serve`
node (fixture primes) join; all show one wallet row, Ethereum
`0x0b2ea905…989ff6` (ECDSA) + Bitcoin `bc1ph90k…h8vrdv` (Taproot). The DKG
screens showed the Ethereum-key phase (desktop waiting for its primes, TUI in
aux info). Two EIP-191 signings initiated from the TUI, co-signed from the
desktop's approval card: TUI "Verified: YES (ecrecover gives 0x0b2e… =
Ethereum account 0)", desktop "Verified: ecrecover gives 0x0b2e… (this
account)", and Foundry `cast wallet verify` accepts both (65 bytes, low-s,
v 28 / 27).

What it does:
- **Wallet = one row per id.** `keystore::group_wallets` / `WalletGroup` fold
  a wallet's key files (`secp256k1-ecdsa/`, `secp256k1/`, `ed25519/`) into one
  entry; TUI lists, counts, exports and signs per group. Ethereum ← ECDSA key,
  Bitcoin ← Taproot key, Solana/Sui ← ed25519. The desktop groups the same way
  and exports every key file of a wallet (one file per key; losing the ECDSA
  file loses the Ethereum account), importing several files or a folder.
- **Signature screens.** Ethereum: "ECDSA signature (65 bytes, r ‖ s ‖ v) —
  ecrecover-compatible", r / s / v shown, and *verified by ecrecover against
  the account address* (Bitcoin: BIP-340 against the P2TR address) — the TUI
  used to hard-code "verified". The account a ceremony signs for is recorded
  when it starts.
- **ECDSA setup status.** `PrimeSupply::status()` (idle / preparing N s /
  ready / insecure test primes): TUI main menu and desktop sidebar show
  "ECDSA setup: preparing safe primes (… so far, usually 1-5 min)" / "ready".
  After the FROST rounds the DKG screen / setup banner show the Ethereum-key
  phase (waiting for this device's primes with elapsed time → aux info →
  keygen). The ECDSA DKG worker reports typed phases (`DkgPhase`). The desktop
  starts prime generation at launch (it used to start at the first DKG).
- **CI**: the serial "Rust tests + real-DKG e2e" job (31.5 min, run
  36671492127) is now 7 parallel shards (wall 7.6 min cold cache, run
  37016702676; 5.3 min with the shared cache, run 37176538239), all
  tests kept: split suites run name filters in one shard and the same filters
  `--skip`ped in the other. Measured per test: each secp256k1 DKG spends
  20-40 s in ECDSA aux info (FROST alone ~4 s); optimizing every dependency in
  dev builds only cut a 2-of-3 DKG test from ~46 s to ~38 s locally, not worth
  the compile time.

Findings for Stage 4+:
- Safe primes took ~4 min in the TUI on a busy 16-core host (Stage 1 measured
  ~1.5 min idle) — "usually 1-5 min" is what the UIs say.
- After the desktop restarted, the cold-start session replay offered the
  already *completed* signing request again (approving it cannot succeed).
  This has been fixed in PR #135 and extension #83: finished and failed
  sessions are withdrawn, and reconnect retries withdrawal.
- The engine raises `dkg_in_progress` when a signing mesh is ready (it reuses
  the FROST round-1 trigger); the desktop now ignores it outside a wallet
  setup, but the flag itself is misleading for other embedders.
- TUI gaps seen in the live run, not part of this stage: the WebSocket box
  always prints the default `wss://panda.qzz.io` even when connected
  elsewhere; wallet delete is still a stub; the confirm-signing modal clips
  the message preview.

## Stage 4: core-wasm + extension (`starlab-wallet`)
**Goal**: WASM bindings for the ECDSA state machines; extension pre-generates
primes in a worker, joins/initiates ECDSA DKG and signing; Send on Sepolia
works.
**Success Criteria**: extension ↔ CLI interop (2-of-3, 3-of-3) green in CI.
**Tests**: bun unit tests (real WASM); interop spec: CLI + extension ECDSA
DKG, co-sign, and extension-initiated sign.
**Status**: Complete (not merged) — implementation, local acceptance and
final remote interoperability are complete.
Engine PR #135, extension PR stars-labs/starlab-wallet#83 and desktop #24
share integrated engine revision `fab8e166`. Extension head `1954ed3` CI
run [37193441768](https://github.com/stars-labs/starlab-wallet/actions/runs/37193441768)
passes check (947 tests, zero failures; Svelte and production build) and
ED25519 and secp256k1 interoperability. Final secp256k1 main suite: 12 passed
(39.7 minutes), one conditional namespace skip; separate namespace recovery:
one passed (3.2 minutes). All three final CI jobs succeed.
Previous `a260304` CI passes all three jobs, including 12 secp256k1 cases
and separate network-namespace recovery. Final local suites also pass both
curves and namespace recovery. Designer acceptance covers 206 extension
images, 58 desktop images and all 27 implemented feature rows against
Rabby, MetaMask and Phantom. Token identities and unknown-token fallback
have reviewed evidence. Live Sepolia receipt acceptance is complete.

What it does:
- **core-wasm**: `EcdsaAuxInfo` / `EcdsaKeygen` / `EcdsaSigning` over
  `EcdsaCeremony` (`receive(senderIndex, payload)`, `proceed()` → output |
  undefined, `take_outgoing()` → `[{recipient: "broadcast" | index,
  payload}]`); signing output hex `r ‖ s ‖ v` (v = 27/28, engine parity);
  `ecdsa_generate_safe_prime` + `ecdsa_primes_from_parts` (one prime per
  worker), `ecdsa_key_share_from_parts`, `ecdsa_share_info`,
  `ecdsa_ethereum_address`, `ecdsa_recover_ethereum_address`,
  `ecdsa_execution_id`, `ecdsa_payload_execution_id`. The FROST
  `get_eth_address` getters are gone (a BIP-340 key has no EOA address);
  `derive_account_addresses("secp256k1-ecdsa", …)` lists Ethereum.
- **Extension**: ceremonies run in Web Workers spawned by the offscreen
  document (one per job); the Stage 2 wire format byte for byte (pinned to
  the engine's test vectors). Primes: four safe primes in parallel workers
  with per-prime progress ("Preparing secure signing (x%)"), started after
  onboarding / unlock, cached AES-GCM-encrypted with the keystore password
  in `chrome.storage.local` (`ecdsa_primes_v1`), consumed by one DKG. A
  secp256k1 DKG finishes only with its ECDSA share, stored as the wallet's
  `secp256k1-ecdsa` curve entry (same password); backups are one engine v2
  file per curve. Ethereum signing / personal_sign / EIP-712 / Send
  (EIP-1559, y_parity = v − 27) go through READY/SET/DONE, proposer or
  co-signer. Test-only fixed primes exist only in a
  `WXT_INSECURE_TEST_PRIMES=1` build (CI asserts the production bundle has
  no hook).

Findings:
- WASM timings (release pkg): aux info ~122 s per party on one thread
  (365 s for 3 parties sequentially, ~128 s with the parties in parallel
  workers), keygen 0.1 s, signing ~11 s per signer.
- One-shot CLI commands (`wallet create`, `sign`) have no test-primes flag,
  so a CLI-created secp256k1 wallet waits for the creator's own primes
  (~1.5 min native); the interop job timeout is 120 min.

## Stage 5: Live 3-client Sepolia transaction
**Goal**: TUI + desktop + extension 2-of-3 wallet sends a real Sepolia tx.
**Success Criteria**: tx hash confirmed on Sepolia from the MPC address.
**Tests**: live run with screenshots + tx hash.
**Status**: Complete (not merged). A real-prime TUI + desktop + production
extension created the 2-of-3 wallet on 2026-10-04. Final desktop `ae34bc6`,
TUI engine `fab8e166` and extension presentation `e18da4c` restored the same
wallet. Extension Send and desktop approval signed a real EIP-1559 self-transfer
of 0.00001 Sepolia ETH. Receipt status is `0x1`, block 11841485, gas 21000:

[0xe83cfdfba02c82fb067e7bdc195b8f729abe32ffaa247be9a2950805d1e4ff54](https://eth-sepolia.blockscout.com/tx/0xe83cfdfba02c82fb067e7bdc195b8f729abe32ffaa247be9a2950805d1e4ff54)

Sender and recipient are MPC account 0
`0x2fb6ae33558e46ec6baa39c9c98cd00f4b0f7548`. Independent raw-transaction
hashing, signature recovery and desktop approval-digest verification pass;
RPC and Blockscout agree. The non-signing TUI request disappears automatically
after withdrawal without manual dismissal. No fixture primes were used.
See [public evidence](docs/testing/evidence/sepolia-live-2026-10-04.json)
and [acceptance report](docs/testing/SEPOLIA_LIVE_ACCEPTANCE.md).
