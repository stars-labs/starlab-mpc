// ===================================================================
// DISTRIBUTED KEY GENERATION (DKG) STATE TYPES
// ===================================================================
//
// This file contains types for managing the state of Distributed Key 
// Generation (DKG) operations in the MPC wallet system.
//
// Key Concepts for Junior Developers:
// - DKG: Distributed Key Generation - a process where multiple parties
//   collaborate to generate a shared cryptographic key without any single
//   party knowing the complete key
// - Rounds: DKG happens in multiple rounds where parties exchange packages
// - FROST: The specific DKG protocol we use (Flexible Round-Optimized
//   Schnorr Threshold signatures)
// - State Machine: DKG follows a strict sequence of states
// ===================================================================

/**
 * All possible states in the DKG (Distributed Key Generation) process.
 * This follows a strict state machine pattern where each state can only
 * transition to specific next states.
 * 
 * State Flow:
 * Idle → Initializing → Round1InProgress → Round1Complete → 
 * Round2InProgress → Round2Complete → Finalizing → Complete
 * 
 * Alternative flow for imported keystores:
 * Idle → KeystoreImported → Complete (when enough peers connect)
 * 
 * Any state can transition to Failed if errors occur.
 */
export enum DkgState {
    /** No DKG operation in progress */
    Idle = 0,

    /** DKG is being set up and initialized */
    Initializing = 1,

    /** Round 1 of DKG is in progress (generating and exchanging commitments) */
    Round1InProgress = 2,

    /** Round 1 is complete, ready for Round 2 */
    Round1Complete = 3,

    /** Round 2 of DKG is in progress (generating key shares) */
    Round2InProgress = 4,

    /** Round 2 is complete, ready for finalization */
    Round2Complete = 5,

    /** Finalizing the DKG process (combining all data) */
    Finalizing = 6,

    /** DKG completed successfully, keys are ready */
    Complete = 7,

    /** DKG failed due to an error */
    Failed = 8,

    /** Keystore imported but not yet connected to other participants */
    KeystoreImported = 9,
}

/**
 * Information about a DKG package received from another participant.
 * Packages are cryptographic data exchanged during DKG rounds.
 */
export interface DkgPackageInfo {
    /** Device ID of the sender */
    fromPeerId: string;

    /** The cryptographic package data (usually hex-encoded) */
    packageData: any;

    /** Timestamp when this package was received */
    receivedAt?: number;

    /** Which DKG round this package belongs to */
    round: 1 | 2;
}

/**
 * Current status and progress of a DKG operation.
 * This provides detailed information about the ongoing DKG process.
 */
export interface DkgStatus {
    /** Current state of the DKG process */
    state: DkgState;

    /** Human-readable name of the current state */
    stateName: string;

    /** Which blockchain this DKG is for */
    blockchain: "ethereum" | "solana" | null;

    /** List of all participants in this DKG */
    participants: string[];

    /** Minimum number of participants needed for signing */
    threshold: number;

    /** The generated group public key (available after completion) */
    groupPublicKey: string | null;

    /** The derived wallet address (available after completion) */
    address: string | null;

    /** This participant's index in the DKG (1-based) */
    participantIndex: number | null;

    /** Session information this DKG belongs to */
    sessionInfo: SessionInfo | null;

    /** Device IDs from which we've received Round 1 packages */
    receivedRound1Packages: string[];

    /** Device IDs from which we've received Round 2 packages */
    receivedRound2Packages: string[];

    /** Whether the FROST DKG instance is properly initialized */
    frostDkgInitialized: boolean;

    /** Any error message if the DKG failed */
    errorMessage?: string;
}

/**
 * Events that can occur during DKG operations.
 * These are used for monitoring and logging DKG progress.
 */
export type DkgEvent =
    | { type: 'StateChanged'; oldState: DkgState; newState: DkgState }
    | { type: 'PackageReceived'; round: 1 | 2; fromPeer: string }
    | { type: 'PackageSent'; round: 1 | 2; toPeer: string }
    | { type: 'RoundCompleted'; round: 1 | 2 }
    | { type: 'KeyGenerated'; publicKey: string; address: string }
    | { type: 'Error'; message: string; state: DkgState };

// Import SessionInfo type (will be available after refactoring)
import type { SessionInfo } from './session';

// Ensure this file is treated as a module
export { };
