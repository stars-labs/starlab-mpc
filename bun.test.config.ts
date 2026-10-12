// bun.test.config.ts
export default {
  timeout: 30000, // Increase timeout to 30 seconds for all tests
  coverage: {
    exclude: [
      // Exclude auto-generated WASM bindings with multiple patterns
      'pkg/**',
      'pkg/**/*',
      'pkg/mpc_wallet.js',
      'pkg/mpc_wallet_bg.js',
      // Exclude Rust build artifacts
      'target/**',
      'target/**/*',
      // Exclude test files and test utilities  
      'tests/**',
      '**/test-utils.ts',
      'src/**/test-utils.ts',
      'src/entrypoints/offscreen/test-utils.ts',
      // Standard exclusions
      'node_modules/**',
      '**/*.test.ts',
      '**/*.test.js',
    ],
  },
  // Add any other test configuration here
};
