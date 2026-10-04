# Finish the threshold-ECDSA implementation

Goal: finish the five stages in IMPLEMENTATION_PLAN.md across starlab-mpc,
starlab-desktop and starlab-wallet, using the existing feature work.

Scope: integrate the engine/TUI/WASM branches; finish extension ECDSA DKG,
signing, encrypted background prime generation and Sepolia Send; verify
extension/CLI 2-of-3 and 3-of-3 interoperability; run a TUI/desktop/extension
2-of-3 ceremony and confirm a real Sepolia transaction from the MPC address.
Address completed-session replay found in the Stage 3 live run. Preserve the
public desktop and WASM contracts and document ECDSA recovery limitations.

Assumptions: use existing feature branches and UI language (English); no
migration layer; fixture primes are for tests only. Live transaction wallets
must use freshly generated primes. No unrelated TUI feature work.

Acceptance evidence:
- Native workspace tests, format, lint and ignored network suites pass.
- WASM and shared TypeScript types build, real-WASM ECDSA tests pass.
- Extension type check and unit tests pass; CLI/extension interop passes for
  ed25519 and secp256k1, including 2-of-3, 3-of-3 and EIP-1559 sender recovery.
- Desktop tests, lint and release build pass against the integrated engine.
- Screenshots show the three real clients in the wallet/signing flow; a
  successful Sepolia receipt proves the transaction sender is the MPC address.
- PRs and implementation documentation reflect verified final state.

Risk: safe-prime generation and interactive browser cryptography are slow;
Sepolia completion needs test ETH and a working RPC. Preserve logs and report
any unverified gate rather than substituting simulated-chain evidence.

## Acceptance evidence (in progress)

- Extension Send regression: real CLI peers + production signing path build,
  EIP-1559 signed sender recovery equals wallet address; 1 passed in 1.8 min.
  Log: /tmp/starlab-send-interop.log.
- Extension real-WASM/ECDSA + approval regression subset: 39 passed, 0 failed,
  129 assertions (470.70 s); /tmp/starlab-extension-unit.log.
- Remaining extension unit tests: 909 passed, 0 failed, 2450 assertions;
  /tmp/starlab-extension-all-unit.log. ECDSA files excluded here because the
  previous gate exercised them separately.
- Extension type check: 0 errors and 0 warnings after lifecycle changes and
  removal of obsolete EVM caveat; /tmp/starlab-extension-check-final.log.
- Rust workspace snapshot before reconnect follow-up: 509 passed, 0 failed,
  31 ignored (26 test/doc-test suites); /tmp/starlab-engine-tests.log.
  Native client follow-up: 274 passed; ignored conformance 9 passed, L3 serve
  8 passed, wire protocol 2 passed. Full ignored e2e had 10 passes and one
  FROST timeout fixture mismatch; corrected fixture passed its targeted retry
  gate without increasing the 3-second timeout. Final workspace rerun:
  513 passed, 0 failed, 31 ignored (26 suites), final-workspace-tests.log.
- ed25519 browser interoperability: 8 passed, 5 conditionally skipped,
  2.3 min; /tmp/starlab-full-ed-interop.log.
- secp256k1 browser interoperability: full process terminated (exit 143)
  before a final summary. Seven completed cases passed; the 2-of-3 account
  case teardown failed because concurrent Playwright jobs shared an output
  directory. Both account cases and remaining Send/signing/persistence cases
  are rerunning with distinct output directories and the final engine build.
  This gate has NOT passed.
- Desktop integrated-engine tests: 47 passed, ignored real 3-device
  DKG/sign/export/import 1 passed (34 s); strict integrated clippy passed.
  Final fmt, strict clippy and release pass: recheck.log.
  Follow-up draft PR: stars-labs/starlab-desktop#24.
- Production extension build succeeds; fixture injection hook absent.
  /tmp/starlab-production-build.log and /tmp/starlab-stage5/extension-build.
- Stage 5 TUI + desktop + production extension completed a real-prime
  2-of-3 wallet ceremony; all show Ethereum account 0
  `0x2fb6ae33558e46ec6baa39c9c98cd00f4b0f7548`.
  Screenshots: /tmp/starlab-stage5/shots and desktop live-shots.
  Sepolia balance remains zero; faucet returned INVALID_CAPTCHA.
  Live extension revealed a popup public-key snapshot defect: header address
  correct, hero falls back to Bitcoin. No-share error was caused by auto-lock;
  after UI unlock, genuine extension + desktop EIP-191 signing completed and
  the invitation was withdrawn. Popup lock/signature feedback repairs and a
  successful real transaction receipt remain required.
  This gate has NOT passed.

- Real-client message signature public evidence:
  /tmp/starlab-stage5/live-signature-evidence.json; independent viem recovery
  matches the wallet address. Desktop Foundry verification against both the
  original EIP-191 text and raw digest passes: cast-signature-verify.log.
  Screenshot: live-shots/14-desktop-signed.png.

- Final functional extension unit suite: 938 passed, 0 failed, 107 files;
  /tmp/starlab-retirement-fullsuite.log. WebSocket test harness constants were
  corrected after the full CI suite exposed undefined OPEN/CLOSED values.
- Account browser cases: 2-of-3 and 3-of-3 both passed (5.2 min),
  /tmp/starlab-secp-final-cosign.log. Send/signing/persistence: 5 passed,
  1 namespace-only case skipped (8.8 min), /tmp/starlab-secp-final-rest.log.
  Namespace-only WebRTC outage separately passed for both curves:
  /tmp/starlab-{secp,ed}-netns.log.
- Final ignored native e2e gate: 11 passed, 0 failed, 687.61 seconds on
  05f219e. /tmp/starlab-retry-budget-final-e2e.log. The fixture gives only
  the abandoned ceremony 3 seconds and restores the production timeout for
  the fresh retry; it no longer mutates process environment variables. This
  supersedes the earlier short-timeout failures under concurrent load.
- Added user scope: designer participates; token icons and every implemented
  feature must meet the competitive visual acceptance matrix in
  2026-10-04-wallet-visual-design.md. Implementation and screenshot review
  are in progress across extension and desktop.

- Live popup completion regression verified after clearing stale browser code
  caches: signature banner shows actual Ethereum address verification, progress
  ends, public signature independently recovers to the wallet address.
  /tmp/starlab-stage5/live-popup-signature-evidence.json;
  shots/19-live-verified-ecdsa-result.png.
- Native timeout fixture now has no process environment mutation: only the
  abandoned proposer ceremony uses 3 seconds, retry uses the existing
  production budget. Final serial full e2e gate passed on 05f219e; prior
  parallel-run environment contamination (1 pass, 10 fails) is recorded and
  superseded by the source fix, not counted as a passing gate.
- User confirmed Rabby, MetaMask and Phantom as the visual benchmark.

- Backup audit found and fixed selected Ethereum metadata becoming Bitcoin
  after multi-file restore. Real restore, ECDSA sign, lock and fresh-service
  unlock regression passes with persisted Ethereum address and chain.
  /tmp/starlab-restore-audit-final.log: 19 passed, 67 assertions.
- Recovery documentation and refresh UI explicitly distinguish FROST share
  refresh (Bitcoin/Solana/Sui) from Ethereum ECDSA keys, which remain unchanged.
