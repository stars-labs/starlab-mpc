<script lang="ts">
    import { onMount } from "svelte";
    import NetworkService from "../services/networkService";
    import { mainnet } from "viem/chains";
    import type { Chain } from "@mpc-wallet/types/network";
    import type { SupportedChain } from "@mpc-wallet/types/appstate";
    import {
        CURVE_COMPATIBLE_CHAINS,
        getCompatibleChains,
        getRequiredCurve,
    } from "@mpc-wallet/types/appstate";
    import { createPublicClient, http } from "viem";
    import { createEventDispatcher } from "svelte";
    import { themeMode, setTheme, type ThemeMode } from "../lib/theme";
    import Button from "../lib/ui/Button.svelte";
    import Icon from "../lib/ui/Icon.svelte";
    import {
        getRoom,
        setRoom,
        newRoom,
        isValidRoom,
    } from "../config/signal-server";

    // Tenant room (multi-tenant signal server, #31). The hosted server routes
    // each ?room=<id> to an isolated instance and REQUIRES a strong room.
    let signalRoom = "";
    let roomStatus = "";
    onMount(async () => {
        signalRoom = (await getRoom()) ?? "";
    });
    function genRoom() {
        signalRoom = newRoom();
        roomStatus = "Generated — Save, then share this exact id with co-signers.";
    }
    async function saveRoom() {
        const r = signalRoom.trim();
        if (!isValidRoom(r)) {
            roomStatus = "✗ Need ≥16 chars of [A-Za-z0-9_-] (use Generate).";
            return;
        }
        if (!(await setRoom(r))) {
            roomStatus = "✗ Save failed — couldn't write to extension storage. Reload the extension and try again.";
            return;
        }
        // Apply immediately: the startup WS connect is roomless (it ran before a
        // room existed) and the multi-tenant server rejects it, so without a
        // reconnect the saved room never takes effect. Ask the background to
        // re-resolve the URL and reconnect.
        roomStatus = "✓ Saved — reconnecting…";
        try {
            await chrome.runtime.sendMessage({ type: "reconnectSignal" });
            roomStatus = "✓ Saved & reconnecting.";
        } catch (e) {
            roomStatus = "✓ Saved. Reload the extension to apply.";
        }
    }
    const dispatch = createEventDispatcher<{
        backToWallet: { chain: string; curve: string };
    }>();

    // Theme is managed by the shared store (system | light | dark) so it
    // stays in sync with the header toggle and survives reloads.
    let currentTheme: ThemeMode = "system";
    themeMode.subscribe((v) => (currentTheme = v));
    const THEME_OPTIONS: { value: ThemeMode; label: string; icon: string }[] = [
        { value: "system", label: "System", icon: "monitor" },
        { value: "light", label: "Light", icon: "sun" },
        { value: "dark", label: "Dark", icon: "moon" },
    ];

    // Wallet configuration
    let curve: "secp256k1" | "ed25519" = "secp256k1";
    let chain: SupportedChain = "ethereum";
    let networks: Chain[] = [];
    let currentNetwork: Chain | undefined;
    let networkService: NetworkService;

    // Custom network form
    let showCustomNetworkForm = false;
    let customNetworkName = "";
    let customNetworkRpcUrl = "";
    let customNetworkChainId = "";
    let customNetworkSymbol = "";
    let customNetworkExplorer = "";
    let customNetworkError = "";

    // Initialize network service and load data
    onMount(async () => {
        networkService = NetworkService.getInstance();

        try {
            // Sync chain with current settings
            const message = await chrome.runtime.sendMessage({
                type: "getState",
            });
            if (message && message.blockchain) {
                chain = message.blockchain;
                // Get curve from state if available, otherwise use sensible default
                if (message.curve) {
                    curve = message.curve;
                } else {
                    // Legacy support: default curve for existing chain selection
                    curve = chain === "ethereum" ? "secp256k1" : "ed25519";
                }
            }

            // Get networks for current chain - use new method that supports Layer 2 chains
            const chainNetworks = networkService.getNetworksForChain(chain);
            if (Array.isArray(chainNetworks)) {
                networks = chainNetworks;
                currentNetwork =
                    networkService.getCurrentNetworkForChain(chain);
            } else {
                console.error(
                    "[Settings] Networks is not an array:",
                    chainNetworks,
                );
                networks = [];
            }
        } catch (error) {
            console.error("[Settings] Error initializing:", error);
        }
    });

    // Handle curve change - validate compatibility and set sensible defaults
    function handleCurveChange() {
        console.log("[Settings] Curve changed to:", curve);

        // Check if current chain is compatible with new curve
        const compatibleChains = getCompatibleChains(curve);
        if (!compatibleChains.includes(chain)) {
            // Current chain is not compatible, switch to the first compatible chain
            chain = compatibleChains[0];
            console.log("[Settings] Switched to compatible chain:", chain);
        }

        updateBlockchainSelection();
    }

    // Handle chain change - validate compatibility and set sensible defaults
    function handleChainChange() {
        console.log("[Settings] Chain changed to:", chain);

        // Ensure curve is compatible with the selected chain
        const requiredCurve = getRequiredCurve(chain);
        if (curve !== requiredCurve) {
            curve = requiredCurve;
            console.log("[Settings] Updated curve to compatible:", curve);
        }

        updateBlockchainSelection();

        try {
            // Update networks list when chain changes - use new method that supports Layer 2 chains
            const chainNetworks = networkService.getNetworksForChain(chain);
            if (Array.isArray(chainNetworks)) {
                networks = chainNetworks;
                currentNetwork =
                    networkService.getCurrentNetworkForChain(chain);
            } else {
                console.error(
                    "[Settings] Networks is not an array:",
                    chainNetworks,
                );
                networks = [];
            }
        } catch (error) {
            console.error("[Settings] Error updating networks:", error);
            networks = [];
        }
    }

    // Update blockchain selection in background
    // Event dispatcher for parent components

    function updateBlockchainSelection() {
        chrome.runtime.sendMessage(
            {
                type: "setBlockchain",
                blockchain: chain,
            },
            (response) => {
                if (chrome.runtime.lastError) {
                    console.error(
                        "[Settings] Error setting blockchain:",
                        chrome.runtime.lastError.message,
                    );
                } else {
                    console.log(
                        "[Settings] Blockchain selection saved:",
                        chain,
                    );
                    // Only update locally without closing settings
                    // We don't want to automatically return to wallet page anymore
                    console.log(
                        "[Settings] Blockchain selection saved locally:",
                        chain,
                    );
                }
            },
        );
    }

    // Handle network change
    async function handleNetworkChange(event: Event) {
        const select = event.target as HTMLSelectElement;
        const networkId = parseInt(select.value, 10);
        try {
            await networkService.setCurrentNetworkForChain(chain, networkId);
            currentNetwork = networkService.getCurrentNetworkForChain(chain);
        } catch (error) {
            console.error("[Settings] Failed to change network:", error);
        }
    }

    // Toggle custom network form
    function toggleCustomNetworkForm() {
        showCustomNetworkForm = !showCustomNetworkForm;
        resetCustomNetworkForm();
    }

    // Reset custom network form
    function resetCustomNetworkForm() {
        customNetworkName = "";
        customNetworkRpcUrl = "";
        customNetworkChainId = "";
        customNetworkSymbol = "";
        customNetworkExplorer = "";
        customNetworkError = "";
    }

    // Add custom network
    async function addCustomNetwork() {
        if (
            !customNetworkName ||
            !customNetworkRpcUrl ||
            !customNetworkChainId ||
            !customNetworkSymbol
        ) {
            customNetworkError = "Please fill out all required fields";
            return;
        }

        const chainId = parseInt(customNetworkChainId, 10);
        if (isNaN(chainId)) {
            customNetworkError = "Chain ID must be a valid number";
            return;
        }

        try {
            // Create a custom chain configuration
            const customChain: Chain = {
                id: chainId,
                name: customNetworkName,
                network: customNetworkName.toLowerCase().replace(/\s+/g, "-"),
                nativeCurrency: {
                    name: customNetworkName,
                    symbol: customNetworkSymbol,
                    decimals: 18,
                },
                rpcUrls: {
                    default: {
                        http: [customNetworkRpcUrl],
                    },
                    public: {
                        http: [customNetworkRpcUrl],
                    },
                },
                blockExplorers: customNetworkExplorer
                    ? {
                          default: {
                              name: "Explorer",
                              url: customNetworkExplorer,
                          },
                      }
                    : undefined,
            };

            // Add the custom network - use new method that supports Layer 2 chains
            await networkService.addNetworkForChain(chain, customChain);

            // Refresh the networks list - use new method that supports Layer 2 chains
            networks = networkService.getNetworksForChain(chain);

            // Switch to the new network - use new method that supports Layer 2 chains
            await networkService.setCurrentNetworkForChain(chain, chainId);
            currentNetwork = networkService.getCurrentNetworkForChain(chain);

            // Hide the form
            showCustomNetworkForm = false;
            resetCustomNetworkForm();
        } catch (error) {
            console.error("[Settings] Failed to add custom network:", error);
            customNetworkError = `Failed to add network: ${error instanceof Error ? error.message : String(error)}`;
        }
    }
</script>

<div class="space-y-4">
    <div class="flex items-center justify-between">
        <h2 class="text-base font-bold">Settings</h2>
        <Button
            variant="secondary"
            size="sm"
            on:click={() => dispatch("backToWallet", { chain, curve })}
        >
            Done
        </Button>
    </div>

    <!-- Appearance -->
    <div class="card card-pad">
        <h3 class="section-title mb-2.5">Appearance</h3>
        <div class="grid grid-cols-3 gap-1.5 rounded-xl bg-surface-2 p-1">
            {#each THEME_OPTIONS as opt}
                <button
                    class="flex flex-col items-center gap-1 rounded-lg py-2 text-xs font-semibold transition {currentTheme ===
                    opt.value
                        ? 'bg-surface text-content shadow-sm'
                        : 'text-muted hover:text-content'}"
                    on:click={() => setTheme(opt.value)}
                >
                    <Icon name={opt.icon} size={16} />
                    {opt.label}
                </button>
            {/each}
        </div>
    </div>

    <!-- Signal server / tenant room -->
    <div class="card card-pad space-y-2">
        <h3 class="section-title">Signal server room</h3>
        <p class="text-xs text-muted">
            The hosted server requires a strong room. All co-signers of a wallet
            must use the <strong>same</strong> room. Reconnect after changing it.
        </p>
        <div class="flex items-center gap-2">
            <input
                type="text"
                class="input flex-1 text-xs"
                placeholder="strong room id (shared)"
                bind:value={signalRoom}
                data-testid="room-input"
            />
            <Button variant="ghost" size="sm" on:click={genRoom}>Generate</Button>
            <Button size="sm" on:click={saveRoom}>Save</Button>
        </div>
        {#if roomStatus}
            <span class="text-xs text-muted" data-testid="room-status">{roomStatus}</span>
        {/if}
    </div>

    <!-- Network -->
    <div class="card card-pad space-y-3">
        <h3 class="section-title">Network</h3>

        <div>
            <label class="label" for="chain-select">Blockchain</label>
            <select
                id="chain-select"
                bind:value={chain}
                on:change={handleChainChange}
                class="select"
            >
                <optgroup label="Ethereum / EVM (secp256k1)">
                    <option value="ethereum">Ethereum</option>
                    <option value="polygon">Polygon</option>
                    <option value="arbitrum">Arbitrum</option>
                    <option value="optimism">Optimism</option>
                    <option value="base">Base</option>
                </optgroup>
                <optgroup label="ed25519">
                    <option value="solana">Solana</option>
                    <option value="sui">Sui</option>
                </optgroup>
            </select>
        </div>

        {#if ["ethereum", "polygon", "arbitrum", "optimism", "base"].includes(chain) && networks.length > 0}
            <div>
                <label class="label" for="network-select">Network</label>
                <select
                    id="network-select"
                    on:change={handleNetworkChange}
                    class="select"
                    value={currentNetwork?.id}
                >
                    {#each networks as network}
                        <option value={network.id}>{network.name}</option>
                    {/each}
                </select>
            </div>

            <button
                class="btn btn-secondary btn-sm btn-block"
                on:click={toggleCustomNetworkForm}
            >
                {showCustomNetworkForm ? "Hide custom network" : "Add custom network"}
            </button>

            {#if currentNetwork}
                <div class="rounded-lg bg-surface-2 p-2.5 text-xs text-muted">
                    <p><span class="font-semibold">Chain ID:</span> {currentNetwork.id}</p>
                    <p>
                        <span class="font-semibold">Name:</span>
                        {currentNetwork.name}
                    </p>
                    {#if currentNetwork.rpcUrls?.default?.http}
                        <p class="break-all">
                            <span class="font-semibold">RPC:</span>
                            {currentNetwork.rpcUrls.default.http[0]}
                        </p>
                    {/if}
                </div>
            {/if}
        {/if}

        <!-- Advanced: curve (technical) -->
        <details class="text-sm">
            <summary class="cursor-pointer text-xs font-semibold text-muted"
                >Advanced</summary
            >
            <div class="mt-2">
                <label class="label" for="curve-select">Signature curve</label>
                <select
                    id="curve-select"
                    bind:value={curve}
                    on:change={handleCurveChange}
                    class="select"
                >
                    <option value="secp256k1">secp256k1</option>
                    <option value="ed25519">ed25519</option>
                </select>
            </div>
        </details>
    </div>

    <!-- Add Custom Network -->
    {#if showCustomNetworkForm}
        <div class="card card-pad space-y-3">
            <h3 class="section-title">Add custom network</h3>

            {#if customNetworkError}
                <div class="alert alert-danger text-xs">{customNetworkError}</div>
            {/if}

            <div>
                <label class="label" for="network-name">Network name</label>
                <input
                    id="network-name"
                    type="text"
                    class="input"
                    placeholder="My Custom Network"
                    bind:value={customNetworkName}
                />
            </div>
            <div>
                <label class="label" for="network-rpc">RPC URL</label>
                <input
                    id="network-rpc"
                    type="text"
                    class="input"
                    placeholder="https://rpc.example.com"
                    bind:value={customNetworkRpcUrl}
                />
            </div>
            <div class="grid grid-cols-2 gap-3">
                <div>
                    <label class="label" for="network-chainid">Chain ID</label>
                    <input
                        id="network-chainid"
                        type="text"
                        class="input"
                        placeholder="1"
                        bind:value={customNetworkChainId}
                    />
                </div>
                <div>
                    <label class="label" for="network-symbol">Symbol</label>
                    <input
                        id="network-symbol"
                        type="text"
                        class="input"
                        placeholder="ETH"
                        bind:value={customNetworkSymbol}
                    />
                </div>
            </div>
            <div>
                <label class="label" for="network-explorer"
                    >Block explorer (optional)</label
                >
                <input
                    id="network-explorer"
                    type="text"
                    class="input"
                    placeholder="https://etherscan.io"
                    bind:value={customNetworkExplorer}
                />
            </div>

            <div class="flex gap-2">
                <Button variant="secondary" block on:click={toggleCustomNetworkForm}>
                    Cancel
                </Button>
                <Button block variant="success" on:click={addCustomNetwork}>
                    Save network
                </Button>
            </div>
        </div>
    {/if}
</div>
