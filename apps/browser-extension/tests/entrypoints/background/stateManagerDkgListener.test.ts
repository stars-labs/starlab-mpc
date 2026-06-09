/**
 * Regression tests for StateManager.addDkgStateListener — the hook
 * that KeepaliveController subscribes to so offscreen pings track
 * ceremony state.
 *
 * We care about four invariants:
 * 1. Initial subscription fires immediately with the current state
 *    (so a subscriber attaching mid-ceremony catches up instead of
 *    sitting idle until the next transition).
 * 2. Listeners fire on transitions from ANY of the mutation APIs —
 *    `updateState({ dkgState })`, `updateStateProperty("dkgState",
 *    ...)`, and the offscreen-originated `dkgStateUpdate` message.
 * 3. No-op transitions (current === new) do NOT re-fire listeners —
 *    prevents the keepalive from flapping on incidental broadcasts.
 * 4. `unsubscribe()` actually detaches the listener.
 */
import { describe, it, expect, jest, beforeEach } from "bun:test";
import { DkgState } from "@starlab/types/dkg";
import { MeshStatusType } from "@starlab/types/mesh";
import { StateManager } from "../../../src/entrypoints/background/stateManager";

function makeStateManager() {
    // The constructor kicks off async chrome.storage.local.get — with
    // the mock in setup-bun.ts that resolves synchronously with
    // empty data. Fine for listener testing.
    return new StateManager({
        deviceId: "test-device",
        dkgState: DkgState.Idle,
    });
}

describe("StateManager.addDkgStateListener", () => {
    let mgr: StateManager;

    beforeEach(() => {
        mgr = makeStateManager();
    });

    it("fires immediately with the current state on subscription", () => {
        const listener = jest.fn();
        mgr.addDkgStateListener(listener);
        expect(listener).toHaveBeenCalledTimes(1);
        expect(listener).toHaveBeenCalledWith(DkgState.Idle);
    });

    it("fires on updateState({ dkgState })", () => {
        const listener = jest.fn();
        mgr.addDkgStateListener(listener);
        listener.mockClear();
        mgr.updateState({ dkgState: DkgState.Initializing });
        expect(listener).toHaveBeenCalledTimes(1);
        expect(listener).toHaveBeenCalledWith(DkgState.Initializing);
    });

    it("fires on updateStateProperty('dkgState', ...)", () => {
        const listener = jest.fn();
        mgr.addDkgStateListener(listener);
        listener.mockClear();
        mgr.updateStateProperty("dkgState", DkgState.Round1InProgress);
        expect(listener).toHaveBeenCalledWith(DkgState.Round1InProgress);
    });

    it("does NOT fire when the new value equals the current value", () => {
        // Invariant #3: same-value writes must be silent. Without this
        // check, the keepalive would flap on benign broadcasts that
        // happen to re-set dkgState to its current value.
        mgr.updateState({ dkgState: DkgState.Round1InProgress });
        const listener = jest.fn();
        mgr.addDkgStateListener(listener);
        listener.mockClear();
        mgr.updateState({ dkgState: DkgState.Round1InProgress });
        mgr.updateStateProperty("dkgState", DkgState.Round1InProgress);
        expect(listener).not.toHaveBeenCalled();
    });

    it("does NOT fire when other state changes but dkgState is untouched", () => {
        const listener = jest.fn();
        mgr.addDkgStateListener(listener);
        listener.mockClear();
        mgr.updateState({
            wsConnected: true,
            meshStatus: { type: MeshStatusType.Ready },
        });
        expect(listener).not.toHaveBeenCalled();
    });

    it("fires on offscreen-originated dkgStateUpdate", () => {
        const listener = jest.fn();
        mgr.addDkgStateListener(listener);
        listener.mockClear();
        mgr.handleOffscreenStateUpdate({
            type: "dkgStateUpdate",
            state: DkgState.Round2InProgress,
        } as any);
        expect(listener).toHaveBeenCalledWith(DkgState.Round2InProgress);
    });

    it("fires on auto-transition KeystoreImported → Complete via mesh ready", () => {
        // The StateManager has business logic that flips KeystoreImported
        // to Complete when the mesh reaches threshold. That internal
        // transition must notify listeners too.
        mgr.updateState({
            dkgState: DkgState.KeystoreImported,
            sessionInfo: {
                session_id: "s",
                proposer_id: "p",
                total: 2,
                threshold: 2,
                participants: ["test-device", "other"],
                accepted_devices: [],
            },
            webrtcConnections: { other: true },
        });

        const listener = jest.fn();
        mgr.addDkgStateListener(listener);
        listener.mockClear();

        mgr.handleOffscreenStateUpdate({
            type: "meshStatusUpdate",
            status: { type: MeshStatusType.Ready },
        } as any);

        // At least one of the calls should be Complete.
        const sawComplete = listener.mock.calls.some(
            (call: any) => call[0] === DkgState.Complete,
        );
        expect(sawComplete).toBe(true);
    });

    it("unsubscribe detaches the listener permanently", () => {
        const listener = jest.fn();
        const unsub = mgr.addDkgStateListener(listener);
        listener.mockClear();
        unsub();
        mgr.updateState({ dkgState: DkgState.Initializing });
        mgr.updateState({ dkgState: DkgState.Round1InProgress });
        expect(listener).not.toHaveBeenCalled();
    });

    it("swallows listener exceptions so one broken subscriber can't wedge others", () => {
        const bad = jest.fn(() => {
            throw new Error("subscriber exploded");
        });
        const good = jest.fn();
        mgr.addDkgStateListener(bad);
        mgr.addDkgStateListener(good);
        good.mockClear();
        bad.mockClear();
        expect(() =>
            mgr.updateState({ dkgState: DkgState.Finalizing }),
        ).not.toThrow();
        expect(bad).toHaveBeenCalledWith(DkgState.Finalizing);
        expect(good).toHaveBeenCalledWith(DkgState.Finalizing);
    });

    it("supports multiple independent subscribers", () => {
        const l1 = jest.fn();
        const l2 = jest.fn();
        mgr.addDkgStateListener(l1);
        mgr.addDkgStateListener(l2);
        l1.mockClear();
        l2.mockClear();
        mgr.updateState({ dkgState: DkgState.Initializing });
        expect(l1).toHaveBeenCalledWith(DkgState.Initializing);
        expect(l2).toHaveBeenCalledWith(DkgState.Initializing);
    });
});
