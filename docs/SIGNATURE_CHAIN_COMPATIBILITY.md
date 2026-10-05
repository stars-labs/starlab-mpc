# Signing keys and chain compatibility

Each wallet account uses the key and signing protocol required by its chain.
A wallet created with all supported key types has separate ed25519 FROST,
secp256k1 Taproot FROST and cggmp24 ECDSA shares. The two secp256k1 keys use
different signing protocols and are not interchangeable.

| Chain | Key tag | Signing protocol | Account path |
|---|---|---|---|
| Ethereum | `secp256k1-ecdsa` | cggmp24 threshold ECDSA, recoverable `r‖s‖v` | `m/44'/60'/0'/0/n` |
| Bitcoin Taproot | `secp256k1` | FROST BIP-340 Schnorr, P2TR | `m/86'/0'/0'/0/n` |
| Solana | `ed25519` | FROST Ed25519 | `m/44'/501'/n'/0'` |
| Sui | `ed25519` | FROST Ed25519 | `m/44'/784'/n'/0'/0'` |

Ethereum signatures support ordinary EOA verification: message signing hashes
with EIP-191, and transaction signing signs the transaction's prehash. A
Schnorr-verifying smart contract is not required. BSC, Polygon and Avalanche
C-Chain use the same ECDSA address encoding; their presence in the TUI chain
configuration does not imply a complete transaction workflow for each network.
The Taproot key exposes Bitcoin addresses only and does not control an EVM EOA.
Legacy Bitcoin and SegWit-v0 ECDSA spending are not supplied by the Taproot suite.

Canonical chain-to-key selection, public account derivation, address encoding
and signature verification live in
[`accounts.rs`](../packages/@starlab/core/src/accounts.rs). The TUI's additional
EVM address aliases delegate to the same Ethereum encoder. Aptos and NEAR
entries in the TUI configuration are separate from the canonical account API;
the configuration alone does not guarantee signing or transaction support.

FROST share refresh can retain the group public key while changing the
participating devices. ECDSA resharing is not implemented. The existing refresh
flow changes only FROST shares; a mixed wallet’s ECDSA
shares and Ethereum participants remain unchanged. Changing its ECDSA
participants requires a new wallet and moving funds to its new address.
See [`RECOVERY_AND_RESHARING.md`](RECOVERY_AND_RESHARING.md).
