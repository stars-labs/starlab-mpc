import { match } from "ts-pattern";
import type { AppState } from "@mpc-wallet/types/appstate";
import { MeshStatusType } from "@mpc-wallet/types/mesh";
import { DkgState } from "@mpc-wallet/types/dkg";
import { INITIAL_APP_STATE } from "@mpc-wallet/types/appstate";

export interface MessageHandlerCallbacks {
  onInitialState: (appState: AppState) => void;
  onWSStatusUpdate: (connected: boolean, error?: string) => void;
  onDkgAddressUpdate: (address: string, blockchain: string) => void;
  onProcessingInvitesUpdate: (processingInvites: Set<string>) => void;
}

export class MessageHandler {
  private callbacks: MessageHandlerCallbacks;
  private processingInvites: Set<string> = new Set();

  constructor(callbacks: MessageHandlerCallbacks) {
    this.callbacks = callbacks;
  }

  handleBackgroundMessage(message: any, appState: AppState): AppState {
    console.log(
      "[MessageHandler] Background message received - Type:",
      message.type,
      "Data:",
      message,
    );

    let newState = { ...appState };

    match(message.type)
      .with("initialState", () => {
        console.log(
          "[MessageHandler] Processing initialState - state restoration from background",
        );

        // Update the entire app state from background
        newState = {
          ...appState,
          deviceId: message.deviceId || "",
          connecteddevices: [...(message.connecteddevices || [])],
          wsConnected: message.wsConnected || false,
          sessionInfo: message.sessionInfo || null,
          invites: message.invites ? [...message.invites] : [],
          meshStatus: message.meshStatus || { type: MeshStatusType.Incomplete },
          dkgState: message.dkgState || DkgState.Idle,
          webrtcConnections: message.webrtcConnections || {},
          chain: message.blockchain || appState.chain || "ethereum",
          blockchain: message.blockchain || appState.blockchain || "ethereum",
        };

        console.log("[MessageHandler] App state updated from initialState:", newState);
        this.callbacks.onInitialState(newState);
      })
      .with("wsStatus", () => {
//         console.log("[MessageHandler] Processing wsStatus:", message);
        newState.wsConnected = message.connected || false;
        
        const error = !message.connected && message.reason 
          ? `WebSocket disconnected: ${message.reason}` 
          : "";
        
        this.callbacks.onWSStatusUpdate(newState.wsConnected, error);
      })
      .with("wsMessage", () => {
//         console.log("[MessageHandler] Processing wsMessage:", message);
        if (message.message) {
          console.log("[MessageHandler] Server message:", message.message);
          if (message.message.type === "devices") {
            newState.connecteddevices = [...(message.message.devices || [])];
          }
        }
      })
      .with("wsError", () => {
//         console.log("[MessageHandler] Processing wsError:", message);
        console.error("[MessageHandler] WebSocket error:", message.error);
      })
      .with("deviceList", () => {
//         console.log("[MessageHandler] Processing deviceList:", message);
        newState.connecteddevices = [...(message.devices || [])];
      })
      .with("sessionUpdate", () => {
//         console.log("[MessageHandler] Processing sessionUpdate:", message);

        // Clear processing state for any sessions that are no longer in invites
        const newInviteIds = new Set(
          (message.invites || []).map((inv: any) => inv.session_id),
        );
        this.processingInvites.forEach((sessionId) => {
          if (!newInviteIds.has(sessionId)) {
            console.log(
              "[MessageHandler] Clearing processing state for accepted/removed session:",
              sessionId,
            );
            this.processingInvites.delete(sessionId);
          }
        });

        newState.sessionInfo = message.sessionInfo || null;
        newState.invites = message.invites ? [...message.invites] : [];
        
        console.log("[MessageHandler] Session update:", {
          sessionInfo: newState.sessionInfo,
          invites: newState.invites,
        });

        // Log accepted devices for debugging
        if (newState.sessionInfo && newState.sessionInfo.accepted_devices) {
          console.log(
            "[MessageHandler] Session accepted devices:",
            newState.sessionInfo.accepted_devices,
          );
          // Filter out any null/undefined values that might have been added
          newState.sessionInfo.accepted_devices =
            newState.sessionInfo.accepted_devices.filter(
              (peer) => peer != null && peer !== undefined,
            );
        }

        this.callbacks.onProcessingInvitesUpdate(this.processingInvites);
      })
      .with("meshStatusUpdate", () => {
//         console.log("[MessageHandler] Processing meshStatusUpdate:", message);
        newState.meshStatus = message.status || {
          type: MeshStatusType.Incomplete,
        };
        console.log("[MessageHandler] Mesh status update:", newState.meshStatus);
      })
      .with("webrtcConnectionUpdate", () => {
//         console.log("[MessageHandler] Processing webrtcConnectionUpdate:", message);

        if (message.deviceId && typeof message.connected === "boolean") {
          console.log(
            "[MessageHandler] Updating peer connection:",
            message.deviceId,
            "->",
            message.connected,
          );

          newState.webrtcConnections = {
            ...newState.webrtcConnections,
            [message.deviceId]: message.connected,
          };

          console.log(
            "[MessageHandler] Updated webrtcConnections:",
            newState.webrtcConnections,
          );
        } else {
          console.warn("[MessageHandler] Invalid webrtcConnectionUpdate message:", message);
        }
      })
      .with("dkgStateUpdate", () => {
//         console.log("[MessageHandler] Processing dkgStateUpdate:", message);
        newState.dkgState = message.state || DkgState.Idle;
        console.log("[MessageHandler] DKG state update:", newState.dkgState);
      })
      .with("fromOffscreen", () => {
//         console.log("[MessageHandler] Processing fromOffscreen wrapper:", message);
        // Handle wrapped messages from offscreen
        if (message.payload) {
          console.log(
            "[MessageHandler] Unwrapping and processing payload:",
            message.payload,
          );
          return this.handleBackgroundMessage(message.payload, newState);
        }
      })
      .with("webrtcStatusUpdate", () => {
//         console.log("[MessageHandler] Processing webrtcStatusUpdate:", message);
        if (message.deviceId && message.status) {
          console.log(
            `[MessageHandler] WebRTC status for ${message.deviceId}: ${message.status}`,
          );
        }
      })
      .with("dataChannelStatusUpdate", () => {
//         console.log("[MessageHandler] Processing dataChannelStatusUpdate:", message);
        if (message.deviceId && message.channelName && message.state) {
          console.log(
            `[MessageHandler] Data channel ${message.channelName} for ${message.deviceId}: ${message.state}`,
          );
        }
      })
      .with("peerConnectionStatusUpdate", () => {
//         console.log("[MessageHandler] Processing peerConnectionStatusUpdate:", message);
        if (message.deviceId && message.connectionState) {
          console.log(
            `[MessageHandler] Peer connection for ${message.deviceId}: ${message.connectionState}`,
          );
        }
      })
      .with("dkgAddressUpdate", () => {
//         console.log("[MessageHandler] Processing dkgAddressUpdate:", message);
        if (message.address && message.blockchain) {
          console.log(
            "[MessageHandler] DKG address automatically fetched:",
            message.address,
            "for",
            message.blockchain,
          );
          this.callbacks.onDkgAddressUpdate(message.address, message.blockchain);
        }
      })
      .otherwise(() => {
        console.log("[MessageHandler] Unhandled message type:", message.type, message);
      });

    return newState;
  }

  getProcessingInvites(): Set<string> {
    return this.processingInvites;
  }

  addProcessingInvite(sessionId: string): void {
    this.processingInvites.add(sessionId);
    this.callbacks.onProcessingInvitesUpdate(this.processingInvites);
  }

  removeProcessingInvite(sessionId: string): void {
    this.processingInvites.delete(sessionId);
    this.callbacks.onProcessingInvitesUpdate(this.processingInvites);
  }
}
