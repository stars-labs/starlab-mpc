// Based on /home/freeman.xiong/Documents/github/hecoinfo/crypto-rust-tools/webrtc-signal-server/src/lib.rs
import type {
    WebSocketClientMsg,
    WebSocketServerMsg,
    OffscreenToBackgroundMsg
} from "../../types/messages";

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

    public relayMessage(to: string, data: any): Promise<void> {
        return new Promise((resolve, reject) => {
            try {
                this.sendMessage({
                    type: "relay",
                    to,
                    data
                });
                // Resolve immediately after successful send since WebSocket is fire-and-forget
                resolve();
            } catch (error) {
                reject(error);
            }
        });
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
