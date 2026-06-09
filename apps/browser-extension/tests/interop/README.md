# L3c interop harness — extension ↔ CLI conformance (#33)

The highest-value, highest-cost test layer: it crosses the **Rust-core ↔
independent-TypeScript/WASM** boundary. The extension acts as the DKG initiator;
`starlab-cli serve` nodes act as the other co-signers. We assert the
cross-implementation invariant from `docs/cli-conformance-testing.md` §5.4:
**all participants agree on the group public key**, and a threshold signature
produced with the extension in the mesh **verifies**.

```
   extension (TS + WASM)  ──┐
                            ├── signal server (room-scoped) ── WebRTC mesh ── DKG + signing
   2× starlab-cli serve ─┘            (Rust core)
```

## Files

| File | Role |
|---|---|
| `fixtures.ts` | Loads the built MV3 extension into a headed, persistent Chromium and resolves its extension id. |
| `cli-peers.ts` | Spawns N `starlab-cli serve` co-signers via `scripts/demo/serve_autojoin.py`; parses their `RESULT … dkg_complete/signature_complete` lines. |
| `extension-actions.ts` | All popup UI steps in one place (room config is data-testid-stable; create/sign are first-run-verify). |
| `boundary.pw.ts` | **Runnable smoke** — loads the extension, saves a strong room. No CLI/server needed. |
| `dkg-signing.pw.ts` | **Full interop** — ext + 2 CLI DKG + signing; group-key cross-assert. |

> Specs use the `.pw.ts` suffix (not `.spec.ts`/`.test.ts`) so Bun's unit test
> runner ignores them; Playwright collects them via `testMatch: **/*.pw.ts`.

## Prerequisites

```bash
# from repo root
cargo build --release -p starlab-cli        # the CLI peers
cd apps/browser-extension
bun run build                                  # produces .output/chrome-mv3
bunx playwright install chromium               # one-time browser download
```

## Run

```bash
# Smoke only (no signal server / CLI): proves the harness + extension load.
bun run test:interop:smoke

# Full interop against the deployed worker (default) or a local server:
INTEROP_SIGNAL=wss://panda.qzz.io bun run test:interop
INTEROP_SIGNAL=ws://127.0.0.1:8787 bun run test:interop   # local wrangler dev
```

Env knobs: `INTEROP_SIGNAL`, `INTEROP_CURVE` (secp256k1|ed25519), `INTEROP_PW`,
`PLAYWRIGHT_CHROME_PATH`.

## Environment requirements (verified)

This layer needs **headed Chrome with a real X display** — there is no
display-less path:

- **Headed is mandatory.** Chromium will not load an unpacked MV3 extension's
  background **service worker** in `--headless=new` (verified empty service-worker
  set after 45 s on Chrome 148, no error). So a display-less runner must wrap the
  run in a virtual framebuffer: `xvfb-run -a bun run test:interop`.
- **System Chrome on Nix.** Playwright's bundled Chromium fails to start under the
  Nix dev shell (`error while loading shared libraries: libglib-2.0.so.0`). Point
  the harness at the Nix-provided browser instead:
  `PLAYWRIGHT_CHROME_PATH=$(which google-chrome-stable)` — `fixtures.ts` honours it.

So a complete display-less invocation is:

```bash
PLAYWRIGHT_CHROME_PATH=$(which google-chrome-stable) \
  xvfb-run -a bun run test:interop:smoke
```

## First-run selector pass

`extension-actions.ts` marks the create-wallet / sign steps `FIRST-RUN`: run
once headed (`bun run test:interop --headed`), confirm the CreateWalletForm /
sign selectors against the live popup, and tighten them in that one file. The
room-config path (`room-input` / `room-status` data-testids) is already stable.

## Status

This is the **scheduled / pre-release** layer (needs headed Chrome + a running
server), per the phased plan in `docs/cli-conformance-testing.md` §10 (Phase 5).
The boundary smoke is CI-able on any runner with an X display (or `xvfb-run`);
the full flow runs nightly / before release. It does **not** run in headless-only
sandboxes — see "Environment requirements" above.
