// ===================================================================
// OFFSCREEN KEEPALIVE CONTROLLER
// ===================================================================
//
// Chrome Extension Manifest V3 rule: offscreen documents are
// automatically torn down after ~30 seconds without any message
// activity, even when they hold long-lived resources like WebRTC
// peer connections. When that happens mid-DKG or mid-signing:
//   - the in-progress FROST rounds die
//   - peers see connection timeouts
//   - the user sees a silent failure (UI says "DKG in progress…"
//     forever)
//
// This controller wakes the offscreen via `chrome.runtime.sendMessage`
// every 25 seconds while a ceremony is active. A 25s interval keeps
// us safely under the ~30s idle limit with margin for chrome
// scheduling jitter.
//
// Scope is deliberately narrow: no ceremony state persistence (the
// rest of the codebase treats session data as intentionally ephemeral
// for security; see `stateManager.ts::persistState` line 117). This
// is purely about keeping the offscreen runtime warm so the ceremony
// that IS happening doesn't die under our feet.
// ===================================================================

import { DkgState } from "@starlab/types/dkg";

/** Message shape sent to offscreen. Must carry `target: "offscreen"`
 *  so the offscreen entrypoint routes it correctly and non-offscreen
 *  listeners (popup, content) ignore it. */
export interface KeepalivePing {
    type: "keepalive";
    target: "offscreen";
    /** Monotonic timestamp; useful if we ever want to track lag. */
    ts: number;
}

/** Runtime constants pulled into their own exports so tests can
 *  override via dependency injection or module mocks. */
export const KEEPALIVE_INTERVAL_MS = 25_000;

/**
 * Test-friendly interface for the underlying setInterval/clearInterval
 * primitives. Production code uses the global `setInterval` etc.;
 * tests inject a fake clock so a 25-second cadence doesn't mean a
 * 25-second test.
 */
export interface Scheduler {
    setInterval(fn: () => void, ms: number): any;
    clearInterval(handle: any): void;
}

/**
 * Likewise injectable: the messaging primitive. Production is
 * `chrome.runtime.sendMessage`; tests can count calls.
 */
export type MessageSender = (msg: KeepalivePing) => Promise<unknown>;

/**
 * Keeps the offscreen document alive while a ceremony is in
 * progress. Subscribe by handing off instances of `.onDkgStateChange`
 * to StateManager — see `StateManager.addDkgStateListener`.
 */
export class KeepaliveController {
    private intervalHandle: any = null;
    private readonly scheduler: Scheduler;
    private readonly send: MessageSender;

    constructor(opts: {
        scheduler?: Scheduler;
        send?: MessageSender;
    } = {}) {
        this.scheduler = opts.scheduler ?? {
            setInterval: (fn, ms) => setInterval(fn, ms),
            clearInterval: (h) => clearInterval(h),
        };
        this.send =
            opts.send ??
            ((msg) => chrome.runtime.sendMessage(msg));
    }

    /**
     * Ceremony phases where the offscreen needs to stay warm. Done
     * states (Complete / Failed / KeystoreImported) are NOT here —
     * the offscreen can idle out safely once FROST has produced a
     * key or given up. Idle is also excluded.
     */
    static isActiveState(state: DkgState): boolean {
        return (
            state === DkgState.Initializing ||
            state === DkgState.Round1InProgress ||
            state === DkgState.Round2InProgress ||
            state === DkgState.Finalizing
        );
    }

    /**
     * Hook this into StateManager's dkgState change listener.
     * Activates the timer on transition into an active phase,
     * deactivates on transition out. Idempotent — calling it
     * twice with the same state is a no-op.
     */
    onDkgStateChange(state: DkgState): void {
        const shouldBeActive = KeepaliveController.isActiveState(state);
        if (shouldBeActive && this.intervalHandle === null) {
            this.start(state);
        } else if (!shouldBeActive && this.intervalHandle !== null) {
            this.stop(state);
        }
    }

    /**
     * Hook for the signing flow once Ext-2 lands. Symmetric to the
     * DKG hook: pass `true` when a signing ceremony begins,
     * `false` when it ends. Same underlying timer — we don't need
     * two if they can't overlap.
     */
    onSigningActiveChange(active: boolean): void {
        if (active && this.intervalHandle === null) {
            this.start("signing");
        } else if (!active && this.intervalHandle !== null) {
            this.stop("signing-done");
        }
    }

    /** For debugging / test assertions. */
    isRunning(): boolean {
        return this.intervalHandle !== null;
    }

    /** Stop the timer regardless of state. Called from shutdown paths. */
    dispose(): void {
        if (this.intervalHandle !== null) {
            this.scheduler.clearInterval(this.intervalHandle);
            this.intervalHandle = null;
        }
    }

    private start(reason: DkgState | "signing"): void {
        console.log(
            `[KeepaliveController] Starting offscreen keepalive (${String(reason)}) every ${KEEPALIVE_INTERVAL_MS}ms`,
        );
        // Send one immediately so the offscreen is touched right away,
        // not only after the first full interval.
        void this.ping();
        this.intervalHandle = this.scheduler.setInterval(() => {
            void this.ping();
        }, KEEPALIVE_INTERVAL_MS);
    }

    private stop(reason: DkgState | "signing-done"): void {
        console.log(
            `[KeepaliveController] Stopping offscreen keepalive (${String(reason)})`,
        );
        this.scheduler.clearInterval(this.intervalHandle);
        this.intervalHandle = null;
    }

    private async ping(): Promise<void> {
        try {
            await this.send({
                type: "keepalive",
                target: "offscreen",
                ts: Date.now(),
            });
        } catch (err) {
            // A failed ping isn't fatal — the next interval tries
            // again. But if the offscreen is genuinely gone (e.g.
            // chrome.runtime reports "Receiving end does not exist")
            // we log it; callers can observe this to decide whether
            // to teardown/rebuild offscreen.
            console.warn(
                "[KeepaliveController] Keepalive ping failed:",
                (err as Error)?.message ?? err,
            );
        }
    }
}
