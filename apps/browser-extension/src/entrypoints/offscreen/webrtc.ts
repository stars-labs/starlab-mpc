import { SessionInfo, DkgState, MeshStatus, MeshStatusType } from "@mpc-wallet/types/appstate";
import { WebRTCAppMessage } from "@mpc-wallet/types/webrtc";
import { WebSocketMessagePayload, WebRTCSignal } from "@mpc-wallet/types/websocket";

export { DkgState, MeshStatusType }; // Export DkgState and MeshStatusType

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

// --- WebRTCManager Class ---
const ICE_SERVERS = [{ urls: 'stun:stun.l.google.com:19302' }]; // Example STUN server

export class WebRTCManager {
  private localPeerId: string;
  private peerConnections: Map<string, RTCPeerConnection> = new Map();
  private dataChannels: Map<string, RTCDataChannel> = new Map();

  public sessionInfo: SessionInfo | null = null;
  public invites: SessionInfo[] = []; // Store incoming session proposals/invites
  public dkgState: DkgState = DkgState.Idle;
  public meshStatus: MeshStatus = { type: MeshStatusType.Incomplete };
  private pendingIceCandidates: Map<string, RTCIceCandidateInit[]> = new Map();

  // Mesh ready tracking to prevent duplicate signals
  private ownMeshReadySent: boolean = false;

  // FROST DKG integration
  private frostDkg: any | null = null;
  private participantIndex: number | null = null;
  private receivedRound1Packages: Set<string> = new Set();
  private receivedRound2Packages: Set<string> = new Set();
  private groupPublicKey: string | null = null;
  private solanaAddress: string | null = null;
  private ethereumAddress: string | null = null; // Ethereum address property
  private walletAddress: string | null = null; // Generic address property for current blockchain
  private currentBlockchain: "ethereum" | "solana" = "solana"; // Store current blockchain selection

  // Package buffering for handling packages that arrive before DKG initialization
  private bufferedRound1Packages: Array<{ fromPeerId: string; packageData: any }> = [];
  private bufferedRound2Packages: Array<{ fromPeerId: string; packageData: any }> = [];

  // FROST Signing integration
  public signingState: SigningState = SigningState.Idle;
  public signingInfo: SigningInfo | null = null;
  private signingCommitments: Map<string, any> = new Map(); // Map peer to commitment data
  private signingShares: Map<string, any> = new Map(); // Map peer to signature share data

  // Callbacks
  public onLog: (message: string) => void = console.log;
  public onSessionUpdate: (sessionInfo: SessionInfo | null, invites: SessionInfo[]) => void = () => { };
  public onMeshStatusUpdate: (status: MeshStatus) => void = () => { };
  public onWebRTCAppMessage: (fromPeerId: string, message: WebRTCAppMessage) => void = () => { };
  public onDkgStateUpdate: (state: DkgState) => void = () => { };
  /**
   * Fires exactly once per ceremony after the FROST DKG finalize
   * succeeds. Payload carries everything the save-keyshare flow
   * needs: group public key, derived address for the selected
   * chain, session context. Kept separate from `onDkgStateUpdate`
   * so late subscribers don't have to race on "was Complete already
   * emitted?" — if this fires, the ceremony is terminally done.
   */
  public onDkgComplete: (payload: {
    groupPublicKey: string;
    address: string | null;
    blockchain: "ethereum" | "solana";
    sessionId: string | null;
    threshold: number;
    total: number;
    participants: string[];
    participantIndex: number | null;
    /** WASM-exported keystore JSON — carries the key package bytes,
     *  FROST identifiers, and public key commitments for each
     *  participant. The background's KeystoreManager consumes this
     *  (via addWallet) to encrypt + persist the keyshare once the
     *  user picks a password. `null` when export failed (logged but
     *  non-fatal). */
    keystoreJson: string | null;
  }) => void = () => { };
  public onSigningStateUpdate: (state: SigningState, info: SigningInfo | null) => void = () => { };
  /**
   * Ext-2d-progress: fires on every signing-ceremony progression
   * milestone (commitment sent/received, share sent/received) with
   * the current per-participant roster. TUI parity with the
   * `signing_commitments_received` / `signing_shares_received`
   * HashSets (051cdbc) that overlay ✓ / ✓✓ on the participant list
   * while the ceremony runs — lets the creator see Bob committing
   * and Carol sharing in real time instead of staring at a blank
   * "waiting" screen between kickoff and SignatureComplete.
   *
   * The payload is JSON-plain (arrays, not Maps) so it crosses the
   * runtime.sendMessage boundary without serialization gymnastics.
   */
  public onSigningProgress: (payload: {
    signingId: string;
    state: SigningState;
    selectedSigners: string[];
    commitmentsReceived: string[];
    sharesReceived: string[];
  }) => void = () => { };
  /**
   * Ext-2d-offscreen-rounds: fires once per ceremony when the final
   * aggregated signature is available. All signers receive this
   * (the aggregator after calling `aggregate_signature`, co-signers
   * when they receive the AggregatedSignature broadcast). Payload
   * carries enough context for the popup to render a
   * SignatureComplete screen and for the dApp bridge (future
   * Ext-4) to resolve its pending personal_sign RPC.
   */
  public onSigningComplete: (payload: {
    signingId: string;
    signature: string;
    messageHex: string;
    blockchain: "ethereum" | "solana";
    /** The session_id the signing_id was derived from (strips the
     *  `sign_` prefix). Used by the popup to cross-reference the
     *  session banner with the final signature. */
    sessionId: string;
  }) => void = () => { };
  public onWebRTCConnectionUpdate: (peerId: string, connected: boolean) => void = () => { };

  // Add the missing callback property and constructor parameter
  private sendPayloadToBackgroundForRelay: ((toPeerId: string, payload: WebSocketMessagePayload) => void) | null = null;

  constructor(localPeerId: string, sendPayloadCallback?: (toPeerId: string, payload: WebSocketMessagePayload) => void) {

    if (typeof localPeerId !== 'string') {
      // Use console.warn for this initial setup phase, as _log depends on localPeerId which is being initialized.
      // JSON.stringify might fail for complex objects or circular refs, but good for simple ones.
      let localPeerIdStringRepresentation = '';
      try {
        localPeerIdStringRepresentation = JSON.stringify(localPeerId);
      } catch (e) {
        localPeerIdStringRepresentation = '[Unserializable Object]';
      }
      console.warn(`[WebRTCManager] Constructor: localPeerId expected a string but received type ${typeof localPeerId}. Value: ${localPeerIdStringRepresentation}`);

      if (localPeerId && typeof (localPeerId as any).id === 'string') {
        this.localPeerId = (localPeerId as any).id;
        console.warn(`[WebRTCManager] Constructor: Using 'id' property from localPeerId object: ${this.localPeerId}`);
      } else {
        this.localPeerId = String(localPeerId); // Fallback, may result in "[object Object]"
        console.warn(`[WebRTCManager] Constructor: Fallback: Converted localPeerId to string: ${this.localPeerId}. This may not be the intended ID. Please check instantiation site.`);
      }
    } else {
      this.localPeerId = localPeerId;
    }

    this.sendPayloadToBackgroundForRelay = sendPayloadCallback || null;
  }

  private _log(message: string) {
    const curve = this.currentBlockchain === "ethereum" ? "secp256k1" : "ed25519";
    this.onLog(`[WebRTCManager-${this.localPeerId}][${curve}] ${message}`);
  }

  private _isTestEnvironment(): boolean {
    return typeof global !== 'undefined' &&
      (global as any).IS_TESTING === true ||
      typeof process !== 'undefined' && process.env.NODE_ENV === 'test' ||
      typeof (globalThis as any).Bun !== 'undefined';
  }

  private _logVerbose(message: string) {
    // Only log verbose messages in non-test environments
    if (!this._isTestEnvironment()) {
      this._log(message);
    }
  }

  private _getErrorMessage(error: any): string {
    if (error instanceof Error) {
      return error.message;
    }
    if (typeof error === 'string') {
      return error;
    }
    if (error && typeof error === 'object' && error.message) {
      return error.message;
    }
    return JSON.stringify(error);
  }

  /**
   * FROST participant identifiers are derived from the SORTED participant
   * device-id order — this MUST match the Rust core's `canonical_identifier`
   * (apps/tui-node/src/protocal/dkg.rs), which does `participants.sort()` then
   * `position + 1`. The signal server grows the participant list in *join*
   * order, which is generally NOT sorted; if we used it as-is, our `indexOf`
   * based identifiers would disagree with the Rust nodes and a mixed
   * extension↔CLI/TUI ceremony would fail. Normalizing here (a plain
   * lexicographic sort, matching Rust `Vec<String>::sort()`) makes every
   * identifier in this class canonical regardless of join order. (#29)
   */
  private _withSortedParticipants(info: SessionInfo | null): SessionInfo | null {
    if (!info || !Array.isArray(info.participants)) return info;
    return { ...info, participants: [...info.participants].sort() };
  }

  private _updateSession(newSessionInfo: SessionInfo | null) {
    this.sessionInfo = this._withSortedParticipants(newSessionInfo);
    this.onSessionUpdate(this.sessionInfo, this.invites);
  }

  private _updateMeshStatus(newStatus: MeshStatus) {
    this.meshStatus = newStatus;
    this.onMeshStatusUpdate(this.meshStatus);

    if (newStatus.type === MeshStatusType.Ready) {
      this._log("Mesh is Ready! Waiting for explicit DKG trigger from background script.");
      // Do NOT automatically trigger DKG here to avoid race conditions
      // DKG will be triggered explicitly by the background script via sessionAllAccepted
    }
  }

  private _updateDkgState(newState: DkgState) {
    this.dkgState = newState;
    this.onDkgStateUpdate(this.dkgState);
  }

  private _updateSigningState(newState: SigningState, info: SigningInfo | null = null) {
    this.signingState = newState;
    this.signingInfo = info;
    this.onSigningStateUpdate(this.signingState, this.signingInfo);
  }

  public handleWebSocketMessagePayload(fromPeerId: string, msg: WebSocketMessagePayload): void {
    this._log(`Received WebSocketMessage from ${fromPeerId}: ${msg.websocket_msg_type}`);
    this._log(`Full message payload: ${JSON.stringify(msg)}`);

    switch (msg.websocket_msg_type) {
      case 'WebRTCSignal':
        this._log(`WebRTCSignal data: ${JSON.stringify(msg)}`);

        // Accept WebRTC signals from any peer - no session requirement
        this._log(`Processing WebRTC signal from ${fromPeerId} (no session check)`);

        // Handle different message structures
        let signalData = null;
        if ((msg as any).data) {
          // Standard structure: { data: { type: "Offer/Answer/Candidate", data: {...} } }
          signalData = (msg as any).data;
        } else if ((msg as any).Offer) {
          // Server structure: { Offer: {...}, websocket_msg_type: "WebRTCSignal" }
          signalData = { type: 'Offer', data: (msg as any).Offer };
        } else if ((msg as any).Answer) {
          // Server structure: { Answer: {...}, websocket_msg_type: "WebRTCSignal" }
          signalData = { type: 'Answer', data: (msg as any).Answer };
        } else if ((msg as any).Candidate) {
          // Server structure: { Candidate: {...}, websocket_msg_type: "WebRTCSignal" }
          signalData = { type: 'Candidate', data: (msg as any).Candidate };
        }

        if (signalData) {
          this._log(`Extracted WebRTC signal: ${JSON.stringify(signalData)}`);
          this.handleWebRTCSignal(fromPeerId, signalData as WebRTCSignal);
        } else {
          this._log(`WebRTCSignal from ${fromPeerId} missing data - full msg: ${JSON.stringify(msg)}`);
        }
        break;

      default:
        // Handle unknown message types with proper logging
        this._log(`Unknown WebSocketMessage type from ${fromPeerId}: ${(msg as any).websocket_msg_type}. Full payload: ${JSON.stringify(msg)}`);
        break;
    }
  }

  public async handleWebRTCSignal(fromPeerId: string, signal: any): Promise<void> {
    try {
      this._log(`handleWebRTCSignal called with: ${JSON.stringify(signal)}`);

      // Normalize signal structure
      let actualSignal = signal;
      if (signal && signal.type && signal.data) {
        actualSignal = signal;
      } else if (signal && (signal.sdp || signal.candidate)) {
        if (signal.sdp) {
          actualSignal = {
            type: signal.type || (signal.sdp.includes('a=sendrecv') ? 'Offer' : 'Answer'),
            data: { sdp: signal.sdp }
          };
        } else if (signal.candidate) {
          actualSignal = {
            type: 'Candidate',
            data: {
              candidate: signal.candidate,
              sdpMid: signal.sdpMid,
              sdpMLineIndex: signal.sdpMLineIndex
            }
          };
        }
      } else {
        this._log(`Invalid WebRTCSignal structure from ${fromPeerId}: ${JSON.stringify(signal)}`);
        return;
      }

      this._log(`Processing WebRTCSignal ${actualSignal.type} from ${fromPeerId}`);

      const pc = await this._getOrCreatePeerConnection(fromPeerId);
      if (!pc) {
        this._log(`No peer connection for ${fromPeerId} to handle signal.`);
        return;
      }

      // Comprehensive pattern matching for signal types
      switch (actualSignal.type) {
        case 'Offer':
          if (actualSignal.data && actualSignal.data.sdp) {
            // When receiving an offer, the offerer should have created the data channel
            // We'll receive it via ondatachannel event
            await pc.setRemoteDescription(new RTCSessionDescription({ type: 'offer', sdp: actualSignal.data.sdp }));
            this._log(`Set remote offer from ${fromPeerId}. Creating answer.`);

            const answer = await pc.createAnswer();
            await pc.setLocalDescription(answer);

            // Create WebSocketMessage that matches Rust enum structure exactly
            const wsMsgPayload = {
              websocket_msg_type: 'WebRTCSignal',
              Answer: { sdp: answer.sdp! }  // Direct at root level, no nesting
            };

            if (this.sendPayloadToBackgroundForRelay) {
              this.sendPayloadToBackgroundForRelay(fromPeerId, wsMsgPayload as any);
              this._log(`Sent Answer to ${fromPeerId} via background`);
            } else {
              this._log(`Cannot send Answer to ${fromPeerId}: no relay callback available`);
            }
          } else {
            this._log(`Offer from ${fromPeerId} missing SDP data. Data: ${JSON.stringify(actualSignal.data)}`);
          }
          break;

        case 'Answer':
          if (actualSignal.data && actualSignal.data.sdp) {
            await pc.setRemoteDescription(new RTCSessionDescription({ type: 'answer', sdp: actualSignal.data.sdp }));
            this._log(`Set remote answer from ${fromPeerId}. Connection should be established soon.`);
          } else {
            this._log(`Answer from ${fromPeerId} missing SDP data. Data: ${JSON.stringify(actualSignal.data)}`);
          }
          break;

        case 'Candidate':
          if (actualSignal.data && actualSignal.data.candidate) {
            // Fix: Handle empty string sdpMid but valid sdpMLineIndex
            const sdpMid = actualSignal.data.sdpMid && actualSignal.data.sdpMid.trim() !== ""
              ? actualSignal.data.sdpMid
              : null;

            // Keep original sdpMLineIndex value (0 is valid!)
            const sdpMLineIndex = actualSignal.data.sdpMLineIndex;

            const candidate = new RTCIceCandidate({
              candidate: actualSignal.data.candidate,
              sdpMid: sdpMid,
              sdpMLineIndex: sdpMLineIndex,
            });

            if (pc.remoteDescription) {
              await pc.addIceCandidate(candidate);
              this._log(`Added ICE candidate from ${fromPeerId}`);
            } else {
              this._log(`Queued ICE candidate from ${fromPeerId} (remote description not set)`);
              const pending = this.pendingIceCandidates.get(fromPeerId) || [];
              pending.push(candidate);
              this.pendingIceCandidates.set(fromPeerId, pending);
            }
          } else {
            this._log(`Candidate from ${fromPeerId} missing candidate data. Data: ${JSON.stringify(actualSignal.data)}`);
          }
          break;

        // Handle potential additional signal types
        case 'offer':
        case 'answer':
          this._log(`Received lowercase signal type '${actualSignal.type}' from ${fromPeerId}, converting to title case`);
          // Recursively handle with proper casing
          const normalizedSignal = {
            ...actualSignal,
            type: actualSignal.type.charAt(0).toUpperCase() + actualSignal.type.slice(1)
          };
          await this.handleWebRTCSignal(fromPeerId, normalizedSignal);
          break;

        default:
          this._log(`Unknown signal type '${actualSignal.type}' from ${fromPeerId}. Full signal: ${JSON.stringify(actualSignal)}`);
          break;
      }
    } catch (error) {
      this._log(`Error handling WebRTCSignal from ${fromPeerId}: ${this._getErrorMessage(error)}. Signal: ${JSON.stringify(signal)}`);
    }
  }

  // --- Session Management ---
  public resetSession(): void {
    this._log("Resetting session state.");

    // Report all connections as disconnected
    this.peerConnections.forEach((pc, peerId) => {
      this.onWebRTCConnectionUpdate(peerId, false);
      pc.close();
    });

    this.peerConnections.clear();
    this.dataChannels.clear();
    this.pendingIceCandidates.clear();
    this._updateSession(null);
    this.invites = [];
    this.onSessionUpdate(this.sessionInfo, this.invites);
    this._updateMeshStatus({ type: MeshStatusType.Incomplete });
    this._updateDkgState(DkgState.Idle);

    // Reset mesh ready flag to allow mesh_ready signals for new sessions
    this.ownMeshReadySent = false;
    this._log("Reset ownMeshReadySent flag for session reset");

    // Reset FROST DKG state
    this._resetDkgState();
  }

  public sendWebRTCAppMessage(toPeerId: string, message: WebRTCAppMessage): void {
    const dc = this.dataChannels.get(toPeerId);
    if (dc && dc.readyState === 'open') {
      dc.send(JSON.stringify(message));
      this._log(`Sent WebRTCAppMessage to ${toPeerId}: ${JSON.stringify(message)}`);
    } else {
      // Use verbose logging for expected failures in test environment
      this._logVerbose(`Cannot send WebRTCAppMessage to ${toPeerId}: data channel not open or doesn't exist.`);
    }
  }

  /**
   * Expose the derived address / group public key for the popup's
   * address-display flows. Offscreen/index.ts calls these by name
   * from the background-command switch; fields are private so they
   * need public accessors. Returns null when the ceremony hasn't
   * completed yet.
   */
  public getGroupPublicKey(): string | null {
    return this.groupPublicKey;
  }
  public getEthereumAddress(): string | null {
    return this.ethereumAddress;
  }
  public getSolanaAddress(): string | null {
    return this.solanaAddress;
  }

  /**
   * Send a plain text / debug message to a peer over the data
   * channel. Distinct from sendWebRTCAppMessage (which requires a
   * discriminated WebRTCAppMessage shape) — this just wraps the
   * string in a SimpleMessage envelope. Used by the popup's
   * developer-debug Send Direct Message feature. Returns whether
   * the send was attempted (false if data channel isn't open).
   */
  public sendDirectMessage(toPeerId: string, text: string): boolean {
    const dc = this.dataChannels.get(toPeerId);
    if (dc && dc.readyState === 'open') {
      dc.send(JSON.stringify({
        webrtc_msg_type: 'SimpleMessage',
        text,
      }));
      this._log(`Sent direct SimpleMessage to ${toPeerId}: ${text}`);
      return true;
    }
    this._logVerbose(`Cannot send direct message to ${toPeerId}: data channel not open.`);
    return false;
  }

  // Missing private methods that tests are calling
  private _handlePeerDisconnection(peerId: string): void {
    this._log(`Handling peer disconnection for ${peerId}`);

    // Close and remove data channel
    const dc = this.dataChannels.get(peerId);
    if (dc) {
      dc.close();
      this.dataChannels.delete(peerId);
    }

    // Close and remove peer connection
    const pc = this.peerConnections.get(peerId);
    if (pc) {
      pc.close();
      this.peerConnections.delete(peerId);
    }

    // Clear any pending ICE candidates
    this.pendingIceCandidates.delete(peerId);

    // Update connection status
    this.onWebRTCConnectionUpdate(peerId, false);

    // Update mesh status - remove disconnected peer from ready_devices
    if (this.meshStatus.type === MeshStatusType.Ready ||
      (this.meshStatus.type === MeshStatusType.PartiallyReady && (this.meshStatus as any).ready_devices)) {
      const currentStatus = this.meshStatus;
      let readyPeers: Set<string>;

      if (this.meshStatus.type === MeshStatusType.PartiallyReady && (this.meshStatus as any).ready_devices) {
        // Copy existing ready_devices
        readyPeers = new Set((currentStatus as any).ready_devices);
      } else {
        // Create from all participants except the disconnected one
        readyPeers = new Set(this.sessionInfo?.participants || []);
      }

      // Remove the disconnected peer
      readyPeers.delete(peerId);

      // Update the mesh status
      const totalPeers = this.sessionInfo?.participants.length || 0;
      if (readyPeers.size >= totalPeers) {
        this._updateMeshStatus({ type: MeshStatusType.Ready });
      } else {
        this._updateMeshStatus({
          type: MeshStatusType.PartiallyReady,
          ready_devices: readyPeers,
          total_devices: totalPeers
        });
      }
    }
  }

  private _sendWebRTCMessage(toPeerId: string, message: WebRTCAppMessage): void {
    this._log(`Sending WebRTC message to ${toPeerId}: ${JSON.stringify(message)}`);
    this.sendWebRTCAppMessage(toPeerId, message);
  }

  // Method to send MeshReady signals to all peers
  private _sendMeshReadyToAllPeers(): void {
    if (!this.sessionInfo) {
      this._log("❌ Cannot send MeshReady: no session info");
      return;
    }

    this._log(`📡 SENDING MESH_READY SIGNALS to all peers`);
    this._log(`Session ID: ${this.sessionInfo.session_id || 'unknown'}`);
    this._log(`Local Peer ID: ${this.localPeerId}`);
    this._log(`Target peers: [${this.sessionInfo.participants.filter(p => p !== this.localPeerId).join(', ')}]`);

    const meshReadyMsg: WebRTCAppMessage = {
      webrtc_msg_type: 'MeshReady',
      session_id: this.sessionInfo.session_id,
      device_id: this.localPeerId
    };

    let sentCount = 0;
    this.sessionInfo.participants.forEach(peerId => {
      if (peerId !== this.localPeerId) {
        const dc = this.dataChannels.get(peerId);
        if (dc && dc.readyState === 'open') {
          this.sendWebRTCAppMessage(peerId, meshReadyMsg);
          sentCount++;
          this._log(`✅ Sent MeshReady signal to ${peerId}`);
        } else {
          this._log(`❌ Cannot send MeshReady to ${peerId}: data channel not open`);
        }
      }
    });

    // Set the flag to prevent duplicate signals even if we couldn't send to all peers
    this.ownMeshReadySent = true;
    this._log(`✅ Set ownMeshReadySent flag to prevent duplicate mesh_ready signals`);
    this._log(`📡 MESH_READY SIGNALS SENT: ${sentCount} signals sent to peers`);
  }

  private async _replayBufferedDkgPackages(): Promise<void> {
    this._log(`🔄 Replaying buffered DKG packages`);

    try {
      // Process any buffered Round 1 packages directly (skip the handler to avoid loops)
      if (this.bufferedRound1Packages.length > 0) {
        this._log(`🔄 Replaying ${this.bufferedRound1Packages.length} buffered Round 1 packages`);
        this._log(`🔄 Current WASM packages before replay: ${this.frostDkg ? 'checking...' : 'no FROST DKG'}`);
        
        // Check WASM state before replay
        if (this.frostDkg) {
          try {
            const canStartBefore = this.frostDkg.can_start_round2();
            this._log(`🔄 WASM can_start_round2 before replay: ${canStartBefore}`);
          } catch (e) {
            this._log(`🔄 Error checking WASM state before replay: ${this._getErrorMessage(e)}`);
          }
        }

        // Create a copy of the buffer to avoid modification during iteration
        const round1Packages = [...this.bufferedRound1Packages];
        this.bufferedRound1Packages = [];

        // Debug session info
        this._log(`🔄 Session participants: ${JSON.stringify(this.sessionInfo?.participants || [])}`);
        this._log(`🔄 Local peer ID: ${this.localPeerId}`);

        // Process each buffered package directly without going through the handler
        for (const { fromPeerId, packageData } of round1Packages) {
          this._log(`🔄 Replaying Round 1 package from ${fromPeerId}`);
          this._log(`🔄 Package data type: ${typeof packageData}, preview: ${JSON.stringify(packageData).substring(0, 100)}...`);
          
          // Skip our own package. frost-core's dkg::part2 contract
          // (see frost-core keys/dkg.rs:505) expects exactly n-1
          // round-1 packages in round1_packages — the signer's own
          // is held as a secret_package on the WASM instance and
          // must NOT be re-added via add_round1_package.
          if (fromPeerId === this.localPeerId) {
            this._log(`🔄 Skipping own Round 1 package during replay (per frost-core n-1 contract)`);
            this.receivedRound1Packages.add(fromPeerId);
            continue;
          }

          // Process the package directly with FROST DKG
          try {
            const participantIndex = this.sessionInfo?.participants.indexOf(fromPeerId);
            const senderIndex = (participantIndex ?? -1) + 1;
            
            this._log(`🔄 Participant index for ${fromPeerId}: ${participantIndex}, sender index: ${senderIndex}`);
            
            if (participantIndex === -1 || participantIndex === undefined) {
              this._log(`🚨 ERROR: ${fromPeerId} not found in session participants list`);
              continue;
            }

            let packageHex: string;

            // Handle package format conversion (same logic as in handler)
            if (typeof packageData === 'string') {
              packageHex = packageData;
              this._log(`🔄 Using string package data`);
            } else if (packageData.data) {
              packageHex = packageData.data;
              this._log(`🔄 Using legacy format with data property`);
            } else {
              // CLI-compatible format: convert JSON to hex
              const packageString = JSON.stringify(packageData);
              const packageBytes = new TextEncoder().encode(packageString);
              packageHex = Array.from(packageBytes).map(b => b.toString(16).padStart(2, '0')).join('');
              this._log(`🔄 Converted CLI format to hex (${packageHex.length} chars)`);
            }

            this._log(`🔄 Package hex length: ${packageHex.length}, starts with: ${packageHex.substring(0, 20)}...`);

            if (senderIndex <= 0) {
              this._log(`🚨 ERROR: Invalid sender index ${senderIndex} for ${fromPeerId}`);
              continue;
            }
            
            if (!packageHex) {
              this._log(`🚨 ERROR: Empty package hex for ${fromPeerId}`);
              continue;
            }
            
            if (!this.frostDkg) {
              this._log(`🚨 ERROR: No FROST DKG instance available`);
              continue;
            }

            // Add the package to WASM with detailed error handling
            this._log(`🔄 About to call frostDkg.add_round1_package(${senderIndex}, packageHex)`);
            this.frostDkg.add_round1_package(senderIndex, packageHex);
            this.receivedRound1Packages.add(fromPeerId);
            this._log(`🔄 ✅ Successfully processed buffered Round 1 package from ${fromPeerId}`);
            
            // Check WASM state after each package
            try {
              const canStartAfter = this.frostDkg.can_start_round2();
              this._log(`🔄 WASM can_start_round2 after adding ${fromPeerId}: ${canStartAfter}`);
            } catch (e) {
              this._log(`🔄 Error checking WASM state after adding ${fromPeerId}: ${this._getErrorMessage(e)}`);
            }
            
          } catch (error) {
            this._log(`🚨 Error processing buffered Round 1 package from ${fromPeerId}: ${this._getErrorMessage(error)}`);
            this._log(`🔄 Error details: ${JSON.stringify(error)}`);
            // Continue processing other packages even if one fails
          }
        }
        
        // Final WASM state check
        if (this.frostDkg) {
          try {
            const finalCanStart = this.frostDkg.can_start_round2();
            this._log(`🔄 Final WASM can_start_round2 after replay: ${finalCanStart}`);
            this._log(`🔄 Final received packages count: ${this.receivedRound1Packages.size}`);
            this._log(`🔄 Expected participants: ${this.sessionInfo?.participants.length || 0}`);
          } catch (e) {
            this._log(`🔄 Error checking final WASM state: ${this._getErrorMessage(e)}`);
          }
        }
      }

      // Process any buffered Round 2 packages
      if (this.bufferedRound2Packages.length > 0 && this.dkgState === DkgState.Round2InProgress) {
        this._log(`🔄 Replaying ${this.bufferedRound2Packages.length} buffered Round 2 packages`);

        // Create a copy of the buffer to avoid modification during iteration
        const round2Packages = [...this.bufferedRound2Packages];
        this.bufferedRound2Packages = [];

        // Process each buffered package
        for (const { fromPeerId, packageData } of round2Packages) {
          this._log(`🔄 Replaying Round 2 package from ${fromPeerId}`);
          await this._handleDkgRound2Package(fromPeerId, packageData);
        }
      }
    } catch (error) {
      this._log(`🚨 ERROR in _replayBufferedDkgPackages: ${this._getErrorMessage(error)}`);
      this._log(`🔍 Error details: ${JSON.stringify(error)}`);
      throw error; // Re-throw to let caller handle
    }
  }

  public async initializeDkg(blockchain: "ethereum" | "solana" = "solana", threshold: number = 0, participants: string[] = [], participantIndex: number = 0): Promise<boolean> {
    // Set blockchain first to ensure correct curve is shown in logs
    this.currentBlockchain = blockchain;

    this._log(`Initializing DKG process for ${blockchain}`);

    if (!this.sessionInfo && participants.length === 0) {
      this._log(`Cannot initialize DKG: no session info or participants provided`);
      return false;
    }

    if (this.dkgState !== DkgState.Idle) {
      this._log(`Cannot initialize DKG: already in progress (state: ${DkgState[this.dkgState]})`);
      return false;
    }

    try {
      // Set to Initializing state immediately to prevent race conditions
      this._updateDkgState(DkgState.Initializing);

      // Reset DKG state
      this._resetDkgState();

      // Set participant index either from params or from session info.
      // Sort the list so the 1-based index matches the Rust core's
      // canonical_identifier (sorted order). If participants come from
      // sessionInfo they're already sorted (_withSortedParticipants), but a
      // direct `participants` arg may not be — sort defensively. (#29)
      const participants_list = (participants.length > 0 ?
        participants :
        this.sessionInfo?.participants || []).slice().sort();

      const threshold_count = threshold > 0 ?
        threshold :
        Math.ceil(participants_list.length / 2); // Default to n/2 + 1

      this.participantIndex = participantIndex > 0 ?
        participantIndex :
        (participants_list.indexOf(this.localPeerId) + 1) || 0; // 1-based, sorted

      if (this.participantIndex <= 0 || this.participantIndex > participants_list.length) {
        throw new Error(`Invalid participant index: ${this.participantIndex}`);
      }

      // Initialize FROST DKG using WebAssembly
      console.log('🔍 FROST DKG INIT: Starting WASM module resolution');
      console.log('🔍 FROST DKG INIT: typeof global:', typeof global);
      console.log('🔍 FROST DKG INIT: typeof window:', typeof window);
      console.log('🔍 FROST DKG INIT: typeof globalThis:', typeof globalThis);
      
      // Check all possible locations for FROST DKG modules
      const FrostDkgEd25519 = 
        (typeof global !== 'undefined' && (global as any).FrostDkgEd25519) ||
        (typeof window !== 'undefined' && (window as any).FrostDkgEd25519) ||
        (typeof globalThis !== 'undefined' && (globalThis as any).FrostDkgEd25519) ||
        null;

      const FrostDkgSecp256k1 = 
        (typeof global !== 'undefined' && (global as any).FrostDkgSecp256k1) ||
        (typeof window !== 'undefined' && (window as any).FrostDkgSecp256k1) ||
        (typeof globalThis !== 'undefined' && (globalThis as any).FrostDkgSecp256k1) ||
        null;

      console.log('🔍 FROST DKG INIT: FrostDkgEd25519 resolved to:', FrostDkgEd25519 ? 'FOUND' : 'NULL');
      console.log('🔍 FROST DKG INIT: FrostDkgSecp256k1 resolved to:', FrostDkgSecp256k1 ? 'FOUND' : 'NULL');
      console.log('🔍 FROST DKG INIT: global.FrostDkgEd25519:', typeof global !== 'undefined' ? typeof (global as any)?.FrostDkgEd25519 : 'global undefined');
      console.log('🔍 FROST DKG INIT: global.FrostDkgSecp256k1:', typeof global !== 'undefined' ? typeof (global as any)?.FrostDkgSecp256k1 : 'global undefined');
      console.log('🔍 FROST DKG INIT: (window as any).FrostDkgEd25519:', typeof (window as any)?.FrostDkgEd25519);
      console.log('🔍 FROST DKG INIT: (window as any).FrostDkgSecp256k1:', typeof (window as any)?.FrostDkgSecp256k1);
      console.log('🔍 FROST DKG INIT: (globalThis as any).FrostDkgEd25519:', typeof (globalThis as any)?.FrostDkgEd25519);
      console.log('🔍 FROST DKG INIT: (globalThis as any).FrostDkgSecp256k1:', typeof (globalThis as any)?.FrostDkgSecp256k1);

      if (!FrostDkgEd25519 || !FrostDkgSecp256k1) {
        console.log('🚨 FROST DKG INIT: WASM modules not found - this will cause failure');
        console.log('🚨 FROST DKG INIT: Available on global:', global ? Object.keys(global).filter(k => k.includes('Frost')) : 'global is null');
        console.log('🚨 FROST DKG INIT: Available on window:', typeof window !== 'undefined' ? Object.keys(window).filter(k => k.includes('Frost')) : 'window is undefined');
        console.log('🚨 FROST DKG INIT: Available on globalThis:', globalThis ? Object.keys(globalThis).filter(k => k.includes('Frost')) : 'globalThis is null');
        throw new Error('FROST DKG WebAssembly modules not found');
      }

      if (blockchain === "ethereum") {
        // Use secp256k1 for Ethereum
        this.frostDkg = new FrostDkgSecp256k1();
        this._log('Created secp256k1 FROST DKG instance for Ethereum');
      } else {
        // Use ed25519 for Solana
        this.frostDkg = new FrostDkgEd25519();
        this._log('Created ed25519 FROST DKG instance for Solana');
      }

      // Initialize the DKG with participant count and threshold
      this.frostDkg.init_dkg(
        this.participantIndex,
        participants_list.length,
        threshold_count
      );

      this._updateDkgState(DkgState.Round1InProgress);
      this._log(`DKG initialized successfully with ${participants_list.length} participants and threshold ${threshold_count}`);

      // Generate and broadcast our Round 1 package
      await this._generateAndBroadcastRound1();

      // Process any buffered packages now that we're initialized
      await this._replayBufferedDkgPackages();

      // Check if we can proceed to Round 2 after processing buffered packages
      this._log(`🔍 POST-REPLAY CHECK: receivedRound1Packages.size=${this.receivedRound1Packages.size}, expected=${this.sessionInfo?.participants.length || 0}`);
      this._log(`🔍 POST-REPLAY CHECK: receivedRound1Packages contents: [${Array.from(this.receivedRound1Packages).join(', ')}]`);
      this._log(`🔍 POST-REPLAY CHECK: sessionInfo.participants: [${this.sessionInfo?.participants.join(', ') || 'none'}]`);
      
      if (this.frostDkg) {
        try {
          const canStartRound2 = this.frostDkg.can_start_round2();
          this._log(`🔍 POST-REPLAY CHECK: WASM can_start_round2=${canStartRound2}`);
        } catch (e) {
          this._log(`🔍 POST-REPLAY CHECK: Error checking can_start_round2: ${this._getErrorMessage(e)}`);
        }
      }
      
      // Check if any participants are missing from received packages
      if (this.sessionInfo) {
        const missing = this.sessionInfo.participants.filter(p => !this.receivedRound1Packages.has(p));
        if (missing.length > 0) {
          this._log(`🚨 POST-REPLAY CHECK: Missing Round 1 packages from: [${missing.join(', ')}]`);

          // Request missing Round 1 packages from peers. Stub —
          // peers will re-broadcast on the next mesh-ready cycle
          // via the normal ordering; an explicit request protocol
          // hasn't been wired yet. Logging so the ask is visible
          // if the retry path ever becomes needed.
          this._log(`📡 Need missing Round 1 packages from: [${missing.join(', ')}] — awaiting peer re-broadcast`);
        }
      }
      
      if (this.sessionInfo && 
          this.receivedRound1Packages.size >= this.sessionInfo.participants.length &&
          this.frostDkg.can_start_round2()) {
        this._log(`✅ All Round 1 packages received after replay, proceeding to Round 2`);
        this._updateDkgState(DkgState.Round2InProgress);
        await this._generateAndBroadcastRound2();
        
        // Process any buffered Round 2 packages
        await this._replayBufferedDkgPackages();
      } else {
        this._log(`❌ Cannot proceed to Round 2 yet:`);
        this._log(`   - Session info: ${!!this.sessionInfo}`);
        this._log(`   - Received packages: ${this.receivedRound1Packages.size}`);
        this._log(`   - Expected packages: ${this.sessionInfo?.participants.length || 0}`);
        this._log(`   - WASM can_start_round2: ${this.frostDkg ? 'checking...' : 'no FROST DKG'}`);
        if (this.frostDkg) {
          try {
            const canStart = this.frostDkg.can_start_round2();
            this._log(`   - WASM can_start_round2: ${canStart}`);
          } catch (e) {
            this._log(`   - WASM can_start_round2: ERROR - ${this._getErrorMessage(e)}`);
          }
        }
      }

      return true;
    } catch (error) {
      this._log(`Error initializing DKG: ${this._getErrorMessage(error)}`);
      this._resetDkgState();
      this._updateDkgState(DkgState.Failed);
      return false;
    }
  }

  // Generate and broadcast Round 1 packages
  private async _generateAndBroadcastRound1(): Promise<void> {
    this._log(`Generating and broadcasting Round 1 packages`);

    if (!this.frostDkg) {
      this._log(`Cannot generate Round 1 packages: DKG not initialized`);
      return;
    }

    try {
      // Update state to Round1InProgress before generating packages
      this._updateDkgState(DkgState.Round1InProgress);

      // Generate Round 1 package using FROST DKG
      const round1Package = this.frostDkg.generate_round1();
      this._log(`Generated Round 1 package (hex): ${round1Package.substring(0, 20)}...`);

      // Decode hex to get JSON string, then parse it as structured object (wire-compatible with the TUI node).
      let packageObject;
      try {
        // First decode the hex string to get the JSON string
        const hexMatches = round1Package.match(/.{1,2}/g);
        if (!hexMatches) {
          throw new Error('Invalid hex string format');
        }

        const packageBytes = new Uint8Array(hexMatches.map((byte: string) => parseInt(byte, 16)));
        const packageJson = new TextDecoder().decode(packageBytes);
        this._log(`Decoded Round 1 package JSON: ${packageJson.substring(0, 100)}...`);

        // Then parse the JSON string to get the structured object
        packageObject = JSON.parse(packageJson);
      } catch (error) {
        throw new Error(`Failed to decode/parse Round 1 package: ${error}`);
      }

      // Broadcast to all participants
      if (this.sessionInfo) {
        this.sessionInfo.participants.forEach(peerId => {
          if (peerId !== this.localPeerId) {
            const message: WebRTCAppMessage = {
              webrtc_msg_type: 'DkgRound1Package' as const,
              package: packageObject  // Send parsed JSON object — TUI peers expect the object shape, not a hex string.
            };
            this.sendWebRTCAppMessage(peerId, message);
          }
        });
      }

      // Process our own package locally - but only add to received set, don't add to FROST DKG
      // (FROST DKG already includes our own package when generate_round1() was called)
      this.receivedRound1Packages.add(this.localPeerId);
      this._log(`Added own Round 1 package to received set. Total: ${this.receivedRound1Packages.size}`);

    } catch (error) {
      this._log(`Error generating/broadcasting Round 1 package: ${this._getErrorMessage(error)}`);
      this._updateDkgState(DkgState.Failed);
    }
  }

  // Add the initiateSigning method that tests are expecting
  public initiateSigning(signingId: string, content: any, threshold: number): void {
    this._log(`Initiating signing process: ${signingId}`);

    if (!this.sessionInfo) {
      this._log(`Cannot initiate signing: no session info`);
      return;
    }

    if (this.signingState !== SigningState.Idle) {
      this._log(`Cannot initiate signing: signing already in progress (state: ${this.signingState})`);
      return;
    }

    // Create signing info
    this.signingInfo = {
      signing_id: signingId,
      transaction_data: typeof content === 'string' ? content : JSON.stringify(content),
      threshold: threshold,
      participants: this.sessionInfo.participants.slice(),
      acceptances: new Map<string, boolean>(), // Initialize empty acceptances map
      accepted_participants: [this.localPeerId], // Initiator auto-accepts
      selected_signers: [],
      step: "pending_acceptance",
      initiator: this.localPeerId
    };

    this._updateSigningState(SigningState.AwaitingAcceptances, this.signingInfo);

    // Broadcast signing request to all participants
    const message: WebRTCAppMessage = {
      webrtc_msg_type: 'SigningRequest' as const,
      signing_id: signingId,
      transaction_data: typeof content === 'string' ? content : JSON.stringify(content),
      required_signers: threshold
    };

    this.sessionInfo.participants.forEach(peerId => {
      if (peerId !== this.localPeerId) {
        this.sendWebRTCAppMessage(peerId, message);
      }
    });

    this._log(`Signing request broadcast to ${this.sessionInfo.participants.length - 1} peers`);
  }

  public handleWebRTCAppMessage(fromPeerId: string, message: WebRTCAppMessage): void {
    this._log(`Handling WebRTC app message from ${fromPeerId}: ${JSON.stringify(message)}`);

    // Call the existing onWebRTCAppMessage callback
    this.onWebRTCAppMessage(fromPeerId, message);

    // Process specific message types using webrtc_msg_type format
    if ((message as any).webrtc_msg_type) {
      switch ((message as any).webrtc_msg_type) {
        case 'SimpleMessage':
          this._log(`Received SimpleMessage from ${fromPeerId}: ${(message as any).text}`);
          break;
        case 'MeshReady':
          this._log(`Received MeshReady message from ${fromPeerId}`);
          this._processPeerMeshReady(fromPeerId);
          break;
        case 'DkgRound1Package':
          if ((message as any).package) {
            this._handleDkgRound1Package(fromPeerId, (message as any).package);
          }
          break;
        case 'DkgRound2Package':
          if ((message as any).package) {
            this._handleDkgRound2Package(fromPeerId, (message as any).package);
          }
          break;
        case 'SigningRequest':
          this._handleSigningRequest(fromPeerId, message as any);
          break;
        case 'SigningAcceptance':
          this._handleSigningAcceptance(fromPeerId, message as any);
          break;
        case 'SignerSelection':
          this._handleSignerSelection(fromPeerId, message as any);
          break;
        case 'SigningCommitment':
          this._handleSigningCommitment(fromPeerId, message as any);
          break;
        case 'SignatureShare':
          this._handleSignatureShare(fromPeerId, message as any);
          break;
        case 'AggregatedSignature':
          this._handleAggregatedSignature(fromPeerId, message as any);
          break;
        default:
          this._log(`Unhandled WebRTC app message type from ${fromPeerId}: ${JSON.stringify(message)}`);
          break;
      }
    } else {
      this._log(`Unhandled WebRTC app message type from ${fromPeerId}: ${JSON.stringify(message)}`);
    }
  }

  private _tryAggregateSignature(): void {
    this._log(`Attempting to aggregate signature`);

    if (!this.signingInfo) {
      this._log(`Cannot aggregate signature: no signing info`);
      return;
    }

    // Check if we have all required signature shares
    const allSharesReceived = this.signingInfo.selected_signers.every(signer =>
      this.signingShares.has(signer)
    );

    if (!allSharesReceived) {
      this._log(`Cannot aggregate signature: missing signature shares`);
      return;
    }

    // If we are the initiator, aggregate the signature
    if (this.signingInfo.initiator === this.localPeerId) {
      this._aggregateSignatureAndBroadcast();
    } else {
      this._log(`Not the initiator, waiting for aggregated signature from ${this.signingInfo.initiator}`);
    }
  }

  private _selectSignersAndProceed(): void {
    this._log(`Selecting signers and proceeding with signing process`);

    if (!this.signingInfo) {
      this._log(`Cannot select signers: no signing info`);
      return;
    }

    // Simple signer selection - use the first 'threshold' number of accepted participants
    const availableSigners = this.signingInfo.accepted_participants.slice(0, this.signingInfo.threshold);
    this.signingInfo.selected_signers = availableSigners;
    this.signingInfo.step = "signer_selection";

    // Broadcast signer selection to all participants
    const message: WebRTCAppMessage = {
      webrtc_msg_type: 'SignerSelection' as const,
      signing_id: this.signingInfo.signing_id,
      selected_signers: this.signingInfo.selected_signers
    };

    if (this.sessionInfo) {
      this.sessionInfo.participants.forEach(peerId => {
        if (peerId !== this.localPeerId) {
          this.sendWebRTCAppMessage(peerId, message);
        }
      });
    }

    this._log(`Selected signers: [${this.signingInfo.selected_signers.join(', ')}]`);

    // Check if we are selected as a signer
    const isSelectedSigner = this.signingInfo.selected_signers.includes(this.localPeerId);

    if (isSelectedSigner) {
      this._log(`We are selected as a signer. Transitioning to CommitmentPhase.`);
      this._updateSigningState(SigningState.CommitmentPhase, this.signingInfo);
      this._generateAndSendCommitment();
    } else {
      this._log(`We are not selected as a signer. Monitoring signing process.`);
      this._updateSigningState(SigningState.CommitmentPhase, this.signingInfo);
    }
  }

  private _handleSigningTimeout(): void {
    this._log(`Handling signing timeout`);

    if (this.signingInfo) {
      this._log(`Signing process ${this.signingInfo.signing_id} timed out`);
    }

    // Reset signing state to idle
    this._resetSigningState();
  }

  // Add all the missing private methods that tests are calling
  private _resetSigningState(): void {
    this._log(`Resetting signing state`);
    this.signingState = SigningState.Idle;
    this.signingInfo = null;
    this.signingCommitments.clear();
    this.signingShares.clear();
    this.onSigningStateUpdate(this.signingState, this.signingInfo);
  }

  private _processPeerMeshReady(fromPeerId: string): void {
    this._log(`Processing mesh ready signal from ${fromPeerId}`);

    if (!this.sessionInfo) {
      this._log(`Cannot process MeshReady from ${fromPeerId}: no active session`);
      return;
    }

    // Update mesh status to include this peer as ready
    const currentStatus = this.meshStatus;
    let readyPeers = new Set<string>();

    if (currentStatus.type === MeshStatusType.PartiallyReady && (currentStatus as any).ready_devices) {
      // Copy existing ready_devices from PartiallyReady state
      readyPeers = new Set((currentStatus as any).ready_devices);
    } else if (currentStatus.type === MeshStatusType.Ready) {
      // If already Ready, all participants are ready
      readyPeers = new Set(this.sessionInfo.participants);
    } else {
      // Initialize with local peer for Incomplete state
      readyPeers = new Set([this.localPeerId]);
    }

    // Add the new ready peer
    readyPeers.add(fromPeerId);

    this._log(`Peer ${fromPeerId} is now mesh ready. Ready peers: [${Array.from(readyPeers).join(', ')}]`);

    // Check if all participants are now ready
    const allParticipantsReady = this.sessionInfo.participants.every(peerId =>
      readyPeers.has(peerId)
    );

    if (allParticipantsReady) {
      this._log("All participants are mesh ready! Transitioning to fully Ready state.");
      this._updateMeshStatus({
        type: MeshStatusType.Ready
      });
    } else {
      this._log(`Not all participants ready yet. Ready: ${readyPeers.size}/${this.sessionInfo.participants.length}`);
      this._updateMeshStatus({
        type: MeshStatusType.PartiallyReady,
        ready_devices: readyPeers,
        total_devices: this.sessionInfo.participants.length
      });
    }
  }

  private _checkMeshStatus(): void {
    if (!this.sessionInfo) return;

    const totalPeers = this.sessionInfo.participants.length;
    const connectedPeers = Array.from(this.dataChannels.keys()).filter(peerId => {
      const dc = this.dataChannels.get(peerId);
      return dc && dc.readyState === 'open';
    }).length + 1; // +1 for self

    // Check if all session participants have accepted
    const allParticipantsAccepted = this.sessionInfo.participants.every(peerId =>
      this.sessionInfo!.accepted_devices.includes(peerId)
    );

    // Enhanced logging to trace mesh status determination
    this._log(`=== MESH STATUS CHECK ===`);
    this._log(`Session ID: ${this.sessionInfo.session_id || 'unknown'}`);
    this._log(`Local Peer ID: ${this.localPeerId}`);
    this._log(`Expected peers: [${this.sessionInfo.participants.join(', ')}]`);
    this._log(`Accepted devices: [${this.sessionInfo.accepted_devices.join(', ')}]`);
    this._log(`All participants accepted: ${allParticipantsAccepted}`);
    this._log(`Open data channels: ${connectedPeers - 1}/${totalPeers - 1}`);
    this._log(`Own mesh ready sent: ${this.ownMeshReadySent}`);
    this._log(`Current mesh status: ${this.meshStatus.type}`);

    // Only send mesh ready if BOTH conditions are met:
    // 1. All data channels are open
    // 2. All session participants have accepted
    if (connectedPeers >= totalPeers && allParticipantsAccepted) {
      this._log("All data channels open AND all participants accepted - sending MeshReady signals if not sent yet");

      // Broadcast MeshReady to all peers if not done yet
      if (!this.ownMeshReadySent) {
        this._sendMeshReadyToAllPeers();
        // Flag is set in _sendMeshReadyToAllPeers
      }

      this._updateMeshStatus({ type: MeshStatusType.Ready });
    } else {
      const reason = [];
      if (connectedPeers < totalPeers) {
        reason.push(`data channels not ready (${connectedPeers}/${totalPeers})`);
      }
      if (!allParticipantsAccepted) {
        reason.push(`not all participants accepted (${this.sessionInfo.accepted_devices.length}/${this.sessionInfo.participants.length})`);
      }
      this._log(`Not ready for mesh ready signals: ${reason.join(', ')}`);

      this._updateMeshStatus({
        type: MeshStatusType.PartiallyReady,
        ready_devices: new Set([this.localPeerId, ...Array.from(this.dataChannels.keys()).filter(peerId => {
          const dc = this.dataChannels.get(peerId);
          return dc && dc.readyState === 'open';
        })]),
        total_devices: totalPeers
      });
    }
  }

  private async _handleDkgRound1Package(fromPeerId: string, packageData: any): Promise<void> {
    this._log(`Handling DKG Round 1 package from ${fromPeerId}`);

    // Skip processing our own Round 1 package. Our secret side is
    // held by the WASM instance from generate_round1(); frost-core's
    // dkg::part2 then expects exactly n-1 peer packages to ingest,
    // NOT our own — if we added self here, can_start_round2 (which
    // checks len == total - 1) would flip false and block progress.
    if (fromPeerId === this.localPeerId) {
      this._log(`Skipping own Round 1 package (dkg::part2 takes n-1 peer packages only)`);
      // Still record our own in the JS-side set so UI progress
      // indicators show the right count.
      if (this.dkgState !== DkgState.Idle) {
        this.receivedRound1Packages.add(fromPeerId);
        this._log(`Added own package to received set. Total: ${this.receivedRound1Packages.size}`);
      }
      return;
    }

    if (this.dkgState === DkgState.Idle) {
      // Buffer the package if DKG hasn't started yet
      this.bufferedRound1Packages.push({ fromPeerId, packageData });
      this._log(`Buffered Round 1 package from ${fromPeerId} (DKG not started)`);

      // Enhanced auto-trigger logic: Start DKG if we have session info and any of:
      // 1. Mesh is fully Ready, OR
      // 2. Mesh is PartiallyReady and all session participants have accepted, OR
      // 3. We have buffered packages from every expected remote peer (TUI or extension).
      const shouldAutoTrigger = this._shouldAutoTriggerDkg();

      this._log(`Auto-trigger evaluation: meshType=${MeshStatusType[this.meshStatus.type]}, sessionInfo=${!!this.sessionInfo}, shouldTrigger=${shouldAutoTrigger}`);

      if (shouldAutoTrigger) {
        this._log(`🚀 Auto-triggering DKG since conditions are met and Round 1 package received`);
        this._log(`🚀 Auto-trigger state: bufferedRound1Packages.length=${this.bufferedRound1Packages.length}`);
        this._log(`🚀 Auto-trigger state: buffered packages from: [${this.bufferedRound1Packages.map(p => p.fromPeerId).join(', ')}]`);
        const blockchain = this.currentBlockchain || "solana"; // Default to solana
        this._log(`Using blockchain: ${blockchain}`);
        await this.initializeDkg(blockchain);
      } else {
        this._log(`❌ Auto-trigger conditions not met - will wait for more conditions or manual trigger`);
      }
      return;
    }

    if (this.dkgState === DkgState.Initializing) {
      // Buffer the package if DKG is currently initializing
      this.bufferedRound1Packages.push({ fromPeerId, packageData });
      this._log(`Buffered Round 1 package from ${fromPeerId} (DKG initializing)`);
      return;
    }

    // Check if we have proper FROST DKG initialized
    if (!this.frostDkg) {
      this._log(`Cannot process Round 1 package: DKG not initialized`);
      return;
    }

    try {
      // Process the Round 1 package with FROST DKG
      const senderIndex = (this.sessionInfo?.participants.indexOf(fromPeerId) ?? -1) + 1;
      let packageHex: string;

      this._log(`🔍 PRE-PROCESS: fromPeerId=${fromPeerId}, senderIndex=${senderIndex}`);
      this._log(`🔍 PRE-PROCESS: participants=${JSON.stringify(this.sessionInfo?.participants)}`);
      this._log(`🔍 PRE-PROCESS: packageData type=${typeof packageData}`);

      // Handle both legacy format (with data property) and new CLI-compatible format (structured object)
      if (typeof packageData === 'string') {
        packageHex = packageData;
        this._log(`🔍 PRE-PROCESS: Using string packageData`);
      } else if (packageData.data) {
        // Legacy format: { sender_index: X, data: "hex_string" }
        packageHex = packageData.data;
        this._log(`🔍 PRE-PROCESS: Using legacy format with data property`);
      } else {
        // New CLI-compatible format: structured object
        // Convert JSON object to proper hex encoding for WASM
        const packageString = JSON.stringify(packageData);
        const packageBytes = new TextEncoder().encode(packageString);
        packageHex = Array.from(packageBytes).map(b => b.toString(16).padStart(2, '0')).join('');
        this._log(`🔍 PRE-PROCESS: Using CLI-compatible format, converted JSON to hex (${packageHex.length} chars)`);
      }

      if (!senderIndex) {
        throw new Error(`Could not determine sender index for ${fromPeerId}`);
      }

      if (!packageHex) {
        throw new Error(`No package data from ${fromPeerId}`);
      }

      this._log(`🔍 PRE-PROCESS: About to call frostDkg.add_round1_package(${senderIndex}, packageHex.length=${packageHex.length})`);

      // Comprehensive validation before WASM call
      if (!this.frostDkg) {
        throw new Error(`FROST DKG instance is null/undefined - cannot add round1 package`);
      }

      if (typeof this.frostDkg.add_round1_package !== 'function') {
        throw new Error(`FROST DKG add_round1_package method is not available. Type: ${typeof this.frostDkg.add_round1_package}`);
      }

      this._log(`🔍 VALIDATION: FROST DKG instance exists and has add_round1_package method`);
      this._log(`🔍 VALIDATION: DKG state: ${this.dkgState}, senderIndex: ${senderIndex}, packageHex type: ${typeof packageHex}`);

      // Additional validation - check FROST DKG internal state
      try {
        // Check if FROST DKG has been properly initialized
        const canStartRound2 = this.frostDkg.can_start_round2 && this.frostDkg.can_start_round2();
        this._log(`🔍 VALIDATION: FROST DKG can_start_round2: ${canStartRound2}`);
      } catch (stateError) {
        this._log(`🚨 VALIDATION WARNING: Could not check FROST DKG state: ${this._getErrorMessage(stateError)}`);
      }

      // Add the Round 1 package to FROST DKG with specific error handling
      try {
        this.frostDkg.add_round1_package(senderIndex, packageHex);
        this._log(`🔍 POST-PROCESS: Successfully added Round 1 package to FROST DKG`);
      } catch (wasmError: any) {
        this._log(`🚨 WASM ERROR in add_round1_package: ${this._getErrorMessage(wasmError)}`);
        this._log(`🔍 WASM Error details: ${JSON.stringify(wasmError)}`);
        this._log(`🔍 WASM Error name: ${wasmError?.name || 'unknown'}`);
        this._log(`🔍 WASM Error message: ${wasmError?.message || 'unknown'}`);
        this._log(`🔍 WASM Error stack: ${wasmError?.stack || 'unknown'}`);
        this._log(`🔍 WASM senderIndex: ${senderIndex} (type: ${typeof senderIndex})`);
        this._log(`🔍 WASM packageHex length: ${packageHex.length} (type: ${typeof packageHex})`);
        this._log(`🔍 WASM packageHex preview: ${packageHex.substring(0, 100)}...`);
        this._log(`🔍 WASM FROST DKG type: ${this.frostDkg.constructor?.name || 'unknown'}`);
        
        // Try to get more info about the FROST DKG state
        try {
          this._log(`🔍 WASM FROST DKG toString: ${this.frostDkg.toString()}`);
        } catch (toStringError) {
          this._log(`🔍 WASM Could not get FROST DKG toString: ${this._getErrorMessage(toStringError)}`);
        }
        
        throw wasmError; // Re-throw to be caught by outer catch
      }

      // Add to received packages set
      this.receivedRound1Packages.add(fromPeerId);
      this._log(`Processed Round 1 package from ${fromPeerId}. Total: ${this.receivedRound1Packages.size}`);
      this._log(`🔍 ROUND1→ROUND2 CHECK: receivedRound1Packages.size=${this.receivedRound1Packages.size}, expected=${this.sessionInfo?.participants.length || 0}`);

      // Check if we have all packages needed and can proceed to Round 2
      if (this.sessionInfo) {
        const hasAllPackages = this.receivedRound1Packages.size >= this.sessionInfo.participants.length;
        let canStartRound2 = false;
        
        try {
          canStartRound2 = this.frostDkg.can_start_round2();
          this._log(`🔍 ROUND1→ROUND2 CHECK: WASM can_start_round2=${canStartRound2}`);
        } catch (e) {
          this._log(`🔍 ROUND1→ROUND2 CHECK: Error checking can_start_round2: ${this._getErrorMessage(e)}`);
        }
        
        this._log(`🔍 ROUND1→ROUND2 CHECK: hasAllPackages=${hasAllPackages}, canStartRound2=${canStartRound2}`);
        
        if (hasAllPackages && canStartRound2) {
          this._log(`✅ All Round 1 packages received and can proceed. Moving to Round 2.`);
          this._updateDkgState(DkgState.Round2InProgress);
          await this._generateAndBroadcastRound2();

          // Process any buffered Round 2 packages that arrived while we were in Round 1
          await this._replayBufferedDkgPackages();
        } else {
          this._log(`❌ Cannot proceed to Round 2: hasAllPackages=${hasAllPackages}, canStartRound2=${canStartRound2}`);
        }
      }
    } catch (error) {
        // Enhanced error logging for debugging DKG failures - with error protection
        try {
          const errorMessage = this._getErrorMessage(error);
          this._log(`🚨 ERROR processing Round 1 package from ${fromPeerId}: ${errorMessage}`);
          
          try {
            this._log(`🔍 Error details: ${JSON.stringify(error)}`);
          } catch (e) {
            this._log(`🔍 Error details: [Cannot stringify error]`);
          }
          
          this._log(`🔍 Package data type: ${typeof packageData}`);
          
          try {
            this._log(`🔍 Package data: ${JSON.stringify(packageData)}`);
          } catch (e) {
            this._log(`🔍 Package data: [Cannot stringify package data]`);
          }
          
          try {
            this._log(`🔍 Session info: ${JSON.stringify(this.sessionInfo)}`);
          } catch (e) {
            this._log(`🔍 Session info: [Cannot stringify session info]`);
          }
          
          this._log(`🔍 DKG state before error: ${DkgState[this.dkgState]}`);
          
          // Additional debugging info
          this._log(`🔍 fromPeerId: ${fromPeerId}`);
          this._log(`🔍 localPeerId: ${this.localPeerId}`);
          this._log(`🔍 receivedRound1Packages.size: ${this.receivedRound1Packages.size}`);
          this._log(`🔍 sessionInfo?.participants: ${this.sessionInfo?.participants || 'null'}`);
          this._log(`🔍 frostDkg exists: ${!!this.frostDkg}`);
          
        } catch (loggingError) {
          // Fallback if logging itself fails
          console.error('Failed to log DKG error:', loggingError);
          console.error('Original DKG error:', error);
        }

        // Use verbose logging for expected DKG failures in test environment
        this._logVerbose(`Error processing Round 1 package from ${fromPeerId}: ${this._getErrorMessage(error)}`);
        this._updateDkgState(DkgState.Failed);
      }
  }

  private async _handleDkgRound2Package(fromPeerId: string, packageData: any): Promise<void> {
    this._log(`Handling DKG Round 2 package from ${fromPeerId}`);

    if (this.dkgState === DkgState.Idle || this.dkgState === DkgState.Initializing || this.dkgState === DkgState.Round1InProgress) {
      // Buffer the package if DKG hasn't started Round 2 yet
      this.bufferedRound2Packages.push({ fromPeerId, packageData });
      this._log(`Buffered Round 2 package from ${fromPeerId} (DKG not in Round 2)`);
      return;
    }

    // Check if we have proper FROST DKG initialized
    if (!this.frostDkg) {
      this._log(`Cannot process Round 2 package: DKG not initialized`);
      return;
    }

    try {
      // Process the Round 2 package with FROST DKG
      const senderIndex = (this.sessionInfo?.participants.indexOf(fromPeerId) ?? -1) + 1;
      let packageHex: string;

      this._log(`🔍 R2 PRE-PROCESS: fromPeerId=${fromPeerId}, senderIndex=${senderIndex}`);
      this._log(`🔍 R2 PRE-PROCESS: participants=${JSON.stringify(this.sessionInfo?.participants)}`);
      this._log(`🔍 R2 PRE-PROCESS: packageData type=${typeof packageData}`);

      // Handle both legacy format (with data property) and new CLI-compatible format (structured object)
      if (typeof packageData === 'string') {
        packageHex = packageData;
        this._log(`🔍 R2 PRE-PROCESS: Using string packageData`);
      } else if (packageData.data) {
        // Legacy format: { sender_index: X, sender_id_hex: Y, data: "hex_string" }
        packageHex = packageData.data;
        this._log(`🔍 R2 PRE-PROCESS: Using legacy format with data property`);
      } else {
        // New CLI-compatible format: structured object
        // Convert JSON object to proper hex encoding for WASM
        const packageString = JSON.stringify(packageData);
        const packageBytes = new TextEncoder().encode(packageString);
        packageHex = Array.from(packageBytes).map(b => b.toString(16).padStart(2, '0')).join('');
        this._log(`🔍 R2 PRE-PROCESS: Using CLI-compatible format, converted JSON to hex (${packageHex.length} chars)`);
      }

      if (!senderIndex) {
        throw new Error(`Could not determine sender index for ${fromPeerId}`);
      }

      if (!packageHex) {
        throw new Error(`No package data from ${fromPeerId}`);
      }

      // Convert to hex if it's not already
      let finalPackageHex: string;
      if (!packageHex.match(/^[0-9a-fA-F]+$/)) {
        // Convert JSON string to hex
        finalPackageHex = Array.from(new TextEncoder().encode(packageHex))
          .map((byte: number) => byte.toString(16).padStart(2, '0'))
          .join('');
      } else {
        finalPackageHex = packageHex;
      }

      // Add the Round 2 package to FROST DKG
      this._log(`🔍 R2 PRE-PROCESS: About to call frostDkg.add_round2_package(${senderIndex}, packageHex.length=${finalPackageHex.length})`);
      try {
        this.frostDkg.add_round2_package(senderIndex, finalPackageHex);
        this._log(`🔍 R2 POST-PROCESS: Successfully added Round 2 package to FROST DKG`);
      } catch (wasmError: any) {
        this._log(`🚨 WASM ERROR in add_round2_package: ${this._getErrorMessage(wasmError)}`);
        this._log(`🔍 R2 WASM Error details: ${JSON.stringify(wasmError)}`);
        this._log(`🔍 R2 WASM senderIndex: ${senderIndex}`);
        this._log(`🔍 R2 WASM packageHex length: ${finalPackageHex.length}`);
        this._log(`🔍 R2 WASM packageHex preview: ${finalPackageHex.substring(0, 100)}...`);
        throw wasmError; // Re-throw to be caught by outer catch
      }

      // Add to received packages set
      this.receivedRound2Packages.add(fromPeerId);
      this._log(`Processed Round 2 package from ${fromPeerId}. Total: ${this.receivedRound2Packages.size}`);

      // Debug: Check if FROST DKG can finalize after adding this package
      const canFinalize = this.frostDkg.can_finalize();
      this._log(`🔍 R2 DEBUG: After adding package from ${fromPeerId}, can_finalize()=${canFinalize}, receivedRound2Packages.size=${this.receivedRound2Packages.size}, expected=${this.sessionInfo?.participants.length}`);

      // Check if we have all packages needed
      if (this.sessionInfo &&
        this.receivedRound2Packages.size >= this.sessionInfo.participants.length &&
        this.frostDkg.can_finalize()) {
        this._log(`All Round 2 packages received and can finalize. Finalizing DKG.`);
        await this._finalizeDkg();
      }
    } catch (error) {
      // Use verbose logging for expected DKG failures in test environment
      this._logVerbose(`Error processing Round 2 package from ${fromPeerId}: ${this._getErrorMessage(error)}`);
      this._updateDkgState(DkgState.Failed);
    }
  }

  private async _generateAndBroadcastRound2(): Promise<void> {
    this._log(`Generating and broadcasting Round 2 packages`);
    // Ensure we have a FROST DKG instance
    if (!this.frostDkg) {
      this._log(`Cannot generate Round 2 packages: DKG not initialized`);
      return;
    }

    try {
      // Generate Round 2 package map using FROST DKG
      const round2PackageMapHex = this.frostDkg.generate_round2();
      this._log(`Generated Round 2 package map: ${round2PackageMapHex.substring(0, 50)}...`);

      // Decode and parse the package map
      const packageMapBytes = new Uint8Array(round2PackageMapHex.match(/.{1,2}/g)!.map((byte: string) => parseInt(byte, 16)));
      const packageMapJson = new TextDecoder().decode(packageMapBytes);
      const packageMap = JSON.parse(packageMapJson);

      this._log(`Package map contains ${Object.keys(packageMap).length} packages`);

      // Send individual packages to each participant
      if (this.sessionInfo) {
        let sentCount = 0;
        this.sessionInfo.participants.forEach(peerId => {
          if (peerId !== this.localPeerId && this.sessionInfo) {
            const peerIndex = this.sessionInfo.participants.indexOf(peerId) + 1;
            const peerIndexStr = peerIndex.toString();

            // Extract the specific package for this peer from the map
            if (packageMap[peerIndexStr]) {
              const individualPackage = packageMap[peerIndexStr];

              // Send the individual package (not the entire map)
              const message: WebRTCAppMessage = {
                webrtc_msg_type: 'DkgRound2Package' as const,
                package: individualPackage
              };

              this.sendWebRTCAppMessage(peerId, message);
              sentCount++;
              this._log(`Sent Round 2 package to ${peerId} (index ${peerIndex})`);
            } else {
              this._log(`Warning: No Round 2 package found for peer ${peerId} (index ${peerIndex})`);
            }
          }
        });

        this._log(`Successfully sent ${sentCount} Round 2 packages`);
      }

      // Add our own package to received packages (we don't send to ourselves)
      this.receivedRound2Packages.add(this.localPeerId);
    } catch (error) {
      this._log(`Error generating Round 2 packages: ${this._getErrorMessage(error)}`);
      this._updateDkgState(DkgState.Failed);
    }
  }

  private async _finalizeDkg(): Promise<void> {
    this._log(`Finalizing DKG process`);

    if (!this.frostDkg) {
      this._log(`Cannot finalize DKG: DKG not initialized`);
      this._updateDkgState(DkgState.Failed);
      return;
    }

    try {
      // Check if we have all Round 2 packages needed
      if (this.sessionInfo && this.receivedRound2Packages.size < this.sessionInfo.participants.length) {
        this._log(`Cannot finalize DKG: missing Round 2 packages`);
        this._updateDkgState(DkgState.Failed);
        return;
      }

      // Check if FROST DKG can finalize
      if (!this.frostDkg.can_finalize()) {
        this._log(`Cannot finalize DKG: FROST DKG not ready to finalize`);
        this._updateDkgState(DkgState.Failed);
        return;
      }

      // Finalize DKG and get group public key
      this.groupPublicKey = this.frostDkg.finalize_dkg();

      // Generate blockchain addresses using proper WASM methods
      if (this.groupPublicKey) {
        if (this.currentBlockchain === 'ethereum') {
          // For Ethereum, use the secp256k1 WASM method
          this.ethereumAddress = (this.frostDkg as any).get_eth_address();
          this.walletAddress = this.ethereumAddress;
        } else {
          // For Solana, use the Ed25519 WASM method for proper Base58 encoding
          this.solanaAddress = (this.frostDkg as any).get_address();
          this.walletAddress = this.solanaAddress;
        }
      }

      this._updateDkgState(DkgState.Complete);
      this._log(`DKG completed successfully. Group public key: ${this.groupPublicKey}`);

      // Ext-1d event propagation: surface the derived key material to
      // the offscreen outer layer (and on up to background + popup)
      // so the post-ceremony flow (display address, prompt password,
      // encrypt + save keyshare) has something to hook into. Fired
      // exactly once per ceremony — the DkgState.Failed branch
      // below doesn't emit this, and we don't re-fire on repeat
      // enters of Complete (dkgState transition already dedups).
      try {
        const address =
          this.currentBlockchain === "ethereum"
            ? this.ethereumAddress
            : this.solanaAddress;
        // Extract the WASM keystore JSON so background can encrypt +
        // persist without needing a return trip. Failure here is
        // non-fatal: the ceremony succeeded, we just can't save yet.
        // The popup will show the address + group key but cannot
        // offer "Save" — user would need to retry.
        let keystoreJson: string | null = null;
        try {
          if (this.frostDkg && typeof (this.frostDkg as any).export_keystore === "function") {
            keystoreJson = (this.frostDkg as any).export_keystore();
          }
        } catch (exportErr) {
          this._log(
            `export_keystore failed (non-fatal): ${this._getErrorMessage(exportErr)}`,
          );
        }
        this.onDkgComplete({
          groupPublicKey: this.groupPublicKey ?? "",
          address: address ?? null,
          blockchain: this.currentBlockchain,
          sessionId: this.sessionInfo?.session_id ?? null,
          threshold: this.sessionInfo?.threshold ?? 0,
          total: this.sessionInfo?.total ?? 0,
          participants: this.sessionInfo?.participants ?? [],
          participantIndex: this.participantIndex,
          keystoreJson,
        });
      } catch (cbErr) {
        // A broken onDkgComplete subscriber must not surface as a
        // FROST failure — we already have the group key, the
        // ceremony succeeded cryptographically. Log and move on.
        this._log(
          `onDkgComplete subscriber threw (non-fatal): ${this._getErrorMessage(cbErr)}`,
        );
      }
    } catch (error) {
      this._log(`Error finalizing DKG: ${this._getErrorMessage(error)}`);
      this._updateDkgState(DkgState.Failed);
    }
  }

  private _resetDkgState(): void {
    this._log(`Resetting DKG state`);
    // Note: Don't reset dkgState here - caller should manage state transitions
    this.frostDkg = null;
    this.participantIndex = null;
    this.receivedRound1Packages.clear();
    this.receivedRound2Packages.clear();
    this.groupPublicKey = null;
    this.solanaAddress = null;
    this.ethereumAddress = null;
    this.bufferedRound1Packages = [];
    this.bufferedRound2Packages = [];
  }

  // Add public resetDkgState method for tests
  public resetDkgState(): void {
    this._resetDkgState();
    this.dkgState = DkgState.Idle;
  }

  public setBlockchain(blockchain: "ethereum" | "solana") {
    this._log(`Setting blockchain to ${blockchain}`);
    this.currentBlockchain = blockchain;
  }

  public async checkAndTriggerDkg(blockchain: string): Promise<boolean> {
    // Set blockchain first to ensure correct curve is shown in logs
    this.currentBlockchain = blockchain as "ethereum" | "solana";

    this._log(`Checking conditions to trigger DKG for ${blockchain}`);

    if (!this.sessionInfo) {
      this._log(`Cannot trigger DKG: no session info`);
      return false;
    }

    if (this.dkgState !== DkgState.Idle) {
      this._log(`Cannot trigger DKG: already in progress (state: ${DkgState[this.dkgState]})`);
      return false;
    }

    if (this.meshStatus.type !== MeshStatusType.Ready) {
      this._log(`Cannot trigger DKG: mesh not ready (status: ${this.meshStatus.type})`);
      return false;
    }

    return await this.initializeDkg(blockchain as "ethereum" | "solana");
  }

  /**
   * Ext-2d-progress: snapshot + emit the signing roster state.
   * Called after every mutation of signingCommitments or
   * signingShares so the popup's progress roster stays live.
   * No-op when no signingInfo is set (idle ceremonies have nothing
   * to report). Converts private Maps to plain arrays of peer-ids
   * for wire transport.
   */
  private _emitSigningProgress(): void {
    if (!this.signingInfo) return;
    this.onSigningProgress({
      signingId: this.signingInfo.signing_id,
      state: this.signingState,
      selectedSigners: this.signingInfo.selected_signers.slice(),
      commitmentsReceived: Array.from(this.signingCommitments.keys()),
      sharesReceived: Array.from(this.signingShares.keys()),
    });
  }

  /**
   * Ext-2d-offscreen: load an existing wallet's key share into a
   * fresh FROST instance so it can be used for signing. Populates
   * `this.frostDkg` the same way `initializeDkg` would after finishing
   * a DKG ceremony. Called when a signing session is about to start
   * and we haven't already loaded a keystore (or we need a different
   * one than what's currently loaded).
   *
   * The keystore data shape must match what frost-core's import_keystore
   * expects — the background's `getActiveKeystore` response already
   * provides this, modulo field-name normalization (snake_case).
   *
   * Returns true on successful load. Throws on keystore parse failure
   * or WASM init failure; callers should treat those as fatal for
   * the ceremony and bail.
   */
  public async loadKeystoreForSigning(
    keyShareData: {
      key_package: string;
      group_public_key: string;
      session_id: string;
      device_id: string;
      participant_index: number;
      threshold: number;
      total_participants: number;
      curve?: "secp256k1" | "ed25519";
    },
    blockchain: "ethereum" | "solana",
  ): Promise<boolean> {
    this.currentBlockchain = blockchain;
    this.participantIndex = keyShareData.participant_index;
    this._log(
      `Loading keystore for signing: ${blockchain} / participant ${keyShareData.participant_index} / session ${keyShareData.session_id}`,
    );

    const { FrostDkgSecp256k1, FrostDkgEd25519 } = await import(
      "@mpc-wallet/core-wasm"
    );
    const instance =
      blockchain === "ethereum"
        ? new FrostDkgSecp256k1()
        : new FrostDkgEd25519();
    instance.import_keystore(
      JSON.stringify({
        key_package: keyShareData.key_package,
        group_public_key: keyShareData.group_public_key,
        session_id: keyShareData.session_id,
        device_id: keyShareData.device_id,
        participant_index: keyShareData.participant_index,
        threshold: keyShareData.threshold,
        total_participants: keyShareData.total_participants,
      }),
    );

    this.frostDkg = instance;
    this.groupPublicKey = keyShareData.group_public_key;
    if (blockchain === "ethereum") {
      try {
        this.ethereumAddress = (instance as any).get_eth_address?.() ?? null;
        this.walletAddress = this.ethereumAddress;
      } catch (e) {
        this._log(`Failed to derive ethereum address: ${e}`);
      }
    } else {
      try {
        this.solanaAddress = (instance as any).get_address?.() ?? null;
        this.walletAddress = this.solanaAddress;
      } catch (e) {
        this._log(`Failed to derive solana address: ${e}`);
      }
    }

    this._log(
      `Keystore loaded into WASM: address=${this.walletAddress ?? "(unknown)"}`,
    );
    return true;
  }

  /**
   * Ext-2d-offscreen: kick off a signing ceremony announced via the
   * session-discovery protocol (sessionReadyForSigning). This is the
   * real-FROST version; the older `_handleSigningRequest` + mock-
   * commitment path is a separate legacy flow that we're leaving
   * in place for now.
   *
   * Flow:
   *   1. Ensure `this.frostDkg` is loaded (caller must run
   *      loadKeystoreForSigning first).
   *   2. Set up signingInfo with selected_signers = the first
   *      `threshold` participants from session_info.participants
   *      (stable-sorted so every peer picks the same subset — a
   *      disagreement here would corrupt the ceremony).
   *   3. Call frostDkg.signing_commit() to generate our commitment
   *      for THIS round.
   *   4. Broadcast the commitment as a SigningCommitment WebRTC
   *      app message to every peer in selected_signers (excluding
   *      self).
   *   5. Record our own commitment locally so we can match on it
   *      when aggregating later.
   *
   * Rounds 2 (signature share) + aggregation land in follow-up
   * commits — `_handleSigningCommitment` still uses mocks.
   */
  public async initiateSigningCeremony(
    sessionInfo: SessionInfo,
    messageHex: string,
  ): Promise<boolean> {
    if (!this.frostDkg) {
      this._log(
        `Cannot start signing ceremony: no FROST instance. Call loadKeystoreForSigning first.`,
      );
      return false;
    }
    if (this.signingState !== SigningState.Idle) {
      this._log(
        `Cannot start signing ceremony: already in state ${this.signingState}`,
      );
      return false;
    }

    // Select the first `threshold` participants deterministically.
    // All peers must pick the same subset — using session_info's
    // array order (which is the canonical order from the server's
    // broadcast) ensures convergence without a separate selection
    // round-trip.
    const threshold = sessionInfo.threshold;
    const selectedSigners = sessionInfo.participants.slice(0, threshold);

    if (!selectedSigners.includes(this.localPeerId)) {
      this._log(
        `Not a selected signer for this ceremony (we're not in the first ${threshold} of ${sessionInfo.participants.length}) — idle until re-invited`,
      );
      return false;
    }

    const signingId = `sign_${sessionInfo.session_id}`;
    this.signingInfo = {
      signing_id: signingId,
      transaction_data: messageHex,
      threshold,
      participants: sessionInfo.participants.slice(),
      acceptances: new Map(),
      accepted_participants: selectedSigners.slice(),
      selected_signers: selectedSigners,
      step: "commitment_phase",
      initiator: sessionInfo.proposer_id,
    };

    this._log(
      `Initiating signing ceremony ${signingId}: ${threshold} of ${sessionInfo.participants.length}, signers=[${selectedSigners.join(", ")}]`,
    );

    // Generate our round-1 commitment. Returns hex string.
    let commitmentHex: string;
    try {
      commitmentHex = this.frostDkg.signing_commit();
    } catch (e) {
      this._log(`signing_commit() failed: ${e}`);
      this.signingState = SigningState.Failed;
      this.onSigningStateUpdate(this.signingState, this.signingInfo);
      return false;
    }

    // Record our own commitment so when it's time to aggregate,
    // we're counted alongside the peers'. Keyed by peer-id to match
    // how _handleSigningCommitment records incoming ones.
    this.signingCommitments.set(this.localPeerId, commitmentHex);

    // Broadcast to all other selected signers.
    const broadcast: WebRTCAppMessage = {
      webrtc_msg_type: "SigningCommitment" as const,
      signing_id: signingId,
      sender_identifier: this.localPeerId,
      commitment: commitmentHex,
    };
    for (const peerId of selectedSigners) {
      if (peerId !== this.localPeerId) {
        this.sendWebRTCAppMessage(peerId, broadcast);
      }
    }

    this.signingState = SigningState.CommitmentPhase;
    this.onSigningStateUpdate(this.signingState, this.signingInfo);
    this._emitSigningProgress();
    this._log(
      `Round 1: broadcast our commitment to ${selectedSigners.length - 1} co-signers`,
    );
    return true;
  }

  private async _getOrCreatePeerConnection(peerId: string): Promise<RTCPeerConnection | null> {
    let pc = this.peerConnections.get(peerId);
    if (!pc) {
      pc = new RTCPeerConnection({ iceServers: ICE_SERVERS });
      this.peerConnections.set(peerId, pc);
      this._setupPeerConnection(pc, peerId);
    }
    return pc;
  }

  private _setupPeerConnection(pc: RTCPeerConnection, peerId: string): void {
    pc.onicecandidate = (event) => {
      if (event.candidate && this.sendPayloadToBackgroundForRelay) {
        const payload = {
          websocket_msg_type: 'WebRTCSignal',
          Candidate: {
            candidate: event.candidate.candidate,
            sdpMid: event.candidate.sdpMid,
            sdpMLineIndex: event.candidate.sdpMLineIndex
          }
        };
        this.sendPayloadToBackgroundForRelay(peerId, payload as any);
      }
    };

    pc.ondatachannel = (event) => {
      this._setupDataChannel(event.channel, peerId);
    };

    pc.onconnectionstatechange = () => {
      const isConnected = pc.connectionState === 'connected';
      this.onWebRTCConnectionUpdate(peerId, isConnected);
      if (!isConnected && pc.connectionState === 'disconnected') {
        this._handlePeerDisconnection(peerId);
      }
    };
  }

  private _setupDataChannel(channel: RTCDataChannel, peerId: string): void {
    this.dataChannels.set(peerId, channel);

    channel.onopen = () => {
      this._log(`Data channel opened with ${peerId}`);
      this._checkMeshStatus();
    };

    channel.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        this.handleWebRTCAppMessage(peerId, message);
      } catch (error) {
        this._log(`Error parsing message from ${peerId}: ${this._getErrorMessage(error)}`);
      }
    };

    channel.onclose = () => {
      this._log(`Data channel closed with ${peerId}`);
      this.dataChannels.delete(peerId);
      this._checkMeshStatus();
    };
  }

  // Signing-related handler methods
  private _handleSigningRequest(fromPeerId: string, message: any): void {
    this._log(`Handling signing request from ${fromPeerId}`);

    if (this.signingState !== SigningState.Idle || this.signingInfo !== null) {
      this._log(`Ignoring signing request: already in signing process`);
      return;
    }

    // Initialize signing info for the request
    this.signingInfo = {
      signing_id: message.signing_id,
      transaction_data: message.transaction_data,
      threshold: message.threshold,
      participants: message.participants,
      acceptances: new Map<string, boolean>(),
      accepted_participants: [],
      selected_signers: [],
      step: "pending_acceptance",
      initiator: fromPeerId
    };

    // Auto-accept the signing request (in real implementation, this might require user confirmation)
    const response: WebRTCAppMessage = {
      webrtc_msg_type: 'SigningAcceptance' as const,
      signing_id: message.signing_id,
      accepted: true
    };

    this.sendWebRTCAppMessage(fromPeerId, response);
    this._log(`Accepted signing request ${message.signing_id} from ${fromPeerId}`);
  }

  private _handleSigningAcceptance(fromPeerId: string, message: any): void {
    this._log(`Handling signing acceptance from ${fromPeerId}: ${message.accepted}`);

    if (!this.signingInfo || this.signingInfo.signing_id !== message.signing_id) {
      this._log(`Ignoring signing acceptance: no matching signing process`);
      return;
    }

    // Record the acceptance in the map
    this.signingInfo.acceptances.set(fromPeerId, message.accepted);

    if (message.accepted && !this.signingInfo.accepted_participants.includes(fromPeerId)) {
      this.signingInfo.accepted_participants.push(fromPeerId);
      this._log(`${fromPeerId} accepted signing. Total acceptances: ${this.signingInfo.accepted_participants.length}`);

      // Check if we have enough acceptances to proceed
      if (this.signingInfo.accepted_participants.length >= this.signingInfo.threshold) {
        this._log(`Sufficient acceptances received. Proceeding with signer selection.`);
        this._selectSignersAndProceed();
      }
    }
  }

  private _handleSignerSelection(fromPeerId: string, message: any): void {
    this._log(`Handling signer selection from ${fromPeerId}`);

    if (!this.signingInfo || this.signingInfo.signing_id !== message.signing_id) {
      this._log(`Ignoring signer selection: no matching signing process`);
      return;
    }

    this.signingInfo.selected_signers = message.selected_signers;
    this.signingInfo.step = "commitment_phase";

    // Check if we are selected as a signer
    const isSelectedSigner = this.signingInfo.selected_signers.includes(this.localPeerId);

    if (isSelectedSigner) {
      this._log(`We are selected as a signer. Generating commitment.`);
      this._updateSigningState(SigningState.CommitmentPhase, this.signingInfo);
      this._generateAndSendCommitment();
    } else {
      this._log(`We are not selected as a signer. Monitoring signing process.`);
      this._updateSigningState(SigningState.CommitmentPhase, this.signingInfo);
    }
  }

  /**
   * Ext-2d-offscreen-rounds: real FROST round-1 commitment handler.
   * Replaces the pre-existing mock that just stored opaque blobs.
   *
   * Per FROST: signing_commit() auto-registers our OWN commitment
   * in the WASM instance's signing_commitments map (required by
   * frost-core's round2::sign — see round2.rs:135,
   * Error::MissingCommitment). Peers' commitments must be
   * explicitly registered via add_signing_commitment(i, hex)
   * before sign() can produce a share using them.
   *
   * Index convention: FROST participants are 1-indexed. Convert from
   * peer-id via `participants.indexOf(peerId) + 1`.
   */
  private _handleSigningCommitment(fromPeerId: string, message: any): void {
    this._log(`Handling signing commitment from ${fromPeerId}`);

    if (!this.signingInfo || this.signingInfo.signing_id !== message.signing_id) {
      this._log(`Ignoring signing commitment: no matching signing process`);
      return;
    }
    if (!this.frostDkg) {
      this._log(`Cannot process commitment: no FROST instance loaded`);
      return;
    }

    // Sorted order to match the Rust core's canonical_identifier — signingInfo
    // participants may come straight off the wire (unsorted). (#29)
    const senderIndex =
      [...this.signingInfo.participants].sort().indexOf(fromPeerId) + 1;
    if (senderIndex <= 0) {
      this._log(
        `Invalid sender index for ${fromPeerId} (not in participants list)`,
      );
      return;
    }

    const commitmentHex =
      typeof message.commitment === "string" ? message.commitment : null;
    if (!commitmentHex) {
      this._log(
        `Invalid commitment format from ${fromPeerId}: expected string, got ${typeof message.commitment}`,
      );
      return;
    }

    if (this.signingCommitments.has(fromPeerId)) {
      this._log(`Duplicate commitment from ${fromPeerId} — ignoring`);
      return;
    }

    try {
      this.frostDkg.add_signing_commitment(senderIndex, commitmentHex);
    } catch (e) {
      this._log(
        `add_signing_commitment(${senderIndex}, ...) failed: ${this._getErrorMessage(e)}`,
      );
      return;
    }

    this.signingCommitments.set(fromPeerId, commitmentHex);
    this._log(
      `Registered commitment from ${fromPeerId} (idx ${senderIndex}). Total: ${this.signingCommitments.size}/${this.signingInfo.selected_signers.length}`,
    );
    this._emitSigningProgress();

    // When we've collected commitments from all selected signers
    // (including our own, which was recorded in initiateSigningCeremony
    // or _generateAndSendCommitment), move to the share phase.
    if (
      this.signingCommitments.size >= this.signingInfo.selected_signers.length
    ) {
      this._log(`All commitments received. Transitioning to share phase.`);
      this._updateSigningState(SigningState.SharePhase, this.signingInfo);
      this._generateAndSendSignatureShare();
    }
  }

  /**
   * Ext-2d-offscreen-rounds: real FROST round-2 share handler.
   * Replaces the pre-existing mock that stored opaque blobs.
   *
   * Per FROST: sign() auto-registers our OWN share in the WASM
   * instance's signature_shares map (required by frost-core's
   * aggregate — the length-match check on signing_commitments vs
   * signature_shares in frost-core/src/lib.rs would fail otherwise).
   * Peers' shares must be added via add_signature_share(i, hex).
   */
  private _handleSignatureShare(fromPeerId: string, message: any): void {
    this._log(`Handling signature share from ${fromPeerId}`);

    if (!this.signingInfo || this.signingInfo.signing_id !== message.signing_id) {
      this._log(`Ignoring signature share: no matching signing process`);
      return;
    }
    if (!this.frostDkg) {
      this._log(`Cannot process share: no FROST instance loaded`);
      return;
    }

    // Sorted order to match the Rust core's canonical_identifier — signingInfo
    // participants may come straight off the wire (unsorted). (#29)
    const senderIndex =
      [...this.signingInfo.participants].sort().indexOf(fromPeerId) + 1;
    if (senderIndex <= 0) {
      this._log(
        `Invalid sender index for ${fromPeerId} (not in participants list)`,
      );
      return;
    }

    // Older mock protocol carried the payload under `signature_share`;
    // new real-FROST protocol under `share` (matches WASM naming).
    // Accept either so mid-flight upgrades don't break.
    const shareHex =
      (typeof message.share === "string" && message.share) ||
      (typeof message.signature_share === "string" &&
        message.signature_share) ||
      null;
    if (!shareHex) {
      this._log(
        `Invalid share format from ${fromPeerId}: expected string under 'share' or 'signature_share'`,
      );
      return;
    }

    if (this.signingShares.has(fromPeerId)) {
      this._log(`Duplicate share from ${fromPeerId} — ignoring`);
      return;
    }

    try {
      this.frostDkg.add_signature_share(senderIndex, shareHex);
    } catch (e) {
      this._log(
        `add_signature_share(${senderIndex}, ...) failed: ${this._getErrorMessage(e)}`,
      );
      return;
    }

    this.signingShares.set(fromPeerId, shareHex);
    this._log(
      `Registered share from ${fromPeerId} (idx ${senderIndex}). Total: ${this.signingShares.size}/${this.signingInfo.selected_signers.length}`,
    );
    this._emitSigningProgress();

    this._tryAggregateSignature();
  }

  private _handleAggregatedSignature(fromPeerId: string, message: any): void {
    this._log(`Handling aggregated signature from ${fromPeerId}`);

    if (!this.signingInfo || this.signingInfo.signing_id !== message.signing_id) {
      this._log(`Ignoring aggregated signature: no matching signing process`);
      return;
    }

    const signatureHex =
      typeof message.signature === "string" ? message.signature : null;
    if (!signatureHex) {
      this._log(
        `Invalid aggregated signature format: expected string, got ${typeof message.signature}`,
      );
      return;
    }

    this.signingInfo.final_signature = signatureHex;
    this.signingInfo.step = "complete";
    this._updateSigningState(SigningState.Complete, this.signingInfo);

    const sessionId = this.signingInfo.signing_id.replace(/^sign_/, "");
    this.onSigningComplete({
      signingId: this.signingInfo.signing_id,
      signature: signatureHex,
      messageHex: this.signingInfo.transaction_data,
      blockchain: this.currentBlockchain,
      sessionId,
    });

    this._log(
      `Signing process ${this.signingInfo.signing_id} completed (received aggregated sig from ${fromPeerId})`,
    );
  }

  private _generateAndSendCommitment(): void {
    this._log(`Generating and sending commitment`);

    if (!this.signingInfo) return;

    // Mock commitment generation
    const commitment = {
      data: `commitment-${this.localPeerId}-${Date.now()}`,
      participant: this.localPeerId
    };

    // Send commitment to all selected signers
    const message: WebRTCAppMessage = {
      webrtc_msg_type: 'SigningCommitment' as const,
      signing_id: this.signingInfo.signing_id,
      sender_identifier: this.localPeerId,
      commitment: commitment
    };

    this.signingInfo.selected_signers.forEach(peerId => {
      if (peerId !== this.localPeerId) {
        this.sendWebRTCAppMessage(peerId, message);
      }
    });

    // Add our own commitment
    this.signingCommitments.set(this.localPeerId, commitment);
  }

  /**
   * Ext-2d-offscreen-rounds: real FROST round-2 share generation.
   * Replaces the mock that emitted `share-${peerId}-${timestamp}`.
   *
   * `sign(messageHex)` uses our internally-stored nonce (from
   * signing_commit), our key package, and the already-registered
   * commitments from peers (add_signing_commitment calls) to produce
   * our FROST signature share. Returns hex. Nonces are one-time — a
   * second sign() call in the same ceremony would error or produce
   * an invalid share.
   */
  private _generateAndSendSignatureShare(): void {
    this._log(`Generating and sending signature share`);

    if (!this.signingInfo) return;
    if (!this.frostDkg) {
      this._log(`Cannot generate share: no FROST instance loaded`);
      this._updateSigningState(SigningState.Failed, this.signingInfo);
      return;
    }

    const messageHex = this.signingInfo.transaction_data;
    let shareHex: string;
    try {
      shareHex = this.frostDkg.sign(messageHex);
    } catch (e) {
      this._log(`sign() failed: ${this._getErrorMessage(e)}`);
      this._updateSigningState(SigningState.Failed, this.signingInfo);
      return;
    }

    const message: WebRTCAppMessage = {
      webrtc_msg_type: "SignatureShare" as const,
      signing_id: this.signingInfo.signing_id,
      sender_identifier: this.localPeerId,
      share: shareHex,
    };

    for (const peerId of this.signingInfo.selected_signers) {
      if (peerId !== this.localPeerId) {
        this.sendWebRTCAppMessage(peerId, message);
      }
    }

    // Record our own share so _tryAggregateSignature sees the full
    // set when it runs. Keyed by peer-id to match _handleSignatureShare.
    this.signingShares.set(this.localPeerId, shareHex);
    this._log(
      `Generated + broadcast signature share (hex length=${shareHex.length}) to ${this.signingInfo.selected_signers.length - 1} peers`,
    );
    this._emitSigningProgress();
  }

  /**
   * Ext-2d-offscreen-rounds: real FROST aggregation. Replaces the
   * mock `aggregated-sig-${timestamp}`.
   *
   * `aggregate_signature(messageHex)` uses all registered shares
   * (ours + peers' via add_signature_share) and the group public
   * key to produce the final threshold signature over messageHex.
   * Only the signer nominated as aggregator (initiator, which for
   * session-based flows maps to the first selected signer) calls
   * this — other signers receive the result via AggregatedSignature
   * broadcast.
   */
  private _aggregateSignatureAndBroadcast(): void {
    this._log(`Aggregating signature and broadcasting result`);

    if (!this.signingInfo) return;
    if (!this.frostDkg) {
      this._log(`Cannot aggregate: no FROST instance loaded`);
      this._updateSigningState(SigningState.Failed, this.signingInfo);
      return;
    }

    const messageHex = this.signingInfo.transaction_data;
    let aggregatedHex: string;
    try {
      aggregatedHex = this.frostDkg.aggregate_signature(messageHex);
    } catch (e) {
      this._log(
        `aggregate_signature() failed: ${this._getErrorMessage(e)}`,
      );
      this._updateSigningState(SigningState.Failed, this.signingInfo);
      return;
    }

    const message: WebRTCAppMessage = {
      webrtc_msg_type: "AggregatedSignature" as const,
      signing_id: this.signingInfo.signing_id,
      signature: aggregatedHex,
    };

    // Broadcast to ALL participants (not just selected_signers) so
    // non-signing keyholders also learn the ceremony completed and
    // what the resulting signature was. Lets them e.g. clear any
    // pending "waiting for signature" UI state.
    for (const peerId of this.signingInfo.participants) {
      if (peerId !== this.localPeerId) {
        this.sendWebRTCAppMessage(peerId, message);
      }
    }

    this.signingInfo.final_signature = aggregatedHex;
    this.signingInfo.step = "complete";
    this._updateSigningState(SigningState.Complete, this.signingInfo);

    const sessionId = this.signingInfo.signing_id.replace(/^sign_/, "");
    this.onSigningComplete({
      signingId: this.signingInfo.signing_id,
      signature: aggregatedHex,
      messageHex,
      blockchain: this.currentBlockchain,
      sessionId,
    });

    this._log(
      `Aggregation complete: broadcast final signature (hex length=${aggregatedHex.length}) to ${this.signingInfo.participants.length - 1} peers`,
    );
  }

  // Add getDkgStatus method that tests are expecting
  public getDkgStatus(): {
    state: DkgState;
    stateName?: string;
    blockchain?: string | null;
    participants?: string[];
    threshold?: number;
    groupPublicKey?: string | null;
    address?: string | null;
    participantIndex?: number | null;
    sessionInfo?: SessionInfo | null;
    receivedRound1Packages?: string[];
    receivedRound2Packages?: string[];
    frostDkgInitialized?: boolean;
  } {
    const stateName = DkgState[this.dkgState];

    return {
      state: this.dkgState,
      stateName,
      blockchain: this.currentBlockchain || null,
      participants: this.sessionInfo?.participants || [],
      threshold: (this.sessionInfo as any)?.threshold || 0,
      groupPublicKey: this.groupPublicKey,
      address: this.currentBlockchain === 'ethereum' ? this.ethereumAddress : this.solanaAddress,
      participantIndex: this.participantIndex,
      sessionInfo: this.sessionInfo,
      receivedRound1Packages: Array.from(this.receivedRound1Packages),
      receivedRound2Packages: Array.from(this.receivedRound2Packages),
      frostDkgInitialized: this.frostDkg !== null
    };
  }

  // --- Test Support Methods ---
  // These methods are added to support error handling tests

  private _handleDataChannelFailure(peerId: string): void {
    this._log(`Handling data channel failure for ${peerId}`);
    // Clean up any existing connection state
    this.dataChannels.delete(peerId);
    this.peerConnections.delete(peerId);
    this.onWebRTCConnectionUpdate(peerId, false);
  }

  private _handleConnectionTimeout(peerId: string): void {
    this._log(`Handling connection timeout for ${peerId}`);
    // Clean up any existing connection state and notify about timeout
    const pc = this.peerConnections.get(peerId);
    if (pc) {
      pc.close();
      this.peerConnections.delete(peerId);
    }
    this.dataChannels.delete(peerId);
    this.onWebRTCConnectionUpdate(peerId, false);
  }

  private async _handleWebRTCMessage(fromPeerId: string, message: any): Promise<void> {
    this._log(`Handling WebRTC message from ${fromPeerId}: ${JSON.stringify(message)}`);

    if (!message) {
      this._log(`Received null/undefined message from ${fromPeerId}`);
      return;
    }

    // Delegate to existing message handler
    // Check if it's a valid WebRTCAppMessage (Rust enum format)
    if (message && typeof message === 'object' && Object.keys(message).length > 0) {
      this.handleWebRTCAppMessage(fromPeerId, message);
    } else {
      this._log(`Unknown message format from ${fromPeerId}: ${JSON.stringify(message)}`);
    }
  }

  // Add the missing method that tests expect
  private _generateSigningCommitment(): void {
    this._log(`Generating signing commitment`);

    if (!this.signingInfo) {
      this._log(`Cannot generate commitment: no signing info`);
      return;
    }

    // This is just the commitment generation part without sending
    const commitment = {
      data: `commitment-${this.localPeerId}-${Date.now()}`,
      participant: this.localPeerId
    };

    // Add our own commitment
    this.signingCommitments.set(this.localPeerId, commitment);
    this._log(`Generated commitment for local peer`);
  }

  // Add the missing initiatePeerConnection method that offscreen/index.ts calls
  public async initiatePeerConnection(peerId: string): Promise<void> {
    this._log(`Initiating peer connection to ${peerId}`);

    try {
      // Get or create peer connection
      const pc = await this._getOrCreatePeerConnection(peerId);
      if (!pc) {
        this._log(`Failed to create peer connection for ${peerId}`);
        return;
      }

      // Create data channel if we are the initiator (following politeness rule)
      if (this.localPeerId < peerId) {
        this._log(`Creating data channel for ${peerId} (we are initiator)`);
        const dataChannel = pc.createDataChannel('frost-dkg', {
          ordered: true
        });
        this._setupDataChannel(dataChannel, peerId);
      }

      // Create and send offer
      this._log(`Creating offer for ${peerId}`);
      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);

      // Send offer via WebSocket relay
      const wsMsgPayload = {
        websocket_msg_type: 'WebRTCSignal',
        Offer: { sdp: offer.sdp! }  // Direct at root level, matching Rust enum structure
      };

      if (this.sendPayloadToBackgroundForRelay) {
        this.sendPayloadToBackgroundForRelay(peerId, wsMsgPayload as any);
        this._log(`Sent Offer to ${peerId} via background`);
      } else {
        this._log(`Cannot send Offer to ${peerId}: no relay callback available`);
      }

    } catch (error) {
      this._log(`Error initiating peer connection to ${peerId}: ${this._getErrorMessage(error)}`);
    }
  }

  // Add missing status methods that offscreen/index.ts calls
  public getDataChannelStatus(): Record<string, string> {
    const status: Record<string, string> = {};
    this.dataChannels.forEach((channel, peerId) => {
      status[peerId] = channel.readyState;
    });
    return status;
  }

  public getConnectedPeers(): string[] {
    return Array.from(this.dataChannels.keys()).filter(peerId => {
      const dc = this.dataChannels.get(peerId);
      return dc && dc.readyState === 'open';
    });
  }

  public getPeerConnectionStatus(): Record<string, string> {
    const status: Record<string, string> = {};
    this.peerConnections.forEach((pc, peerId) => {
      status[peerId] = pc.connectionState;
    });
    return status;
  }

  // Method to update session info and trigger mesh status check
  public updateSessionInfo(sessionInfo: SessionInfo): void {
    this._log(`Updating session info for session ${sessionInfo.session_id}`);
    this._log(`Participants: [${sessionInfo.participants.join(', ')}]`);
    this._log(`Accepted devices: [${sessionInfo.accepted_devices.join(', ')}]`);

    // Canonicalize participant order (sorted) so FROST identifiers match the
    // Rust core regardless of join order — see _withSortedParticipants (#29).
    this.sessionInfo = this._withSortedParticipants(sessionInfo)!;

    // Trigger mesh status check when session acceptance status changes
    this._checkMeshStatus();
  }

  // Enhanced auto-trigger logic for DKG
  private _shouldAutoTriggerDkg(): boolean {
    if (!this.sessionInfo) {
      this._log(`Auto-trigger check: No session info`);
      return false;
    }

    // Condition 1: Mesh is fully Ready
    if (this.meshStatus.type === MeshStatusType.Ready) {
      this._log(`Auto-trigger check: Mesh is Ready ✅`);
      return true;
    }

    // Condition 2: Mesh is PartiallyReady and all session participants have accepted
    if (this.meshStatus.type === MeshStatusType.PartiallyReady) {
      const allParticipantsAccepted = this.sessionInfo.participants.every(peerId =>
        this.sessionInfo!.accepted_devices.includes(peerId)
      );

      this._log(`Auto-trigger check: Mesh PartiallyReady, all participants accepted: ${allParticipantsAccepted} (${this.sessionInfo.accepted_devices.length}/${this.sessionInfo.participants.length})`);

      if (allParticipantsAccepted) {
        return true;
      }
    }

    // Condition 3: We have buffered packages from every remote peer (TUI or extension — both valid co-signers).
    const expectedParticipants = this.sessionInfo.participants.filter(p => p !== this.localPeerId);
    const bufferedFromParticipants = new Set(this.bufferedRound1Packages.map(pkg => pkg.fromPeerId));
    const allPeersReady = expectedParticipants.every(p => bufferedFromParticipants.has(p));

    this._log(`Auto-trigger check: peers-ready check - expected: [${expectedParticipants.join(', ')}], buffered from: [${Array.from(bufferedFromParticipants).join(', ')}], all ready: ${allPeersReady}`);

    if (allPeersReady && expectedParticipants.length > 0) {
      this._log(`Auto-trigger check: All peers have sent Round 1 packages ✅`);
      return true;
    }

    this._log(`Auto-trigger check: No conditions met yet`);
    return false;
  }
}