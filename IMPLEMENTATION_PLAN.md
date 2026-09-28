# EVM → threshold ECDSA (cggmp24)

Decision (plan C, 2026-09-27): ed25519 chains and Bitcoin (Taproot) stay on
FROST; EVM accounts move to threshold ECDSA via `cggmp24` (MIT/Apache-2.0,
Kudelski-audited, 0.7.0-alpha). FROST Schnorr signatures can't be verified by
an EOA (`ecrecover`), so today no EVM transaction can be sent.

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
**Status**: Not Started

## Stage 2: CLI (conformance oracle) + wire protocol
**Goal**: `starlab-cli` runs ECDSA DKG (with aux info) and ECDSA signing over
the signal server + WebRTC; new frame kind documented in CLAUDE.md.
**Success Criteria**: CLI e2e green in CI.
**Tests**: real-network e2e: 2-of-3 DKG, EIP-191 sign → ecrecover == address;
EIP-1559 Sepolia tx signed → sender recovered == address (offline check);
two consecutive signings; ceremony with a late joiner.
**Status**: Not Started

## Stage 3: TUI + desktop (engine clients)
**Goal**: wallet creation runs FROST (ed25519, Taproot) + ECDSA; Ethereum
signing uses ECDSA; keystore stores the ECDSA share (encrypted, v2 format,
import/export covered); primes pre-generated in the background.
**Success Criteria**: TUI + desktop (`starlab-desktop`, lock bump) create a
wallet and sign an Ethereum message/tx together.
**Tests**: unit + L3 serve-process tests; live TUI+desktop run.
**Status**: Not Started

## Stage 4: core-wasm + extension (`starlab-wallet`)
**Goal**: WASM bindings for the ECDSA state machines; extension pre-generates
primes in a worker, joins/initiates ECDSA DKG and signing; Send on Sepolia
works.
**Success Criteria**: extension ↔ CLI interop (2-of-3, 3-of-3) green in CI.
**Tests**: bun unit tests (real WASM); interop spec: CLI + extension ECDSA
DKG, co-sign, and extension-initiated sign.
**Status**: Not Started

## Stage 5: Live 3-client Sepolia transaction
**Goal**: TUI + desktop + extension 2-of-3 wallet sends a real Sepolia tx.
**Success Criteria**: tx hash confirmed on Sepolia from the MPC address.
**Tests**: live run with screenshots + tx hash.
**Status**: Not Started
