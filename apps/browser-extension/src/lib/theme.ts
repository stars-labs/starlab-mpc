/**
 * Theme management for the popup.
 *
 * Three modes: "system" (default — follows the OS color scheme and
 * updates live), "light", "dark". The choice is persisted to
 * localStorage under THEME_KEY and applied by toggling the `.dark`
 * class on <html>. The same class drives both the Tailwind `dark:`
 * variant and the CSS-variable palette in app.css.
 *
 * index.html runs an inline copy of {@link resolveDark} before the
 * bundle loads to avoid a flash of the wrong theme.
 */
import { writable } from "svelte/store";

export type ThemeMode = "system" | "light" | "dark";

export const THEME_KEY = "starlab_mpc_theme";

function systemPrefersDark(): boolean {
    return (
        typeof window !== "undefined" &&
        window.matchMedia?.("(prefers-color-scheme: dark)").matches === true
    );
}

/** Whether the given mode should render dark right now. */
export function resolveDark(mode: ThemeMode): boolean {
    if (mode === "dark") return true;
    if (mode === "light") return false;
    return systemPrefersDark();
}

function readStored(): ThemeMode {
    try {
        const v = localStorage.getItem(THEME_KEY);
        if (v === "light" || v === "dark" || v === "system") return v;
    } catch {
        /* ignore */
    }
    return "system";
}

function applyDarkClass(isDark: boolean) {
    if (typeof document === "undefined") return;
    document.documentElement.classList.toggle("dark", isDark);
}

/** Current theme mode (system | light | dark). */
export const themeMode = writable<ThemeMode>(readStored());

/** Set + persist the theme mode and apply it immediately. */
export function setTheme(mode: ThemeMode) {
    try {
        localStorage.setItem(THEME_KEY, mode);
    } catch {
        /* ignore */
    }
    themeMode.set(mode);
    applyDarkClass(resolveDark(mode));
}

/**
 * Wire up live theme application. Call once on popup mount. Returns a
 * cleanup function. Applies the current mode and, while in "system"
 * mode, re-applies whenever the OS preference flips.
 */
export function initTheme(): () => void {
    const mode = readStored();
    applyDarkClass(resolveDark(mode));

    if (typeof window === "undefined" || !window.matchMedia) {
        return () => {};
    }
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
        if (readStored() === "system") {
            applyDarkClass(mq.matches);
        }
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
}
