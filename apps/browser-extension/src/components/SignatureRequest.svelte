<script lang="ts">
    import { createEventDispatcher } from 'svelte';
    import Button from "../lib/ui/Button.svelte";
    import Icon from "../lib/ui/Icon.svelte";

    export let signingId: string;
    export let message: string;
    export let origin: string;
    export let fromAddress: string;
    
    const dispatch = createEventDispatcher();
    
    // Track approval state
    let approving = false;
    let rejecting = false;
    
    // Format message for display
    function formatMessage(msg: string): string {
        // Check if it's hex data
        if (msg.startsWith('0x')) {
            // If it's a long hex string, truncate it
            if (msg.length > 66) {
                return msg.slice(0, 20) + '...' + msg.slice(-20);
            }
            return msg;
        }
        // Regular text message
        return msg;
    }
    
    // Format address for display
    function formatAddress(addr: string): string {
        if (addr.length > 10) {
            return addr.slice(0, 6) + '...' + addr.slice(-4);
        }
        return addr;
    }
    
    async function handleApprove() {
        approving = true;
        try {
            chrome.runtime.sendMessage({
                type: 'approveMessageSignature',
                requestId: signingId,
                approved: true
            }, (response) => {
                if (chrome.runtime.lastError) {
                    console.error('[SignatureRequest] Error approving signature:', chrome.runtime.lastError.message);
                    return;
                }
                console.log('[SignatureRequest] Signature approved');
                dispatch('complete');
            });
        } catch (error) {
            console.error('[SignatureRequest] Error approving signature:', error);
        } finally {
            approving = false;
        }
    }
    
    async function handleReject() {
        rejecting = true;
        try {
            chrome.runtime.sendMessage({
                type: 'approveMessageSignature',
                requestId: signingId,
                approved: false
            }, (response) => {
                if (chrome.runtime.lastError) {
                    console.error('[SignatureRequest] Error rejecting signature:', chrome.runtime.lastError.message);
                    return;
                }
                console.log('[SignatureRequest] Signature rejected');
                dispatch('complete');
            });
        } catch (error) {
            console.error('[SignatureRequest] Error rejecting signature:', error);
        } finally {
            rejecting = false;
        }
    }
</script>

<div class="card card-pad">
    <div class="mb-3 flex items-center justify-between">
        <h3 class="flex items-center gap-1.5 text-sm font-bold">
            <Icon name="edit" size={15} /> Signature request
        </h3>
        <span class="badge badge-warning">Pending</span>
    </div>

    <dl class="space-y-2 text-xs">
        <div>
            <dt class="label mb-1">From site</dt>
            <dd class="mono break-all rounded-lg bg-surface-2 px-2 py-1">{origin}</dd>
        </div>
        <div>
            <dt class="label mb-1">Account</dt>
            <dd class="mono rounded-lg bg-surface-2 px-2 py-1">
                {formatAddress(fromAddress)}
            </dd>
        </div>
        <div>
            <dt class="label mb-1">Message</dt>
            <dd class="mono break-all rounded-lg bg-surface-2 p-2">
                {formatMessage(message)}
            </dd>
        </div>
    </dl>

    <p class="mt-3 text-xs text-muted">
        <span class="font-semibold text-content">{origin}</span> wants you to sign
        this message with your wallet.
    </p>

    <div class="mt-3 flex gap-2">
        <Button
            variant="secondary"
            block
            on:click={handleReject}
            disabled={approving || rejecting}
        >
            {rejecting ? "Rejecting…" : "Reject"}
        </Button>
        <Button block on:click={handleApprove} disabled={approving || rejecting}>
            {approving ? "Signing…" : "Sign"}
        </Button>
    </div>
</div>