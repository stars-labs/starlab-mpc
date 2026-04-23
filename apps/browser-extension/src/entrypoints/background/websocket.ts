// Based on /home/freeman.xiong/Documents/github/hecoinfo/crypto-rust-tools/webrtc-signal-server/src/lib.rs
import type {
    WebSocketClientMsg,
    WebSocketServerMsg,
    OffscreenToBackgroundMsg
} from "@mpc-wallet/types/messages";

type MessageCallback = (message: WebSocketServerMsg) => void;
type ErrorCallback = (error: Event) => void;
type CloseCallback = (event: CloseEvent) => void;
type OpenCallback = (event: Event) => void;


export class WebSocketClient {
    private ws: WebSocket | null = null;
    private url: string;
    private onOpenCallbacks: Array<() => void> = [];
    private onMessageCallbacks: Array<(message: WebSocketServerMsg) => void> = [];
    private onErrorCallbacks: Array<(event: Event) => void> = [];
    private onCloseCallbacks: Array<(event: CloseEvent) => void> = [];

    constructor(url: string) {
        this.url = url;
    }

    public onOpen(callback: () => void): void {
        this.onOpenCallbacks.push(callback);
    }

    public onMessage(callback: (message: WebSocketServerMsg) => void): void {
        this.onMessageCallbacks.push(callback);
    }

    public onError(callback: (event: Event) => void): void {
        this.onErrorCallbacks.push(callback);
    }

    public onClose(callback: (event: CloseEvent) => void): void {
        this.onCloseCallbacks.push(callback);
    }

    public connect(): void {
        try {
            this.ws = new WebSocket(this.url);

            this.ws.onopen = () => {
                console.log("WebSocket connection established");
                this.onOpenCallbacks.forEach(callback => callback());
            };

            this.ws.onmessage = (event) => {
                const message: WebSocketServerMsg = JSON.parse(event.data);
                console.log("Received from server:", {
                    type: message?.type,
                    from: message?.from,
                    data_type: message?.data?.websocket_msg_type,
                    data_preview: typeof message?.data === 'object' ?
                        JSON.stringify(message.data).substring(0, 100) + '...' : message?.data
                });
                this.onMessageCallbacks.forEach(callback => callback(message));
            };

            this.ws.onerror = (event) => {
                console.error("WebSocket error:", event);
                this.onErrorCallbacks.forEach(callback => callback(event));
            };

            this.ws.onclose = (event) => {
                console.log("WebSocket connection closed:", event.code, event.reason);
                this.onCloseCallbacks.forEach(callback => callback(event));
                this.ws = null;
            };
        } catch (error) {
            console.error("Error establishing WebSocket connection:", error);
        }
    }

    public register(deviceId: string): void {
        this.sendMessage({
            type: "register",
            device_id: deviceId
        });
    }

    public listdevices(): void {
        console.log("Sending listdevices command to WebSocket server");
        this.sendMessage({
            type: "list_devices"
        });
    }

    public relayMessage(to: string, data: any): void {
        this.sendMessage({
            type: "relay",
            to,
            data
        });
    }

    /**
     * Announce a session (DKG or Signing) to the signal server. The
     * server stores it in its Durable Object and broadcasts a
     * `session_available` frame to every OTHER connected device — the
     * same channel TUI uses (see `apps/tui-node/src/elm/command.rs`
     * near line 555 + signal-server's cloudflare-worker lib.rs:218).
     *
     * `sessionInfo` must already be in the TUI-compatible wire shape
     * (use `buildWireSessionInfo()` from utils/session-parse.ts).
     * This function does no shape validation — garbage in, garbage
     * broadcast.
     */
    public announceSession(sessionInfo: Record<string, unknown>): void {
        this.sendMessage({
            type: "announce_session",
            session_info: sessionInfo,
        } as any);
    }

    /**
     * Ask the server for every session currently stored that might be
     * relevant to this device. Reply arrives as `sessions_for_device`
     * with an array of `session_info` objects. Useful on reconnect so
     * we don't miss sessions announced while we were offline.
     */
    public requestActiveSessions(): void {
        this.sendMessage({ type: "request_active_sessions" } as any);
    }

    /**
     * Notify the signal server that this device has joined a session.
     * Shape mirrors TUI's `JoinDKG` command in command.rs:903 —
     * minimal payload: `{ session_id, participant_joined }`. Server
     * appends `participant_joined` to the stored session's
     * `participants` array and broadcasts a fresh `session_available`
     * to every participant so the creator's UI roster flips to show
     * the new joiner.
     *
     * NOTE: this is the *join intent* signal only — it doesn't
     * transport any FROST round data. Round 1/2/3 packages travel
     * over WebRTC data channels once the mesh is established.
     */
    public sendSessionStatusUpdate(sessionId: string, deviceId: string): void {
        this.sendMessage({
            type: "session_status_update",
            session_info: {
                session_id: sessionId,
                participant_joined: deviceId,
            },
        } as any);
    }

    private sendMessage(message: WebSocketClientMsg): void {
        if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
            console.error("WebSocket is not connected. Cannot send message:", message);
            return;
        }

        try {
            this.ws.send(JSON.stringify(message));
            console.log("Sent to server:", message);
        } catch (error) {
            console.error("Error sending WebSocket message:", error);
        }
    }

    public getReadyState(): number {
        return this.ws?.readyState ?? WebSocket.CLOSED;
    }

    public getWebSocket(): WebSocket | null {
        return this.ws;
    }

    public disconnect(): void {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }
}
