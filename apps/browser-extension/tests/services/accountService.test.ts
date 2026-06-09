import { describe, it, expect, beforeEach, afterEach, mock } from 'bun:test';
import AccountService from '../../src/services/accountService';
import type { Account } from '@starlab/types/account';

// Helper function to create test accounts with required fields
function createTestAccount(partial: Partial<Account> & { 
    id: string; 
    address: string; 
    name: string; 
    blockchain: 'ethereum' | 'solana' 
}): Account {
    return {
        balance: '0',
        publicKey: 'test-public-key',
        created: Date.now(),
        ...partial
    };
}

// Create a comprehensive mock for Chrome storage
const mockStorage = {
    data: {} as Record<string, any>,
    get: mock(async (keys: string | string[] | Record<string, any>) => {
        if (typeof keys === 'string') {
            return { [keys]: mockStorage.data[keys] };
        } else if (Array.isArray(keys)) {
            const result: Record<string, any> = {};
            keys.forEach(key => {
                result[key] = mockStorage.data[key];
            });
            return result;
        } else {
            const result: Record<string, any> = {};
            Object.keys(keys).forEach(key => {
                result[key] = mockStorage.data[key] || keys[key];
            });
            return result;
        }
    }),
    set: mock(async (data: Record<string, any>) => {
        Object.assign(mockStorage.data, data);
    }),
    clear: mock(async () => {
        mockStorage.data = {};
    })
};

// Mock Chrome API
(global as any).chrome = {
    storage: {
        local: mockStorage
    }
};

describe('AccountService', () => {
    let accountService: AccountService;

    beforeEach(async () => {
        // Re-install file-local mockStorage onto chrome.storage.local.
        // setup-bun.ts has its own beforeEach that overwrites
        // chrome.storage.local with a fresh mock — run last, that would
        // bury our spy. Re-assigning here makes the file's mockStorage
        // the one the service actually hits.
        (global as any).chrome.storage.local = mockStorage;

        // Clear mock storage
        await mockStorage.clear();
        mockStorage.get.mockClear();
        mockStorage.set.mockClear();

        // Reset singleton instance before each test
        AccountService.resetInstance();

        // Get fresh instance
        accountService = AccountService.getInstance();

        // Ensure it's initialized
        await accountService.ensureInitialized();
    });

    afterEach(async () => {
        await mockStorage.clear();
        AccountService.resetInstance();
    });

    describe('Singleton Pattern', () => {
        it('should return the same instance', () => {
            const instance1 = AccountService.getInstance();
            const instance2 = AccountService.getInstance();
            expect(instance1).toBe(instance2);
        });

        it('should reset instance for testing', () => {
            const instance1 = AccountService.getInstance();
            AccountService.resetInstance();
            const instance2 = AccountService.getInstance();
            expect(instance1).not.toBe(instance2);
        });
    });

    describe('Account Management', () => {
        it('should initialize with empty accounts list', async () => {
            const accounts = await accountService.getAccounts();
            expect(accounts).toEqual([]);
        });

        it('should add new account successfully', async () => {
            const newAccount = createTestAccount({
                id: 'account-1',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            const result = await accountService.addAccount(newAccount);
            
            expect(result).toEqual(newAccount);
            const accounts = await accountService.getAccounts();
            expect(accounts).toHaveLength(1);
            expect(accounts[0]).toMatchObject(newAccount);
        });

        it('should reject duplicate account IDs', async () => {
            const account1 = createTestAccount({
                id: 'account-1',
                name: 'First Account',
                address: '0x1111111111111111111111111111111111111111',
                blockchain: 'ethereum'
            });

            const account2 = createTestAccount({
                id: 'account-1', // Same ID
                name: 'Second Account',
                address: '0x2222222222222222222222222222222222222222',
                blockchain: 'ethereum'
            });

            await accountService.addAccount(account1);

            await expect(accountService.addAccount(account2))
                .rejects.toThrow('Account with this ID already exists');
        });

        it('should reject duplicate addresses within same blockchain', async () => {
            const account1 = createTestAccount({
                id: 'account-1',
                name: 'First Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            const account2 = createTestAccount({
                id: 'account-2',
                name: 'Second Account',
                address: '0x1234567890123456789012345678901234567890', // Same address
                blockchain: 'ethereum'
            });

            await accountService.addAccount(account1);

            await expect(accountService.addAccount(account2))
                .rejects.toThrow('Account with this address already exists for this blockchain');
        });

        it('should allow same address on different blockchains', async () => {
            const ethereumAccount = createTestAccount({
                id: 'eth-account',
                name: 'Ethereum Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            const solanaAccount = createTestAccount({
                id: 'sol-account',
                name: 'Solana Account',
                address: '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM', // Different format
                blockchain: 'solana'
            });

            await accountService.addAccount(ethereumAccount);
            const result = await accountService.addAccount(solanaAccount);

            expect(result).toEqual(solanaAccount);
            const accounts = await accountService.getAccounts();
            expect(accounts).toHaveLength(2);
        });

        it('should update existing account', async () => {
            const originalAccount = createTestAccount({
                id: 'account-1',
                name: 'Original Name',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            await accountService.addAccount(originalAccount);

            const updatedAccount = {
                ...originalAccount,
                name: 'Updated Name',
                balance: '1000000000000000000' // 1 ETH in wei
            };

            const result = await accountService.updateAccount(updatedAccount);

            expect(result.name).toBe('Updated Name');
            expect(result.balance).toBe('1000000000000000000');

            const accounts = await accountService.getAccounts();
            expect(accounts[0].name).toBe('Updated Name');
            expect(accounts[0].balance).toBe('1000000000000000000');
        });

        it('should throw error when updating non-existent account', async () => {
            const nonExistentAccount = createTestAccount({
                id: 'non-existent',
                name: 'Non-existent',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            await expect(accountService.updateAccount(nonExistentAccount))
                .rejects.toThrow('Account not found');
        });

        it('should remove account successfully', async () => {
            const account = createTestAccount({
                id: 'account-to-remove',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            await accountService.addAccount(account);
            expect(await accountService.getAccounts()).toHaveLength(1);

            await accountService.removeAccount('account-to-remove');
            expect(await accountService.getAccounts()).toHaveLength(0);
        });

        it('should throw error when removing non-existent account', async () => {
            await expect(accountService.removeAccount('non-existent'))
                .rejects.toThrow('Account not found');
        });
    });

    describe('Current Account Management', () => {
        let testAccount: Account;

        beforeEach(async () => {
            testAccount = createTestAccount({
                id: 'test-account',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });
            await accountService.addAccount(testAccount);
        });

        it('should return null for current account initially', async () => {
            const currentAccount = await accountService.getCurrentAccount();
            expect(currentAccount).toBeNull();
        });

        it('should set and get current account', async () => {
            await accountService.setCurrentAccount('test-account');
            const currentAccount = await accountService.getCurrentAccount();
            
            expect(currentAccount).not.toBeNull();
            expect(currentAccount?.id).toBe('test-account');
        });

        it('should throw error when setting non-existent account as current', async () => {
            await expect(accountService.setCurrentAccount('non-existent'))
                .rejects.toThrow('Account not found');
        });

        it('should clear current account', async () => {
            await accountService.setCurrentAccount('test-account');
            expect(await accountService.getCurrentAccount()).not.toBeNull();

            await accountService.setCurrentAccount(null);
            expect(await accountService.getCurrentAccount()).toBeNull();
        });
    });

    describe('Account Filtering and Querying', () => {
        beforeEach(async () => {
            const accounts = [
                createTestAccount({
                    id: 'eth-1',
                    name: 'Ethereum Main',
                    address: '0x1111111111111111111111111111111111111111',
                    blockchain: 'ethereum',
                    balance: '1000000000000000000'
                }),
                createTestAccount({
                    id: 'eth-2',
                    name: 'Ethereum Secondary',
                    address: '0x2222222222222222222222222222222222222222',
                    blockchain: 'ethereum',
                    balance: '500000000000000000'
                }),
                createTestAccount({
                    id: 'sol-1',  
                    name: 'Solana Main',
                    address: '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM',
                    blockchain: 'solana',
                    balance: '2000000000'
                })
            ];

            for (const account of accounts) {
                await accountService.addAccount(account);
            }
        });

        it('should get account by ID', async () => {
            const account = await accountService.getAccountById('eth-1');
            expect(account).not.toBeNull();
            expect(account?.name).toBe('Ethereum Main');
        });

        it('should return null for non-existent account ID', async () => {
            const account = await accountService.getAccountById('non-existent');
            expect(account).toBeNull();
        });

        it('should get account by address', async () => {
            const account = await accountService.getAccountByAddress('0x1111111111111111111111111111111111111111');
            expect(account).not.toBeNull();
            expect(account?.name).toBe('Ethereum Main');
        });

        it('should filter accounts by blockchain', async () => {
            const ethereumAccounts = await accountService.getAccountsByBlockchain('ethereum');
            const solanaAccounts = await accountService.getAccountsByBlockchain('solana');

            expect(ethereumAccounts).toHaveLength(2);
            expect(solanaAccounts).toHaveLength(1);
            
            expect(ethereumAccounts.every(acc => acc.blockchain === 'ethereum')).toBe(true);
            expect(solanaAccounts.every(acc => acc.blockchain === 'solana')).toBe(true);
        });
    });

    describe('Event System', () => {
        let eventCallback: any;

        beforeEach(() => {
            eventCallback = mock(() => {});
        });

        it('should register change callback', () => {
            accountService.onAccountChange(eventCallback);
            expect(eventCallback).not.toHaveBeenCalled();
        });

        it('should trigger callback when account changes', async () => {
            accountService.onAccountChange(eventCallback);
            
            const testAccount = createTestAccount({
                id: 'test-account',
                name: 'Test Account', 
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            await accountService.addAccount(testAccount);
            await accountService.setCurrentAccount('test-account');

            expect(eventCallback).toHaveBeenCalledWith(testAccount);
        });

        it('should trigger callback with null when clearing current account', async () => {
            const testAccount = createTestAccount({
                id: 'test-account',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890', 
                blockchain: 'ethereum'
            });

            await accountService.addAccount(testAccount);
            await accountService.setCurrentAccount('test-account');
            
            accountService.onAccountChange(eventCallback);
            await accountService.setCurrentAccount(null);

            expect(eventCallback).toHaveBeenCalledWith(null);
        });

        it('should remove callback', async () => {
            accountService.onAccountChange(eventCallback);
            accountService.offAccountChange(eventCallback);

            const testAccount = createTestAccount({
                id: 'test-account',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            await accountService.addAccount(testAccount);
            await accountService.setCurrentAccount('test-account');

            expect(eventCallback).not.toHaveBeenCalled();
        });
    });

    describe('Persistence', () => {
        it('should persist accounts to storage', async () => {
            const testAccount = createTestAccount({
                id: 'persistent-account',
                name: 'Persistent Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            await accountService.addAccount(testAccount);

            expect(mockStorage.set).toHaveBeenCalledWith(
                expect.objectContaining({
                    wallet_accounts: [testAccount]
                })
            );
        });

        it('should persist current account to storage', async () => {
            const testAccount = createTestAccount({
                id: 'current-account',
                name: 'Current Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            await accountService.addAccount(testAccount);
            await accountService.setCurrentAccount('current-account');

            expect(mockStorage.set).toHaveBeenCalledWith(
                expect.objectContaining({
                    wallet_current_account: 'current-account'
                })
            );
        });

        it('should load accounts from storage on initialization', async () => {
            const storedAccounts = [
                createTestAccount({
                    id: 'stored-account',
                    name: 'Stored Account',
                    address: '0x1234567890123456789012345678901234567890',
                    blockchain: 'ethereum'
                })
            ];

            mockStorage.data = {
                wallet_accounts: storedAccounts,
                wallet_current_account: 'stored-account'
            };

            // Create new service instance to trigger loading
            AccountService.resetInstance();
            const newService = AccountService.getInstance();
            await newService.ensureInitialized();

            const accounts = await newService.getAccounts();
            const currentAccount = await newService.getCurrentAccount();

            expect(accounts).toEqual(storedAccounts);
            expect(currentAccount?.id).toBe('stored-account');
        });
    });

    describe('Edge Cases and Error Handling', () => {
        it('should handle storage errors gracefully', async () => {
            mockStorage.set.mockRejectedValueOnce(new Error('Storage error'));

            const testAccount = createTestAccount({
                id: 'test-account',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            await expect(accountService.addAccount(testAccount))
                .rejects.toThrow('Storage error');
        });

        it('should handle corrupted storage data', async () => {
            mockStorage.data = {
                wallet_accounts: 'invalid-data', // Should be array
                wallet_current_account: 'test'
            };

            AccountService.resetInstance();
            const newService = AccountService.getInstance();
            await newService.ensureInitialized();

            const accounts = await newService.getAccounts();
            expect(accounts).toEqual([]); // Should default to empty array
        });

        it('should validate account data on add', async () => {
            const invalidAccount = {
                id: '', // Empty ID
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum' as const,
                balance: '0'
            };

            await expect(accountService.addAccount(invalidAccount))
                .rejects.toThrow();
        });

        it('should handle missing storage API gracefully', async () => {
            // Temporarily remove chrome.storage
            const originalChrome = (global as any).chrome;
            delete (global as any).chrome;

            AccountService.resetInstance();
            const newService = AccountService.getInstance();
            await newService.ensureInitialized();

            const accounts = await newService.getAccounts();
            expect(accounts).toEqual([]);

            // Restore chrome
            (global as any).chrome = originalChrome;
        });
    });

    describe('Account Metadata', () => {
        it('should handle account metadata correctly', async () => {
            const accountWithMetadata = createTestAccount({
                id: 'meta-account',
                name: 'Account with Metadata',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum',
                metadata: {
                    derivationPath: "m/44'/60'/0'/0/0",
                    source: 'generated',
                    tags: ['main', 'business']
                }
            });

            await accountService.addAccount(accountWithMetadata);
            const retrieved = await accountService.getAccountById('meta-account');

            expect(retrieved?.metadata?.derivationPath).toBe("m/44'/60'/0'/0/0");
            expect(retrieved?.metadata?.source).toBe('generated');
            expect(retrieved?.metadata?.tags).toEqual(['main', 'business']);
        });

        it('should update account timestamps', async () => {
            const account = createTestAccount({
                id: 'timestamp-account',
                name: 'Timestamp Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum'
            });

            const beforeAdd = Date.now();
            await accountService.addAccount(account);
            const afterAdd = Date.now();

            const retrieved = await accountService.getAccountById('timestamp-account');
            expect(retrieved?.created).toBeGreaterThanOrEqual(beforeAdd);
            expect(retrieved?.created).toBeLessThanOrEqual(afterAdd);
        });
    });
});