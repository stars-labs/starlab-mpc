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
  gate without increasing the 3-second timeout. Final workspace rerun pending.
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
  Live extension also revealed saved-key/address restoration defects:
  header address correct, hero falls back to Bitcoin, Sign reports no share.
  Repairs and a successful real transaction receipt remain required.
  This gate has NOT passed.
