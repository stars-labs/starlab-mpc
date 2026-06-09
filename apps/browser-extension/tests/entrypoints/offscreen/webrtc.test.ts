import { Buffer } from 'buffer';
import wasmInit from '@starlab/core-wasm';
import { beforeAll } from 'bun:test';

// Export helper functions for use in other test files
export function hexEncode(str: string): string {
    return Buffer.from(str, 'utf8').toString('hex');
}

// Extract a specific participant's package from a Round 2 package map.
//
// The WASM's generate_round2 output is hex-encoded JSON whose keys are
// numeric u16 participant indices (JSON stringifies to "1", "2", ...)
// and whose values are the hex-encoded round2 Package for each peer.
// Earlier versions of this helper tried to construct a 32-byte hex
// identifier as the key — the WASM never emitted that shape, so every
// lookup missed. Second arg kept for call-site compatibility only.
export function extractPackageFromMap(recipientIndex: number, packageMapHex: string, _isSecp256k1: boolean): string {
    const packageMap = JSON.parse(Buffer.from(packageMapHex, 'hex').toString());
    const key = String(recipientIndex);
    const individualPackageHex = packageMap[key];
    if (!individualPackageHex) {
        throw new Error(`Package for recipient ${recipientIndex} (key "${key}") not found in map ${JSON.stringify(packageMap)}`);
    }
    return individualPackageHex;
}

// Initialize WASM once before all tests
let wasmInitialized = false;

beforeAll(async () => {
    if (!wasmInitialized) {
        try {
            await wasmInit();
            console.log('✅ WASM initialized successfully for tests');
            wasmInitialized = true;
        } catch (error) {
            console.warn('⚠️ WASM initialization failed, tests will use fallback simulation:', error);
        }
    }
});

// Export for use in other test files
export { wasmInitialized };

// Note: The actual test suites for WebRTCManager DKG Process and mesh readiness
// are now in their respective files:
// - webrtc.dkg.test.ts for DKG process tests
// - webrtc.mesh.test.ts for mesh readiness tests