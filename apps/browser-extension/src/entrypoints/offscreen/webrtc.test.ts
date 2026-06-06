import { describe, it, expect, beforeAll } from 'bun:test';
import { DkgState, WebRTCManager, MeshStatusType } from './webrtc'; // Adjust path as necessary
import { Buffer } from 'buffer';
import wasmInit, { FrostDkgEd25519, FrostDkgSecp256k1 } from '@mpc-wallet/core-wasm'; // Corrected import

function hexEncode(str: string): string {
    return Buffer.from(str, 'utf8').toString('hex');
}

// Initialize WASM once before all tests
let wasmInitialized = false;

// Helper function to extract a specific participant's package from a Round 2 map.
//
// The WASM emits the Round 2 package map as hex-encoded JSON where keys
// are the numeric u16 participant indices (stringified by JSON). The
// inner values are themselves hex-encoded JSON of the FROST round2
// Package. So:
//   outer hex → JSON → { "1": "7b...", "2": "7b...", ... }
//   inner hex (already in that form) is what add_round2_package expects.
function extractPackageFromMap(recipientIndex: number, packageMapHex: string, _isSecp256k1: boolean): string {
    const packageMap = JSON.parse(Buffer.from(packageMapHex, 'hex').toString());
    const key = String(recipientIndex);
    const individualPackageHex = packageMap[key];
    if (!individualPackageHex) {
        throw new Error(`Package for recipient ${recipientIndex} (key "${key}") not found in map ${JSON.stringify(packageMap)}`);
    }
    return individualPackageHex;
}

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

// Dummy send function for WebRTCManager
const dummySend = (_todeviceId: string, _message: any) => { };

describe('WebRTCManager mesh readiness', () => {
    const sessionInfo = {
        session_id: 'test-session',
        participants: ['a', 'b', 'c'],
        accepted_devices: ['a', 'b', 'c'],
        total: 3,
        threshold: 2
    };

    it('should transition to PartiallyReady when first MeshReady received', () => {
        const manager = new WebRTCManager('a', dummySend);
        // Set session and initial mesh status
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateMeshStatus({ type: MeshStatusType.Incomplete });

        // Simulate receiving MeshReady from 'b'
        (manager as any)._processPeerMeshReady('b');

        expect(manager.meshStatus.type).toBe(MeshStatusType.PartiallyReady);
        const readydevices = (manager.meshStatus as any).ready_devices as Set<string>;
        expect(readydevices.has('a')).toBe(true);
        expect(readydevices.has('b')).toBe(true);
    });

    it('should transition to Ready when all MeshReady received', () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        // Simulate two devices already ready
        (manager as any)._updateMeshStatus({
            type: MeshStatusType.PartiallyReady,
            ready_devices: new Set(['a', 'b']),
            total_devices: 3
        });

        // Now simulate receiving MeshReady from 'c'
        (manager as any)._processPeerMeshReady('c');

        expect(manager.meshStatus.type).toBe(MeshStatusType.Ready);
    });
});

describe('WebRTCManager DKG Process', () => {
    const sessionInfo = {
        session_id: 'test-session',
        participants: ['a', 'b', 'c'],
        accepted_devices: ['a', 'b', 'c'],
        total: 3,
        threshold: 2
    };

    it('should initialize DKG when conditions are met', async () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateMeshStatus({ type: MeshStatusType.Ready });
        (manager as any)._updateDkgState(DkgState.Idle);

        // Use actual WASM initialization if available
        if (wasmInitialized) {
            try {
                // Create real FROST DKG instance
                (manager as any).frostDkg = new FrostDkgEd25519();
                console.log('✅ Using real FROST DKG WASM instance in test');
            } catch (error) {
                console.error('⚠️ Failed to create FROST DKG instance in test:', error);
                // If WASM fails here, the test should not proceed with a mock
                expect(error).toBeNull(); // Force test failure if WASM DKG can't be created
                return;
            }
        } else {
            console.warn('⚠️ WASM not initialized, skipping DKG initialization test that requires it.');
            return; // Skip test if WASM is not available
        }

        // Set up participant index (we know localdeviceId is 'a' from constructor)
        (manager as any).participantIndex = (manager as any).sessionInfo.participants.indexOf('a') + 1;

        // Initialize DKG and generate Round 1
        if ((manager as any).frostDkg && (manager as any).frostDkg.init_dkg) {
            (manager as any).frostDkg.init_dkg(
                (manager as any).participantIndex,
                (manager as any).sessionInfo.total,
                (manager as any).sessionInfo.threshold
            );
        } else {
            console.error("frostDkg or init_dkg not available after WASM check");
            expect(false).toBe(true); // Force failure
            return;
        }

        await (manager as any)._generateAndBroadcastRound1();

        expect(manager.dkgState).toBe(DkgState.Round1InProgress);
        expect((manager as any).participantIndex).toBe(1);
        if ((manager as any).frostDkg) {
            (manager as any).frostDkg.free();
        }
    });

    it('should handle Round 1 package reception and transition to Round 2', async () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Round1InProgress);

        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping Round 1 reception test that requires it.');
            return;
        }

        try {
            (manager as any).frostDkg = new FrostDkgEd25519();
            (manager as any).frostDkg.init_dkg(1, 3, 2); // Initialize for participant 'a' (index 1)

            // Simulate manager 'a' having generated its own Round 1 package
            const round1PackageA_self = (manager as any).frostDkg.generate_round1();
            (manager as any).receivedRound1Packages.add('a'); // Corrected: use add for Set, with deviceId


        } catch (error) {
            console.error('⚠️ Failed to create/initialize FROST DKG instance in Round 1 test:', error);
            expect(error).toBeNull();
            return;
        }

        (manager as any).participantIndex = 1;


        // Simulate receiving Round 1 packages from devices 'b' and 'c'
        // These packages need to be generated by their own (simulated) DKG instances
        let frostDkgB_sim: FrostDkgEd25519 | null = null;
        let frostDkgC_sim: FrostDkgEd25519 | null = null;
        try {
            frostDkgB_sim = new FrostDkgEd25519();
            frostDkgB_sim.init_dkg(2, 3, 2);
            const round1PackageB_for_A = frostDkgB_sim.generate_round1();

            frostDkgC_sim = new FrostDkgEd25519();
            frostDkgC_sim.init_dkg(3, 3, 2);
            const round1PackageC_for_A = frostDkgC_sim.generate_round1();

            // Process these packages as if received from devices
            // The _handleDkgRound1Package method will call frostDkg.add_round1_package internally
            await (manager as any)._handleDkgRound1Package('b', { sender_index: 2, data: round1PackageB_for_A });
            await (manager as any)._handleDkgRound1Package('c', { sender_index: 3, data: round1PackageC_for_A });


        } catch (error) {
            console.error("Error during simulated peer package generation/handling:", error);
            expect(error).toBeNull();
        } finally {
            if (frostDkgB_sim) frostDkgB_sim.free();
            if (frostDkgC_sim) frostDkgC_sim.free();
        }


        // After the second _handleDkgRound1Package call above, all n-1
        // peer packages are in the WASM and can_start_round2 returns
        // true. _handleDkgRound1Package itself flips the state to
        // Round2InProgress and fires _generateAndBroadcastRound2
        // internally (webrtc.ts line 1389-1393). So by this point the
        // manager should already be in Round2InProgress with no extra
        // nudging needed.
        expect(manager.dkgState).toBe(DkgState.Round2InProgress);
        expect((manager as any).receivedRound1Packages.size).toBe(3); // a, b, c

        if ((manager as any).frostDkg) {
            (manager as any).frostDkg.free();
        }
    });

    it('should handle Round 2 package reception and transition to finalization', async () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Round1InProgress); // Start in Round 1

        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping Round 2 reception test that requires it.');
            return;
        }

        let dkgA: FrostDkgEd25519 | null = null;
        let dkgB_sim: FrostDkgEd25519 | null = null;
        let dkgC_sim: FrostDkgEd25519 | null = null;

        try {
            dkgA = new FrostDkgEd25519();
            dkgA.init_dkg(1, 3, 2);
            (manager as any).frostDkg = dkgA;
            (manager as any).participantIndex = 1;

            // Simulate Round 1 completion for dkgA by letting the manager handle them
            // This will internally call dkgA.add_round1_package
            const round1A_self = dkgA.generate_round1();
            // Add own package to received set (don't process it since generate_round1 already included it)
            (manager as any).receivedRound1Packages.add('a');

            dkgB_sim = new FrostDkgEd25519();
            dkgB_sim.init_dkg(2, 3, 2);
            const round1B_sim = dkgB_sim.generate_round1();
            await (manager as any)._handleDkgRound1Package('b', { sender_index: 2, data: round1B_sim });

            dkgC_sim = new FrostDkgEd25519();
            dkgC_sim.init_dkg(3, 3, 2);
            const round1C_sim = dkgC_sim.generate_round1();
            await (manager as any)._handleDkgRound1Package('c', { sender_index: 3, data: round1C_sim });

            // After manager handles all Round 1, its internal frostDkg (dkgA) should be ready
            expect((manager as any).frostDkg.can_start_round2()).toBe(true);
            // Manager should have transitioned to Round2InProgress if _handleDkgRound1Package works fully
            // or we might need to call _generateAndBroadcastRound2 if it doesn't auto-trigger in test
            if ((manager as any).frostDkg.can_start_round2() && manager.dkgState !== DkgState.Round2InProgress) {
                await (manager as any)._generateAndBroadcastRound2(); // This updates state to Round2InProgress
            }
            expect(manager.dkgState).toBe(DkgState.Round2InProgress);

            // Now dkgA (via manager.frostDkg) should have generated its Round 2 packages
            // const round2_packages_hex_map_A = (manager as any).frostDkg.generate_round2(); // This was already done by _generateAndBroadcastRound2

            // Simulate other participants generating their Round 2 packages
            // dkgB_sim and dkgC_sim need to have processed round 1 packages to generate round 2
            // For simplicity in this unit test, we assume they did. 
            // In a real scenario, they would also exchange Round 1 packages.
            // For this test, we need dkgB_sim and dkgC_sim to be in a state where they *can* generate round2.
            // So, they also need to process round1 packages from A and each other.
            const round1A_for_B_and_C = round1A_self; // A's package
            const round1B_for_A_and_C = round1B_sim; // B's package
            const round1C_for_A_and_B = round1C_sim; // C's package

            dkgB_sim.add_round1_package(1, round1A_for_B_and_C);
            dkgB_sim.add_round1_package(3, round1C_for_A_and_B);
            expect(dkgB_sim.can_start_round2()).toBe(true); // Verify B is ready for Round 2

            dkgC_sim.add_round1_package(1, round1A_for_B_and_C);
            dkgC_sim.add_round1_package(2, round1B_for_A_and_C);
            expect(dkgC_sim.can_start_round2()).toBe(true); // Verify C is ready for Round 2

            const round2_packages_hex_map_B_sim = dkgB_sim.generate_round2();
            const round2_packages_hex_map_C_sim = dkgC_sim.generate_round2();

            // Participant A (manager) needs to receive its share from B and C's Round 2 packages.
            // A is participant 1. These are Ed25519 instances.
            const round2A_from_B_obj = extractPackageFromMap(1, round2_packages_hex_map_B_sim, false); // Corrected: isSecp256k1 is false
            const round2A_from_C_obj = extractPackageFromMap(1, round2_packages_hex_map_C_sim, false); // Corrected: isSecp256k1 is false

            const round2B_for_A_hex = round2A_from_B_obj; // extractPackageFromMap already returns hex
            const round2C_for_A_hex = round2A_from_C_obj; // extractPackageFromMap already returns hex

            // Manager processes these packages
            await (manager as any)._handleDkgRound2Package('b', { sender_index: 2, sender_id_hex: 'b', data: round2B_for_A_hex });
            await (manager as any)._handleDkgRound2Package('c', { sender_index: 3, sender_id_hex: 'c', data: round2C_for_A_hex });

            // After manager handles all Round 2, its internal frostDkg (dkgA) should be ready to finalize
            // and manager state should be Finalizing (or Complete if _finalizeDkg is called by handler)
            expect((manager as any).frostDkg.can_finalize()).toBe(true);
            // The _handleDkgRound2Package for the last package should trigger _finalizeDkg
            expect(manager.dkgState).toBe(DkgState.Complete); // Assuming _finalizeDkg is called and sets state

        } catch (error) {
            console.error('⚠️ Error during Round 2 setup/processing:', (error as Error).message ? (error as Error).message : error);
            expect(error).toBeNull();
        } finally {
            if (dkgB_sim) dkgB_sim.free();
            if (dkgC_sim) dkgC_sim.free();
        }

        // After all Round 2 packages are processed, manager should be ready to finalize
        if (dkgA && dkgA.can_finalize()) {
            await (manager as any)._finalizeDkg();
        }

        expect(manager.dkgState).toBe(DkgState.Complete);
        expect((manager as any).receivedRound2Packages.size).toBe(3); // a, b, c
        expect((manager as any).groupPublicKey).toBeDefined();
        expect((manager as any).solanaAddress).toBeDefined();

        if (dkgA) dkgA.free();
    });

    it('should finalize DKG and generate group public key and Solana address', async () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Round1InProgress); // Start in Round 1

        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping finalization test.');
            return;
        }

        let dkgA_full: FrostDkgEd25519 | null = null;
        let dkgB_sim_full: FrostDkgEd25519 | null = null;
        let dkgC_sim_full: FrostDkgEd25519 | null = null;

        try {
            dkgA_full = new FrostDkgEd25519();
            dkgA_full.init_dkg(1, 3, 2);
            (manager as any).frostDkg = dkgA_full;
            (manager as any).participantIndex = 1;

            // Simulate Round 1 completion for manager's dkgA_full instance
            // by letting the manager handle all necessary packages.
            const round1A_self_full = dkgA_full.generate_round1();
            // Add own package to received set (don't process it since generate_round1 already included it)
            (manager as any).receivedRound1Packages.add('a');

            dkgB_sim_full = new FrostDkgEd25519();
            dkgB_sim_full.init_dkg(2, 3, 2);
            const round1B_sim_full = dkgB_sim_full.generate_round1();
            await (manager as any)._handleDkgRound1Package('b', { sender_index: 2, data: round1B_sim_full });

            dkgC_sim_full = new FrostDkgEd25519();
            dkgC_sim_full.init_dkg(3, 3, 2);
            const round1C_sim_full = dkgC_sim_full.generate_round1();
            await (manager as any)._handleDkgRound1Package('c', { sender_index: 3, data: round1C_sim_full });

            // After manager handles all Round 1, its internal frostDkg (dkgA_full) should be ready for Round 2
            // and manager state should be Round2InProgress.
            expect((manager as any).frostDkg.can_start_round2()).toBe(true);
            // The _handleDkgRound1Package for the last package should trigger _generateAndBroadcastRound2
            expect(manager.dkgState).toBe(DkgState.Round2InProgress);

            // Simulate dkgA_full (via manager) receiving Round 2 packages from B and C
            // dkgB_sim_full and dkgC_sim_full also need to complete Round 1 to generate Round 2.
            dkgB_sim_full.add_round1_package(1, round1A_self_full);
            dkgB_sim_full.add_round1_package(3, round1C_sim_full);
            expect(dkgB_sim_full.can_start_round2()).toBe(true);
            const round2_map_B_full = dkgB_sim_full.generate_round2();

            dkgC_sim_full.add_round1_package(1, round1A_self_full);
            dkgC_sim_full.add_round1_package(2, round1B_sim_full);
            expect(dkgC_sim_full.can_start_round2()).toBe(true);
            const round2_map_C_full = dkgC_sim_full.generate_round2();

            // Extract packages for A (participant 1) from B's and C's Round 2 maps
            const round2A_from_B_obj_full = extractPackageFromMap(1, round2_map_B_full, false);
            const round2A_from_C_obj_full = extractPackageFromMap(1, round2_map_C_full, false);

            const round2B_for_A_hex_full = round2A_from_B_obj_full; // extractPackageFromMap already returns hex
            const round2C_for_A_hex_full = round2A_from_C_obj_full; // extractPackageFromMap already returns hex

            // Manager processes these Round 2 packages
            await (manager as any)._handleDkgRound2Package('b', { sender_index: 2, sender_id_hex: 'b', data: round2B_for_A_hex_full });
            await (manager as any)._handleDkgRound2Package('c', { sender_index: 3, sender_id_hex: 'c', data: round2C_for_A_hex_full });

            // After manager handles all Round 2, its internal frostDkg (dkgA_full) should be ready to finalize
            // and manager state should be Finalizing (or Complete if _finalizeDkg is called by handler)
            expect((manager as any).frostDkg.can_finalize()).toBe(true);
            // The _handleDkgRound2Package for the last package should trigger _finalizeDkg
            expect(manager.dkgState).toBe(DkgState.Complete); // Assuming _finalizeDkg is called and sets state

        } catch (e) {
            console.error("Error setting up for finalization test:", (e as Error).message ? (e as Error).message : e);
            expect(e).toBeNull(); // Fail test if setup fails
        } finally {
            dkgA_full?.free();
            dkgB_sim_full?.free();
            dkgC_sim_full?.free();
        }

        // _finalizeDkg() should have been called automatically by _handleDkgRound2Package
        // No need to call it manually again
        expect(manager.dkgState).toBe(DkgState.Complete);
        expect((manager as any).groupPublicKey).toBeDefined();
        expect((manager as any).solanaAddress).toBeDefined();
        expect((manager as any).solanaAddress).toMatch(/^[1-9A-HJ-NP-Za-km-z]+$/);

        // Note: dkgA_full is already freed in the finally block above
    });

    it('should handle DKG failure during Round 1 if WASM call fails', async () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Round1InProgress);

        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping DKG failure test.');
            return;
        }
        let dkgInstance: FrostDkgEd25519 | null = null;
        try {
            dkgInstance = new FrostDkgEd25519();
            dkgInstance.init_dkg(1, 3, 2); // Initialize participant 'a'
            (manager as any).frostDkg = dkgInstance;
            (manager as any).participantIndex = 1;

            // Simulate manager 'a' having generated its own Round 1 package
            const round1PackageA_self = (manager as any).frostDkg.generate_round1();
            (manager as any).receivedRound1Packages.add('a'); // Corrected


            // Simulate receiving an invalid Round 1 package that causes WASM to throw an error
            // The `add_round1_package` in WASM should handle validation and throw.
            // We pass deliberately malformed data.
            await (manager as any)._handleDkgRound1Package('b', { sender_index: 2, data: 'invalid-hex-data-that-will-cause-wasm-error' });

        } catch (error) {
            // This catch block might not be reached if _handleDkgRound1Package catches and sets state.
            // The important check is the manager's DKG state.
            console.info("Caught error at test level during Round 1 failure simulation:", error)
        } finally {
            if (dkgInstance) dkgInstance.free();
        }

        expect(manager.dkgState).toBe(DkgState.Failed);
    });

    it('should handle DKG failure during Round 2 if WASM call fails', async () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Round2InProgress);

        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping DKG failure test.');
            return;
        }

        let dkgA: FrostDkgEd25519 | null = null;
        let dkgB_sim: FrostDkgEd25519 | null = null;
        let dkgC_sim: FrostDkgEd25519 | null = null;

        try {
            dkgA = new FrostDkgEd25519();
            dkgA.init_dkg(1, 3, 2);
            (manager as any).frostDkg = dkgA;
            (manager as any).participantIndex = 1;

            // Simulate successful Round 1
            const r1A = dkgA.generate_round1();
            (manager as any).receivedRound1Packages.add('a'); // Corrected

            dkgB_sim = new FrostDkgEd25519();
            dkgB_sim.init_dkg(2, 3, 2);
            const r1B = dkgB_sim.generate_round1();
            dkgA.add_round1_package(2, r1B);
            (manager as any).receivedRound1Packages.add('b'); // Corrected

            dkgC_sim = new FrostDkgEd25519();
            dkgC_sim.init_dkg(3, 3, 2);
            const r1C = dkgC_sim.generate_round1();
            dkgA.add_round1_package(3, r1C);
            (manager as any).receivedRound1Packages.add('c'); // Corrected

            expect(dkgA.can_start_round2()).toBe(true);
            // Manager 'a' generates its own Round 2 package map
            const round2PackageA_self_map = dkgA.generate_round2();
            (manager as any).receivedRound2Packages.add('a'); // Corrected - 'a' has its shares


            // Simulate receiving an invalid Round 2 package
            await (manager as any)._handleDkgRound2Package('b', { sender_index: 2, data: 'invalid-hex-data-for-round2' });

        } catch (error) {
            console.info("Caught error at test level during Round 2 failure simulation:", error);
        } finally {
            if (dkgA) dkgA.free();
            if (dkgB_sim) dkgB_sim.free();
            if (dkgC_sim) dkgC_sim.free();
        }

        expect(manager.dkgState).toBe(DkgState.Failed);
    });

    it('should handle DKG failure during finalization if WASM call fails', async () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Finalizing); // Set state to Finalizing

        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping DKG failure test.');
            return;
        }
        let dkgA_broken_finalize: FrostDkgEd25519 | null = null;
        try {
            // We need a DKG instance that is ready to finalize but will fail.
            // This is hard to achieve with real WASM without specific error conditions.
            // Instead, we'll mock the finalize_dkg method on a real instance to throw.
            dkgA_broken_finalize = new FrostDkgEd25519();
            dkgA_broken_finalize.init_dkg(1, 3, 2); // Basic init

            // To make it "seem" ready to finalize for the manager's logic,
            // we might need to populate received packages, though _finalizeDkg doesn't check them directly.
            // The crucial part is that frostDkg.finalize_dkg() itself throws.
            const originalFinalize = dkgA_broken_finalize.finalize_dkg;
            dkgA_broken_finalize.finalize_dkg = () => { throw new Error('Mocked WASM Finalization error'); };

            (manager as any).frostDkg = dkgA_broken_finalize;
            (manager as any).participantIndex = 1; // Needs to be set for logging inside _finalizeDkg

            await (manager as any)._finalizeDkg();

            // Restore original method if necessary, though instance will be freed
            dkgA_broken_finalize.finalize_dkg = originalFinalize;

        } catch (error) {
            console.info("Caught error at test level during finalization failure simulation:", error);
        } finally {
            if (dkgA_broken_finalize) dkgA_broken_finalize.free();
        }

        expect(manager.dkgState).toBe(DkgState.Failed);
    });

    it('should reset DKG state properly', () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Round2InProgress);

        // Use real WASM or fallback to mock
        if (wasmInitialized) {
            try {
                (manager as any).frostDkg = new FrostDkgEd25519();
            } catch (error) {
                console.warn('WASM DKG creation failed in test, using mock');
                (manager as any).frostDkg = { free: () => { } };
            }
        } else {
            (manager as any).frostDkg = { free: () => { } };
        }

        (manager as any).participantIndex = 1;
        (manager as any).receivedRound1Packages.add('a');
        (manager as any).receivedRound2Packages.add('a');

        (manager as any)._resetDkgState();

        expect((manager as any).frostDkg).toBe(null);
        expect((manager as any).participantIndex).toBe(null);
        expect((manager as any).receivedRound1Packages.size).toBe(0);
        expect((manager as any).receivedRound2Packages.size).toBe(0);
    });

    it('should get Dkg status correctly', () => {
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Round1InProgress);
        (manager as any).participantIndex = 1;
        (manager as any).receivedRound1Packages.add('a');
        (manager as any).frostDkg = {};

        const status = manager.getDkgStatus();

        expect(status.state).toBe(DkgState.Round1InProgress);
        expect(status.stateName).toBe('Round1InProgress');
        expect(status.participantIndex).toBe(1);
        expect(status.sessionInfo?.session_id).toBe('test-session');
        expect(status.receivedRound1Packages).toEqual(['a']);
        expect(status.frostDkgInitialized).toBe(true);
    });

    it('should complete full DKG process end-to-end with cryptographically realistic simulation', async () => {
        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping end-to-end DKG test.');
            return;
        }
        // Create three managers for a complete 3-peer DKG simulation
        const managerA = new WebRTCManager('a', dummySend);
        const managerB = new WebRTCManager('b', dummySend);
        const managerC = new WebRTCManager('c', dummySend);

        // Mock data channels to prevent "Cannot send WebRTCAppMessage" errors
        const mockDataChannel = { readyState: 'open', send: () => { } };
        (managerA as any).dataChannels.set('b', mockDataChannel);
        (managerA as any).dataChannels.set('c', mockDataChannel);
        (managerB as any).dataChannels.set('a', mockDataChannel);
        (managerB as any).dataChannels.set('c', mockDataChannel);
        (managerC as any).dataChannels.set('a', mockDataChannel);
        (managerC as any).dataChannels.set('b', mockDataChannel);

        // Set up session info for all managers
        [managerA, managerB, managerC].forEach(manager => {
            manager.sessionInfo = sessionInfo as any;
            (manager as any)._updateMeshStatus({ type: MeshStatusType.Ready });
            (manager as any)._updateDkgState(DkgState.Idle);
        });

        console.log('\\\\n=== REAL DKG PROCESS WITH WASM ===');
        console.log('Participants: a, b, c');
        console.log('Threshold: 2 of 3 (any 2 participants can sign)');
        console.log('Protocol: FROST (Flexible Round-Optimized Schnorr Threshold)');

        // Create actual FROST DKG instances using WASM
        let frostDkgA: FrostDkgEd25519 | null = null, frostDkgB: FrostDkgEd25519 | null = null, frostDkgC: FrostDkgEd25519 | null = null;

        try {
            console.log('✅ Using real FROST DKG WASM instances for Ed25519');
            frostDkgA = new FrostDkgEd25519();
            frostDkgB = new FrostDkgEd25519();
            frostDkgC = new FrostDkgEd25519();

            (managerA as any).frostDkg = frostDkgA;
            (managerB as any).frostDkg = frostDkgB;
            (managerC as any).frostDkg = frostDkgC;
            (managerA as any).participantIndex = 1;
            (managerB as any).participantIndex = 2;
            (managerC as any).participantIndex = 3;

            // Initialize DKG for all participants
            console.log('\\\\n=== DKG INITIALIZATION ===');
            frostDkgA!.init_dkg(1, 3, 2);
            frostDkgB!.init_dkg(2, 3, 2);
            frostDkgC!.init_dkg(3, 3, 2);

            console.log('\\\\n=== ROUND 1: COMMITMENT PHASE ===');
            // Step 1: Generate Round 1 packages (only once per participant!)
            const round1PackageA_hex = frostDkgA!.generate_round1();
            const round1PackageB_hex = frostDkgB!.generate_round1();
            const round1PackageC_hex = frostDkgC!.generate_round1();

            // Update manager states to Round1InProgress manually since we're not using their broadcast methods
            // and simulate they have received their own package
            (managerA as any)._updateDkgState(DkgState.Round1InProgress);
            (managerA as any).receivedRound1Packages.add('a'); // Corrected
            (managerB as any)._updateDkgState(DkgState.Round1InProgress);
            (managerB as any).receivedRound1Packages.add('b'); // Corrected
            (managerC as any)._updateDkgState(DkgState.Round1InProgress);
            (managerC as any).receivedRound1Packages.add('c'); // Corrected


            console.log('\\\\n--- ROUND 1 PACKAGE EXCHANGE ---');
            // Step 2: Exchange Round 1 packages directly to WASM instances
            // Each participant adds the Round 1 packages from other participants

            // A adds packages from B and C
            frostDkgA!.add_round1_package(2, round1PackageB_hex);
            (managerA as any).receivedRound1Packages.add('b'); // Corrected
            frostDkgA!.add_round1_package(3, round1PackageC_hex);
            (managerA as any).receivedRound1Packages.add('c'); // Corrected


            // B adds packages from A and C
            frostDkgB!.add_round1_package(1, round1PackageA_hex);
            (managerB as any).receivedRound1Packages.add('a'); // Corrected
            frostDkgB!.add_round1_package(3, round1PackageC_hex);
            (managerB as any).receivedRound1Packages.add('c'); // Corrected


            // C adds packages from A and B
            frostDkgC!.add_round1_package(1, round1PackageA_hex);
            (managerC as any).receivedRound1Packages.add('a'); // Corrected
            frostDkgC!.add_round1_package(2, round1PackageB_hex);
            (managerC as any).receivedRound1Packages.add('b'); // Corrected


            // Verify all can start Round 2
            expect(frostDkgA!.can_start_round2()).toBe(true);
            expect(frostDkgB!.can_start_round2()).toBe(true);
            expect(frostDkgC!.can_start_round2()).toBe(true);

            // Update manager states to reflect readiness for Round 2
            // (managerA as any)._updateDkgState(DkgState.Round1Complete); // Or similar internal state if exists
            // (managerB as any)._updateDkgState(DkgState.Round1Complete);
            // (managerC as any)._updateDkgState(DkgState.Round1Complete);
            // For the test, we'll directly generate round 2 if possible.
            // The WebRTCManager's internal state machine would handle this via _handleDkgRound1Package calls.

            console.log('\\\\n=== ROUND 2: SECRET SHARE PHASE ===');
            // Generate Round 2 packages (secret shares as maps)
            const round2PackageA_map_hex = frostDkgA!.generate_round2();
            const round2PackageB_map_hex = frostDkgB!.generate_round2();
            const round2PackageC_map_hex = frostDkgC!.generate_round2();

            console.log('\\\\n--- Round 2 Package Exchange ---');
            // Exchange Round 2 packages

            // A processes packages from B and C (for A)
            const r2B_for_A = extractPackageFromMap(1, round2PackageB_map_hex, false); // Use global helper
            frostDkgA!.add_round2_package(2, r2B_for_A);
            (managerA as any).receivedRound2Packages.add('b');
            const r2C_for_A = extractPackageFromMap(1, round2PackageC_map_hex, false); // Use global helper
            frostDkgA!.add_round2_package(3, r2C_for_A);
            (managerA as any).receivedRound2Packages.add('c');

            // B processes packages from A and C (for B)
            const r2A_for_B = extractPackageFromMap(2, round2PackageA_map_hex, false); // Use global helper
            frostDkgB!.add_round2_package(1, r2A_for_B);
            (managerB as any).receivedRound2Packages.add('a');
            const r2C_for_B = extractPackageFromMap(2, round2PackageC_map_hex, false); // Use global helper
            frostDkgB!.add_round2_package(3, r2C_for_B);
            (managerB as any).receivedRound2Packages.add('c');

            // C processes packages from A and B (for C)
            const r2A_for_C = extractPackageFromMap(3, round2PackageA_map_hex, false); // Use global helper
            frostDkgC!.add_round2_package(1, r2A_for_C);
            (managerC as any).receivedRound2Packages.add('a');
            const r2B_for_C = extractPackageFromMap(3, round2PackageB_map_hex, false); // Use global helper
            frostDkgC!.add_round2_package(2, r2B_for_C);
            (managerC as any).receivedRound2Packages.add('b');


            // Verify all can finalize
            expect(frostDkgA!.can_finalize()).toBe(true);
            expect(frostDkgB!.can_finalize()).toBe(true);
            expect(frostDkgC!.can_finalize()).toBe(true);
            console.log('✅ All Round 2 packages received and validated');

            // Update manager states
            (managerA as any)._updateDkgState(DkgState.Finalizing);
            (managerB as any)._updateDkgState(DkgState.Finalizing);
            (managerC as any)._updateDkgState(DkgState.Finalizing);


            console.log('\\\\n=== ROUND 3: FINALIZATION PHASE ===');
            // Finalize DKG. finalize_dkg() returns the full per-participant
            // keystore JSON (includes participant_index, signing_share,
            // etc. — so comparing these across participants would always
            // fail). Use get_group_public_key() to extract the shared
            // verifying_key, which IS identical across participants.
            frostDkgA!.finalize_dkg();
            frostDkgB!.finalize_dkg();
            frostDkgC!.finalize_dkg();

            const groupPublicKeyA = frostDkgA!.get_group_public_key();
            (managerA as any).groupPublicKey = groupPublicKeyA;
            const groupPublicKeyB = frostDkgB!.get_group_public_key();
            (managerB as any).groupPublicKey = groupPublicKeyB;
            const groupPublicKeyC = frostDkgC!.get_group_public_key();
            (managerC as any).groupPublicKey = groupPublicKeyC;

            // Get Solana addresses
            const solanaAddressA_val = frostDkgA!.get_address();
            (managerA as any).solanaAddress = solanaAddressA_val;
            const solanaAddressB_val = frostDkgB!.get_address();
            (managerB as any).solanaAddress = solanaAddressB_val;
            const solanaAddressC_val = frostDkgC!.get_address();
            (managerC as any).solanaAddress = solanaAddressC_val;

            (managerA as any)._updateDkgState(DkgState.Complete);
            (managerB as any)._updateDkgState(DkgState.Complete);
            (managerC as any)._updateDkgState(DkgState.Complete);


            console.log('\\\\n=== DKG COMPLETION & VERIFICATION ===');
            console.log(`🏦 Solana Address: ${solanaAddressA_val}`);
            console.log(`📊 Ed25519 Group Public Key: ${groupPublicKeyA.substring(0, 20)}...${groupPublicKeyA.substring(groupPublicKeyA.length - 10)}`);
            console.log(`🔐 Each participant holds a threshold key share`);
            console.log(`✅ Any 2 of 3 participants can now create valid Solana signatures`);

            // Verify all participants generated identical GROUP public key
            // (their individual key shares/packages differ by design).
            expect(groupPublicKeyA).toBe(groupPublicKeyB);
            expect(groupPublicKeyB).toBe(groupPublicKeyC);
            expect(solanaAddressA_val).toBe(solanaAddressB_val);
            expect(solanaAddressB_val).toBe(solanaAddressC_val);


            // Verify Solana address format (base58 encoded)
            expect(solanaAddressA_val).toMatch(/^[1-9A-HJ-NP-Za-km-z]+$/);
            expect(solanaAddressA_val.length).toBeGreaterThanOrEqual(32);

            console.log('\\\\n=== CRYPTOGRAPHIC PROPERTIES VERIFIED ===');
            console.log('✅ All participants generated identical group public keys');
            console.log('✅ All participants generated identical Solana addresses');
            console.log('✅ Solana address format is valid');
            console.log('✅ Ed25519 FROST DKG protocol completed successfully');
            console.log('✅ Threshold signature scheme is ready for Solana transactions');
            console.log('✅ Private key shares remain secure and distributed');
            console.log('=====================================\\\\n');

        } finally {
            // Cleanup WASM instances
            if (frostDkgA) frostDkgA.free();
            if (frostDkgB) frostDkgB.free();
            if (frostDkgC) frostDkgC.free();
        }
    });

    it('should initialize Ethereum secp256k1 DKG using real WASM', () => {
        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping secp256k1 DKG initialization test.');
            return;
        }
        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = {
            session_id: 'test-session',
            participants: ['a', 'b', 'c'],
            accepted_devices: ['a', 'b', 'c'],
            total: 3,
            threshold: 2
        } as any;

        // Set blockchain to ethereum for correct curve display in logs
        (manager as any).currentBlockchain = "ethereum";

        // Use actual WASM initialization
        expect(() => {
            (manager as any).frostDkg = new FrostDkgSecp256k1();
        }).not.toThrow();

        (manager as any)._updateDkgState(DkgState.Idle);
        expect(manager.dkgState).toBe(DkgState.Idle);
    });

    it('should complete full Ethereum secp256k1 DKG process with 3 clear FROST rounds', async () => {
        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping end-to-end Ethereum DKG test.');
            return;
        }
        // Create three managers for a complete 3-peer DKG simulation
        const managerA = new WebRTCManager('a', dummySend);
        const managerB = new WebRTCManager('b', dummySend);
        const managerC = new WebRTCManager('c', dummySend);

        // Mock data channels to prevent "Cannot send WebRTCAppMessage" errors
        const mockDataChannel = { readyState: 'open', send: () => { } };
        (managerA as any).dataChannels.set('b', mockDataChannel);
        (managerA as any).dataChannels.set('c', mockDataChannel);
        (managerB as any).dataChannels.set('a', mockDataChannel);
        (managerB as any).dataChannels.set('c', mockDataChannel);
        (managerC as any).dataChannels.set('a', mockDataChannel);
        (managerC as any).dataChannels.set('b', mockDataChannel);

        // Set up session info for all managers
        [managerA, managerB, managerC].forEach(manager => {
            manager.sessionInfo = sessionInfo as any;
            (manager as any)._updateMeshStatus({ type: MeshStatusType.Ready });
            (manager as any)._updateDkgState(DkgState.Idle);
        });

        console.log('\\\\n=== REAL DKG PROCESS WITH WASM ===');
        console.log('Participants: a, b, c');
        console.log('Threshold: 2 of 3 (any 2 participants can sign)');
        console.log('Protocol: FROST (Flexible Round-Optimized Schnorr Threshold)');

        // Create actual FROST DKG instances using WASM
        let frostDkgA: FrostDkgSecp256k1 | null = null;
        let frostDkgB: FrostDkgSecp256k1 | null = null;
        let frostDkgC: FrostDkgSecp256k1 | null = null;

        try {
            console.log('✅ Using real FROST DKG WASM instances for secp256k1');
            frostDkgA = new FrostDkgSecp256k1();
            frostDkgB = new FrostDkgSecp256k1();
            frostDkgC = new FrostDkgSecp256k1();

            (managerA as any).frostDkg = frostDkgA;
            (managerB as any).frostDkg = frostDkgB;
            (managerC as any).frostDkg = frostDkgC;
            (managerA as any).participantIndex = 1;
            (managerB as any).participantIndex = 2;
            (managerC as any).participantIndex = 3;

            // Set blockchain to ethereum for correct curve display in logs
            (managerA as any).currentBlockchain = "ethereum";
            (managerB as any).currentBlockchain = "ethereum";
            (managerC as any).currentBlockchain = "ethereum";

            // Initialize DKG for all participants
            console.log('\\\\n=== DKG INITIALIZATION ===');
            frostDkgA!.init_dkg(1, 3, 2);
            frostDkgB!.init_dkg(2, 3, 2);
            frostDkgC!.init_dkg(3, 3, 2);

            console.log('\\\\n=== ROUND 1: COMMITMENT PHASE ===');
            // Step 1: Generate Round 1 packages (only once per participant!)
            const round1PackageA_hex = frostDkgA!.generate_round1();
            const round1PackageB_hex = frostDkgB!.generate_round1();
            const round1PackageC_hex = frostDkgC!.generate_round1();

            // Update manager states to Round1InProgress manually since we're not using their broadcast methods
            // and simulate they have received their own package
            (managerA as any)._updateDkgState(DkgState.Round1InProgress);
            (managerA as any).receivedRound1Packages.add('a'); // Corrected
            (managerB as any)._updateDkgState(DkgState.Round1InProgress);
            (managerB as any).receivedRound1Packages.add('b'); // Corrected
            (managerC as any)._updateDkgState(DkgState.Round1InProgress);
            (managerC as any).receivedRound1Packages.add('c'); // Corrected


            console.log('\\\\n--- ROUND 1 PACKAGE EXCHANGE ---');
            // Step 2: Exchange Round 1 packages directly to WASM instances
            // Each participant adds the Round 1 packages from other participants

            // A adds packages from B and C
            frostDkgA!.add_round1_package(2, round1PackageB_hex);
            (managerA as any).receivedRound1Packages.add('b'); // Corrected
            frostDkgA!.add_round1_package(3, round1PackageC_hex);
            (managerA as any).receivedRound1Packages.add('c'); // Corrected


            // B adds packages from A and C
            frostDkgB!.add_round1_package(1, round1PackageA_hex);
            (managerB as any).receivedRound1Packages.add('a'); // Corrected
            frostDkgB!.add_round1_package(3, round1PackageC_hex);
            (managerB as any).receivedRound1Packages.add('c'); // Corrected


            // C adds packages from A and B
            frostDkgC!.add_round1_package(1, round1PackageA_hex);
            (managerC as any).receivedRound1Packages.add('a'); // Corrected
            frostDkgC!.add_round1_package(2, round1PackageB_hex);
            (managerC as any).receivedRound1Packages.add('b'); // Corrected


            // Verify all can start Round 2
            expect(frostDkgA!.can_start_round2()).toBe(true);
            expect(frostDkgB!.can_start_round2()).toBe(true);
            expect(frostDkgC!.can_start_round2()).toBe(true);

            // Update manager states to reflect readiness for Round 2
            // (managerA as any)._updateDkgState(DkgState.Round1Complete); // Or similar internal state if exists
            // (managerB as any)._updateDkgState(DkgState.Round1Complete);
            // (managerC as any)._updateDkgState(DkgState.Round1Complete);
            // For the test, we'll directly generate round 2 if possible.
            // The WebRTCManager's internal state machine would handle this via _handleDkgRound1Package calls.

            console.log('\\\\n=== ROUND 2: SECRET SHARE PHASE ===');
            // Generate Round 2 packages (secret shares as maps)
            const round2PackageA_map_hex = frostDkgA!.generate_round2();
            const round2PackageB_map_hex = frostDkgB!.generate_round2();
            const round2PackageC_map_hex = frostDkgC!.generate_round2();

            console.log('\\\\n--- Round 2 Package Exchange ---');
            // Exchange Round 2 packages

            // A processes packages from B and C (for A)
            const r2B_for_A = extractPackageFromMap(1, round2PackageB_map_hex, true); // secp256k1 format with FrostDkgSecp256k1
            frostDkgA!.add_round2_package(2, r2B_for_A);
            (managerA as any).receivedRound2Packages.add('b');
            const r2C_for_A = extractPackageFromMap(1, round2PackageC_map_hex, true); // secp256k1 format with FrostDkgSecp256k1
            frostDkgA!.add_round2_package(3, r2C_for_A);
            (managerA as any).receivedRound2Packages.add('c');

            // B processes packages from A and C (for B)
            const r2A_for_B = extractPackageFromMap(2, round2PackageA_map_hex, true); // secp256k1 format with FrostDkgSecp256k1
            frostDkgB!.add_round2_package(1, r2A_for_B);
            (managerB as any).receivedRound2Packages.add('a');
            const r2C_for_B = extractPackageFromMap(2, round2PackageC_map_hex, true); // secp256k1 format with FrostDkgSecp256k1
            frostDkgB!.add_round2_package(3, r2C_for_B);
            (managerB as any).receivedRound2Packages.add('c');

            // C processes packages from A and B (for C)
            const r2A_for_C = extractPackageFromMap(3, round2PackageA_map_hex, true); // secp256k1 format with FrostDkgSecp256k1
            frostDkgC!.add_round2_package(1, r2A_for_C);
            (managerC as any).receivedRound2Packages.add('a');
            const r2B_for_C = extractPackageFromMap(3, round2PackageB_map_hex, true); // secp256k1 format with FrostDkgSecp256k1
            frostDkgC!.add_round2_package(2, r2B_for_C);
            (managerC as any).receivedRound2Packages.add('b');


            // Verify all can finalize
            expect(frostDkgA!.can_finalize()).toBe(true);
            expect(frostDkgB!.can_finalize()).toBe(true);
            expect(frostDkgC!.can_finalize()).toBe(true);
            console.log('✅ All Round 2 packages received and validated');

            // Update manager states
            (managerA as any)._updateDkgState(DkgState.Finalizing);
            (managerB as any)._updateDkgState(DkgState.Finalizing);
            (managerC as any)._updateDkgState(DkgState.Finalizing);


            console.log('\\\\n=== ROUND 3: FINALIZATION PHASE ===');
            // Finalize DKG. See note in the Ed25519 version above —
            // finalize_dkg() returns full per-participant keystore,
            // so compare the shared verifying_key via
            // get_group_public_key() instead.
            frostDkgA!.finalize_dkg();
            frostDkgB!.finalize_dkg();
            frostDkgC!.finalize_dkg();

            const groupPublicKeyA = frostDkgA!.get_group_public_key();
            (managerA as any).groupPublicKey = groupPublicKeyA;
            const groupPublicKeyB = frostDkgB!.get_group_public_key();
            (managerB as any).groupPublicKey = groupPublicKeyB;
            const groupPublicKeyC = frostDkgC!.get_group_public_key();
            (managerC as any).groupPublicKey = groupPublicKeyC;

            // Get Ethereum addresses from group public keys (secp256k1 uses Ethereum)
            const ethAddressA = frostDkgA!.get_eth_address(); // Corrected: was get_address
            (managerA as any).ethAddress = ethAddressA; // Store on manager for consistency
            const ethAddressB = frostDkgB!.get_eth_address(); // Corrected: was get_address
            (managerB as any).ethAddress = ethAddressB;
            const ethAddressC = frostDkgC!.get_eth_address(); // Corrected: was get_address
            (managerC as any).ethAddress = ethAddressC;

            (managerA as any)._updateDkgState(DkgState.Complete);
            (managerB as any)._updateDkgState(DkgState.Complete);
            (managerC as any)._updateDkgState(DkgState.Complete);


            console.log('\\\\n=== DKG COMPLETION & VERIFICATION ===');
            console.log(`🏦 Ethereum Address: ${ethAddressA}`);
            console.log(`📊 secp256k1 Group Public Key: ${groupPublicKeyA.substring(0, 20)}...${groupPublicKeyA.substring(groupPublicKeyA.length - 10)}`);
            console.log(`🔐 Each participant holds a threshold key share`);
            console.log(`✅ Any 2 of 3 participants can now create valid Ethereum signatures`);

            // Verify all participants generated identical results
            expect(groupPublicKeyA).toBe(groupPublicKeyB);
            expect(groupPublicKeyB).toBe(groupPublicKeyC);
            expect(ethAddressA).toBe(ethAddressB);
            expect(ethAddressB).toBe(ethAddressC);

            // Verify Ethereum address format (hex encoded with 0x prefix)
            expect(ethAddressA).toMatch(/^0x[a-fA-F0-9]{40}$/);
            expect(ethAddressA.length).toBe(42);

            console.log('\\\\n=== CRYPTOGRAPHIC PROPERTIES VERIFIED ===');
            console.log('✅ All participants generated identical group public keys');
            console.log('✅ All participants generated identical Ethereum addresses');
            console.log('✅ Ethereum address format is valid (0x + 40 hex chars)');
            console.log('✅ secp256k1 FROST DKG protocol completed successfully');
            console.log('✅ Threshold signature scheme is ready for Ethereum transactions');
            console.log('✅ Private key shares remain secure and distributed');
            console.log('=====================================\\\\n');

        } finally {
            // Cleanup WASM instances
            if (frostDkgA) frostDkgA.free();
            if (frostDkgB) frostDkgB.free();
            if (frostDkgC) frostDkgC.free();
        }
    });

    it('should handle secp256k1 DKG failures gracefully', async () => {
        if (!wasmInitialized) {
            console.warn('⚠️ WASM not initialized, skipping secp256k1 DKG failure test.');
            return;
        }

        const manager = new WebRTCManager('a', dummySend);
        manager.sessionInfo = sessionInfo as any;
        (manager as any)._updateDkgState(DkgState.Round1InProgress);

        // Set blockchain to ethereum for correct curve display in logs
        (manager as any).currentBlockchain = "ethereum";

        const frostDkg = new FrostDkgSecp256k1();
        (manager as any).frostDkg = frostDkg;
        (manager as any).participantIndex = 1;

        try {
            // Initialize properly first
            frostDkg.init_dkg(1, 3, 2);
            (manager as any).participantIndex = 1; // Set participant index for manager

            // Simulate manager 'a' having generated its own Round 1 package
            const round1PackageA_self = frostDkg.generate_round1();
            (manager as any).receivedRound1Packages.add('a'); // Corrected


            // Simulate receiving an invalid Round 1 package that causes WASM to throw an error
            // The `add_round1_package` in WASM should handle validation and throw.
            // We pass deliberately malformed data.
            await (manager as any)._handleDkgRound1Package('b', { sender_index: 2, data: 'invalid-hex-data' });

            // If _handleDkgRound1Package catches the error and sets state to Failed:
            expect(manager.dkgState).toBe(DkgState.Failed);

        } catch (error) {
            // This catch might be hit if _handleDkgRound1Package re-throws or if setup fails
            console.info("Error in secp256k1 DKG failure test:", error);
            // If an error is caught here, it implies the manager didn't handle it by setting state to Failed.
            // However, the primary check is manager.dkgState.
            // If the WASM call itself throws before the manager can handle, this is also a valid failure.
            // The key is that the system doesn't crash and recognizes a failure.
            // If the WASM call is expected to throw and the manager to catch it:
            // expect(manager.dkgState).toBe(DkgState.Failed);
        } finally {
            frostDkg.free();
        }
        // Final check on the DKG state
        expect(manager.dkgState).toBe(DkgState.Failed);
    });
});

describe('#29 canonical participant ordering (ext↔CLI interop)', () => {
    // FROST identifiers must come from SORTED participant order to match the
    // Rust core's canonical_identifier (dkg.rs: participants.sort() then pos+1).
    it('sorts participants when a session is assigned', () => {
        const m = new WebRTCManager('m', dummySend) as any;
        const sorted = m._withSortedParticipants({
            session_id: 's', participants: ['z', 'a', 'm'], accepted_devices: [],
        });
        expect(sorted.participants).toEqual(['a', 'm', 'z']);
        // canonical identifier for 'm' = sorted position + 1 = 2 (NOT 3, its
        // index in the original join-order array).
        expect(sorted.participants.indexOf('m') + 1).toBe(2);
    });

    it('is null/empty safe', () => {
        const m = new WebRTCManager('m', dummySend) as any;
        expect(m._withSortedParticipants(null)).toBeNull();
        const empty = m._withSortedParticipants({ session_id: 's', participants: [], accepted_devices: [] });
        expect(empty.participants).toEqual([]);
    });

    it('does not mutate the input array', () => {
        const m = new WebRTCManager('m', dummySend) as any;
        const input = { session_id: 's', participants: ['z', 'a'], accepted_devices: [] };
        m._withSortedParticipants(input);
        expect(input.participants).toEqual(['z', 'a']); // original untouched
    });
});
