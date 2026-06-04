<script lang="ts">
    import Icon from "./Icon.svelte";

    export let value: string;
    export let label: string = "Copy";
    /** "button" = full labelled button; "icon" = compact icon-only. */
    export let variant: "button" | "icon" = "button";
    export let title: string = "Copy to clipboard";

    let copied = false;
    async function copy() {
        try {
            await navigator.clipboard.writeText(value);
            copied = true;
            setTimeout(() => (copied = false), 1600);
        } catch (e) {
            console.warn("[UI] Clipboard copy failed:", e);
        }
    }
</script>

{#if variant === "icon"}
    <button
        class="icon-btn"
        style="width:1.9rem;height:1.9rem"
        on:click={copy}
        {title}
        aria-label={title}
    >
        <Icon name={copied ? "check" : "copy"} size={15} />
    </button>
{:else}
    <button class="btn btn-secondary btn-sm" on:click={copy} {title}>
        <Icon name={copied ? "check" : "copy"} size={14} />
        {copied ? "Copied" : label}
    </button>
{/if}
