<script lang="ts">
    import { createEventDispatcher } from "svelte";
    import Icon from "./Icon.svelte";

    export let title: string = "";
    export let subtitle: string = "";
    /** Show the ✕ close affordance + allow backdrop click to close. */
    export let dismissable: boolean = true;

    const dispatch = createEventDispatcher();
    function close() {
        if (dismissable) dispatch("close");
    }
</script>

<div
    class="modal-backdrop"
    role="presentation"
    on:click|self={close}
>
    <div class="modal-panel" role="dialog" aria-modal="true">
        {#if title || dismissable}
            <div class="mb-3 flex items-start justify-between gap-3">
                <div>
                    {#if title}
                        <h3 class="text-base font-bold text-content">{title}</h3>
                    {/if}
                    {#if subtitle}
                        <p class="mt-0.5 text-xs text-muted">{subtitle}</p>
                    {/if}
                </div>
                {#if dismissable}
                    <button
                        class="icon-btn -mr-1 -mt-1 shrink-0"
                        style="width:1.9rem;height:1.9rem"
                        on:click={close}
                        aria-label="Close"
                    >
                        <Icon name="x" size={16} />
                    </button>
                {/if}
            </div>
        {/if}
        <slot />
    </div>
</div>
