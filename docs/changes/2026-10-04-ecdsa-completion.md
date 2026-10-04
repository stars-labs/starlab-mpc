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
