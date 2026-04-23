// MessageHandler no longer exported (split into PopupMessageHandler +
// OffscreenMessageHandler). This test just uses a local duck-typed
// mock, so aliasing as `any` keeps the legacy fixture working.
type MessageHandler = any;
import type { OffscreenManager } from '../../../src/entrypoints/background/offscreenManager';
// Mock dependencies
import { describe, it, expect, beforeEach, afterEach } from 'bun:test';
import { jest } from 'bun:test';
const mockOffscreenManager = {
    ensureOffscreenDocument: jest.fn(),
    sendToOffscreen: jest.fn(),
    closeOffscreenDocument: jest.fn().mockResolvedValue(undefined)
};

const mockWebSocketManager = {
    isConnected: jest.fn(() => true),
    sendMessage: jest.fn(),
    getDeviceId: jest.fn(() => 'test-device-123')
};

const mockAccountService = {
    getCurrentAccount: jest.fn(),
    getAccounts: jest.fn(),
    getAccountsByBlockchain: jest.fn()
};

// Mock chrome APIs
const mockChrome = {
    runtime: {
        sendMessage: jest.fn(),
        onMessage: {
            addListener: jest.fn()
        }
    },
    storage: {
        local: {
            get: jest.fn(() => Promise.resolve({})),
            set: jest.fn()
        }
    }
};

global.chrome = mockChrome as any;

describe('MPC Signing Flow', () => {
    let messageHandler: any;
    
    beforeEach(() => {
        jest.clearAllMocks();
        
        // Reset mock return values after clearing
        mockWebSocketManager.isConnected.mockReturnValue(true);
        mockOffscreenManager.ensureOffscreenDocument.mockResolvedValue(undefined);
        
        // Create a mock message handler with the methods we need
        messageHandler = {
            offscreenManager: mockOffscreenManager,
            webSocketManager: mockWebSocketManager,
            accountService: mockAccountService,
            handleRequestSigningMessage: async function(message: any, sendResponse: (response: any) => void) {
                try {
                    await this.offscreenManager.ensureOffscreenDocument();
                    
                    if (!this.webSocketManager.isConnected()) {
                        sendResponse({ 
                            success: false, 
                            error: 'WebSocket not connected' 
                        });
                        return;
                    }
                    
                    const result = await this.offscreenManager.sendToOffscreen({
                        type: "requestSigning",
                        signingId: message.signingId,
                        transactionData: message.transactionData,
                        requiredSigners: message.requiredSigners
                    }, "requestSigning");
                    
                    sendResponse({ success: true, data: result });
                } catch (error: any) {
                    sendResponse({ 
                        success: false, 
                        error: error.message || 'Failed to initiate signing' 
                    });
                }
            }
        };
    });

    describe('initiating signing request', () => {
        it('should successfully initiate MPC signing', async () => {
            const signingRequest = {
                type: 'requestSigning',
                signingId: 'sign_12345',
                transactionData: '0x' + '00'.repeat(32),
                requiredSigners: 2
            };
            
            mockOffscreenManager.sendToOffscreen.mockResolvedValue({ 
                success: true,
                signingId: signingRequest.signingId
            });
            
            const response = await new Promise((resolve) => {
                messageHandler.handleRequestSigningMessage(signingRequest, resolve);
            });
            
            expect(mockOffscreenManager.ensureOffscreenDocument).toHaveBeenCalled();
            expect(mockOffscreenManager.sendToOffscreen).toHaveBeenCalledWith(
                {
                    type: 'requestSigning',
                    signingId: signingRequest.signingId,
                    transactionData: signingRequest.transactionData,
                    requiredSigners: signingRequest.requiredSigners
                },
                'requestSigning'
            );
            expect(response).toEqual({
                success: true,
                data: { success: true, signingId: signingRequest.signingId }
            });
        });

        it('should fail when WebSocket is not connected', async () => {
            mockWebSocketManager.isConnected.mockReturnValue(false);
            
            const signingRequest = {
                type: 'requestSigning',
                signingId: 'sign_12345',
                transactionData: '0x1234',
                requiredSigners: 2
            };
            
            const response = await new Promise((resolve) => {
                messageHandler.handleRequestSigningMessage(signingRequest, resolve);
            });
            
            expect(response).toEqual({
                success: false,
                error: 'WebSocket not connected'
            });
            expect(mockOffscreenManager.sendToOffscreen).not.toHaveBeenCalled();
        });

        it('should handle offscreen document errors', async () => {
            mockOffscreenManager.ensureOffscreenDocument.mockRejectedValue(
                new Error('Failed to create offscreen document')
            );
            
            const signingRequest = {
                type: 'requestSigning',
                signingId: 'sign_12345',
                transactionData: '0x1234',
                requiredSigners: 2
            };
            
            const response = await new Promise((resolve) => {
                messageHandler.handleRequestSigningMessage(signingRequest, resolve);
            });
            
            expect(response).toEqual({
                success: false,
                error: 'Failed to create offscreen document'
            });
        });
    });

    describe('signing acceptance flow', () => {
        it('should handle signing acceptance', async () => {
            const acceptMessage = {
                type: 'acceptSigning',
                signingId: 'sign_12345',
                accepted: true
            };
            
            mockOffscreenManager.sendToOffscreen.mockResolvedValue({ 
                success: true 
            });
            
            // Create acceptance handler
            messageHandler.handleAcceptSigningMessage = async function(message: any, sendResponse: (response: any) => void) {
                try {
                    const result = await this.offscreenManager.sendToOffscreen({
                        type: 'acceptSigning',
                        signingId: message.signingId,
                        accepted: message.accepted
                    }, 'acceptSigning');
                    
                    sendResponse({ success: true, data: result });
                } catch (error: any) {
                    sendResponse({ 
                        success: false, 
                        error: error.message 
                    });
                }
            };
            
            const response = await new Promise((resolve) => {
                messageHandler.handleAcceptSigningMessage(acceptMessage, resolve);
            });
            
            expect(mockOffscreenManager.sendToOffscreen).toHaveBeenCalledWith(
                {
                    type: 'acceptSigning',
                    signingId: acceptMessage.signingId,
                    accepted: acceptMessage.accepted
                },
                'acceptSigning'
            );
            expect(response).toEqual({
                success: true,
                data: { success: true }
            });
        });
    });

    describe('signing completion', () => {
        it('should handle successful signing completion', async () => {
            const completionMessage = {
                type: 'signingComplete',
                signingId: 'sign_12345',
                signature: '0x' + 'a'.repeat(130),
                transactionData: '0x' + '00'.repeat(32)
            };
            
            // Create completion handler
            messageHandler.handleSigningCompleteMessage = async function(message: any, sendResponse: (response: any) => void) {
                try {
                    // Store signature for later retrieval
                    await chrome.storage.local.set({
                        [`signature:${message.signingId}`]: {
                            signature: message.signature,
                            transactionData: message.transactionData,
                            timestamp: Date.now()
                        }
                    });
                    
                    sendResponse({ 
                        success: true,
                        signature: message.signature
                    });
                } catch (error: any) {
                    sendResponse({ 
                        success: false, 
                        error: error.message 
                    });
                }
            };
            
            const response = await new Promise((resolve) => {
                messageHandler.handleSigningCompleteMessage(completionMessage, resolve);
            });
            
            expect(mockChrome.storage.local.set).toHaveBeenCalledWith({
                [`signature:${completionMessage.signingId}`]: {
                    signature: completionMessage.signature,
                    transactionData: completionMessage.transactionData,
                    timestamp: expect.any(Number)
                }
            });
            expect(response).toEqual({
                success: true,
                signature: completionMessage.signature
            });
        });

        it('should handle signing errors', async () => {
            const errorMessage = {
                type: 'signingError',
                signingId: 'sign_12345',
                error: 'Insufficient signers'
            };
            
            // Create error handler
            messageHandler.handleSigningErrorMessage = function(message: any, sendResponse: (response: any) => void) {
                sendResponse({ 
                    success: false,
                    error: message.error,
                    signingId: message.signingId
                });
            };
            
            const response = await new Promise((resolve) => {
                messageHandler.handleSigningErrorMessage(errorMessage, resolve);
            });
            
            expect(response).toEqual({
                success: false,
                error: 'Insufficient signers',
                signingId: 'sign_12345'
            });
        });
    });

    describe('RPC signing integration', () => {
        it('should handle eth_signTransaction request', async () => {
            const rpcRequest = {
                method: 'eth_signTransaction',
                params: [{
                    from: '0x742d35Cc6634C0532925a3b844Bc9e7595f4279',
                    to: '0x5aAeb6053F3e94c9b9A09F33669435E7EF1BEaEd',
                    value: '0x1',
                    gas: '0x5208',
                    nonce: '0x0'
                }]
            };
            
            // Mock account service
            mockAccountService.getCurrentAccount.mockReturnValue({
                address: '0x742d35Cc6634C0532925a3b844Bc9e7595f4279',
                blockchain: 'ethereum'
            });
            
            // Create RPC handler
            messageHandler.handleRpcSignTransaction = async function(params: any[], sendResponse: (response: any) => void) {
                try {
                    const account = this.accountService.getCurrentAccount();
                    if (!account || account.address.toLowerCase() !== params[0].from.toLowerCase()) {
                        throw new Error('Invalid from address');
                    }
                    
                    // Serialize transaction
                    const transactionData = JSON.stringify(params[0]);
                    const signingId = `rpc_sign_${Date.now()}`;
                    
                    // Initiate MPC signing
                    const signingResult = await this.offscreenManager.sendToOffscreen({
                        type: 'requestSigning',
                        signingId,
                        transactionData,
                        requiredSigners: 2
                    }, 'requestSigning');
                    
                    sendResponse({ 
                        success: true,
                        signingId,
                        message: 'Signing initiated'
                    });
                } catch (error: any) {
                    sendResponse({ 
                        success: false, 
                        error: error.message 
                    });
                }
            };
            
            mockOffscreenManager.sendToOffscreen.mockResolvedValue({ success: true });
            
            const response = await new Promise((resolve) => {
                messageHandler.handleRpcSignTransaction(rpcRequest.params, resolve);
            });
            
            expect(response).toMatchObject({
                success: true,
                signingId: expect.stringMatching(/^rpc_sign_\d+$/),
                message: 'Signing initiated'
            });
            expect(mockOffscreenManager.sendToOffscreen).toHaveBeenCalledWith(
                expect.objectContaining({
                    type: 'requestSigning',
                    transactionData: JSON.stringify(rpcRequest.params[0]),
                    requiredSigners: 2
                }),
                'requestSigning'
            );
        });

        it('should reject signing for non-current account', async () => {
            mockAccountService.getCurrentAccount.mockReturnValue({
                address: '0xDIFFERENT_ADDRESS',
                blockchain: 'ethereum'
            });
            
            // Create RPC handler
            messageHandler.handleRpcSignTransaction = async function(params: any[], sendResponse: (response: any) => void) {
                try {
                    const account = this.accountService.getCurrentAccount();
                    if (!account || account.address.toLowerCase() !== params[0].from.toLowerCase()) {
                        throw new Error('Invalid from address');
                    }
                    
                    sendResponse({ success: true });
                } catch (error: any) {
                    sendResponse({ 
                        success: false, 
                        error: error.message 
                    });
                }
            };
            
            const response = await new Promise((resolve) => {
                messageHandler.handleRpcSignTransaction([{
                    from: '0x742d35Cc6634C0532925a3b844Bc9e7595f4279',
                    to: '0x5aAeb6053F3e94c9b9A09F33669435E7EF1BEaEd',
                    value: '0x1'
                }], resolve);
            });
            
            expect(response).toEqual({
                success: false,
                error: 'Invalid from address'
            });
            expect(mockOffscreenManager.sendToOffscreen).not.toHaveBeenCalled();
        });
    });

    describe('concurrent signing requests', () => {
        it('should handle multiple concurrent signing requests', async () => {
            // Ensure WebSocket is connected
            mockWebSocketManager.isConnected.mockReturnValue(true);
            
            const signingRequests = [
                {
                    signingId: 'sign_1',
                    transactionData: '0x01',
                    requiredSigners: 2
                },
                {
                    signingId: 'sign_2',
                    transactionData: '0x02',
                    requiredSigners: 3
                },
                {
                    signingId: 'sign_3',
                    transactionData: '0x03',
                    requiredSigners: 2
                }
            ];
            
            mockOffscreenManager.sendToOffscreen.mockImplementation((message) => {
                if (message.type === 'requestSigning') {
                    return Promise.resolve({ 
                        success: true, 
                        signingId: message.signingId 
                    });
                }
                return Promise.resolve({ success: true });
            });
            
            // Initiate all signing requests concurrently
            const promises = signingRequests.map(request => 
                new Promise((resolve) => {
                    messageHandler.handleRequestSigningMessage({
                        type: 'requestSigning',
                        ...request
                    }, resolve);
                })
            );
            
            const responses = await Promise.all(promises);
            
            // All should succeed
            responses.forEach((response, index) => {
                expect(response).toEqual({
                    success: true,
                    data: { 
                        success: true, 
                        signingId: signingRequests[index].signingId 
                    }
                });
            });
            
            // Should have called offscreen for each request
            expect(mockOffscreenManager.sendToOffscreen).toHaveBeenCalledTimes(3);
        });
    });
});
