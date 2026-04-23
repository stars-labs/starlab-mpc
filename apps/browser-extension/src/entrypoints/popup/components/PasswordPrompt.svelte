<script lang="ts">
    import { createEventDispatcher } from 'svelte';
    
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

<div class="password-overlay" role="dialog" aria-modal="true" aria-labelledby="password-prompt-title" tabindex="-1" on:keydown={handleKeydown}>
    <div class="password-modal">
        <h2 id="password-prompt-title">{title}</h2>
        <p class="message">{message}</p>
        
        <div class="input-group">
            <label for="password">Password</label>
            <div class="password-input-wrapper">
                <input
                    id="password"
                    type={showPassword ? 'text' : 'password'}
                    bind:value={password}
                    placeholder="Enter password"
                    autocomplete={confirmMode ? 'new-password' : 'current-password'}
                    use:autofocus
                    disabled={loading}
                />
                <button
                    type="button"
                    class="toggle-password"
                    on:click={() => showPassword = !showPassword}
                    tabindex="-1"
                >
                    {showPassword ? '👁️‍🗨️' : '👁️'}
                </button>
            </div>
        </div>
        
        {#if confirmMode}
            <div class="input-group">
                <label for="confirmPassword">Confirm Password</label>
                <div class="password-input-wrapper">
                    <input
                        id="confirmPassword"
                        type={showConfirmPassword ? 'text' : 'password'}
                        bind:value={confirmPassword}
                        placeholder="Confirm password"
                        autocomplete="new-password"
                        disabled={loading}
                    />
                    <button
                        type="button"
                        class="toggle-password"
                        on:click={() => showConfirmPassword = !showConfirmPassword}
                        tabindex="-1"
                    >
                        {showConfirmPassword ? '👁️‍🗨️' : '👁️'}
                    </button>
                </div>
            </div>
        {/if}
        
        {#if error}
            <div class="error-message">{error}</div>
        {/if}
        
        <div class="button-group">
            <button
                type="button"
                class="cancel-button"
                on:click={handleCancel}
                disabled={loading}
            >
                Cancel
            </button>
            <button
                type="button"
                class="submit-button"
                on:click={handleSubmit}
                disabled={loading || !password || (confirmMode && !confirmPassword)}
            >
                {loading ? 'Processing...' : 'Submit'}
            </button>
        </div>
    </div>
</div>

<style>
    .password-overlay {
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background-color: rgba(0, 0, 0, 0.5);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
    }
    
    .password-modal {
        background: var(--color-surface, #ffffff);
        border-radius: 12px;
        padding: 24px;
        width: 90%;
        max-width: 400px;
        box-shadow: 0 10px 25px rgba(0, 0, 0, 0.1);
    }
    
    h2 {
        margin: 0 0 8px 0;
        color: var(--color-text, #333);
        font-size: 20px;
    }
    
    .message {
        margin: 0 0 20px 0;
        color: var(--color-text-secondary, #666);
        font-size: 14px;
    }
    
    .input-group {
        margin-bottom: 16px;
    }
    
    label {
        display: block;
        margin-bottom: 6px;
        color: var(--color-text, #333);
        font-size: 14px;
        font-weight: 500;
    }
    
    .password-input-wrapper {
        position: relative;
        display: flex;
        align-items: center;
    }
    
    input {
        width: 100%;
        padding: 10px 40px 10px 12px;
        border: 1px solid var(--color-border, #ddd);
        border-radius: 8px;
        font-size: 14px;
        background: var(--color-input-bg, #fff);
        color: var(--color-text, #333);
        transition: border-color 0.2s;
    }
    
    input:focus {
        outline: none;
        border-color: var(--color-primary, #007bff);
    }
    
    input:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }
    
    .toggle-password {
        position: absolute;
        right: 8px;
        background: none;
        border: none;
        padding: 6px;
        cursor: pointer;
        font-size: 16px;
        color: var(--color-text-secondary, #666);
        transition: color 0.2s;
    }
    
    .toggle-password:hover {
        color: var(--color-text, #333);
    }
    
    .error-message {
        color: var(--color-error, #dc3545);
        font-size: 13px;
        margin-top: -8px;
        margin-bottom: 16px;
    }
    
    .button-group {
        display: flex;
        gap: 12px;
        justify-content: flex-end;
        margin-top: 24px;
    }
    
    button {
        padding: 10px 20px;
        border: none;
        border-radius: 8px;
        font-size: 14px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s;
    }
    
    .cancel-button {
        background: var(--color-button-secondary, #f0f0f0);
        color: var(--color-text, #333);
    }
    
    .cancel-button:hover:not(:disabled) {
        background: var(--color-button-secondary-hover, #e0e0e0);
    }
    
    .submit-button {
        background: var(--color-primary, #007bff);
        color: white;
    }
    
    .submit-button:hover:not(:disabled) {
        background: var(--color-primary-hover, #0056b3);
    }
    
    button:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }
</style>