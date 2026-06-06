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

/** chrome.storage.local key for the tenant room (multi-tenant, #31). */
export const ROOM_STORAGE_KEY = "signalRoom";

/**
 * Minimum room length — must match the server's `MIN_ROOM_LEN`
 * (apps/signal-server/cloudflare-worker/src/lib.rs). The room IS the tenant
 * boundary, so it must be a strong, shared id (e.g. a UUID); the server
 * rejects anything shorter / with a missing room.
 */
export const MIN_ROOM_LEN = 16;

/** True iff `room` is strong enough for the server to accept. */
export function isValidRoom(room: string): boolean {
    return /^[A-Za-z0-9_-]+$/.test(room) && room.length >= MIN_ROOM_LEN;
}

/**
 * Append `room` to a ws URL as a query param. Ensures a path exists before the
 * query (`wss://host` → `wss://host/?room=…`) — a WebSocket handshake to a bare
 * host with a query but no path is malformed and gets rejected.
 */
export function mergeRoom(url: string, room: string | null | undefined): string {
    if (!room || url.includes("room=")) return url;
    if (url.includes("?")) return `${url}&room=${room}`;
    const afterScheme = url.split("://")[1] ?? url;
    return afterScheme.includes("/") ? `${url}?room=${room}` : `${url}/?room=${room}`;
}

/** Read the stored room, or null if unset/unavailable. */
export async function getRoom(): Promise<string | null> {
    try {
        if (typeof chrome === "undefined" || !chrome.storage?.local?.get) return null;
        const stored = await chrome.storage.local.get(ROOM_STORAGE_KEY);
        const room = stored[ROOM_STORAGE_KEY];
        return typeof room === "string" && room.length > 0 ? room : null;
    } catch {
        return null;
    }
}

/** Persist the tenant room. Rejects weak rooms (must satisfy isValidRoom). */
export async function setRoom(room: string): Promise<boolean> {
    if (!isValidRoom(room)) return false;
    try {
        if (typeof chrome === "undefined" || !chrome.storage?.local?.set) return false;
        await chrome.storage.local.set({ [ROOM_STORAGE_KEY]: room });
        return true;
    } catch {
        return false;
    }
}

/** Generate a strong random room id (UUIDv4) for a new ceremony/tenant. */
export function newRoom(): string {
    if (typeof crypto !== "undefined" && crypto.randomUUID) return crypto.randomUUID();
    // Fallback: 32 hex chars.
    const b = new Uint8Array(16);
    (crypto as Crypto).getRandomValues(b);
    return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

/**
 * Resolve the effective signal-server URL **with the tenant room merged in**
 * (`?room=<id>`). Base URL: chrome.storage override or
 * {@link DEFAULT_SIGNAL_SERVER_URL}. If no room is configured the URL is
 * returned without one — the server then rejects the connection (by design,
 * #31), prompting the user to set a room. Guards against chrome.storage being
 * unavailable by returning the default silently.
 */
export async function getSignalServerUrl(): Promise<string> {
    let base = DEFAULT_SIGNAL_SERVER_URL;
    try {
        if (typeof chrome !== "undefined" && chrome.storage?.local?.get) {
            const stored = await chrome.storage.local.get(SIGNAL_SERVER_STORAGE_KEY);
            const override = stored[SIGNAL_SERVER_STORAGE_KEY];
            if (typeof override === "string" && override.length > 0) base = override;
        }
    } catch {
        // fall through to default
    }
    return mergeRoom(base, await getRoom());
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
