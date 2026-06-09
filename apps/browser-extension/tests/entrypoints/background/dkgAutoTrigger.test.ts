/**
 * Ext-1c regression tests: auto-trigger DKG ceremony on "all
 * participants joined" threshold.
 *
 * The trigger must fire exactly once when a `session_available`
 * broadcast brings `participants.length` up to `total`, for a
 * session we're participating in. This closes the Wave 1 DKG loop:
 * after Ext-1b/1e land the session in appState, this commit kicks
 * off the WebRTC mesh + FROST ceremony automatically.
 *
 * Test invariants:
 * 1. Trigger fires when (our session) AND (we're a participant) AND
 *    (participants.length === total).
 * 2. Trigger does NOT fire for sessions we haven't joined / created.
 * 3. Trigger does NOT fire if we're not a listed participant.
 * 4. Trigger does NOT fire if the threshold isn't met yet.
 * 5. Trigger fires EXACTLY ONCE per session (idempotent under
 *    server re-broadcasts).
 * 6. Dispatched payload has the wire shape the existing offscreen
 *    `sessionAllAccepted` handler expects: `{sessionInfo,
 *    blockchain}` with `accepted_devices` fully populated.
 * 7. Blockchain is derived from `curve_type` (ed25519 → solana,
 *    secp256k1 → ethereum).
 */
import { describe, it, expect, beforeEach, jest } from "bun:test";
import { WebSocketManager } from "../../../src/entrypoints/background/webSocketManager";
import { DkgState } from "@starlab/types/dkg";
import { MeshStatusType } from "@starlab/types/mesh";
import type { AppState } from "@starlab/types/appstate";
import type { SessionInfo } from "@starlab/types/session";

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
        totalParticipants: 3,
        threshold: 2,
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

function inviteReady(
    sessionId: string,
    deviceId: string,
    total = 3,
    threshold = 2,
): SessionInfo {
    // Session with N participants, exactly at `total`. The only
    // transition check the auto-trigger cares about.
    const participants: string[] = [];
    for (let i = 0; i < total; i++) {
        participants.push(i === 0 ? deviceId : `peer-${i}`);
    }
    return {
        session_id: sessionId,
        proposer_id: deviceId,
        total,
        threshold,
        participants,
        session_type: "dkg",
        curve_type: "secp256k1",
        coordination_type: "Network",
        accepted_devices: [],
    };
}

function frame(session: SessionInfo) {
    // Matches the server's `session_available` broadcast shape.
    const {
        accepted_devices: _acc,
        status: _st,
        ...wire
    } = session;
    return {
        type: "session_available" as const,
        session_info: wire,
    };
}

describe("WebSocketManager: DKG auto-trigger on all-joined", () => {
    let env: ReturnType<typeof makeManager>;

    beforeEach(() => {
        env = makeManager({
            deviceId: "ext-bob",
            sessionInfo: null,
        });
    });

    it("fires sessionAllAccepted once when our session reaches full participants", () => {
        // Set our own sessionInfo as the creator of dkg_go, and we
        // haven't seen a full-participants broadcast yet.
        const session = inviteReady("dkg_go", "ext-bob");
        env.appState.sessionInfo = { ...session, participants: ["ext-bob"] };

        // The broadcast comes back with all 3 participants.
        (env.mgr as any).handleWebSocketMessage(frame(session));

        expect(env.sentToOffscreen.length).toBe(1);
        expect(env.sentToOffscreen[0].msg.type).toBe("sessionAllAccepted");
        expect(env.sentToOffscreen[0].msg.sessionInfo.session_id).toBe("dkg_go");
    });

    it("populates accepted_devices with all participants on the dispatched payload", () => {
        // TUI's wire format doesn't carry accepted_devices — the
        // existing offscreen handler uses it for the "all accepted"
        // check. Auto-trigger must fill it in.
        const session = inviteReady("dkg_ac", "ext-bob");
        env.appState.sessionInfo = { ...session, participants: ["ext-bob"] };

        (env.mgr as any).handleWebSocketMessage(frame(session));

        const dispatched = env.sentToOffscreen[0].msg.sessionInfo;
        expect(dispatched.accepted_devices).toEqual(dispatched.participants);
    });

    it("picks blockchain from curve_type", () => {
        // ed25519 → solana
        const sol = inviteReady("dkg_sol", "ext-bob");
        sol.curve_type = "ed25519";
        env.appState.sessionInfo = { ...sol, participants: ["ext-bob"] };
        (env.mgr as any).handleWebSocketMessage(frame(sol));
        expect(env.sentToOffscreen[0].msg.blockchain).toBe("solana");

        // secp256k1 → ethereum (default)
        const env2 = makeManager({ deviceId: "ext-bob" });
        const eth = inviteReady("dkg_eth", "ext-bob");
        env2.appState.sessionInfo = { ...eth, participants: ["ext-bob"] };
        (env2.mgr as any).handleWebSocketMessage(frame(eth));
        expect(env2.sentToOffscreen[0].msg.blockchain).toBe("ethereum");
    });

    it("does NOT trigger when session_id doesn't match our active session", () => {
        // We're in session A; a broadcast for session B arrives at
        // full count. Should NOT trigger — B isn't ours.
        const mine = inviteReady("dkg_mine", "ext-bob");
        env.appState.sessionInfo = {
            ...mine,
            participants: ["ext-bob"],
        };
        const other = inviteReady("dkg_other", "some-other-creator");
        (env.mgr as any).handleWebSocketMessage(frame(other));
        expect(env.sentToOffscreen.length).toBe(0);
    });

    it("does NOT trigger when we're not a listed participant", () => {
        // session_id matches ours but participants list doesn't
        // include our device_id — server-side data corruption or
        // bug; refuse to trigger.
        const session = inviteReady("dkg_no_self", "ext-bob");
        env.appState.sessionInfo = {
            ...session,
            participants: ["ext-bob"],
        };
        const tampered: SessionInfo = {
            ...session,
            participants: ["alice", "bob", "charlie"], // no ext-bob
        };
        (env.mgr as any).handleWebSocketMessage(frame(tampered));
        expect(env.sentToOffscreen.length).toBe(0);
    });

    it("does NOT trigger when participants.length is below total", () => {
        const session = inviteReady("dkg_partial", "ext-bob");
        env.appState.sessionInfo = {
            ...session,
            participants: ["ext-bob"],
        };
        const partial: SessionInfo = {
            ...session,
            total: 3,
            participants: ["ext-bob", "peer-1"], // 2 of 3
        };
        (env.mgr as any).handleWebSocketMessage(frame(partial));
        expect(env.sentToOffscreen.length).toBe(0);
    });

    it("does NOT trigger when we have no active session", () => {
        // Session reaches full count on the wire, but we haven't
        // joined it locally — appState.sessionInfo is null. Could
        // be another session we're watching; no trigger.
        expect(env.appState.sessionInfo).toBeNull();
        const session = inviteReady("dkg_no_local", "other");
        (env.mgr as any).handleWebSocketMessage(frame(session));
        expect(env.sentToOffscreen.length).toBe(0);
    });

    it("is idempotent: a repeated full-count broadcast does not re-trigger", () => {
        // Server re-broadcast on peer reconnect / status refresh
        // shouldn't kick off DKG twice. The offscreen's
        // checkAndTriggerDkg also guards against that via dkgState
        // checks, but dedup here saves the round trip.
        const session = inviteReady("dkg_idem", "ext-bob");
        env.appState.sessionInfo = {
            ...session,
            participants: ["ext-bob"],
        };

        (env.mgr as any).handleWebSocketMessage(frame(session));
        expect(env.sentToOffscreen.length).toBe(1);
        (env.mgr as any).handleWebSocketMessage(frame(session));
        (env.mgr as any).handleWebSocketMessage(frame(session));
        expect(env.sentToOffscreen.length).toBe(1);
    });

    it("fires for a different session after one already triggered", () => {
        // Sequential DKG sessions should each get their own trigger.
        // Idempotence is per-session, not a global latch.
        const sess1 = inviteReady("dkg_first", "ext-bob");
        env.appState.sessionInfo = { ...sess1, participants: ["ext-bob"] };
        (env.mgr as any).handleWebSocketMessage(frame(sess1));
        expect(env.sentToOffscreen.length).toBe(1);

        // User finishes first ceremony + starts another.
        const sess2 = inviteReady("dkg_second", "ext-bob");
        env.appState.sessionInfo = { ...sess2, participants: ["ext-bob"] };
        (env.mgr as any).handleWebSocketMessage(frame(sess2));
        expect(env.sentToOffscreen.length).toBe(2);
        expect(env.sentToOffscreen[1].msg.sessionInfo.session_id).toBe(
            "dkg_second",
        );
    });

    it("wire payload carries threshold/total unchanged for offscreen to consume", () => {
        // Offscreen uses threshold/total for participant-index
        // calculations. These must pass through untouched.
        const session = inviteReady("dkg_n", "ext-bob", 5, 3);
        env.appState.sessionInfo = { ...session, participants: ["ext-bob"] };

        (env.mgr as any).handleWebSocketMessage(frame(session));

        const si = env.sentToOffscreen[0].msg.sessionInfo;
        expect(si.total).toBe(5);
        expect(si.threshold).toBe(3);
    });
});
