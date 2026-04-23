import { Buffer } from 'buffer';
import wasmInit, { FrostDkgEd25519, FrostDkgSecp256k1 } from '@mpc-wallet/core-wasm';
import fs from 'fs';
import path from 'path';

// Initialize WASM once for all tests
let wasmInitialized = false;

export async function initializeWasmIfNeeded(): Promise<boolean> {
    if (!wasmInitialized) {
        try {
            // For test environment, pass the WASM ArrayBuffer directly
            if (typeof window === 'undefined' && typeof global !== 'undefined') {
                const wasmPath = path.join(__dirname, '../../../pkg/mpc_wallet_bg.wasm');
                const wasmBuffer = fs.readFileSync(wasmPath);
                // Convert Node.js Buffer to ArrayBuffer
                const arrayBuffer = new ArrayBuffer(wasmBuffer.length);
                const view = new Uint8Array(arrayBuffer);
                for (let i = 0; i < wasmBuffer.length; i++) {
                    view[i] = wasmBuffer[i];
                }
                await wasmInit(arrayBuffer);
            } else {
                await wasmInit();
            }
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

// Helper function to convert participant index to the hex key format used in DKG package maps
export function participantIndexToHexKey(index: number, isSecp256k1: boolean): string {
    const buffer = Buffer.alloc(32); // FROST identifiers are 32 bytes
    if (isSecp256k1) {
        // For Secp256k1, observed keys are like "00...0001" for index 1.
        buffer.writeUInt32BE(index, 28); // Write 4 bytes (u32) at offset 28 for a 32-byte buffer
    } else {
        // For Ed25519, observed keys are like "010000..." for index 1.
        buffer.writeUInt16LE(index, 0);  // Write 2 bytes (u16) at offset 0
    }
    return buffer.toString('hex');
}

// Extract package from DKG package map for specific recipient
export function extractPackageFromMap(recipientIndex: number, packageMapHex: string, isSecp256k1: boolean): string {
    const packageMap = JSON.parse(Buffer.from(packageMapHex, 'hex').toString());
    let hexKey: string;
    if (isSecp256k1) {
        // For secp256k1, the key is 32 bytes, recipient index in the LAST 4 bytes (big-endian u32)
        const buffer = Buffer.alloc(32);
        buffer.writeUInt32BE(recipientIndex, 28); // Write u32 at offset 28 for big-endian
        hexKey = buffer.toString('hex');
    } else {
        // For ed25519, the key is 32 bytes, recipient index in the FIRST 2 bytes (little-endian u16)
        const buffer = Buffer.alloc(32);
        buffer.writeUInt16LE(recipientIndex, 0); // Write u16 at offset 0 for little-endian
        hexKey = buffer.toString('hex');
    }
    const individualPackage = packageMap[hexKey];
    if (!individualPackage) {
        throw new Error(`Package for recipient ${recipientIndex} (key ${hexKey}, isSecp256k1: ${isSecp256k1}) not found in map ${JSON.stringify(packageMap)}`);
    }
    // Return hex string instead of JSON string for WASM compatibility
    const packageJson = JSON.stringify(individualPackage);
    return Buffer.from(packageJson, 'utf8').toString('hex');
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
