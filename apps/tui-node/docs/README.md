# Terminal UI Node Documentation

## Overview

The MPC Wallet Terminal UI (TUI) Node is a keyboard-driven terminal application for distributed key generation (FROST DKG), threshold signing, and encrypted keystore management. Built in Rust with Ratatui via the tui-realm Elm-architecture framework; interoperates with the browser extension + native desktop app over the same wire protocol and keystore format.

## Documentation Structure

### Core Documentation
- [Architecture](architecture/) - System design, state management, network protocols
- [User Guide](guides/) - Complete user manual and tutorials
- [Protocol](protocol/) - WebRTC signaling and keystore session protocols
- [UI Design](ui/) - Interface wireframes and navigation flows

## Features

### Core Capabilities
- **Interactive Terminal UI**: Menu-driven interface with keyboard navigation
- **FROST MPC Protocol**: Secure threshold signatures with distributed key generation
- **Multi-Blockchain Support**: Ethereum (secp256k1) and Solana (ed25519)
- **Offline Mode**: Air-gapped operations for maximum security
- **Encrypted Keystore**: Password-protected local key storage

### Additional Features
- **WebSocket Reconnect**: Signal-server connection is re-established
  automatically after drops; mid-ceremony resumption depends on the
  peers still holding their in-memory FROST state.
- **Multi-Wallet Support**: Manage multiple wallets per device. Each
  wallet is partitioned by curve (ed25519 / secp256k1) under the
  device_id in `~/.frost_keystore/`.
- **Import/Export**: Keystore round-trips with the browser extension
  are test-covered (same PBKDF2 + AES-256-GCM format).

## Quick Start

### Prerequisites
- Rust 1.85+ (workspace is edition 2024; see root `Cargo.toml`)
- Linux, macOS, or Windows
- Terminal with UTF-8 support

### Installation

```bash
# Clone repository
git clone https://github.com/hecoinfo/mpc-wallet
cd mpc-wallet/apps/tui-node

# Build from source
cargo build --release

# Run the application
cargo run --release -- --device-id Device-001
```

### Command Line Options

See § Configuration → CLI Flags below for the real flag list
(`--device-id` / `--offline` / `--signal-server` / `--log-location` /
`--log-level`). Earlier drafts of this section duplicated the list
with fabricated flags (`--config <PATH>`, `--keystore <PATH>`, a
`[required]` marker on `--device-id`, explicit `--help`). None of
those are real:

- `--config` / `--keystore` — do not exist; the TUI has no config
  file and the keystore path is hardcoded to `~/.frost_keystore`.
- `--device-id` is **optional**, not required — defaults to the
  machine hostname via `gethostname::gethostname()`.
- `--help` works because `clap::Parser` generates it automatically;
  it isn't an explicit flag in the `Args` struct.

Verified against `apps/tui-node/src/bin/mpc-wallet-tui.rs:13-39`.

## User Interface

### Main Screen Layout

Launch the binary and you see the main menu defined in
`src/elm/components/main_menu.rs:55-114`. Items vary with wallet
state:

- **Always shown**: `Create New Wallet`, `Join Session`,
  `Settings`, `Exit`
- **Added when `wallet_count > 0`**: `Manage Wallets`; DKG-progress
  and signing surfaces live inside sub-screens rather than the
  top-level menu.

Earlier drafts of this section printed a `[1] Wallet / [2] DKG /
[3] Sign / [4] Session / [5] Network / [6] Settings / [Q] Quit`
numbered layout with a right-hand pane showing `Current Wallet` /
`Address` / `Balance: 1.234 ETH`. None of that is real — the menu
uses arrow-key navigation over an emoji-icon list with no number
hotkeys, there is no wallet-summary side pane, and the TUI does
NOT query on-chain balances (so the `1.234 ETH` figure was
fabricated).

### Navigation

Keys actually handled by `src/elm/update.rs` + per-component
`on(KeyEvent)` impls:

| Key | Action |
|-----|--------|
| ↑ / ↓ | Move selection within a menu or list |
| Enter | Select the highlighted item |
| Esc | Go back one screen / cancel the current operation |
| Tab | Move focus between fields inside a form |
| q | Quit from the main menu |

Earlier drafts listed `Space` / `?` bindings — neither is wired up,
and no context-help modal ships. See
[`KEYBOARD_NAVIGATION_GUIDE.md`](./KEYBOARD_NAVIGATION_GUIDE.md)
for the authoritative reference.

## Core Workflows

### Creating a Wallet

1. From the main menu (arrow keys, not number hotkeys), select
   **Create New Wallet**.
2. Fill the form: Name, Threshold, Total participants, Blockchain
   (Ethereum and/or Solana — unified DKG produces both address
   types from a single ceremony via
   `frost-core::unified_dkg`).
3. The creator's node mints a session id and broadcasts
   `AnnounceSession`; co-signers discover it via
   `SessionAvailable`.
4. Co-signers pick the session from **Join Session** and enter
   the session id.
5. When participants == total, DKG runs automatically
   (Round 1 → Round 2 → Finalization).
6. The wallet is persisted to
   `~/.frost_keystore/<device_id>/<curve>/<wallet_id>.{json,dat}`.

Earlier drafts said "Select [1] Wallet from main menu" — no such
item exists; the top-level label is "Create New Wallet".

### Distributed Key Generation (DKG)

```
Online DKG:
Device A ──┐
           ├──► Signal Server ──► WebRTC Mesh ──► FROST DKG ──► Key Shares
Device B ──┘

Offline DKG (SD-card air-gap):
Device A ──► Export (.json) ──► USB ──► Device B
                                          ↓
                                   Process Offline
                                          ↓
Device B ──► Export (.json) ──► USB ──► Device A
```

### Transaction Signing

Live today the TUI supports **message signing**, not full
transaction construction-and-signing — see the "Phase C scope"
note in `src/elm/components/sign_transaction.rs`. The flow:

1. From the main menu select **Manage Wallets** and pick a wallet.
2. Choose **Sign Message** (EIP-191 `personal_sign` shape, so the
   resulting secp256k1 signature is `ecrecover`-compatible for
   Ethereum; ed25519 wallets produce raw signatures for Solana).
3. Broadcast a signing session; co-signers discover it via
   **Join Session** (same transport as DKG).
4. Each participant approves the message. Once threshold
   commitments + threshold shares arrive the aggregator emits
   the final signature.

Earlier drafts said "Select [3] Sign / load transaction from
file / 7-step flow". The numeric hotkey doesn't exist, and
transaction-from-file loading is not a shipped code path — signing
input is a hex-encoded message supplied in the SignTransaction
component.

## Configuration

The TUI has no config-file support today — all runtime settings are
passed as CLI flags. Keystore location is fixed at `~/.frost_keystore`.

### CLI Flags

```
--device-id <ID>            Device identity in the FROST mesh.
                            Defaults to the machine hostname.
--signal-server <URL>       WebSocket signal server.
                            Default: wss://xiongchenyu.dpdns.org
--offline                   Run without network (SD-card DKG mode).
--log-location <PATH>       Log file path.
                            Default: ~/.frost_keystore/logs/mpc-wallet.log
--log-level <LEVEL>         error | warn | info | debug | trace
                            Default: info
```

See `apps/tui-node/src/bin/mpc-wallet-tui.rs` for the authoritative
definitions.

### Environment Variables

Only two env vars are consulted:

| Variable           | Effect                                       |
|--------------------|----------------------------------------------|
| `HOME`             | Used to compute the keystore path.           |
| `PERF_MONITORING`  | If set (any value), enables perf counters.   |
| `RUST_LOG`         | Standard tracing-subscriber directive.       |

## Advanced Features

### Offline Mode

For air-gapped operations:

```bash
# Generate offline session
mpc-wallet-tui --offline --device-id Device-001

# Export session data
[4] Session → Export to File → session_001.json

# Transfer via secure medium (USB, QR code)
# Import on other device
[4] Session → Import from File → session_001.json
```

### Multi-Wallet Management

- Support for multiple wallets per device
- Easy switching between wallets
- Hierarchical deterministic (HD) wallet support
- Import/export keystore functionality

### Performance Optimization

The TUI includes several optimizations:
- Connection pooling for WebRTC
- Message batching for efficiency
- State caching to reduce computation
- Lazy loading of wallet data

## Troubleshooting

### Common Issues

#### Connection Problems
```bash
# Check signal server connectivity (TLS + HTTP, not a health endpoint —
# the server is WebSocket-only, so this just verifies reachability)
curl -v https://xiongchenyu.dpdns.org/

# Enable debug logging
RUST_LOG=debug mpc-wallet-tui --device-id Device-001
```

#### DKG Failures
- Ensure all participants are online
- Check network firewall settings
- Verify matching threshold/participant counts
- Review logs for timeout issues

#### Performance Issues
- Reduce UI refresh rate in config
- Disable animations for slow terminals
- Check system resources (CPU, memory)

## Security Considerations

- **Never share** your device ID password
- **Always verify** session IDs with participants
- **Use offline mode** for high-security operations
- **Regular backups** of keystore recommended
- **Encrypt keystores** at rest

## Resources

- [Architecture Documentation](architecture/)
- [User Guide](guides/USER_GUIDE.md)
- [Protocol Specification](protocol/)
- [UI Wireframes](ui/)
- [Main Project Docs](../../../docs/)

## Support

For help and support:
- [GitHub Issues](https://github.com/hecoinfo/mpc-wallet/issues)
- [Documentation](../../../docs/)

---

[← Back to Apps](../../) | [→ Architecture](architecture/)