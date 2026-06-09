/**
 * Ext-3a regression tests: push notifications for incoming signing
 * invites.
 *
 * The gate conditions must be strict — false positives spam the
 * desktop notification tray for every session update; false negatives
 * defeat the whole point of the feature.
 *
 * Gate (matches TUI's Stage 1 auto-modal behavior, a4c52ca):
 *   - session_type === "signing"
 *   - our deviceId IS a participant
 *   - our deviceId is NOT the proposer
 *   - dedup per session_id (no re-fire on re-broadcasts)
 *
 * DKG sessions must NEVER trigger this — they use the wallet-status
 * banner, and a flood of DKG notifications would desensitize users.
 */
import { describe, it, expect, beforeEach, jest } from "bun:test";
import {
    SigningNotifier,
    buildMessagePreview,
} from "../../../src/entrypoints/background/signingNotification";
import type { SessionInfo } from "@starlab/types/session";

function makeNotifier() {
    const calls: Array<{ id: string; options: any }> = [];
    const create = jest.fn(
        (id: string, options: any, _cb?: (id: string) => void) => {
            calls.push({ id, options });
        },
    );
    const notifier = new SigningNotifier({
        notifications: { create } as any,
        log: () => {},
    });
    return { notifier, create, calls };
}

function signingSession(overrides: Partial<SessionInfo> = {}): SessionInfo {
    return {
        session_id: "sign_abc",
        proposer_id: "alice",
        total: 3,
        threshold: 2,
        participants: ["alice", "ext-bob", "carol"],
        session_type: "signing",
        curve_type: "secp256k1",
        coordination_type: "Network",
        accepted_devices: [],
        wallet_name: "Treasury",
        group_public_key: "02cafe",
        blockchain: "ethereum",
        signing_message_hex: "deadbeef",
        ...overrides,
    };
}

describe("SigningNotifier.maybeNotify", () => {
    let env: ReturnType<typeof makeNotifier>;
    beforeEach(() => {
        env = makeNotifier();
    });

    it("fires a notification for signing invites we're a participant in", () => {
        const fired = env.notifier.maybeNotify(signingSession(), "ext-bob");
        expect(fired).toBe(true);
        expect(env.calls.length).toBe(1);
        expect(env.calls[0].id).toBe("mpc-signing-req:sign_abc");
        expect(env.calls[0].options.type).toBe("basic");
        expect(env.calls[0].options.title).toContain("Treasury");
    });

    it("includes proposer id + threshold in the notification body", () => {
        env.notifier.maybeNotify(signingSession(), "ext-bob");
        const body = env.calls[0].options.message as string;
        expect(body).toContain("alice"); // proposer
        expect(body).toContain("2 of 3"); // threshold
    });

    it("does NOT fire for DKG sessions", () => {
        const dkg = signingSession({
            session_id: "dkg_x",
            session_type: "dkg",
            wallet_name: undefined,
            group_public_key: undefined,
            signing_message_hex: undefined,
        });
        const fired = env.notifier.maybeNotify(dkg, "ext-bob");
        expect(fired).toBe(false);
        expect(env.calls.length).toBe(0);
    });

    it("does NOT fire when we're the proposer (no self-notification)", () => {
        const fired = env.notifier.maybeNotify(
            signingSession({ proposer_id: "ext-bob" }),
            "ext-bob",
        );
        expect(fired).toBe(false);
        expect(env.calls.length).toBe(0);
    });

    it("does NOT fire when we're not a participant", () => {
        const fired = env.notifier.maybeNotify(
            signingSession({ participants: ["alice", "carol", "dave"] }),
            "ext-bob",
        );
        expect(fired).toBe(false);
        expect(env.calls.length).toBe(0);
    });

    it("does NOT fire when our deviceId is unset", () => {
        const fired = env.notifier.maybeNotify(signingSession(), undefined);
        expect(fired).toBe(false);
        expect(env.calls.length).toBe(0);
    });

    it("is idempotent: a second update on the same session does not re-fire", () => {
        env.notifier.maybeNotify(signingSession(), "ext-bob");
        env.notifier.maybeNotify(signingSession(), "ext-bob");
        env.notifier.maybeNotify(signingSession(), "ext-bob");
        expect(env.calls.length).toBe(1);
    });

    it("fires independently for a different session id", () => {
        env.notifier.maybeNotify(signingSession({ session_id: "s1" }), "ext-bob");
        env.notifier.maybeNotify(signingSession({ session_id: "s2" }), "ext-bob");
        expect(env.calls.length).toBe(2);
        expect(env.calls[0].id).toBe("mpc-signing-req:s1");
        expect(env.calls[1].id).toBe("mpc-signing-req:s2");
    });

    it("falls back to threshold title when wallet_name is missing", () => {
        env.notifier.maybeNotify(
            signingSession({ wallet_name: undefined }),
            "ext-bob",
        );
        expect(env.calls[0].options.title).toContain("2/3");
    });

    it("title format includes wallet name when present", () => {
        env.notifier.maybeNotify(
            signingSession({ wallet_name: "Main Wallet" }),
            "ext-bob",
        );
        expect(env.calls[0].options.title).toContain("Main Wallet");
    });
});

describe("buildMessagePreview", () => {
    it("returns placeholder when hex is missing", () => {
        expect(buildMessagePreview(undefined, "ethereum")).toBe("(no message)");
    });

    it("returns hex preview for ethereum sessions", () => {
        // EIP-191 hash — always binary, never decode.
        const hex = "a".repeat(64);
        const preview = buildMessagePreview(hex, "ethereum");
        expect(preview.startsWith("0x")).toBe(true);
    });

    it("strips leading 0x if present", () => {
        const hex = "0x" + "b".repeat(10);
        const preview = buildMessagePreview(hex, "ethereum");
        // Preview re-prefixes 0x exactly once.
        expect(preview.startsWith("0xbb")).toBe(true);
        expect(preview.startsWith("0x0x")).toBe(false);
    });

    it("decodes printable UTF-8 for solana sessions", () => {
        // "hi" = 0x6869
        const preview = buildMessagePreview("6869", "solana");
        expect(preview).toBe("hi");
    });

    it("falls back to hex on binary payloads for solana", () => {
        // 0x00 is non-printable → hex fallback.
        const preview = buildMessagePreview("0001020304", "solana");
        expect(preview.startsWith("0x")).toBe(true);
    });

    it("truncates long messages", () => {
        const long = "a".repeat(200);
        const preview = buildMessagePreview(long, "ethereum");
        expect(preview.length).toBeLessThanOrEqual(61); // 60 + 1 for ellipsis
        expect(preview.endsWith("…")).toBe(true);
    });
});
