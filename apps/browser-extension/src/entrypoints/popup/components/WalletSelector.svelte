<script lang="ts">
    import { createEventDispatcher } from 'svelte';
    import type { ExtensionWalletMetadata } from "@starlab/types/keystore";
    
    export let wallets: ExtensionWalletMetadata[] = [];
    export let activeWallet: ExtensionWalletMetadata | null = null;
    export let showDropdown: boolean = false;
    
    const dispatch = createEventDispatcher();
    
    function toggleDropdown() {
        showDropdown = !showDropdown;
    }
    
    function selectWallet(wallet: ExtensionWalletMetadata) {
        dispatch('select', wallet);
        showDropdown = false;
    }
    
    function formatAddress(address: string): string {
        if (!address) return '';
        return `${address.slice(0, 6)}...${address.slice(-4)}`;
    }
    
    function getWalletIcon(blockchain: string): string {
        switch (blockchain) {
            case 'ethereum':
                return '⟠';
            case 'solana':
                return '◎';
            default:
                return '●';
        }
    }
    
    // Close dropdown when clicking outside
    function handleClickOutside(event: MouseEvent) {
        const target = event.target as HTMLElement;
        if (!target.closest('.wallet-selector')) {
            showDropdown = false;
        }
    }
</script>

<svelte:window on:click={handleClickOutside} />

{#if wallets.length > 0}
    <div class="wallet-selector">
        <button
            class="wallet-selector-button"
            on:click={toggleDropdown}
            aria-expanded={showDropdown}
            aria-haspopup="true"
        >
            <div class="wallet-info">
                {#if activeWallet}
                    <span class="wallet-icon">{getWalletIcon(activeWallet.blockchain)}</span>
                    <span class="wallet-name">{activeWallet.name}</span>
                    <span class="wallet-address">{formatAddress(activeWallet.address)}</span>
                {:else}
                    <span class="no-wallet">No wallet selected</span>
                {/if}
            </div>
            <svg
                class="dropdown-arrow"
                class:open={showDropdown}
                width="12"
                height="12"
                viewBox="0 0 12 12"
            >
                <path
                    d="M3 5L6 8L9 5"
                    stroke="currentColor"
                    stroke-width="2"
                    fill="none"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                />
            </svg>
        </button>
        
        {#if showDropdown}
            <div class="wallet-dropdown">
                <div class="dropdown-header">
                    <h3>Select Wallet</h3>
                    <button class="add-wallet-button" on:click={() => dispatch('add')}>
                        + Add Wallet
                    </button>
                </div>
                
                <div class="wallet-list">
                    {#each wallets as wallet (wallet.id)}
                        <button
                            class="wallet-item"
                            class:active={wallet.id === activeWallet?.id}
                            on:click={() => selectWallet(wallet)}
                        >
                            <span class="wallet-icon">{getWalletIcon(wallet.blockchain)}</span>
                            <div class="wallet-details">
                                <span class="wallet-name">{wallet.name}</span>
                                <span class="wallet-address">{formatAddress(wallet.address)}</span>
                            </div>
                            {#if wallet.id === activeWallet?.id}
                                <span class="active-indicator">✓</span>
                            {/if}
                        </button>
                    {/each}
                </div>
                
                <div class="dropdown-footer">
                    <button class="manage-button" on:click={() => dispatch('manage')}>
                        Manage Wallets
                    </button>
                </div>
            </div>
        {/if}
    </div>
{/if}

<style>
    .wallet-selector {
        position: relative;
        width: 100%;
    }
    
    .wallet-selector-button {
        width: 100%;
        padding: 12px 16px;
        background: var(--c-surface);
        border: 1px solid var(--c-line);
        border-radius: 14px;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: space-between;
        transition: all 0.2s;
        font-family: inherit;
    }

    .wallet-selector-button:hover {
        background: var(--c-surface-2);
        border-color: var(--c-line-strong);
    }
    
    .wallet-info {
        display: flex;
        align-items: center;
        gap: 8px;
        flex: 1;
        text-align: left;
    }
    
    .wallet-icon {
        font-size: 18px;
        display: flex;
        align-items: center;
        justify-content: center;
        width: 24px;
        height: 24px;
    }
    
    .wallet-name {
        font-weight: 600;
        color: var(--c-text);
        font-size: 14px;
    }

    .wallet-address {
        color: var(--c-muted);
        font-size: 13px;
        font-family: var(--font-mono, monospace);
    }

    .no-wallet {
        color: var(--c-muted);
        font-size: 14px;
    }

    .dropdown-arrow {
        transition: transform 0.2s;
        color: var(--c-muted);
    }

    .dropdown-arrow.open {
        transform: rotate(180deg);
    }

    .wallet-dropdown {
        position: absolute;
        top: calc(100% + 8px);
        left: 0;
        right: 0;
        background: var(--c-surface);
        border: 1px solid var(--c-line);
        border-radius: 14px;
        box-shadow: var(--shadow-pop);
        z-index: 100;
        overflow: hidden;
        animation: slideDown 0.2s ease-out;
    }
    
    @keyframes slideDown {
        from {
            opacity: 0;
            transform: translateY(-10px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }
    
    .dropdown-header {
        padding: 14px 16px;
        border-bottom: 1px solid var(--c-line);
        display: flex;
        align-items: center;
        justify-content: space-between;
    }

    .dropdown-header h3 {
        margin: 0;
        font-size: 14px;
        font-weight: 700;
        color: var(--c-text);
    }

    .add-wallet-button {
        padding: 6px 12px;
        background: var(--c-primary);
        color: var(--c-primary-fg);
        border: none;
        border-radius: 8px;
        font-size: 13px;
        font-weight: 600;
        cursor: pointer;
        transition: filter 0.2s;
    }

    .add-wallet-button:hover {
        filter: brightness(1.06);
    }
    
    .wallet-list {
        max-height: 300px;
        overflow-y: auto;
    }
    
    .wallet-item {
        width: 100%;
        padding: 12px 16px;
        background: none;
        border: none;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 12px;
        transition: background 0.2s;
        text-align: left;
    }
    
    .wallet-item:hover {
        background: var(--c-surface-2);
    }

    .wallet-item.active {
        background: var(--c-primary-soft);
    }
    
    .wallet-details {
        flex: 1;
        display: flex;
        flex-direction: column;
        gap: 2px;
    }
    
    .wallet-details .wallet-name {
        font-size: 14px;
    }
    
    .wallet-details .wallet-address {
        font-size: 12px;
    }
    
    .active-indicator {
        color: var(--c-success);
        font-size: 16px;
    }

    .dropdown-footer {
        padding: 12px 16px;
        border-top: 1px solid var(--c-line);
    }

    .manage-button {
        width: 100%;
        padding: 8px;
        background: none;
        border: 1px solid var(--c-line);
        border-radius: 8px;
        color: var(--c-text);
        font-size: 13px;
        cursor: pointer;
        transition: all 0.2s;
    }

    .manage-button:hover {
        background: var(--c-surface-2);
        border-color: var(--c-line-strong);
    }
</style>