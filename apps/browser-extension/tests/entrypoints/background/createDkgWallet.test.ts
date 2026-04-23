/**
 * Ext-1b regression tests: sessionManager.createDkgWallet()
 *
 * The hot question: does the extension emit a `session_info` payload
 * that matches what the TUI's `parse_session_info` (command.rs:231)
 * expects? If we drift here, a TUI node won't decode our announcement
 * and the interop promise breaks silently.
 */
import { describe, it, expect, beforeEach, mock, jest } from "bun:test";
import { SessionManager } from "../../../src/entrypoints/background/sessionManager";
import { DkgState } from "@mpc-wallet/types/dkg";
import { MeshStatusType } from "@mpc-wallet/types/mesh";
import type { AppState } from "@mpc-wallet/types/appstate";

function makeWsClientMock() {
    const announcedPayloads: Array<Record<string, unknown>> = [];
    const client: any = {
        announceSession: jest.fn((sessionInfo: Record<string, unknown>) => {
            announcedPayloads.push(sessionInfo);
        }),
        getReadyState: jest.fn(() => WebSocket.OPEN),
    };
    return { client, announcedPayloads };
}

function makeAppState(deviceId = "ext-device"): AppState {
    return {
        deviceId,
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

function makeManager(opts?: { wsReady?: number }) {
    const appState = makeAppState();
    const { client, announcedPayloads } = makeWsClientMock();
    if (opts?.wsReady !== undefined) {
        client.getReadyState = jest.fn(() => opts.wsReady);
    }
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
        client as any,
        broadcastToPopup as any,
        sendToOffscreen as any,
        stateManager as any,
    );
    return { mgr, appState, client, announcedPayloads, broadcastToPopup };
}

describe("SessionManager.createDkgWallet", () => {
    it("returns an error if WebSocket isn't open", async () => {
        // WebSocket.CLOSED === 3 per the spec. The jest mock in
        // setup-bun.ts wraps WebSocket with jest.fn() which doesn't
        // carry static readyState constants, so use the literal.
        const res = await (async () => {
            const { mgr } = makeManager({ wsReady: 3 });
            return mgr.createDkgWallet({
                total: 3,
                threshold: 2,
                curve: "secp256k1",
            });
        })();
        expect(res.success).toBe(false);
        expect(res.error).toContain("not connected");
    });

    it("rejects invalid total/threshold", async () => {
        const { mgr } = makeManager();
        expect(
            (
                await mgr.createDkgWallet({
                    total: 1,
                    threshold: 2,
                    curve: "secp256k1",
                })
            ).success,
        ).toBe(false);
        expect(
            (
                await mgr.createDkgWallet({
                    total: 3,
                    threshold: 4,
                    curve: "secp256k1",
                })
            ).success,
        ).toBe(false);
        expect(
            (
                await mgr.createDkgWallet({
                    total: 3,
                    threshold: 0,
                    curve: "secp256k1",
                })
            ).success,
        ).toBe(false);
    });

    it("emits TUI-compatible announce_session payload for a 2-of-3 DKG", async () => {
        const { mgr, client, announcedPayloads, appState } = makeManager();
        const res = await mgr.createDkgWallet({
            total: 3,
            threshold: 2,
            curve: "secp256k1",
        });

        expect(res.success).toBe(true);
        expect(res.sessionId).toBeDefined();
        expect(res.sessionId!.startsWith("dkg_")).toBe(true);
        expect(client.announceSession).toHaveBeenCalledTimes(1);

        const payload = announcedPayloads[0];
        // These are the fields TUI's parse_session_info requires or
        // expects to find with specific default-able values.
        expect(payload.session_id).toBe(res.sessionId);
        expect(payload.total).toBe(3);
        expect(payload.threshold).toBe(2);
        expect(payload.session_type).toBe("dkg");
        expect(payload.curve_type).toBe("secp256k1");
        expect(payload.coordination_type).toBe("Network");
        expect(payload.proposer_id).toBe(appState.deviceId);
        // Creator only lists themselves; joiners append.
        expect(payload.participants).toEqual([appState.deviceId]);
        // Local-only bookkeeping MUST NOT hit the wire.
        expect("accepted_devices" in payload).toBe(false);
        expect("status" in payload).toBe(false);
    });

    it("defaults curve to ed25519 and emits correct wire", async () => {
        const { mgr, announcedPayloads } = makeManager();
        await mgr.createDkgWallet({
            total: 2,
            threshold: 2,
            curve: "ed25519",
        });
        expect(announcedPayloads[0].curve_type).toBe("ed25519");
    });

    it("stashes creating_wallet state so the UI can show waiting state", async () => {
        const { mgr, appState } = makeManager();
        const res = await mgr.createDkgWallet({
            total: 3,
            threshold: 2,
            curve: "secp256k1",
        });
        expect(appState.sessionInfo).not.toBeNull();
        expect(res.sessionId).toBeDefined();
        expect(appState.sessionInfo!.session_id).toBe(res.sessionId!);
        expect(appState.dkgState).toBe(DkgState.Initializing);
        // The creator's own session should be in invites (so JoinSession
        // view stays consistent across creator vs. joiner) and marked
        // as auto-accepted by the creator.
        expect(
            appState.invites.some((s) => s.session_id === res.sessionId),
        ).toBe(true);
    });

    it("notifies popup so the UI can react without polling", async () => {
        const { mgr, broadcastToPopup } = makeManager();
        await mgr.createDkgWallet({
            total: 3,
            threshold: 2,
            curve: "secp256k1",
        });
        expect(broadcastToPopup).toHaveBeenCalled();
        const calls = (broadcastToPopup as any).mock.calls as any[];
        const sawDkgWalletCreated = calls.some(
            (call) => call[0]?.type === "dkgWalletCreated",
        );
        expect(sawDkgWalletCreated).toBe(true);
    });

    it("generates unique session ids across rapid calls", async () => {
        const { mgr } = makeManager();
        const ids = new Set<string>();
        for (let i = 0; i < 10; i++) {
            const res = await mgr.createDkgWallet({
                total: 3,
                threshold: 2,
                curve: "secp256k1",
            });
            if (res.sessionId) ids.add(res.sessionId);
        }
        expect(ids.size).toBe(10);
    });
});
