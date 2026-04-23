# Test Structure

This directory contains all tests for the MPC Wallet browser extension.

## Organization

```
tests/
├── __mocks__/        # Module-level mocks (WXT #imports, etc.)
├── components/       # UI component tests
├── config/           # Configuration tests
├── entrypoints/      # Extension entrypoint tests
│   ├── background/   # Background service worker tests
│   └── offscreen/    # Offscreen document tests (WebRTC, FROST)
├── integration/      # Integration tests
├── services/         # Service layer tests
├── utils/            # Utility tests
├── setup-bun.ts      # Preload: Chrome + crypto mocks, exports
│                     # REAL_WEBCRYPTO for roundtrip tests
└── wxt-imports-mock.ts
```

## Running Tests

```bash
bun test                    # all tests
bun test --watch            # watch mode
bun test:unit               # services + components + config only
bun test:integration        # integration tests
bun test:webrtc             # offscreen webrtc.*.test.ts
bun test:coverage           # with coverage
```

## Test Runner

- **Bun**: Used for all tests with native WebAssembly support
- Tests are preloaded with `setup-bun.ts` which provides Chrome API mocks and crypto mocks

## Writing Tests

1. Place test files next to the code they test with `.test.ts` extension
2. Use descriptive test names
3. Follow the existing test patterns
4. Mock external dependencies appropriately
5. Ensure tests are deterministic and don't depend on external state