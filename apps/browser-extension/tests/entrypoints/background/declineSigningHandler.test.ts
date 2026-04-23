/**
 * Ext-3c handler-level tests for PopupMessageHandler.handleDeclineSigningSessionRequest.
 *
 * signingDecline.test.ts covers the wire layer (WebSocketManager.relayToPeer
 * + inbound handleRelayMessage routing). This suite fills the gap between:
 * popup → background dispatch arrives at the message handler, which must
 * look up the invite, extract the proposer_id, call relayToPeer with the
 * right shape, and update appState.invites. Each gate check (missing
 * session_id, unknown invite, self-proposal, missing deviceId, relay
 * failure) must produce a distinct error so the popup can surface the
 * right message — and none of them should silently drop.
 */
import { describe, it, expect, beforeEach, jest, mock } from "bun:test";

import * as mockImports from "../../wxt-imports-mock";
mock.module("#imports", () => mockImports);

import { PopupMessageHandler } from "../../../src/entrypoints/background/messageHandlers";
import type { SessionInfo } from "@mpc-wallet/types/session";
import { DkgState } from "@mpc-wallet/types/dkg";
import { MeshStatusType } from "@mpc-wallet/types/mesh";
import type { AppState } from "@mpc-wallet/types/appstate";

function signingInvite(overrides: Partial<SessionInfo> = {}): SessionInfo {
    return {
        session_id: "sign_abc",
        proposer_id: "ext-alice",
        total: 3,
        threshold: 2,
        participants: ["ext-alice", "ext-bob"],
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

function makeHandler(opts?: {
    deviceId?: string;
    invites?: SessionInfo[];
    relaySucceeds?: boolean;
}) {
    const appState: AppState = {
        deviceId: opts?.deviceId ?? "ext-bob",
        connecteddevices: [],
        wsConnected: true,
        sessionInfo: null,
        invites: opts?.invites ?? [signingInvite()],
        meshStatus: { type: MeshStatusType.Incomplete },
        dkgState: DkgState.Idle,
        webrtcConnections: {},
        blockchain: "ethereum",
    };

    const relayCalls: Array<{ to: string; data: any }> = [];
    const stateManager = {
        getState: () => appState,
        updateInvites: jest.fn((inv: SessionInfo[]) => {
            appState.invites = inv;
        }),
    };
    const webSocketManager = {
        relayToPeer: jest.fn((to: string, data: any) => {
            relayCalls.push({ to, data });
            return opts?.relaySucceeds ?? true;
        }),
    };

    const handler = new PopupMessageHandler(
        stateManager as any,
        {} as any,
        webSocketManager as any,
        {} as any,
        {} as any,
        {} as any,
    );

    return { handler, appState, stateManager, webSocketManager, relayCalls };
}

async function callDecline(
    handler: PopupMessageHandler,
    message: any,
): Promise<any> {
    return new Promise((resolve) => {
        (handler as any).handleDeclineSigningSessionRequest(message, resolve);
    });
}

describe("PopupMessageHandler.handleDeclineSigningSessionRequest", () => {
    let env: ReturnType<typeof makeHandler>;
    beforeEach(() => {
        env = makeHandler();
    });

    it("happy path: relays SigningDecline to proposer with correct payload", async () => {
        const res = await callDecline(env.handler, { session_id: "sign_abc" });
        expect(res.success).toBe(true);
        expect(env.webSocketManager.relayToPeer).toHaveBeenCalledTimes(1);
        expect(env.relayCalls[0].to).toBe("ext-alice");
        expect(env.relayCalls[0].data.websocket_msg_type).toBe(
            "SigningDecline",
        );
        expect(env.relayCalls[0].data.signing_id).toBe("sign_abc");
        expect(env.relayCalls[0].data.decliner_id).toBe("ext-bob");
    });

    it("happy path: removes the declined session from appState.invites", async () => {
        // Two invites; decline one, the other stays.
        const env2 = makeHandler({
            invites: [
                signingInvite({ session_id: "sign_keep" }),
                signingInvite({ session_id: "sign_drop" }),
            ],
        });
        const res = await callDecline(env2.handler, {
            session_id: "sign_drop",
        });
        expect(res.success).toBe(true);
        expect(env2.appState.invites.length).toBe(1);
        expect(env2.appState.invites[0].session_id).toBe("sign_keep");
    });

    it("rejects missing session_id", async () => {
        const res = await callDecline(env.handler, {});
        expect(res.success).toBe(false);
        expect(res.error).toContain("session_id required");
        expect(env.webSocketManager.relayToPeer).not.toHaveBeenCalled();
    });

    it("rejects non-string session_id", async () => {
        const res = await callDecline(env.handler, { session_id: 42 });
        expect(res.success).toBe(false);
        expect(res.error).toContain("session_id required");
    });

    it("rejects empty string session_id", async () => {
        const res = await callDecline(env.handler, { session_id: "" });
        expect(res.success).toBe(false);
        expect(res.error).toContain("session_id required");
    });

    it("rejects unknown session_id (no matching invite)", async () => {
        const res = await callDecline(env.handler, {
            session_id: "sign_unknown",
        });
        expect(res.success).toBe(false);
        expect(res.error).toContain("No known invite for sign_unknown");
        // Importantly: no relay dispatched for sessions we don't
        // recognize — otherwise a rogue popup could spam proposers.
        expect(env.webSocketManager.relayToPeer).not.toHaveBeenCalled();
    });

    it("rejects when device id is not set", async () => {
        const env2 = makeHandler({ deviceId: "" });
        const res = await callDecline(env2.handler, {
            session_id: "sign_abc",
        });
        expect(res.success).toBe(false);
        expect(res.error).toContain("Device id not set");
    });

    it("rejects self-proposal (can't decline your own session)", async () => {
        const env2 = makeHandler({
            deviceId: "ext-alice", // We're also the proposer.
            invites: [signingInvite()], // proposer_id === "ext-alice"
        });
        const res = await callDecline(env2.handler, {
            session_id: "sign_abc",
        });
        expect(res.success).toBe(false);
        expect(res.error).toContain("Invalid proposer for decline");
        expect(env2.webSocketManager.relayToPeer).not.toHaveBeenCalled();
    });

    it("rejects invite with missing proposer_id", async () => {
        const env2 = makeHandler({
            invites: [
                signingInvite({ proposer_id: "" } as any),
            ],
        });
        const res = await callDecline(env2.handler, {
            session_id: "sign_abc",
        });
        expect(res.success).toBe(false);
        expect(res.error).toContain("Invalid proposer for decline");
    });

    it("propagates relay failure (WS not open) as an error — but still removes invite locally", async () => {
        // The intent is: the user's "Decline" button shouldn't leave
        // a stale invite hanging around even if the relay didn't
        // reach the signal server. They can manually re-relay later
        // if needed, but locally we've decided.
        const env2 = makeHandler({ relaySucceeds: false });
        const res = await callDecline(env2.handler, {
            session_id: "sign_abc",
        });
        expect(res.success).toBe(false);
        expect(res.error).toContain("Signal server not connected");
        // Relay was attempted.
        expect(env2.webSocketManager.relayToPeer).toHaveBeenCalledTimes(1);
        // Invite still removed locally (see handler impl:
        // updateInvites happens before the ok-check).
        expect(env2.appState.invites.length).toBe(0);
    });
});
