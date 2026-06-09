# MPC Wallet

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=flat&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![WebRTC](https://img.shields.io/badge/WebRTC-333333?style=flat&logo=webrtc&logoColor=white)](https://webrtc.org/)

A Multi-Party Computation (MPC) wallet implementing FROST (Flexible Round-Optimized Schnorr Threshold) signatures for secure distributed key management across multiple platforms. Currently early-stage development software — every workspace crate is at `0.1.0`, no tagged release has been cut, and no third-party security audit has been performed (see § Security below). Earlier drafts of this line called the repo "production-ready" — that claim contradicts all the caveats elsewhere in this README (no audit, no benchmarks, no regulatory certification) so it has been removed.

## Overview

MPC Wallet enables threshold signatures where private keys are split across multiple parties, requiring a minimum threshold to sign transactions. No single party ever has access to the complete private key, providing superior security for digital asset management.

### Key Features

- **Real FROST DKG**: Distributed key generation via the ZCash Foundation's `frost-core 2.2` crates
- **Threshold Signatures**: Configurable t-of-n threshold signing
- **Multi-Platform**: Browser extension, desktop GUI, and terminal UI
- **Multi-Chain Support**: Ethereum (secp256k1) and Solana (ed25519)
- **Peer-to-Peer**: Direct WebRTC connections between participants
- **Offline Mode**: Air-gapped SD-card operation option
- **Test Coverage**: `cargo test --workspace` runs ~180 Rust tests (184 `#[test]` / `#[tokio::test]` annotations as of this writing; the number drifts as tests land on `main`); the browser extension has 500+ Bun tests — no third-party security audit of this codebase has been performed (report security issues via GitHub Security Advisories)

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/hecoinfo/frost-mpc.git
cd frost-mpc

# Install dependencies
bun install

# Build WASM modules
bun run build:wasm

# Start development
bun run dev
```

### Basic Usage

#### Browser Extension

1. Build and load the extension:
```bash
cd apps/browser-extension
bun run dev
```

2. Load unpacked extension in Chrome from `.output/chrome-mv3`

3. Create a wallet through the extension popup

#### Terminal UI

```bash
# Run the TUI application (binary name is frost-mpc-tui,
# lives in the tui-node package)
cargo run -p tui-node --bin frost-mpc-tui -- --device-id Device-001
```

Inside the TUI, navigate with arrow keys → `Create New Wallet`
and fill in the form; there is no REPL prompt. (Earlier drafts of
this section showed a `> create my_wallet 2 3` command; no such
slash-style command shipped — the TUI is ratatui-based, not a
line-mode REPL.)

#### Desktop Application

```bash
# Run the native desktop app (package name is frost-mpc-native,
# NOT native-node — the directory is native-node/ but the
# Cargo package is frost-mpc-native)
cargo run -p frost-mpc-native
```

## Documentation

### 📚 Documentation Hub
- [Technical Documentation](docs/MPC_WALLET_TECHNICAL_DOCUMENTATION.md) - Comprehensive technical reference (~1,440 lines; earlier drafts said "100+ pages" — overstated, it's closer to ~30 pages at typical print density)
- [Contributing Guidelines](docs/CONTRIBUTING.md) - How to contribute to the project

### 🏗️ Architecture & Design
- [Monorepo Architecture](docs/MONOREPO_ARCHITECTURE.md) - Monorepo structure and organization

### 📖 Application Documentation

#### Browser Extension
- [Browser Extension Guide](apps/browser-extension/docs/README.md) - Complete browser extension documentation
- [UI Components](apps/browser-extension/docs/ui/README.md) - UI implementation and components

#### Terminal UI (TUI)
- [TUI Documentation](apps/tui-node/docs/README.md) - Terminal UI comprehensive guide
- [TUI Architecture](apps/tui-node/docs/architecture/ARCHITECTURE.md) - System architecture
- [DKG Flows](apps/tui-node/docs/architecture/DKG_FLOWS.md) - Distributed key generation flows
- [User Guide](apps/tui-node/docs/guides/USER_GUIDE.md) - Complete user manual
- [Protocol Specs](apps/tui-node/docs/protocol/) - WebRTC and keystore session protocols
- [Offline Mode](apps/tui-node/docs/guides/offline-mode.md) - Air-gapped operation guide

#### Native Desktop Application
- [Native App README](apps/native-node/README.md) - Architecture diagram, feature-parity matrix, build + run instructions
- [Docs subtree](apps/native-node/docs/README.md) - Additional native-node documentation

#### Signal Server
- [Signal Server Guide](apps/signal-server/docs/README.md) - WebRTC signaling server
- [Deployment](apps/signal-server/docs/deployment/cloudflare-deployment.md) - Cloudflare deployment guide

### 🔧 Development Resources
- [Testing Documentation](docs/testing/README.md) - Testing strategies and tools
  - [Test Coverage](docs/testing/COVERAGE.md) - Code coverage reports
  - [E2E Testing Plan](docs/testing/E2E_TEST_IMPLEMENTATION_PLAN.md) - Plan for real-signal-server E2E tests
  - [Running Tests](docs/testing/RUN_TEST_INSTRUCTIONS.md) - How to run test suites

### 🚀 Deployment & Operations
- [Deployment Guide](docs/deployment/README.md) - Production deployment instructions
- [Cloudflare Deployment](docs/deployment/CLOUDFLARE_DEPLOYMENT.md) - Deploy to Cloudflare Workers
- [TUI Deployment Guide](apps/tui-node/docs/DEPLOYMENT_GUIDE.md) - Deploy TUI application

### 🔍 Implementation Details
- [Implementation Docs](docs/implementation/) - Feature implementation details
  - [EIP-6963 Implementation](docs/implementation/EIP-6963-IMPLEMENTATION.md) - Wallet provider discovery
  - [Multi-Layer2 Support](docs/implementation/MULTI_LAYER2_SUPPORT.md) - Layer 2 chain support

### 📝 Additional Resources
- [Changelog](docs/CHANGELOG.md) - Version history and release notes

## Project Structure

```
frost-mpc/
├── apps/                         # Applications
│   ├── browser-extension/        # Chrome/Firefox extension
│   ├── native-node/              # Desktop GUI application (Iced)
│   ├── tui-node/                 # Terminal UI application (Ratatui)
│   └── signal-server/            # WebRTC signaling (server + Cloudflare Worker)
│
├── packages/@frost-mpc/         # Shared packages
│   ├── frost-core/               # FROST protocol implementation (Rust)
│   ├── core-wasm/                # WebAssembly bindings
│   ├── blockchain/               # Multi-chain support (Ethereum/Solana/Bitcoin)
│   └── types/                    # TypeScript type definitions
│
├── docs/                         # Documentation
└── scripts/                      # Build, test, and operational scripts
```

## Technology Stack

### Core Technologies

- **Rust**: Core cryptographic implementation
- **TypeScript**: Browser extension and web components
- **WebAssembly**: Bridge between Rust and JavaScript
- **WebRTC**: Peer-to-peer communication
- **Svelte**: Browser extension UI
- **Iced**: Native desktop UI framework (MIT)
- **Ratatui**: Terminal UI framework

### Cryptography

- **FROST**: Threshold signature scheme
- **secp256k1**: Ethereum signatures
- **ed25519**: Solana signatures
- **AES-256-GCM**: Encryption at rest
- **PBKDF2**: Key derivation

## Use Cases

### Individual Users
- Secure personal wallet with distributed backups
- Multi-device wallet control
- Enhanced security for high-value accounts

### Organizations
- Corporate treasury management
- Multi-signature custody solutions
- Distributed key management for exchanges
- Secure validator key management

### Developers
- Integration into existing applications
- Custom threshold signature implementations
- Research and development platform

## Security

The MPC Wallet is designed around threshold cryptography primitives:

- Root secret entropy is split via FROST DKG — the combined private key
  never exists in memory on any single participant
- Keystore at rest is PBKDF2 + AES-256-GCM (see `packages/@frost-mpc/frost-core/src/keystore.rs`)
- Peer-to-peer traffic rides WebRTC (DTLS-SRTP); signaling over WSS
- FROST implementation comes from the [ZCash Foundation](https://github.com/ZcashFoundation/frost)
  crates (`frost-core 2.2`, `frost-ed25519 2.2`, `frost-secp256k1 2.2`)

No third-party security audit has been performed on this codebase as a
whole. Report vulnerabilities via [GitHub Security Advisories](https://github.com/hecoinfo/frost-mpc/security/advisories/new).

## Performance

The repo has no `criterion` benches yet (PR welcome — see the open
deferred work in `CLAUDE.md`). Functional coverage that exercises the
real FROST paths:

- `cargo test` — ~180 tests across the workspace (184
  `#[test]` / `#[tokio::test]` annotations as of this writing;
  covers DKG, signing, keystore round-trip, HD derivation,
  WebRTC mesh simulator). Refresh count via
  `grep -c '#\[test\]\|#\[tokio::test\]' $(find . -name '*.rs'
  | grep -v target)`.
- `bun test` — ~530 test cases in the browser extension (529
  `test()` / `it()` call sites across 45 `*.test.ts` files;
  covers RPC, session lifecycle, DKG auto-trigger, signing
  auto-trigger, decline paths). Earlier drafts said "509
  passing"; number drifts as suites land on main.
- FROST itself is parameter-generic over `t`/`n`; the bottleneck at
  larger cohorts is the WebRTC full-mesh degree (n·(n-1)/2 peer
  connections), not the cryptography. No hard participant cap is
  enforced, but production use has only been exercised at small
  cohorts (2-of-3, 3-of-5).

## Contributing

We welcome contributions! Please see our [Contributing Guide](docs/CONTRIBUTING.md) for details on:

- Code of Conduct
- Development setup
- Submitting pull requests
- Reporting issues
- Security vulnerabilities

## Support

### Community

- [GitHub Issues](https://github.com/hecoinfo/frost-mpc/issues) - Report bugs
- [GitHub Discussions](https://github.com/hecoinfo/frost-mpc/discussions) - Ask questions
- [Documentation](docs/) - Full documentation in this repo

## Roadmap

### Shipped (through early 2025)
- [x] Browser extension (Chrome / Firefox) — FROST DKG + threshold
  signing + EIP-1193 / EIP-6963 dApp integration
- [x] Terminal UI (`apps/tui-node/`) — keyboard-driven FROST
  frontend with online (WebRTC mesh) + offline (SD-card) modes
- [x] Desktop application (`apps/native-node/`) — Iced GUI reusing
  `tui-node::core::*Manager` types; feature-parity with TUI except
  for the `SigningManager::approve` stub (see
  [`apps/native-node/README.md`](apps/native-node/README.md) for
  the precise ⚠-marked matrix)
- [x] Cloudflare Worker signal server (Rust-over-WASM) and
  standalone `cargo`-built signal server

### Open work (no committed timelines)

Items below are listed in rough priority order. None has a
scheduled delivery date; contributions welcome via PR. See
[`CLAUDE.md`](CLAUDE.md) for deeper context where noted.

- [ ] Extract `SigningManager::approve` onto a ciphersuite-generic
  backend so native-node shares the real signing path with the TUI
  (last remaining feature-parity gap — see the native-node README
  + `CLAUDE.md § Native desktop node`).
- [ ] `criterion` benches for DKG / signing / keystore so future
  perf-optimization claims have reproducible numbers.
- [ ] FROST share refresh (proactive share rotation preserving the
  same group key). FROST supports it in principle; this crate
  doesn't wire it up yet.
- [ ] Third-party security audit of the full stack. The upstream
  ZCash Foundation `frost-*` crates are audited; this workspace's
  integration layer + extension + TUI + native frontends are not.
- [ ] Hardware-wallet co-signer integration (Ledger / Trezor).
- [ ] Additional blockchains beyond Ethereum (secp256k1) + Solana
  (ed25519) — each new chain needs per-curve address derivation
  + encoding work (see `packages/@frost-mpc/blockchain/`).
- [ ] Structured audit-log emission (the absent feature flagged
  across the security docs).

## License

The workspace-level `Cargo.toml` declares `license = "Apache-2.0"`.
Individual crates under `packages/` and `apps/signal-server/` set their
own — see each crate's `Cargo.toml` for specifics
(`packages/@frost-mpc/blockchain` is MIT; signal-server is dual
MIT-or-Apache-2.0; everything else Apache-2.0). A repo-root `LICENSE`
file hasn't been committed yet — contribute the Apache-2.0 text
(or whichever single license you pick for the project) to settle
the ambiguity.

## Acknowledgments

- [FROST Paper](https://eprint.iacr.org/2020/852) by Komlo & Goldberg
- [ZCash Foundation](https://github.com/ZcashFoundation/frost) for FROST implementation
- [WebRTC Project](https://webrtc.org/) for P2P communication
- All our contributors and community members

## Citation

If you use this software in your research, please cite:

```bibtex
@software{frost_mpc,
  title = {MPC Wallet: Multi-Party Computation Wallet},
  author = {MPC Wallet Team},
  year = {2025},
  url = {https://github.com/hecoinfo/frost-mpc}
}
```

---

**Built with ❤️ by the MPC Wallet Team**

*Secure. Distributed. Open Source.*