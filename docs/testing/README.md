# Testing Documentation

Reference material for the MPC Wallet test suite.

## Contents

- `TESTING.md` — general testing guidelines and patterns
- `COVERAGE.md` — coverage configuration + Bun limitations
- `RUN_TEST_INSTRUCTIONS.md` — 3-node manual test run
- [END_TO_END.md](END_TO_END.md) — implemented coverage, execution commands, and evidence boundaries

Per-crate test docs live with the code:
- Browser-extension tests moved with the extension to
  [`stars-labs/starlab-wallet`](https://github.com/stars-labs/starlab-wallet)
- Rust tests run with `cargo test --workspace --locked`; expensive network
  suites require the explicit commands in [END_TO_END.md](END_TO_END.md).
