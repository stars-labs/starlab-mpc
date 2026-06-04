<script lang="ts">
    import Icon from "./Icon.svelte";

    export let title: string;
    export let subtitle: string = "";
    export let icon: string | undefined = undefined;
    export let open: boolean = false;

    function toggle() {
        open = !open;
    }
</script>

<div class="card overflow-hidden">
    <button
        class="flex w-full items-center gap-2.5 px-3.5 py-3 text-left"
        on:click={toggle}
        aria-expanded={open}
    >
        {#if icon}
            <span class="text-muted"><Icon name={icon} size={16} /></span>
        {/if}
        <span class="min-w-0 flex-1">
            <span class="block text-sm font-semibold text-content">{title}</span>
            {#if subtitle}
                <span class="block truncate text-xs text-muted">{subtitle}</span>
            {/if}
        </span>
        <span
            class="text-faint transition-transform duration-200"
            class:rotate-180={open}
        >
            <Icon name="chevron" size={16} />
        </span>
    </button>
    {#if open}
        <div class="border-t border-line px-3.5 py-3">
            <slot />
        </div>
    {/if}
</div>
