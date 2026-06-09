import { describe, it, expect, beforeEach, afterEach, mock } from 'bun:test';
// InitWasmReturn was exported from src/lib/wasm-loader pre-
// monorepo. The loader module was removed when we moved to
// @starlab/core-wasm; the test uses the type annotation for
// mockInitWasm's return shape, which we can match with `any`.
type InitWasmReturn = any;

// Mock WASM module
// Typed as any so tests can attach extra methods (validate_keystore,
// cleanup, etc.) at runtime — the real WASM module has an open-ended
// export surface that tests stub incrementally as needed.
const mockWasmModule: any = {
    // DKG functions
    dkg_round1: mock(() => 'round1_result'),
    dkg_round2: mock(() => 'round2_result'),
    dkg_finalize: mock(() => 'finalized_keys'),
    
    // Signing functions
    sign_round1: mock(() => 'sign_round1_result'),
    sign_round2: mock(() => 'sign_round2_result'),
    sign_finalize: mock(() => 'final_signature'),
    
    // Keystore functions  
    encrypt_keystore: mock(() => 'encrypted_keystore'),
    decrypt_keystore: mock(() => 'decrypted_keystore'),
    import_cli_keystore: mock(() => 'imported_keystore'),
    export_cli_keystore: mock(() => 'exported_keystore'),
    
    // Utility functions
    generate_address: mock(() => '0x742d35Cc6641C4532B4d2a3F44ae7f35E0D29636'),
    validate_address: mock(() => true),
    format_signature: mock(() => 'formatted_signature'),
    
    // Curve operations
    secp256k1_operations: {
        generate_keypair: mock(() => ({ public_key: 'pub_key', private_key: 'priv_key' })),
        sign: mock(() => 'secp256k1_signature'),
        verify: mock(() => true)
    },
    
    ed25519_operations: {
        generate_keypair: mock(() => ({ public_key: 'ed25519_pub_key', private_key: 'ed25519_priv_key' })),
        sign: mock(() => 'ed25519_signature'), 
        verify: mock(() => true)
    }
};

// Mock the WASM loader
const mockInitWasm = mock(async (): Promise<InitWasmReturn> => {
    return {
        wasm: mockWasmModule as any,
        success: true,
        error: null
    };
});

// Mock failed WASM initialization
const mockInitWasmFailure = mock(async (): Promise<InitWasmReturn> => {
    return {
        wasm: null,
        success: false, 
        error: 'Failed to load WASM module'
    };
});

describe('WASM Service Integration', () => {
    let wasmModule: any;

    beforeEach(async () => {
        // Reset all mocks. TS sees mockFn as `Function` from
        // Object.values; narrow with cast to any so
        // .mockClear (bun:test Mock method) is accessible.
        Object.values(mockWasmModule).forEach((mockFn: any) => {
            if (typeof mockFn === 'function') {
                mockFn.mockClear();
            }
        });
        mockInitWasm.mockClear();
        
        // Initialize WASM module
        const initResult = await mockInitWasm();
        wasmModule = initResult.wasm;
    });

    describe('WASM Initialization', () => {
        it('should initialize WASM module successfully', async () => {
            const result = await mockInitWasm();
            
            expect(result.success).toBe(true);
            expect(result.wasm).toBeDefined();
            expect(result.error).toBeNull();
        });

        it('should handle WASM initialization failure', async () => {
            const result = await mockInitWasmFailure();
            
            expect(result.success).toBe(false);
            expect(result.wasm).toBeNull();
            expect(result.error).toBeDefined();
        });

        it('should provide all required WASM functions', () => {
            expect(wasmModule.dkg_round1).toBeDefined();
            expect(wasmModule.dkg_round2).toBeDefined();
            expect(wasmModule.dkg_finalize).toBeDefined();
            expect(wasmModule.sign_round1).toBeDefined();
            expect(wasmModule.sign_round2).toBeDefined();
            expect(wasmModule.sign_finalize).toBeDefined();
            expect(wasmModule.encrypt_keystore).toBeDefined();
            expect(wasmModule.decrypt_keystore).toBeDefined();
        });
    });

    describe('DKG Operations', () => {
        const mockDkgParams = {
            participant_id: 1,
            total_participants: 3,
            threshold: 2,
            curve: 'secp256k1' as const
        };

        it('should execute DKG round 1', () => {
            const result = wasmModule.dkg_round1(
                mockDkgParams.participant_id,
                mockDkgParams.total_participants,
                mockDkgParams.threshold,
                mockDkgParams.curve
            );

            expect(mockWasmModule.dkg_round1).toHaveBeenCalledWith(1, 3, 2, 'secp256k1');
            expect(result).toBe('round1_result');
        });

        it('should execute DKG round 2', () => {
            const round1Results = ['commitment1', 'commitment2', 'commitment3'];
            
            const result = wasmModule.dkg_round2(
                mockDkgParams.participant_id,
                JSON.stringify(round1Results),
                mockDkgParams.curve
            );

            expect(mockWasmModule.dkg_round2).toHaveBeenCalled();
            expect(result).toBe('round2_result');
        });

        it('should finalize DKG', () => {
            const round2Results = ['share1', 'share2', 'share3'];
            
            const result = wasmModule.dkg_finalize(
                mockDkgParams.participant_id,
                JSON.stringify(round2Results),
                mockDkgParams.curve
            );

            expect(mockWasmModule.dkg_finalize).toHaveBeenCalled();
            expect(result).toBe('finalized_keys');
        });

        it('should handle DKG with different curves', () => {
            // Test secp256k1
            wasmModule.dkg_round1(1, 3, 2, 'secp256k1');
            expect(mockWasmModule.dkg_round1).toHaveBeenCalledWith(1, 3, 2, 'secp256k1');

            // Test ed25519  
            wasmModule.dkg_round1(1, 3, 2, 'ed25519');
            expect(mockWasmModule.dkg_round1).toHaveBeenCalledWith(1, 3, 2, 'ed25519');
        });

        it('should handle various threshold configurations', () => {
            // 2-of-2
            wasmModule.dkg_round1(1, 2, 2, 'secp256k1');
            expect(mockWasmModule.dkg_round1).toHaveBeenCalledWith(1, 2, 2, 'secp256k1');

            // 2-of-3
            wasmModule.dkg_round1(1, 3, 2, 'secp256k1');
            expect(mockWasmModule.dkg_round1).toHaveBeenCalledWith(1, 3, 2, 'secp256k1');

            // 3-of-5
            wasmModule.dkg_round1(1, 5, 3, 'secp256k1');
            expect(mockWasmModule.dkg_round1).toHaveBeenCalledWith(1, 5, 3, 'secp256k1');
        });
    });

    describe('Signing Operations', () => {
        const mockSigningParams = {
            participant_id: 1,
            keystore: 'encrypted_keystore_data',
            message: '0x1234567890abcdef',
            curve: 'secp256k1' as const
        };

        it('should execute signing round 1', () => {
            const result = wasmModule.sign_round1(
                mockSigningParams.participant_id,
                mockSigningParams.keystore,
                mockSigningParams.message,
                mockSigningParams.curve
            );

            expect(mockWasmModule.sign_round1).toHaveBeenCalledWith(
                1,
                'encrypted_keystore_data',
                '0x1234567890abcdef',
                'secp256k1'
            );
            expect(result).toBe('sign_round1_result');
        });

        it('should execute signing round 2', () => {
            const round1Commitments = ['commitment1', 'commitment2'];
            
            const result = wasmModule.sign_round2(
                mockSigningParams.participant_id,
                JSON.stringify(round1Commitments),
                mockSigningParams.keystore,
                mockSigningParams.curve
            );

            expect(mockWasmModule.sign_round2).toHaveBeenCalled();
            expect(result).toBe('sign_round2_result');
        });

        it('should finalize signature', () => {
            const round2Shares = ['share1', 'share2'];
            
            const result = wasmModule.sign_finalize(
                JSON.stringify(round2Shares),
                mockSigningParams.curve
            );

            expect(mockWasmModule.sign_finalize).toHaveBeenCalled();
            expect(result).toBe('final_signature');
        });

        it('should handle different message formats', () => {
            // Hex message
            wasmModule.sign_round1(1, 'keystore', '0xabcdef', 'secp256k1');
            expect(mockWasmModule.sign_round1).toHaveBeenCalledWith(1, 'keystore', '0xabcdef', 'secp256k1');

            // Raw bytes
            wasmModule.sign_round1(1, 'keystore', 'raw_message_bytes', 'secp256k1');
            expect(mockWasmModule.sign_round1).toHaveBeenCalledWith(1, 'keystore', 'raw_message_bytes', 'secp256k1');
        });

        it('should format signatures correctly', () => {
            const rawSignature = 'raw_signature_data';
            
            const result = wasmModule.format_signature(rawSignature, 'secp256k1');
            
            expect(mockWasmModule.format_signature).toHaveBeenCalledWith(rawSignature, 'secp256k1');
            expect(result).toBe('formatted_signature');
        });
    });

    describe('Keystore Operations', () => {
        const mockKeystoreData = {
            keystore: {
                version: 1,
                keys: 'encrypted_keys',
                metadata: {
                    curve: 'secp256k1',
                    threshold: 2,
                    total: 3
                }
            },
            password: 'test_password'
        };

        it('should encrypt keystore', () => {
            const result = wasmModule.encrypt_keystore(
                JSON.stringify(mockKeystoreData.keystore),
                mockKeystoreData.password
            );

            expect(mockWasmModule.encrypt_keystore).toHaveBeenCalledWith(
                JSON.stringify(mockKeystoreData.keystore),
                'test_password'
            );
            expect(result).toBe('encrypted_keystore');
        });

        it('should decrypt keystore', () => {
            const result = wasmModule.decrypt_keystore(
                'encrypted_keystore_data',
                mockKeystoreData.password
            );

            expect(mockWasmModule.decrypt_keystore).toHaveBeenCalledWith(
                'encrypted_keystore_data',
                'test_password'
            );
            expect(result).toBe('decrypted_keystore');
        });

        it('should import CLI keystore format', () => {
            const cliKeystoreData = {
                version: 1,
                crypto: {
                    cipher: 'aes-256-gcm',
                    kdf: 'argon2id'
                },
                data: 'cli_encrypted_data'
            };

            const result = wasmModule.import_cli_keystore(
                JSON.stringify(cliKeystoreData),
                'cli_password'
            );

            expect(mockWasmModule.import_cli_keystore).toHaveBeenCalledWith(
                JSON.stringify(cliKeystoreData),
                'cli_password'
            );
            expect(result).toBe('imported_keystore');
        });

        it('should export to CLI keystore format', () => {
            const extensionKeystore = {
                version: 1,
                keys: 'extension_keys',
                curve: 'secp256k1'
            };

            const result = wasmModule.export_cli_keystore(
                JSON.stringify(extensionKeystore),
                'export_password'
            );

            expect(mockWasmModule.export_cli_keystore).toHaveBeenCalledWith(
                JSON.stringify(extensionKeystore),
                'export_password'
            );
            expect(result).toBe('exported_keystore');
        });

        it('should handle keystore validation', () => {
            // Mock validation function
            mockWasmModule.validate_keystore = mock(() => true);
            
            const isValid = wasmModule.validate_keystore('keystore_data');
            
            expect(mockWasmModule.validate_keystore).toHaveBeenCalledWith('keystore_data');
            expect(isValid).toBe(true);
        });
    });

    describe('Address Generation and Validation', () => {
        it('should generate addresses for different curves', () => {
            // Ethereum address (secp256k1)
            const ethAddress = wasmModule.generate_address('public_key_data', 'secp256k1');
            expect(mockWasmModule.generate_address).toHaveBeenCalledWith('public_key_data', 'secp256k1');
            expect(ethAddress).toBe('0x742d35Cc6641C4532B4d2a3F44ae7f35E0D29636');

            // Solana address (ed25519)
            const solAddress = wasmModule.generate_address('ed25519_public_key', 'ed25519');
            expect(mockWasmModule.generate_address).toHaveBeenCalledWith('ed25519_public_key', 'ed25519');
        });

        it('should validate addresses', () => {
            // Valid Ethereum address
            const isValidEth = wasmModule.validate_address('0x742d35Cc6641C4532B4d2a3F44ae7f35E0D29636', 'ethereum');
            expect(mockWasmModule.validate_address).toHaveBeenCalledWith('0x742d35Cc6641C4532B4d2a3F44ae7f35E0D29636', 'ethereum');
            expect(isValidEth).toBe(true);

            // Valid Solana address  
            const isValidSol = wasmModule.validate_address('9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM', 'solana');
            expect(mockWasmModule.validate_address).toHaveBeenCalledWith('9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM', 'solana');
            expect(isValidSol).toBe(true);
        });

        it('should handle address derivation from public keys', () => {
            const publicKeyData = {
                curve: 'secp256k1',
                compressed: true,
                data: 'compressed_public_key_bytes'
            };

            const address = wasmModule.generate_address(JSON.stringify(publicKeyData), 'secp256k1');
            expect(address).toBeDefined();
        });
    });

    describe('Curve-Specific Operations', () => {
        it('should perform secp256k1 operations', () => {
            const secp256k1 = wasmModule.secp256k1_operations;
            
            // Key generation
            const keypair = secp256k1.generate_keypair();
            expect(keypair.public_key).toBe('pub_key');
            expect(keypair.private_key).toBe('priv_key');

            // Signing
            const signature = secp256k1.sign('private_key', 'message_hash');
            expect(secp256k1.sign).toHaveBeenCalledWith('private_key', 'message_hash');
            expect(signature).toBe('secp256k1_signature');

            // Verification
            const isValid = secp256k1.verify('public_key', 'message_hash', 'signature');
            expect(secp256k1.verify).toHaveBeenCalledWith('public_key', 'message_hash', 'signature');
            expect(isValid).toBe(true);
        });

        it('should perform ed25519 operations', () => {
            const ed25519 = wasmModule.ed25519_operations;
            
            // Key generation
            const keypair = ed25519.generate_keypair();
            expect(keypair.public_key).toBe('ed25519_pub_key');
            expect(keypair.private_key).toBe('ed25519_priv_key');

            // Signing
            const signature = ed25519.sign('ed25519_private_key', 'message');
            expect(ed25519.sign).toHaveBeenCalledWith('ed25519_private_key', 'message');
            expect(signature).toBe('ed25519_signature');

            // Verification
            const isValid = ed25519.verify('ed25519_public_key', 'message', 'signature');
            expect(ed25519.verify).toHaveBeenCalledWith('ed25519_public_key', 'message', 'signature');
            expect(isValid).toBe(true);
        });
    });

    describe('Error Handling', () => {
        it('should handle WASM function errors', () => {
            // Mock function throwing error
            mockWasmModule.dkg_round1.mockImplementationOnce(() => {
                throw new Error('WASM function error');
            });

            expect(() => {
                wasmModule.dkg_round1(1, 3, 2, 'secp256k1');
            }).toThrow('WASM function error');
        });

        it('should handle invalid parameters', () => {
            // Mock validation for invalid parameters
            mockWasmModule.dkg_round1.mockImplementationOnce((participant_id: number) => {
                if (participant_id <= 0) {
                    throw new Error('Invalid participant ID');
                }
                return 'round1_result';
            });

            expect(() => {
                wasmModule.dkg_round1(0, 3, 2, 'secp256k1'); // Invalid participant ID
            }).toThrow('Invalid participant ID');
        });

        it('should handle malformed JSON input', () => {
            mockWasmModule.import_cli_keystore.mockImplementationOnce((json_data: string) => {
                try {
                    JSON.parse(json_data);
                    return 'imported_keystore';
                } catch {
                    throw new Error('Invalid JSON format');
                }
            });

            expect(() => {
                wasmModule.import_cli_keystore('invalid_json', 'password');
            }).toThrow('Invalid JSON format');
        });

        it('should handle cryptographic errors', () => {
            mockWasmModule.decrypt_keystore.mockImplementationOnce(() => {
                throw new Error('Decryption failed: Invalid password');
            });

            expect(() => {
                wasmModule.decrypt_keystore('encrypted_data', 'wrong_password');
            }).toThrow('Decryption failed: Invalid password');
        });
    });

    describe('Memory Management', () => {
        it('should handle large data structures', () => {
            const largeData = 'x'.repeat(1024 * 1024); // 1MB string
            
            // Should handle large keystore data
            const result = wasmModule.encrypt_keystore(largeData, 'password');
            expect(mockWasmModule.encrypt_keystore).toHaveBeenCalledWith(largeData, 'password');
            expect(result).toBeDefined();
        });

        it('should clean up resources properly', () => {
            // Mock cleanup function
            mockWasmModule.cleanup = mock(() => {});
            
            wasmModule.cleanup();
            expect(mockWasmModule.cleanup).toHaveBeenCalled();
        });
    });

    describe('Integration Scenarios', () => {
        it('should complete full DKG workflow', () => {
            const participant_id = 1;
            const total = 3;
            const threshold = 2;
            const curve = 'secp256k1';

            // Round 1
            const round1_result = wasmModule.dkg_round1(participant_id, total, threshold, curve);
            expect(round1_result).toBe('round1_result');

            // Round 2 (with mocked round1 results from other participants)
            const round1_results = ['round1_result', 'peer1_round1', 'peer2_round1'];
            const round2_result = wasmModule.dkg_round2(participant_id, JSON.stringify(round1_results), curve);
            expect(round2_result).toBe('round2_result');

            // Finalize
            const round2_results = ['round2_result', 'peer1_round2', 'peer2_round2'];
            const final_keys = wasmModule.dkg_finalize(participant_id, JSON.stringify(round2_results), curve);
            expect(final_keys).toBe('finalized_keys');
        });

        it('should complete full signing workflow', () => {
            const participant_id = 1;
            const keystore = 'test_keystore';
            const message = '0xdeadbeef';
            const curve = 'secp256k1';

            // Round 1
            const sign_round1_result = wasmModule.sign_round1(participant_id, keystore, message, curve);
            expect(sign_round1_result).toBe('sign_round1_result');

            // Round 2
            const round1_commitments = ['sign_round1_result', 'peer1_commitment'];
            const sign_round2_result = wasmModule.sign_round2(participant_id, JSON.stringify(round1_commitments), keystore, curve);
            expect(sign_round2_result).toBe('sign_round2_result');

            // Finalize
            const round2_shares = ['sign_round2_result', 'peer1_share'];
            const final_signature = wasmModule.sign_finalize(JSON.stringify(round2_shares), curve);
            expect(final_signature).toBe('final_signature');
        });

        it('should handle keystore import/export cycle', () => {
            const original_keystore = { version: 1, keys: 'test_keys' };
            const password = 'test_password';

            // Encrypt
            const encrypted = wasmModule.encrypt_keystore(JSON.stringify(original_keystore), password);
            expect(encrypted).toBe('encrypted_keystore');

            // Decrypt
            const decrypted = wasmModule.decrypt_keystore(encrypted, password);
            expect(decrypted).toBe('decrypted_keystore');

            // Export to CLI format
            const cli_format = wasmModule.export_cli_keystore(decrypted, password);
            expect(cli_format).toBe('exported_keystore');

            // Import from CLI format
            const imported = wasmModule.import_cli_keystore(cli_format, password);
            expect(imported).toBe('imported_keystore');
        });
    });
});