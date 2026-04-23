/**
 * Ext-3c regression tests for the signing-decline path.
 *
 * Two halves to cover — both sides of the relay:
 *
 *   DECLINER SIDE (handleDeclineSigningSessionRequest):
 *     Takes {session_id} from the popup → looks up invite →
 *     extracts proposer_id → relays a SigningDecline frame to
 *     the proposer via signal server → removes from invites
 *     locally.
 *
 *   PROPOSER SIDE (webSocketManager handleRelayMessage case
 *   "SigningDecline"):
 *     Server forwards the relay → routes it to popup as a
 *     `signingPeerDeclined` broadcast (triggers the amber toast).
 *
 * The two sides are independent — decline can be built and shipped
 * without a real WebSocket, and reception can be tested without a
 * real declining peer. Mock the moving parts, assert the wire shape.
 */
import { describe, it, expect, beforeEach, jest } from "bun:test";
import { WebSocketManager } from "../../../src/entrypoints/background/webSocketManager";
import { DkgState } from "@mpc-wallet/types/dkg";
import { MeshStatusType } from "@mpc-wallet/types/mesh";
import type { AppState } from "@mpc-wallet/types/appstate";
import type { SessionInfo } from "@mpc-wallet/types/session";

function makeWsManager(opts?: {
    deviceId?: string;
    invites?: SessionInfo[];
}) {
    const appState: AppState = {
        deviceId: opts?.deviceId ?? "ext-proposer",
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
    const broadcasts: Array<Record<string, unknown>> = [];
    const broadcast = jest.fn((msg: any) => {
        broadcasts.push(msg);
    });
    const sendToOffscreen = jest.fn(async () => ({ success: true }));
    const stateManager = {
        updateInvites: jest.fn((inv: SessionInfo[]) => {
            appState.invites = inv;
        }),
        getState: () => appState,
    };
    const mgr = new WebSocketManager(
        appState,
        {} as any,
        broadcast as any,
        sendToOffscreen as any,
        stateManager as any,
    );
    return { mgr, appState, broadcasts, stateManager };
}

describe("WebSocketManager handleRelayMessage: SigningDecline inbound", () => {
    it("broadcasts signingPeerDeclined with the decliner and session id", () => {
        const { mgr, broadcasts } = makeWsManager({
            deviceId: "ext-proposer",
        });

        // Simulate the server forwarding a SigningDecline relay.
        // Wire shape matches what messageHandlers.relayToPeer emits
        // on the decliner's side: data.websocket_msg_type +
        // signing_id + decliner_id, and msg.from filled in by the
        // server.
        const relayFrame = {
            type: "relay",
            from: "ext-bob",
            data: {
                websocket_msg_type: "SigningDecline",
                signing_id: "sign_xyz",
                decliner_id: "ext-bob",
            },
        };
        (mgr as any).handleWebSocketMessage(relayFrame);

        const declined = broadcasts.filter(
            (b) => b.type === "signingPeerDeclined",
        );
        expect(declined.length).toBe(1);
        expect((declined[0] as any).sessionId).toBe("sign_xyz");
        expect((declined[0] as any).declinerId).toBe("ext-bob");
    });

    it("falls back to msg.from when decliner_id is absent", () => {
        // Older clients or relay rewrites may drop `decliner_id`.
        // The proposer should still attribute the decline to the
        // WebSocket peer who sent the relay — msg.from is the
        // server's canonical identifier of the sender.
        const { mgr, broadcasts } = makeWsManager({
            deviceId: "ext-proposer",
        });
        (mgr as any).handleWebSocketMessage({
            type: "relay",
            from: "ext-carol",
            data: {
                websocket_msg_type: "SigningDecline",
                signing_id: "sign_no_decliner_field",
            },
        });
        const declined = broadcasts.filter(
            (b) => b.type === "signingPeerDeclined",
        );
        expect(declined.length).toBe(1);
        expect((declined[0] as any).declinerId).toBe("ext-carol");
    });

    it("unknown websocket_msg_type in relay is a no-op (no decline broadcast)", () => {
        const { mgr, broadcasts } = makeWsManager({
            deviceId: "ext-proposer",
        });
        (mgr as any).handleWebSocketMessage({
            type: "relay",
            from: "ext-unknown",
            data: {
                websocket_msg_type: "UnexpectedMessage",
                foo: "bar",
            },
        });
        const declined = broadcasts.filter(
            (b) => b.type === "signingPeerDeclined",
        );
        expect(declined.length).toBe(0);
    });
});

describe("WebSocketManager.relayToPeer (outbound)", () => {
    it("returns false when WebSocket isn't open", () => {
        const { mgr } = makeWsManager();
        // No wsClient initialized → relay can't dispatch.
        const ok = mgr.relayToPeer("ext-alice", {
            websocket_msg_type: "SigningDecline",
            signing_id: "sign_no_ws",
        });
        expect(ok).toBe(false);
    });

    it("calls wsClient.relayMessage when connected", () => {
        const { mgr } = makeWsManager();
        const relayCalls: Array<{ to: string; data: any }> = [];
        (mgr as any).wsClient = {
            getReadyState: () => WebSocket.OPEN,
            relayMessage: jest.fn((to: string, data: any) => {
                relayCalls.push({ to, data });
            }),
        };
        const ok = mgr.relayToPeer("ext-alice", {
            websocket_msg_type: "SigningDecline",
            signing_id: "sign_ok",
            decliner_id: "ext-bob",
        });
        expect(ok).toBe(true);
        expect(relayCalls.length).toBe(1);
        expect(relayCalls[0].to).toBe("ext-alice");
        expect(relayCalls[0].data.websocket_msg_type).toBe("SigningDecline");
        expect(relayCalls[0].data.signing_id).toBe("sign_ok");
        expect(relayCalls[0].data.decliner_id).toBe("ext-bob");
    });
});
