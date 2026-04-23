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
bun test                           # all tests
bun test --watch                   # watch mode (built-in flag)
bun run test:watch                 # same, via package.json script
bun run test:unit                  # services + config only
bun run test:integration           # integration tests
bun run test:webrtc                # offscreen webrtc.*.test.ts
bun run test:coverage              # with coverage
bun test tests/services/walletClient.test.ts  # a specific file
```

Note: `test:unit` / `test:integration` / `test:webrtc` /
`test:coverage` are scripts defined in the extension's
`package.json` — invoke with `bun run <script>`, not
`bun <script>`. Plain `bun test` (no `run`) calls the test
runner directly, which is why `--watch` also works as a flag
on `bun test` itself.

## Test Runner

- **Bun**: Used for all tests with native WebAssembly support
- Tests are preloaded with `setup-bun.ts` which provides Chrome API mocks and crypto mocks

## Writing Tests

1. Place test files next to the code they test with `.test.ts` extension
2. Use descriptive test names
3. Follow the existing test patterns
4. Mock external dependencies appropriately
5. Ensure tests are deterministic and don't depend on external state