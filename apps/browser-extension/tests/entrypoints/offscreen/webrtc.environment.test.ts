// Environment difference test for FROST DKG in production vs test
import { DkgState, WebRTCManager } from '../../../src/entrypoints/offscreen/webrtc';
import wasmInit, { FrostDkgEd25519, FrostDkgSecp256k1 } from '@mpc-wallet/core-wasm';
let originalConsoleLog: any;
let originalConsoleError: any;
let originalConsoleWarn: any;

describe('FROST DKG Environment', () => {
  let wasmInitialized = false;

  beforeAll(async () => {
    // Store original console methods
    originalConsoleLog = console.log;
    originalConsoleError = console.error;
    originalConsoleWarn = console.warn;
    
    if (!wasmInitialized) {
      try {
        await wasmInit();
        console.log('✅ WASM initialized successfully for environment test');
        wasmInitialized = true;
      } catch (error) {
        console.warn('⚠️ WASM initialization failed:', error);
      }
    }
    
    // Suppress console output after initialization
    console.log = jest.fn();
    console.error = jest.fn();
    console.warn = jest.fn();
  });
  
  afterAll(() => {
    // Restore console methods
    console.log = originalConsoleLog;
    console.error = originalConsoleError;
    console.warn = originalConsoleWarn;
  });

  test('should analyze FROST DKG initialization differences', async () => {
    // Temporarily restore console.log for this test
    const tempLog = console.log;
    console.log = originalConsoleLog;
    
    console.log('=== ENVIRONMENT ANALYSIS ===');
    
    // Test 1: Direct WASM module access (like in tests)
    console.log('1. Direct WASM module access:');
    console.log('   FrostDkgEd25519 available:', typeof FrostDkgEd25519);
    console.log('   FrostDkgSecp256k1 available:', typeof FrostDkgSecp256k1);
    
    // Test 2: Global/window access (like in production)
    console.log('2. Global/window access:');
    console.log('   (global as any).FrostDkgEd25519:', typeof (global as any).FrostDkgEd25519);
    console.log('   (global as any).FrostDkgSecp256k1:', typeof (global as any).FrostDkgSecp256k1);
    console.log('   (globalThis as any).FrostDkgEd25519:', typeof (globalThis as any).FrostDkgEd25519);
    console.log('   (globalThis as any).FrostDkgSecp256k1:', typeof (globalThis as any).FrostDkgSecp256k1);
    
    // Test 3: Create instances using both methods
    let directInstance = null;
    let globalInstance = null;
    
    try {
      directInstance = new FrostDkgSecp256k1();
      console.log('3a. Direct instance creation: SUCCESS');
      console.log('    Instance type:', directInstance.constructor.name);
      console.log('    Has add_round1_package:', typeof directInstance.add_round1_package);
    } catch (error) {
      console.log('3a. Direct instance creation: FAILED', error);
    }
    
    try {
      const GlobalClass = (global as any).FrostDkgSecp256k1 || (globalThis as any).FrostDkgSecp256k1;
      if (GlobalClass) {
        globalInstance = new GlobalClass();
        console.log('3b. Global instance creation: SUCCESS');
        console.log('    Instance type:', globalInstance.constructor.name);
        console.log('    Has add_round1_package:', typeof globalInstance.add_round1_package);
      } else {
        console.log('3b. Global instance creation: NO GLOBAL CLASS AVAILABLE');
      }
    } catch (error) {
      console.log('3b. Global instance creation: FAILED', error);
    }
    
    // Test 4: Compare instance behavior
    if (directInstance && globalInstance) {
      console.log('4. Instance comparison:');
      console.log('   Direct === Global:', directInstance.constructor === globalInstance.constructor);
      console.log('   Direct prototype === Global prototype:', 
        Object.getPrototypeOf(directInstance) === Object.getPrototypeOf(globalInstance));
    }
    
    // Test 5: FROST DKG initialization pattern (like WebRTCManager does)
    console.log('5. WebRTCManager FROST DKG pattern simulation:');
    try {
      const FrostDkgEd25519_fromGlobal = typeof global !== 'undefined' && (global as any).FrostDkgEd25519 ?
        (global as any).FrostDkgEd25519 :
        (typeof window !== 'undefined' && (window as any).FrostDkgEd25519 ? (window as any).FrostDkgEd25519 : null);

      const FrostDkgSecp256k1_fromGlobal = typeof global !== 'undefined' && (global as any).FrostDkgSecp256k1 ?
        (global as any).FrostDkgSecp256k1 :
        (typeof window !== 'undefined' && (window as any).FrostDkgSecp256k1 ? (window as any).FrostDkgSecp256k1 : null);

      console.log('   FrostDkgEd25519 from global check:', FrostDkgEd25519_fromGlobal ? 'FOUND' : 'NOT FOUND');
      console.log('   FrostDkgSecp256k1 from global check:', FrostDkgSecp256k1_fromGlobal ? 'FOUND' : 'NOT FOUND');
      
      if (!FrostDkgEd25519_fromGlobal || !FrostDkgSecp256k1_fromGlobal) {
        console.log('   🚨 THIS WOULD CAUSE PRODUCTION FAILURE: FROST DKG WebAssembly modules not found');
      } else {
        const testInstance = new FrostDkgSecp256k1_fromGlobal();
        console.log('   ✅ Production pattern SUCCESS - instance created');
        console.log('   Instance has add_round1_package:', typeof testInstance.add_round1_package);
      }
    } catch (error) {
      console.log('   🚨 Production pattern FAILED:', error);
    }
    
    console.log('=== ANALYSIS COMPLETE ===');
    
    // Restore console mock
    console.log = tempLog;
  });

  test('should test real FROST DKG package processing', async () => {
    // Create a minimal WebRTCManager to test actual DKG processing
    const manager = new WebRTCManager('test-peer');
    
    // Set up session info
    manager.sessionInfo = {
      session_id: 'test-session',
      participants: ['test-peer', 'other-peer'],
      accepted_devices: ['test-peer', 'other-peer'],
      status: 'accepted' as any
    };
    
    // Initialize DKG with Ethereum (secp256k1) like in production
    const success = await manager.initializeDkg('ethereum', 2, ['test-peer', 'other-peer'], 1);
    console.log('Real DKG initialization success:', success);
    
    if (success) {
      // Try to process a real package similar to production
      const mockPackage = {
        sender_index: 2,
        data: "507b2268656164657222..."  // This would be a real package hex
      };
      
      try {
        // This should trigger the same code path as production
        await (manager as any)._handleDkgRound1Package('other-peer', mockPackage);
        console.log('✅ Real package processing: SUCCESS');
      } catch (error) {
        console.log('🚨 Real package processing: FAILED', error);
      }
    }
  });
});
