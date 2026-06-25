# Testing Documentation

Reference material for the MPC Wallet test suite.

## Contents

- `TESTING.md` — general testing guidelines and patterns
- `COVERAGE.md` — coverage configuration + Bun limitations
- `RUN_TEST_INSTRUCTIONS.md` — 3-node manual test run
- `E2E_TEST_IMPLEMENTATION_PLAN.md` — plan for real-signal-server E2E

Per-crate test docs live with the code:
- Browser-extension tests moved with the extension to
  [`stars-labs/starlab-wallet`](https://github.com/stars-labs/starlab-wallet)
- Rust tests run with `cargo test --workspace` (see repo CLAUDE.md)