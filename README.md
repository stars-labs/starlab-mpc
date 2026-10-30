# Starlab MPC

[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=flat&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![WebRTC](https://img.shields.io/badge/WebRTC-333333?style=flat&logo=webrtc&logoColor=white)](https://webrtc.org/)

**Starlab MPC** is a threshold wallet engine for **Ethereum, Bitcoin, Solana, and Sui**. Ethereum uses cggmp24 threshold ECDSA; Bitcoin uses FROST BIP-340 signatures, and Solana/Sui use FROST ed25519. A unified wallet contains separate distributed keys for these signing suites, split across the same devices. No single device holds a complete private key.

> **Status — early-stage (`0.1.0`).** The engine works end-to-end (real multi-device DKG, signing, and FROST resharing, exercised by CI), but it is **not audited**: no third-party security review, no `criterion` benchmarks, no regulatory certification. Treat it as research-grade until those land (see § Security).

## Overview

Starlab MPC enables threshold signatures where the private key is split across multiple parties, requiring a minimum threshold (t-of-n) to sign. The combined key never exists in memory on any single participant. A **single DKG ceremony produces one unified wallet** that derives addresses across every supported chain.

### Key Features

- **Distributed key generation**: FROST via the ZCash Foundation's `frost-core 3.0` crates, plus cggmp24 ECDSA for Ethereum
- **Threshold Signatures**: Configurable t-of-n threshold signing
- **Unified multi-chain wallet**: One setup ceremony creates separate ECDSA, Taproot and ed25519 keys; account addresses derive publicly without unlocking a share
- **Multi-Platform**: Browser extension, desktop GUI, terminal UI, and a headless CLI
- **Peer-to-Peer**: Direct WebRTC connections between participants (signaling over WSS)
- **FROST key resharing**: Rotate Bitcoin/Solana/Sui shares without changing their group public keys. The ECDSA key cannot be refreshed or reshared; replacing an Ethereum signer requires a new key and moving funds (see [Recovery and Resharing](docs/RECOVERY_AND_RESHARING.md))
- **Offline Mode**: Air-gapped SD-card operation option
- **Tested**: `cargo test --workspace --locked`, with real network suites run using `--ignored`; browser extension tests live in [stars-labs/starlab-wallet](https://github.com/stars-labs/starlab-wallet). Current source, artifact hashes and verification boundaries are recorded in [product acceptance](docs/changes/2026-10-05-product-closeout.md).

## Use it as a library

Packages are published under `starlab-*` on [crates.io](https://crates.io/crates/starlab-cli) and `@stars-labs/*` on [npm](https://www.npmjs.com/package/@stars-labs/core-wasm). The registry installation examples below refer to published releases, separate from the reviewed development revision. The GUI repositories pin their engine dependency and document their source builds.

**Rust (crates.io):**

```toml
[dependencies]
starlab-core = "0.1"          # published core package
starlab-blockchain = "0.1"    # published multi-chain package
```

```bash
cargo install starlab-cli     # headless MPC node: DKG, signing, resharing over a WebRTC mesh
```

Also published: `starlab-core-wasm` (browser bindings), `starlab-client` (engine + TUI), `starlab-signal-server`.

**TypeScript (npm):**

```bash
npm install @stars-labs/core-wasm @stars-labs/types
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
```

### Basic Usage

#### Browser Extension

The extension lives in its own repo: **[stars-labs/starlab-wallet](https://github.com/stars-labs/starlab-wallet)**. It consumes this repo's `@stars-labs/core-wasm` and `@stars-labs/types` packages.

#### Terminal UI

```bash
# Run the TUI application (binary name is starlab-tui,
# lives in the starlab-client package)
cargo run -p starlab-client --bin starlab-tui -- --device-id Device-001
```

Inside the TUI, use the arrow keys to select `Create New Wallet` and fill in
the form.

#### Desktop Application

The Iced desktop app lives in its own repo: **[stars-labs/starlab-desktop](https://github.com/stars-labs/starlab-desktop)**. It consumes this repo's `starlab-client` library (engine + `core::*Manager`) as a dependency.

## Documentation

### 📚 Documentation Hub
- [Technical Documentation](docs/MPC_WALLET_TECHNICAL_DOCUMENTATION.md) - Technical reference
- [Contributing Guidelines](docs/CONTRIBUTING.md) - How to contribute to the project

### 🏗️ Architecture & Design
- [Monorepo Architecture](docs/MONOREPO_ARCHITECTURE.md) - Monorepo structure and organization

### 📖 Application Documentation

#### Browser Extension
- Moved to its own repo: **[stars-labs/starlab-wallet](https://github.com/stars-labs/starlab-wallet)** (consumes this repo's `@stars-labs/core-wasm` + `@stars-labs/types`).

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
  - [End-to-End Tests](docs/testing/END_TO_END.md) - Implemented network coverage, commands and fixture boundaries
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
│   ├── cli/                      # Headless CLI (starlab-cli) — also the conformance oracle
│   ├── tui/                      # Terminal UI + engine lib (crate: starlab-client, bin: starlab-tui)
│   └── signal-server/            # WebRTC signaling (server + Cloudflare Worker)
│   # Browser extension moved to stars-labs/starlab-wallet
│   # Desktop GUI moved to stars-labs/starlab-desktop (Iced)
│
├── packages/@starlab/            # Shared packages
│   ├── core/                     # FROST protocol core (crate: starlab-core)
│   ├── core-wasm/                # WebAssembly bindings (crate: starlab-core-wasm)
│   ├── blockchain/               # Multi-chain support (EVM / Bitcoin / Solana / Sui)
│   └── types/                    # TypeScript type definitions (@stars-labs/types)
│
├── docs/                         # Documentation
└── scripts/                      # Build, test, and operational scripts
```

## Technology Stack

### Core Technologies

- **Rust**: Core cryptographic implementation
- **TypeScript**: Shared types (`@stars-labs/types`) for WASM consumers
- **WebAssembly**: Bridge between Rust and JavaScript
- **WebRTC**: Peer-to-peer communication
- **Iced**: Native desktop UI framework (MIT, in starlab-desktop)
- **Ratatui**: Terminal UI framework

### Cryptography

- **FROST**: Threshold signature scheme
- **secp256k1**: Ethereum ECDSA and Bitcoin BIP-340 signatures
- **ed25519**: Solana and Sui signatures
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
  crates (`frost-core 3.0`, `frost-ed25519 3.0`, `frost-secp256k1-tr 3.0`)

No third-party security audit has been performed on this codebase as a
whole. Report vulnerabilities via [GitHub Security Advisories](https://github.com/stars-labs/starlab-mpc/security/advisories/new).

## Performance

The repo has no `criterion` benchmarks. Functional test timings and screenshot
capture timings are not performance benchmarks. [Product acceptance](docs/changes/2026-10-05-product-closeout.md)
records executed counts, exact revisions and production-prime evidence;
[end-to-end coverage](docs/testing/END_TO_END.md) lists commands for the
default-ignored network suites. Bare `cargo test` covers only the default TUI
member; use `cargo test --workspace --locked` for the full workspace.

Browser tests live in [stars-labs/starlab-wallet](https://github.com/stars-labs/starlab-wallet).
The WebRTC mesh needs n·(n-1)/2 peer connections. Ethereum setup also performs
safe-prime generation and Paillier proofs; the cold-start tests exercise that
work separately from faster protocol tests using explicit insecure fixtures.

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
- [x] **FROST key resharing** — rotate Bitcoin/Solana/Sui shares over the mesh
  while preserving their group public keys; Ethereum ECDSA shares remain unchanged
- [x] Browser extension (verified Chromium runtime) — FROST DKG + threshold
  signing + EIP-1193 / EIP-6963 dApp integration; now in its own repo
  **[stars-labs/starlab-wallet](https://github.com/stars-labs/starlab-wallet)**
- [x] Terminal UI (`apps/tui/`) — keyboard-driven FROST
  frontend with online (WebRTC mesh) + offline (SD-card) modes
- [x] Headless CLI (`starlab-cli`) — scriptable DKG / signing /
  resharing; doubles as the conformance oracle in CI
- [x] Desktop application — Iced GUI reusing this repo's
  `starlab-client::core::*Manager` types; now in its own repo
  **[stars-labs/starlab-desktop](https://github.com/stars-labs/starlab-desktop)**
- [x] Cloudflare Worker signal server (Rust-over-WASM) and
  standalone `cargo`-built signal server
- [x] Published to crates.io (`starlab-*`) and npm (`@stars-labs/*`)

### Open work (no committed timelines)

Items below have no scheduled delivery date; contributions welcome via PR.
Firefox build success does not establish its offscreen cryptographic runtime;
the verified extension runtime is Chromium.
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
  title = {Starlab MPC: A Multi-Chain Threshold Wallet Engine},
  author = {Stars Labs},
  year = {2026},
  url = {https://github.com/stars-labs/starlab-mpc}
}
```

---

**Built with ❤️ by Stars Labs**

*Secure. Distributed. Open Source.*
