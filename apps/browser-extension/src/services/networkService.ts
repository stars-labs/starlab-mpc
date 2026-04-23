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
            if (typeof chrome !== 'undefined' && chrome.storage) {
                const result = await chrome.storage.local.get([
                    this.STORAGE_KEY,
                    this.CURRENT_BLOCKCHAIN_KEY,
                    this.CURRENT_NETWORK_KEY
                ]);
                const storedNetworks = result[this.STORAGE_KEY];

                // Defensive: corrupted storage (non-object) resets to empty.
                if (!storedNetworks || typeof storedNetworks !== 'object') {
                    this.networks = { ethereum: [], solana: [] };
                } else {
                    this.networks = {
                        ethereum: Array.isArray(storedNetworks.ethereum) ? storedNetworks.ethereum : [],
                        solana: Array.isArray(storedNetworks.solana) ? storedNetworks.solana : []
                    };
                }

                // Validate currentBlockchain against allowed values.
                const storedBlockchain = result[this.CURRENT_BLOCKCHAIN_KEY];
                this.currentBlockchain = (storedBlockchain === 'ethereum' || storedBlockchain === 'solana')
                    ? storedBlockchain
                    : 'ethereum';

                this.currentNetworks = result[this.CURRENT_NETWORK_KEY] || { ethereum: undefined, solana: undefined };
            } else {
                // No chrome.storage (test environment or non-extension context):
                // start from empty — defaults get seeded below.
                this.networks = { ethereum: [], solana: [] };
                this.currentBlockchain = 'ethereum';
                this.currentNetworks = { ethereum: undefined, solana: undefined };
            }
        } catch (error) {
            console.error('Failed to load networks:', error);
            this.networks = { ethereum: [], solana: [] };
            this.currentBlockchain = 'ethereum';
            this.currentNetworks = { ethereum: undefined, solana: undefined };
        }

        // Seed defaults regardless of storage path — tests expect the
        // "missing storage API" branch to still have Ethereum defaults.
        if (!this.networks.ethereum || this.networks.ethereum.length === 0) {
            const defaultEthNetworks = this.getDefaultNetworks('ethereum');
            for (const network of defaultEthNetworks) {
                const exists = this.networks.ethereum.some(n => n.id === network.id);
                if (!exists) {
                    try {
                        await this.addNetwork('ethereum', network);
                    } catch {
                        // Ignore persistence failures during default seeding —
                        // in-memory state is what callers observe via getNetworks().
                        this.networks.ethereum.push(network);
                    }
                }
            }
        }

        if (!this.currentNetworks.ethereum) {
            const mainnetNetwork = this.networks.ethereum.find(n => n.id === mainnet.id);
            if (mainnetNetwork) {
                try {
                    await this.setCurrentNetwork('ethereum', mainnetNetwork.id);
                } catch {
                    this.currentNetworks.ethereum = mainnetNetwork;
                }
            }
        }
    }

    private getDefaultNetworks(blockchain: 'ethereum' | 'solana'): Chain[] {
        if (blockchain === 'ethereum') {
            return [
                {
                    id: mainnet.id,
                    name: mainnet.name,
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
        // Propagate storage failures — callers (addCustomNetwork,
        // removeCustomNetwork) need to know if persistence failed.
        // Silently swallowing would leave in-memory state diverged from disk.
        if (typeof chrome !== 'undefined' && chrome.storage) {
            await chrome.storage.local.set({ [this.STORAGE_KEY]: this.networks });
        }
    }

    private async saveCurrentNetworks(): Promise<void> {
        if (typeof chrome !== 'undefined' && chrome.storage) {
            await chrome.storage.local.set({ [this.CURRENT_NETWORK_KEY]: this.currentNetworks });
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

    /**
     * Alias for `addNetwork`. Some callers prefer the "Custom" name
     * to emphasize that they're adding a user-defined network on
     * top of the built-in list; semantically identical.
     */
    public async addCustomNetwork(blockchain: 'ethereum' | 'solana', network: Chain): Promise<void> {
        return this.addNetwork(blockchain, network);
    }

    /**
     * Alias for `removeNetwork`. Same semantics — built-in networks
     * protected via PROTECTED_NETWORK_IDS are rejected, custom ones
     * (which is in practice what you'd be calling this on) are
     * removable.
     */
    public async removeCustomNetwork(blockchain: 'ethereum' | 'solana', chainId: number): Promise<void> {
        return this.removeNetwork(blockchain, chainId);
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

    /**
     * Flat list of every registered network across blockchains.
     * Useful for UIs that show a single unified list (network
     * picker / search) rather than the map form from getNetworks().
     */
    public getAllNetworks(): Chain[] {
        return [...this.networks.ethereum, ...this.networks.solana];
    }

    /**
     * Find a network by chainId across both blockchains (ethereum
     * first, then solana). Handy when callers have only a chainId
     * and don't know which blockchain it belongs to — e.g. dApps
     * that use chainId for routing without caring about the curve.
     * Returns undefined when the chainId isn't registered anywhere.
     */
    public getNetworkByChainId(chainId: number): Chain | undefined {
        return (
            this.networks.ethereum.find(n => n.id === chainId) ||
            this.networks.solana.find(n => n.id === chainId)
        );
    }

    /**
     * Collect all configured HTTP RPC URLs for a given network —
     * flattens default + public lists into a single array. Handy
     * for UI display / fallback rotation. Returns [] when the
     * chain has no rpcUrls populated (sketch networks).
     */
    public getNetworkRPCUrls(network: Chain): string[] {
        if (!network.rpcUrls) return [];
        const defaultHttp = network.rpcUrls.default?.http ?? [];
        const publicHttp = network.rpcUrls.public?.http ?? [];
        return [...defaultHttp, ...publicHttp];
    }

    /**
     * Predicate counterpart to getNetworkByChainId. Cheaper than
     * the full find when the caller only needs a bool (e.g. for
     * validating user input before a setCurrentNetwork call).
     */
    public networkExists(chainId: number): boolean {
        return (
            this.networks.ethereum.some(n => n.id === chainId) ||
            this.networks.solana.some(n => n.id === chainId)
        );
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

    /**
     * Overloads so callers can pass either a chainId (legacy — the
     * original signature was `chainId: number`) or a full Chain
     * object (more ergonomic when you just pulled one from
     * getNetworks().find(...)). Both paths look up via id and
     * validate the network is registered for this blockchain.
     */
    public async setCurrentNetwork(blockchain: 'ethereum' | 'solana', chainId: number): Promise<void>;
    public async setCurrentNetwork(blockchain: 'ethereum' | 'solana', network: Chain): Promise<void>;
    public async setCurrentNetwork(
        blockchain: 'ethereum' | 'solana',
        chainIdOrNetwork: number | Chain,
    ): Promise<void> {
        const chainId =
            typeof chainIdOrNetwork === 'number'
                ? chainIdOrNetwork
                : chainIdOrNetwork.id;
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

    /**
     * Symmetric with onNetworkChange. Delegates to
     * removeNetworkChangeListener — same naming pattern as the
     * offAccountChange alias on AccountService (af416a3).
     */
    public offNetworkChange(callback: NetworkChangeCallback): void {
        this.removeNetworkChangeListener(callback);
    }

    public removeNetworkChangeListener(callback: NetworkChangeCallback): void {
        this.changeCallbacks = this.changeCallbacks.filter(cb => cb !== callback);
    }

    private notifyNetworkChange(network: Chain | undefined): void {
        this.changeCallbacks.forEach(callback => callback(network));
    }

    /**
     * viem PublicClient cache keyed by chain id. Tests rely on
     * repeated calls with the same network returning the same
     * client instance; without caching, a fresh HTTP transport
     * would spin up per call and invalidate connection reuse.
     */
    private publicClientCache: Map<number, ReturnType<typeof createPublicClient>> = new Map();

    /**
     * Probe network connectivity by issuing a cheap RPC call
     * (getBlockNumber). Returns true if the call resolves, false
     * on any exception. Used by the Settings UI to show live
     * connection indicators per configured chain.
     */
    public async testNetworkConnectivity(network: Chain): Promise<boolean> {
        try {
            const client = this.getPublicClient(network);
            await (client as any).getBlockNumber();
            return true;
        } catch {
            return false;
        }
    }

    /**
     * Shallow Chain-shape validator. Rejects obvious garbage:
     * non-numeric id, empty name, zero RPC URLs. Callers that
     * want stricter validation should combine this with chainlist
     * lookup or a getBlockNumber probe.
     */
    public validateNetworkConfig(network: unknown): network is Chain {
        if (!network || typeof network !== 'object') return false;
        const n = network as Partial<Chain>;
        if (typeof n.id !== 'number') return false;
        if (typeof n.name !== 'string' || n.name.length === 0) return false;
        if (!n.rpcUrls || !n.rpcUrls.default) return false;
        const http = n.rpcUrls.default.http;
        if (!Array.isArray(http) || http.length === 0) return false;
        return true;
    }

    /**
     * Helper method to get a viem PublicClient for either the
     * current network (when called with no args) or a specific
     * network (when passed a Chain). Cached per-chain-id so
     * repeat calls return the same instance.
     */
    public getPublicClient(network?: Chain): ReturnType<typeof createPublicClient> {
        const target = network ?? this.currentNetworks.ethereum;
        if (!target) {
            throw new Error('No current Ethereum network selected');
        }
        // Current-network path still gates on blockchain check; explicit
        // network overrides the gate (caller knows what they want).
        if (!network && this.currentBlockchain !== 'ethereum') {
            throw new Error('Public client is only available for Ethereum networks');
        }

        const cached = this.publicClientCache.get(target.id);
        if (cached) return cached;

        // Convert our Chain to viem's Chain format
        const viemChain: any = {
            id: target.id,
            name: target.name,
            network: target.network,
            nativeCurrency: target.nativeCurrency || {
                name: 'Ether',
                symbol: 'ETH',
                decimals: 18
            },
            rpcUrls: target.rpcUrls || {
                default: { http: [] },
                public: { http: [] }
            },
            blockExplorers: target.blockExplorers
        };

        // viem PublicClient return type encodes the full chain
        // shape via generic inference; our cache declares the
        // no-arg createPublicClient return which diverges on a
        // handful of method-signature details. Both types name
        // themselves the same way ("Client"), causing TS to show
        // "Two different types with this name exist" — cast to
        // any to suppress and accept the imperfect alignment.
        const client = createPublicClient({
            chain: viemChain,
            transport: http()
        }) as any;
        this.publicClientCache.set(target.id, client);
        return client;
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