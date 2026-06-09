<script lang="ts">
    import { onMount } from 'svelte';
    import type { Account } from "@starlab/types/account";
    import AccountService from '../services/accountService';
    
    export let blockchain: 'ethereum' | 'solana' = 'ethereum';
    
    let currentAccount: Account | null = null;
    let accounts: Account[] = [];
    let showAccountSelector = false;
    let showCreateAccount = false;
    let newAccountName = '';
    let accountService: AccountService;
    
    onMount(async () => {
        accountService = AccountService.getInstance();
        await loadAccounts();
        
        // Get current account from AccountService
        currentAccount = accountService.getCurrentAccount();
        
        // Listen for account changes
        accountService.onAccountChange((account) => {
            currentAccount = account;
            loadAccounts();
        });
    });
    
    async function loadAccounts() {
        await accountService.ensureInitialized();
        accounts = accountService.getAccountsByBlockchain(blockchain);
        console.log('[AccountManager] Loaded accounts:', accounts);
    }
    
    function toggleAccountSelector() {
        showAccountSelector = !showAccountSelector;
        showCreateAccount = false;
    }
    
    async function selectAccount(accountId: string) {
        try {
            await accountService.setCurrentAccount(accountId);
            showAccountSelector = false;
        } catch (error) {
            console.error('[AccountManager] Error selecting account:', error);
        }
    }
    
    async function createNewAccount() {
        if (!newAccountName.trim()) {
            alert('Please enter an account name');
            return;
        }
        
        try {
            // Create a new account session - this will trigger a new DKG
            const newSession = await accountService.generateNewAccount(newAccountName, blockchain);
            console.log('[AccountManager] Created new account session:', newSession);
            
            // Show message to user
            alert(`New account "${newSession.name}" is being created. Please complete the DKG process with other participants.`);
            
            newAccountName = '';
            showCreateAccount = false;
            
            // TODO: Trigger DKG session proposal UI
            // For now, the user needs to manually propose a session with the new session ID
            
        } catch (error) {
            console.error('[AccountManager] Error creating account:', error);
            alert('Error creating account: ' + (error instanceof Error ? error.message : String(error)));
        }
    }
    
    function formatAddress(address: string): string {
        if (address.length <= 10) return address;
        return `${address.slice(0, 6)}...${address.slice(-4)}`;
    }
    
    function copyAddress(address: string) {
        navigator.clipboard.writeText(address);
        // Could add a toast notification here
    }
</script>

<div class="account-manager">
    <div
        class="current-account"
        role="button"
        tabindex="0"
        on:click={toggleAccountSelector}
        on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && toggleAccountSelector()}
    >
        {#if currentAccount}
            <div class="account-info">
                <div class="account-name">{currentAccount.name}</div>
                <div class="account-address">{formatAddress(currentAccount.address)}</div>
            </div>
            <div class="account-balance">
                {currentAccount.balance || '0'} {blockchain === 'ethereum' ? 'ETH' : 'SOL'}
            </div>
        {:else}
            <div class="no-account">No account selected</div>
        {/if}
        <svg class="chevron {showAccountSelector ? 'rotate' : ''}" width="12" height="12" viewBox="0 0 12 12">
            <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" fill="none"/>
        </svg>
    </div>
    
    {#if showAccountSelector}
        <div class="account-dropdown">
            <div class="dropdown-header">
                <h3>My Accounts</h3>
                <button
                    class="icon-button"
                    aria-label="Create new account"
                    on:click={() => showCreateAccount = !showCreateAccount}
                >
                    <svg width="16" height="16" viewBox="0 0 16 16">
                        <path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.5" fill="none"/>
                    </svg>
                </button>
            </div>
            
            {#if showCreateAccount}
                <div class="create-account-form">
                    <input
                        type="text"
                        placeholder="Account name"
                        bind:value={newAccountName}
                        on:keydown={(e) => e.key === 'Enter' && createNewAccount()}
                    />
                    <button class="create-button" on:click={createNewAccount}>
                        Create
                    </button>
                </div>
            {/if}
            
            <div class="accounts-list">
                {#each accounts as account}
                    <div
                        class="account-item {account.id === currentAccount?.id ? 'selected' : ''}"
                        role="button"
                        tabindex="0"
                        on:click={() => selectAccount(account.id)}
                        on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && selectAccount(account.id)}
                    >
                        <div class="account-item-info">
                            <div class="account-item-name">{account.name}</div>
                            <div class="account-item-address">
                                {formatAddress(account.address)}
                                <button
                                    class="copy-button"
                                    aria-label="Copy address"
                                    on:click|stopPropagation={() => copyAddress(account.address)}
                                >
                                    <svg width="12" height="12" viewBox="0 0 12 12">
                                        <path d="M3 2h5v5M3 3v6h6V4" stroke="currentColor" fill="none"/>
                                    </svg>
                                </button>
                            </div>
                        </div>
                        <div class="account-item-balance">
                            {account.balance || '0'} {blockchain === 'ethereum' ? 'ETH' : 'SOL'}
                        </div>
                    </div>
                {/each}
                
                {#if accounts.length === 0}
                    <div class="no-accounts">
                        No accounts yet. Create one above.
                    </div>
                {/if}
            </div>
        </div>
    {/if}
</div>

<style>
    .account-manager {
        position: relative;
        width: 100%;
    }

    .current-account {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 12px 16px;
        background: var(--c-surface);
        border: 1px solid var(--c-line);
        border-radius: 14px;
        cursor: pointer;
        transition: background-color 0.2s;
    }

    .current-account:hover {
        background: var(--c-surface-2);
    }

    .account-info {
        flex: 1;
        min-width: 0;
    }

    .account-name {
        font-weight: 600;
        font-size: 14px;
        color: var(--c-text);
    }

    .account-address {
        font-size: 12px;
        color: var(--c-muted);
        font-family: var(--font-mono, monospace);
    }

    .account-balance {
        font-weight: 600;
        font-size: 14px;
        color: var(--c-text);
        margin-right: 8px;
    }

    .no-account {
        color: var(--c-muted);
        font-style: italic;
    }

    .chevron {
        transition: transform 0.2s;
        color: var(--c-muted);
    }

    .chevron.rotate {
        transform: rotate(180deg);
    }

    .account-dropdown {
        position: absolute;
        top: calc(100% + 6px);
        left: 0;
        right: 0;
        background: var(--c-surface);
        border: 1px solid var(--c-line);
        border-radius: 14px;
        box-shadow: var(--shadow-pop);
        z-index: 100;
        max-height: 400px;
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }

    .dropdown-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 12px 16px;
        border-bottom: 1px solid var(--c-line);
    }

    .dropdown-header h3 {
        margin: 0;
        font-size: 14px;
        font-weight: 700;
        color: var(--c-text);
    }

    .icon-button {
        background: none;
        border: none;
        cursor: pointer;
        padding: 4px;
        border-radius: 8px;
        color: var(--c-muted);
        transition: background-color 0.2s;
    }

    .icon-button:hover {
        background: var(--c-surface-2);
        color: var(--c-text);
    }

    .create-account-form {
        display: flex;
        gap: 8px;
        padding: 12px 16px;
        border-bottom: 1px solid var(--c-line);
    }

    .create-account-form input {
        flex: 1;
        padding: 8px 12px;
        border: 1px solid var(--c-line-strong);
        border-radius: 8px;
        font-size: 14px;
        background: var(--c-surface);
        color: var(--c-text);
    }

    .create-button {
        padding: 8px 16px;
        background: var(--c-primary);
        color: var(--c-primary-fg);
        border: none;
        border-radius: 8px;
        font-size: 14px;
        font-weight: 600;
        cursor: pointer;
        transition: filter 0.2s;
    }

    .create-button:hover {
        filter: brightness(1.06);
    }

    .accounts-list {
        overflow-y: auto;
        max-height: 300px;
    }

    .account-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 12px 16px;
        cursor: pointer;
        transition: background-color 0.2s;
    }

    .account-item:hover {
        background: var(--c-surface-2);
    }

    .account-item.selected {
        background: var(--c-primary-soft);
    }

    .account-item-info {
        flex: 1;
        min-width: 0;
    }

    .account-item-name {
        font-weight: 600;
        font-size: 14px;
        color: var(--c-text);
    }

    .account-item-address {
        display: flex;
        align-items: center;
        gap: 4px;
        font-size: 12px;
        color: var(--c-muted);
        font-family: var(--font-mono, monospace);
    }

    .copy-button {
        background: none;
        border: none;
        cursor: pointer;
        padding: 2px;
        color: var(--c-muted);
        opacity: 0.7;
        transition: opacity 0.2s;
    }

    .copy-button:hover {
        opacity: 1;
        color: var(--c-text);
    }

    .account-item-balance {
        font-size: 14px;
        color: var(--c-text);
    }

    .no-accounts {
        padding: 24px;
        text-align: center;
        color: var(--c-muted);
        font-size: 14px;
    }
</style>
