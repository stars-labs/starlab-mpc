# Testing Guide

> **The browser-extension test suite moved out of this repo.** It now
> lives in `stars-labs/starlab-wallet` (at `apps/extension/`). Run and
> contribute those `bun:test` suites **in that repo** — the
> `apps/browser-extension/...` paths and the `bun run test:extension`
> script below no longer exist here. The rest of this guide is retained
> as a reference for the extension suite (it documents how the suite is
> structured in starlab-wallet); the Rust-side guidance immediately below
> stays valid for this repo.

**This repo (Rust engine + terminal clients):** run `cargo test --workspace`,
plus [`docs/testing/RUN_TEST_INSTRUCTIONS.md`](./RUN_TEST_INSTRUCTIONS.md)
for the 3-node manual mesh smoke test. (Earlier drafts of this
line pointed at `apps/tui/docs/RUN_TEST_INSTRUCTIONS.md` —
that path doesn't exist; the file lives at the workspace `docs/`
level, not under the TUI crate.)

---

The remainder of this guide describes the **extension test suite in
`stars-labs/starlab-wallet`**. All paths below are relative to that repo's
`apps/extension/` tree.

## Where tests live

The extension test tree is rooted at
`apps/extension/tests/` and has colocated `.test.ts` files
next to the modules they exercise (e.g.
`src/services/walletClient.test.ts`).

```
apps/extension/            # in stars-labs/starlab-wallet
├── tests/
│   ├── config/            # Configuration tests
│   ├── entrypoints/
│   │   ├── background/    # Service-worker regression suites
│   │   └── offscreen/     # WebRTC + FROST / WASM tests
│   ├── integration/       # Cross-component integration
│   ├── services/          # Service-layer unit tests
│   ├── utils/             # Test helpers
│   ├── __mocks__/         # Manual mocks
│   ├── setup-bun.ts       # Bun test setup + global mocks
│   ├── wxt-imports-mock.ts
│   └── README.md          # Test-tree doc
└── src/**/*.test.ts       # Colocated unit tests
```

## Test runner

The extension uses **Bun's built-in test runner**, not Vitest, not Jest.
Test files import from `bun:test`:

```ts
import { describe, expect, test, beforeEach } from "bun:test";
```

Coverage configuration is in `bunfig.toml` at the extension root;
see [COVERAGE.md](COVERAGE.md) for the caveats about Bun's
coverage-exclusion limitations.

## Running tests

From the starlab-wallet repo root:

```bash
bun run test              # all extension tests
```

From inside `apps/extension/` (in starlab-wallet):

```bash
bun test                                   # full suite
bun test tests/services/walletClient.test.ts
bun run test:watch                         # watch mode
bun run test:coverage                      # coverage report
bun run test:unit                          # tests/services + tests/config
bun run test:integration                   # tests/integration
bun run test:webrtc                        # tests/entrypoints/offscreen/webrtc.*.test.ts
```

No `test:e2e` or `test:ui` script exists — earlier drafts of this
doc mentioned them.

## Writing tests

1. Place test files under `apps/extension/tests/` (or colocate
   next to the module as `<name>.test.ts`).
2. Import from `bun:test`, not `vitest` or `@jest/globals`.
3. Use existing mock patterns from `tests/__mocks__/` and
   `tests/setup-bun.ts`.
4. For WebRTC / WASM-touching tests, mirror the patterns in
   `src/entrypoints/offscreen/webrtc.test.ts` and
   `tests/entrypoints/offscreen/`.
5. For signing / DKG regression suites, see
   `tests/entrypoints/background/` — existing suites cover
   `dkgAutoTrigger`, `signingAutoTrigger`, `signingNotification`,
   `dappSignatureApproval`, and `signingDecline`.

## Svelte type checking

Separate from tests. Run from inside the extension directory:

```bash
cd apps/extension && bun run check    # in starlab-wallet
```

## Live signal-server smoke tests

Real WebRTC/native/browser harnesses are implemented. See
[END_TO_END.md](END_TO_END.md) for their coverage, execution commands, and
production-prime evidence boundaries.
