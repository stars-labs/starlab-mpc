<script lang="ts">
    /**
     * Ext-1b: minimal "Create Wallet" form. Fires
     * CREATE_DKG_WALLET over chrome.runtime. Background announces via
     * `announce_session` — any TUI or extension peer on the same
     * signal server can discover and join.
     *
     * This is the DKG *initiator* flow only. Joiner side is Ext-1e.
     */
    import { createEventDispatcher } from "svelte";
    import { MESSAGE_TYPES } from "@mpc-wallet/types/messages";
    import Button from "../lib/ui/Button.svelte";
    import Icon from "../lib/ui/Icon.svelte";
    import { guideError } from "../utils/error-guidance";

    let showAdvanced = false;

    export let deviceId: string = "";
    export let wsConnected: boolean = false;

    const dispatch = createEventDispatcher<{
        created: { sessionId: string };
        cancel: void;
    }>();

    // Defaults match TUI's ThresholdConfig screen starting values.
    let total = 3;
    let threshold = 2;
    let curve: "secp256k1" | "ed25519" = "secp256k1";
    let walletName = "";
    let submitting = false;
    let errorMessage = "";

    // Keep threshold clamped to [2, total] — threshold < 2 defeats the
    // purpose of multisig, and > total is nonsense. Mirror TUI.
    $: if (threshold > total) threshold = total;
    $: if (threshold < 2) threshold = 2;
    $: if (total < 2) total = 2;
    $: if (total > 10) total = 10;

    async function handleSubmit() {
        errorMessage = "";
        if (!wsConnected) {
            errorMessage =
                "Signal server not connected. Check Settings → Signal Server.";
            return;
        }
        submitting = true;
        try {
            const response = await chrome.runtime.sendMessage({
                type: MESSAGE_TYPES.CREATE_DKG_WALLET,
                name: walletName.trim() || undefined,
                total,
                threshold,
                curve,
            });
            if (response?.success && response.sessionId) {
                dispatch("created", { sessionId: response.sessionId });
            } else {
                errorMessage = guideError(
                    response?.error ?? "Failed to create wallet (no error returned)",
                    "dkg",
                );
            }
        } catch (e) {
            errorMessage = guideError((e as Error).message ?? String(e), "dkg");
        } finally {
            submitting = false;
        }
    }
</script>

<div class="card card-pad">
    <h2 class="text-base font-bold">Create a wallet</h2>
    <p class="mt-1 text-xs text-muted">
        You'll set up a shared wallet that several devices co-manage. Others
        join from their own wallet once you create it — no single device can
        sign alone.
    </p>

    <div class="mt-4">
        <label class="label" for="cw-name">Wallet name (optional)</label>
        <input
            id="cw-name"
            type="text"
            bind:value={walletName}
            placeholder="e.g. Treasury"
            class="input"
            disabled={submitting}
        />
    </div>

    <div class="mt-3 grid grid-cols-2 gap-3">
        <div>
            <label class="label" for="cw-total">Total devices</label>
            <input
                id="cw-total"
                type="number"
                min="2"
                max="10"
                bind:value={total}
                class="input"
                disabled={submitting}
            />
        </div>
        <div>
            <label class="label" for="cw-threshold">Needed to sign</label>
            <input
                id="cw-threshold"
                type="number"
                min="2"
                max={total}
                bind:value={threshold}
                class="input"
                disabled={submitting}
            />
        </div>
    </div>

    <div class="mt-3">
        <label class="label" for="cw-curve">Network</label>
        <select
            id="cw-curve"
            bind:value={curve}
            class="select"
            disabled={submitting}
        >
            <option value="secp256k1">Ethereum &amp; EVM chains</option>
            <option value="ed25519">Solana</option>
        </select>
    </div>

    <div class="mt-3 alert alert-info text-xs">
        Any <b>{threshold}</b> of <b>{total}</b> devices will be able to approve
        a transaction together.
    </div>

    <!-- Advanced / technical details -->
    <button
        type="button"
        class="mt-3 flex w-full items-center justify-between text-xs font-semibold text-muted"
        on:click={() => (showAdvanced = !showAdvanced)}
    >
        <span>Advanced details</span>
        <span class="transition-transform" class:rotate-180={showAdvanced}>
            <Icon name="chevron" size={15} />
        </span>
    </button>
    {#if showAdvanced}
        <div class="mt-2 space-y-1 rounded-lg bg-surface-2 p-2.5 text-xs text-muted">
            <p>
                Runs a FROST <b>{threshold}-of-{total}</b> distributed key
                generation (DKG) on the
                <span class="mono"
                    >{curve === "secp256k1" ? "secp256k1" : "ed25519"}</span
                > curve.
            </p>
            <p class="mono break-all">
                initiator: {deviceId || "unregistered"}
            </p>
            <p>
                The session is announced on the signal server; any TUI node or
                extension can discover and join it.
            </p>
        </div>
    {/if}

    {#if errorMessage}
        <div class="mt-3 alert alert-danger text-xs">{errorMessage}</div>
    {/if}

    <div class="mt-4 flex gap-2">
        <Button
            block
            on:click={handleSubmit}
            disabled={submitting || !wsConnected}
        >
            {submitting ? "Creating…" : "Create wallet"}
        </Button>
        <Button
            variant="secondary"
            on:click={() => dispatch("cancel")}
            disabled={submitting}
        >
            Cancel
        </Button>
    </div>
</div>
