/**
 * Signing Manager - Handles FROST Threshold Signing
 * 
 * This module manages the complete FROST signing lifecycle:
 * - Signing request initiation and acceptance
 * - Participant selection and commitment phase
 * - Signature share generation and collection
 * - Signature aggregation and finalization
 * 
 * Extracted from the monolithic webrtc.ts for better maintainability
 */

import type { WebRTCAppMessage } from "../../types/webrtc";
import type { SessionInfo } from "../../types/session";
import { match } from "ts-pattern";

// Signing state enumeration to track signing process
export enum SigningState {
    Idle = "Idle",
    AwaitingAcceptances = "AwaitingAcceptances", // Waiting for peers to accept signing request
    CommitmentPhase = "CommitmentPhase", // FROST Round 1 - collecting commitments
    SharePhase = "SharePhase", // FROST Round 2 - collecting signature shares
    Complete = "Complete", // Signing completed successfully
    Failed = "Failed" // Signing failed
}

// Signing process information
export interface SigningInfo {
    signing_id: string;
    transaction_data: string;
    threshold: number;
    participants: string[];
    acceptances: Map<string, boolean>; // Map peer ID to acceptance status
    accepted_participants: string[];
    selected_signers: string[];
    step: "pending_acceptance" | "signer_selection" | "commitment_phase" | "share_phase" | "complete";
    initiator: string;
    final_signature?: string; // Final aggregated signature as string
}

/**
 * Callback interface for signing events
 */
export interface SigningManagerCallbacks {
    onLog: (message: string) => void;
    onSigningStateUpdate: (state: SigningState, info: SigningInfo | null) => void;
    onSendMessage: (toPeerId: string, message: WebRTCAppMessage) => void;
}

/**
 * Signing Manager class handles all FROST signing operations
 */
export class SigningManager {
    private localPeerId: string;
    private callbacks: SigningManagerCallbacks;

    // Signing state tracking
    public signingState: SigningState = SigningState.Idle;
    public signingInfo: SigningInfo | null = null;

    // FROST signing integration
    private signingCommitments: Map<string, any> = new Map(); // Map peer to commitment data
    private signingShares: Map<string, any> = new Map(); // Map peer to signature share data

    constructor(localPeerId: string, callbacks: SigningManagerCallbacks) {
        this.localPeerId = localPeerId;
        this.callbacks = callbacks;
        this._log("SigningManager initialized");
    }

    /**
     * Request a new signing operation
     */
    async requestSigning(transactionData: string, sessionInfo: SessionInfo | null): Promise<void> {
        if (!sessionInfo) {
            throw new Error("No active session for signing");
        }

        if (this.signingState !== SigningState.Idle) {
            throw new Error(`Cannot start signing: current state is ${this.signingState}`);
        }

        try {
            const signingId = `signing_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;

            this._log(`Initiating signing request: ${signingId}`);

            this.signingInfo = {
                signing_id: signingId,
                transaction_data: transactionData,
                threshold: sessionInfo.threshold,
                participants: [...sessionInfo.participants],
                acceptances: new Map(),
                accepted_participants: [],
                selected_signers: [],
                step: "pending_acceptance",
                initiator: this.localPeerId
            };

            this.signingState = SigningState.AwaitingAcceptances;

            // Send signing request to all participants
            const signingRequest: WebRTCAppMessage = {
                webrtc_msg_type: 'SigningRequest',
                signing_id: signingId,
                transaction_data: transactionData,
                required_signers: sessionInfo.threshold
            };

            // Send to all other participants
            const otherParticipants = sessionInfo.participants.filter(p => p !== this.localPeerId);
            for (const peerId of otherParticipants) {
                this.callbacks.onSendMessage(peerId, signingRequest);
            }

            this.callbacks.onSigningStateUpdate(this.signingState, this.signingInfo);
            this._log(`Signing request sent to ${otherParticipants.length} participants`);
        } catch (error) {
            this._log(`Error requesting signing: ${error}`);
            this.signingState = SigningState.Failed;
            this.callbacks.onSigningStateUpdate(this.signingState, this.signingInfo);
            throw error;
        }
    }

    /**
     * Accept a signing request
     */
    async acceptSigning(signingId: string): Promise<void> {
        try {
            this._log(`Accepting signing request: ${signingId}`);

            if (!this.signingInfo || this.signingInfo.signing_id !== signingId) {
                throw new Error(`No matching signing request found: ${signingId}`);
            }

            const acceptance: WebRTCAppMessage = {
                webrtc_msg_type: 'SigningAcceptance',
                signing_id: signingId,
                accepted: true
            };

            // Send acceptance to initiator
            this.callbacks.onSendMessage(this.signingInfo.initiator, acceptance);
            this._log(`Signing acceptance sent for: ${signingId}`);
        } catch (error) {
            this._log(`Error accepting signing: ${error}`);
            throw error;
        }
    }

    /**
     * Handle incoming signing-related messages
     */
    async handleMessage(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        this._log(`Handling signing message from ${fromPeerId}: ${message.webrtc_msg_type}`);

        try {
            await match(message.webrtc_msg_type)
                .with('SigningRequest', async () => {
                    await this.handleSigningRequest(fromPeerId, message);
                })
                .with('SigningAcceptance', async () => {
                    await this.handleSigningAcceptance(fromPeerId, message);
                })
                .with('SigningCommitment', async () => {
                    await this.handleSigningCommitment(fromPeerId, message);
                })
                .with('SignatureShare', async () => {
                    await this.handleSigningShare(fromPeerId, message);
                })
                .otherwise(() => {
                    this._log(`Unhandled signing message type: ${message.webrtc_msg_type}`);
                    return Promise.resolve();
                });
        } catch (error) {
            this._log(`Error handling signing message: ${error}`);
        }
    }

    /**
     * Handle incoming signing request
     */
    private async handleSigningRequest(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        // Type guard to ensure this is a SigningRequest message
        if (message.webrtc_msg_type !== 'SigningRequest') {
            throw new Error("Invalid message type for signing request handler");
        }

        const signingRequest = message as { webrtc_msg_type: 'SigningRequest'; signing_id: string; transaction_data: string; required_signers: number };

        this._log(`Received signing request from ${fromPeerId}: ${signingRequest.signing_id}`);

        this.signingInfo = {
            signing_id: signingRequest.signing_id,
            transaction_data: signingRequest.transaction_data,
            threshold: signingRequest.required_signers,
            participants: [], // Will be filled as acceptances come in
            acceptances: new Map(),
            accepted_participants: [],
            selected_signers: [],
            step: "pending_acceptance",
            initiator: fromPeerId
        };

        this.signingState = SigningState.AwaitingAcceptances;
        this.callbacks.onSigningStateUpdate(this.signingState, this.signingInfo);
    }

    /**
     * Handle signing acceptance from participants
     */
    private async handleSigningAcceptance(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        if (!this.signingInfo) {
            return;
        }

        // Type guard to ensure this is a SigningAcceptance message
        if (message.webrtc_msg_type !== 'SigningAcceptance') {
            return;
        }

        const acceptance = message as { webrtc_msg_type: 'SigningAcceptance'; signing_id: string; accepted: boolean };

        if (acceptance.signing_id !== this.signingInfo.signing_id) {
            this._log(`Signing ID mismatch: expected ${this.signingInfo.signing_id}, got ${acceptance.signing_id}`);
            return;
        }

        this._log(`Received signing acceptance from ${fromPeerId}: ${acceptance.accepted}`);

        this.signingInfo.acceptances.set(fromPeerId, acceptance.accepted);

        if (acceptance.accepted) {
            this.signingInfo.accepted_participants.push(fromPeerId);
        }

        // Check if we have enough acceptances
        if (this.signingInfo.accepted_participants.length >= this.signingInfo.threshold - 1) { // -1 because initiator is included
            this._log("Sufficient acceptances received, moving to commitment phase");
            await this.startCommitmentPhase();
        }

        this.callbacks.onSigningStateUpdate(this.signingState, this.signingInfo);
    }

    /**
     * Start the commitment phase of signing
     */
    private async startCommitmentPhase(): Promise<void> {
        if (!this.signingInfo) return;

        this._log("Starting signing commitment phase");

        this.signingState = SigningState.CommitmentPhase;
        this.signingInfo.step = "commitment_phase";

        // Select signers (for now, use all accepted participants plus initiator)
        this.signingInfo.selected_signers = [
            this.signingInfo.initiator,
            ...this.signingInfo.accepted_participants.slice(0, this.signingInfo.threshold - 1)
        ];

        // TODO: Implement FROST commitment generation
        // For now, simulate commitment

        const commitmentMessage: WebRTCAppMessage = {
            webrtc_msg_type: 'SigningCommitment',
            signing_id: this.signingInfo.signing_id,
            sender_identifier: this.localPeerId,
            commitment: `mock_commitment_${Date.now()}`
        };

        // Send to all selected signers
        for (const peerId of this.signingInfo.selected_signers.filter(p => p !== this.localPeerId)) {
            this.callbacks.onSendMessage(peerId, commitmentMessage);
        }

        this.callbacks.onSigningStateUpdate(this.signingState, this.signingInfo);
    }

    /**
     * Handle signing commitment from participants
     */
    private async handleSigningCommitment(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        if (!this.signingInfo) return;

        // Type guard to ensure this is a SigningCommitment message
        if (message.webrtc_msg_type !== 'SigningCommitment') {
            return;
        }

        const commitment = message as { webrtc_msg_type: 'SigningCommitment'; signing_id: string; sender_identifier: any; commitment: any };

        if (commitment.signing_id !== this.signingInfo.signing_id) {
            return;
        }

        this._log(`Received signing commitment from ${fromPeerId}`);
        this.signingCommitments.set(fromPeerId, commitment.commitment);

        // Check if we have all commitments
        const expectedCommitments = this.signingInfo.selected_signers.length - 1; // -1 for our own
        if (this.signingCommitments.size >= expectedCommitments) {
            this._log("All commitments received, moving to share phase");
            await this.startSharePhase();
        }
    }

    /**
     * Start the share phase of signing
     */
    private async startSharePhase(): Promise<void> {
        if (!this.signingInfo) return;

        this._log("Starting signing share phase");

        this.signingState = SigningState.SharePhase;
        this.signingInfo.step = "share_phase";

        // TODO: Implement FROST signature share generation
        // For now, simulate share

        const shareMessage: WebRTCAppMessage = {
            webrtc_msg_type: 'SignatureShare',
            signing_id: this.signingInfo.signing_id,
            sender_identifier: this.localPeerId,
            share: `mock_share_${Date.now()}`
        };

        // Send to all selected signers
        for (const peerId of this.signingInfo.selected_signers.filter(p => p !== this.localPeerId)) {
            this.callbacks.onSendMessage(peerId, shareMessage);
        }

        this.callbacks.onSigningStateUpdate(this.signingState, this.signingInfo);
    }

    /**
     * Handle signing share from participants
     */
    private async handleSigningShare(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        if (!this.signingInfo) return;

        // Type guard to ensure this is a SignatureShare message
        if (message.webrtc_msg_type !== 'SignatureShare') {
            return;
        }

        const shareMessage = message as { webrtc_msg_type: 'SignatureShare'; signing_id: string; sender_identifier: any; share: any };

        if (shareMessage.signing_id !== this.signingInfo.signing_id) {
            return;
        }

        this._log(`Received signing share from ${fromPeerId}`);
        this.signingShares.set(fromPeerId, shareMessage.share);

        // Check if we have all shares
        const expectedShares = this.signingInfo.selected_signers.length - 1; // -1 for our own
        if (this.signingShares.size >= expectedShares) {
            this._log("All shares received, finalizing signature");
            await this.finalizeSignature();
        }
    }

    /**
     * Finalize the signature
     */
    private async finalizeSignature(): Promise<void> {
        if (!this.signingInfo) return;

        this._log("Finalizing signature");

        // TODO: Implement FROST signature aggregation
        // For now, simulate final signature
        this.signingInfo.final_signature = `mock_signature_${Date.now()}`;
        this.signingInfo.step = "complete";
        this.signingState = SigningState.Complete;

        this.callbacks.onSigningStateUpdate(this.signingState, this.signingInfo);
        this._log(`Signing completed successfully: ${this.signingInfo.signing_id}`);
    }

    /**
     * Reset signing state
     */
    resetSigning(): void {
        this._log("Resetting signing state");
        this.signingState = SigningState.Idle;
        this.signingInfo = null;
        this.signingCommitments.clear();
        this.signingShares.clear();
        this.callbacks.onSigningStateUpdate(this.signingState, this.signingInfo);
    }

    /**
     * Cleanup resources
     */
    async cleanup(): Promise<void> {
        this._log("Cleaning up SigningManager");
        this.resetSigning();
    }

    /**
     * Internal logging with peer ID prefix
     */
    private _log(message: string): void {
        const logMessage = `[SigningManager:${this.localPeerId}] ${message}`;
        this.callbacks.onLog(logMessage);
    }
}
