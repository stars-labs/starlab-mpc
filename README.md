# Starlab MPC

[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=flat&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![WebRTC](https://img.shields.io/badge/WebRTC-333333?style=flat&logo=webrtc&logoColor=white)](https://webrtc.org/)

**Starlab MPC** is a Multi-Party Computation wallet engine built on FROST (Flexible Round-Optimized Schnorr Threshold) signatures — one distributed key, split across devices, that controls accounts on **Ethereum, Bitcoin, Solana, and Sui** at once. No single party ever holds the whole key.

> **Status — early-stage (`0.1.0`).** The engine works end-to-end (real multi-device DKG, signing, and resharing, exercised by a CI real-DKG e2e), but it is **not audited**: no third-party security review, no `criterion` benchmarks, no regulatory certification. Treat it as research-grade until those land (see § Security). Earlier drafts called this "production-ready"; that claim has been removed.

## Overview

Starlab MPC enables threshold signatures where the private key is split across multiple parties, requiring a minimum threshold (t-of-n) to sign. The combined key never exists in memory on any single participant. A **single DKG ceremony produces one unified wallet** that derives addresses across every supported chain.

### Key Features

- **Real FROST DKG**: Distributed key generation via the ZCash Foundation's `frost-core 2.2` crates
- **Threshold Signatures**: Configurable t-of-n threshold signing
- **Unified multi-chain wallet**: One DKG → addresses on **Ethereum & Bitcoin** (secp256k1) plus **Solana & Sui** (ed25519)
- **Multi-Platform**: Browser extension, desktop GUI, terminal UI, and a headless CLI
- **Peer-to-Peer**: Direct WebRTC connections between participants (signaling over WSS)
- **Key resharing**: Recover or rotate the cohort without changing the group public key
- **Offline Mode**: Air-gapped SD-card operation option
- **Tested**: `cargo test --workspace` (~180 Rust tests incl. a real-DKG e2e) + 500+ Bun tests in the browser extension — **not** a substitute for a security audit

## Use it as a library

The engine is published to **crates.io** and **npm** under the `starlab` / `@starlab` names.

**Rust (crates.io):**

```toml
[dependencies]
starlab-core = "0.1"          # ciphersuite-generic FROST: DKG, signing, unified keystore
starlab-blockchain = "0.1"    # address derivation + tx building (EVM / BTC / Solana / Sui)
```

```bash
cargo install starlab-cli     # headless MPC node: DKG, signing, resharing over a WebRTC mesh
```

Also published: `starlab-core-wasm` (browser bindings), `starlab-client` (engine + TUI), `starlab-signal-server`.

**TypeScript (npm):**

```bash
npm install @starlab/core-wasm @starlab/types
```

## Quick Start

### Installation (from source)

```bash
# Clone the repository
git clone https://github.com/stars-labs/starlab-mpc.git
cd starlab-mpc

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
# Run the TUI application (binary name is starlab-tui,
# lives in the starlab-client package)
cargo run -p starlab-client --bin starlab-tui -- --device-id Device-001
```

Inside the TUI, navigate with arrow keys → `Create New Wallet`
and fill in the form; there is no REPL prompt. (Earlier drafts of
this section showed a `> create my_wallet 2 3` command; no such
slash-style command shipped — the TUI is ratatui-based, not a
line-mode REPL.)

#### Desktop Application

The Iced desktop app lives in its own repo: **[stars-labs/starlab-desktop](https://github.com/stars-labs/starlab-desktop)**. It consumes this repo's `starlab-client` library (engine + `core::*Manager`) as a dependency.

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
- [TUI Documentation](apps/tui/docs/README.md) - Terminal UI comprehensive guide
- [TUI Architecture](apps/tui/docs/architecture/ARCHITECTURE.md) - System architecture
- [DKG Flows](apps/tui/docs/architecture/DKG_FLOWS.md) - Distributed key generation flows
- [User Guide](apps/tui/docs/guides/USER_GUIDE.md) - Complete user manual
- [Protocol Specs](apps/tui/docs/protocol/) - WebRTC and keystore session protocols
- [Offline Mode](apps/tui/docs/guides/offline-mode.md) - Air-gapped operation guide

#### Native Desktop Application
- Moved to its own repo: **[stars-labs/starlab-desktop](https://github.com/stars-labs/starlab-desktop)** (Iced GUI consuming this repo's `starlab-client`).

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
- [TUI Deployment Guide](apps/tui/docs/DEPLOYMENT_GUIDE.md) - Deploy TUI application

### 🔍 Implementation Details
- [Implementation Docs](docs/implementation/) - Feature implementation details
  - [EIP-6963 Implementation](docs/implementation/EIP-6963-IMPLEMENTATION.md) - Wallet provider discovery
  - [Multi-Layer2 Support](docs/implementation/MULTI_LAYER2_SUPPORT.md) - Layer 2 chain support

### 📝 Additional Resources
- [Changelog](docs/CHANGELOG.md) - Version history and release notes

## Project Structure

```
starlab-mpc/
├── apps/                         # Applications
│   ├── browser-extension/        # Chrome/Firefox extension (also → stars-labs/starlab-wallet)
│   ├── cli/                      # Headless CLI (starlab-cli) — also the conformance oracle
│   ├── tui/                      # Terminal UI + engine lib (crate: starlab-client, bin: starlab-tui)
│   └── signal-server/            # WebRTC signaling (server + Cloudflare Worker)
│   # Desktop GUI moved to stars-labs/starlab-desktop (Iced)
│
├── packages/@starlab/            # Shared packages
│   ├── core/                     # FROST protocol core (crate: starlab-core)
│   ├── core-wasm/                # WebAssembly bindings (crate: starlab-core-wasm)
│   ├── blockchain/               # Multi-chain support (EVM / Bitcoin / Solana / Sui)
│   └── types/                    # TypeScript type definitions (@starlab/types)
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
- Keystore at rest is PBKDF2 + AES-256-GCM (see `packages/@starlab/core/src/keystore.rs`)
- Peer-to-peer traffic rides WebRTC (DTLS-SRTP); signaling over WSS
- FROST implementation comes from the [ZCash Foundation](https://github.com/ZcashFoundation/frost)
  crates (`frost-core 2.2`, `frost-ed25519 2.2`, `frost-secp256k1 2.2`)

No third-party security audit has been performed on this codebase as a
whole. Report vulnerabilities via [GitHub Security Advisories](https://github.com/stars-labs/starlab-mpc/security/advisories/new).

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

- [GitHub Issues](https://github.com/stars-labs/starlab-mpc/issues) - Report bugs
- [GitHub Discussions](https://github.com/stars-labs/starlab-mpc/discussions) - Ask questions
- [Documentation](docs/) - Full documentation in this repo

## Roadmap

### Shipped
- [x] **Unified multi-chain wallet** — one DKG ceremony → one wallet
  with addresses on Ethereum, Bitcoin, Solana, and Sui
- [x] **Key resharing** — recover/rotate the cohort over the mesh while
  preserving the group public key (driven by `starlab-cli`)
- [x] Browser extension (Chrome / Firefox) — FROST DKG + threshold
  signing + EIP-1193 / EIP-6963 dApp integration
- [x] Terminal UI (`apps/tui/`) — keyboard-driven FROST
  frontend with online (WebRTC mesh) + offline (SD-card) modes
- [x] Headless CLI (`starlab-cli`) — scriptable DKG / signing /
  resharing; doubles as the conformance oracle in CI
- [x] Desktop application — Iced GUI reusing this repo's
  `starlab-client::core::*Manager` types; now in its own repo
  **[stars-labs/starlab-desktop](https://github.com/stars-labs/starlab-desktop)**
- [x] Cloudflare Worker signal server (Rust-over-WASM) and
  standalone `cargo`-built signal server
- [x] Published to crates.io (`starlab-*`) and npm (`@starlab/*`)

### Open work (no committed timelines)

Items below are listed in rough priority order. None has a
scheduled delivery date; contributions welcome via PR. See
[`CLAUDE.md`](CLAUDE.md) for deeper context where noted.

- [ ] Extract `SigningManager::approve` onto a ciphersuite-generic
  backend so the desktop app (starlab-desktop) shares the real
  signing path with the TUI (its last feature-parity gap).
- [ ] `criterion` benches for DKG / signing / keystore so future
  perf-optimization claims have reproducible numbers.
- [ ] Third-party security audit of the full stack. The upstream
  ZCash Foundation `frost-*` crates are audited; this workspace's
  integration layer + TUI + the GUI frontends are not.
- [ ] Hardware-wallet co-signer integration (Ledger / Trezor).
- [ ] Additional blockchains beyond the current four (Ethereum, Bitcoin,
  Solana, Sui) — each new chain needs per-curve address derivation
  + encoding work (see `packages/@starlab/blockchain/`).
- [ ] Structured audit-log emission (the absent feature flagged
  across the security docs).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option. Every workspace crate declares `license = "MIT OR Apache-2.0"`.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Acknowledgments

- [FROST Paper](https://eprint.iacr.org/2020/852) by Komlo & Goldberg
- [ZCash Foundation](https://github.com/ZcashFoundation/frost) for FROST implementation
- [WebRTC Project](https://webrtc.org/) for P2P communication
- All our contributors and community members

## Citation

If you use this software in your research, please cite:

```bibtex
@software{starlab_mpc,
  title = {Starlab MPC: A FROST Threshold-Signature Wallet Engine},
  author = {Stars Labs},
  year = {2026},
  url = {https://github.com/stars-labs/starlab-mpc}
}
```

---

**Built with ❤️ by Stars Labs**

*Secure. Distributed. Open Source.*