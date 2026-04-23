import { Buffer } from 'buffer';
import wasmInit from '@mpc-wallet/core-wasm';
import { beforeAll } from 'bun:test';

// Export helper functions for use in other test files
export function hexEncode(str: string): string {
    return Buffer.from(str, 'utf8').toString('hex');
}

// Helper function to convert participant index to the hex key format used in DKG package maps
export function participantIndexToHexKey(index: number, isSecp256k1: boolean): string {
    const buffer = Buffer.alloc(32); // FROST identifiers are 32 bytes
    if (isSecp256k1) {
        // For Secp256k1, observed keys are like "00...0001" for index 1.
        // This implies index is treated as u32 big-endian at the end of the buffer for secp256k1 based on typical C implementations or specific library choices.
        // Let's try u32 big-endian, as u16 might be too small if participant count grows, though current examples show small indices.
        // The error log showed "0000...0001" for participant 1, which is more like a 4-byte representation if it's at the end.
        buffer.writeUInt32BE(index, 28); // Write 4 bytes (u32) at offset 28 for a 32-byte buffer
    } else {
        // For Ed25519, observed keys are like "010000..." for index 1.
        // This implies index is treated as u16 little-endian at the start of the buffer.
        buffer.writeUInt16LE(index, 0);  // Write 2 bytes (u16) at offset 0
    }
    return buffer.toString('hex');
}

// Helper function to extract a specific participant's package from a Round 2 map
export function extractPackageFromMap(recipientIndex: number, packageMapHex: string, isSecp256k1: boolean): string {
    const packageMap = JSON.parse(Buffer.from(packageMapHex, 'hex').toString());
    const hexKey = participantIndexToHexKey(recipientIndex, isSecp256k1);
    const individualPackage = packageMap[hexKey];
    if (!individualPackage) {
        throw new Error(`Package for recipient ${recipientIndex} (key ${hexKey}, isSecp256k1: ${isSecp256k1}) not found in map ${JSON.stringify(packageMap)}`);
    }
    return Buffer.from(JSON.stringify(individualPackage)).toString('hex');
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