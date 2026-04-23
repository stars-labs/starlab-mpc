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

Each wallet lives as a pair of files under
`~/.frost_keystore/<device_id>/<curve>/`:

- `<wallet_id>.json` — plaintext metadata (the `WalletMetadata`
  struct above, serialized).
- `<wallet_id>.dat` — raw AES-256-GCM output holding the encrypted
  FROST key share:
  ```
  [ salt (16 B) ][ nonce (12 B) ][ ciphertext ][ GCM auth tag (16 B) ]
  ```

There is **no** single-JSON-with-embedded-base64-blob format
(earlier drafts of this doc showed one, which is Ethereum
keystore-V3 style, not what this codebase uses). The metadata
sidecar and the ciphertext blob stay in separate files so that
non-secret metadata is trivially inspectable without involving a
password.

`<wallet_id>.json` example:

```json
{
  "session_id": "company-wallet-2of3",
  "device_id": "alice-laptop",
  "curve_type": "secp256k1",
  "threshold": 2,
  "total_participants": 3,
  "participant_index": 1,
  "group_public_key": "frost_public_key_hex",
  "participants": ["alice-laptop", "bob-desktop", "charlie-mobile"],
  "created_at": "2024-01-01T00:00:00Z",
  "last_modified": "2024-01-01T00:00:00Z"
}
```

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