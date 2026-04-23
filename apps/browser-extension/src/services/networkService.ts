import { mainnet, sepolia } from 'viem/chains';
import type { Chain } from "@mpc-wallet/types/network";
import type { SupportedChain } from "@mpc-wallet/types/appstate";
import { createPublicClient, http } from 'viem';

type NetworkChangeCallback = (network: Chain | undefined) => void;
type LegacyBlockchain = 'ethereum' | 'solana';

/**
 * Maps supported chains to their underlying blockchain infrastructure.
 * This enables backward compatibility while supporting Layer 2 chains.
 */
const CHAIN_TO_BLOCKCHAIN: Record<SupportedChain, LegacyBlockchain> = {
    // secp256k1-based chains -> use ethereum infrastructure
    ethereum: 'ethereum',
    polygon: 'ethereum',
    arbitrum: 'ethereum',
    optimism: 'ethereum',
    base: 'ethereum',
    // ed25519-based chains -> use solana infrastructure  
    solana: 'solana',
    sui: 'solana',
};

/**
 * Helper function to get the blockchain type for a given chain.
 */
function getBlockchainForChain(chain: SupportedChain): LegacyBlockchain {
    return CHAIN_TO_BLOCKCHAIN[chain];
}

class NetworkService {
    private static instance: NetworkService;
    private networks: Record<string, Chain[]> = {
        ethereum: [],
        solana: []
    };
    private currentBlockchain: 'ethereum' | 'solana' = 'ethereum';
    private currentNetworks: Record<string, Chain | undefined> = {
        ethereum: undefined,
        solana: undefined
    };
    private readonly STORAGE_KEY = 'wallet_networks';
    private readonly CURRENT_NETWORK_KEY = 'wallet_current_networks';
    private readonly CURRENT_BLOCKCHAIN_KEY = 'wallet_current_blockchain';
    private changeCallbacks: NetworkChangeCallback[] = [];
    private initialized: boolean = false;

    // Protected network chainIds by blockchain
    private static readonly PROTECTED_NETWORK_IDS: Record<string, number[]> = {
        ethereum: [mainnet.id, sepolia.id],
        solana: []
    };

    private constructor() {
        this.initializeAsync();
    }

    public static getInstance(): NetworkService {
        if (!NetworkService.instance) {
            NetworkService.instance = new NetworkService();
        }
        return NetworkService.instance;
    }

    private async initializeAsync(): Promise<void> {
        if (!this.initialized) {
            await this.loadNetworks();
            this.initialized = true;
        }
    }

    public async ensureInitialized(): Promise<void> {
        if (!this.initialized) {
            await this.initializeAsync();
        }
    }

    // Testing utility method to reset singleton instance
    public static resetInstance(): void {
        NetworkService.instance = undefined as any;
    }

    private async loadNetworks(): Promise<void> {
        try {
            // Load stored networks
            if (typeof chrome !== 'undefined' && chrome.storage) {
                const result = await chrome.storage.local.get([
                    this.STORAGE_KEY,
                    this.CURRENT_BLOCKCHAIN_KEY,
                    this.CURRENT_NETWORK_KEY
                ]);
                const storedNetworks = result[this.STORAGE_KEY];

                // Initialize with default structure if not found or invalid
                if (!storedNetworks || typeof storedNetworks !== 'object') {
                    this.networks = { ethereum: [], solana: [] };
                } else {
                    // Ensure the networks object has the correct structure
                    this.networks = {
                        ethereum: Array.isArray(storedNetworks.ethereum) ? storedNetworks.ethereum : [],
                        solana: Array.isArray(storedNetworks.solana) ? storedNetworks.solana : []
                    };
                }

                // Load current blockchain
                this.currentBlockchain = result[this.CURRENT_BLOCKCHAIN_KEY] || 'ethereum';

                // Load current networks for each blockchain
                this.currentNetworks = result[this.CURRENT_NETWORK_KEY] || { ethereum: undefined, solana: undefined };

                // Add default networks if none exist for the current blockchain
                if (!this.networks.ethereum || this.networks.ethereum.length === 0) {
                    const defaultEthNetworks = this.getDefaultNetworks('ethereum');
                    for (const network of defaultEthNetworks) {
                        // Check if network already exists before adding
                        const exists = this.networks.ethereum.some(n => n.id === network.id);
                        if (!exists) {
                            await this.addNetwork('ethereum', network);
                        }
                    }
                }

                // If no current network is set for ethereum, set to mainnet
                if (!this.currentNetworks.ethereum) {
                    const mainnetNetwork = this.networks.ethereum.find(n => n.id === mainnet.id);
                    if (mainnetNetwork) {
                        await this.setCurrentNetwork('ethereum', mainnetNetwork.id);
                    }
                }
            } else {
                // Fallback for test environment
                this.networks = { ethereum: [], solana: [] };
                this.currentBlockchain = 'ethereum';
                this.currentNetworks = { ethereum: undefined, solana: undefined };
            }
        } catch (error) {
            console.error('Failed to load networks:', error);
            this.networks = { ethereum: [], solana: [] };
        }
    }

    private getDefaultNetworks(blockchain: 'ethereum' | 'solana'): Chain[] {
        if (blockchain === 'ethereum') {
            return [
                {
                    id: mainnet.id,
                    name: 'Mainnet',
                    network: 'mainnet',
                    nativeCurrency: {
                        name: mainnet.nativeCurrency.name,
                        symbol: mainnet.nativeCurrency.symbol,
                        decimals: mainnet.nativeCurrency.decimals
                    },
                    rpcUrls: {
                        default: {
                            http: [...(mainnet.rpcUrls.default.http as readonly string[])]
                        }
                    },
                    blockExplorers: {
                        default: {
                            name: mainnet.blockExplorers.default.name,
                            url: mainnet.blockExplorers.default.url
                        }
                    }
                },
                {
                    id: sepolia.id,
                    name: sepolia.name,
                    network: 'sepolia',
                    nativeCurrency: {
                        name: sepolia.nativeCurrency.name,
                        symbol: sepolia.nativeCurrency.symbol,
                        decimals: sepolia.nativeCurrency.decimals
                    },
                    rpcUrls: {
                        default: {
                            http: [...(sepolia.rpcUrls.default.http as readonly string[])]
                        }
                    },
                    blockExplorers: {
                        default: {
                            name: sepolia.blockExplorers.default.name,
                            url: sepolia.blockExplorers.default.url
                        }
                    }
                }
            ];
        }
        // Add default Solana networks if needed
        return [];
    }

    private async saveNetworks(): Promise<void> {
        try {
            if (typeof chrome !== 'undefined' && chrome.storage) {
                await chrome.storage.local.set({ [this.STORAGE_KEY]: this.networks });
            }
        } catch (error) {
            console.error('Failed to save networks:', error);
        }
    }

    private async saveCurrentNetworks(): Promise<void> {
        try {
            if (typeof chrome !== 'undefined' && chrome.storage) {
                await chrome.storage.local.set({ [this.CURRENT_NETWORK_KEY]: this.currentNetworks });
            }
        } catch (error) {
            console.error('Failed to save current networks:', error);
        }
    }

    private async saveCurrentBlockchain(): Promise<void> {
        try {
            if (typeof chrome !== 'undefined' && chrome.storage) {
                await chrome.storage.local.set({ [this.CURRENT_BLOCKCHAIN_KEY]: this.currentBlockchain });
            }
        } catch (error) {
            console.error('Failed to save current blockchain:', error);
        }
    }

    public async addNetwork(blockchain: 'ethereum' | 'solana', network: Chain): Promise<void> {
        // Validate network structure
        if (!network || typeof network !== 'object') {
            throw new Error('Invalid network: must be an object');
        }

        if (typeof network.id !== 'number') {
            throw new Error('Invalid network: id is required and must be a number');
        }

        if (!network.name || typeof network.name !== 'string') {
            throw new Error('Invalid network: name is required and must be a string');
        }

        if (this.networks[blockchain].some(n => n.id === network.id)) {
            throw new Error('Network with this ID already exists');
        }

        this.networks[blockchain].push(network);
        await this.saveNetworks();
    }

    public async removeNetwork(blockchain: 'ethereum' | 'solana', chainId: number): Promise<void> {
        const network = this.networks[blockchain].find(n => n.id === chainId);
        if (!network) {
            throw new Error('Network not found');
        }

        // Cannot remove protected networks
        if (NetworkService.PROTECTED_NETWORK_IDS[blockchain].includes(chainId)) {
            throw new Error('Cannot remove protected network');
        }

        // If removing the current network, switch to a default one
        if (this.currentNetworks[blockchain]?.id === chainId) {
            if (blockchain === 'ethereum') {
                const mainnetNetwork = this.networks[blockchain].find(n => n.id === mainnet.id);
                if (mainnetNetwork) {
                    await this.setCurrentNetwork(blockchain, mainnetNetwork.id);
                }
            }
        }

        this.networks[blockchain] = this.networks[blockchain].filter(n => n.id !== chainId);
        await this.saveNetworks();
    }

    public async updateNetwork(blockchain: 'ethereum' | 'solana', network: Chain): Promise<void> {
        const index = this.networks[blockchain].findIndex(n => n.id === network.id);
        if (index === -1) {
            throw new Error('Network not found');
        }

        // Cannot modify protected networks
        if (NetworkService.PROTECTED_NETWORK_IDS[blockchain].includes(network.id)) {
            throw new Error('Cannot modify protected network');
        }

        this.networks[blockchain][index] = network;
        await this.saveNetworks();
    }

    /**
     * Function overloads so TypeScript narrows the return type based
     * on whether a blockchain arg is supplied. Previously the union
     * return (`Chain[] | Record<string, Chain[]>`) forced every caller
     * to type-assert; now `getNetworks()` returns the map and
     * `getNetworks('ethereum')` returns a Chain[] directly.
     */
    public getNetworks(): Record<'ethereum' | 'solana', Chain[]>;
    public getNetworks(blockchain: 'ethereum' | 'solana'): Chain[];
    public getNetworks(
        blockchain?: 'ethereum' | 'solana',
    ): Chain[] | Record<'ethereum' | 'solana', Chain[]> {
        if (blockchain) {
            return this.networks[blockchain];
        }
        return this.networks;
    }

    public getNetwork(blockchain: 'ethereum' | 'solana', chainId: number): Chain | undefined {
        return this.networks[blockchain].find(n => n.id === chainId);
    }

    public getCurrentNetwork(blockchain?: 'ethereum' | 'solana'): Chain | undefined {
        if (blockchain) {
            return this.currentNetworks[blockchain];
        }
        return this.currentNetworks[this.currentBlockchain];
    }

    public getCurrentBlockchain(): 'ethereum' | 'solana' {
        return this.currentBlockchain;
    }

    public async setCurrentBlockchain(blockchain: 'ethereum' | 'solana'): Promise<void> {
        this.currentBlockchain = blockchain;
        await this.saveCurrentBlockchain();
        this.notifyNetworkChange(this.currentNetworks[blockchain]);
    }

    public async setCurrentNetwork(blockchain: 'ethereum' | 'solana', chainId: number): Promise<void> {
        const network = this.networks[blockchain].find(n => n.id === chainId);
        if (!network) {
            throw new Error('Network not found');
        }

        this.currentNetworks[blockchain] = network;
        await this.saveCurrentNetworks();
        this.notifyNetworkChange(this.currentNetworks[blockchain]);
    }

    public async clearNetworks(blockchain: 'ethereum' | 'solana'): Promise<void> {
        // Only keep protected networks for the specified blockchain
        this.networks[blockchain] = this.networks[blockchain].filter(n =>
            NetworkService.PROTECTED_NETWORK_IDS[blockchain].includes(n.id)
        );
        await this.saveNetworks();
    }

    public onNetworkChange(callback: NetworkChangeCallback): void {
        this.changeCallbacks.push(callback);
    }

    public removeNetworkChangeListener(callback: NetworkChangeCallback): void {
        this.changeCallbacks = this.changeCallbacks.filter(cb => cb !== callback);
    }

    private notifyNetworkChange(network: Chain | undefined): void {
        this.changeCallbacks.forEach(callback => callback(network));
    }

    // Helper method to get a public client for the current network
    public getPublicClient() {
        if (this.currentBlockchain !== 'ethereum') {
            throw new Error('Public client is only available for Ethereum networks');
        }

        const currentNetwork = this.currentNetworks.ethereum;
        if (!currentNetwork) {
            throw new Error('No current Ethereum network selected');
        }

        // Convert our Chain to viem's Chain format
        const viemChain: any = {
            id: currentNetwork.id,
            name: currentNetwork.name,
            network: currentNetwork.network,
            nativeCurrency: currentNetwork.nativeCurrency || {
                name: 'Ether',
                symbol: 'ETH',
                decimals: 18
            },
            rpcUrls: currentNetwork.rpcUrls || {
                default: { http: [] },
                public: { http: [] }
            },
            blockExplorers: currentNetwork.blockExplorers
        };

        return createPublicClient({
            chain: viemChain,
            transport: http()
        });
    }

    // ===================================================================
    // ENHANCED METHODS FOR MULTI-CHAIN SUPPORT
    // These methods provide backward compatibility while supporting
    // Layer 2 chains and the new SupportedChain type.
    // ===================================================================

    /**
     * Get networks for a supported chain (with Layer 2 support).
     * This method maps Layer 2 chains to their underlying blockchain.
     */
    public getNetworksForChain(chain: SupportedChain): Chain[] {
        const blockchain = getBlockchainForChain(chain);
        return this.networks[blockchain];
    }

    /**
     * Get current network for a supported chain.
     */
    public getCurrentNetworkForChain(chain: SupportedChain): Chain | undefined {
        const blockchain = getBlockchainForChain(chain);
        return this.currentNetworks[blockchain];
    }

    /**
     * Set current network for a supported chain.
     */
    public async setCurrentNetworkForChain(chain: SupportedChain, chainId: number): Promise<void> {
        const blockchain = getBlockchainForChain(chain);
        await this.setCurrentNetwork(blockchain, chainId);
    }

    /**
     * Add network for a supported chain.
     */
    public async addNetworkForChain(chain: SupportedChain, network: Chain): Promise<void> {
        const blockchain = getBlockchainForChain(chain);
        await this.addNetwork(blockchain, network);
    }
}

export default NetworkService;