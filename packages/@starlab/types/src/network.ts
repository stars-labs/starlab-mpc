// ===================================================================
// BLOCKCHAIN NETWORK TYPES
// ===================================================================
//
// This file contains types related to blockchain networks and their
// configuration. These types help manage connections to different
// blockchain networks (Ethereum, Solana, etc.) and their properties.
//
// Key Concepts for Junior Developers:
// - Blockchain Network: A specific instance of a blockchain (like Ethereum Mainnet)
// - RPC URL: A server endpoint for interacting with the blockchain
// - Chain ID: A unique number identifying a specific blockchain network
// - Native Currency: The main token of a blockchain (ETH for Ethereum, SOL for Solana)
// - Block Explorer: A website for viewing blockchain transactions and addresses
// ===================================================================

/**
 * Represents a blockchain network configuration.
 * This follows a similar structure to popular Web3 libraries.
 */
export interface Chain {
    /** Unique numeric identifier for this chain */
    id: number;

    /** Human-readable name of the chain */
    name: string;

    /** Network identifier (used internally) */
    network: string;

    /** Information about the native currency of this chain */
    nativeCurrency?: {
        /** Full name of the currency */
        name: string;
        /** Short symbol (e.g., "ETH", "SOL") */
        symbol: string;
        /** Number of decimal places */
        decimals: number;
    };

    /** RPC endpoints for connecting to this chain */
    rpcUrls?: {
        /** Default RPC endpoints */
        default: {
            http: string[];
            webSocket?: string[];
        };
        /** Public RPC endpoints (if different from default) */
        public?: {
            http: string[];
            webSocket?: string[];
        };
    };

    /** Block explorer configuration */
    blockExplorers?: {
        /** Default block explorer */
        default: {
            name: string;
            url: string;
        };
        /** Alternative block explorers */
        alternatives?: Array<{
            name: string;
            url: string;
        }>;
    };

    /** Additional chain properties */
    testnet?: boolean;
    /** Icon URL for the chain */
    iconUrl?: string;
    /** Whether this chain supports our MPC wallet features */
    supported?: boolean;
}

/**
 * Network connection status and health information.
 */
export interface NetworkStatus {
    /** Which chain we're currently connected to */
    currentChain: Chain | null;

    /** Whether we have an active connection */
    connected: boolean;

    /** Current block number (if available) */
    blockNumber?: number;

    /** Network latency in milliseconds */
    latency?: number;

    /** When we last successfully connected */
    lastConnected?: number;

    /** Any current connection errors */
    error?: string;

    /** RPC endpoint currently being used */
    activeRpcUrl?: string;
}

/**
 * Configuration for network connections.
 */
export interface NetworkConfig {
    /** List of supported chains */
    supportedChains: Chain[];

    /** Default chain to connect to */
    defaultChain: Chain;

    /** Connection preferences */
    preferences: {
        /** Whether to auto-switch networks when needed */
        autoSwitch: boolean;
        /** Timeout for RPC requests in milliseconds */
        rpcTimeout: number;
        /** Whether to use fallback RPC URLs */
        useFallbackRpcs: boolean;
        /** Maximum number of connection retries */
        maxRetries: number;
    };
}

/**
 * Events related to network management.
 */
export type NetworkEvent =
    | { type: 'Connected'; chain: Chain }
    | { type: 'Disconnected'; chain: Chain }
    | { type: 'ChainSwitched'; from: Chain | null; to: Chain }
    | { type: 'BlockUpdate'; blockNumber: number; chain: Chain }
    | { type: 'ConnectionError'; error: string; chain: Chain }
    | { type: 'LatencyUpdate'; latency: number; chain: Chain };

/**
 * Predefined popular blockchain networks.
 */
export const SUPPORTED_CHAINS: Record<string, Chain> = {
    ethereum: {
        id: 1,
        name: 'Ethereum',
        network: 'ethereum',
        nativeCurrency: {
            name: 'Ether',
            symbol: 'ETH',
            decimals: 18,
        },
        rpcUrls: {
            default: {
                http: ['https://cloudflare-eth.com'],
            },
            public: {
                http: ['https://cloudflare-eth.com'],
            },
        },
        blockExplorers: {
            default: {
                name: 'Etherscan',
                url: 'https://etherscan.io',
            },
        },
        supported: true,
    },

    solana: {
        id: 101, // Solana doesn't use traditional chain IDs, but we need a unique number
        name: 'Solana',
        network: 'solana',
        nativeCurrency: {
            name: 'Solana',
            symbol: 'SOL',
            decimals: 9,
        },
        rpcUrls: {
            default: {
                http: ['https://api.mainnet-beta.solana.com'],
            },
            public: {
                http: ['https://api.mainnet-beta.solana.com'],
            },
        },
        blockExplorers: {
            default: {
                name: 'Solscan',
                url: 'https://solscan.io',
            },
        },
        supported: true,
    },

    // Test networks
    sepolia: {
        id: 11155111,
        name: 'Sepolia',
        network: 'sepolia',
        nativeCurrency: {
            name: 'Sepolia Ether',
            symbol: 'ETH',
            decimals: 18,
        },
        rpcUrls: {
            default: {
                http: ['https://rpc.sepolia.org'],
            },
        },
        blockExplorers: {
            default: {
                name: 'Etherscan',
                url: 'https://sepolia.etherscan.io',
            },
        },
        testnet: true,
        supported: true,
    },
};