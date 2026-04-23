# Scripts Directory

Utility scripts for development, testing, and building the MPC Wallet extension.

## Structure

### `/test`
- `run-all-tests.sh` — run all test suites
- `run-tests.sh` — run specific tests

### `/build`
- `fix-all-syntax-errors.sh` — fix syntax errors in source files
- `fix-bun-imports.js` — fix import statements for Bun compatibility
- `remove-debug-logs.sh` — strip debug logging

### `/` (top-level)
- `gen-frost-fixtures.ts` — generate FROST test fixtures (DKG round 1/2 packages, signing shares) used by the Vitest / bun-test suites
- `test-dkg-ui.sh` — headless UI smoke-test for the DKG flow

## Usage

### Running tests
```bash
./scripts/test/run-all-tests.sh
```

### Regenerate FROST fixtures
```bash
bun run scripts/gen-frost-fixtures.ts
```

### Build fixes
```bash
./scripts/build/fix-all-syntax-errors.sh
```
