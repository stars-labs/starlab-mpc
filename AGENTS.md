# AGENTS.md

This file provides guidance to coding agents when working with code in this repository.

## Build & Test Commands

```bash
# Rust (all workspace crates)
# NOTE: `default-members = ["apps/tui"]`, so bare cargo commands only cover the
# TUI package. Pass --workspace (or -p <pkg>) for everything else.
cargo build --workspace                  # Build all workspace members
cargo test --workspace                   # Run all Rust tests
cargo test -p starlab-core               # Test specific package
cargo test -p starlab-client             # Test TUI node
cargo test --workspace test_name         # Run single test by name
cargo run --example unified_dkg -p starlab-core  # Run example
cargo run                                # Run TUI app (the default member)
cargo check --workspace                  # Fast type check without codegen

# WASM + TypeScript packages (Bun)
bun install                              # Install JS dependencies (from repo root)
bun run build:wasm                       # Build WASM bindings (wasm-pack, from repo root)
bun run --filter '@stars-labs/types' build   # Build the shared TS types package
```

## Architecture

Rust monorepo (edition 2024) with Bun-managed WASM/TypeScript packages. Seven Cargo workspace members:

### Core Library: `packages/@starlab/core/`
Shared FROST cryptographic implementation used by all Rust targets. Key modules:
- `unified_dkg.rs` — Runs FROST DKG for ed25519 + secp256k1 simultaneously from a single root secret
- `hd_derivation.rs` — BIP-44 style HD key derivation using additive scalar offsets (no extra DKG rounds); `AccountKeyFinalize` turns a secp256k1 account child into its BIP-86 output key (share path and public-only path alike)
- `traits.rs` — `FrostCurve` trait abstracting over curve operations
- `ed25519.rs` / `secp256k1.rs` — Curve implementations. secp256k1 FROST is the BIP-340 / Taproot suite (`frost-secp256k1-tr`): signatures spend Bitcoin P2TR; EVM needs ECDSA (see `ecdsa/`)
- `accounts.rs` — BIP-44/86 account model (Bitcoin = P2TR `bc1p…` on `m/86'/0'/0'/0/n`; `verify_taproot_signature` checks a BIP-340 sig against an address), the single source of truth for ALL clients: `Wallet → Account(index) → per-chain address`, derivation paths pinned here; address derivation is public-only (no key share / password needed)
- `curve_registry.rs` — Tag → ciphersuite table; object-safe `CurveDkg` so multi-curve DKG loops over registered curves instead of hard-coded arms
- `resharing.rs` — Share refresh via `frost_core::keys::refresh`: rotate shares / drop a device WITHOUT changing the group public key (see `docs/RECOVERY_AND_RESHARING.md`)
- `keystore.rs` — Encrypted key share storage (PBKDF2 + AES-256-GCM); legacy-ciphertext fixtures in its tests pin the on-disk format
- `rng.rs` — RNG bridge: rand_core 0.10 RNGs → the rand_core 0.6 traits frost 3.0 takes
- `root_secret.rs` — Root entropy → deterministic per-curve RNGs via HKDF
- `ecdsa/` — Threshold ECDSA (secp256k1, `cggmp24` 0.7) for EVM accounts, curve tag `secp256k1-ecdsa`. `EcdsaCeremony` is a sync, transport-agnostic driver over round_based's state machine (aux-info / keygen / signing; feed `receive(sender, bytes)`, `proceed()`, drain `take_outgoing()`; parties addressed by keygen index; wire format v1 = 35-byte header + CBOR, see `ecdsa/driver.rs`); `execution_id(session, protocol, participants)`; `pregenerate_primes` (slow — ~1.5 min native, ~5 min wasm; do it in the background); `EcdsaKeyShare` (serde, keystore plaintext); `hd::StarlabHd` plugs our `hd_derivation` math into cggmp24 so signing at `m/44'/60'/0'/0/n` matches `accounts.rs`. Full interactive signing only — no presignatures; no key refresh (cggmp24 has none)

### Applications
- **`apps/tui/`** — Terminal UI (Ratatui) with Elm architecture (`src/elm/` for Model/Update/View). Exposes `lib.rs` (the Elm core + `core::*Manager` + `HeadlessRunner`) so the desktop app (`stars-labs/starlab-desktop`) can reuse the business logic cross-repo. Supports online (WebRTC mesh) and offline (SD card air-gap) DKG modes.
- **`apps/signal-server/`** — WebRTC signaling: standard WebSocket server + Cloudflare Worker variant

### WASM & Blockchain
- **`packages/@starlab/core-wasm/`** — Thin `wasm-bindgen` wrapper around `starlab-core`
- **`packages/@starlab/blockchain/`** — Multi-chain support. Only `solana-sdk` is pulled in directly; `bitcoin.rs` and `ethereum.rs` are hand-rolled over `sha2` / `sha3` / `bs58` primitives (ethers/bitcoin crate deps were removed when their dependent examples were disabled — see the Cargo.toml comment at line 27)

## Key Patterns

**FROST ciphersuite type names**: `frost_ed25519::Ed25519Sha512` and `frost_secp256k1_tr::Secp256K1Sha256TR` (note capital K in Secp256K1).

**frost-core internal types**: `SigningShare::new()`, `VerifyingShare::new()`, `VerifyingKey::new()` are `pub(crate)`. To construct these from outside frost-core, use `serialize()` / `deserialize()` round-trips through `Field::serialize`/`Group::serialize`.

**UIProvider trait** (`apps/tui/src/elm/provider.rs`): Abstracts the TUI's Elm app loop over a UI backend. Separate from `UICallback` (see below).

**UICallback trait** (`apps/tui/src/core/mod.rs`): Event-push surface for the non-Elm managers in `starlab-client::core`. The TUI goes through the Elm loop; the desktop app (`starlab-desktop`, cross-repo) implements `UICallback` directly to push `UiEvent`s into an mpsc channel that its Iced `Subscription` turns into messages. Keep this trait + the `core::*Manager` types `pub` for that cross-repo consumer.

**Elm architecture** in TUI: State is `Model`, transitions via `Update`, rendering via `View`. Event-driven through `InternalCommand<C>` enum.

## core-wasm API contract (consumed by starlab-wallet cross-repo)

The browser extension lives in `stars-labs/starlab-wallet` and loads `@stars-labs/core-wasm` — the WASM surface here is a cross-repo contract; breaking it breaks the extension.

WASM FROST methods called for signing: `signing_commit()` (returns hex), `add_signing_commitment(idx, hex)`, `sign(msgHex)` (returns hex), `add_signature_share(idx, hex)`, `aggregate_signature(msgHex)` (returns hex). Participant indices are 1-based; compute as `participants.indexOf(peerId) + 1`. Both `signing_commit()` and `sign()` auto-register the local side of their output (our commitment, then our share) into the WASM instance's internal maps — callers must NOT call `add_signing_commitment` / `add_signature_share` for their own index, those are peer-only. This keeps the contract uniform across every `add_*` method (peer-only) while satisfying frost-core's requirement that the signer's own commitment + share appear in the signing_package / aggregate input.

DKG is analogous: `generate_round1()` returns our round-1 package as hex; `add_round1_package(idx, hex)` is called for peer packages only. `can_start_round2()` returns true once all n-1 peer packages are ingested (matches frost-core's `dkg::part2` contract which wants exactly n-1). Same for round 2.

## Wire protocol (signal server)

All clients (TUI, CLI, extension in starlab-wallet) are shape-compatible — see TUI's `command.rs`. Top-level serde tag `type`, `snake_case`.

- `announce_session` / `session_available` — session-discovery broadcasts. Flat `session_type: "dkg" | "signing"` string; signing sessions carry top-level `wallet_name`, `group_public_key`, `blockchain`, `signing_message_hex` siblings. See `packages/@starlab/types/src/session.ts`.
- `request_active_sessions` / `sessions_for_device` — cold-start replay of sessions announced before a client connected.
- `session_status_update` — emitted on join.
- `relay` (generic peer-to-peer, wraps `websocket_msg_type`) — used for WebRTCSignal, SessionProposal, SessionResponse, and `SigningDecline` (explicit rejection without joining the mesh).

### Threshold ECDSA (Ethereum) on the data channel

Ethereum accounts use the `secp256k1-ecdsa` key (cggmp24); Bitcoin (Taproot) and ed25519 chains stay on FROST. Every client (TUI/CLI/desktop via `starlab-client`, extension via core-wasm) implements this byte for byte; the engine (`apps/tui/src/protocal/ecdsa/`) is the reference.

- **DKG**: after the FROST ceremonies of a secp256k1/unified DKG, the same participants run ECDSA aux-info then keygen. Keygen index = position in the sorted participant list; execution id = `starlab_core::ecdsa::execution_id(session_id, protocol, sorted ids)`. The DKG finishes only once the ECDSA share is persisted (keystore file `secp256k1-ecdsa/<wallet_id>.json`, same encrypted v2 format and password; wallet export writes one file per curve).
- **Primes**: safe-prime generation is slow (~1.5 min native, ~5 min wasm), so clients pre-generate in the background and a DKG consumes one ready set. Primes are never stored in plaintext. Test-only injection exists only behind names containing `insecure` (`serve --insecure-test-primes`, the extension's `WXT_INSECURE_TEST_PRIMES=1` build).
- **Frames** — `SimpleMessage` texts on the WebRTC data channel (`{"webrtc_msg_type":"SimpleMessage","text":"…"}`); the sender is the channel's peer. All are ceremony frames: resent on reconnect and de-duplicated on receipt.
  ```text
  ECDSA:<id>:<index>:<count>:<base64 chunk>
  ECDSA_SIGN_READY:<base64 JSON {"session_id":"sign_…"}>
  ECDSA_SIGN_SET:<base64 JSON {"session_id":"sign_…","signers":["dev-a","dev-b"]}>
  ECDSA_SIGN_DONE:<base64 JSON {"session_id":"sign_…","signature":"<hex r‖s‖v>"}>
  ```
  `ECDSA:` carries one driver payload (35-byte header: wire version 1, protocol 1 aux-info / 2 keygen / 3 signing, kind 0 broadcast / 1 p2p, 32-byte execution id; then CBOR), split into ≤ 16 KiB chunks. `<id>` = lowercase hex of the first 8 bytes of SHA-256(payload); `<index>` 0-based, `<count>` 1–64, decimal; standard padded base64. Receivers reassemble per (sender, id), check the digest, and feed the payload with the sender's keygen index. Broadcast payloads go to every other party of the ceremony (signing: the other signers), p2p payloads to that party only.
- **Signing**: account `{root}-ethereum-{n}` signs with the root ECDSA share at `m/44'/60'/0'/0/n` in a fresh `sign_<uuid>` session. Announce: `curve_type: "secp256k1-ecdsa"`, `blockchain: "ethereum"`, `wallet_name: "{root}-ethereum-{n}"`, `group_public_key` = ECDSA root key, `signing_message_hex` = the 32-byte hash (EIP-191 hash, or a prehash such as an EIP-1559 sighash). Every joined node sends READY to the proposer; the proposer fixes the set (itself + the first `threshold-1` READY senders) and sends SET to all participants; listed nodes run cggmp24 signing; nodes outside the set get DONE and accept it only if it `ecrecover`s to the account address. Output: 65 bytes `r ‖ s ‖ v`, low-s, `v = 27 + recovery_id` (a typed tx uses `y_parity = v - 27`). Timeouts: signing 120 s (`STARLAB_SIGNING_TIMEOUT_MS`), DKG ceremony 10 min after primes are ready.
- **Limits**: full interactive signing only (no presignatures — forbidden with HD / raw hashes in cggmp24 0.7). cggmp24 has **no key refresh**: the ECDSA key cannot be reshared or a lost device dropped without a new key and moving funds; `resharing.rs` covers the FROST keys only.

## GUI products live in their own repos

This repo is the **headless engine + terminal clients**: the crypto packages
(`starlab-core` / `core-wasm` / `blockchain` / `types`), the CLI (`starlab-cli`,
the conformance oracle + headless automation), the TUI (`starlab-client`), and the
signal server. The consumer GUI products are separate:

- **Desktop app** — `stars-labs/starlab-desktop` (Iced, MIT). It consumes
  `starlab-client` (and its `core::*Manager` + `CoreState` + `HeadlessRunner`) as a
  cross-repo dependency, so `starlab-client` MUST keep that surface `pub`. The
  SD-card air-gap convention is shared: export/import dirs are
  `starlab_export` / `starlab_import` (see `starlab-client`'s `offline_manager.rs`)
  — a mismatch breaks desktop↔TUI air-gap interop.
- **Browser extension** — `stars-labs/starlab-wallet` (FROST + YubiKey +
  sandbox execution); consumes `@stars-labs/core-wasm` + `@stars-labs/types`.

When changing `starlab-client::core` (`WalletManager`, `SessionManager`, `DkgManager`,
`SigningManager`, `OfflineManager`, `ConnectionManager`, `CoreState`) or
`HeadlessRunner`, remember the desktop app depends on it across repos.

## Dependencies

FROST: `frost-core` 3.0, `frost-ed25519` 3.0, `frost-secp256k1-tr` 3.0 (ZCash implementations; secp256k1 is BIP-340 only — the vanilla suite is gone). frost 3.0 still bounds its APIs on `rand_core 0.6`, while our own RNG stack is `rand_core`/`rand_chacha` 0.10 + `getrandom` 0.4 — always pass RNGs via `starlab_core::rng` (`os_rng()`, `ChaCha20Rng`, `FrostRng<R>` bridge), never a direct old `rand_core`.
Threshold ECDSA: `cggmp24` `=0.7.0-alpha.3` (features `curve-secp256k1`, `hd-wallet`, `state-machine`; default `backend-num-bigint`, builds for wasm32) with `cggmp24-keygen` / `paillier-zk` pinned `=0.7.0-alpha.3` (alpha.4 breaks alpha.3); `round-based` / `hd-wallet` / `generic-ec` are used through cggmp24's re-exports; `ciborium` (CBOR) encodes protocol messages. cggmp24 also takes `rand_core 0.6` RNGs — pass `starlab_core::rng` ones. The root `Cargo.toml` optimizes the bigint/Paillier crates in dev/test profiles (otherwise the ECDSA tests take >10 min).
Crypto: `sha2`, `sha3`, `k256`, `aes-gcm`, `argon2`, `pbkdf2` (keystore KDF — used in both `starlab-client::keystore::encryption` and `starlab-core::keystore`), `hkdf` (root-secret expansion in `starlab-core`), `hmac` (both HKDF and BIP-32-style HD derivation in `starlab-core`'s `hd_derivation.rs`). No direct `ed25519-dalek` — ed25519 curve ops go through `frost-ed25519` which pulls `curve25519-dalek` transitively.
Dev environment: Nix flake (`nix develop`) provides all system deps including graphics libs.

## Workspace Layout

```
Cargo.toml              # Workspace root, resolver = "2"
package.json            # Bun monorepo (core-wasm + types packages)
flake.nix               # Nix dev environment (Linux + macOS)
apps/
  tui/                  # Crate `starlab-client`: binary `starlab-tui` + library (lib reused by starlab-desktop)
  cli/                  # Headless CLI (crate starlab-cli) — conformance oracle
  signal-server/        # server/ + cloudflare-worker/
packages/@starlab/
  core/                 # Crate `starlab-core` — core crypto library
  core-wasm/            # WASM bindings
  blockchain/           # Chain integrations
  types/                # Shared TypeScript types (Bun workspace only, not in Cargo)
```
