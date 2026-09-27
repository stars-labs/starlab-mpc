# Bitcoin → Taproot (secp256k1 FROST becomes BIP-340)

Decision (plan C): ed25519 chains + Bitcoin stay on FROST; EVM moves to
threshold ECDSA (cggmp24, separate plan). Vanilla FROST secp256k1 signatures
verify on neither EVM (needs ECDSA) nor Bitcoin P2WPKH (needs ECDSA), so the
`secp256k1` FROST ciphersuite becomes `frost-secp256k1-tr` (BIP-340) for good.
Not launched → no compatibility layer; wire/keystore format changes land in
all clients together.

Key model: one secp256k1 DKG (tr's `post_dkg` applies the BIP-341 no-script
tweak to the root). An account child is derived with the existing additive
offsets and then BIP-86-tweaked (`Q = P + H_TapTweak(P)·G`) at derivation
time, so the stored child key IS the P2TR output key: plain FROST signing
yields BIP-340 signatures valid for its `bc1p…` address. Public-only address
derivation applies the same tweak. Path: `m/86'/0'/0'/0/n`.

## Stage 1: Core ciphersuite + accounts
**Goal**: `starlab-core` uses `frost-secp256k1-tr` for `secp256k1`; Bitcoin
accounts derive P2TR (bech32m) addresses; child keys are BIP-86 tweaked.
**Success Criteria**: duplicate `secp256k1_tr` module removed; P2WPKH code gone.
**Tests**: BIP-86 output-key/address vector (cross-checked with the `bitcoin`
crate); t-of-n threshold signature by an account child verifies as BIP-340
against the x-only key encoded in its address (k256 schnorr); public-only
derivation == full derivation; existing DKG / reshare / HD tests pass.
**Status**: Complete

## Stage 2: Engine clients (core-wasm, TUI, CLI)
**Goal**: `FrostDkgSecp256k1` (same JS name) runs Taproot; `FrostDkgSecp256k1Tr`
removed; TUI/CLI hash by chain (Bitcoin signs the raw 32-byte sighash, EVM
keeps EIP-191).
**Success Criteria**: workspace builds for native + wasm32.
**Tests**: all unit tests; the 4 CLI e2e suites; CLI sign of a 32-byte hex
sighash on a Bitcoin account verifies as BIP-340 against its P2TR address.
**Status**: Complete

## Stage 3: Cross-repo clients
**Goal**: extension (vendored core-wasm) and desktop build against the new
engine; Bitcoin shows `bc1p…` everywhere.
**Success Criteria**: TUI + desktop + extension agree on the P2TR address.
**Tests**: extension `bun test` + check; desktop build; live 3-client DKG.
**Status**: Not Started

## Stage 4: Docs + cleanup
**Goal**: CLAUDE.md / docs describe the Taproot model; no stale P2WPKH or
vanilla-secp256k1 references.
**Status**: Complete (engine repo; archive/ docs left as history)
