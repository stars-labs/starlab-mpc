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

## Historical acceptance evidence (4 October 2026)

The revisions and live transaction below describe the original cryptographic
acceptance. [Product closeout](2026-10-05-product-closeout.md) records subsequent
UI, lifecycle, encrypted-prime cache, packaging and final-source verification.
These historical results are not relabeled as tests of later product commits.

Source pins: native engine `fab8e166`, desktop `ae34bc6` (all three Git
engine dependencies and Cargo.lock pinned to `fab8e166`), extension
`1954ed3`, presentation `e18da4c`. No local dependency overrides.

### Native, WASM and desktop

- Engine [CI run 37194799128](https://github.com/stars-labs/starlab-mpc/actions/runs/37194799128)
  passes all eight jobs on documentation head `550707f`; native source remains
  `fab8e166`, whose run 37193367450 also passes all eight jobs.
- Native workspace baseline: 513 passed, zero failed, 31 ignored across 26
  suites (`/tmp/starlab-final-workspace-tests.log`). Final session-modal fix
  adds three lifecycle tests; current full client suite: 277 passed, zero
  failed. Formatting, all-target check and TUI release pass.
- Ignored native e2e: 11 passed, zero failed, 687.61 seconds on `05f219e`
  (`/tmp/starlab-retry-budget-final-e2e.log`). Crypto source is unchanged in
  the final engine; only matching withdrawn TUI prompts are additionally
  dismissed. Conformance: 9 passed; L3 serve: 8; wire: 2.
- The timeout fixture applies 3 seconds only to an abandoned ceremony;
  the fresh retry uses the production budget. It does not mutate process
  environment variables.
- Fresh WASM and shared TypeScript types build. Vendored WASM SHA-256
  `7dd20b0da9c6a09236b53fd7441d84c82c18566a510a98b8f2101cff19b3d389`
  matches the fresh integrated build.
- Desktop locked tests: 49 passed, zero failed, one ignored; strict Clippy,
  formatting and locked release pass. Explicit real three-node
  DKG/sign/account-1 recovery/encrypted FROST export/import: one passed,
  37.56 seconds. No temporary patches or Cargo overrides.
- Final desktop release SHA-256:
  `d668b407ab273db91426a8c2f12b1cd2f4f6bc4122f5c3b9ef4d045a3e672d7d`.
  Final TUI release SHA-256:
  `f55a9504ae3d764f83f914446afeac6b1a00fe15d43df75345a2777c4331b268`.

### Extension

- Final `1954ed3` [CI run 37193441768](https://github.com/stars-labs/starlab-wallet/actions/runs/37193441768):
  all three jobs pass. Secp256k1 main suite: 12 passed (39.7 minutes),
  one conditional namespace skip; separate namespace recovery: one passed
  (3.2 minutes). Log: `/tmp/starlab-extension-1954-secp-job.log`.
  Check: 947 tests passed, zero failed, 2,581 assertions, 620.88 seconds;
  Svelte has zero errors/warnings and the production build passes.
- Prior `a260304` [run 37190375406](https://github.com/stars-labs/starlab-wallet/actions/runs/37190375406)
  passes all three jobs. Secp256k1: 12 cases passed (41.4 minutes), separate
  network-namespace recovery: one passed (3.8 minutes).
- Final local ED25519: 8 passed and 5 conditional skips; namespace outage:
  one passed. Final local secp256k1: 12 passed, zero failed, one conditional
  namespace skip (25 minutes); separate namespace recovery: one passed
  (2.4 minutes). These cover threshold sizes, accounts, Ethereum/Bitcoin
  signing, EIP-1559 sender recovery, FROST refresh, signaling outage and
  imported-wallet restart.
- Encrypted restore/chain metadata/sign/lock/restart regression: 19 passed,
  67 assertions. Recipient-topic discovery real-RPC regression: 24 passed.
  Mock-isolation/real-transport combined regression: 103 passed. Production
  receive QR clearing and asynchronous stale-result guard pass.
- Final presentation fixes correctly label raw-hash signatures and show
  unavailable fiat value as a dash. Production-component checks exercise
  missing prices, network/account changes and delayed stale responses;
  ten light/dark captures have zero action/page errors.
- Production live bundle contains no insecure fixture-prime hook.

### Designer and real-client transaction

- User confirmed **Rabby, MetaMask and Phantom**. Designer reviewed every
  implemented feature against the [27-row matrix](2026-10-04-wallet-visual-design.md)
  and the [public competitor reference audit](2026-10-04-wallet-competitor-reference.md).
- Final extension manifest: 206 unique public-fixture images, zero action/page
  errors, per-image source provenance. Desktop: 58 images across 29 states.
  Native/curated token icons are keyed by chain and contract, with neutral
  unknown-token fallback. Nonexistent features are explicitly not applicable.
- Actual real-prime TUI + desktop + production extension 2-of-3 wallet and
  final GUI transaction acceptance **pass**. MPC account 0 is
  `0x2fb6ae33558e46ec6baa39c9c98cd00f4b0f7548`.
- Final [Sepolia transaction](https://eth-sepolia.blockscout.com/tx/0xe83cfdfba02c82fb067e7bdc195b8f729abe32ffaa247be9a2950805d1e4ff54)
  succeeds (`0x1`), block 11841485, gas 21000. Independent signed-transaction
  hashing, sender recovery and desktop approval-digest verification pass;
  RPC and Blockscout agree. The non-signing TUI request closes automatically
  on withdrawal without manual dismissal.
- [Durable public transaction evidence](../testing/evidence/sepolia-live-2026-10-04.json)
  and [live acceptance report](../testing/SEPOLIA_LIVE_ACCEPTANCE.md) preserve
  source revisions and independently reviewable receipt/signature checks.
- Ethereum ECDSA has no share refresh. Documentation and GUI clearly state
  that FROST refresh covers Bitcoin/Solana/Sui and leaves Ethereum unchanged.

### Completion audit

All five stages are complete with the original scope: transport-independent
threshold ECDSA and HD/replay/serialization checks; native client, CLI and
wire integration; TUI/desktop operation; WASM/extension operation and final
remote interop; real three-client wallet and confirmed Sepolia transaction.
Designer participation, token icons and every implemented feature's visual
acceptance against the three user-selected competitors are complete.

Engine #135, extension #83 and desktop #24 are reviewable follow-up PRs on
the existing feature branches. Their complete implementations and acceptance
evidence are ready for coordinated review; they have not been merged to main.
The final documentation-only commit does not change the validated runtime
source pins or vendored WASM.
