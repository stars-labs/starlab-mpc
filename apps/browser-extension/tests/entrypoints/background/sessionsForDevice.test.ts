/**
 * Regression tests for Ext-1a+ cold-start session replay.
 *
 * Two paths deliver pre-existing sessions to a freshly-connected
 * client:
 *   1. `request_active_sessions` → server replies with N individual
 *      `session_available` frames (Cloudflare Worker path, line 301:
 *      `ServerMsg::SessionAvailable { session_info }`). Each frame
 *      already goes through handleSessionAvailable.
 *   2. `query_my_active_sessions` → server replies with a bulk
 *      `sessions_for_device` payload (line 374: `ServerMsg::
 *      SessionsForDevice { sessions }`). This test locks down that
 *      the extension routes each entry of the bulk payload through
 *      the same handleSessionAvailable merge path.
 *
 * Without this routing, a device that reconnects mid-ceremony would
 * get the bulk reply but never populate `appState.invites`, so the
 * popup would show an empty Join Session tab even though sessions
 * exist.
 */
import { describe, it, expect, beforeEach, jest } from "bun:test";
import { WebSocketManager } from "../../../src/entrypoints/background/webSocketManager";
import { DkgState } from "@starlab/types/dkg";
import { MeshStatusType } from "@starlab/types/mesh";
import type { AppState } from "@starlab/types/appstate";

function makeAppState(): AppState {
    return {
        deviceId: "ext-device",
        connecteddevices: [],
        wsConnected: true,
        sessionInfo: null,
        invites: [],
        meshStatus: { type: MeshStatusType.Incomplete },
        dkgState: DkgState.Idle,
        webrtcConnections: {},
        blockchain: "ethereum",
        totalParticipants: 3,
        threshold: 2,
    };
}

function makeManager() {
    const appState = makeAppState();
    const broadcast = jest.fn();
    const sendToOffscreen = jest.fn(async () => ({ success: true }));
    const sessionManager: any = {};
    const stateManager: any = {
        updateInvites: jest.fn((invites: any[]) => {
            appState.invites = invites;
        }),
    };
    const mgr = new WebSocketManager(
        appState,
        sessionManager,
        broadcast as any,
        sessionManager.sendToOffscreen ?? sendToOffscreen,
        stateManager,
    );
    return { mgr, appState, broadcast, stateManager };
}

describe("WebSocketManager — sessions_for_device bulk reply routing", () => {
    let env: ReturnType<typeof makeManager>;

    beforeEach(() => {
        env = makeManager();
    });

    it("routes each entry of the sessions_for_device payload as an individual session_available", () => {
        const frame = {
            type: "sessions_for_device",
            sessions: [
                {
                    session_id: "dkg_a",
                    proposer_id: "tui-1",
                    total: 3,
                    threshold: 2,
                    participants: ["tui-1"],
                    session_type: "dkg",
                    curve_type: "secp256k1",
                    coordination_type: "Network",
                },
                {
                    session_id: "sig_b",
                    proposer_id: "tui-2",
                    total: 3,
                    threshold: 2,
                    participants: ["tui-2", "tui-3"],
                    session_type: "signing",
                    curve_type: "secp256k1",
                    coordination_type: "Network",
                    wallet_name: "w",
                    group_public_key: "02aabb",
                    blockchain: "ethereum",
                    signing_message_hex: "deadbeef",
                },
            ],
        };
        (env.mgr as any).handleWebSocketMessage(frame);
        // Both sessions land on appState.invites, in order.
        expect(env.appState.invites.length).toBe(2);
        expect(env.appState.invites[0].session_id).toBe("dkg_a");
        expect(env.appState.invites[1].session_id).toBe("sig_b");
        // Synthesised accepted_devices for both.
        expect(env.appState.invites[0].accepted_devices).toEqual([]);
        expect(env.appState.invites[1].accepted_devices).toEqual([]);
        // Signing-specific fields survived the route-through.
        expect(env.appState.invites[1].signing_message_hex).toBe("deadbeef");
    });

    it("handles empty sessions_for_device gracefully", () => {
        (env.mgr as any).handleWebSocketMessage({
            type: "sessions_for_device",
            sessions: [],
        });
        expect(env.appState.invites.length).toBe(0);
    });

    it("handles malformed sessions_for_device without crashing", () => {
        expect(() =>
            (env.mgr as any).handleWebSocketMessage({
                type: "sessions_for_device",
                // Not an array.
                sessions: "not an array",
            }),
        ).not.toThrow();
        expect(env.appState.invites.length).toBe(0);
    });

    it("silently drops malformed entries while keeping well-formed ones", () => {
        (env.mgr as any).handleWebSocketMessage({
            type: "sessions_for_device",
            sessions: [
                // Missing session_id — should drop.
                { total: 3, threshold: 2 },
                // Valid.
                {
                    session_id: "dkg_good",
                    total: 3,
                    threshold: 2,
                    participants: [],
                },
                // Bad types — drop.
                "string",
                null,
                42,
            ],
        });
        expect(env.appState.invites.length).toBe(1);
        expect(env.appState.invites[0].session_id).toBe("dkg_good");
    });

    it("merges follow-up session_available over a bulk-delivered entry", () => {
        // First: cold-start bulk reply with 1 session in initial state.
        (env.mgr as any).handleWebSocketMessage({
            type: "sessions_for_device",
            sessions: [
                {
                    session_id: "dkg_merge",
                    proposer_id: "tui-1",
                    total: 3,
                    threshold: 2,
                    participants: ["tui-1"],
                    session_type: "dkg",
                    curve_type: "secp256k1",
                    coordination_type: "Network",
                },
            ],
        });
        expect(env.appState.invites[0].participants.length).toBe(1);

        // Then: a live session_available update with 2 participants.
        (env.mgr as any).handleWebSocketMessage({
            type: "session_available",
            session_info: {
                session_id: "dkg_merge",
                proposer_id: "tui-1",
                total: 3,
                threshold: 2,
                participants: ["tui-1", "tui-2"],
                session_type: "dkg",
                curve_type: "secp256k1",
                coordination_type: "Network",
            },
        });
        // Should have replaced — not appended — so we still have
        // exactly one entry for dkg_merge, now with 2 participants.
        expect(env.appState.invites.length).toBe(1);
        expect(env.appState.invites[0].participants).toEqual([
            "tui-1",
            "tui-2",
        ]);
    });
});
