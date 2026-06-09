# FROST MPC Keystore Architecture

This document provides a detailed technical overview of the keystore architecture for the FROST MPC CLI Node.

## Design Goals

The keystore architecture is designed with the following goals:

1. **Production-Ready Security**: Strong encryption and secure key management practices
2. **Multi-Account Support**: Users can create and manage multiple wallets
3. **Multi-Device Management**: Support for key shares across different devices
4. **Recovery Mechanisms**: Ability to recover from device loss within threshold limits
5. **User Experience**: Simple interface for key management tasks
6. **Future Extensibility**: Architecture allows for future enhancements

## Component Overview

```
┌──────────────────────────────────────────┐
│                                          │
│                 CLI Node                 │
│                                          │
├──────────────────────────────────────────┤
│                                          │
│                  TUI                     │
│                                          │
├─────────────┬────────────┬───────────────┤
│ FROST       │ WebRTC     │ Keystore      │
│ Protocol    │ Networking │ Management    │
├─────────────┼────────────┼───────────────┤
│                                          │
│              Keystore API                │
│                                          │
└───────────────────┬──────────────────────┘
                    │
       ┌────────────▼─────────────┐
       │                          │
       │     Keystore Storage     │
       │                          │
       └──────────────────────────┘
```

## Keystore API

### Core Classes

1. **`Keystore`**: Main interface for interacting with stored keys
2. **`KeystoreFile<C>`**: Container for encrypted key material
3. **`WalletInfo`**: Metadata for a wallet
4. **`DeviceInfo`**: Metadata for a device

### Key Components

#### `Keystore` Class

The `Keystore` class provides methods for managing wallets and keys:

```rust
pub struct Keystore {
    base_path: PathBuf,
    device_id: String,
    device_name: String,
    index: KeystoreIndex,
}

impl Keystore {
    // Create or load a keystore
    pub fn new(base_path: &str, device_name: &str) -> Result<Self, String>;
    
    // Save the keystore index
    fn save_index(&self) -> Result<(), String>;
    
    // Create a new wallet and add local device as a participant
    pub fn create_wallet<C: Ciphersuite + Serialize>(...) -> Result<String, String>;
    
    // Load a wallet's key material
    pub fn load_wallet<C: Ciphersuite + for<'de> Deserialize<'de>>(...) -> Result<(...), String>;
    
    // List available wallets
    pub fn list_wallets(&self) -> Vec<&WalletInfo>;
    
    // Add another device to a wallet
    pub fn add_device_to_wallet(...) -> Result<(), String>;
    
    // Import a share from another device
    pub fn import_share<C: Ciphersuite + Serialize + for<'de> Deserialize<'de>>(...) -> Result<(), String>;
    
    // Export a share for backup or sharing with another device
    pub fn export_share<C: Ciphersuite + Serialize>(...) -> Result<Vec<u8>, String>;
    
    // Delete a wallet
    pub fn delete_wallet(&mut self, wallet_id: &str) -> Result<(), String>;
    
    // Encrypt/decrypt data
    fn encrypt_data(data: &str, password: &str) -> Result<Vec<u8>, String>;
    fn decrypt_data(encrypted_data: &[u8], password: &str) -> Result<String, String>;
}
```

#### Data Structures

**`KeystoreFile<C>`**: Contains key material and metadata for a specific device's share of a wallet

```rust
#[derive(Serialize, Deserialize)]
struct KeystoreFile<C: Ciphersuite> {
    version: u8,                  // Keystore format version
    wallet_id: String,            // Which wallet this belongs to
    device_id: String,            // Which device this belongs to
    key_package: KeyPackage<C>,   // Device's key share
    identifier_map: BTreeMap<String, Identifier<C>>, // Maps device IDs to identifiers
    created_at: u64,              // When this keystore was created
    last_modified: u64,           // When this keystore was last modified
    metadata: HashMap<String, String>, // Custom metadata
}
```

**`WalletInfo`**: Contains metadata about a wallet

```rust
#[derive(Serialize, Deserialize)]
pub struct WalletInfo {
    wallet_id: String,            // Unique wallet ID (UUID)
    name: String,                 // User-friendly wallet name
    curve_type: String,           // "secp256k1" or "ed25519"
    blockchain: String,           // "ethereum" or "solana"
    public_address: String,       // Public blockchain address
    threshold: u16,               // Required signers
    total_participants: u16,      // Total participants
    created_at: u64,              // Creation timestamp
    group_public_key: String,     // Serialized public key package
    devices: Vec<DeviceInfo>,     // Known devices for this wallet
    tags: Vec<String>,            // User-defined tags
    description: Option<String>,  // Optional wallet description
}
```

**`DeviceInfo`**: Contains metadata about a device

```rust
#[derive(Serialize, Deserialize)]
pub struct DeviceInfo {
    device_id: String,        // Unique device ID
    name: String,             // User-friendly device name
    device_id: String,          // Device ID used in FROST protocol
    identifier: String,       // Serialized FROST identifier 
    last_seen: u64,           // Last connection timestamp
}
```

**`KeystoreIndex`**: Master index of wallets and devices

```rust
#[derive(Serialize, Deserialize)]
struct KeystoreIndex {
    version: u8,
    wallets: Vec<WalletInfo>,
    devices: Vec<DeviceInfo>,
}
```

## Encryption Details

### Algorithm

The keystore uses AES-256-GCM with Argon2id key derivation:

1. **Key Derivation**:
   - Use Argon2id to derive a 32-byte key from the password
   - Use a unique salt for each file

2. **Encryption**:
   - Use AES-256-GCM for authenticated encryption
   - Use a unique nonce for each encryption operation
   - Store the salt and nonce alongside the ciphertext

3. **File Format**:
   - `salt (16 bytes) + nonce (12 bytes) + ciphertext`

## File Structure

### Directory Layout

```
keystore/
├── index.json           # Master index of wallets and devices
├── device_id            # Unique identifier for this device
└── wallets/
    ├── <wallet_id>.key      # Device's key share for wallet
    ├── <wallet_id>_<device_id>.share  # Imported share from another device
    └── ...
```

### File Formats

**index.json**: Contains metadata about all wallets and devices

```json
{
  "version": 1,
  "wallets": [
    {
      "wallet_id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Corporate Treasury",
      "curve_type": "secp256k1",
      "blockchain": "ethereum",
      "public_address": "0x1234...",
      "threshold": 2,
      "total_participants": 3,
      "created_at": 1686123456,
      "group_public_key": "...",
      "devices": [...],
      "tags": ["ethereum", "corporate"],
      "description": "Corporate Treasury wallet"
    }
  ],
  "devices": [...]
}
```

**Wallet Key File**: Encrypted file containing a device's share of a wallet

```
Format: salt + nonce + encrypted(KeystoreFile)
```

**Share File**: Encrypted file containing an imported share from another device

```
Format: salt + nonce + encrypted(KeystoreFile)
```

## Integration with FROST MPC CLI Node

### CLI Command Flow

1. **Command Entry**: User enters command in TUI
2. **Command Parsing**: TUI parses command and creates an `InternalCommand`
3. **Command Handling**: Command handler calls appropriate `Keystore` method
4. **State Update**: Application state is updated with loaded key material
5. **Feedback**: Results are displayed in the TUI log

### Key Material Usage

When a wallet is loaded:

1. The `KeyPackage` is stored in `AppState.key_package`
2. The `PublicKeyPackage` is stored in `AppState.group_public_key`
3. The identifier map is stored in `AppState.identifier_map`
4. The DKG state is set to `DkgState::Complete`

This allows the application to use the loaded keys for signing operations without repeating the DKG process.

## Security Considerations

### Threat Model

The keystore is designed to protect against the following threats:

1. **Device Compromise**: Encrypted storage protects keys even if a device is compromised
2. **Malware**: Password protection prevents automated key extraction
3. **Insider Threats**: Threshold requirements prevent single-party attacks
4. **Physical Theft**: Encrypted storage protects keys on stolen devices

### Mitigations

1. **Strong Encryption**: AES-256-GCM with Argon2id key derivation
2. **Threshold Security**: No single point of compromise
3. **Minimal Key Exposure**: Keys are decrypted only when needed
4. **Password Protection**: All sensitive data requires a password

## Future Enhancements

The architecture is designed to support these future enhancements:

1. **Key Rotation**: Ability to refresh shares while maintaining the same public key
2. **Hierarchical Wallets**: Support for HD wallet structures with derived keys
3. **Hardware Security Module Integration**: Support for HSM storage of shares
4. **Multi-party Recovery**: Advanced protocols for secure recovery options
5. **Auditing and Compliance Tools**: Logging and verification for enterprise use

## Glossary

- **DKG**: Distributed Key Generation - the process of generating key shares across devices
- **FROST**: Flexible Round-Optimized Schnorr Threshold signatures
- **Key Package**: Contains a device's share of the signing key
- **Group Public Key**: The public key for the wallet, visible on blockchain
- **Share**: A portion of a signing key held by a participant
- **Threshold**: Minimum number of participants needed to sign
- **Wallet**: A collection of key shares that can sign for one blockchain address