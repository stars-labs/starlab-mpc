# Simplified Keystore Design (KISS & Orthogonal)

## Design Philosophy

The keystore follows **KISS** (Keep It Simple, Stupid) and **Orthogonal** principles:
- **Single Source of Truth**: The FROST group public key determines all blockchain addresses
- **No Redundancy**: Don't store data that can be derived
- **Minimal Storage**: Only store essential cryptographic material

## Core Wallet Metadata (Essential Fields Only)

```rust
pub struct WalletMetadata {
    // === Identity ===
    session_id: String,           // Wallet identifier
    device_id: String,            // Which device owns this share
    
    // === Cryptography ===
    curve_type: String,           // "secp256k1" or "ed25519"
    group_public_key: String,     // FROST public key (source of truth)
    
    // === Threshold ===
    threshold: u16,               // K in K-of-N
    total_participants: u16,      // N in K-of-N
    participant_index: u16,       // This device's index (1-based)
    
    // === Timestamps ===
    created_at: String,           // ISO 8601
    last_modified: String,        // ISO 8601
}
```

## Address Derivation (Not Stored)

All blockchain addresses are **deterministically derived** from:
- `group_public_key` + `curve_type`

### For secp256k1 curve:
- **Ethereum**: Keccak256 hash of public key, last 20 bytes
- **All EVM chains**: Same address (Polygon, BSC, Arbitrum, etc.)
- **Bitcoin**: Different derivation but same source key

### For ed25519 curve:
- **Solana**: Base58 encoding of the public key
- **Other ed25519 chains**: Chain-specific encoding

## On-disk file layout

Each wallet lives as a **single JSON file** per wallet under
`~/.frost_keystore/<device_id>/<curve>/<wallet_id>.json`. The file
serializes the `WalletFile` struct
(`apps/tui-node/src/keystore/models.rs:438-453`), which wraps
plaintext metadata and the base64-encoded encrypted share in one
document. The serializer is `save_wallet_file_v2_with_method` in
`src/keystore/storage.rs:216-247`.

Earlier revisions of this doc claimed the opposite — a two-file
split with `<wallet_id>.json` carrying only metadata + a separate
`<wallet_id>.dat` blob holding `salt | nonce | ciphertext | tag`,
and explicitly asserted "no single-JSON-with-embedded-base64-blob
format" existed. That retraction ran in the wrong direction: the
single-JSON-with-embedded-blob format is exactly what ships. There
is no `.dat` file written to disk, and `save_wallet_file_v2_*`
only calls `File::create(wallet_path)` once, where `wallet_path` is
the `<wallet_id>.json` path.

`<wallet_id>.json` example (real shape):

```json
{
  "version": "2.0",
  "encrypted": true,
  "algorithm": "AES-256-GCM-Argon2id",
  "data": "<base64 ciphertext — salt+nonce+ct+tag framing is
           internal to encrypt_data_with_method, NOT visible here>",
  "metadata": {
    "session_id": "company-wallet-2of3",
    "device_id": "alice-laptop",
    "curve_type": "secp256k1",
    "threshold": 2,
    "total_participants": 3,
    "participant_index": 1,
    "group_public_key": "frost_public_key_hex",
    "participants": ["alice-laptop", "bob-desktop", "charlie-phone"],
    "created_at": "2025-06-27T12:00:00Z",
    "last_modified": "2025-06-27T12:00:00Z"
  }
}
```

Real metadata field names come from `WalletMetadata` at
`src/keystore/models.rs:222-273`. Notable details (verified
against source — earlier drafts of THIS note had the inversions
backwards):

- Canonical field is `session_id`; `wallet_id` is a
  `#[serde(alias)]` for backward-compat reads only.
- `created_at` + `last_modified` are ISO-8601 **strings**, NOT
  `u64` unix-timestamps. An earlier retraction here claimed the
  reverse — wrong; the struct field is `pub created_at: String`
  and the `///` doc comment says "ISO 8601 timestamp".
- Participant roster is `participants: Vec<String>` of
  device_ids, NOT a `devices: Vec<DeviceInfo>` array. An earlier
  retraction here claimed `devices: Vec<DeviceInfo>` is real —
  also wrong; the `DeviceInfo` struct is used elsewhere
  (in the unrelated `WalletInfo` struct at models.rs:56) but
  `WalletMetadata` embedded inside `WalletFile` uses
  `Vec<String>`.
- Legacy `device_name?` + `blockchains` fields exist with
  `#[serde(skip_serializing_if = ...)]` guards and don't appear
  on fresh writes; address derivation uses
  `group_public_key + curve_type` directly via
  `WalletMetadata::derive_ethereum_address` /
  `derive_solana_address`.

## Benefits of This Approach

### 1. **Simplicity**
- Minimal fields to maintain
- Clear purpose for each field
- Easy to understand and debug

### 2. **Consistency**
- Addresses always match the key
- No possibility of address/key mismatch
- Single source of truth

### 3. **Future-Proof**
- New blockchains can be supported without schema changes
- Address formats can evolve without updating stored data
- Chain IDs and networks are runtime concerns

### 4. **Storage Efficiency**
- Smaller file size
- No redundant blockchain arrays
- Faster to read/write

### 5. **Security**
- Less data exposure
- Simpler to audit
- Fewer fields to validate

## Migration from Legacy Format

Legacy wallets with stored blockchain info are automatically supported:
- Old fields are preserved for backward compatibility
- New wallets use simplified format
- Address derivation works for both formats

## API Usage

```rust
// Creating a wallet (blockchain info ignored)
keystore.create_wallet(
    name,
    curve_type,
    threshold,
    total,
    group_public_key,
    key_share_data,
    password,
    participant_index
)

// Getting addresses (derived on-demand)
let metadata = keystore.get_wallet(wallet_id);
let eth_address = metadata.derive_ethereum_address();
let sol_address = metadata.derive_solana_address();

// Or get all supported chains
let addresses = metadata.get_blockchain_addresses();
```

## Implementation Notes

1. **Address derivation** is handled by blockchain-specific modules
2. **Chain configuration** (RPC endpoints, chain IDs) is a runtime concern
3. **User preferences** (which chains to use) are stored separately
4. **Wallet files** remain portable and self-contained

This design ensures the keystore remains simple, maintainable, and focused solely on secure storage of cryptographic material.