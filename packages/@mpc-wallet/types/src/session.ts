// ===================================================================
// SESSION MANAGEMENT TYPES
// ===================================================================
//
// This file contains all types related to MPC wallet session management.
// Sessions are collaborative key generation or signing operations between
// multiple participants in the MPC (Multi-Party Computation) protocol.
//
// Key Concepts for Junior Developers:
// - Session: A collaborative operation involving multiple devices/participants
// - Proposer: The device that initiates a session  
// - Participants: All devices that will participate in the session
// - Threshold: Minimum number of participants needed for operations
// - Total: Maximum number of participants in the session
// ===================================================================

/**
 * Flat string discriminator for session purpose. This is what
 * actually goes on the wire — TUI's announcer emits
 * `"session_type": "dkg"` (command.rs near line 560) and TUI's
 * parser reads it as a plain string (command.rs::parse_session_info
 * line 279). **Do not** try to serialize a tagged enum; the wire
 * format is literally a lowercase string. For signing sessions the
 * ceremony-specific fields (wallet_name, group_public_key,
 * blockchain, signing_message_hex) live as top-level siblings of
 * `session_type`, not inside a nested content object.
 */
export type SessionTypeTag = "dkg" | "signing";

/**
 * Represents a session in the MPC wallet system.
 * A session is a collaborative operation (like key generation or signing)
 * that involves multiple participants working together.
 *
 * Shape must match what TUI emits on `announce_session` and accepts
 * on `session_available` (see `apps/tui-node/src/elm/command.rs`
 * parse_session_info + the announce builder). All DKG-common fields
 * are top-level; signing-specific fields are top-level siblings.
 */
export interface SessionInfo {
    /** Unique identifier for this session */
    session_id: string;

    /** Device ID of the participant who proposed this session */
    proposer_id: string;

    /** Maximum number of participants that can join this session */
    total: number; // u16 in Rust backend

    /** Minimum number of participants needed for operations to succeed */
    threshold: number; // u16 in Rust backend

    /** List of all device IDs that are part of this session */
    participants: string[];

    /** "dkg" or "signing". TUI's parser defaults to "dkg" if absent,
     *  so the extension should too. */
    session_type?: SessionTypeTag;

    /** "secp256k1" or "ed25519". TUI defaults to "secp256k1" when
     *  parsing if absent. */
    curve_type?: string;

    /** "Network" (WebRTC mesh) or "Offline" (SD-card air gap). TUI
     *  defaults to "Network". */
    coordination_type?: string;

    // ----- Signing-only fields (top-level, not nested) -----
    /** Name/ID of the wallet the signing ceremony targets. */
    wallet_name?: string;
    /** Hex-encoded serialized FROST group verifying key. */
    group_public_key?: string;
    /** e.g. "ethereum", "solana" — informational. */
    blockchain?: string;
    /** Signing-only: hex-encoded payload to sign. `undefined` on DKG. */
    signing_message_hex?: string;

    // ----- Extension-local bookkeeping (not on the TUI wire) -----

    /** List of device IDs that have accepted to join this session.
     *  Populated locally from SessionResponse replies. Not present on
     *  TUI-originated announcements — the wire-parse helper synthesises
     *  `[]` when it's absent, which is why callers can always assume
     *  the array exists. */
    accepted_devices: string[];

    /** Optional status field for session state tracking */
    status?: string;
}

/**
 * Used when proposing a new session to other participants.
 * This is sent over the network to invite others to join.
 */
export interface SessionProposal {
    /** Unique identifier for the proposed session */
    session_id: string;

    /** Maximum number of participants */
    total: number;

    /** Minimum number of participants needed */
    threshold: number;

    /** List of device IDs being invited to participate */
    participants: string[];
}

/**
 * Response sent by a participant when they receive a session proposal.
 * Each participant must respond with whether they accept or reject.
 */
export interface SessionResponse {
    /** The session ID they are responding to */
    session_id: string;

    /** Whether they accept (true) or reject (false) the invitation */
    accepted: boolean;
}

/**
 * Helper type for session validation and utilities
 */
export interface SessionValidation {
    /** Whether the session has enough participants */
    hasMinimumParticipants: boolean;

    /** Whether all invited participants have responded */
    allParticipantsResponded: boolean;

    /** Whether the session can proceed with operations */
    canProceed: boolean;
}

// Utility functions (can be implemented elsewhere)
export type SessionValidator = (session: SessionInfo) => SessionValidation;

// Ensure this file is treated as a module
export { };
