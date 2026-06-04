/**
 * Signal-server URL config, single source of truth.
 *
 * The default is `wss://panda.qzz.io` (see `apps/tui-node/src/elm/model.rs`),
 * which points at the Cloudflare Worker variant of the signal server — the
 * TUI and the extension must share this default so a TUI node and an
 * extension can see each other's session broadcasts.
 *
 * This module centralizes the decision: one default, one override
 * mechanism, one call site for callers to fetch the value. Every caller
 * routes through {@link getSignalServerUrl}.
 */

/**
 * Fallback URL used when the user hasn't set an override. Intentionally
 * matches the TUI default so extension+TUI can interop out of the box.
 */
export const DEFAULT_SIGNAL_SERVER_URL = "wss://panda.qzz.io";

/**
 * chrome.storage.local key for the user-configurable override. Reads
 * return `undefined` when unset, which falls through to the default.
 */
export const SIGNAL_SERVER_STORAGE_KEY = "signalServerUrl";

/**
 * Resolve the effective signal-server URL. Checks chrome.storage.local
 * first; falls back to {@link DEFAULT_SIGNAL_SERVER_URL}. Guards against
 * chrome.storage being unavailable (e.g. tests, non-extension contexts)
 * by returning the default silently.
 */
export async function getSignalServerUrl(): Promise<string> {
    try {
        if (typeof chrome === "undefined" || !chrome.storage?.local?.get) {
            return DEFAULT_SIGNAL_SERVER_URL;
        }
        const stored = await chrome.storage.local.get(SIGNAL_SERVER_STORAGE_KEY);
        const override = stored[SIGNAL_SERVER_STORAGE_KEY];
        if (typeof override === "string" && override.length > 0) {
            return override;
        }
        return DEFAULT_SIGNAL_SERVER_URL;
    } catch {
        return DEFAULT_SIGNAL_SERVER_URL;
    }
}

/**
 * Persist a user-selected URL as the override. Validates the ws:// or
 * wss:// scheme before writing. Returns true if the write succeeded,
 * false on a scheme-rejection or missing chrome.storage.
 */
export async function setSignalServerUrl(url: string): Promise<boolean> {
    if (!url.startsWith("ws://") && !url.startsWith("wss://")) {
        return false;
    }
    try {
        if (typeof chrome === "undefined" || !chrome.storage?.local?.set) {
            return false;
        }
        await chrome.storage.local.set({ [SIGNAL_SERVER_STORAGE_KEY]: url });
        return true;
    } catch {
        return false;
    }
}
