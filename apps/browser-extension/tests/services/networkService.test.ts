import { describe, it, expect, beforeEach, afterEach, mock } from 'bun:test';
import NetworkService from '../../src/services/networkService';
import { mainnet, sepolia, polygon, arbitrum } from 'viem/chains';
import type { Chain } from '@mpc-wallet/types/network';

// Create comprehensive mock for Chrome storage
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

// Mock viem clients to avoid network calls. Each call returns a FRESH
// wrapper object (distinct identity, required by "different clients"
// tests) but all wrappers share the same underlying method mocks so
// connectivity tests can assert `.toHaveBeenCalled()` on a stable
// reference.
const mockPublicClient = {
    getBlockNumber: mock(async () => 18000000n),
    getChainId: mock(async () => 1),
    getBalance: mock(async () => 1000000000000000000n), // 1 ETH
};
const createPublicClientMock = mock(() => ({ ...mockPublicClient }));

// mock.module avoids the ESM readonly-export trap: direct property
// assignment on an imported module throws at top-level and takes down
// every other test file via "unhandled error between tests".
mock.module('viem', () => ({
    createPublicClient: createPublicClientMock,
    http: mock(() => ({})),
    custom: mock(() => ({})),
    createWalletClient: mock(() => ({ account: { address: '0x0' } })),
}));

describe('NetworkService', () => {
    let networkService: NetworkService;

    beforeEach(async () => {
        // Re-install file-local mockStorage onto chrome.storage.local.
        // setup-bun.ts beforeEach replaces chrome.storage.local with its
        // own mock; re-assign here so our spy is the one the service hits.
        (global as any).chrome.storage.local = mockStorage;

        // Clear mock storage and reset mocks
        await mockStorage.clear();
        mockStorage.get.mockClear();
        mockStorage.set.mockClear();
        createPublicClientMock.mockClear();

        // Reset singleton instance
        NetworkService.resetInstance();

        // Get fresh instance
        networkService = NetworkService.getInstance();
        await networkService.ensureInitialized();
    });

    afterEach(async () => {
        await mockStorage.clear();
        NetworkService.resetInstance();
    });

    describe('Singleton Pattern', () => {
        it('should return the same instance', () => {
            const instance1 = NetworkService.getInstance();
            const instance2 = NetworkService.getInstance();
            expect(instance1).toBe(instance2);
        });

        it('should reset instance for testing', () => {
            const instance1 = NetworkService.getInstance();
            NetworkService.resetInstance();
            const instance2 = NetworkService.getInstance();
            expect(instance1).not.toBe(instance2);
        });
    });

    describe('Network Management', () => {
        it('should initialize with default networks', async () => {
            const networks = networkService.getNetworks();
            
            expect(networks.ethereum).toBeDefined();
            expect(networks.solana).toBeDefined();
            expect(Array.isArray(networks.ethereum)).toBe(true);
            expect(Array.isArray(networks.solana)).toBe(true);

            // Should include default networks (mainnet, sepolia)
            const ethereumNetworks = networks.ethereum;
            expect(ethereumNetworks.length).toBeGreaterThanOrEqual(2);
            
            const mainnetFound = ethereumNetworks.find((n: Chain) => n.id === mainnet.id);
            const sepoliaFound = ethereumNetworks.find((n: Chain) => n.id === sepolia.id);
            
            expect(mainnetFound).toBeDefined();
            expect(sepoliaFound).toBeDefined();
        });

        it('should add custom network successfully', async () => {
            const customNetwork: Chain = {
                id: 123456,
                name: 'Custom Network',
                network: 'custom',
                nativeCurrency: {
                    name: 'Custom Token',
                    symbol: 'CTK',
                    decimals: 18
                },
                rpcUrls: {
                    default: { http: ['https://custom-rpc.example.com'] },
                    public: { http: ['https://custom-rpc.example.com'] }
                }
            };

            await networkService.addCustomNetwork('ethereum', customNetwork);
            
            const networks = networkService.getNetworks();
            const customFound = networks.ethereum.find((n: Chain) => n.id === 123456);
            
            expect(customFound).toBeDefined();
            expect(customFound?.name).toBe('Custom Network');
        });

        it('should reject duplicate network IDs', async () => {
            // Cast to any — viem's `mainnet` has a different shape
            // than our Chain (viem omits the `network` field which
            // our Chain type requires). The test just needs the
            // same `.id`, not full interface conformance.
            const duplicateMainnet: Chain = {
                ...mainnet,
                name: 'Fake Mainnet',
            } as any;

            await expect(networkService.addCustomNetwork('ethereum', duplicateMainnet))
                .rejects.toThrow('Network with this ID already exists');
        });

        it('should remove custom networks', async () => {
            const customNetwork: Chain = {
                id: 999999,
                name: 'Removable Network',
                network: 'removable',
                nativeCurrency: {
                    name: 'Removable Token',
                    symbol: 'RTK',
                    decimals: 18
                },
                rpcUrls: {
                    default: { http: ['https://removable-rpc.example.com'] },
                    public: { http: ['https://removable-rpc.example.com'] }
                }
            };

            await networkService.addCustomNetwork('ethereum', customNetwork);
            
            // Verify it was added
            let networks = networkService.getNetworks();
            expect(networks.ethereum.find((n: Chain) => n.id === 999999)).toBeDefined();

            // Remove it
            await networkService.removeCustomNetwork('ethereum', 999999);
            
            // Verify it was removed
            networks = networkService.getNetworks();
            expect(networks.ethereum.find((n: Chain) => n.id === 999999)).toBeUndefined();
        });

        it('should not remove protected networks', async () => {
            await expect(networkService.removeCustomNetwork('ethereum', mainnet.id))
                .rejects.toThrow('Cannot remove protected network');
                
            await expect(networkService.removeCustomNetwork('ethereum', sepolia.id))
                .rejects.toThrow('Cannot remove protected network');
        });
    });

    describe('Current Network Management', () => {
        it('should set and get current network', async () => {
            const networks = networkService.getNetworks();
            const mainnetNetwork = networks.ethereum.find((n: Chain) => n.id === mainnet.id);
            
            expect(mainnetNetwork).toBeDefined();

            await networkService.setCurrentNetwork('ethereum', mainnetNetwork!);
            const currentNetwork = networkService.getCurrentNetwork('ethereum');
            
            expect(currentNetwork?.id).toBe(mainnet.id);
            expect(currentNetwork?.name).toBe(mainnet.name);
        });

        it('should handle different blockchains separately', async () => {
            const networks = networkService.getNetworks();
            const mainnetNetwork = networks.ethereum.find((n: Chain) => n.id === mainnet.id);
            
            // Set current ethereum network
            await networkService.setCurrentNetwork('ethereum', mainnetNetwork!);
            
            // Ethereum should have current network, solana should not
            expect(networkService.getCurrentNetwork('ethereum')).toBeDefined();
            expect(networkService.getCurrentNetwork('solana')).toBeUndefined();
        });

        it('should switch blockchain', async () => {
            expect(networkService.getCurrentBlockchain()).toBe('ethereum');
            
            await networkService.setCurrentBlockchain('solana');
            expect(networkService.getCurrentBlockchain()).toBe('solana');
        });

        it('should get network by chain ID', () => {
            const mainnetNetwork = networkService.getNetworkByChainId(mainnet.id);
            const sepoliaNetwork = networkService.getNetworkByChainId(sepolia.id);
            
            expect(mainnetNetwork?.name).toBe(mainnet.name);
            expect(sepoliaNetwork?.name).toBe(sepolia.name);
        });

        it('should return undefined for unknown chain ID', () => {
            const unknownNetwork = networkService.getNetworkByChainId(999999);
            expect(unknownNetwork).toBeUndefined();
        });
    });

    describe('RPC Client Management', () => {
        it('should create RPC client for valid network', () => {
            const networks = networkService.getNetworks();
            const mainnetNetwork = networks.ethereum.find((n: Chain) => n.id === mainnet.id);
            
            expect(mainnetNetwork).toBeDefined();

            const client = networkService.getPublicClient(mainnetNetwork!);
            expect(client).toBeDefined();
            expect(createPublicClientMock).toHaveBeenCalled();
        });

        it('should use cached clients', () => {
            const networks = networkService.getNetworks();
            const mainnetNetwork = networks.ethereum.find((n: Chain) => n.id === mainnet.id);
            
            // Call twice
            const client1 = networkService.getPublicClient(mainnetNetwork!);
            const client2 = networkService.getPublicClient(mainnetNetwork!);
            
            expect(client1).toBe(client2); // Should be same instance
            expect(createPublicClientMock).toHaveBeenCalledTimes(1); // Should only create once
        });

        it('should create different clients for different networks', () => {
            const networks = networkService.getNetworks();
            const mainnetNetwork = networks.ethereum.find((n: Chain) => n.id === mainnet.id);
            const sepoliaNetwork = networks.ethereum.find((n: Chain) => n.id === sepolia.id);
            
            const mainnetClient = networkService.getPublicClient(mainnetNetwork!);
            const sepoliaClient = networkService.getPublicClient(sepoliaNetwork!);
            
            expect(mainnetClient).not.toBe(sepoliaClient);
            expect(createPublicClientMock).toHaveBeenCalledTimes(2);
        });
    });

    describe('Network Validation', () => {
        it('should validate network connectivity', async () => {
            const networks = networkService.getNetworks();
            const mainnetNetwork = networks.ethereum.find((n: Chain) => n.id === mainnet.id);
            
            const isConnected = await networkService.testNetworkConnectivity(mainnetNetwork!);
            expect(isConnected).toBe(true);
            expect(mockPublicClient.getBlockNumber).toHaveBeenCalled();
        });

        it('should handle network connectivity failures', async () => {
            // Mock network failure
            mockPublicClient.getBlockNumber.mockRejectedValueOnce(new Error('Network error'));
            
            const networks = networkService.getNetworks();
            const mainnetNetwork = networks.ethereum.find((n: Chain) => n.id === mainnet.id);
            
            const isConnected = await networkService.testNetworkConnectivity(mainnetNetwork!);
            expect(isConnected).toBe(false);
        });

        it('should validate network configuration', () => {
            const validNetwork: Chain = {
                id: 123,
                name: 'Test Network',
                network: 'test',
                nativeCurrency: {
                    name: 'Test Token',
                    symbol: 'TEST',
                    decimals: 18
                },
                rpcUrls: {
                    default: { http: ['https://rpc.test.com'] },
                    public: { http: ['https://rpc.test.com'] }
                }
            };

            const isValid = networkService.validateNetworkConfig(validNetwork);
            expect(isValid).toBe(true);
        });

        it('should reject invalid network configuration', () => {
            const invalidNetwork = {
                id: 'invalid', // Should be number
                name: 'Invalid Network',
                rpcUrls: {
                    default: { http: [] } // Empty RPC URLs
                }
            } as any;

            const isValid = networkService.validateNetworkConfig(invalidNetwork);
            expect(isValid).toBe(false);
        });
    });

    describe('Event System', () => {
        let changeCallback: any;

        beforeEach(() => {
            changeCallback = mock(() => {});
        });

        it('should register network change callback', () => {
            networkService.onNetworkChange(changeCallback);
            expect(changeCallback).not.toHaveBeenCalled();
        });

        it('should trigger callback when network changes', async () => {
            networkService.onNetworkChange(changeCallback);
            
            const networks = networkService.getNetworks();
            const sepoliaNetwork = networks.ethereum.find((n: Chain) => n.id === sepolia.id);
            
            await networkService.setCurrentNetwork('ethereum', sepoliaNetwork!);
            
            expect(changeCallback).toHaveBeenCalledWith(sepoliaNetwork);
        });

        it('should remove callback correctly', async () => {
            networkService.onNetworkChange(changeCallback);
            networkService.offNetworkChange(changeCallback);
            
            const networks = networkService.getNetworks();
            const sepoliaNetwork = networks.ethereum.find((n: Chain) => n.id === sepolia.id);
            
            await networkService.setCurrentNetwork('ethereum', sepoliaNetwork!);
            
            expect(changeCallback).not.toHaveBeenCalled();
        });
    });

    describe('Persistence', () => {
        it('should persist networks to storage', async () => {
            const customNetwork: Chain = {
                id: 555555,
                name: 'Persistent Network',
                network: 'persistent',
                nativeCurrency: {
                    name: 'Persistent Token',
                    symbol: 'PER',
                    decimals: 18
                },
                rpcUrls: {
                    default: { http: ['https://persistent-rpc.example.com'] },
                    public: { http: ['https://persistent-rpc.example.com'] }
                }
            };

            await networkService.addCustomNetwork('ethereum', customNetwork);

            expect(mockStorage.set).toHaveBeenCalledWith(
                expect.objectContaining({
                    wallet_networks: expect.objectContaining({
                        ethereum: expect.arrayContaining([
                            expect.objectContaining({ id: 555555 })
                        ])
                    })
                })
            );
        });

        it('should persist current network selection', async () => {
            const networks = networkService.getNetworks();
            const sepoliaNetwork = networks.ethereum.find((n: Chain) => n.id === sepolia.id);
            
            await networkService.setCurrentNetwork('ethereum', sepoliaNetwork!);

            expect(mockStorage.set).toHaveBeenCalledWith(
                expect.objectContaining({
                    wallet_current_networks: expect.objectContaining({
                        ethereum: sepoliaNetwork
                    })
                })
            );
        });

        it('should load networks from storage on initialization', async () => {
            const customNetwork: Chain = {
                id: 777777,
                name: 'Stored Network',
                network: 'stored',
                nativeCurrency: {
                    name: 'Stored Token',
                    symbol: 'STO',
                    decimals: 18
                },
                rpcUrls: {
                    default: { http: ['https://stored-rpc.example.com'] },
                    public: { http: ['https://stored-rpc.example.com'] }
                }
            };

            mockStorage.data = {
                wallet_networks: {
                    ethereum: [mainnet, sepolia, customNetwork],
                    solana: []
                },
                wallet_current_networks: {
                    ethereum: customNetwork,
                    solana: undefined
                },
                wallet_current_blockchain: 'ethereum'
            };

            // Create new service instance to trigger loading
            NetworkService.resetInstance();
            const newService = NetworkService.getInstance();
            await newService.ensureInitialized();

            const networks = newService.getNetworks();
            const storedNetwork = networks.ethereum.find((n: Chain) => n.id === 777777);
            const currentNetwork = newService.getCurrentNetwork('ethereum');

            expect(storedNetwork).toBeDefined();
            expect(storedNetwork?.name).toBe('Stored Network');
            expect(currentNetwork?.id).toBe(777777);
        });
    });

    describe('Error Handling', () => {
        it('should handle storage errors gracefully', async () => {
            mockStorage.set.mockRejectedValueOnce(new Error('Storage error'));

            const customNetwork: Chain = {
                id: 888888,
                name: 'Error Network',
                network: 'error',
                nativeCurrency: {
                    name: 'Error Token',
                    symbol: 'ERR',
                    decimals: 18
                },
                rpcUrls: {
                    default: { http: ['https://error-rpc.example.com'] },
                    public: { http: ['https://error-rpc.example.com'] }
                }
            };

            await expect(networkService.addCustomNetwork('ethereum', customNetwork))
                .rejects.toThrow('Storage error');
        });

        it('should handle corrupted storage data', async () => {
            mockStorage.data = {
                wallet_networks: 'invalid-data', // Should be object
                wallet_current_networks: null,
                wallet_current_blockchain: 'invalid'
            };

            NetworkService.resetInstance();
            const newService = NetworkService.getInstance();
            await newService.ensureInitialized();

            const networks = newService.getNetworks();
            expect(networks.ethereum.length).toBeGreaterThanOrEqual(2); // Should have defaults
            expect(newService.getCurrentBlockchain()).toBe('ethereum'); // Should default
        });

        it('should handle missing storage API', async () => {
            // Temporarily remove chrome.storage. Use try/finally so any
            // exception in the body can't leave chrome deleted — setup-bun's
            // beforeEach would then fail in every subsequent test with
            // "chrome is not defined".
            const originalChrome = (global as any).chrome;
            try {
                delete (global as any).chrome;

                NetworkService.resetInstance();
                const newService = NetworkService.getInstance();
                await newService.ensureInitialized();

                const networks = newService.getNetworks();
                expect(networks.ethereum.length).toBeGreaterThanOrEqual(2); // Should still have defaults
            } finally {
                (global as any).chrome = originalChrome;
            }
        });
    });

    describe('Chain Detection and Mapping', () => {
        it('should map supported chains to blockchains correctly', () => {
            // Test blockchain mapping (internal function, but observable through behavior)
            const networks = networkService.getNetworks();
            
            // Ethereum-based chains should be in ethereum networks
            expect(networks.ethereum.find(n => n.id === mainnet.id)).toBeDefined();
            expect(networks.ethereum.find(n => n.id === sepolia.id)).toBeDefined();
        });

        it('should handle network switching across different chains', async () => {
            // Add polygon network (Ethereum-compatible). Cast —
            // viem's polygon lacks the `network` field our Chain
            // type requires.
            const polygonNetwork: Chain = {
                ...polygon,
                rpcUrls: {
                    default: { http: ['https://polygon-rpc.com'] },
                    public: { http: ['https://polygon-rpc.com'] }
                }
            } as any;

            await networkService.addCustomNetwork('ethereum', polygonNetwork);
            await networkService.setCurrentNetwork('ethereum', polygonNetwork);

            const currentNetwork = networkService.getCurrentNetwork('ethereum');
            expect(currentNetwork?.id).toBe(polygon.id);
            expect(currentNetwork?.name).toBe(polygon.name);
        });
    });

    describe('Network Utilities', () => {
        it('should get all networks across blockchains', () => {
            const allNetworks = networkService.getAllNetworks();
            
            expect(allNetworks.length).toBeGreaterThanOrEqual(2);
            expect(allNetworks.some(n => n.id === mainnet.id)).toBe(true);
            expect(allNetworks.some(n => n.id === sepolia.id)).toBe(true);
        });

        it('should check if network exists', () => {
            expect(networkService.networkExists(mainnet.id)).toBe(true);
            expect(networkService.networkExists(sepolia.id)).toBe(true);
            expect(networkService.networkExists(999999)).toBe(false);
        });

        it('should get supported RPC URLs for network', () => {
            const networks = networkService.getNetworks();
            const mainnetNetwork = networks.ethereum.find((n: Chain) => n.id === mainnet.id);
            
            const rpcUrls = networkService.getNetworkRPCUrls(mainnetNetwork!);
            
            expect(Array.isArray(rpcUrls)).toBe(true);
            expect(rpcUrls.length).toBeGreaterThan(0);
            expect(rpcUrls.every(url => typeof url === 'string')).toBe(true);
        });
    });
});