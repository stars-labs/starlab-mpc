import { Buffer } from 'buffer';
import wasmInit, { FrostDkgEd25519, FrostDkgSecp256k1 } from '@starlab/core-wasm';
import fs from 'fs';
import path from 'path';

// Initialize WASM once for all tests
let wasmInitialized = false;

export async function initializeWasmIfNeeded(): Promise<boolean> {
    if (!wasmInitialized) {
        try {
            // Previous versions of this helper read the WASM bytes from
            // `apps/browser-extension/pkg/starlab_mpc_bg.wasm` — a stale
            // path from before core-wasm was split into its own package.
            // The @starlab/core-wasm entry (pkg/starlab_core_wasm.js)
            // resolves the adjacent .wasm file itself under Bun, so the
            // no-arg form works correctly.
            await wasmInit();
            console.log('✅ WASM initialized successfully for tests');
            wasmInitialized = true;
        } catch (error) {
            console.warn('⚠️ WASM initialization failed, tests will use fallback simulation:', error);
        }
    }
    return wasmInitialized;
}

export function isWasmInitialized(): boolean {
    return wasmInitialized;
}

export function hexEncode(str: string): string {
    return Buffer.from(str, 'utf8').toString('hex');
}

// Extract package from DKG package map for specific recipient.
//
// The WASM's generate_round2 output layout:
//   outer = hex-encoded JSON of a map
//   inner keys  = numeric u16 participant indices (JSON stringifies to "1", "2", ...)
//   inner values = hex-encoded JSON of the per-peer round2 Package
//
// Earlier versions of this helper tried to reconstruct hex-encoded
// 32-byte FROST identifiers and look those up as keys — but the WASM
// never emitted that shape, so the lookup always missed. Use the
// numeric-string key directly. The second arg is kept only to avoid
// churn in callers; it has no effect.
export function extractPackageFromMap(recipientIndex: number, packageMapHex: string, _isSecp256k1: boolean): string {
    const packageMap = JSON.parse(Buffer.from(packageMapHex, 'hex').toString());
    const key = String(recipientIndex);
    const individualPackageHex = packageMap[key];
    if (!individualPackageHex) {
        throw new Error(`Package for recipient ${recipientIndex} (key "${key}") not found in map ${JSON.stringify(packageMap)}`);
    }
    return individualPackageHex;
}

// Common session info for tests
export const createTestSessionInfo = () => ({
    session_id: 'test-session',
    participants: ['a', 'b', 'c'],
    accepted_devices: ['a', 'b', 'c'],
    total: 3,
    threshold: 2
});

// Dummy send function for WebRTCManager
export const dummySend = (_todeviceId: string, _message: any) => { };

// Create mock data channel for testing
export const createMockDataChannel = () => ({
    readyState: 'open',
    send: () => { },
    close: () => { },
    addEventListener: () => { },
    removeEventListener: () => { }
});

// Create and initialize FROST DKG instances for testing
export async function createTestDkgInstances(isSecp256k1: boolean = false): Promise<{
    frostDkgA: FrostDkgEd25519 | FrostDkgSecp256k1,
    frostDkgB: FrostDkgEd25519 | FrostDkgSecp256k1,
    frostDkgC: FrostDkgEd25519 | FrostDkgSecp256k1
}> {
    if (!await initializeWasmIfNeeded()) {
        throw new Error('WASM not initialized for test DKG instances');
    }

    let frostDkgA, frostDkgB, frostDkgC;

    if (isSecp256k1) {
        frostDkgA = new FrostDkgSecp256k1();
        frostDkgB = new FrostDkgSecp256k1();
        frostDkgC = new FrostDkgSecp256k1();
    } else {
        frostDkgA = new FrostDkgEd25519();
        frostDkgB = new FrostDkgEd25519();
        frostDkgC = new FrostDkgEd25519();
    }

    // Initialize all instances
    frostDkgA.init_dkg(1, 3, 2);
    frostDkgB.init_dkg(2, 3, 2);
    frostDkgC.init_dkg(3, 3, 2);

    return { frostDkgA, frostDkgB, frostDkgC };
}

// Cleanup DKG instances
export function cleanupDkgInstances(...instances: Array<FrostDkgEd25519 | FrostDkgSecp256k1 | null>) {
    instances.forEach(instance => {
        if (instance) {
            try {
                instance.free();
            } catch (error) {
                console.warn('Error freeing DKG instance:', error);
            }
        }
    });
}
