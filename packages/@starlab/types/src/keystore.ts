// ===================================================================
// KEYSTORE TYPES - FROST KEY SHARE STORAGE
// ===================================================================
//
// This module defines types for securely storing FROST key shares
// and wallet metadata in the browser extension, aligned with CLI format.
// ===================================================================

/**
 * Blockchain information for multi-chain support (matches CLI BlockchainInfo)
 */
export interface BlockchainInfo {
    /** Blockchain identifier (e.g., "ethereum", "bsc", "polygon", "solana") */
    blockchain: string;
    
    /** Network type (e.g., "mainnet", "testnet", "devnet") */
    network: string;
    
    /** Chain ID for EVM-compatible chains */
    chain_id?: number;
    
    /** Address on this blockchain */
    address: string;
    
    /** Address format/encoding (e.g., "EIP-55", "base58", "bech32") */
    address_format: string;
    
    /** Whether this blockchain is actively used */
    enabled: boolean;
    
    /** Optional custom RPC endpoint */
    rpc_endpoint?: string;
    
    /** Additional metadata specific to this blockchain */
    metadata?: any;
}

/**
 * Wallet metadata embedded in CLI keystore files (matches CLI WalletMetadata)
 */
export interface WalletMetadata {
    /** Unique identifier for this wallet */
    wallet_id: string;
    
    /** Device ID that owns this key share */
    device_id: string;
    
    /** User-friendly device name */
    device_name: string;
    
    /** Type of cryptographic curve used ("secp256k1" or "ed25519") */
    curve_type: string;
    
    /** List of blockchains supported by this wallet */
    blockchains: BlockchainInfo[];
    
    /** Legacy fields for backward compatibility */
    blockchain?: string;
    public_address?: string;
    
    /** Minimum number of participants required to sign */
    threshold: number;
    
    /** Total number of participants */
    total_participants: number;
    
    /** This device's participant index/identifier */
    participant_index: number;
    
    /** Serialized group public key */
    group_public_key: string;
    
    /** ISO 8601 timestamp when created */
    created_at: string;
    
    /** ISO 8601 timestamp when last modified */
    last_modified: string;
    
    /** User-defined tags */
    tags: string[];
    
    /** Optional description */
    description?: string;
}

/**
 * Self-contained wallet file format (matches CLI WalletFile)
 */
export interface WalletFile {
    /** Format version */
    version: string;
    
    /** Whether the data is encrypted */
    encrypted: boolean;
    
    /** Encryption algorithm used */
    algorithm: string;
    
    /** Base64-encoded encrypted data */
    data: string;
    
    /** Embedded metadata */
    metadata: WalletMetadata;
}

/**
 * Core FROST key share data for extension operations
 */
export interface KeyShareData {
    // Core FROST key material
    key_package: string; // Serialized FROST KeyPackage (encrypted)
    group_public_key: string; // The group's public key
    
    // Session information
    session_id: string; // DKG session identifier
    device_id: string; // This device's identifier in the group
    participant_index: number; // This participant's index (1-based)
    
    // Threshold configuration
    threshold: number; // Required signers (t)
    total_participants: number; // Total participants (n)
    participants: string[]; // List of all participant device IDs
    
    // Blockchain specific
    curve: 'secp256k1' | 'ed25519'; // Ethereum or Solana
    blockchains: BlockchainInfo[]; // Multi-chain support
    
    // Legacy support
    ethereum_address?: string; // Derived Ethereum address
    solana_address?: string; // Derived Solana address
    
    // Metadata
    created_at: number; // Timestamp
    last_used?: number; // Last signing operation
    backup_date?: number; // Last backup timestamp
}

/**
 * Extension-specific wallet metadata for UI
 */
export interface ExtensionWalletMetadata {
    id: string; // Unique wallet ID (matches account ID)
    name: string; // User-friendly name
    blockchain: string; // Primary blockchain
    address: string; // The primary address
    session_id: string; // Links to KeyShareData

    // Visual
    color?: string; // For UI identification
    icon?: string; // Custom icon

    // Status
    isActive: boolean; // Whether this wallet is currently usable
    hasBackup: boolean; // Whether user has backed up this wallet

    // Timing
    /** Unix timestamp (ms) when this wallet was added. Optional for
     *  backward-compat with older stored metadata that predates the
     *  field — code should default to `wallet.createdAt ?? 0`. */
    createdAt?: number;
    /** Unix timestamp (ms) of the last time this wallet was used
     *  for a signing operation. Updates lazily on signingComplete. */
    lastUsed?: number;
}

/**
 * Keystore index for managing multiple wallets
 */
export interface KeystoreIndex {
    version: string; // Keystore format version
    wallets: ExtensionWalletMetadata[]; // All wallets
    activeWalletId?: string; // Currently selected wallet
    device_id: string; // This device's global identifier
    
    // Security
    isEncrypted: boolean; // Whether key shares are encrypted
    encryptionMethod?: 'password' | 'biometric' | 'none';
    lastModified: number;
}

/**
 * Encrypted key share (extension format)
 */
export interface EncryptedKeyShare {
    walletId: string;
    algorithm: 'AES-GCM'; // Encryption algorithm
    salt: string; // Salt for key derivation (base64)
    iv: string; // Initialization vector (base64)
    ciphertext: string; // Encrypted KeyShareData (base64)
    authTag?: string; // Authentication tag for GCM (base64)
}

/**
 * Keystore backup format
 */
export interface KeystoreBackup {
    version: string;
    device_id: string;
    exportedAt: number;
    wallets: Array<{
        metadata: ExtensionWalletMetadata;
        encryptedShare: EncryptedKeyShare;
    }>;
}

/**
 * CLI-compatible keystore backup (matches CLI ExtensionKeystoreBackup)
 */
export interface CLIKeystoreBackup {
    version: string;
    device_id: string;
    exported_at: number;
    wallets: Array<{
        metadata: WalletMetadata;
        encrypted_share: {
            wallet_id: string;
            algorithm: string;
            salt: string;
            iv: string;
            ciphertext: string;
            auth_tag?: string;
        };
    }>;
}

// Key derivation parameters (similar to CLI implementation)
export interface KeyDerivationParams {
    algorithm: 'argon2id' | 'pbkdf2';
    salt: Uint8Array;
    iterations?: number; // For PBKDF2
    memory?: number; // For Argon2
    parallelism?: number; // For Argon2
    keyLength: number; // Output key size
}

// Session info for creating new accounts
export interface NewAccountSession {
    session_id: string;
    name: string;
    blockchain: 'ethereum' | 'solana';
    threshold: number;
    total_participants: number;
    participants: string[];
    status: 'proposing' | 'waiting_acceptance' | 'dkg_in_progress' | 'completed' | 'failed';
    created_at: number;
}

// Events emitted by keystore
export interface KeystoreEvent {
    type: 'wallet_added' | 'wallet_removed' | 'wallet_updated' | 'keystore_locked' | 'keystore_unlocked';
    walletId?: string;
    timestamp: number;
}