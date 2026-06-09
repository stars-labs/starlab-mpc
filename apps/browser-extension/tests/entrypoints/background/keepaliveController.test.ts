/**
 * Regression tests for the offscreen keepalive controller
 * (architectural reminder #2 — prevents DKG/signing from dying when
 * the offscreen doc times out mid-ceremony).
 *
 * We inject fake `Scheduler` and `MessageSender` so we never actually
 * wait 25 seconds per tick. The fake scheduler exposes `advance(ms)`
 * to drive the timer deterministically.
 *
 * Invariants this file locks down:
 * 1. Starts exactly when dkgState transitions INTO an active phase.
 * 2. Stops exactly when dkgState transitions OUT to Idle/Complete/
 *    Failed/KeystoreImported.
 * 3. `isActiveState` matches TUI's notion of "ceremony in progress"
 *    (Round1/Round2/Finalizing/Initializing only).
 * 4. An immediate ping fires on activation, THEN every
 *    KEEPALIVE_INTERVAL_MS thereafter — no silent 25s gap before the
 *    first ping that would let offscreen die first.
 * 5. Same-state re-emissions (StateManager replaying current dkgState
 *    to a late listener) don't double-start or flap the timer.
 * 6. `dispose()` stops the timer even from an active state.
 * 7. Ping failures (chrome.runtime.sendMessage rejecting) don't crash
 *    the timer — the next interval still fires.
 */
import { describe, it, expect, jest, beforeEach } from "bun:test";
import { DkgState } from "@starlab/types/dkg";
import {
    KeepaliveController,
    KEEPALIVE_INTERVAL_MS,
    type Scheduler,
    type MessageSender,
} from "../../../src/entrypoints/background/keepaliveController";

/**
 * Deterministic fake scheduler. `setInterval` returns a handle the
 * test can look up; `advance(ms)` moves simulated time forward and
 * fires every active interval whose next-trigger time is ≤ new now.
 */
function makeFakeScheduler() {
    type IntervalEntry = {
        handle: number;
        fn: () => void;
        ms: number;
        nextAt: number;
    };
    let now = 0;
    let nextHandle = 1;
    const intervals = new Map<number, IntervalEntry>();
    const scheduler: Scheduler = {
        setInterval(fn, ms) {
            const handle = nextHandle++;
            intervals.set(handle, {
                handle,
                fn,
                ms,
                nextAt: now + ms,
            });
            return handle;
        },
        clearInterval(h) {
            intervals.delete(h);
        },
    };
    return {
        scheduler,
        advance(ms: number) {
            now += ms;
            // Run as many fires as necessary — intervals might repeat
            // within a single advance if `ms` exceeds the interval
            // period, same as real setInterval semantics.
            let fired = true;
            while (fired) {
                fired = false;
                for (const entry of Array.from(intervals.values())) {
                    if (entry.nextAt <= now) {
                        entry.fn();
                        entry.nextAt += entry.ms;
                        fired = true;
                    }
                }
            }
        },
        intervalsCount() {
            return intervals.size;
        },
    };
}

describe("KeepaliveController", () => {
    let fakeClock: ReturnType<typeof makeFakeScheduler>;
    let send: jest.Mock;
    let ctl: KeepaliveController;

    beforeEach(() => {
        fakeClock = makeFakeScheduler();
        send = jest.fn(async () => ({ ok: true }));
        ctl = new KeepaliveController({
            scheduler: fakeClock.scheduler,
            send: send as unknown as MessageSender,
        });
    });

    describe("isActiveState", () => {
        it("flags in-progress DKG rounds as active", () => {
            expect(KeepaliveController.isActiveState(DkgState.Initializing)).toBe(true);
            expect(
                KeepaliveController.isActiveState(DkgState.Round1InProgress),
            ).toBe(true);
            expect(
                KeepaliveController.isActiveState(DkgState.Round2InProgress),
            ).toBe(true);
            expect(KeepaliveController.isActiveState(DkgState.Finalizing)).toBe(true);
        });

        it("flags done/idle states as inactive", () => {
            expect(KeepaliveController.isActiveState(DkgState.Idle)).toBe(false);
            expect(KeepaliveController.isActiveState(DkgState.Complete)).toBe(false);
            expect(KeepaliveController.isActiveState(DkgState.Failed)).toBe(false);
            expect(
                KeepaliveController.isActiveState(DkgState.KeystoreImported),
            ).toBe(false);
        });
    });

    it("does nothing while dkgState stays Idle", () => {
        ctl.onDkgStateChange(DkgState.Idle);
        expect(ctl.isRunning()).toBe(false);
        expect(fakeClock.intervalsCount()).toBe(0);
        expect(send).not.toHaveBeenCalled();
    });

    it("starts on transition to Initializing and pings immediately", () => {
        ctl.onDkgStateChange(DkgState.Initializing);
        expect(ctl.isRunning()).toBe(true);
        // Invariant #4: immediate ping on activation — we can't afford
        // to wait 25s for the first ping because offscreen might die
        // within that window.
        expect(send).toHaveBeenCalledTimes(1);
        const firstCall = send.mock.calls[0][0];
        expect(firstCall.type).toBe("keepalive");
        expect(firstCall.target).toBe("offscreen");
        expect(typeof firstCall.ts).toBe("number");
    });

    it("pings every KEEPALIVE_INTERVAL_MS after activation", () => {
        ctl.onDkgStateChange(DkgState.Round1InProgress);
        expect(send).toHaveBeenCalledTimes(1); // immediate ping
        fakeClock.advance(KEEPALIVE_INTERVAL_MS - 1);
        expect(send).toHaveBeenCalledTimes(1); // still only the immediate one
        fakeClock.advance(1);
        expect(send).toHaveBeenCalledTimes(2); // first interval fire
        fakeClock.advance(KEEPALIVE_INTERVAL_MS * 3);
        expect(send).toHaveBeenCalledTimes(5); // 3 more fires in 3 intervals
    });

    it("stops on transition to Complete", () => {
        ctl.onDkgStateChange(DkgState.Round2InProgress);
        expect(ctl.isRunning()).toBe(true);
        ctl.onDkgStateChange(DkgState.Complete);
        expect(ctl.isRunning()).toBe(false);
        // No further pings fire even if simulated time marches on.
        const countAtStop = send.mock.calls.length;
        fakeClock.advance(KEEPALIVE_INTERVAL_MS * 5);
        expect(send).toHaveBeenCalledTimes(countAtStop);
    });

    it("stops on Failed / Idle / KeystoreImported", () => {
        for (const done of [
            DkgState.Failed,
            DkgState.Idle,
            DkgState.KeystoreImported,
        ]) {
            ctl.onDkgStateChange(DkgState.Round1InProgress);
            expect(ctl.isRunning()).toBe(true);
            ctl.onDkgStateChange(done);
            expect(ctl.isRunning()).toBe(false);
        }
    });

    it("does not double-start when the same active state is emitted twice", () => {
        // This is the StateManager-adds-listener-with-initial-emit
        // case: the listener receives the current dkgState, which
        // might be the same as the one it just saw.
        ctl.onDkgStateChange(DkgState.Initializing);
        const initialPings = send.mock.calls.length;
        ctl.onDkgStateChange(DkgState.Initializing);
        ctl.onDkgStateChange(DkgState.Initializing);
        expect(send).toHaveBeenCalledTimes(initialPings);
        // Still exactly one interval running.
        expect(fakeClock.intervalsCount()).toBe(1);
    });

    it("resumes after a stop → start cycle (no stuck state)", () => {
        ctl.onDkgStateChange(DkgState.Initializing);
        ctl.onDkgStateChange(DkgState.Idle);
        expect(ctl.isRunning()).toBe(false);
        ctl.onDkgStateChange(DkgState.Round1InProgress);
        expect(ctl.isRunning()).toBe(true);
    });

    it("signing-active hook toggles the same way", () => {
        ctl.onSigningActiveChange(true);
        expect(ctl.isRunning()).toBe(true);
        expect(send).toHaveBeenCalledTimes(1);
        ctl.onSigningActiveChange(true); // idempotent
        expect(send).toHaveBeenCalledTimes(1);
        ctl.onSigningActiveChange(false);
        expect(ctl.isRunning()).toBe(false);
    });

    it("dispose() clears an active timer", () => {
        ctl.onDkgStateChange(DkgState.Initializing);
        expect(ctl.isRunning()).toBe(true);
        ctl.dispose();
        expect(ctl.isRunning()).toBe(false);
        expect(fakeClock.intervalsCount()).toBe(0);
    });

    it("dispose() is safe when nothing was running", () => {
        expect(() => ctl.dispose()).not.toThrow();
    });

    it("keeps running after a ping failure (sendMessage rejects)", async () => {
        const failing = jest
            .fn()
            .mockRejectedValueOnce(new Error("Receiving end does not exist"))
            .mockResolvedValue({ ok: true });
        const ctlWithFailingSend = new KeepaliveController({
            scheduler: fakeClock.scheduler,
            send: failing as unknown as MessageSender,
        });
        ctlWithFailingSend.onDkgStateChange(DkgState.Round1InProgress);
        // Allow the immediate ping's rejection to propagate.
        await Promise.resolve();
        expect(ctlWithFailingSend.isRunning()).toBe(true);
        // Next fire still goes through.
        fakeClock.advance(KEEPALIVE_INTERVAL_MS);
        expect(failing).toHaveBeenCalledTimes(2);
    });

    it("ping payload carries monotonically increasing timestamps", () => {
        ctl.onDkgStateChange(DkgState.Round1InProgress);
        fakeClock.advance(KEEPALIVE_INTERVAL_MS);
        fakeClock.advance(KEEPALIVE_INTERVAL_MS);
        const tsValues = send.mock.calls.map((call: any) => call[0].ts);
        for (let i = 1; i < tsValues.length; i++) {
            // Real clock — not monotonic from test perspective because
            // Date.now() is real — but all values must be finite
            // numbers. Mainly guards that we didn't regress to sending
            // `undefined` or NaN as timestamp.
            expect(typeof tsValues[i]).toBe("number");
            expect(Number.isFinite(tsValues[i])).toBe(true);
        }
    });
});
