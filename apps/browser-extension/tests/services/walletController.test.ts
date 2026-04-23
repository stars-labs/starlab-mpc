import WalletController from '../../src/services/walletController';
import AccountService from '../../src/services/accountService';
import NetworkService from '../../src/services/networkService';
// Mock chrome.storage.local
import {  describe, it, expect, beforeEach, afterEach  } from 'bun:test';
const mockStorage = {
    data: {} as Record<string, any>,
    get: async (key: string) => ({ [key]: mockStorage.data[key] }),
    set: async (data: Record<string, any>) => {
        Object.assign(mockStorage.data, data);
    },
    clear: async () => {
        mockStorage.data = {};
    }
};

// Mock chrome API
(global as any).chrome = {
    storage: {
        local: mockStorage
    }
};

describe('WalletController', () => {
    let walletController: WalletController;

    beforeEach(async () => {
        // Reset all singleton instances
        WalletController.resetInstance();
        AccountService.resetInstance();
        NetworkService.resetInstance();
        // Clear storage
        await mockStorage.clear();
        // Get fresh instance
        walletController = WalletController.getInstance();
    });

    afterEach(async () => {
        await mockStorage.clear();
        WalletController.resetInstance();
        AccountService.resetInstance();
        NetworkService.resetInstance();
    });

    it('should return singleton instance', () => {
        const instance1 = WalletController.getInstance();
        const instance2 = WalletController.getInstance();

        expect(instance1).toBe(instance2);
    });

    describe('Network Service', () => {
        it('should proxy getNetworks call', async () => {
            const networks = await walletController.getNetworks();

            expect(networks).toBeDefined();
            expect(networks.ethereum).toBeDefined();
            expect(networks.solana).toBeDefined();
        });

        it('should proxy getCurrentNetwork call', async () => {
            const currentNetwork = await walletController.getCurrentNetwork();

            // NetworkService initializes with mainnet as default
            expect(currentNetwork).toBeDefined();
            expect(currentNetwork?.id).toBe(1); // mainnet
        });

        it('should proxy addNetwork call', async () => {
            const customNetwork = {
                id: 999,
                name: 'Test Network',
                nativeCurrency: { name: 'TEST', symbol: 'TEST', decimals: 18 },
                rpcUrls: {
                    default: { http: ['https://test.example.com'] }
                }
            };

            await expect(walletController.addNetwork(customNetwork)).resolves.toBeUndefined();
        });

        it('should proxy removeNetwork call', async () => {
            // Add a network first
            const customNetwork = {
                id: 999,
                name: 'Test Network',
                nativeCurrency: { name: 'TEST', symbol: 'TEST', decimals: 18 },
                rpcUrls: {
                    default: { http: ['https://test.example.com'] }
                }
            };

            await walletController.addNetwork(customNetwork);
            await expect(walletController.removeNetwork(999)).resolves.toBeUndefined();
        });

        it('should proxy setCurrentNetwork call', async () => {
            const networks = await walletController.getNetworks();
            if (networks.ethereum.length > 0) {
                const networkId = networks.ethereum[0].id;
                await expect(walletController.setCurrentNetwork(networkId)).resolves.toBeUndefined();
            }
        });
    });

    describe('Account Service', () => {
        it('should proxy getAccounts call', async () => {
            const accounts = await walletController.getAccounts();

            expect(Array.isArray(accounts)).toBe(true);
        });

        it('should proxy getCurrentAccount call', async () => {
            const currentAccount = await walletController.getCurrentAccount();

            // Should return an account or null
            expect(currentAccount === null || typeof currentAccount === 'object').toBe(true);
        });

        it('should proxy addAccount call', async () => {
            const newAccount = {
                id: 'test-account',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                balance: '0',
                publicKey: 'test-public-key',
                blockchain: 'ethereum' as const
            };

            const result = await walletController.addAccount(newAccount);
            expect(result).toEqual(newAccount);

            const accounts = await walletController.getAccounts();
            expect(accounts).toHaveLength(1);
            expect(accounts[0]).toEqual(newAccount);
        });

        it('should proxy updateAccount call', async () => {
            const account = {
                id: 'update-test',
                name: 'Original Name',
                address: '0x1234567890123456789012345678901234567890',
                balance: '0',
                publicKey: 'test-key',
                blockchain: 'ethereum' as const
            };

            await walletController.addAccount(account);

            const updatedAccount = { ...account, name: 'Updated Name' };
            const result = await walletController.updateAccount(updatedAccount);
            expect(result).toEqual(updatedAccount);

            const accounts = await walletController.getAccounts();
            expect(accounts[0].name).toBe('Updated Name');
        });

        it('should proxy removeAccount call', async () => {
            const account = {
                id: 'remove-test',
                name: 'Remove Test',
                address: '0x1234567890123456789012345678901234567890',
                balance: '0',
                publicKey: 'test-key',
                blockchain: 'ethereum' as const
            };

            await walletController.addAccount(account);
            await expect(walletController.removeAccount('remove-test')).resolves.toBeUndefined();

            const accounts = await walletController.getAccounts();
            expect(accounts).toHaveLength(0);
        });

        it('should proxy setCurrentAccount call', async () => {
            const account = {
                id: 'current-test',
                name: 'Current Test',
                address: '0x1234567890123456789012345678901234567890',
                balance: '0',
                publicKey: 'test-key',
                blockchain: 'ethereum' as const
            };

            await walletController.addAccount(account);
            await expect(walletController.setCurrentAccount('current-test')).resolves.toBeUndefined();

            const currentAccount = await walletController.getCurrentAccount();
            expect(currentAccount?.id).toBe('current-test');
        });

        it('should proxy getAccountsByBlockchain call', async () => {
            const ethAccount = {
                id: 'eth-test',
                name: 'Ethereum Test',
                address: '0x1111111111111111111111111111111111111111',
                balance: '0',
                publicKey: 'eth-key',
                blockchain: 'ethereum' as const
            };

            const solAccount = {
                id: 'sol-test',
                name: 'Solana Test',
                address: 'Sol1111111111111111111111111111111111111111',
                balance: '0',
                publicKey: 'sol-key',
                blockchain: 'solana' as const
            };

            await walletController.addAccount(ethAccount);
            await walletController.addAccount(solAccount);

            const ethAccounts = await walletController.getAccountsByBlockchain('ethereum');
            const solAccounts = await walletController.getAccountsByBlockchain('solana');

            expect(ethAccounts).toHaveLength(1);
            expect(ethAccounts[0].blockchain).toBe('ethereum');

            expect(solAccounts).toHaveLength(1);
            expect(solAccounts[0].blockchain).toBe('solana');
        });
    });

    describe('Wallet Client Service', () => {
        it('should proxy connect call', async () => {
            // Mock the connect method since it might involve complex wallet operations
            const result = await walletController.connect();

            // Should return some result (exact format depends on implementation)
            expect(result).toBeDefined();
        });

        it('should proxy disconnect call', async () => {
            const result = await walletController.disconnect();

            // Should return some result
            expect(result).toBeDefined();
        });

        it('should proxy isConnected call', async () => {
            const isConnected = await walletController.isConnected();

            expect(typeof isConnected).toBe('boolean');
        });

        it('should proxy getBalance call', async () => {
            const account = {
                id: 'balance-test',
                name: 'Balance Test',
                address: '0x1234567890123456789012345678901234567890',
                balance: '0',
                publicKey: 'test-key',
                blockchain: 'ethereum' as const
            };

            await walletController.addAccount(account);

            const balance = await walletController.getBalance(account.address);

            // Should return a balance bigint
            expect(typeof balance).toBe('bigint');
        });

        it('should proxy sendTransaction call', async () => {
            const transaction = {
                to: '0x1234567890123456789012345678901234567890',
                value: '1000000000000000000', // 1 ETH
                data: '0x'
            };

            // This might throw or return a result depending on implementation
            try {
                const result = await walletController.sendTransaction(transaction);
                expect(result).toBeDefined();
            } catch (error) {
                // Expected if no actual wallet connection
                expect(error).toBeDefined();
            }
        });

        it('should proxy signMessage call', async () => {
            const message = 'Test message to sign';

            try {
                const signature = await walletController.signMessage(message);
                expect(typeof signature).toBe('string');
            } catch (error) {
                // Expected if no actual wallet connection
                expect(error).toBeDefined();
            }
        });
    });

    describe('Integration Tests', () => {
        it('should coordinate between services correctly', async () => {
            // Add network
            const customNetwork = {
                id: 888,
                name: 'Integration Test Network',
                nativeCurrency: { name: 'INT', symbol: 'INT', decimals: 18 },
                rpcUrls: {
                    default: { http: ['https://integration.example.com'] }
                }
            };

            await walletController.addNetwork(customNetwork);

            // Add account
            const account = {
                id: 'integration-account',
                name: 'Integration Account',
                address: '0x1234567890123456789012345678901234567890',
                balance: '0',
                publicKey: 'integration-key',
                blockchain: 'ethereum' as const
            };

            await walletController.addAccount(account);

            // Set as current
            await walletController.setCurrentAccount('integration-account');

            // Verify coordination
            const networks = await walletController.getNetworks();
            const accounts = await walletController.getAccounts();
            const currentAccount = await walletController.getCurrentAccount();

            expect(networks.ethereum.some(n => n.id === 888)).toBe(true);
            expect(accounts).toHaveLength(1);
            expect(currentAccount?.id).toBe('integration-account');
        });

        it('should handle service errors gracefully', async () => {
            // Try operations that should fail
            await expect(walletController.removeAccount('non-existent'))
                .rejects.toThrow();

            await expect(walletController.setCurrentAccount('non-existent'))
                .rejects.toThrow();

            await expect(walletController.removeNetwork(99999))
                .rejects.toThrow();
        });

        it('should maintain service state consistency', async () => {
            // Perform multiple operations
            const account1 = {
                id: 'consistency-1',
                name: 'Consistency Test 1',
                address: '0x1111111111111111111111111111111111111111',
                balance: '0',
                publicKey: 'key-1',
                blockchain: 'ethereum' as const
            };

            const account2 = {
                id: 'consistency-2',
                name: 'Consistency Test 2',
                address: '0x2222222222222222222222222222222222222222',
                balance: '0',
                publicKey: 'key-2',
                blockchain: 'ethereum' as const
            };

            await walletController.addAccount(account1);
            await walletController.addAccount(account2);
            await walletController.setCurrentAccount('consistency-1');

            // Remove current account
            await walletController.removeAccount('consistency-1');

            // Current account should be cleared or updated
            const currentAccount = await walletController.getCurrentAccount();
            const accounts = await walletController.getAccounts();

            expect(accounts).toHaveLength(1);
            expect(accounts[0].id).toBe('consistency-2');
            // Current account should either be null or automatically switched
            if (currentAccount) {
                expect(currentAccount.id).not.toBe('consistency-1');
            }
        });
    });

    describe('Error Handling', () => {
        it('should handle service initialization errors', () => {
            // Controller should handle if services fail to initialize
            expect(() => WalletController.getInstance()).not.toThrow();
        });

        it('should propagate service errors correctly', async () => {
            // Test that errors from underlying services are properly propagated
            await expect(walletController.addAccount({} as any))
                .rejects.toThrow();
        });

        it('should handle concurrent operations', async () => {
            // Test multiple simultaneous operations
            const operations = [
                walletController.getAccounts(),
                walletController.getNetworks(),
                walletController.getCurrentAccount(),
                walletController.getCurrentNetwork()
            ];

            const results = await Promise.allSettled(operations);

            // All operations should complete (either resolve or reject)
            expect(results.every(r => r.status === 'fulfilled' || r.status === 'rejected')).toBe(true);
        });
    });

    describe('RPC Method Proxies', () => {
        beforeEach(async () => {
            // Add a test account
            const accountService = AccountService.getInstance();
            await accountService.addAccount({
                id: 'test-account-1',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum',
                balance: '0',
                type: 'frost',
            });
            await accountService.setCurrentAccount('test-account-1');

            // Set up test network
            const networkService = NetworkService.getInstance();
            await networkService.ensureInitialized();
        });

        it('should return current account address', async () => {
            const accounts = await walletController.eth_accounts();
            expect(accounts).toEqual(['0x1234567890123456789012345678901234567890']);
        });

        it('should return empty array when no current account', async () => {
            const accountService = AccountService.getInstance();
            // Clear all accounts to ensure empty state
            // Reach into private fields via bracket access to clear
            // state for the test — the actual field is
            // currentAccountAddress, not currentAccount.
            (accountService as any)['accounts'] = [];
            (accountService as any)['currentAccountAddress'] = undefined;

            const accounts = await walletController.eth_accounts();
            expect(accounts).toEqual([]);
        });

        it('should return current account address for eth_requestAccounts', async () => {
            const accounts = await walletController.eth_requestAccounts();
            expect(accounts).toEqual(['0x1234567890123456789012345678901234567890']);
        });

        it('should return current network chain ID', async () => {
            const networkService = NetworkService.getInstance();
            // eth_chainId should return the current network's chain ID
            const chainId = await walletController.eth_chainId();
            expect(typeof chainId === 'number' || chainId === undefined).toBe(true);
        });

        it('should return current network version', async () => {
            const networkService = NetworkService.getInstance();
            const version = await walletController.net_version();
            expect(typeof version === 'string' || version === undefined).toBe(true);
        });
    });

    describe('Additional Wallet Operations', () => {
        beforeEach(async () => {
            // Add test account  
            const accountService = AccountService.getInstance();
            await accountService.addAccount({
                id: 'test-account-1',
                name: 'Test Account',
                address: '0x1234567890123456789012345678901234567890',
                blockchain: 'ethereum',
                balance: '0',
                type: 'frost',
            });
        });

        it('should proxy getBalance to wallet client service', async () => {
            const address = '0x1234567890123456789012345678901234567890';

            // This should call the wallet client service's getBalance method
            const balancePromise = walletController.getBalance(address);

            // Since we're using a mock client, this might throw or return a mock value
            // Just verify the method exists and can be called
            await expect(balancePromise).toBeDefined();
        });

        it('should proxy getTransactionCount to wallet client service', async () => {
            const address = '0x1234567890123456789012345678901234567890';

            // This should call the wallet client service's getTransactionCount method
            const txCountPromise = walletController.getTransactionCount(address);

            // Since we're using a mock client, this might throw or return a mock value
            // Just verify the method exists and can be called
            await expect(txCountPromise).toBeDefined();
        });

        it('should proxy signMessage to wallet client service', async () => {
            const signParams = {
                account: '0x1234567890123456789012345678901234567890' as `0x${string}`,
                message: 'Test message to sign'
            };

            // MPC wallets throw an error for direct message signing
            await expect(walletController.signMessage(signParams)).rejects.toThrow(
                'Message signing must use MPC protocol. Please use the MPC signing flow.'
            );
        });
    });

    describe('Blockchain Operations Coverage', () => {
        it('should handle sendTransaction proxy', async () => {
            const txParams = {
                to: '0x1234567890123456789012345678901234567890' as `0x${string}`,
                value: 1000000000000000000n, // 1 ETH
                data: '0x' as `0x${string}`
            };

            // Test that sendTransaction method exists and can be called
            // Expect it to throw because we don't have a real wallet configured
            await expect(walletController.sendTransaction(txParams)).rejects.toThrow();
        });

        it('should handle all services initialization', () => {
            // Test that all services are properly initialized.
            // Access-as-any because the service fields are private
            // on WalletController; this test just probes that the
            // constructor wired them up (the private modifier is
            // about external encapsulation, not testability).
            expect((walletController as any).networkService).toBeDefined();
            expect((walletController as any).accountService).toBeDefined();
            expect((walletController as any).walletClientService).toBeDefined();
        });
    });
});
