import { Account } from "@starlab/types/account";
import { KeystoreManager } from './keystoreManager';
import type { NewAccountSession } from "@starlab/types/keystore";

type AccountChangeCallback = (account: Account | null) => void;

class AccountService {
    private static instance: AccountService;
    private accounts: Account[] = [];
    private readonly STORAGE_KEY = 'wallet_accounts';
    private readonly CURRENT_ACCOUNT_KEY = 'wallet_current_account';
    private currentAccountId: string | undefined;
    private changeCallbacks: AccountChangeCallback[] = [];
    private initialized: boolean = false;

    private constructor() {
        // Don't call async methods in constructor
        this.initializeAsync();
    }

    public static getInstance(): AccountService {
        if (!AccountService.instance) {
            AccountService.instance = new AccountService();
        }
        return AccountService.instance;
    }

    private async initializeAsync(): Promise<void> {
        if (!this.initialized) {
            await this.loadAccounts();
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
        AccountService.instance = undefined as any;
    }

    private async loadAccounts(): Promise<void> {
        try {
            // In test environment, chrome.storage might not be available
            if (typeof chrome !== 'undefined' && chrome.storage) {
                const result = await chrome.storage.local.get([this.STORAGE_KEY, this.CURRENT_ACCOUNT_KEY]);
                // Defensive: corrupted storage (non-array) must reset to [].
                const raw = result[this.STORAGE_KEY];
                this.accounts = Array.isArray(raw) ? raw : [];
                this.currentAccountId = result[this.CURRENT_ACCOUNT_KEY];
            } else {
                // Fallback for test environment
                this.accounts = [];
                this.currentAccountId = undefined;
            }
        } catch (error) {
            console.error('Failed to load accounts:', error);
            this.accounts = [];
        }
    }

    private async saveAccounts(): Promise<void> {
        // Propagate storage failures — callers (addAccount, removeAccount)
        // need to know if persistence failed, otherwise we corrupt the
        // in-memory cache without anything on disk to back it up.
        if (typeof chrome !== 'undefined' && chrome.storage) {
            await chrome.storage.local.set({ [this.STORAGE_KEY]: this.accounts });
        }
    }

    private async saveCurrentAccount(): Promise<void> {
        if (typeof chrome !== 'undefined' && chrome.storage) {
            await chrome.storage.local.set({ [this.CURRENT_ACCOUNT_KEY]: this.currentAccountId });
        }
    }

    public async addAccount(account: Account): Promise<Account> {
        // Validate account structure
        if (!account || typeof account !== 'object') {
            throw new Error('Invalid account: must be an object');
        }
        
        if (!account.id || typeof account.id !== 'string') {
            throw new Error('Invalid account: id is required and must be a string');
        }
        
        if (!account.name || typeof account.name !== 'string') {
            throw new Error('Invalid account: name is required and must be a string');
        }
        
        if (!account.address || typeof account.address !== 'string') {
            throw new Error('Invalid account: address is required and must be a string');
        }
        
        if (!account.blockchain || typeof account.blockchain !== 'string') {
            throw new Error('Invalid account: blockchain is required and must be a string');
        }

        // Check for duplicate ID
        if (this.accounts.some(acc => acc.id === account.id)) {
            throw new Error('Account with this ID already exists');
        }

        // Check for duplicate address on same blockchain
        if (this.accounts.some(acc => acc.address === account.address && acc.blockchain === account.blockchain)) {
            throw new Error('Account with this address already exists for this blockchain');
        }

        this.accounts.push(account);
        await this.saveAccounts();
        return account;
    }

    public async updateAccount(account: Account): Promise<Account> {
        // Validate account
        if (!account.id || typeof account.id !== 'string') {
            throw new Error('Invalid account: id is required and must be a string');
        }

        const existingIndex = this.accounts.findIndex(acc => acc.id === account.id);
        if (existingIndex === -1) {
            throw new Error('Account not found');
        }

        // Update the account
        this.accounts[existingIndex] = {
            ...this.accounts[existingIndex],
            ...account,
            // Preserve original creation time
            created: this.accounts[existingIndex].created
        };
        
        await this.saveAccounts();
        return this.accounts[existingIndex];
    }

    public async addOrUpdateAccount(account: Account): Promise<Account> {
        const existingAccount = this.accounts.find(acc => acc.id === account.id);
        if (existingAccount) {
            console.log(`[AccountService] Account ${account.id} already exists, updating instead`);
            return await this.updateAccount(account);
        } else {
            console.log(`[AccountService] Creating new account ${account.id}`);
            return await this.addAccount(account);
        }
    }

    public async removeAccount(accountId: string): Promise<void> {
        const accountIndex = this.accounts.findIndex(acc => acc.id === accountId);
        if (accountIndex === -1) {
            throw new Error('Account not found');
        }

        const removedAccount = this.accounts[accountIndex];
        this.accounts.splice(accountIndex, 1);

        // If this was the current account, clear current account
        if (this.currentAccountId === removedAccount.id) {
            this.currentAccountId = undefined;
        }

        await this.saveAccounts();
        await this.saveCurrentAccount();
        this.notifyAccountChange(this.getCurrentAccount());
    }

    public getAccounts(): Account[] {
        return this.accounts;
    }

    public getAccount(address: string): Account | undefined {
        return this.accounts.find(acc => acc.address === address);
    }

    /**
     * Address-keyed lookup. Complementary to `getAccountById`; matches
     * the pair naming convention the test suite expects. Returns null
     * (not undefined) for consistency with `getAccountById`.
     */
    public getAccountByAddress(address: string): Account | null {
        return this.accounts.find(acc => acc.address === address) || null;
    }

    public getAccountById(id: string): Account | null {
        return this.accounts.find(acc => acc.id === id) || null;
    }

    public getAccountsByBlockchain(blockchain: 'ethereum' | 'solana'): Account[] {
        return this.accounts.filter(acc => acc.blockchain === blockchain);
    }

    public getCurrentAccount(): Account | null {
        // Pure read: returns null when no account has been explicitly
        // selected. Callers that want "pick one if unset" must use
        // ensureDefaultAccount(), which is the opt-in helper.
        if (!this.currentAccountId) return null;
        return this.accounts.find(acc => acc.id === this.currentAccountId) || null;
    }

    public async ensureDefaultAccount(): Promise<Account | null> {
        // If a current account is already explicitly set, use it.
        const existingCurrent = this.getCurrentAccount();
        if (existingCurrent) return existingCurrent;

        // Otherwise, if we have at least one account, promote the first
        // to current (replaces the implicit auto-select that used to
        // live in getCurrentAccount()).
        if (this.accounts.length > 0) {
            await this.setCurrentAccount(this.accounts[0].id);
            return this.accounts[0];
        }

        try {
            // Try to get address from WebRTC DKG if possible
            const address = await this.getAddressFromDKG();
            if (address) {
                const account: Account = {
                    id: `mpc-${Date.now()}`,
                    address,
                    name: 'Account 1',
                    balance: '0',
                    blockchain: 'ethereum',
                    created: Date.now(),
                    metadata: {
                        derivationPath: "m/44'/60'/0'/0/0", // Standard Ethereum derivation path
                        source: 'dkg'
                    }
                };
                await this.addAccount(account);
                await this.setCurrentAccount(account.id);
                return account;
            }
        } catch (error) {
            console.error('Failed to create default account:', error);
        }

        return null;
    }

    /**
     * Generate a new account by initiating a new DKG session
     * Each account requires its own DKG session since FROST doesn't support HD derivation
     */
    public async generateNewAccount(name?: string, blockchain: 'ethereum' | 'solana' = 'ethereum'): Promise<NewAccountSession> {
        try {
            const existingAccountsCount = this.getAccountsByBlockchain(blockchain).length;
            
            // Create a new account session that will trigger DKG
            const sessionId = `account_${blockchain}_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
            
            const newSession: NewAccountSession = {
                session_id: sessionId,
                name: name || `Account ${existingAccountsCount + 1}`,
                blockchain,
                threshold: 2, // Default threshold
                total_participants: 3, // Default participants
                participants: [], // Will be filled when session is proposed
                status: 'proposing',
                created_at: Date.now(),
            };
            
            // Store the pending session
            await this.storePendingSession(newSession);
            
            // The actual DKG will be initiated through the normal session proposal flow
            // This just creates the intent to create a new account
            console.log('[AccountService] Created new account session:', sessionId);
            
            return newSession;
        } catch (error) {
            console.error('Failed to create new account session:', error);
            throw error;
        }
    }
    
    /**
     * Complete account creation after DKG finishes
     */
    public async completeAccountCreation(
        sessionId: string, 
        address: string,
        keyShareData: any
    ): Promise<Account | null> {
        console.log(`[AccountService] Completing account creation for session ${sessionId} with address ${address}`);
        
        try {
            const pendingSession = await this.getPendingSession(sessionId);
            
            // If no pending session, create account directly from DKG data
            let account: Account;
            
            if (pendingSession) {
                account = {
                    id: `mpc-${sessionId}`,
                    address,
                    name: pendingSession.name,
                    balance: '0',
                    blockchain: pendingSession.blockchain,
                    created: Date.now(),
                    metadata: {
                        sessionId,
                        source: 'dkg',
                        threshold: pendingSession.threshold,
                        totalParticipants: pendingSession.total_participants,
                    }
                };
            } else {
                // Create account directly from DKG session
                console.log('[AccountService] No pending session found, creating account from DKG data');
                const existingAccountsCount = this.getAccountsByBlockchain(keyShareData.curve === 'secp256k1' ? 'ethereum' : 'solana').length;
                
                account = {
                    id: `mpc-${sessionId}`,
                    address,
                    name: `Account ${existingAccountsCount + 1}`,
                    balance: '0',
                    blockchain: keyShareData.curve === 'secp256k1' ? 'ethereum' : 'solana',
                    created: Date.now(),
                    metadata: {
                        sessionId,
                        source: 'dkg',
                        threshold: keyShareData.threshold || 2,
                        totalParticipants: keyShareData.total_participants || 3
                    }
                };
            }
            
            // Save key share to keystore
            const keystoreManager = KeystoreManager.getInstance();
            if (!keystoreManager.isLocked()) {
                await keystoreManager.addWallet(account.id, keyShareData, {
                    id: account.id,
                    name: account.name,
                    blockchain: account.blockchain,
                    address: account.address,
                    session_id: sessionId,
                    isActive: true,
                    hasBackup: false,
                    createdAt: Date.now()
                });
            }
            
            // Add or update account
            const savedAccount = await this.addOrUpdateAccount(account);
            
            // Remove pending session if it exists
            if (pendingSession) {
                await this.removePendingSession(sessionId);
            }
            
            return savedAccount;
        } catch (error) {
            console.error('Failed to complete account creation:', error);
            return null;
        }
    }
    
    private async storePendingSession(session: NewAccountSession): Promise<void> {
        try {
            if (typeof chrome !== 'undefined' && chrome.storage) {
                const key = `pending_session_${session.session_id}`;
                await chrome.storage.local.set({ [key]: session });
            }
        } catch (error) {
            console.error('Failed to store pending session:', error);
        }
    }
    
    private async getPendingSession(sessionId: string): Promise<NewAccountSession | null> {
        try {
            if (typeof chrome !== 'undefined' && chrome.storage) {
                const key = `pending_session_${sessionId}`;
                const result = await chrome.storage.local.get(key);
                return result[key] || null;
            }
            return null;
        } catch (error) {
            console.error('Failed to get pending session:', error);
            return null;
        }
    }
    
    private async removePendingSession(sessionId: string): Promise<void> {
        try {
            if (typeof chrome !== 'undefined' && chrome.storage) {
                const key = `pending_session_${sessionId}`;
                await chrome.storage.local.remove(key);
            }
        } catch (error) {
            console.error('Failed to remove pending session:', error);
        }
    }

    private async getSolanaAddressFromDKG(): Promise<string | null> {
        try {
            // Send message to offscreen document to get Solana address
            const response = await chrome.runtime.sendMessage({
                type: "getSolanaAddress"
            });

            if (response && response.success && response.data && response.data.solanaAddress) {
                return response.data.solanaAddress;
            }

            return null;
        } catch (error) {
            console.error('Error getting Solana address from DKG:', error);
            return null;
        }
    }

    private async getAddressFromDKG(): Promise<string | null> {
        try {
            // Send message to offscreen document to get Ethereum address
            const response = await chrome.runtime.sendMessage({
                type: "getEthereumAddress"
            });

            if (response && response.success && response.data && response.data.ethereumAddress) {
                return response.data.ethereumAddress;
            }

            return null;
        } catch (error) {
            console.error('Error getting address from DKG:', error);
            return null;
        }
    }

    /**
     * Set the active account by id, or pass `null` to clear the
     * active selection (useful for sign-out / account-reset flows
     * without wiping the full account list).
     */
    public async setCurrentAccount(accountId: string | null): Promise<void> {
        if (accountId === null) {
            this.currentAccountId = undefined;
            await this.saveCurrentAccount();
            this.notifyAccountChange(null);
            return;
        }
        const account = this.accounts.find(acc => acc.id === accountId);
        if (!account) {
            throw new Error('Account not found');
        }
        this.currentAccountId = account.id;
        await this.saveCurrentAccount();
        this.notifyAccountChange(account);
    }

    public async clearAccounts(): Promise<void> {
        this.accounts = [];
        this.currentAccountId = undefined;
        await this.saveAccounts();
        await this.saveCurrentAccount();
        this.notifyAccountChange(null);
    }

    public onAccountChange(callback: AccountChangeCallback): void {
        this.changeCallbacks.push(callback);
    }

    public removeAccountChangeListener(callback: AccountChangeCallback): void {
        this.changeCallbacks = this.changeCallbacks.filter(cb => cb !== callback);
    }

    /**
     * Symmetric with `onAccountChange` — the on/off naming pair
     * is what most listener APIs expose (EventTarget, Node EE).
     * Delegates to removeAccountChangeListener for the same effect.
     */
    public offAccountChange(callback: AccountChangeCallback): void {
        this.removeAccountChangeListener(callback);
    }

    private notifyAccountChange(account: Account | null): void {
        this.changeCallbacks.forEach(callback => callback(account));
    }
}

export default AccountService; 