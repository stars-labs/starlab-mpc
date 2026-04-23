/**
 * DKG Manager - Handles FROST Distributed Key Generation
 * 
 * This module manages the complete DKG lifecycle:
 * - Session initialization and participant management
 * - Round 1: Commitment generation and collection
 * - Round 2: Share generation and distribution
 * - Round 3: Key package finalization and address derivation
 * 
 * Extracted from the monolithic webrtc.ts for better maintainability
 */

import { DkgState } from "@mpc-wallet/types/dkg";
import type { WebRTCAppMessage } from "@mpc-wallet/types/webrtc";
import type { SessionInfo } from "@mpc-wallet/types/session";
import type { KeyShareData } from "@mpc-wallet/types/keystore";

export { DkgState };

/**
 * Callback interface for DKG events
 */
export interface DkgManagerCallbacks {
    onLog: (message: string) => void;
    onDkgStateUpdate: (state: DkgState) => void;
    onSendMessage: (toPeerId: string, message: WebRTCAppMessage) => void;
    onDkgComplete?: (state: DkgState, keyShareData: KeyShareData) => void;
}

/**
 * DKG round package information for tracking
 */
interface DkgPackageInfo {
    fromPeerId: string;
    packageData: any;
    round: 1 | 2;
    receivedAt: number;
}

/**
 * DKG Manager class handles all FROST DKG operations
 */
export class DkgManager {
    private localPeerId: string;
    private callbacks: DkgManagerCallbacks;

    // DKG state tracking
    public dkgState: DkgState = DkgState.Idle;
    public sessionInfo: SessionInfo | null = null;

    // FROST DKG integration
    private frostDkg: any | null = null;
    private participant_index: number | null = null;

    // Package tracking for rounds
    private receivedRound1Packages: Set<string> = new Set();
    private receivedRound2Packages: Set<string> = new Set();

    // Package buffering for handling packages that arrive before DKG initialization
    private bufferedRound1Packages: Array<DkgPackageInfo> = [];
    private bufferedRound2Packages: Array<DkgPackageInfo> = [];

    // Generated keys and addresses
    private group_public_key: string | null = null;
    private solanaAddress: string | null = null;
    private ethereumAddress: string | null = null;
    private walletAddress: string | null = null;
    private currentBlockchain: "ethereum" | "solana" = "solana";

    constructor(localPeerId: string, callbacks: DkgManagerCallbacks) {
        this.localPeerId = localPeerId;
        this.callbacks = callbacks;
    }

    /**
     * Initialize DKG process for a session
     */
    public initializeDkg(sessionInfo: SessionInfo): boolean {
        if (this.dkgState !== DkgState.Idle) {
            this._log(`Cannot initialize DKG: current state is ${this.dkgState}`);
            return false;
        }

        this.sessionInfo = sessionInfo;
        
        // CRITICAL: Sort participants alphabetically to ensure consistent identifier assignment across all peers
        // This matches the behavior of the working CLI implementation
        const sortedParticipants = [...sessionInfo.participants].sort();
        this.participant_index = sortedParticipants.indexOf(this.localPeerId) + 1; // 1-based indexing

        if (this.participant_index <= 0) {
            this._log(`Error: Local peer ID ${this.localPeerId} not found in session participants`);
            return false;
        }
        
        this._log(`Participant order: ${sortedParticipants.join(', ')}`);
        this._log(`My participant index: ${this.participant_index}`);

        this._log(`Initializing DKG for session ${sessionInfo.session_id} as participant ${this.participant_index}`);
        this._updateDkgState(DkgState.Initializing);

        return true;
    }

    /**
     * Start DKG process when mesh is ready
     */
    public startDkg(): boolean {
        if (this.dkgState !== DkgState.Initializing) {
            this._log(`Cannot start DKG: current state is ${this.dkgState}`);
            return false;
        }

        if (!this.sessionInfo) {
            this._log(`Cannot start DKG: no session info`);
            return false;
        }

        this._log(`Starting DKG process with ${this.sessionInfo.participants.length} participants`);
        this._initializeFrostDkg();
        this._updateDkgState(DkgState.Round1InProgress);
        this._generateAndBroadcastRound1();

        return true;
    }

    /**
     * Handle received DKG Round 1 package
     */
    public handleDkgRound1Package(fromPeerId: string, packageData: any): void {
        this._log(`Received DKG Round 1 package from ${fromPeerId}`);

        if (this.dkgState !== DkgState.Round1InProgress) {
            this._log(`Buffering Round 1 package from ${fromPeerId} (current state: ${this.dkgState})`);
            this.bufferedRound1Packages.push({
                fromPeerId,
                packageData,
                round: 1,
                receivedAt: Date.now()
            });
            return;
        }

        this._processRound1Package(fromPeerId, packageData);
    }

    /**
     * Handle received DKG Round 2 package
     */
    public handleDkgRound2Package(fromPeerId: string, packageData: any): void {
        this._log(`Received DKG Round 2 package from ${fromPeerId}`);

        if (this.dkgState !== DkgState.Round2InProgress) {
            this._log(`Buffering Round 2 package from ${fromPeerId} (current state: ${this.dkgState})`);
            this.bufferedRound2Packages.push({
                fromPeerId,
                packageData,
                round: 2,
                receivedAt: Date.now()
            });
            return;
        }

        this._processRound2Package(fromPeerId, packageData);
    }

    /**
     * Get the current blockchain address
     */
    public getCurrentAddress(): string | null {
        return this.walletAddress;
    }

    /**
     * Get Ethereum address specifically
     */
    public getEthereumAddress(): string | null {
        return this.ethereumAddress;
    }

    /**
     * Get Solana address specifically
     */
    public getSolanaAddress(): string | null {
        return this.solanaAddress;
    }

    /**
     * Switch blockchain context
     */
    public setCurrentBlockchain(blockchain: "ethereum" | "solana"): void {
        this.currentBlockchain = blockchain;
        this.walletAddress = blockchain === "ethereum" ? this.ethereumAddress : this.solanaAddress;
        this._log(`Switched to ${blockchain} blockchain. Address: ${this.walletAddress}`);
    }

    /**
     * Reset DKG state for new session
     */
    public resetDkg(): void {
        this._log("Resetting DKG state");

        this.dkgState = DkgState.Idle;
        this.sessionInfo = null;
        this.frostDkg = null;
        this.participant_index = null;

        this.receivedRound1Packages.clear();
        this.receivedRound2Packages.clear();
        this.bufferedRound1Packages = [];
        this.bufferedRound2Packages = [];

        this.group_public_key = null;
        this.solanaAddress = null;
        this.ethereumAddress = null;
        this.walletAddress = null;

        this._updateDkgState(DkgState.Idle);
    }

    /**
     * Get DKG completion status
     */
    public isDkgComplete(): boolean {
        return this.dkgState === DkgState.Complete;
    }

    /**
     * Get DKG session information
     */
    public getSessionInfo(): SessionInfo | null {
        return this.sessionInfo;
    }

    // Private methods

    /**
     * Initialize FROST DKG instance
     */
    private _initializeFrostDkg(): void {
        try {
            if (!this.sessionInfo) {
                throw new Error("No session info available");
            }

            // Check if WASM is available
            const wasmModule = (globalThis as any).wasmModule;
            if (!wasmModule || !wasmModule.FrostDkgEd25519) {
                throw new Error("FROST DKG WASM module not available");
            }

            // Create FROST DKG instance
            this.frostDkg = new wasmModule.FrostDkgEd25519();
            this.frostDkg.init_dkg(
                this.participant_index!,
                this.sessionInfo.participants.length,
                this.sessionInfo.threshold
            );

            this._log(`FROST DKG initialized for participant ${this.participant_index}`);
        } catch (error) {
            this._log(`Failed to initialize FROST DKG: ${error}`);
            this._updateDkgState(DkgState.Failed);
        }
    }

    /**
     * Generate and broadcast Round 1 package
     */
    private _generateAndBroadcastRound1(): void {
        try {
            if (!this.frostDkg) {
                throw new Error("FROST DKG not initialized");
            }

            const round1Package = this.frostDkg.generate_round1();
            this._log(`Generated Round 1 package: ${round1Package.substring(0, 32)}...`);

            // Broadcast to all other participants
            this.sessionInfo!.participants.forEach(peerId => {
                if (peerId !== this.localPeerId) {
                    const message: WebRTCAppMessage = {
                        webrtc_msg_type: 'DkgRound1Package',
                        package: round1Package
                    };
                    this.callbacks.onSendMessage(peerId, message);
                }
            });

            this._log(`Broadcast Round 1 package to ${this.sessionInfo!.participants.length - 1} peers`);
        } catch (error) {
            this._log(`Failed to generate Round 1 package: ${error}`);
            this._updateDkgState(DkgState.Failed);
        }
    }

    /**
     * Process received Round 1 package
     */
    private _processRound1Package(fromPeerId: string, packageData: any): void {
        try {
            if (!this.frostDkg) {
                throw new Error("FROST DKG not initialized");
            }

            if (this.receivedRound1Packages.has(fromPeerId)) {
                this._log(`Duplicate Round 1 package from ${fromPeerId}, ignoring`);
                return;
            }

            // Use sorted participants for consistent identifier assignment
            const sortedParticipants = [...this.sessionInfo!.participants].sort();
            const participantIndex = sortedParticipants.indexOf(fromPeerId) + 1;
            if (participantIndex <= 0) {
                throw new Error(`Unknown peer: ${fromPeerId}`);
            }

            this.frostDkg.add_round1_package(participantIndex, packageData);
            this.receivedRound1Packages.add(fromPeerId);

            this._log(`Processed Round 1 package from ${fromPeerId} (${this.receivedRound1Packages.size}/${this.sessionInfo!.participants.length - 1})`);

            this._checkRound1Completion();
        } catch (error) {
            this._log(`Failed to process Round 1 package from ${fromPeerId}: ${error}`);
        }
    }

    /**
     * Check if Round 1 is complete and proceed to Round 2
     */
    private _checkRound1Completion(): void {
        const expectedPackages = this.sessionInfo!.participants.length - 1; // Exclude self

        if (this.receivedRound1Packages.size >= expectedPackages) {
            this._log(`Round 1 complete. Proceeding to Round 2.`);
            this._updateDkgState(DkgState.Round2InProgress);
            this._generateAndBroadcastRound2();
            this._processBufferedRound2Packages();
        }
    }

    /**
     * Generate and broadcast Round 2 packages
     */
    private _generateAndBroadcastRound2(): void {
        try {
            if (!this.frostDkg) {
                throw new Error("FROST DKG not initialized");
            }

            if (!this.frostDkg.can_start_round2()) {
                throw new Error("Not ready for Round 2");
            }

            const round2Packages = this.frostDkg.generate_round2();
            this._log(`Generated Round 2 packages`);

            // Parse and broadcast packages
            const packagesData = JSON.parse(round2Packages);

            Object.entries(packagesData).forEach(([participantIndexStr, packageData]) => {
                const participantIndex = parseInt(participantIndexStr);
                const peerId = this.sessionInfo!.participants[participantIndex - 1]; // Convert to 0-based

                if (peerId && peerId !== this.localPeerId) {
                    const message: WebRTCAppMessage = {
                        webrtc_msg_type: 'DkgRound2Package',
                        package: packageData
                    };
                    this.callbacks.onSendMessage(peerId, message);
                }
            });

            this._log(`Broadcast Round 2 packages to peers`);
        } catch (error) {
            this._log(`Failed to generate Round 2 packages: ${error}`);
            this._updateDkgState(DkgState.Failed);
        }
    }

    /**
     * Process received Round 2 package
     */
    private _processRound2Package(fromPeerId: string, packageData: any): void {
        try {
            if (!this.frostDkg) {
                throw new Error("FROST DKG not initialized");
            }

            if (this.receivedRound2Packages.has(fromPeerId)) {
                this._log(`Duplicate Round 2 package from ${fromPeerId}, ignoring`);
                return;
            }

            // Use sorted participants for consistent identifier assignment
            const sortedParticipants = [...this.sessionInfo!.participants].sort();
            const participantIndex = sortedParticipants.indexOf(fromPeerId) + 1;
            if (participantIndex <= 0) {
                throw new Error(`Unknown peer: ${fromPeerId}`);
            }

            this.frostDkg.add_round2_package(participantIndex, packageData);
            this.receivedRound2Packages.add(fromPeerId);

            this._log(`Processed Round 2 package from ${fromPeerId} (${this.receivedRound2Packages.size}/${this.sessionInfo!.participants.length - 1})`);

            this._checkRound2Completion();
        } catch (error) {
            this._log(`Failed to process Round 2 package from ${fromPeerId}: ${error}`);
        }
    }

    /**
     * Check if Round 2 is complete and finalize DKG
     */
    private _checkRound2Completion(): void {
        const expectedPackages = this.sessionInfo!.participants.length - 1; // Exclude self

        if (this.receivedRound2Packages.size >= expectedPackages) {
            this._log(`Round 2 complete. Finalizing DKG.`);
            this._updateDkgState(DkgState.Finalizing);
            this._finalizeDkg();
        }
    }

    /**
     * Finalize DKG and generate addresses
     */
    private _finalizeDkg(): void {
        try {
            if (!this.frostDkg) {
                throw new Error("FROST DKG not initialized");
            }

            const result = this.frostDkg.finalize_dkg();
            const finalData = JSON.parse(result);

            this.group_public_key = finalData.group_public_key;
            this.solanaAddress = finalData.solana_address;
            this.ethereumAddress = finalData.ethereum_address;

            // Set default address based on current blockchain
            this.walletAddress = this.currentBlockchain === "ethereum" ? this.ethereumAddress : this.solanaAddress;

            this._log(`DKG completed successfully!`);
            this._log(`Group Public Key: ${this.group_public_key}`);
            this._log(`Solana Address: ${this.solanaAddress}`);
            this._log(`Ethereum Address: ${this.ethereumAddress}`);
            
            // Save key share data for persistence
            this._saveKeyShare(finalData);

            this._updateDkgState(DkgState.Complete);
        } catch (error) {
            this._log(`Failed to finalize DKG: ${error}`);
            this._updateDkgState(DkgState.Failed);
        }
    }

    /**
     * Process buffered Round 1 packages
     */
    private _processBufferedRound1Packages(): void {
        if (this.bufferedRound1Packages.length === 0) return;

        this._log(`Processing ${this.bufferedRound1Packages.length} buffered Round 1 packages`);

        const packages = [...this.bufferedRound1Packages];
        this.bufferedRound1Packages = [];

        packages.forEach(pkg => {
            this._processRound1Package(pkg.fromPeerId, pkg.packageData);
        });
    }

    /**
     * Process buffered Round 2 packages
     */
    private _processBufferedRound2Packages(): void {
        if (this.bufferedRound2Packages.length === 0) return;

        this._log(`Processing ${this.bufferedRound2Packages.length} buffered Round 2 packages`);

        const packages = [...this.bufferedRound2Packages];
        this.bufferedRound2Packages = [];

        packages.forEach(pkg => {
            this._processRound2Package(pkg.fromPeerId, pkg.packageData);
        });
    }

    /**
     * Update DKG state and notify callbacks
     */
    private _updateDkgState(newState: DkgState): void {
        if (this.dkgState !== newState) {
            this.dkgState = newState;
            this._log(`DKG state: ${newState}`);
            this.callbacks.onDkgStateUpdate(newState);
        }
    }

    /**
     * Handle incoming DKG-related messages
     */
    async handleMessage(fromPeerId: string, message: WebRTCAppMessage): Promise<void> {
        this._log(`Handling DKG message from ${fromPeerId}: ${message.webrtc_msg_type}`);

        try {
            switch (message.webrtc_msg_type) {
                case 'DkgRound1Package':
                    if ('packageData' in message) {
                        this.handleDkgRound1Package(fromPeerId, (message as any).packageData);
                    }
                    break;

                case 'DkgRound2Package':
                    if ('packageData' in message) {
                        this.handleDkgRound2Package(fromPeerId, (message as any).packageData);
                    }
                    break;

                default:
                    this._log(`Unhandled DKG message type: ${message.webrtc_msg_type}`);
            }
        } catch (error) {
            this._log(`Error handling DKG message: ${error}`);
        }
    }

    /**
     * Cleanup DKG resources
     */
    async cleanup(): Promise<void> {
        this._log("Cleaning up DKG resources");

        // Free FROST DKG instance if it exists
        if (this.frostDkg) {
            try {
                if (typeof this.frostDkg.free === 'function') {
                    this.frostDkg.free();
                }
            } catch (error) {
                this._log(`Error freeing FROST DKG instance: ${error}`);
            }
            this.frostDkg = null;
        }

        // Reset state
        this.resetDkg();
    }

    /**
     * Log message with prefix
     */
    private _log(message: string): void {
        this.callbacks.onLog(`[DkgManager:${this.localPeerId}] ${message}`);
    }
    
    /**
     * Save key share data to keystore
     */
    private async _saveKeyShare(finalData: any): Promise<void> {
        try {
            if (!this.sessionInfo) {
                throw new Error("No session info available");
            }
            
            // Create blockchain info array matching CLI format
            const blockchains: import("@mpc-wallet/types/keystore").BlockchainInfo[] = [];
            
            // Add Ethereum support for secp256k1
            if (this.ethereumAddress && (this.currentBlockchain === 'ethereum' || finalData.ethereum_address)) {
                blockchains.push({
                    blockchain: "ethereum",
                    network: "mainnet",
                    chain_id: 1,
                    address: this.ethereumAddress || finalData.ethereum_address,
                    address_format: "EIP-55",
                    enabled: this.currentBlockchain === 'ethereum',
                    rpc_endpoint: undefined,
                    metadata: undefined
                });
            }
            
            // Add Solana support for ed25519
            if (this.solanaAddress && (this.currentBlockchain === 'solana' || finalData.solana_address)) {
                blockchains.push({
                    blockchain: "solana",
                    network: "mainnet",
                    chain_id: undefined,
                    address: this.solanaAddress || finalData.solana_address,
                    address_format: "base58",
                    enabled: this.currentBlockchain === 'solana',
                    rpc_endpoint: undefined,
                    metadata: undefined
                });
            }
            
            // Prepare key share data with CLI-compatible multi-chain format.
            // KeyShareData uses snake_case to match the Rust/CLI on-disk
            // shape — don't switch to camelCase here or cross-client
            // interop breaks.
            const keyShareData: import("@mpc-wallet/types/keystore").KeyShareData = {
                // Core FROST key material
                key_package: finalData.key_package || '',
                group_public_key: finalData.group_public_key,

                // Session information
                session_id: this.sessionInfo.session_id,
                device_id: this.localPeerId,
                participant_index: this.participant_index!,

                // Threshold configuration
                threshold: this.sessionInfo.threshold,
                total_participants: this.sessionInfo.total,
                participants: [...this.sessionInfo.participants],

                // Blockchain specific (multi-chain support)
                curve: this.currentBlockchain === 'ethereum' ? 'secp256k1' : 'ed25519',
                blockchains,

                // Legacy support for backward compatibility
                ethereum_address: this.ethereumAddress ?? undefined,
                solana_address: this.solanaAddress ?? undefined,

                // Metadata
                created_at: Date.now(),
            };
            
            // Send to background for storage
            this.callbacks.onLog(`[DkgManager] Sending key share for storage`);
            
            // Use a callback or send message to background
            if (this.callbacks.onDkgComplete) {
                this.callbacks.onDkgComplete(this.dkgState, keyShareData);
            }
            
        } catch (error) {
            this._log(`Failed to save key share: ${error}`);
        }
    }
}
