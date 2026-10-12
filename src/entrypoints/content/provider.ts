import { MESSAGE_PREFIX, MessageType } from '../../constants';
import type {
    ContentToBackgroundMsg,
    BackgroundToContentMsg,
    InjectedToContentMsg,
    ContentToInjectedMsg,
    JsonRpcRequest,
    JsonRpcResponse
} from '../../types/messages';
import { match } from "ts-pattern";

// 定义消息接口
export interface ContentMessage {
    type: string;
    payload: JsonRpcRequest | JsonRpcResponse;
}

export class ContentProvider {

    constructor() {
        // 监听来自页面的消息
        window.addEventListener('message', this.handlePageMessage);

        // 监听来自扩展的消息
        chrome.runtime.onMessage.addListener(this.handleExtensionMessage);
    }

    // 处理来自页面的消息
    private handlePageMessage = (event: MessageEvent) => {
        // 忽略来自其他源的消息
        if (event.source !== window) return;

        try {
            const data = event.data as InjectedToContentMsg;

            // Debug the received message
            console.log('[Content Script] Received message from page:', data);

            // 检查消息是否来自我们的注入脚本
            if (data && typeof data === 'object' && data.type === `${MESSAGE_PREFIX}${MessageType.REQUEST}`) {
                // 解析请求
                const request = data.payload as JsonRpcRequest;
                console.log(`[Content Script] Processing request: ${request.method}`, request);

                // Handle eth_requestAccounts and eth_accounts specially for quick response
                if (request.method === 'eth_requestAccounts' || request.method === 'eth_accounts') {
                    console.log(`[Content Script] Fast-tracking ${request.method} request`);

                    // Quickly respond with MPC address from storage if available
                    chrome.storage.local.get(['mpc_ethereum_address'], (result) => {
                        try {
                            if (result && result.mpc_ethereum_address) {
                                console.log('[Content Script] Using saved MPC Ethereum address:', result.mpc_ethereum_address);
                                // Store address in sessionStorage for direct access by injected script
                                try {
                                    if (window.sessionStorage) {
                                        window.sessionStorage.setItem('starlab_wallet_accounts', JSON.stringify([result.mpc_ethereum_address]));
                                    }
                                } catch (err) {
                                    console.log('[Content Script] Unable to update sessionStorage:', err);
                                }

                                this.sendToPage({
                                    type: 'WALLET_RESPONSE',
                                    payload: {
                                        id: request.id,
                                        jsonrpc: '2.0',
                                        result: [result.mpc_ethereum_address]
                                    }
                                });
                            } else {
                                // Still forward to background for processing
                                console.log('[Content Script] No saved address, forwarding to background');
                                this.forwardToBackground(request);
                            }
                        } catch (err) {
                            console.error('[Content Script] Error in storage callback:', err);
                            // Send error to page
                            this.sendToPage({
                                type: 'WALLET_RESPONSE',
                                payload: {
                                    id: request.id,
                                    jsonrpc: '2.0',
                                    error: {
                                        code: -32603,
                                        message: 'Internal error: ' + (err instanceof Error ? err.message : 'Unknown error')
                                    }
                                }
                            });
                        }
                    });
                } else {
                    // 转发到 background script
                    this.forwardToBackground(request);
                }
            }
        } catch (err) {
            console.error('[Content Script] Error handling page message:', err);
        }
    };

    // 处理来自扩展的消息
    private handleExtensionMessage = (
        message: BackgroundToContentMsg,
        sender: chrome.runtime.MessageSender,
        sendResponse: (response?: any) => void
    ) => {
        console.log('[Content Script] Received message from extension:', message);

        // 转发响应到页面
        if (message.type === 'REQUEST_RESPONSE' && message.payload) {
            console.log('[Content Script] Forwarding RPC response to page:', message.payload);
            this.sendToPage({
                type: 'WALLET_RESPONSE',
                payload: message.payload
            });
            sendResponse({ received: true });
        } else if (message.type === 'GET_STATE') {
            // Return content script state information
            sendResponse({ active: true });
        } else if (message.type === 'SAVE_ADDRESS') {
            // Save address to storage for quick retrieval
            if (message.address && typeof message.address === 'string') {
                console.log('[Content Script] Saving address to storage:', message.address);
                chrome.storage.local.set({
                    ['mpc_' + (message.blockchain || 'ethereum') + '_address']: message.address
                }, () => {
                    sendResponse({ success: true });
                });
                return true; // Keep sendResponse valid
            }
        } else {
            console.log('[Content Script] Unknown message type:', message.type);
            sendResponse({ error: 'Unknown message type' });
        }
    };

    private forwardToBackground(request: JsonRpcRequest) {
        // Check if we can handle the request directly in the content script first
        if (this.canHandleLocally(request)) {
            console.log('[Content Script] Handling request locally:', request.method);
            this.handleRequestLocally(request);
            return;
        }

        // Use a simplified message structure that our background handler will recognize
        const message = {
            // Include type but don't rely on it
            type: 'FORWARD_REQUEST',
            // The payload is what matters - must have jsonrpc, id, method fields
            payload: request,
            timestamp: Date.now()
        };

        console.log('[Content Script] Forwarding request to background:', request.method);
        chrome.runtime.sendMessage(message, (response) => {
            if (chrome.runtime.lastError) {
                console.error('[Content Script] Error forwarding request:', chrome.runtime.lastError);
                // Send error response to page
                this.sendToPage({
                    type: 'WALLET_RESPONSE',
                    payload: {
                        id: request.id,
                        jsonrpc: '2.0',
                        error: {
                            code: -32603,
                            message: 'Internal error: ' + chrome.runtime.lastError.message
                        }
                    }
                });
                return;
            }

            if (response) {
                console.log('[Content Script] Got direct response from background:', response);
                // If we got a direct response, use it
                let payload: JsonRpcResponse;

                if (response.success && 'result' in response) {
                    payload = {
                        id: request.id,
                        jsonrpc: '2.0',
                        result: response.result
                    };
                } else if (response.error) {
                    // Handle different error formats
                    let errorMessage = response.error;
                    let errorCode = -32603; // Default Internal Error code

                    // Check if error is already an object with message
                    if (typeof response.error === 'object' && response.error !== null) {
                        errorMessage = response.error.message || JSON.stringify(response.error);
                        errorCode = response.error.code || errorCode;
                    }

                    payload = {
                        id: request.id,
                        jsonrpc: '2.0',
                        error: {
                            code: errorCode,
                            message: errorMessage
                        }
                    };
                } else if (request.method === 'eth_requestAccounts' && response.data && Array.isArray(response.data)) {
                    // Special handling for eth_requestAccounts
                    payload = {
                        id: request.id,
                        jsonrpc: '2.0',
                        result: response.data
                    };
                } else {
                    // Default to just returning the full response as result
                    payload = {
                        id: request.id,
                        jsonrpc: '2.0',
                        result: response
                    };
                }

                this.sendToPage({
                    type: 'WALLET_RESPONSE',
                    payload
                });
            }
            // If no response, background will send it later via handleExtensionMessage
        });
    }

    private sendToPage(message: ContentToInjectedMsg) {
        window.postMessage({
            ...message,
            type: `${MESSAGE_PREFIX}${MessageType.RESPONSE}`
        }, '*');
    }

    // Helper function to determine if a request can be handled locally
    private canHandleLocally(request: JsonRpcRequest): boolean {
        // List of methods we can handle directly in the content script
        const locallyHandlableMethods = [
            'eth_chainId',
            'eth_getBalance',
            'net_version',
            'eth_accounts',
            'eth_requestAccounts'
        ];

        return locallyHandlableMethods.includes(request.method);
    }

    // Handle RPC requests directly in the content script
    private handleRequestLocally(request: JsonRpcRequest): void {
        const { method, id } = request;

        console.log(`[Content Script] Handling ${method} locally`);

        const handleAccountsRequest = () => {
            // Get accounts from storage
            chrome.storage.local.get(['mpc_ethereum_address'], (result) => {
                try {
                    if (result && result.mpc_ethereum_address) {
                        console.log('[Content Script] Using saved MPC Ethereum address:', result.mpc_ethereum_address);

                        // Store in sessionStorage for direct access by injected script
                        try {
                            if (window.sessionStorage) {
                                window.sessionStorage.setItem('starlab_wallet_accounts', JSON.stringify([result.mpc_ethereum_address]));
                            }
                        } catch (err) {
                            console.log('[Content Script] Unable to update sessionStorage:', err);
                        }

                        this.sendToPage({
                            type: 'WALLET_RESPONSE',
                            payload: {
                                id,
                                jsonrpc: '2.0',
                                result: [result.mpc_ethereum_address]
                            }
                        });
                    } else {
                        // No saved address, fall back to requesting it from background
                        console.log('[Content Script] No saved address, requesting from background');
                        chrome.runtime.sendMessage(
                            { type: 'getEthereumAddress' },
                            (addressResponse) => {
                                if (chrome.runtime.lastError) {
                                    console.error('[Content Script] Error getting address:', chrome.runtime.lastError);
                                }

                                if (addressResponse && addressResponse.success &&
                                    addressResponse.data && addressResponse.data.ethereumAddress) {

                                    const address = addressResponse.data.ethereumAddress;
                                    console.log('[Content Script] Got address from background:', address);

                                    // Save to sessionStorage
                                    try {
                                        if (window.sessionStorage) {
                                            window.sessionStorage.setItem('starlab_wallet_accounts', JSON.stringify([address]));
                                        }
                                    } catch (err) {
                                        console.log('[Content Script] Unable to update sessionStorage:', err);
                                    }

                                    this.sendToPage({
                                        type: 'WALLET_RESPONSE',
                                        payload: {
                                            id,
                                            jsonrpc: '2.0',
                                            result: [address]
                                        }
                                    });
                                } else {
                                    // Forward original request to background as last resort
                                    console.log('[Content Script] Failed to get address, forwarding original request');
                                    const originalMessage = {
                                        type: 'FORWARD_REQUEST',
                                        payload: request,
                                        timestamp: Date.now()
                                    };
                                    chrome.runtime.sendMessage(originalMessage);
                                }
                            }
                        );
                    }
                } catch (err) {
                    console.error('[Content Script] Error handling accounts request:', err);
                    this.sendToPage({
                        type: 'WALLET_RESPONSE',
                        payload: {
                            id,
                            jsonrpc: '2.0',
                            error: {
                                code: -32603,
                                message: 'Internal error: ' + (err instanceof Error ? err.message : 'Unknown error')
                            }
                        }
                    });
                }
            });
        };

        match(method)
            .with('eth_chainId', () => {
                // Return Ethereum mainnet chainId by default
                this.sendToPage({
                    type: 'WALLET_RESPONSE',
                    payload: {
                        id,
                        jsonrpc: '2.0',
                        result: '0x1' // Ethereum mainnet
                    }
                });
            })
            .with('net_version', () => {
                // Return Ethereum mainnet network version by default
                this.sendToPage({
                    type: 'WALLET_RESPONSE',
                    payload: {
                        id,
                        jsonrpc: '2.0',
                        result: '1' // Ethereum mainnet
                    }
                });
            })
            .with('eth_accounts', handleAccountsRequest)
            .with('eth_requestAccounts', handleAccountsRequest)
            .otherwise(() => {
                // If we get here, we should be able to handle it but haven't implemented it yet
                console.warn(`[Content Script] Method ${method} marked as locally handleable but not implemented`);
                // Fall back to background
                const message = {
                    type: 'FORWARD_REQUEST',
                    payload: request,
                    timestamp: Date.now()
                };
                chrome.runtime.sendMessage(message);
            });
    }

    // 清理方法
    public cleanup() {
        window.removeEventListener('message', this.handlePageMessage);
        // 注意：Chrome 扩展 API 不提供 removeListener 的标准方式
    }
}

export default new ContentProvider();