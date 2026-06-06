# L3c interop harness — extension ↔ CLI conformance (#33)

The highest-value, highest-cost test layer: it crosses the **Rust-core ↔
independent-TypeScript/WASM** boundary. The extension acts as the DKG initiator;
`mpc-wallet-cli serve` nodes act as the other co-signers. We assert the
cross-implementation invariant from `docs/cli-conformance-testing.md` §5.4:
**all participants agree on the group public key**, and a threshold signature
produced with the extension in the mesh **verifies**.

```
   extension (TS + WASM)  ──┐
                            ├── signal server (room-scoped) ── WebRTC mesh ── DKG + signing
   2× mpc-wallet-cli serve ─┘            (Rust core)
```

## Files

| File | Role |
|---|---|
| `fixtures.ts` | Loads the built MV3 extension into a headed, persistent Chromium and resolves its extension id. |
| `cli-peers.ts` | Spawns N `mpc-wallet-cli serve` co-signers via `scripts/demo/serve_autojoin.py`; parses their `RESULT … dkg_complete/signature_complete` lines. |
| `extension-actions.ts` | All popup UI steps in one place (room config is data-testid-stable; create/sign are first-run-verify). |
| `boundary.pw.ts` | **Runnable smoke** — loads the extension, saves a strong room. No CLI/server needed. |
| `dkg-signing.pw.ts` | **Full interop** — ext + 2 CLI DKG + signing; group-key cross-assert. |

> Specs use the `.pw.ts` suffix (not `.spec.ts`/`.test.ts`) so Bun's unit test
> runner ignores them; Playwright collects them via `testMatch: **/*.pw.ts`.

## Prerequisites

```bash
# from repo root
cargo build --release -p mpc-wallet-cli        # the CLI peers
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

Env knobs: `INTEROP_SIGNAL`, `INTEROP_CURVE` (secp256k1|ed25519), `INTEROP_PW`.

## First-run selector pass

`extension-actions.ts` marks the create-wallet / sign steps `FIRST-RUN`: run
once headed (`bun run test:interop --headed`), confirm the CreateWalletForm /
sign selectors against the live popup, and tighten them in that one file. The
room-config path (`room-input` / `room-status` data-testids) is already stable.

## Status

This is the **scheduled / pre-release** layer (needs a browser + a running
server), per the phased plan in `docs/cli-conformance-testing.md` §10 (Phase 5).
The boundary smoke is CI-able on any headed runner; the full flow runs nightly /
before release.
