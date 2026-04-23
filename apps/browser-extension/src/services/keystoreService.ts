// ===================================================================
// KEYSTORE SERVICE - FROST KEY SHARE MANAGEMENT
// ===================================================================
//
// This service manages the secure storage of FROST key shares for
// multiple accounts. Each account represents a separate DKG session
// with its own key material. The service handles encryption, 
// persistence, and recovery of key shares.
// ===================================================================

import { storage } from "#imports";
import type { 
    KeyShareData, 
    WalletMetadata,
    ExtensionWalletMetadata, 
    KeystoreIndex, 
    EncryptedKeyShare,
    KeystoreBackup,
    CLIKeystoreBackup,
    WalletFile,
    BlockchainInfo,
    NewAccountSession
} from "@mpc-wallet/types/keystore";

export class KeystoreService {
    private static instance: KeystoreService;
    private static testMode: boolean = false;
    private keystoreIndex: KeystoreIndex | null = null;
    private keyShares: Map<string, KeyShareData> = new Map();
    private password: string | null = null;
    private isUnlocked: boolean = false;
    
    private readonly STORAGE_PREFIX = "mpc_keystore";
    private readonly INDEX_KEY = `${this.STORAGE_PREFIX}_index`;
    private readonly SHARES_KEY_PREFIX = `${this.STORAGE_PREFIX}_share_`;
    
    private constructor() {
        this.loadKeystoreIndex();
    }
    
    public static getInstance(): KeystoreService {
        if (!KeystoreService.instance) {
            KeystoreService.instance = new KeystoreService();
        }
        return KeystoreService.instance;
    }
    
    /**
     * Reset the singleton instance (for testing only)
     */
    public static resetInstance(): void {
        if (KeystoreService.instance) {
            KeystoreService.instance.lock();
            KeystoreService.instance.keyShares.clear();
            KeystoreService.instance.keystoreIndex = null;
        }
        KeystoreService.instance = null as any;
        KeystoreService.testMode = true;
    }
    
    /**
     * Initialize or create keystore
     */
    public async initialize(deviceId: string): Promise<void> {
        if (!this.keystoreIndex) {
            this.keystoreIndex = {
                version: "1.0.0",
                wallets: [],
                device_id: deviceId,
                isEncrypted: true,
                encryptionMethod: 'password',
                lastModified: Date.now()
            };
            await this.saveKeystoreIndex();
        }
    }
    
    /**
     * Check if keystore is locked
     */
    public isLocked(): boolean {
        return !this.isUnlocked;
    }
    
    /**
     * Unlock keystore with password
     */
    public async unlock(password: string): Promise<boolean> {
        try {
            this.password = password;
            
            // Try to decrypt one key share to verify password
            if (this.keystoreIndex && this.keystoreIndex.wallets.length > 0) {
                const firstWallet = this.keystoreIndex.wallets[0];
                await this.loadKeyShare(firstWallet.id);
            }
            
            this.isUnlocked = true;
            return true;
        } catch (error) {
            console.error("[KeystoreService] Failed to unlock:", error);
            this.password = null;
            this.isUnlocked = false;
            return false;
        }
    }
    
    /**
     * Lock keystore
     */
    public lock(): void {
        this.password = null;
        this.isUnlocked = false;
        this.keyShares.clear();
    }
    
    /**
     * Add a new wallet after DKG completion
     */
    public async addWallet(
        walletId: string,
        keyShareData: KeyShareData,
        metadata: ExtensionWalletMetadata
    ): Promise<void> {
        if (!this.isUnlocked || !this.password) {
            throw new Error("Keystore is locked");
        }
        
        // Encrypt and save key share
        const encrypted = await this.encryptKeyShare(walletId, keyShareData);
        await this.saveEncryptedShare(walletId, encrypted);
        
        // Update index
        if (!this.keystoreIndex) {
            throw new Error("Keystore not initialized");
        }
        
        this.keystoreIndex.wallets.push(metadata);
        this.keystoreIndex.lastModified = Date.now();
        await this.saveKeystoreIndex();
        
        // Cache decrypted share
        this.keyShares.set(walletId, keyShareData);
        
        console.log("[KeystoreService] Added wallet:", walletId);
    }
    
    /**
     * Import wallet from CLI-compatible format
     */
    public async importCLIWallet(walletFile: WalletFile, password: string): Promise<void> {
        if (!this.isUnlocked) {
            throw new Error("Keystore is locked");
        }
        
        const cliMetadata = walletFile.metadata;
        const walletId = cliMetadata.wallet_id;
        
        // Check if wallet already exists
        if (this.getWallet(walletId)) {
            console.warn("[KeystoreService] Wallet already exists:", walletId);
            return;
        }
        
        // Decrypt the CLI wallet data using CLI password
        const decryptedData = await this.decryptCLIData(walletFile.data, password);
        const cliKeyData = JSON.parse(decryptedData);
        
        // Convert CLI data to extension format
        const keyShareData: KeyShareData = {
            key_package: cliKeyData.key_package || '',
            group_public_key: cliMetadata.group_public_key,
            session_id: cliMetadata.wallet_id,
            device_id: cliMetadata.device_id,
            participant_index: cliMetadata.participant_index,
            threshold: cliMetadata.threshold,
            total_participants: cliMetadata.total_participants,
            participants: [cliMetadata.device_id], // Only this device known from CLI
            curve: cliMetadata.curve_type as 'secp256k1' | 'ed25519',
            blockchains: cliMetadata.blockchains,
            ethereum_address: cliMetadata.blockchains.find(b => b.blockchain === 'ethereum')?.address,
            solana_address: cliMetadata.blockchains.find(b => b.blockchain === 'solana')?.address,
            created_at: new Date(cliMetadata.created_at).getTime()
        };
        
        // Convert to extension metadata
        const primaryBlockchain = cliMetadata.blockchains.find(b => b.enabled) || cliMetadata.blockchains[0];
        const extensionMetadata: ExtensionWalletMetadata = {
            id: walletId,
            name: walletId,
            blockchain: primaryBlockchain?.blockchain || 'ethereum',
            address: primaryBlockchain?.address || '',
            session_id: cliMetadata.wallet_id,
            isActive: true,
            hasBackup: true
        };
        
        // Re-encrypt with extension password
        const encrypted = await this.encryptKeyShare(walletId, keyShareData);
        await this.saveEncryptedShare(walletId, encrypted);
        
        // Update index
        if (!this.keystoreIndex) {
            throw new Error("Keystore not initialized");
        }
        
        this.keystoreIndex.wallets.push(extensionMetadata);
        this.keystoreIndex.lastModified = Date.now();
        await this.saveKeystoreIndex();
        
        // Cache decrypted share
        this.keyShares.set(walletId, keyShareData);
        
        console.log("[KeystoreService] Imported CLI wallet:", walletId);
    }
    
    /**
     * Export wallet in CLI-compatible format
     */
    public async exportCLIWallet(walletId: string): Promise<WalletFile> {
        if (!this.isUnlocked || !this.keystoreIndex) {
            throw new Error("Keystore is locked");
        }
        
        const metadata = this.getWallet(walletId);
        if (!metadata) {
            throw new Error("Wallet not found");
        }
        
        const keyShareData = await this.getKeyShare(walletId);
        if (!keyShareData) {
            throw new Error("Key share not found");
        }
        
        // Create CLI-compatible metadata
        const cliMetadata: WalletMetadata = {
            wallet_id: walletId,
            device_id: keyShareData.device_id,
            device_name: keyShareData.device_id,
            curve_type: keyShareData.curve,
            blockchains: keyShareData.blockchains || [],
            threshold: keyShareData.threshold,
            total_participants: keyShareData.total_participants,
            participant_index: keyShareData.participant_index,
            group_public_key: keyShareData.group_public_key,
            created_at: new Date(keyShareData.created_at).toISOString(),
            last_modified: new Date().toISOString(),
            tags: [],
            description: `Exported from Chrome extension on ${new Date().toISOString()}`
        };
        
        // Prepare CLI key data
        const cliKeyData = {
            key_package: keyShareData.key_package,
            group_public_key: keyShareData.group_public_key,
            session_id: keyShareData.session_id,
            device_id: keyShareData.device_id,
        };
        
        // Encrypt with current extension password (CLI will need to re-encrypt with their password)
        const encryptedData = await this.encryptCLIData(JSON.stringify(cliKeyData));
        
        return {
            version: "2.0",
            encrypted: true,
            algorithm: "AES-256-GCM",
            data: encryptedData,
            metadata: cliMetadata
        };
    }
    
    /**
     * Get key share for a wallet
     */
    public async getKeyShare(walletId: string): Promise<KeyShareData | null> {
        if (!this.isUnlocked) {
            throw new Error("Keystore is locked");
        }
        
        // Check cache first
        if (this.keyShares.has(walletId)) {
            return this.keyShares.get(walletId)!;
        }
        
        // Load and decrypt
        try {
            const keyShare = await this.loadKeyShare(walletId);
            if (keyShare) {
                this.keyShares.set(walletId, keyShare);
            }
            return keyShare;
        } catch (error) {
            console.error("[KeystoreService] Failed to load key share:", error);
            return null;
        }
    }
    
    /**
     * Get all wallet metadata
     */
    public getWallets(): ExtensionWalletMetadata[] {
        return this.keystoreIndex?.wallets || [];
    }
    
    /**
     * Get wallet metadata by ID
     */
    public getWallet(walletId: string): ExtensionWalletMetadata | null {
        return this.keystoreIndex?.wallets.find(w => w.id === walletId) || null;
    }
    
    /**
     * Remove a wallet
     */
    public async removeWallet(walletId: string): Promise<void> {
        if (!this.keystoreIndex) return;
        
        // Remove from index
        this.keystoreIndex.wallets = this.keystoreIndex.wallets.filter(
            w => w.id !== walletId
        );
        this.keystoreIndex.lastModified = Date.now();
        await this.saveKeystoreIndex();
        
        // Remove encrypted share
        await storage.removeItem(`local:${this.SHARES_KEY_PREFIX}${walletId}`);
        
        // Remove from cache
        this.keyShares.delete(walletId);
    }
    
    /**
     * Export wallet for backup
     */
    public async exportWallet(walletId: string): Promise<KeystoreBackup> {
        if (!this.isUnlocked || !this.keystoreIndex) {
            throw new Error("Keystore is locked");
        }
        
        const metadata = this.getWallet(walletId);
        if (!metadata) {
            throw new Error("Wallet not found");
        }
        
        const encryptedShare = await this.loadEncryptedShare(walletId);
        if (!encryptedShare) {
            throw new Error("Encrypted share not found");
        }
        
        return {
            version: this.keystoreIndex.version,
            device_id: this.keystoreIndex.device_id,
            exportedAt: Date.now(),
            wallets: [{
                metadata,
                encryptedShare
            }]
        };
    }
    
    /**
     * Import wallet from backup
     */
    public async importWallet(backup: KeystoreBackup, password: string): Promise<void> {
        if (!this.isUnlocked) {
            throw new Error("Keystore is locked");
        }
        
        for (const wallet of backup.wallets) {
            // Check if wallet already exists
            if (this.getWallet(wallet.metadata.id)) {
                console.warn("[KeystoreService] Wallet already exists:", wallet.metadata.id);
                continue;
            }
            
            // Save encrypted share
            await this.saveEncryptedShare(wallet.metadata.id, wallet.encryptedShare);
            
            // Try to decrypt to verify password
            try {
                await this.loadKeyShare(wallet.metadata.id);
                
                // Add to index
                this.keystoreIndex!.wallets.push(wallet.metadata);
                this.keystoreIndex!.lastModified = Date.now();
                await this.saveKeystoreIndex();
            } catch (error) {
                // Remove if decryption failed
                await storage.removeItem(`local:${this.SHARES_KEY_PREFIX}${wallet.metadata.id}`);
                throw new Error("Invalid password for backup");
            }
        }
    }
    
    // === Private Helper Methods ===
    
    private async loadKeystoreIndex(): Promise<void> {
        // Skip loading in test mode to ensure clean state
        if (KeystoreService.testMode) {
            return;
        }
        
        try {
            const stored = await storage.getItem<KeystoreIndex>(`local:${this.INDEX_KEY}`);
            if (stored) {
                this.keystoreIndex = stored;
                console.log("[KeystoreService] Loaded keystore index with", stored.wallets?.length || 0, "wallets");
            }
        } catch (error) {
            console.error("[KeystoreService] Failed to load index:", error);
        }
    }
    
    private async saveKeystoreIndex(): Promise<void> {
        if (!this.keystoreIndex) return;
        
        try {
            await storage.setItem(`local:${this.INDEX_KEY}`, this.keystoreIndex);
        } catch (error) {
            console.error("[KeystoreService] Failed to save index:", error);
        }
    }
    
    private async encryptKeyShare(walletId: string, keyShare: KeyShareData): Promise<EncryptedKeyShare> {
        if (!this.password) {
            throw new Error("No password set");
        }
        
        // Generate salt and IV
        const salt = crypto.getRandomValues(new Uint8Array(16));
        const iv = crypto.getRandomValues(new Uint8Array(12));
        
        // Derive key from password
        const key = await this.deriveKey(this.password, salt);
        
        // Encrypt data
        const encoder = new TextEncoder();
        const data = encoder.encode(JSON.stringify(keyShare));
        
        const ciphertext = await crypto.subtle.encrypt(
            { name: 'AES-GCM', iv },
            key,
            data
        );
        
        return {
            walletId,
            algorithm: 'AES-GCM',
            salt: btoa(String.fromCharCode(...salt)),
            iv: btoa(String.fromCharCode(...iv)),
            ciphertext: btoa(String.fromCharCode(...new Uint8Array(ciphertext)))
        };
    }
    
    private async decryptKeyShare(encrypted: EncryptedKeyShare): Promise<KeyShareData> {
        if (!this.password) {
            throw new Error("No password set");
        }

        // Decode base64
        const salt = Uint8Array.from(atob(encrypted.salt), c => c.charCodeAt(0));
        const iv = Uint8Array.from(atob(encrypted.iv), c => c.charCodeAt(0));
        const ciphertext = Uint8Array.from(atob(encrypted.ciphertext), c => c.charCodeAt(0));

        // Derive key
        const key = await this.deriveKey(this.password, salt);

        // Decrypt
        const decrypted = await crypto.subtle.decrypt(
            { name: 'AES-GCM', iv },
            key,
            ciphertext
        );

        const decoder = new TextDecoder();
        const json = decoder.decode(decrypted);
        return JSON.parse(json);
    }
    
    private async deriveKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
        const encoder = new TextEncoder();
        const keyMaterial = await crypto.subtle.importKey(
            'raw',
            encoder.encode(password),
            'PBKDF2',
            false,
            ['deriveKey']
        );
        
        return crypto.subtle.deriveKey(
            {
                name: 'PBKDF2',
                salt,
                iterations: 100000,
                hash: 'SHA-256'
            },
            keyMaterial,
            { name: 'AES-GCM', length: 256 },
            false,
            ['encrypt', 'decrypt']
        );
    }
    
    private async saveEncryptedShare(walletId: string, encrypted: EncryptedKeyShare): Promise<void> {
        await storage.setItem(`local:${this.SHARES_KEY_PREFIX}${walletId}`, encrypted);
    }
    
    private async loadEncryptedShare(walletId: string): Promise<EncryptedKeyShare | null> {
        return await storage.getItem<EncryptedKeyShare>(`local:${this.SHARES_KEY_PREFIX}${walletId}`);
    }
    
    private async loadKeyShare(walletId: string): Promise<KeyShareData | null> {
        const encrypted = await this.loadEncryptedShare(walletId);
        if (!encrypted) return null;
        
        return await this.decryptKeyShare(encrypted);
    }

    /**
     * Decrypt CLI data using base64 format
     */
    public async decryptCLIData(base64Data: string, password: string): Promise<string> {
        // CLI uses base64 encoding for the entire encrypted blob
        const encryptedData = Uint8Array.from(atob(base64Data), c => c.charCodeAt(0));
        
        // CLI format: salt (16) + nonce (12) + ciphertext
        if (encryptedData.length < 28) {
            throw new Error("Invalid CLI encrypted data");
        }
        
        const salt = encryptedData.slice(0, 16);
        const nonce = encryptedData.slice(16, 28);
        const ciphertext = encryptedData.slice(28);
        
        // Note: CLI uses Argon2id, but for now we'll use PBKDF2
        // This is a limitation that would need proper Argon2id implementation
        const key = await this.deriveCLIKey(password, salt);
        
        // Decrypt using AES-GCM
        const decrypted = await crypto.subtle.decrypt(
            { name: 'AES-GCM', iv: nonce },
            key,
            ciphertext
        );
        
        const decoder = new TextDecoder();
        return decoder.decode(decrypted);
    }

    /**
     * Encrypt data in CLI format
     */
    private async encryptCLIData(data: string): Promise<string> {
        if (!this.password) {
            throw new Error("No password set");
        }

        // Generate salt and nonce like CLI
        const salt = crypto.getRandomValues(new Uint8Array(16));
        const nonce = crypto.getRandomValues(new Uint8Array(12));
        
        // Derive key (should be Argon2id for full CLI compatibility)
        const key = await this.deriveCLIKey(this.password, salt);
        
        // Encrypt data
        const encoder = new TextEncoder();
        const dataBytes = encoder.encode(data);
        
        const ciphertext = await crypto.subtle.encrypt(
            { name: 'AES-GCM', iv: nonce },
            key,
            dataBytes
        );
        
        // Combine salt + nonce + ciphertext like CLI
        const combined = new Uint8Array(salt.length + nonce.length + ciphertext.byteLength);
        combined.set(salt, 0);
        combined.set(nonce, salt.length);
        combined.set(new Uint8Array(ciphertext), salt.length + nonce.length);
        
        // Encode as base64
        return btoa(String.fromCharCode(...combined));
    }

    /**
     * Derive key compatible with CLI (should use Argon2id for full compatibility)
     */
    private async deriveCLIKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
        // NOTE: CLI uses Argon2id, but WebCrypto doesn't support it natively
        // For now, using PBKDF2 with higher iterations as a fallback
        const encoder = new TextEncoder();
        const keyMaterial = await crypto.subtle.importKey(
            'raw',
            encoder.encode(password),
            'PBKDF2',
            false,
            ['deriveKey']
        );
        
        return crypto.subtle.deriveKey(
            {
                name: 'PBKDF2',
                salt,
                iterations: 600000, // Higher iterations to compensate for not using Argon2id
                hash: 'SHA-256'
            },
            keyMaterial,
            { name: 'AES-GCM', length: 256 },
            false,
            ['encrypt', 'decrypt']
        );
    }
}

// Export singleton getter
export const getKeystoreService = (): KeystoreService => {
    return KeystoreService.getInstance();
};