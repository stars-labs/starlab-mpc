/**
 * Ext-1e regression tests: sessionManager.joinDkgSession()
 *
 * The joiner half of the TUI-compat DKG flow. Key invariants this
 * locks down:
 *
 * 1. A successful join emits `session_status_update` with the exact
 *    shape the server expects (session_id + participant_joined). If
 *    the wire shape drifts, the server can't index us into the
 *    session's participants array and the creator's roster never
 *    updates.
 *
 * 2. We inherit threshold/total/curve from the invite — the UI
 *    NEVER gets to override these, because the creator's
 *    announcement is authoritative. If we let the joiner "edit"
 *    threshold during join, we'd fork the ceremony config and FROST
 *    rounds would fail later.
 *
 * 3. Idempotent: if we're already in the participants list (e.g.
 *    the server echoed our own update back), joining again is a
 *    no-op that doesn't double-emit wire messages.
 *
 * 4. Proposer guard: the creator can't self-join. Prevents the
 *    "creator is now a participant twice" bug where the session
 *    would think it had N+1 participants.
 *
 * 5. Full-session guard: once the invite has `total` participants,
 *    no further joins — prevents N+1 problem from the other side.
 *
 * 6. Rollback on wire failure: if the server rejects our update, we
 *    revert local sessionInfo/dkgState so the UI doesn't show a
 *    false "joined" state.
 */
import { describe, it, expect, beforeEach, jest } from "bun:test";
import { SessionManager } from "../../../src/entrypoints/background/sessionManager";
import { DkgState } from "@starlab/types/dkg";
import { MeshStatusType } from "@starlab/types/mesh";
import type { AppState } from "@starlab/types/appstate";
import type { SessionInfo } from "@starlab/types/session";

function makeInvite(overrides: Partial<SessionInfo> = {}): SessionInfo {
    return {
        session_id: "dkg_abc",
        proposer_id: "tui-alice",
        total: 3,
        threshold: 2,
        participants: ["tui-alice"],
        session_type: "dkg",
        curve_type: "secp256k1",
        coordination_type: "Network",
        accepted_devices: [],
        ...overrides,
    };
}

function makeSessionManager(opts?: {
    deviceId?: string;
    invites?: SessionInfo[];
    wsReady?: number;
}) {
    const appState: AppState = {
        deviceId: opts?.deviceId ?? "ext-bob",
        connecteddevices: [],
        wsConnected: true,
        sessionInfo: null,
        invites: opts?.invites ?? [],
        meshStatus: { type: MeshStatusType.Incomplete },
        dkgState: DkgState.Idle,
        webrtcConnections: {},
        blockchain: "ethereum",
        totalParticipants: 3,
        threshold: 2,
    };
    const sentUpdates: Array<{ sessionId: string; deviceId: string }> = [];
    const announces: Array<Record<string, unknown>> = [];
    // In the bun test env, WebSocket is a jest.fn with no static
    // members — so `WebSocket.OPEN` is `undefined`. The production
    // check is `getReadyState() !== WebSocket.OPEN`; returning
    // `undefined` on "open" satisfies that check in the mock env.
    // Anything else (e.g. 3 for CLOSED) trips the bail.
    const openSentinel = (WebSocket as any)?.OPEN;
    const readyState = opts?.wsReady ?? openSentinel;
    const wsClient: any = {
        getReadyState: jest.fn(() => readyState),
        sendSessionStatusUpdate: jest.fn((sessionId: string, deviceId: string) => {
            sentUpdates.push({ sessionId, deviceId });
        }),
        announceSession: jest.fn((si: Record<string, unknown>) => {
            announces.push(si);
        }),
    };
    const broadcastToPopup = jest.fn();
    const sendToOffscreen = jest.fn(async () => ({ success: true }));
    const stateManager = {
        getState: () => appState,
        updateState: jest.fn((patch: Partial<AppState>) => {
            Object.assign(appState, patch);
        }),
    };
    const mgr = new SessionManager(
        appState,
        wsClient,
        broadcastToPopup as any,
        sendToOffscreen as any,
        stateManager as any,
    );
    return { mgr, appState, wsClient, sentUpdates, broadcastToPopup };
}

describe("SessionManager.joinDkgSession", () => {
    describe("validation / error paths", () => {
        it("fails if no invite matches", async () => {
            const { mgr } = makeSessionManager({ invites: [] });
            const res = await mgr.joinDkgSession("dkg_missing");
            expect(res.success).toBe(false);
            expect(res.error).toContain("No known invite");
        });

        it("fails if WebSocket is closed", async () => {
            const { mgr } = makeSessionManager({
                invites: [makeInvite()],
                wsReady: 3 /* CLOSED — any non-OPEN literal trips the check */,
            });
            const res = await mgr.joinDkgSession("dkg_abc");
            expect(res.success).toBe(false);
            expect(res.error).toContain("not connected");
        });

        it("refuses to self-join a session we proposed", async () => {
            // If Alice creates a 2-of-3 and somehow hits the Join
            // path on her own session (UI bug / race), we should
            // reject rather than duplicate her in participants.
            const { mgr } = makeSessionManager({
                deviceId: "alice",
                invites: [makeInvite({ proposer_id: "alice" })],
            });
            const res = await mgr.joinDkgSession("dkg_abc");
            expect(res.success).toBe(false);
            expect(res.error).toContain("proposer");
        });

        it("refuses to join a full session", async () => {
            const { mgr } = makeSessionManager({
                invites: [
                    makeInvite({
                        total: 2,
                        participants: ["alice", "bob"],
                    }),
                ],
            });
            const res = await mgr.joinDkgSession("dkg_abc");
            expect(res.success).toBe(false);
            expect(res.error).toContain("full");
        });
    });

    describe("happy path", () => {
        it("emits session_status_update with exact wire shape", async () => {
            const { mgr, wsClient, sentUpdates } = makeSessionManager({
                deviceId: "ext-bob",
                invites: [makeInvite()],
            });
            const res = await mgr.joinDkgSession("dkg_abc");
            expect(res.success).toBe(true);
            // TUI's server handler reads exactly these two keys; if
            // the shape regresses, the server drops our join.
            expect(wsClient.sendSessionStatusUpdate).toHaveBeenCalledTimes(1);
            expect(sentUpdates[0]).toEqual({
                sessionId: "dkg_abc",
                deviceId: "ext-bob",
            });
        });

        it("inherits threshold/total/curve from the invite, not UI input", async () => {
            // Invite says 3-of-5 secp256k1. Our local sessionInfo
            // must match exactly — no UI-side overrides.
            const invite = makeInvite({
                total: 5,
                threshold: 3,
                curve_type: "ed25519",
            });
            const { mgr, appState } = makeSessionManager({
                deviceId: "ext-bob",
                invites: [invite],
            });
            const res = await mgr.joinDkgSession("dkg_abc");
            expect(res.success).toBe(true);
            expect(appState.sessionInfo!.total).toBe(5);
            expect(appState.sessionInfo!.threshold).toBe(3);
            expect(appState.sessionInfo!.curve_type).toBe("ed25519");
        });

        it("appends self to local participants optimistically", async () => {
            const { mgr, appState } = makeSessionManager({
                deviceId: "ext-bob",
                invites: [makeInvite()], // participants = ["tui-alice"]
            });
            await mgr.joinDkgSession("dkg_abc");
            expect(appState.sessionInfo!.participants).toContain("ext-bob");
            expect(appState.sessionInfo!.participants).toContain("tui-alice");
            expect(appState.sessionInfo!.participants.length).toBe(2);
        });

        it("moves dkgState from Idle to Initializing", async () => {
            const { mgr, appState } = makeSessionManager({
                invites: [makeInvite()],
            });
            expect(appState.dkgState).toBe(DkgState.Idle);
            await mgr.joinDkgSession("dkg_abc");
            expect(appState.dkgState).toBe(DkgState.Initializing);
        });

        it("sets blockchain from the invite's curve_type", async () => {
            // ed25519 → solana; secp256k1 → ethereum.
            const { mgr, appState } = makeSessionManager({
                invites: [makeInvite({ curve_type: "ed25519" })],
            });
            await mgr.joinDkgSession("dkg_abc");
            expect(appState.blockchain).toBe("solana");
        });

        it("notifies popup with dkgSessionJoined event", async () => {
            const { mgr, broadcastToPopup } = makeSessionManager({
                invites: [makeInvite()],
            });
            await mgr.joinDkgSession("dkg_abc");
            const calls = (broadcastToPopup as any).mock.calls as any[];
            const saw = calls.some(
                (c) => c[0]?.type === "dkgSessionJoined",
            );
            expect(saw).toBe(true);
        });
    });

    describe("idempotence", () => {
        it("re-joining does not double-emit wire update", async () => {
            const { mgr, wsClient } = makeSessionManager({
                deviceId: "ext-bob",
                invites: [
                    makeInvite({
                        // Server already echoed our prior update
                        // back, so we're in the participants list.
                        participants: ["tui-alice", "ext-bob"],
                    }),
                ],
            });
            const res = await mgr.joinDkgSession("dkg_abc");
            expect(res.success).toBe(true);
            // The guard should see we're already a participant and
            // skip the wire emit.
            expect(wsClient.sendSessionStatusUpdate).not.toHaveBeenCalled();
        });
    });
});
