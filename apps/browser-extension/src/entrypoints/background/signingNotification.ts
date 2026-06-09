/**
 * Ext-3a: Push notifications for incoming signing requests.
 *
 * TUI Stage 1 parity (a4c52ca). If a co-signer is idle in another
 * tab or the browser is backgrounded, a silent `session_available`
 * broadcast alone won't reach them — the service worker logs it
 * and the popup's Join Session tab doesn't surface unless it's
 * already open. For signing sessions specifically, the creator is
 * blocked waiting for threshold acceptances; invisible invites
 * guarantee timeouts.
 *
 * Fire a `chrome.notifications.create` for signing invites where
 *   - session_type === "signing"
 *   - our deviceId IS in participants
 *   - our deviceId is NOT the proposer (no self-notification)
 *   - we haven't already notified for this session_id (dedup)
 *
 * DKG sessions intentionally skip notifications — they're less
 * time-sensitive (the creator is expected to wait for all N) and
 * the existing UI banner is sufficient.
 *
 * The notification click handler focuses the popup; WXT's
 * action.default_popup ensures the popup itself routes to the
 * signing invite via appState.invites.
 */

import type { SessionInfo } from "@starlab/types/session";

const NOTIFICATION_ID_PREFIX = "mpc-signing-req:";

export interface SigningNotifierDeps {
    /**
     * Injected for testability. Prod passes `chrome.notifications`
     * directly. Tests pass a stub that records calls.
     */
    notifications: {
        create: (
            notificationId: string,
            options: chrome.notifications.NotificationOptions,
            callback?: (id: string) => void,
        ) => void;
    };
    /** Injected for testability. Defaults to console.log. */
    log?: (message: string) => void;
}

export class SigningNotifier {
    private firedFor = new Set<string>();
    private notifications: SigningNotifierDeps["notifications"];
    private log: (message: string) => void;

    constructor(deps: SigningNotifierDeps) {
        this.notifications = deps.notifications;
        this.log = deps.log ?? ((m) => console.log("[SigningNotifier]", m));
    }

    /**
     * Called by WebSocketManager on every session_available update
     * (the same place invites are merged into appState). Evaluates
     * gate conditions and fires a browser notification if applicable.
     * Idempotent per session_id — later status updates on the same
     * session won't re-notify.
     */
    maybeNotify(session: SessionInfo, ourDeviceId: string | undefined): boolean {
        if (!ourDeviceId) return false;
        if (session.session_type !== "signing") return false;
        if (!session.participants.includes(ourDeviceId)) return false;
        if (session.proposer_id === ourDeviceId) return false;
        if (this.firedFor.has(session.session_id)) return false;

        this.firedFor.add(session.session_id);

        const title = session.wallet_name
            ? `Signing request: ${session.wallet_name}`
            : `Signing request (${session.threshold}/${session.total})`;
        const preview = buildMessagePreview(
            session.signing_message_hex,
            session.blockchain,
        );
        const body =
            `From: ${session.proposer_id}\n` +
            `Threshold: ${session.threshold} of ${session.total}\n` +
            `Message: ${preview}`;

        this.log(
            `Firing notification for ${session.session_id} (wallet=${session.wallet_name ?? "?"}, proposer=${session.proposer_id})`,
        );

        this.notifications.create(
            `${NOTIFICATION_ID_PREFIX}${session.session_id}`,
            {
                type: "basic",
                iconUrl: "icon/128.png",
                title,
                message: body,
                priority: 2,
                requireInteraction: true,
            },
        );
        return true;
    }

    /** Test seam: reset dedup state (prod doesn't call this). */
    reset(): void {
        this.firedFor.clear();
    }
}

/**
 * Turn a hex-encoded signing payload into a human-readable preview.
 * For Ethereum (EIP-191) the hex is a 32-byte keccak256 hash — we
 * show it truncated. For Solana (ed25519) the hex is the raw UTF-8
 * bytes; try to decode and fall back to hex if non-printable.
 *
 * Truncated to 60 chars so the notification body stays within the
 * ~150-char visual budget chrome.notifications gives us on most
 * desktop OSes.
 */
export function buildMessagePreview(
    hex: string | undefined,
    blockchain: string | undefined,
): string {
    if (!hex) return "(no message)";
    const clean = hex.startsWith("0x") ? hex.slice(2) : hex;

    // Solana ed25519 encodes raw UTF-8 — try decoding.
    if (blockchain === "solana") {
        try {
            const bytes = hexToBytes(clean);
            const decoded = new TextDecoder("utf-8", { fatal: true }).decode(
                bytes,
            );
            if (isPrintable(decoded)) return truncate(decoded, 60);
        } catch {
            /* fall through to hex display */
        }
    }

    // Ethereum / EIP-191: show first + last bytes of the hash.
    return truncate(`0x${clean}`, 60);
}

function hexToBytes(clean: string): Uint8Array {
    if (clean.length % 2 !== 0) throw new Error("odd-length hex");
    const out = new Uint8Array(clean.length / 2);
    for (let i = 0; i < clean.length; i += 2) {
        const byte = parseInt(clean.slice(i, i + 2), 16);
        if (Number.isNaN(byte)) throw new Error("non-hex char");
        out[i / 2] = byte;
    }
    return out;
}

function isPrintable(s: string): boolean {
    // ASCII printable + common whitespace (tab, LF). Reject nulls
    // and most control chars — those are the signal that this
    // "message" is actually binary and should be shown as hex.
    for (const ch of s) {
        const code = ch.codePointAt(0) ?? 0;
        if (code === 0) return false;
        if (code < 0x20 && code !== 0x09 && code !== 0x0a && code !== 0x0d)
            return false;
    }
    return true;
}

function truncate(s: string, n: number): string {
    if (s.length <= n) return s;
    return `${s.slice(0, n - 1)}…`;
}
