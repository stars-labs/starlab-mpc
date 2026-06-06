<script lang="ts">
    import { createEventDispatcher } from 'svelte';
    import Button from "../../../lib/ui/Button.svelte";

    export let title: string = "Enter Password";
    export let message: string = "Please enter your keystore password";
    export let confirmMode: boolean = false;
    export let minLength: number = 8;
    
    const dispatch = createEventDispatcher();
    
    let password: string = '';
    let confirmPassword: string = '';
    let showPassword: boolean = false;
    let showConfirmPassword: boolean = false;
    let error: string = '';
    let loading: boolean = false;
    
    function validatePassword(): boolean {
        error = '';
        
        if (password.length < minLength) {
            error = `Password must be at least ${minLength} characters`;
            return false;
        }
        
        if (confirmMode && password !== confirmPassword) {
            error = "Passwords do not match";
            return false;
        }
        
        return true;
    }
    
    async function handleSubmit() {
        if (!validatePassword()) {
            return;
        }
        
        loading = true;
        dispatch('submit', { password });
    }
    
    function handleCancel() {
        dispatch('cancel');
    }
    
    function handleKeydown(event: KeyboardEvent) {
        if (event.key === 'Enter' && !confirmMode) {
            handleSubmit();
        } else if (event.key === 'Escape') {
            handleCancel();
        }
    }

    // Svelte action — deliberate autofocus for a modal primary input.
    // Using the HTML `autofocus` attribute triggers a svelte-check a11y
    // warning (correct in the general case), but on a modal that has
    // just appeared the focus landing on the password field is the
    // expected UX, not a surprise.
    function autofocus(node: HTMLElement) {
        node.focus();
    }
</script>

<div
    class="modal-backdrop"
    role="dialog"
    aria-modal="true"
    aria-labelledby="password-prompt-title"
    tabindex="-1"
    on:keydown={handleKeydown}
>
    <div class="modal-panel">
        <h2 id="password-prompt-title" class="text-base font-bold">{title}</h2>
        <p class="mt-1 text-xs text-muted">{message}</p>

        <div class="mt-4">
            <label class="label" for="password">Password</label>
            <div class="relative">
                <input
                    id="password"
                    class="input pr-16"
                    type={showPassword ? "text" : "password"}
                    bind:value={password}
                    placeholder="Enter password"
                    autocomplete={confirmMode
                        ? "new-password"
                        : "current-password"}
                    use:autofocus
                    disabled={loading}
                />
                <button
                    type="button"
                    class="absolute right-2 top-1/2 -translate-y-1/2 text-xs font-semibold text-muted hover:text-content"
                    on:click={() => (showPassword = !showPassword)}
                    tabindex="-1"
                >
                    {showPassword ? "Hide" : "Show"}
                </button>
            </div>
        </div>

        {#if confirmMode}
            <div class="mt-3">
                <label class="label" for="confirmPassword">Confirm password</label
                >
                <div class="relative">
                    <input
                        id="confirmPassword"
                        class="input pr-16"
                        type={showConfirmPassword ? "text" : "password"}
                        bind:value={confirmPassword}
                        placeholder="Confirm password"
                        autocomplete="new-password"
                        disabled={loading}
                    />
                    <button
                        type="button"
                        class="absolute right-2 top-1/2 -translate-y-1/2 text-xs font-semibold text-muted hover:text-content"
                        on:click={() =>
                            (showConfirmPassword = !showConfirmPassword)}
                        tabindex="-1"
                    >
                        {showConfirmPassword ? "Hide" : "Show"}
                    </button>
                </div>
            </div>
        {/if}

        {#if error}
            <div class="mt-3 alert alert-danger text-xs">{error}</div>
        {/if}

        <div class="mt-5 flex gap-2">
            <Button
                block
                on:click={handleSubmit}
                disabled={loading ||
                    !password ||
                    (confirmMode && !confirmPassword)}
            >
                {loading ? "Working…" : "Continue"}
            </Button>
            <Button variant="secondary" on:click={handleCancel} disabled={loading}>
                Cancel
            </Button>
        </div>
    </div>
</div>