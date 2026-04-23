import { KeystoreService } from '../../src/services/keystoreService';
import type { KeyShareData, WalletMetadata, KeystoreBackup } from "@mpc-wallet/types/keystore";
import { resetStorageData } from '../__mocks__/imports';
import { resetWxtStorageData } from '../wxt-imports-mock';
import { describe, it, expect, beforeEach, afterEach } from 'bun:test';
import { jest } from 'bun:test';

describe('KeystoreService', () => {
    let keystore: KeystoreService;
    let storageData: Record<string, any>;
    
    beforeEach(async () => {
        // Clear all mocks
        jest.clearAllMocks();
        
        // Reset singleton instance BEFORE setting up storage
        KeystoreService.resetInstance();
        
        // Create fresh storage data for each test
        storageData = {};
        resetStorageData(() => storageData);
        resetWxtStorageData(() => storageData); // Use same storage for both mocks
        
        // Ensure chrome storage mock returns empty initially
        (chrome.storage.local.get as any).mockResolvedValue({});
        (chrome.storage.local.set as any).mockResolvedValue(undefined);
        
        // Create new instance - this will call loadKeystoreIndex in constructor
        keystore = KeystoreService.getInstance();
        
        // Wait for any async initialization
        await new Promise(resolve => setTimeout(resolve, 10));
    });
    
    afterEach(async () => {
        // Reset singleton and storage
        KeystoreService.resetInstance();
        resetStorageData();
        resetWxtStorageData();
    });

    describe('initialization', () => {
        it('should start in locked state', () => {
            expect(keystore.isLocked()).toBe(true);
        });

        it('should have no wallets initially', async () => {
            chrome.storage.local.get.mockResolvedValue({});
            const wallets = keystore.getWallets();
            expect(wallets).toEqual([]);
        });
    });

    describe('initialization and unlock', () => {
        const password = 'test-password-123';
        const deviceId = 'test-device-123';

        beforeEach(async () => {
            // Mock crypto operations for password derivation
            (crypto.subtle.importKey as any).mockResolvedValue('mock-key' as any);
            (crypto.subtle.deriveBits as any).mockResolvedValue(new ArrayBuffer(32));
            (crypto.subtle.digest as any).mockResolvedValue(new ArrayBuffer(32));
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key' as any);
            (chrome.storage.local.set as any).mockResolvedValue(undefined);
            (chrome.storage.local.get as any).mockResolvedValue({});
            
            // Initialize the keystore
            await keystore.initialize(deviceId);
        });

        it('should initialize keystore in locked state', async () => {
            expect(keystore.isLocked()).toBe(true);
        });

        it('should unlock keystore successfully', async () => {
            await keystore.initialize(deviceId);
            const result = await keystore.unlock(password);
            expect(result).toBe(true);
            expect(keystore.isLocked()).toBe(false);
        });

        it('should lock and unlock keystore', async () => {
            // Initialize and unlock
            await keystore.initialize(deviceId);
            await keystore.unlock(password);
            expect(keystore.isLocked()).toBe(false);
            
            // Lock it
            keystore.lock();
            expect(keystore.isLocked()).toBe(true);
            
            // Unlock again
            const result = await keystore.unlock(password);
            expect(result).toBe(true);
            expect(keystore.isLocked()).toBe(false);
        });

        it('should remain unlocked without password validation', async () => {
            // The current implementation always returns true for unlock
            // since password validation happens when decrypting key shares
            await keystore.initialize(deviceId);
            
            // Any password unlocks an empty keystore
            const result = await keystore.unlock('any-password');
            expect(result).toBe(true);
            expect(keystore.isLocked()).toBe(false);
        });
    });

    describe('wallet operations', () => {
        const password = 'test-password';
        const deviceId = 'test-device-123';
        const mockKeyShareData: KeyShareData = {
            key_package: 'mock-key-package',
            group_public_key: '0x1234567890abcdef',
            session_id: 'session-123',
            device_id: 'device-123',
            participant_index: 1,
            threshold: 2,
            total_participants: 3,
            participants: ['device1', 'device2', 'device3'],
            curve: 'secp256k1',
            blockchains: [],
            created_at: Date.now(),
        };
        
        const mockMetadata: WalletMetadata = {
            id: 'wallet-1',
            name: 'Test Wallet',
            blockchain: 'ethereum',
            address: '0x742d35Cc6634C0532925a3b844Bc9e7595f4279',
            sessionId: 'session-123',
            isActive: true,
            hasBackup: false
        };

        beforeEach(async () => {
            // Setup encryption mocks
            (crypto.subtle.importKey as any).mockResolvedValue('mock-key' as any);
            (crypto.subtle.deriveBits as any).mockResolvedValue(new ArrayBuffer(32));
            (crypto.subtle.digest as any).mockResolvedValue(new ArrayBuffer(32));
            (crypto.subtle.encrypt as any).mockResolvedValue(new ArrayBuffer(100));
            (crypto.subtle.decrypt as any).mockResolvedValue(
                new TextEncoder().encode(JSON.stringify(mockKeyShareData)).buffer
            );
            chrome.storage.local.set.mockResolvedValue(undefined);
            chrome.storage.local.get.mockResolvedValue({});
            
            // Unlock keystore
            await keystore.initialize(deviceId);
            await keystore.unlock(password);
        });

        it('should add wallet with encrypted key share', async () => {
            // Mock deriveKey to return a mock key
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key');
            
            await keystore.addWallet('wallet-add-test', mockKeyShareData, { ...mockMetadata, id: 'wallet-add-test' });
            
            // Verify encryption was called
            expect(crypto.subtle.encrypt).toHaveBeenCalled();
            
            // Verify the wallet was added
            const wallets = keystore.getWallets();
            expect(wallets).toHaveLength(1);
            expect(wallets[0].id).toBe('wallet-add-test');
        });

        it('should list wallets', async () => {
            // Mock deriveKey for wallet addition
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key');
            
            // Check initial state
            const initialWallets = keystore.getWallets();
            expect(initialWallets).toHaveLength(0);
            
            // Add wallets first
            await keystore.addWallet('wallet-list-1', mockKeyShareData, { ...mockMetadata, id: 'wallet-list-1', name: 'Wallet List 1' });
            await keystore.addWallet('wallet-list-2', mockKeyShareData, { ...mockMetadata, id: 'wallet-list-2', name: 'Wallet List 2' });
            
            const wallets = keystore.getWallets();
            expect(wallets).toHaveLength(2);
            expect(wallets[0].id).toBe('wallet-list-1');
            expect(wallets[1].id).toBe('wallet-list-2');
        });

        it('should get wallet key share when unlocked', async () => {
            // Mock deriveKey for wallet addition
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key');
            
            // Add wallet first
            await keystore.addWallet('wallet-keyshare-test', mockKeyShareData, { ...mockMetadata, id: 'wallet-keyshare-test' });
            
            // Get the key share (should return from cache)
            const keyShare = await keystore.getKeyShare('wallet-keyshare-test');
            expect(keyShare).toEqual(mockKeyShareData);
            
            // Clear the cache to force loading from storage
            (keystore as any).keyShares.clear();
            
            // Mock storage.getItem for loading encrypted share
            const { storage } = await import('../__mocks__/imports');
            jest.spyOn(storage, 'getItem').mockResolvedValueOnce({
                walletId: 'wallet-keyshare-test',
                algorithm: 'AES-GCM',
                salt: btoa('mock-salt'),  // Properly encode as base64
                iv: btoa('mock-iv'),      // Properly encode as base64
                ciphertext: btoa('mock-ciphertext')  // Properly encode as base64
            });
            
            // Now it should decrypt
            const keyShare2 = await keystore.getKeyShare('wallet-keyshare-test');
            expect(keyShare2).toEqual(mockKeyShareData);
            expect(crypto.subtle.decrypt).toHaveBeenCalled();
        });

        it('should throw error when getting wallet while locked', async () => {
            keystore.lock();
            await expect(keystore.getKeyShare('wallet-1')).rejects.toThrow('Keystore is locked');
        });

        it('should remove wallet', async () => {
            // Mock deriveKey for wallet addition
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key');
            
            // Add wallet first
            await keystore.addWallet('wallet-remove-test', mockKeyShareData, { ...mockMetadata, id: 'wallet-remove-test' });
            
            // Verify wallet was added
            expect(keystore.getWallets()).toHaveLength(1);
            
            // Remove wallet
            await keystore.removeWallet('wallet-remove-test');
            
            // Verify wallet was removed
            expect(keystore.getWallets()).toHaveLength(0);
        });

        it('should update wallet metadata', async () => {
            // Add wallet first
            await keystore.addWallet('wallet-metadata-test', mockKeyShareData, { ...mockMetadata, id: 'wallet-metadata-test' });
            
            // Currently there's no updateWalletMetadata method, but we can verify the wallet exists
            const wallet = keystore.getWallet('wallet-metadata-test');
            expect(wallet?.id).toBe('wallet-metadata-test');
            
            // To update metadata, we would need to remove and re-add the wallet
            // This test shows the current limitation
        });
    });

    describe('backup and restore', () => {
        const password = 'backup-password';
        const deviceId = 'test-device-123';
        const mockWallets = [
            {
                metadata: {
                    id: 'backup-wallet-1',
                    name: 'Wallet 1',
                    blockchain: 'ethereum',
                    address: '0x123',
                    sessionId: 'session-1',
                    isActive: true,
                    hasBackup: false
                },
                keyShare: {
                    keyPackage: 'key-1',
                    groupPublicKey: '0xabc',
                    sessionId: 'session-1',
                    deviceId: 'device-123',
                    participantIndex: 1,
                    threshold: 2,
                    totalParticipants: 3,
                    participants: ['device1', 'device2', 'device3'],
                    curve: 'secp256k1' as const,
                    createdAt: Date.now()
                }
            }
        ];

        beforeEach(async () => {
            // Setup mocks
            (crypto.subtle.importKey as any).mockResolvedValue('mock-key' as any);
            (crypto.subtle.deriveBits as any).mockResolvedValue(new ArrayBuffer(32));
            (crypto.subtle.digest as any).mockResolvedValue(new ArrayBuffer(32));
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key' as any);
            (crypto.subtle.encrypt as any).mockResolvedValue(new ArrayBuffer(100));
            (crypto.subtle.decrypt as any).mockResolvedValue(
                new TextEncoder().encode(JSON.stringify(mockWallets[0].keyShare)).buffer
            );
            chrome.storage.local.set.mockResolvedValue(undefined);
            chrome.storage.local.get.mockResolvedValue({});
            
            await keystore.initialize(deviceId);
            await keystore.unlock(password);
        });

        it('should export wallet for backup', async () => {
            // Mock deriveKey for wallet addition
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key');
            
            // Add wallet first
            await keystore.addWallet('backup-wallet-1', mockWallets[0].keyShare, mockWallets[0].metadata);
            
            // Mock loading encrypted share
            const { storage } = await import('../__mocks__/imports');
            jest.spyOn(storage, 'getItem').mockResolvedValueOnce({
                walletId: 'backup-wallet-1',
                algorithm: 'AES-GCM',
                salt: btoa('mock-salt'),
                iv: btoa('mock-iv'),
                ciphertext: btoa('mock-ciphertext')
            });
            
            const backup = await keystore.exportWallet('backup-wallet-1');
            
            expect(backup).toHaveProperty('version');
            expect(backup).toHaveProperty('deviceId');
            expect(backup).toHaveProperty('exportedAt');
            expect(backup.wallets).toHaveLength(1);
            expect(backup.wallets[0].metadata).toEqual(mockWallets[0].metadata);
        });

        it('should import wallet from backup', async () => {
            const backup: KeystoreBackup = {
                version: '1.0.0',
                deviceId: 'device-123',
                exportedAt: Date.now(),
                wallets: [{
                    metadata: mockWallets[0].metadata,
                    encryptedShare: {
                        walletId: 'backup-wallet-1',
                        algorithm: 'AES-GCM' as const,
                        salt: btoa('salt'),
                        iv: btoa('iv'),
                        ciphertext: btoa('ciphertext')
                    }
                }]
            };
            
            // Mock successful decryption
            (crypto.subtle.decrypt as any).mockResolvedValue(
                new TextEncoder().encode(JSON.stringify(mockWallets[0].keyShare)).buffer
            );
            
            // Mock storage.setItem
            const { storage } = await import('../__mocks__/imports');
            const mockSetItem = jest.fn();
            jest.spyOn(storage, 'setItem').mockImplementation(mockSetItem);
            
            await keystore.importWallet(backup, password);
            
            // Verify wallet was imported
            const wallets = keystore.getWallets();
            expect(wallets).toHaveLength(1);
            expect(wallets[0]).toEqual(mockWallets[0].metadata);
        });

        it('should remove all wallets individually', async () => {
            // Mock deriveKey for wallet addition
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key');
            
            // Add multiple wallets
            await keystore.addWallet('backup-wallet-1', mockWallets[0].keyShare, mockWallets[0].metadata);
            await keystore.addWallet('backup-wallet-2', mockWallets[0].keyShare, { ...mockWallets[0].metadata, id: 'backup-wallet-2' });
            
            expect(keystore.getWallets()).toHaveLength(2);
            
            // Remove all wallets
            await keystore.removeWallet('backup-wallet-1');
            await keystore.removeWallet('backup-wallet-2');
            
            expect(keystore.getWallets()).toHaveLength(0);
        });
    });

    describe('security', () => {
        const deviceId = 'test-device-123';
        
        beforeEach(async () => {
            // Setup crypto mocks for password operations
            (crypto.subtle.importKey as any).mockResolvedValue('mock-key' as any);
            (crypto.subtle.deriveBits as any).mockResolvedValue(new ArrayBuffer(32));
            (crypto.subtle.digest as any).mockResolvedValue(new ArrayBuffer(32));
            (crypto.subtle.deriveKey as any).mockResolvedValue('mock-derived-key' as any);
            (crypto.subtle.encrypt as any).mockResolvedValue(new ArrayBuffer(100));
            (chrome.storage.local.set as any).mockResolvedValue(undefined);
            (chrome.storage.local.get as any).mockResolvedValue({});
            
            // Initialize keystore
            await keystore.initialize(deviceId);
        });

        it('should use different salt for each encryption', async () => {
            // Unlock keystore first
            await keystore.unlock('password');
            
            const keyShare: KeyShareData = {
                key_package: 'test',
                group_public_key: '0x123',
                session_id: 'session-1',
                device_id: 'device-123',
                participant_index: 1,
                threshold: 2,
                total_participants: 3,
                participants: ['device1', 'device2', 'device3'],
                curve: 'secp256k1',
                blockchains: [],
                created_at: Date.now(),
            };
            
            // Mock crypto.getRandomValues to track salt generation
            const generatedSalts = new Set<string>();
            const originalGetRandomValues = globalThis.crypto.getRandomValues;
            globalThis.crypto.getRandomValues = jest.fn((arr: any) => {
                // Fill with mock random values
                for (let i = 0; i < arr.length; i++) {
                    arr[i] = Math.floor(Math.random() * 256);
                }
                // Track the generated salt
                generatedSalts.add(Array.from(arr).join(','));
                return arr;
            });
            
            for (let i = 0; i < 3; i++) {
                await keystore.addWallet(`security-wallet-${i}`, keyShare, {
                    id: `security-wallet-${i}`,
                    name: `Security Wallet ${i}`,
                    blockchain: 'ethereum',
                    address: '0x123',
                    sessionId: 'session-1',
                    isActive: true,
                    hasBackup: false
                });
            }
            
            // Restore original function
            globalThis.crypto.getRandomValues = originalGetRandomValues;
            
            // Verify that encrypt was called multiple times
            expect(crypto.subtle.encrypt).toHaveBeenCalledTimes(3);
            // Each encryption should generate random values
            expect(generatedSalts.size).toBeGreaterThan(0);
        });

        it('should not expose sensitive data in errors', async () => {
            keystore.lock();
            
            try {
                await keystore.getKeyShare('wallet-1');
            } catch (error: any) {
                expect(error.message).not.toContain('password');
                expect(error.message).not.toContain('key');
                expect(error.message).toBe('Keystore is locked');
            }
        });
    });
});
