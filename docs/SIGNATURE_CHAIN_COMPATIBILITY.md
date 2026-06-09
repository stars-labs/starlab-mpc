# Signature Scheme ↔ Chain Compatibility

**Status:** Authoritative caveat (matches shipped code)
**Scope:** which chains can actually **verify** a FROST threshold signature, and the sharp exception for standard Ethereum-family EOAs.
**Code:** `apps/tui/src/blockchain_config.rs` (chain↔curve table + `signing_caveat`), `packages/@starlab/blockchain/`

---

## TL;DR

FROST produces **Schnorr** signatures.

- **Bitcoin (Taproot / BIP-340)** and **ed25519 chains (Solana, Sui, Aptos, NEAR)**
  verify them natively. ✅
- A **standard Ethereum-family EOA** (Ethereum, BSC, Polygon, Avalanche C-Chain)
  expects **ECDSA**. Our Schnorr signature will **not** be accepted by a plain
  externally-owned-account transaction. ❌ — EVM usage needs a **smart-contract
  account** (ERC-4337 or a Schnorr-verifying contract).

A secp256k1 FROST wallet can *display* a correct-looking `0x…` address, but that
does not mean a normal EVM transfer from it will verify. Do not imply otherwise
in any UI.

---

## 1. The compatibility table

| Chain | Curve | Sig scheme the chain verifies | FROST signature accepted? |
|---|---|---|:--:|
| Bitcoin (Taproot / P2TR, BIP-340) | secp256k1 | Schnorr | ✅ native |
| Bitcoin (legacy/SegWit-v0 spends) | secp256k1 | ECDSA | ❌ (key-path Taproot only) |
| Solana | ed25519 | Ed25519 (Schnorr) | ✅ native |
| Sui | ed25519 | Ed25519 | ✅ native |
| Aptos | ed25519 | Ed25519 | ✅ native |
| NEAR | ed25519 | Ed25519 | ✅ native |
| Ethereum — **EOA** | secp256k1 | **ECDSA** (secp256k1) | ❌ |
| BSC / Polygon / Avalanche C — **EOA** | secp256k1 | **ECDSA** | ❌ |
| Ethereum-family — **contract account** | secp256k1 | whatever the contract verifies | ✅ *if* the contract verifies Schnorr |

### Why the EVM EOA row is ❌

FROST = threshold **Schnorr**. Ethereum's base protocol authenticates an EOA
transaction with **ECDSA over secp256k1** (`ecrecover`). Same curve, **different
signature algorithm** — an ECDSA verifier rejects a Schnorr signature. The
address derivation is fine (`keccak(X‖Y)[12..]`, see code), so the *receive
address* is real and can hold funds; what fails is **spending** via a normal EOA
transaction, because the signature type doesn't match.

---

## 2. The path that works on EVM: contract accounts

To use a FROST secp256k1 wallet on an EVM chain, route through a **smart-contract
account** rather than an EOA:

- **ERC-4337 (account abstraction).** The account is a contract; a `UserOperation`
  is validated by *your* `validateUserOp` logic, which can verify a Schnorr
  signature (or a custom threshold scheme) instead of relying on `ecrecover`.
- **A Schnorr-verifier contract / module.** A deployed verifier (e.g. a Safe
  module or a bespoke contract) checks the BIP-340-style Schnorr signature on
  chain and authorizes the action.

In both cases the **funds and identity live at the contract address**, not at the
EOA derived directly from the group key.

### Status in this repo

Address derivation for EVM chains is implemented and correct
(`generate_address_for_chain`). An on-chain ERC-4337 / Schnorr-verifier
integration is **not** shipped. Until it is, treat EVM-EOA spending as **out of
scope**: the wallet can receive and display an EVM address, but signing a
standard EOA transaction that the base protocol will accept is not supported.

---

## 3. What the UI must communicate

When a user selects an **EVM chain** (`ethereum` / `bsc` / `polygon` /
`avalanche`) for a **secp256k1** wallet, surface the caveat at chain-selection
and/or signing time:

> ⚠️ This is a threshold-**Schnorr** wallet. A standard Ethereum-family EOA
> transaction verifies with **ECDSA** and will not accept this signature. EVM use
> requires a smart-contract account (ERC-4337 / a Schnorr-verifier contract).
> Receiving to the displayed address is fine; spending via a normal EOA
> transaction is not supported.

A single source of truth for this string lives in code as
`blockchain_config::signing_caveat(chain)` (Rust) so the TUI/native/CLI share one
message; the extension mirrors it in its chain config. **No UI may claim a
standard EVM EOA transfer is supported** for a FROST wallet.

---

## 4. Cross-references

- Chain↔curve table and the caveat helper: `apps/tui/src/blockchain_config.rs`.
- Address derivation (correct for all listed chains): same file,
  `generate_address_for_chain`.
- Why a secp256k1 and an ed25519 wallet are different keys (not one key across
  chains): [`MULTI_CURVE_DERIVATION.md`](MULTI_CURVE_DERIVATION.md).
