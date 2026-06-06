# docs/

Cross-cutting documentation for the MPC Wallet monorepo. The project
landing page is [/README.md](../README.md) at the repo root; this
directory collects documentation that doesn't belong to a single
workspace member.

## Contents

- [`LIVE_MPC_DEMO.md`](LIVE_MPC_DEMO.md) — **run the live, independently
  verifiable MPC demo** for investors: raw `mpc-wallet-cli` commands across
  three machines (ed25519), with the signature verified by an *external* tool
  (Node/Python/OpenSSL) and the key shown as a real Solana address. Includes
  talking points, the "one device can't sign alone" proof, fallbacks, and
  troubleshooting.
- [`MONOREPO_ARCHITECTURE.md`](MONOREPO_ARCHITECTURE.md) — workspace
  layout, how the Rust + Bun workspaces fit together, build order.
- [`MPC_WALLET_TECHNICAL_DOCUMENTATION.md`](MPC_WALLET_TECHNICAL_DOCUMENTATION.md)
  — comprehensive technical reference (architecture, protocol,
  crypto details, deployment scenarios). ~1,440 lines (earlier
  drafts said "several hundred pages" — not literally page-based;
  at typical print density this is closer to ~30 pages).
- [`MULTI_CURVE_DERIVATION.md`](MULTI_CURVE_DERIVATION.md) — how one
  root secret yields per-curve / per-account threshold wallets, and
  the **recovery model** (the seed alone ≠ your share; back up the
  keystore). Read alongside the next entry.
- [`SIGNATURE_CHAIN_COMPATIBILITY.md`](SIGNATURE_CHAIN_COMPATIBILITY.md)
  — which chains can actually *verify* a FROST (Schnorr) signature,
  and the sharp EVM-EOA exception (standard Ethereum EOAs verify
  ECDSA → need a smart-contract account).
- [`RECOVERY_AND_RESHARING.md`](RECOVERY_AND_RESHARING.md) — "what if a
  device is lost or stolen?" — threat model, recovery flows, and the
  design for share refresh/resharing (same address, fresh shares, drop a
  lost device) built on `frost-core::keys::refresh`. Investor talking
  points included.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — code-of-conduct, branching
  model, commit format, PR guidelines.
- [`CHANGELOG.md`](CHANGELOG.md) — release history.

### Subdirectories

- [`deployment/`](deployment/) — production deployment guide.
  Covers the four real targets this repo ships: Cloudflare
  Worker signal server (canonical production path), self-hosted
  Rust signal server behind a TLS terminator, browser extension
  builds for web-store distribution, and single-binary cargo
  builds for TUI / native-node end-user distribution.
  Worker-specific details in
  [`CLOUDFLARE_DEPLOYMENT.md`](deployment/CLOUDFLARE_DEPLOYMENT.md).

  Docker / K8s / Helm charts / Prometheus scaffolding are NOT
  shipped — earlier drafts of this doc tree claimed otherwise;
  the rewritten deployment guide (see `deployment/README.md`)
  explicitly documents what's absent.
- [`implementation/`](implementation/) — deep-dives on specific
  cross-cutting implementation choices. Notable:
  [`EIP-6963-IMPLEMENTATION.md`](implementation/EIP-6963-IMPLEMENTATION.md)
  (wallet provider discovery) and
  [`MULTI_LAYER2_SUPPORT.md`](implementation/MULTI_LAYER2_SUPPORT.md).
- [`testing/`](testing/) — testing strategy + harness docs. See
  [`testing/README.md`](testing/README.md) as the index; the
  [`RUN_TEST_INSTRUCTIONS.md`](testing/RUN_TEST_INSTRUCTIONS.md)
  is the practical how-to.

## Per-workspace-member docs

Each app and package has its own docs subtree:

- [`apps/tui-node/docs/`](../apps/tui-node/docs/) — largest
  subtree; architecture, protocol, keyboard handling, keystore
  internals. The many historical phase-summary / dev-journal
  docs have been moved under
  [`apps/tui-node/docs/archive/dev-journal/`](../apps/tui-node/docs/archive/dev-journal/)
  with a per-doc index explaining what each artefact documents
  (mostly retrospectives of fixes that have long since
  landed).
- [`apps/browser-extension/docs/`](../apps/browser-extension/docs/)
- [`apps/native-node/docs/`](../apps/native-node/docs/) — defers
  most content to the parent [`apps/native-node/README.md`](../apps/native-node/README.md).
- [`apps/signal-server/docs/`](../apps/signal-server/docs/)
- (Rust library crates don't have docs subtrees; their public API
  is documented via `///` rustdoc.)
