/**
 * Ext-2d regression tests: auto-trigger signing ceremony on
 * threshold-participants-joined.
 *
 * Signing differs from DKG in its "ready" condition:
 *   - DKG needs all N participants (N-of-N) — every shareholder
 *     MUST contribute to key generation.
 *   - Signing only needs `threshold` signers — FROST produces a
 *     valid signature from any threshold-of-N subset. Waiting for
 *     all N would stall whenever a non-signing keyholder is offline.
 *
 * The trigger must fire EXACTLY ONCE when:
 *   - The session is ours (session_id matches appState.sessionInfo).
 *   - We are in `participants`.
 *   - `session_type === "signing"`.
 *   - `participants.length >= threshold`.
 *
 * And it must dispatch `sessionReadyForSigning` (NOT
 * `sessionAllAccepted` — that's the DKG path; confusing them would
 * trigger offscreen to start DKG round 1 over an existing wallet's
 * key share and corrupt it).
 *
 * Test invariants mirror the DKG suite, flipped for signing semantics.
 */
import { describe, it, expect, beforeEach, jest } from "bun:test";
import { WebSocketManager } from "../../../src/entrypoints/background/webSocketManager";
import { DkgState } from "@mpc-wallet/types/dkg";
import { MeshStatusType } from "@mpc-wallet/types/mesh";
import type { AppState } from "@mpc-wallet/types/appstate";
import type { SessionInfo } from "@mpc-wallet/types/session";

function makeManager(opts?: {
    deviceId?: string;
    sessionInfo?: SessionInfo | null;
}) {
    const appState: AppState = {
        deviceId: opts?.deviceId ?? "ext-bob",
        connecteddevices: [],
        wsConnected: true,
        sessionInfo: opts?.sessionInfo ?? null,
        invites: [],
        meshStatus: { type: MeshStatusType.Incomplete },
        dkgState: DkgState.Idle,
        webrtcConnections: {},
        blockchain: "ethereum",
    };
    const broadcast = jest.fn();
    const sentToOffscreen: Array<{ msg: any; desc: string }> = [];
    const sendToOffscreen = jest.fn(async (msg: any, desc: string) => {
        sentToOffscreen.push({ msg, desc });
        return { success: true };
    });
    const stateManager = {
        updateInvites: jest.fn((inv: any[]) => {
            appState.invites = inv;
        }),
    };
    const mgr = new WebSocketManager(
        appState,
        {} as any,
        broadcast as any,
        sendToOffscreen as any,
        stateManager as any,
    );
    return { mgr, appState, sentToOffscreen };
}

function signingSession(
    sessionId: string,
    deviceId: string,
    participants: string[],
    threshold = 2,
    total = 3,
    curve: "secp256k1" | "ed25519" = "secp256k1",
): SessionInfo {
    return {
        session_id: sessionId,
        proposer_id: deviceId,
        total,
        threshold,
        participants,
        session_type: "signing",
        curve_type: curve,
        coordination_type: "Network",
        accepted_devices: [],
        wallet_name: "Test",
        group_public_key: "02cafe",
        blockchain: curve === "ed25519" ? "solana" : "ethereum",
        signing_message_hex: "deadbeef",
    };
}

function frame(session: SessionInfo) {
    const { accepted_devices: _acc, status: _st, ...wire } = session;
    return {
        type: "session_available" as const,
        session_info: wire,
    };
}

describe("WebSocketManager: signing auto-trigger at threshold", () => {
    let env: ReturnType<typeof makeManager>;

    beforeEach(() => {
        env = makeManager({ deviceId: "ext-bob", sessionInfo: null });
    });

    it("fires sessionReadyForSigning once when participants reach threshold", () => {
        // threshold=2, total=3. As soon as 2 of 3 have joined,
        // the ceremony can start — we don't wait for the 3rd.
        const seed = signingSession("sign_go", "ext-bob", ["ext-bob"], 2, 3);
        env.appState.sessionInfo = seed;

        // Second joiner — participants goes 1 → 2, meets threshold.
        const reached = signingSession(
            "sign_go",
            "ext-bob",
            ["ext-bob", "peer-1"],
            2,
            3,
        );
        (env.mgr as any).handleWebSocketMessage(frame(reached));

        expect(env.sentToOffscreen.length).toBe(1);
        expect(env.sentToOffscreen[0].msg.type).toBe("sessionReadyForSigning");
        expect(env.sentToOffscreen[0].msg.sessionInfo.session_id).toBe(
            "sign_go",
        );
    });

    it("does NOT fire sessionAllAccepted for signing sessions", () => {
        // Critical: signing sessions must NOT accidentally hit the
        // DKG path — that would try to start DKG round 1 on an
        // existing wallet and corrupt the keystore.
        const seed = signingSession("sign_notdkg", "ext-bob", ["ext-bob"]);
        env.appState.sessionInfo = seed;
        const reached = signingSession("sign_notdkg", "ext-bob", [
            "ext-bob",
            "peer-1",
        ]);
        (env.mgr as any).handleWebSocketMessage(frame(reached));

        const types = env.sentToOffscreen.map((s) => s.msg.type);
        expect(types).not.toContain("sessionAllAccepted");
        expect(types).toContain("sessionReadyForSigning");
    });

    it("populates accepted_devices with all participants on signing payload", () => {
        // Offscreen's signing handler will use participants for
        // the signing set. accepted_devices must be filled from
        // participants (wire doesn't carry it).
        const seed = signingSession("sign_ac", "ext-bob", ["ext-bob"]);
        env.appState.sessionInfo = seed;
        const reached = signingSession("sign_ac", "ext-bob", [
            "ext-bob",
            "peer-1",
        ]);

        (env.mgr as any).handleWebSocketMessage(frame(reached));

        const dispatched = env.sentToOffscreen[0].msg.sessionInfo;
        expect(dispatched.accepted_devices).toEqual(dispatched.participants);
    });

    it("picks blockchain from curve_type for signing", () => {
        const seed = signingSession(
            "sign_sol",
            "ext-bob",
            ["ext-bob"],
            2,
            3,
            "ed25519",
        );
        env.appState.sessionInfo = seed;
        const reached = signingSession(
            "sign_sol",
            "ext-bob",
            ["ext-bob", "peer-1"],
            2,
            3,
            "ed25519",
        );
        (env.mgr as any).handleWebSocketMessage(frame(reached));

        expect(env.sentToOffscreen[0].msg.blockchain).toBe("solana");
    });

    it("does NOT trigger below threshold", () => {
        // Signing needs AT LEAST threshold participants. A 1-of-2
        // pre-threshold update should not start the ceremony.
        const seed = signingSession(
            "sign_partial",
            "ext-bob",
            ["ext-bob"],
            2,
            3,
        );
        env.appState.sessionInfo = seed;
        // Still only 1 participant (ourself) — below threshold=2.
        (env.mgr as any).handleWebSocketMessage(frame(seed));
        expect(env.sentToOffscreen.length).toBe(0);
    });

    it("triggers when participants exceed threshold (not equal)", () => {
        // threshold=2, all 3 joined. Should still fire (once) —
        // "at least threshold" is the gate, not "exactly threshold".
        const seed = signingSession("sign_full", "ext-bob", ["ext-bob"]);
        env.appState.sessionInfo = seed;
        const full = signingSession("sign_full", "ext-bob", [
            "ext-bob",
            "peer-1",
            "peer-2",
        ]);
        (env.mgr as any).handleWebSocketMessage(frame(full));
        expect(env.sentToOffscreen.length).toBe(1);
        expect(env.sentToOffscreen[0].msg.type).toBe("sessionReadyForSigning");
    });

    it("is idempotent: subsequent threshold-met broadcasts don't re-trigger", () => {
        // Once threshold is met, a follow-up broadcast (e.g. the
        // 3rd participant joining on top of the already-met 2)
        // should NOT re-fire the ceremony trigger.
        const seed = signingSession("sign_idem", "ext-bob", ["ext-bob"]);
        env.appState.sessionInfo = seed;

        const two = signingSession("sign_idem", "ext-bob", [
            "ext-bob",
            "peer-1",
        ]);
        (env.mgr as any).handleWebSocketMessage(frame(two));
        expect(env.sentToOffscreen.length).toBe(1);

        // A third joiner arrives after trigger fired — no re-fire.
        const three = signingSession("sign_idem", "ext-bob", [
            "ext-bob",
            "peer-1",
            "peer-2",
        ]);
        (env.mgr as any).handleWebSocketMessage(frame(three));
        expect(env.sentToOffscreen.length).toBe(1);
    });

    it("does NOT trigger when we're not a participant", () => {
        const seed = signingSession(
            "sign_no_self",
            "ext-bob",
            ["ext-bob"],
            2,
            3,
        );
        env.appState.sessionInfo = seed;
        // Tampered: our device_id got swapped out.
        const tampered = signingSession("sign_no_self", "alice", [
            "alice",
            "carol",
        ]);
        (env.mgr as any).handleWebSocketMessage(frame(tampered));
        expect(env.sentToOffscreen.length).toBe(0);
    });

    it("does NOT trigger when session_id is not ours", () => {
        const mine = signingSession("sign_mine", "ext-bob", ["ext-bob"]);
        env.appState.sessionInfo = mine;
        const other = signingSession("sign_other", "ext-bob", [
            "ext-bob",
            "peer-1",
        ]);
        (env.mgr as any).handleWebSocketMessage(frame(other));
        expect(env.sentToOffscreen.length).toBe(0);
    });

    it("DKG sessions still require all N (signing gate doesn't leak over)", () => {
        // Regression: making sure signing's threshold gate didn't
        // accidentally become the DKG gate. DKG still waits for
        // total=3 exactly.
        const seed: SessionInfo = {
            session_id: "dkg_mixed",
            proposer_id: "ext-bob",
            total: 3,
            threshold: 2,
            participants: ["ext-bob"],
            session_type: "dkg",
            curve_type: "secp256k1",
            coordination_type: "Network",
            accepted_devices: [],
        };
        env.appState.sessionInfo = seed;
        const halfway: SessionInfo = {
            ...seed,
            participants: ["ext-bob", "peer-1"], // 2 of 3 — meets signing's
            // threshold but not DKG's total.
        };
        (env.mgr as any).handleWebSocketMessage(frame(halfway));
        expect(env.sentToOffscreen.length).toBe(0);
    });
});
