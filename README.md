# MPC Wallet

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=flat&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![WebRTC](https://img.shields.io/badge/WebRTC-333333?style=flat&logo=webrtc&logoColor=white)](https://webrtc.org/)

A production-ready Multi-Party Computation (MPC) wallet implementing FROST (Flexible Round-Optimized Schnorr Threshold) signatures for secure distributed key management across multiple platforms.

## Overview

MPC Wallet enables threshold signatures where private keys are split across multiple parties, requiring a minimum threshold to sign transactions. No single party ever has access to the complete private key, providing superior security for digital asset management.

### Key Features

- **Real FROST DKG**: Distributed key generation via the ZCash Foundation's `frost-core 2.2` crates
- **Threshold Signatures**: Configurable t-of-n threshold signing
- **Multi-Platform**: Browser extension, desktop GUI, and terminal UI
- **Multi-Chain Support**: Ethereum (secp256k1) and Solana (ed25519)
- **Peer-to-Peer**: Direct WebRTC connections between participants
- **Offline Mode**: Air-gapped SD-card operation option
- **Test Coverage**: `cargo test --workspace` runs 174 Rust tests; the browser extension has 500+ Bun tests — no third-party security audit of this codebase has been performed (report security issues via GitHub Security Advisories)

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/hecoinfo/mpc-wallet.git
cd mpc-wallet

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
# Run the TUI application
cargo run -p tui-node -- --device-id Device-001

# Create a wallet (interactive)
> create my_wallet 2 3
```

#### Desktop Application

```bash
# Run the native desktop app
cargo run -p native-node
```

## Documentation

### 📚 Documentation Hub
- [Technical Documentation](docs/MPC_WALLET_TECHNICAL_DOCUMENTATION.md) - Comprehensive technical reference (100+ pages)
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
mpc-wallet/
├── apps/                         # Applications
│   ├── browser-extension/        # Chrome/Firefox extension
│   ├── native-node/              # Desktop GUI application (Slint)
│   ├── tui-node/                 # Terminal UI application (Ratatui)
│   └── signal-server/            # WebRTC signaling (server + Cloudflare Worker)
│
├── packages/@mpc-wallet/         # Shared packages
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
- **Slint**: Native desktop UI framework
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
- Keystore at rest is PBKDF2 + AES-256-GCM (see `packages/@mpc-wallet/frost-core/src/keystore.rs`)
- Peer-to-peer traffic rides WebRTC (DTLS-SRTP); signaling over WSS
- FROST implementation comes from the [ZCash Foundation](https://github.com/ZcashFoundation/frost)
  crates (`frost-core 2.2`, `frost-ed25519 2.2`, `frost-secp256k1 2.2`)

No third-party security audit has been performed on this codebase as a
whole. Report vulnerabilities via [GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new).

## Performance

The repo has no `criterion` benches yet (PR welcome — see the open
deferred work in `CLAUDE.md`). Functional coverage that exercises the
real FROST paths:

- `cargo test` — 174 tests passing across the workspace (DKG, signing,
  keystore round-trip, HD derivation, WebRTC mesh simulator)
- `bun test` — 509 tests passing in the browser extension (RPC,
  session lifecycle, DKG auto-trigger, signing auto-trigger, decline
  paths)
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

- [GitHub Issues](https://github.com/hecoinfo/mpc-wallet/issues) - Report bugs
- [GitHub Discussions](https://github.com/hecoinfo/mpc-wallet/discussions) - Ask questions
- [Documentation](docs/) - Full documentation in this repo

## Roadmap

### Q1 2025
- [x] Browser extension MVP
- [x] Terminal UI application
- [x] Desktop application
- [ ] Mobile application (in progress)

### Q2 2025
- [ ] Hardware wallet integration
- [ ] Additional blockchain support
- [ ] Advanced recovery mechanisms
- [ ] Enterprise features

### Q3 2025
- [ ] Formal verification
- [ ] Performance optimizations
- [ ] Enhanced UI/UX
- [ ] Regulatory compliance features

## License

The workspace-level `Cargo.toml` declares `license = "Apache-2.0"`.
Individual crates under `packages/` and `apps/signal-server/` set their
own — see each crate's `Cargo.toml` for specifics
(`packages/@mpc-wallet/blockchain` is MIT; signal-server is dual
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
@software{mpc_wallet,
  title = {MPC Wallet: Multi-Party Computation Wallet},
  author = {MPC Wallet Team},
  year = {2025},
  url = {https://github.com/hecoinfo/mpc-wallet}
}
```

---

**Built with ❤️ by the MPC Wallet Team**

*Secure. Distributed. Open Source.*