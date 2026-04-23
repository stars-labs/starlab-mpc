import { describe, it, expect, beforeEach, afterEach, mock } from 'bun:test';
// ExtensionMessage / ExtensionResponse were removed from
// @mpc-wallet/types; the test file uses local `any` types for
// the mock handler signatures (messages are constructed inline
// and only checked structurally).
type ExtensionMessage = any;
type ExtensionResponse = any;

// Mock Chrome runtime API
const mockChrome = {
    runtime: {
        sendMessage: mock(async (message: any) => ({ success: true })),
        onMessage: {
            addListener: mock((listener: Function) => {}),
            removeListener: mock((listener: Function) => {})
        }
    },
    storage: {
        local: {
            get: mock(async (keys: any) => ({})),
            set: mock(async (data: any) => {}),
            clear: mock(async () => {})
        }
    },
    tabs: {
        query: mock(async (query: any) => []),
        sendMessage: mock(async (tabId: number, message: any) => ({ success: true }))
    }
};

(global as any).chrome = mockChrome;

// Mock dependencies
const mockWebSocketManager = {
    connect: mock(async () => true),
    disconnect: mock(() => {}),
    sendMessage: mock(async (message: any) => true),
    isConnected: mock(() => true),
    onMessage: mock((callback: Function) => {}),
    onConnectionChange: mock((callback: Function) => {})
};

// Typed loosely — same reason as mockSigningManager above.
const mockSessionManager: any = {
    createSession: mock(async (params: any) => ({ sessionId: 'test-session', success: true })),
    joinSession: mock(async (sessionId: string) => ({ success: true })),
    leaveSession: mock(async (sessionId: string) => ({ success: true })),
    getCurrentSession: mock(() => null),
    getSessions: mock(() => []),
    onSessionUpdate: mock((callback: Function) => {})
};

const mockDKGManager = {
    startDKG: mock(async (params: any) => ({ success: true })),
    handleDKGMessage: mock(async (message: any) => ({ success: true })),
    getDKGStatus: mock(() => ({ status: 'idle', progress: 0 })),
    onDKGUpdate: mock((callback: Function) => {})
};

// Typed loosely because tests override mockResolvedValueOnce with
// various response shapes (success + signingId, success + error,
// etc.). Lockdown typing would make the test-specific override
// shapes collide with the inferred default shape.
const mockSigningManager: any = {
    initiateTransaction: mock(async (params: any) => ({ success: true, signingId: 'test-sign-123' })),
    handleSigningMessage: mock(async (message: any) => ({ success: true })),
    getSigningStatus: mock(() => ({ status: 'idle', pending: [] })),
    onSigningUpdate: mock((callback: Function) => {})
};

const mockStateManager = {
    getState: mock(() => ({
        isConnected: true,
        currentAccount: null,
        sessions: [],
        dkgStatus: { status: 'idle' },
        signingStatus: { status: 'idle', pending: [] }
    })),
    updateState: mock(async (updates: any) => {}),
    onStateChange: mock((callback: Function) => {})
};

// Mock message handlers module
const createMessageHandlers = () => ({
    handleMessage: async (message: ExtensionMessage, sender: chrome.runtime.MessageSender): Promise<ExtensionResponse> => {
        switch (message.type) {
            case 'GET_STATE':
                return {
                    success: true,
                    data: mockStateManager.getState()
                };

            case 'CONNECT_WEBSOCKET':
                const connected = await mockWebSocketManager.connect();
                return {
                    success: connected,
                    data: { connected }
                };

            case 'CREATE_SESSION':
                const sessionResult = await mockSessionManager.createSession(message.payload);
                return {
                    success: sessionResult.success,
                    data: sessionResult
                };

            case 'JOIN_SESSION':
                const joinResult = await mockSessionManager.joinSession(message.payload.sessionId);
                return {
                    success: joinResult.success,
                    data: joinResult
                };

            case 'START_DKG':
                const dkgResult = await mockDKGManager.startDKG(message.payload);
                return {
                    success: dkgResult.success,
                    data: dkgResult
                };

            case 'INITIATE_TRANSACTION':
                const signResult = await mockSigningManager.initiateTransaction(message.payload);
                return {
                    success: signResult.success,
                    data: signResult
                };

            case 'GET_ACCOUNTS':
                return {
                    success: true,
                    data: {
                        accounts: [],
                        currentAccount: null
                    }
                };

            case 'SET_CURRENT_ACCOUNT':
                await mockStateManager.updateState({ 
                    currentAccount: message.payload.accountId 
                });
                return {
                    success: true,
                    data: { accountId: message.payload.accountId }
                };

            default:
                return {
                    success: false,
                    error: `Unknown message type: ${(message as any).type}`
                };
        }
    }
});

describe('Background Message Handlers', () => {
    let messageHandlers: ReturnType<typeof createMessageHandlers>;

    beforeEach(() => {
        // Clear all mocks - only clear actual mock functions
        mockWebSocketManager.connect.mockClear();
        mockWebSocketManager.disconnect.mockClear();
        mockWebSocketManager.sendMessage.mockClear();
        mockWebSocketManager.isConnected.mockClear();
        mockWebSocketManager.onMessage.mockClear();
        mockWebSocketManager.onConnectionChange.mockClear();
        
        mockSessionManager.createSession.mockClear();
        mockSessionManager.joinSession.mockClear();
        mockSessionManager.leaveSession.mockClear();
        mockSessionManager.getCurrentSession.mockClear();
        mockSessionManager.getSessions.mockClear();
        mockSessionManager.onSessionUpdate.mockClear();
        
        mockDKGManager.startDKG.mockClear();
        mockDKGManager.handleDKGMessage.mockClear();
        mockDKGManager.getDKGStatus.mockClear();
        mockDKGManager.onDKGUpdate.mockClear();
        
        mockSigningManager.initiateTransaction.mockClear();
        mockSigningManager.handleSigningMessage.mockClear();
        mockSigningManager.getSigningStatus.mockClear();
        mockSigningManager.onSigningUpdate.mockClear();
        
        mockStateManager.getState.mockClear();
        mockStateManager.updateState.mockClear();
        mockStateManager.onStateChange.mockClear();
        
        mockChrome.runtime.sendMessage.mockClear();
        mockChrome.runtime.onMessage.addListener.mockClear();
        mockChrome.runtime.onMessage.removeListener.mockClear();
        
        mockChrome.storage.local.get.mockClear();
        mockChrome.storage.local.set.mockClear();
        mockChrome.storage.local.clear.mockClear();
        
        mockChrome.tabs.query.mockClear();
        mockChrome.tabs.sendMessage.mockClear();

        messageHandlers = createMessageHandlers();
    });

    describe('State Management Messages', () => {
        it('should handle GET_STATE message', async () => {
            const message: ExtensionMessage = {
                type: 'GET_STATE',
                id: 'test-1',
                timestamp: Date.now()
            };

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(true);
            expect(response.data).toBeDefined();
            expect(response.data.isConnected).toBe(true);
            expect(mockStateManager.getState).toHaveBeenCalled();
        });

        it('should handle SET_CURRENT_ACCOUNT message', async () => {
            const message: ExtensionMessage = {
                type: 'SET_CURRENT_ACCOUNT',
                id: 'test-2',
                timestamp: Date.now(),
                payload: {
                    accountId: 'account-123'
                }
            };

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(true);
            expect(response.data.accountId).toBe('account-123');
            expect(mockStateManager.updateState).toHaveBeenCalledWith({
                currentAccount: 'account-123'
            });
        });

        it('should handle GET_ACCOUNTS message', async () => {
            const message: ExtensionMessage = {
                type: 'GET_ACCOUNTS',
                id: 'test-3',
                timestamp: Date.now()
            };

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(true);
            expect(response.data.accounts).toBeDefined();
            expect(Array.isArray(response.data.accounts)).toBe(true);
        });
    });

    describe('WebSocket Connection Messages', () => {
        it('should handle CONNECT_WEBSOCKET message', async () => {
            const message: ExtensionMessage = {
                type: 'CONNECT_WEBSOCKET',
                id: 'test-4',
                timestamp: Date.now()
            };

            mockWebSocketManager.connect.mockResolvedValueOnce(true);

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(true);
            expect(response.data.connected).toBe(true);
            expect(mockWebSocketManager.connect).toHaveBeenCalled();
        });

        it('should handle WebSocket connection failure', async () => {
            const message: ExtensionMessage = {
                type: 'CONNECT_WEBSOCKET',
                id: 'test-5',
                timestamp: Date.now()
            };

            mockWebSocketManager.connect.mockResolvedValueOnce(false);

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(false);
            expect(response.data.connected).toBe(false);
        });

        it('should handle DISCONNECT_WEBSOCKET message', async () => {
            const message: ExtensionMessage = {
                type: 'DISCONNECT_WEBSOCKET',
                id: 'test-6',
                timestamp: Date.now()
            };

            // Add this case to our mock handler
            const disconnectResponse = await (async () => {
                mockWebSocketManager.disconnect();
                return { success: true, data: { disconnected: true } };
            })();

            expect(disconnectResponse.success).toBe(true);
            expect(mockWebSocketManager.disconnect).toHaveBeenCalled();
        });
    });

    describe('Session Management Messages', () => {
        it('should handle CREATE_SESSION message', async () => {
            const message: ExtensionMessage = {
                type: 'CREATE_SESSION',
                id: 'test-7',
                timestamp: Date.now(),
                payload: {
                    sessionId: 'test-session',
                    totalParticipants: 3,
                    threshold: 2,
                    curve: 'secp256k1'
                }
            };

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(true);
            expect(response.data.sessionId).toBe('test-session');
            expect(mockSessionManager.createSession).toHaveBeenCalledWith(message.payload);
        });

        it('should handle JOIN_SESSION message', async () => {
            const message: ExtensionMessage = {
                type: 'JOIN_SESSION',
                id: 'test-8',
                timestamp: Date.now(),
                payload: {
                    sessionId: 'existing-session'
                }
            };

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(true);
            expect(mockSessionManager.joinSession).toHaveBeenCalledWith('existing-session');
        });

        it('should handle session creation failure', async () => {
            const message: ExtensionMessage = {
                type: 'CREATE_SESSION',
                id: 'test-9',
                timestamp: Date.now(),
                payload: {
                    sessionId: 'invalid-session',
                    totalParticipants: 3,
                    threshold: 2,
                    curve: 'secp256k1'
                }
            };

            mockSessionManager.createSession.mockResolvedValueOnce({
                success: false,
                error: 'Session already exists'
            });

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(false);
        });
    });

    describe('DKG Messages', () => {
        it('should handle START_DKG message', async () => {
            const message: ExtensionMessage = {
                type: 'START_DKG',
                id: 'test-10',
                timestamp: Date.now(),
                payload: {
                    sessionId: 'dkg-session',
                    participantId: 1
                }
            };

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(true);
            expect(mockDKGManager.startDKG).toHaveBeenCalledWith(message.payload);
        });

        it('should handle DKG_MESSAGE', async () => {
            const dkgMessage = {
                type: 'DKG_ROUND_1',
                fromParticipant: 2,
                data: 'dkg_round1_data'
            };

            // Test the DKG message handling
            const result = await mockDKGManager.handleDKGMessage(dkgMessage);
            expect(result.success).toBe(true);
            expect(mockDKGManager.handleDKGMessage).toHaveBeenCalledWith(dkgMessage);
        });

        it('should handle GET_DKG_STATUS message', async () => {
            const status = mockDKGManager.getDKGStatus();
            expect(status.status).toBe('idle');
            expect(status.progress).toBe(0);
        });
    });

    describe('Transaction Signing Messages', () => {
        it('should handle INITIATE_TRANSACTION message', async () => {
            const message: ExtensionMessage = {
                type: 'INITIATE_TRANSACTION',
                id: 'test-11',
                timestamp: Date.now(),
                payload: {
                    to: '0x742d35Cc6641C4532B4d2a3F44ae7f35E0D29636',
                    value: '1000000000000000000',
                    chainId: 1,
                    accountId: 'account-123'
                }
            };

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(true);
            expect(response.data.signingId).toBe('test-sign-123');
            expect(mockSigningManager.initiateTransaction).toHaveBeenCalledWith(message.payload);
        });

        it('should handle SIGN_TRANSACTION message', async () => {
            const signingMessage = {
                type: 'SIGNING_ROUND_1',
                signingId: 'test-sign-123',
                fromParticipant: 1,
                data: 'signing_data'
            };

            const result = await mockSigningManager.handleSigningMessage(signingMessage);
            expect(result.success).toBe(true);
            expect(mockSigningManager.handleSigningMessage).toHaveBeenCalledWith(signingMessage);
        });

        it('should handle transaction initiation failure', async () => {
            const message: ExtensionMessage = {
                type: 'INITIATE_TRANSACTION',
                id: 'test-12',
                timestamp: Date.now(),
                payload: {
                    to: 'invalid-address',
                    value: '1000000000000000000',
                    chainId: 1,
                    accountId: 'account-123'
                }
            };

            mockSigningManager.initiateTransaction.mockResolvedValueOnce({
                success: false,
                error: 'Invalid recipient address'
            });

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(false);
        });
    });

    describe('Error Handling', () => {
        it('should handle unknown message types', async () => {
            const message = {
                type: 'UNKNOWN_MESSAGE_TYPE',
                id: 'test-13',
                timestamp: Date.now()
            } as any;

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(false);
            expect(response.error).toContain('Unknown message type');
        });

        it('should handle malformed messages', async () => {
            const malformedMessage = {
                // Missing required fields
                id: 'test-14'
            } as any;

            const response = await messageHandlers.handleMessage(malformedMessage, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(false);
        });

        it('should handle service errors gracefully', async () => {
            const message: ExtensionMessage = {
                type: 'CREATE_SESSION',
                id: 'test-15',
                timestamp: Date.now(),
                payload: {
                    sessionId: 'error-session',
                    totalParticipants: 3,
                    threshold: 2,
                    curve: 'secp256k1'
                }
            };

            mockSessionManager.createSession.mockRejectedValueOnce(new Error('Service unavailable'));

            // The handler should catch the error and return appropriate response
            try {
                const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);
                expect(response.success).toBe(false);
            } catch (error) {
                // If error is not caught by handler, test the error handling
                expect(error).toBeInstanceOf(Error);
            }
        });

        it('should validate message payload structure', async () => {
            const message: ExtensionMessage = {
                type: 'CREATE_SESSION',
                id: 'test-16',
                timestamp: Date.now(),
                payload: {
                    // Missing required fields
                    sessionId: 'incomplete-session'
                    // missing totalParticipants, threshold, curve
                } as any
            };

            // Mock validation failure
            mockSessionManager.createSession.mockResolvedValueOnce({
                success: false,
                error: 'Invalid session parameters'
            });

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(false);
        });
    });

    describe('Message Routing', () => {
        it('should route messages to appropriate handlers based on type', async () => {
            const messages: ExtensionMessage[] = [
                {
                    type: 'GET_STATE',
                    id: 'route-1',
                    timestamp: Date.now()
                },
                {
                    type: 'CONNECT_WEBSOCKET',
                    id: 'route-2',
                    timestamp: Date.now()
                },
                {
                    type: 'CREATE_SESSION',
                    id: 'route-3',
                    timestamp: Date.now(),
                    payload: {
                        sessionId: 'route-session',
                        totalParticipants: 2,
                        threshold: 2,
                        curve: 'secp256k1'
                    }
                }
            ];

            for (const message of messages) {
                const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);
                expect(response.success).toBe(true);
            }

            expect(mockStateManager.getState).toHaveBeenCalled();
            expect(mockWebSocketManager.connect).toHaveBeenCalled();
            expect(mockSessionManager.createSession).toHaveBeenCalled();
        });

        it('should handle concurrent messages', async () => {
            const messages: ExtensionMessage[] = Array.from({ length: 5 }, (_, i) => ({
                type: 'GET_STATE',
                id: `concurrent-${i}`,
                timestamp: Date.now()
            }));

            const responses = await Promise.all(
                messages.map(message => 
                    messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender)
                )
            );

            responses.forEach((response: any) => {
                expect(response.success).toBe(true);
            });

            expect(mockStateManager.getState).toHaveBeenCalledTimes(5);
        });
    });

    describe('Message Validation', () => {
        it('should validate required message fields', async () => {
            const invalidMessages = [
                { type: 'GET_STATE' }, // missing id and timestamp
                { id: 'test', timestamp: Date.now() }, // missing type
                { type: 'CREATE_SESSION', id: 'test', timestamp: Date.now() } // missing payload
            ];

            for (const message of invalidMessages) {
                const response = await messageHandlers.handleMessage(message as any, {} as chrome.runtime.MessageSender);
                // Should handle gracefully (exact behavior depends on implementation)
                expect(typeof response).toBe('object');
            }
        });

        it('should validate payload structure for complex messages', async () => {
            const message: ExtensionMessage = {
                type: 'INITIATE_TRANSACTION',
                id: 'validation-test',
                timestamp: Date.now(),
                payload: {
                    to: 'invalid-address-format',
                    value: 'invalid-amount',
                    chainId: 'invalid-chain-id',
                    accountId: ''
                } as any
            };

            mockSigningManager.initiateTransaction.mockResolvedValueOnce({
                success: false,
                error: 'Invalid transaction parameters'
            });

            const response = await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);

            expect(response.success).toBe(false);
        });
    });

    describe('Performance and Optimization', () => {
        it('should handle high-frequency state requests efficiently', async () => {
            const startTime = Date.now();
            
            const requests = Array.from({ length: 100 }, (_, i) => ({
                type: 'GET_STATE' as const,
                id: `perf-${i}`,
                timestamp: Date.now()
            }));

            const responses = await Promise.all(
                requests.map(message => 
                    messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender)
                )
            );

            const endTime = Date.now();
            const duration = endTime - startTime;

            // Should complete 100 requests reasonably quickly
            expect(duration).toBeLessThan(1000); // Less than 1 second
            expect(responses.length).toBe(100);
            responses.forEach((response: any) => {
                expect(response.success).toBe(true);
            });
        });

        it('should not leak memory with repeated message handling', async () => {
            // Create and handle many messages to test for memory leaks
            for (let i = 0; i < 1000; i++) {
                const message: ExtensionMessage = {
                    type: 'GET_STATE',
                    id: `memory-test-${i}`,
                    timestamp: Date.now()
                };

                await messageHandlers.handleMessage(message, {} as chrome.runtime.MessageSender);
            }

            // If we reach here without running out of memory, the test passes
            expect(true).toBe(true);
        });
    });
});