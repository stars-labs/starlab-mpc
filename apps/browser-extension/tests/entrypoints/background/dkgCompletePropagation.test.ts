/**
 * Ext-1d event-propagation regression tests.
 *
 * The webrtc.ts finalize path emits an `onDkgComplete` payload with
 * the derived group public key + address. Offscreen forwards this to
 * background as a `fromOffscreen { type: "dkgComplete", ... }`
 * message, which StateManager must route onto appState so the popup
 * can render the completion banner.
 *
 * This test covers the middle slice: given a `dkgComplete` payload
 * from offscreen, does StateManager stash it correctly + broadcast
 * a `dkgCompleted` popup event?
 *
 * Does NOT cover:
 * - WASM finalize / address derivation (lives in webrtc.ts).
 * - The popup's Svelte rendering (lives in App.svelte).
 * - Encrypt + save keyshare (deferred: separate commit).
 */
import { describe, it, expect, beforeEach, jest } from "bun:test";
import { StateManager } from "../../../src/entrypoints/background/stateManager";
import { DkgState } from "@mpc-wallet/types/dkg";

describe("StateManager: dkgComplete propagation", () => {
    let mgr: StateManager;

    beforeEach(() => {
        mgr = new StateManager({
            deviceId: "ext-bob",
            dkgState: DkgState.Finalizing,
        });
    });

    it("stashes address + groupPublicKey on appState", () => {
        mgr.handleOffscreenStateUpdate({
            type: "dkgComplete",
            address: "0xAbCd1234",
            groupPublicKey: "02cafebabe",
            blockchain: "ethereum",
            sessionId: "dkg_x",
            threshold: 2,
            total: 3,
            participants: ["a", "b", "c"],
            participantIndex: 1,
        } as any);

        const state = mgr.getState() as any;
        expect(state.dkgAddress).toBe("0xAbCd1234");
        expect(state.dkgGroupPublicKey).toBe("02cafebabe");
        expect(state.dkgLastResult.sessionId).toBe("dkg_x");
        expect(state.dkgLastResult.participants).toEqual(["a", "b", "c"]);
    });

    it("is idempotent-friendly — repeated dkgComplete updates just overwrite", () => {
        // If the offscreen fires dkgComplete twice (e.g. late retry
        // path), the state should end up reflecting the LATEST
        // payload, not a merged or rejected state. This locks down
        // "last writer wins" semantics.
        mgr.handleOffscreenStateUpdate({
            type: "dkgComplete",
            address: "0xFirst",
            groupPublicKey: "first_pk",
            blockchain: "ethereum",
            sessionId: "dkg_a",
            threshold: 2,
            total: 2,
            participants: ["x", "y"],
            participantIndex: 1,
        } as any);
        mgr.handleOffscreenStateUpdate({
            type: "dkgComplete",
            address: "0xSecond",
            groupPublicKey: "second_pk",
            blockchain: "ethereum",
            sessionId: "dkg_a",
            threshold: 2,
            total: 2,
            participants: ["x", "y"],
            participantIndex: 1,
        } as any);
        const state = mgr.getState() as any;
        expect(state.dkgAddress).toBe("0xSecond");
        expect(state.dkgGroupPublicKey).toBe("second_pk");
    });

    it("handles a null address gracefully (ed25519 WASM returned null)", () => {
        mgr.handleOffscreenStateUpdate({
            type: "dkgComplete",
            address: null,
            groupPublicKey: "abcd",
            blockchain: "solana",
            sessionId: "dkg_null",
            threshold: 2,
            total: 2,
            participants: [],
            participantIndex: null,
        } as any);
        const state = mgr.getState() as any;
        expect(state.dkgAddress).toBe("");
        expect(state.dkgGroupPublicKey).toBe("abcd");
    });
});
