# Changelog

The format is inspired by [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
but note that no tagged release has been cut yet — `git tag -l` is
empty and every workspace crate is at `0.1.0` (signal-server is at
`0.1.1` because it was published to crates.io once before being
absorbed into the monorepo).

This file records notable milestones as they land on `main`; version
numbers below are milestone labels, not published release tags.

## Unreleased

Ongoing development on `main`. Recent threads include repo-hygiene
cleanup (doc-vs-code accuracy, unused-dependency pruning, stale
archive) and feature work on native-node and browser-extension
signing UX. See `git log` for the authoritative record.

## Monorepo migration milestone — July 2025

### Added
- **Monorepo layout**: Workspace reorganized into `apps/` (starlab-client,
  native-node, signal-server, browser-extension) and
  `packages/@starlab/` (frost-core, core-wasm, blockchain, types).
- **Native desktop app** (`apps/native-node/`): Slint 1.x UI reusing
  `starlab-client::core::{*Manager, CoreState}` via a `UICallback` trait.
- **Shared packages**:
  - `@starlab/core`: ciphersuite-generic FROST library used
    by TUI, native, and (via WASM) the browser extension.
  - `@starlab/core-wasm`: thin `wasm-bindgen` wrapper.
  - `@starlab/types`: shared TypeScript types for the extension.
- **Build tooling**: Unified Cargo workspace (edition 2024, requires
  Rust 1.85+); Bun workspace for TypeScript packages.

### Changed
- **Browser extension**: Moved from repo root to `apps/browser-extension/`.
- **TypeScript imports**: Standardized on `@starlab/types` package path.
- **Build commands**: All `bun run *` scripts run from the repo root.
- **Nix flake**: Added GUI libs (Wayland, X11, accesskit deps) for Slint.

### Breaking
- File paths changed wholesale due to the monorepo restructure.
- Import statements rewritten to use `@starlab/types`.
- Build commands must be run from the workspace root, not inside apps.

## Pre-monorepo milestone — July 2025

### Added
- Initial browser extension (MV3) with FROST-over-WebRTC threshold signing.
- CLI / TUI node with FROST ed25519 + secp256k1 DKG and signing.
- WebRTC peer-to-peer networking (full-mesh) with WebSocket signal server.
- Multi-chain address derivation (Ethereum, Solana).
- Keystore import/export between TUI and extension.
