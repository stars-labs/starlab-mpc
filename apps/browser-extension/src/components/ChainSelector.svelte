<script lang="ts">
    import { createEventDispatcher } from 'svelte';
    import { MultiChainNetworkService } from '../services/multiChainNetworkService';
    import type { ChainInfo } from '../config/chains';
    
    const dispatch = createEventDispatcher();
    const networkService = MultiChainNetworkService.getInstance();
    
    // Props
    export let currentChain: ChainInfo | undefined = undefined;
    export let showTestnets: boolean = false;
    
    // State
    let showDropdown = false;
    let searchQuery = '';
    let selectedCategory: string = 'all';
    
    // Get grouped networks
    $: allNetworks = networkService.getAllNetworksGrouped();
    
    // Filter networks based on search and testnet toggle
    $: filteredNetworks = (() => {
        const result: Record<string, ChainInfo[]> = {};
        
        Object.entries(allNetworks).forEach(([category, chains]) => {
            const filtered = chains.filter(chain => {
                // Filter by testnet
                if (!showTestnets && chain.testnet) return false;
                
                // Filter by search query
                if (searchQuery) {
                    const query = searchQuery.toLowerCase();
                    return (
                        chain.name.toLowerCase().includes(query) ||
                        (chain.nativeCurrency?.symbol.toLowerCase().includes(query) ?? false) ||
                        chain.network.toLowerCase().includes(query)
                    );
                }
                
                return true;
            });
            
            if (filtered.length > 0) {
                result[category] = filtered;
            }
        });
        
        // Filter by category if selected
        if (selectedCategory !== 'all') {
            const categoryChains = result[selectedCategory];
            return categoryChains ? { [selectedCategory]: categoryChains } : {};
        }
        
        return result;
    })();
    
    // Category labels
    const categoryLabels: Record<string, string> = {
        bitcoin: '₿ Bitcoin',
        ethereum: 'Ξ Ethereum',
        evm: '🔷 EVM Compatible',
        solana: '◎ Solana',
        aptos: '🔺 Aptos',
        sui: '🌊 Sui'
    };
    
    // Get categories with chains
    $: availableCategories = Object.keys(allNetworks).filter(
        cat => allNetworks[cat as keyof typeof allNetworks].length > 0
    );
    
    function selectChain(chain: ChainInfo) {
        networkService.setCurrentNetwork(chain.id);
        dispatch('chainSelected', chain);
        showDropdown = false;
        searchQuery = '';
    }
    
    function toggleDropdown() {
        showDropdown = !showDropdown;
        if (!showDropdown) {
            searchQuery = '';
        }
    }
    
    function handleClickOutside(event: MouseEvent) {
        const target = event.target as HTMLElement;
        if (!target.closest('.chain-selector')) {
            showDropdown = false;
            searchQuery = '';
        }
    }
    
    // Format chain display
    function formatChainName(chain: ChainInfo): string {
        if (chain.testnet) {
            return `${chain.name} (Testnet)`;
        }
        return chain.name;
    }
    
    // Get chain icon based on category
    function getChainIcon(category: string): string {
        const icons: Record<string, string> = {
            bitcoin: '₿',
            ethereum: 'Ξ',
            evm: '🔷',
            solana: '◎',
            aptos: '🔺',
            sui: '🌊'
        };
        return icons[category] || '🔗';
    }
</script>

<svelte:window on:click={handleClickOutside} />

<div class="chain-selector">
    <button
        class="chain-selector-button"
        on:click={toggleDropdown}
        class:active={showDropdown}
    >
        {#if currentChain}
            <span class="chain-icon">{getChainIcon(currentChain.category)}</span>
            <span class="chain-name">{formatChainName(currentChain)}</span>
            <span class="chain-symbol">({currentChain.nativeCurrency.symbol})</span>
        {:else}
            <span class="select-chain">Select Network</span>
        {/if}
        <svg class="chevron" class:rotate={showDropdown} width="12" height="12" viewBox="0 0 12 12">
            <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" fill="none"/>
        </svg>
    </button>
    
    {#if showDropdown}
        <div class="dropdown">
            <div class="dropdown-header">
                <input
                    type="text"
                    class="search-input"
                    placeholder="Search networks..."
                    bind:value={searchQuery}
                    on:click|stopPropagation
                />
                
                <label class="testnet-toggle">
                    <input
                        type="checkbox"
                        bind:checked={showTestnets}
                    />
                    <span>Show Testnets</span>
                </label>
            </div>
            
            <div class="category-tabs">
                <button
                    class="category-tab"
                    class:active={selectedCategory === 'all'}
                    on:click={() => selectedCategory = 'all'}
                >
                    All
                </button>
                {#each availableCategories as category}
                    <button
                        class="category-tab"
                        class:active={selectedCategory === category}
                        on:click={() => selectedCategory = category}
                    >
                        {categoryLabels[category] || category}
                    </button>
                {/each}
            </div>
            
            <div class="chain-list">
                {#each Object.entries(filteredNetworks) as [category, chains]}
                    <div class="category-section">
                        <h3 class="category-title">{categoryLabels[category] || category}</h3>
                        {#each chains as chain}
                            <button
                                class="chain-item"
                                class:selected={currentChain?.id === chain.id}
                                on:click={() => selectChain(chain)}
                            >
                                <span class="chain-icon">{getChainIcon(category)}</span>
                                <div class="chain-info">
                                    <span class="chain-name">{formatChainName(chain)}</span>
                                    <span class="chain-details">
                                        Chain ID: {chain.id} • {chain.nativeCurrency?.symbol ?? "?"}
                                    </span>
                                </div>
                                {#if currentChain?.id === chain.id}
                                    <span class="check-icon">✓</span>
                                {/if}
                            </button>
                        {/each}
                    </div>
                {/each}
                
                {#if Object.keys(filteredNetworks).length === 0}
                    <div class="no-results">
                        No networks found matching your criteria
                    </div>
                {/if}
            </div>
        </div>
    {/if}
</div>

<style>
    .chain-selector {
        position: relative;
    }
    
    .chain-selector-button {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 10px 16px;
        background: white;
        border: 1px solid #e0e0e0;
        border-radius: 8px;
        cursor: pointer;
        transition: all 0.2s;
        width: 100%;
        justify-content: space-between;
    }
    
    .chain-selector-button:hover {
        border-color: #3b82f6;
    }
    
    .chain-selector-button.active {
        border-color: #3b82f6;
        box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
    }
    
    .chain-icon {
        font-size: 20px;
    }
    
    .chain-name {
        font-weight: 500;
        flex: 1;
        text-align: left;
    }
    
    .chain-symbol {
        color: #666;
        font-size: 14px;
    }
    
    .select-chain {
        color: #999;
        flex: 1;
        text-align: left;
    }
    
    .chevron {
        transition: transform 0.2s;
    }
    
    .chevron.rotate {
        transform: rotate(180deg);
    }
    
    .dropdown {
        position: absolute;
        top: calc(100% + 8px);
        left: 0;
        right: 0;
        background: white;
        border: 1px solid #e0e0e0;
        border-radius: 12px;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.1);
        max-height: 500px;
        overflow: hidden;
        display: flex;
        flex-direction: column;
        z-index: 1000;
    }
    
    .dropdown-header {
        padding: 16px;
        border-bottom: 1px solid #e0e0e0;
    }
    
    .search-input {
        width: 100%;
        padding: 8px 12px;
        border: 1px solid #e0e0e0;
        border-radius: 6px;
        font-size: 14px;
        margin-bottom: 12px;
    }
    
    .search-input:focus {
        outline: none;
        border-color: #3b82f6;
    }
    
    .testnet-toggle {
        display: flex;
        align-items: center;
        gap: 8px;
        font-size: 14px;
        cursor: pointer;
    }
    
    .testnet-toggle input {
        cursor: pointer;
    }
    
    .category-tabs {
        display: flex;
        gap: 8px;
        padding: 0 16px 16px;
        overflow-x: auto;
        border-bottom: 1px solid #e0e0e0;
    }
    
    .category-tab {
        padding: 6px 12px;
        background: #f5f5f5;
        border: none;
        border-radius: 16px;
        font-size: 13px;
        cursor: pointer;
        white-space: nowrap;
        transition: all 0.2s;
    }
    
    .category-tab:hover {
        background: #e0e0e0;
    }
    
    .category-tab.active {
        background: #3b82f6;
        color: white;
    }
    
    .chain-list {
        overflow-y: auto;
        flex: 1;
        max-height: 300px;
    }
    
    .category-section {
        padding: 8px 0;
    }
    
    .category-title {
        padding: 8px 16px;
        font-size: 12px;
        font-weight: 600;
        color: #666;
        text-transform: uppercase;
    }
    
    .chain-item {
        display: flex;
        align-items: center;
        gap: 12px;
        width: 100%;
        padding: 12px 16px;
        background: none;
        border: none;
        cursor: pointer;
        transition: background 0.2s;
    }
    
    .chain-item:hover {
        background: #f5f5f5;
    }
    
    .chain-item.selected {
        background: #e6f2ff;
    }
    
    .chain-info {
        flex: 1;
        text-align: left;
    }
    
    .chain-details {
        display: block;
        font-size: 12px;
        color: #666;
        margin-top: 2px;
    }
    
    .check-icon {
        color: #3b82f6;
        font-weight: bold;
    }
    
    .no-results {
        padding: 32px 16px;
        text-align: center;
        color: #999;
        font-size: 14px;
    }
</style>