<script lang="ts">
    export let variant:
        | "primary"
        | "secondary"
        | "ghost"
        | "success"
        | "danger" = "primary";
    export let size: "md" | "sm" = "md";
    export let block: boolean = false;
    export let type: "button" | "submit" = "button";
    export let disabled: boolean = false;
    export let title: string | undefined = undefined;
    let cls = "";
    export { cls as class };
</script>

<button
    {type}
    {disabled}
    {title}
    class="btn btn-{variant} {size === 'sm' ? 'btn-sm' : ''} {block
        ? 'btn-block'
        : ''} {cls}"
    on:click
>
    <slot />
</button>
