// ===================================================================
// ACCOUNT AND WALLET TYPES
// ===================================================================
//
// This file contains types related to user accounts and wallet management
// in the MPC wallet system. These represent the accounts that users
// interact with in the UI.
//
// Key Concepts for Junior Developers:
// - Account: A wallet address that can hold cryptocurrency
// - Balance: How much cryptocurrency an account currently holds
// - Public Key: The cryptographic key that others can see (for receiving funds)
// - Blockchain: Which cryptocurrency network (Ethereum, Solana, etc.)
// - Multi-Account: Users can have multiple accounts on different blockchains
// ===================================================================

/**
 * Represents a single cryptocurrency account/wallet.
 * Each account is tied to a specific blockchain and has its own address.
 */
export interface Account {
    /** Unique identifier for this account within the app */
    id: string;

    /** The blockchain address (what others use to send you funds) */
    address: string;

    /** User-friendly name for this account (set by user) */
    name: string;

    /** Current balance as a string (to avoid floating point precision issues) */
    balance: string;

    /** The public key associated with this account (optional) */
    publicKey?: string;

    /** Which blockchain network this account belongs to */
    blockchain: 'ethereum' | 'solana';

    /** Timestamp when this account was created (Unix timestamp) */
    created?: number;

    /** Timestamp when this account was last used (Unix timestamp) */
    lastUsed?: number;

    /** Whether this account is currently active/selected */
    isActive?: boolean;

    /**
     * Account type discriminator. 'frost' for MPC/threshold accounts
     * created via DKG ceremony — the primary kind in this extension.
     * Left optional and string-unioned open for future account kinds
     * (e.g. 'watch-only', 'hardware').
     */
    type?: 'frost' | 'watch-only' | 'hardware' | string;

    /** Optional metadata for the account */
    metadata?: {
        /** Derivation path used to generate this account */
        derivationPath?: string;
        /** How this account was created:
         *    - 'generated': single-party generation (legacy; kept for tests)
         *    - 'imported': user-supplied CLI keystore import
         *    - 'dkg': multi-party threshold DKG ceremony (the primary path) */
        source: 'generated' | 'imported' | 'dkg';
        /** Custom tags or labels */
        tags?: string[];
        /** For 'dkg' source: the DKG session id that produced this account. */
        sessionId?: string;
        /** For 'dkg' source: signing threshold (t in t-of-n). */
        threshold?: number;
        /** For 'dkg' source: total participants (n in t-of-n). */
        totalParticipants?: number;
    };
}

/**
 * Storage structure for managing multiple accounts.
 * This represents how account data is persisted.
 */
export interface AccountStorage {
    /** List of all user accounts */
    accounts: Account[];

    /** ID of the currently selected/active account */
    currentAccount: string | null;

    /** Settings related to account management */
    settings?: {
        /** Whether to auto-switch to newly created accounts */
        autoSwitchToNew: boolean;
        /** Default blockchain for new accounts */
        defaultBlockchain: 'ethereum' | 'solana';
        /** Whether to show test networks */
        showTestNetworks: boolean;
    };
}

/**
 * Information about account balances across different tokens/assets.
 */
export interface AccountBalance {
    /** Native token balance (ETH for Ethereum, SOL for Solana) */
    native: {
        amount: string;
        symbol: string;
        decimals: number;
    };

    /** Balances of other tokens/assets */
    tokens?: Array<{
        /** Token contract address or mint address */
        address: string;
        /** Token symbol (e.g., "USDC", "DAI") */
        symbol: string;
        /** Token name */
        name: string;
        /** Balance amount as string */
        amount: string;
        /** Number of decimal places */
        decimals: number;
        /** USD value if available */
        usdValue?: string;
    }>;

    /** Total USD value of all assets (if available) */
    totalUsdValue?: string;

    /** When this balance information was last updated */
    lastUpdated: number;
}

/**
 * Events related to account management.
 */
export type AccountEvent =
    | { type: 'AccountCreated'; account: Account }
    | { type: 'AccountUpdated'; accountId: string; changes: Partial<Account> }
    | { type: 'AccountDeleted'; accountId: string }
    | { type: 'AccountSwitched'; fromId: string | null; toId: string }
    | { type: 'BalanceUpdated'; accountId: string; balance: AccountBalance }
    | { type: 'AccountRenamed'; accountId: string; oldName: string; newName: string };

/**
 * Utility type for account validation and operations.
 */
export interface AccountValidation {
    /** Whether the account address is valid for its blockchain */
    isValidAddress: boolean;

    /** Whether the account has sufficient balance for operations */
    hasSufficientBalance: boolean;

    /** Whether the account is ready for transactions */
    isReady: boolean;

    /** Any validation errors */
    errors: string[];
}
