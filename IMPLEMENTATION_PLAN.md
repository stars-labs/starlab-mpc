# Unified networked DKG — one wallet, all chains (ETH+BTC+SOL+Sui)

**Goal:** the tui-node networked (WebRTC mesh) DKG produces a wallet holding BOTH
ed25519 + secp256k1 keys from a single root secret, so one wallet shows
Ethereum/Bitcoin (secp256k1) + Solana/Sui (ed25519). Driven by frost-core's
`UnifiedDkg` (crypto already implemented; the browser/core-wasm already uses it).
The desktop (starlab-desktop) consumes the multi-curve wallet and shows all chains.

**Why it's an engine feature:** `protocal/dkg.rs` runs generic single-ciphersuite
`frost_core::keys::dkg::part1::<C>` — one curve per ceremony. `UnifiedDkg` is
concrete (ed25519+secp256k1) and isn't wired into the mesh transport.

**Reference:** `packages/@frost-mpc/core-wasm/src/lib.rs` (UnifiedDkg over the
browser mesh) defines the wire shape we must match for cross-client interop:
`UnifiedRound1Package { ed25519, secp256k1 }`, `UnifiedRound2Packages`.

---

## Stage 1: keystore + AppState plumbing
**Goal:** persist + load a "unified" wallet (same `wallet_id` in both
`device/ed25519/` and `device/secp256k1/`); hold UnifiedDkg state on AppState.
**Changes:** AppState gets `unified_dkg: Option<UnifiedDkg>` (concrete, not `C`).
Keystore helper `save_unified_wallet(wallet_id, ed_keypkg, secp_keypkg, …)` that
writes two `WalletMetadata` under one id (storage already supports per-curve dirs).
**Test:** keystore round-trip — save a unified wallet, reload, both curves present
with matching group keys.
**Status:** Not Started

## Stage 2: networked UnifiedDkg driver
**Goal:** `protocal/unified_dkg_net.rs` mirroring `protocal/dkg.rs`:
trigger_round1 (`UnifiedDkg::generate_round1` → broadcast `UnifiedRound1Package`),
process_round1, trigger_round2, process_round2, finalize → both key packages →
`save_unified_wallet`.
**Test:** in-process 2-of-2 (simulate-style) — both nodes agree on both curves'
group keys; persisted wallet has ed25519+secp256k1.
**Status:** Not Started

## Stage 3: wire protocol + create flow
**Goal:** data-channel messages for the unified rounds (match core-wasm shape);
`HeadlessCreateWallet` drives the unified path (a `unified` mode flag, default for
new wallets). CLI `wallet create` + `session join` do unified.
**Test:** cross-process (ceremony.sh-style) unified DKG over a local signal server
→ wallet with ETH+BTC+SOL+Sui addresses; all nodes agree.
**Status:** Not Started

## Stage 4: desktop UI — show all chains
**Goal:** starlab-desktop groups wallet entries by `wallet_id` and renders every
chain address (ETH, BTC, Solana, Sui) per wallet; Create uses the unified path.
**Test:** launch desktop, run a unified DKG, screenshot showing one wallet with
all four chains.
**Status:** Not Started

## Stage 5 (optional): cross-client interop + cleanup
**Goal:** desktop ↔ browser ↔ CLI run a unified DKG together (wire-shape parity);
update CLAUDE.md/docs; remove this plan file.
**Status:** Not Started
