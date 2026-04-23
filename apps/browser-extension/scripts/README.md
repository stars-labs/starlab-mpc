# Scripts Directory

Utility scripts for development, testing, and building the MPC Wallet extension.

## Structure

### `/build`
- `remove-debug-logs.sh` — ad-hoc one-shot that comments out
  decorative `console.log` calls in a curated list of files
  (preserves `console.error` + audit messages). Creates a
  `src.backup.<timestamp>` first. Not wired into CI — invoke
  manually when the logger-noise review is due.

### `/` (top-level)
- `gen-frost-fixtures.ts` — generate FROST test fixtures (real 2-of-3 DKG round 1/2 packages, signing shares) used by the bun-test suites under `../test-data/real-*`. Re-run whenever the on-disk keystore schema in `packages/@mpc-wallet/frost-core` changes.
- `test-dkg-ui.sh` — headless UI smoke-test for the DKG flow.

## Usage

### Running tests
`bun test` at the browser-extension root runs all 509 tests
(preload + module resolution come from `bunfig.toml`). Sub-suites
are scriptable via the `test:*` entries in `package.json` (e.g.
`bun run test:webrtc`, `bun run test:integration`).

### Regenerate FROST fixtures
```bash
bun run scripts/gen-frost-fixtures.ts
```

### Strip decorative debug logs
```bash
./scripts/build/remove-debug-logs.sh
```
