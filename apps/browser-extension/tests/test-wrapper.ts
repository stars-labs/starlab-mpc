// Test wrapper that provides mocks for #imports
import { jest, mock } from 'bun:test';

// Create module mock
const importsMock = {
  browser: (global as any).chrome,
  storage: {
    defineItem: jest.fn((name: string) => ({
      getValue: jest.fn().mockResolvedValue(null),
      setValue: jest.fn().mockResolvedValue(undefined),
      removeValue: jest.fn().mockResolvedValue(undefined),
      watch: jest.fn()
    }))
  }
};

// Use bun:test's mock.module (the `Bun.mock.module` path doesn't
// exist on the global Bun namespace — correct import is from
// 'bun:test').
mock.module('#imports', () => importsMock);

// Export for use in tests
export { importsMock };