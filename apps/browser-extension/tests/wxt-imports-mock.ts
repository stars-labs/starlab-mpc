// Module mock for #imports used in service files during testing
import { jest } from 'bun:test';

// Use the same storage isolation approach as the other mock
let getStorageData: () => Record<string, any> = () => ({});

export const resetWxtStorageData = (dataGetter?: () => Record<string, any>) => {
  if (dataGetter) {
    getStorageData = dataGetter;
  } else {
    const freshData: Record<string, any> = {};
    getStorageData = () => freshData;
  }
};

// Initialize with empty storage
resetWxtStorageData();

export const storage = {
  defineItem: jest.fn((name: string) => ({
    getValue: jest.fn().mockImplementation(async () => getStorageData()[name] || null),
    setValue: jest.fn().mockImplementation(async (value: any) => {
      getStorageData()[name] = value;
    }),
    removeValue: jest.fn().mockImplementation(async () => {
      delete getStorageData()[name];
    }),
    watch: jest.fn()
  })),
  getItem: jest.fn().mockImplementation(async (name: string) => getStorageData()[name] || null),
  setItem: jest.fn().mockImplementation(async (name: string, value: any) => {
    getStorageData()[name] = value;
  }),
  removeItem: jest.fn().mockImplementation(async (name: string) => {
    delete getStorageData()[name];
  }),
  clear: jest.fn().mockImplementation(async () => {
    const data = getStorageData();
    Object.keys(data).forEach(key => delete data[key]);
  })
};

export const browser = (global as any).chrome || {
  runtime: {
    id: 'test-extension-id',
    sendMessage: jest.fn(),
    onMessage: {
      addListener: jest.fn(),
      removeListener: jest.fn()
    }
  },
  storage: {
    local: {
      get: jest.fn(),
      set: jest.fn(),
      remove: jest.fn()
    }
  }
};

/**
 * WXT's `defineBackground` wraps the service worker setup function.
 * At module load (not call) time, `background/index.ts` calls
 * `defineBackground(async () => {...})` as its default export. In
 * tests, any transitive import chain that touches background/index.ts
 * needs this symbol — otherwise bun's module evaluator fails with
 * "Export named 'defineBackground' not found in module '#imports'".
 *
 * Mock implementation: identity-ish — return the setup function
 * back so `export default defineBackground(...)` becomes
 * `export default asyncSetupFn`. Safe because tests don't actually
 * run the background; they import individual classes from it.
 */
export const defineBackground = jest.fn(
  (main: (() => void | Promise<void>) | { main: () => void | Promise<void> }) => {
    if (typeof main === 'function') return main;
    return main.main;
  },
);

export default {
  browser,
  storage,
  defineBackground,
};