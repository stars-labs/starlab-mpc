/**
 * Ext-2a/b regression tests for sessionManager.createSigningSession.
 *
 * The wire shape this emits must match TUI's
 * `parse_session_info` expectations for signing sessions
 * (command.rs:231 + the signing-specific top-level fields). If the
 * shape drifts, TUI can't decode the announcement and the signing
 * ceremony never starts cross-client.
 */
import { describe, it, expect, beforeEach, jest } from "bun:test";
import { SessionManager } from "../../../src/entrypoints/background/sessionManager";
import { DkgState } from "@starlab/types/dkg";
import { MeshStatusType } from "@starlab/types/mesh";
import type { AppState } from "@starlab/types/appstate";

function makeSessionManager() {
    const appState: AppState = {
        deviceId: "ext-alice",
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
    const announces: Array<Record<string, unknown>> = [];
    const openSentinel = (WebSocket as any)?.OPEN;
    const wsClient: any = {
        getReadyState: jest.fn(() => openSentinel),
        announceSession: jest.fn((si: Record<string, unknown>) => {
            announces.push(si);
        }),
        sendSessionStatusUpdate: jest.fn(),
    };
    const broadcast = jest.fn();
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
        broadcast as any,
        sendToOffscreen as any,
        stateManager as any,
    );
    return { mgr, appState, announces, broadcast, wsClient };
}

const validConfig = () => ({
    walletId: "wallet-xyz",
    walletName: "Treasury",
    groupPublicKey: "02cafe",
    blockchain: "ethereum" as const,
    threshold: 2,
    total: 3,
    signingMessageHex: "deadbeef",
});

describe("SessionManager.createSigningSession", () => {
    let env: ReturnType<typeof makeSessionManager>;
    beforeEach(() => {
        env = makeSessionManager();
    });

    it("rejects non-hex signingMessageHex", async () => {
        const res = await env.mgr.createSigningSession({
            ...validConfig(),
            signingMessageHex: "not-hex!",
        });
        expect(res.success).toBe(false);
        expect(res.error).toContain("hex");
    });

    it("rejects missing walletId / groupPublicKey", async () => {
        const r1 = await env.mgr.createSigningSession({
            ...validConfig(),
            walletId: "",
        });
        expect(r1.success).toBe(false);
        const r2 = await env.mgr.createSigningSession({
            ...validConfig(),
            groupPublicKey: "",
        });
        expect(r2.success).toBe(false);
    });

    it("emits TUI-compatible signing announce_session payload", async () => {
        const res = await env.mgr.createSigningSession(validConfig());
        expect(res.success).toBe(true);
        expect(res.sessionId).toMatch(/^sign_/);
        expect(env.announces.length).toBe(1);

        const wire = env.announces[0];
        expect(wire.session_id).toBe(res.sessionId);
        expect(wire.session_type).toBe("signing");
        expect(wire.total).toBe(3);
        expect(wire.threshold).toBe(2);
        expect(wire.proposer_id).toBe("ext-alice");
        expect(wire.participants).toEqual(["ext-alice"]);
        expect(wire.curve_type).toBe("secp256k1");
        expect(wire.coordination_type).toBe("Network");
        // Signing-specific sibling fields TUI reads at top level
        // (not nested under a content object).
        expect(wire.wallet_name).toBe("Treasury");
        expect(wire.group_public_key).toBe("02cafe");
        expect(wire.blockchain).toBe("ethereum");
        expect(wire.signing_message_hex).toBe("deadbeef");
        // accepted_devices is extension-local — must NOT be on the wire.
        expect("accepted_devices" in wire).toBe(false);
    });

    it("maps ed25519 curve correctly on the wire", async () => {
        await env.mgr.createSigningSession({
            ...validConfig(),
            blockchain: "solana",
        });
        expect(env.announces[0].curve_type).toBe("ed25519");
        expect(env.announces[0].blockchain).toBe("solana");
    });

    it("stashes as active sessionInfo + flips dkgState to Initializing", async () => {
        await env.mgr.createSigningSession(validConfig());
        expect(env.appState.sessionInfo).not.toBeNull();
        expect(env.appState.sessionInfo!.session_type).toBe("signing");
        expect(env.appState.dkgState).toBe(DkgState.Initializing);
    });

    it("notifies popup with signingSessionCreated event", async () => {
        await env.mgr.createSigningSession(validConfig());
        const calls = (env.broadcast as any).mock.calls as any[];
        const saw = calls.some(
            (c) => c[0]?.type === "signingSessionCreated",
        );
        expect(saw).toBe(true);
    });

    it("generates unique session ids across rapid calls", async () => {
        const ids = new Set<string>();
        for (let i = 0; i < 5; i++) {
            const r = await env.mgr.createSigningSession(validConfig());
            if (r.sessionId) ids.add(r.sessionId);
        }
        expect(ids.size).toBe(5);
    });

    it("rolls back state on wire-announce failure", async () => {
        env.wsClient.announceSession = jest.fn(() => {
            throw new Error("ws blew up");
        });
        const res = await env.mgr.createSigningSession(validConfig());
        expect(res.success).toBe(false);
        expect(env.appState.sessionInfo).toBeNull();
        expect(env.appState.dkgState).toBe(DkgState.Idle);
    });
});
