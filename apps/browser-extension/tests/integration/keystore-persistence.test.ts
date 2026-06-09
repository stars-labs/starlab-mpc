import { describe, it, expect, beforeEach, afterEach } from 'bun:test';
import { REAL_WEBCRYPTO } from '../setup-bun';
import { KeystoreManager } from '../../src/services/keystoreManager';
import { KeystoreService } from '../../src/services/keystoreService';
import { resetStorageData } from '../__mocks__/imports';
import type { KeyShareData, ExtensionWalletMetadata } from "@starlab/types/keystore";

// Mock chrome.storage
const mockStorage = {
    local: {
        data: {} as Record<string, any>,
        get: async (keys: string | string[]): Promise<Record<string, any>> => {
            const keysArray = Array.isArray(keys) ? keys : [keys];
            const result: Record<string, any> = {};
            keysArray.forEach(key => {
                if (key in mockStorage.local.data) {
                    result[key] = mockStorage.local.data[key];
                }
            });
            return result;
        },
        set: async (items: Record<string, any>): Promise<void> => {
            Object.assign(mockStorage.local.data, items);
        },
        remove: async (keys: string | string[]): Promise<void> => {
            const keysArray = Array.isArray(keys) ? keys : [keys];
            keysArray.forEach(key => {
                delete mockStorage.local.data[key];
            });
        }
    },
    session: {
        data: {} as Record<string, any>,
        get: async (keys: string | string[]): Promise<Record<string, any>> => {
            const keysArray = Array.isArray(keys) ? keys : [keys];
            const result: Record<string, any> = {};
            keysArray.forEach(key => {
                if (key in mockStorage.session.data) {
                    result[key] = mockStorage.session.data[key];
                }
            });
            return result;
        },
        set: async (items: Record<string, any>): Promise<void> => {
            Object.assign(mockStorage.session.data, items);
        }
    }
};

(global as any).chrome = { storage: mockStorage };

describe('Keystore Persistence Integration', () => {
    // Shared #imports mock storage that persists across singleton
    // resets within a single test (that's the whole point of the
    // "close extension, reopen" simulation — data survives).
    let sharedData: Record<string, any> = {};

    // Stash the pre-test global.crypto so afterEach can restore it —
    // without this, replacing crypto with REAL_WEBCRYPTO would persist
    // into sibling tests (e.g. extensionCliInterop) that expect the
    // setup-bun jest.fn() mocks on crypto.subtle.*.
    let savedCrypto: any;

    beforeEach(() => {
        savedCrypto = (global as any).crypto;

        // Re-install real WebCrypto (captured by setup-bun BEFORE
        // it installed its own mock — see REAL_WEBCRYPTO export).
        (global as any).crypto = REAL_WEBCRYPTO;

        // Re-install file-local mockStorage so our chrome spies actually
        // see reads/writes from the service layer.
        (global as any).chrome = { storage: mockStorage };

        // Clear storage (both file-local and the #imports shared store)
        mockStorage.local.data = {};
        mockStorage.session.data = {};
        sharedData = {};
        resetStorageData(() => sharedData);

        // Reset singletons
        (KeystoreManager as any).instance = null;
        KeystoreService.resetInstance();
    });

    afterEach(() => {
        (global as any).crypto = savedCrypto;
    });
    
    it('should persist imported keystore and restore on restart', async () => {
        // Step 1: Initialize and create keystore
        const manager1 = KeystoreManager.getInstance();
        await manager1.initialize('test-device');
        await manager1.createKeystore('password123');
        
        // Step 2: Import a keystore (simulate CLI import)
        // Note: this literal intentionally uses an older KeyShareData
        // shape (keystore_id, current_round, party_index, key_packages,
        // round1/round2 slots) that predates the Rust-aligned interface.
        // The persistence roundtrip test is about encrypt/store/decrypt,
        // not KeyShareData field conformance — cast to any to bypass
        // type checking while keeping the test intent intact.
        const keyShareData: any = {
            keystore_id: 'cli-imported',
            current_round: 2,
            threshold: 2,
            total_participants: 3,
            curve_type: 'secp256k1',
            blockchain: 'ethereum',
            party_index: 1,
            key_packages: {
                '1': '0x1234',
                '2': '0x5678',
                '3': '0x9abc'
            },
            round1_secret_package: null,
            round1_packages: {},
            round2_secret_package: null,
            round2_public_packages: {}
        };
        
        const metadata: ExtensionWalletMetadata = {
            id: 'imported-wallet',
            name: 'Imported from CLI',
            blockchain: 'ethereum',
            address: '0x742d35Cc6634C0532925a3b844Bc9e7595f84D02',
            session_id: 'cli-session-123',
            isActive: true,
            hasBackup: true,
            createdAt: Date.now()
        };
        
        const addResult = await manager1.addWallet('imported-wallet', keyShareData, metadata);
        expect(addResult).toBe(true);
        
        // Verify wallet is stored
        const wallets1 = manager1.getWallets();
        expect(wallets1).toHaveLength(1);
        expect(wallets1[0].id).toBe('imported-wallet');
        
        // Step 3: Lock the keystore (simulate closing extension)
        manager1.lock();
        
        // Step 4: Simulate extension restart - create new manager instance
        (KeystoreManager as any).instance = null;
        const manager2 = KeystoreManager.getInstance();
        await manager2.initialize('test-device');
        
        // Step 5: Check if keystore is initialized (has wallets)
        expect(await manager2.isInitialized()).toBe(true);
        expect(manager2.isLocked()).toBe(true);
        
        // Step 6: Unlock with password
        const unlockResult = await manager2.unlock('password123');
        expect(unlockResult).toBe(true);
        
        // Step 7: Verify imported wallet is still there
        const wallets2 = manager2.getWallets();
        expect(wallets2).toHaveLength(1);
        expect(wallets2[0].id).toBe('imported-wallet');
        expect(wallets2[0].name).toBe('Imported from CLI');
        expect(wallets2[0].address).toBe('0x742d35Cc6634C0532925a3b844Bc9e7595f84D02');
        
        // Step 8: Verify we can get the key share
        const keyShare = await manager2.getKeyShare('imported-wallet');
        expect(keyShare).toBeDefined();
        // Legacy-shape fields preserved by the round-trip; access
        // through `any` since they're not on the KeyShareData type
        // (see comment on the addWallet site above).
        expect((keyShare as any)?.keystore_id).toBe('cli-imported');
        expect((keyShare as any)?.party_index).toBe(1);
    });
    
    it('should handle multiple wallets and active wallet switching', async () => {
        const manager = KeystoreManager.getInstance();
        await manager.initialize('test-device');
        await manager.createKeystore('password123');
        
        // Add Ethereum wallet
        const ethWallet: ExtensionWalletMetadata = {
            id: 'eth-wallet',
            name: 'Ethereum Wallet',
            blockchain: 'ethereum',
            address: '0x1111111111111111111111111111111111111111',
            session_id: 'eth-session',
            isActive: true,
            hasBackup: false,
            createdAt: Date.now()
        };
        
        // Legacy shape — see comment on keyShareData at line ~68.
        await manager.addWallet('eth-wallet', {
            keystore_id: 'eth-keystore',
            current_round: 2,
            threshold: 2,
            total_participants: 3,
            curve_type: 'secp256k1',
            blockchain: 'ethereum',
            party_index: 1,
            key_packages: {},
            round1_secret_package: null,
            round1_packages: {},
            round2_secret_package: null,
            round2_public_packages: {}
        } as any, ethWallet);
        
        // Add Solana wallet
        const solWallet: ExtensionWalletMetadata = {
            id: 'sol-wallet',
            name: 'Solana Wallet',
            blockchain: 'solana',
            address: 'So1111111111111111111111111111111111111111',
            session_id: 'sol-session',
            isActive: false,
            hasBackup: false,
            createdAt: Date.now()
        };
        
        await manager.addWallet('sol-wallet', {
            keystore_id: 'sol-keystore',
            current_round: 2,
            threshold: 2,
            total_participants: 3,
            curve_type: 'ed25519',
            blockchain: 'solana',
            party_index: 1,
            key_packages: {},
            round1_secret_package: null,
            round1_packages: {},
            round2_secret_package: null,
            round2_public_packages: {}
        } as any, solWallet);
        
        // Verify both wallets exist
        const wallets = manager.getWallets();
        expect(wallets).toHaveLength(2);
        
        // Verify active wallet
        const activeWallet = manager.getActiveWallet();
        expect(activeWallet?.id).toBe('eth-wallet');
        
        // Switch active wallet
        await manager.switchWallet('sol-wallet');
        
        // Verify switch worked
        const newActiveWallet = manager.getActiveWallet();
        expect(newActiveWallet?.id).toBe('sol-wallet');
        
        // Verify persistence
        manager.lock();
        
        // Create new instance
        (KeystoreManager as any).instance = null;
        const manager2 = KeystoreManager.getInstance();
        await manager2.initialize('test-device');
        await manager2.unlock('password123');
        
        // Verify active wallet persisted
        const persistedActiveWallet = manager2.getActiveWallet();
        expect(persistedActiveWallet?.id).toBe('sol-wallet');
    });
});