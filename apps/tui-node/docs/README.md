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

```bash
mpc-wallet-tui [OPTIONS]

OPTIONS:
    --device-id <ID>         Unique device identifier [required]
    --config <PATH>          Path to configuration file
    --keystore <PATH>        Path to keystore directory
    --signal-server <URL>    WebSocket signal server URL
    --offline               Run in offline mode
    --log-level <LEVEL>     Logging level (debug, info, warn, error)
    --help                  Show help information
```

## User Interface

### Main Screen Layout

```
┌──────────────────────────────────────────────────┐
│  MPC Wallet TUI v0.1.0 - Device: Device-001     │
├─────────────┬────────────────────────────────────┤
│   Menu      │        Main Content                │
│             │                                    │
│ [1] Wallet  │  Current Wallet: my_wallet        │
│ [2] DKG     │  Address: 0x742d35Cc6634C053...  │
│ [3] Sign    │  Balance: 1.234 ETH               │
│ [4] Session │                                    │
│ [5] Network │  Peers: 2/3 connected             │
│ [6] Settings│  Session: Active                  │
│             │                                    │
│ [Q] Quit    │  Press ? for help                 │
└─────────────┴────────────────────────────────────┘
│ Status: Connected | Ready for operations         │
└──────────────────────────────────────────────────┘
```

### Navigation

| Key | Action |
|-----|--------|
| ↑/↓/←/→ | Navigate menu items |
| Enter | Select option |
| Esc | Go back / Cancel |
| Tab | Switch panels |
| Space | Toggle selection |
| ? | Show context help |
| Q | Quit application |

## Core Workflows

### Creating a Wallet

1. Select **[1] Wallet** from main menu
2. Choose **Create New Wallet**
3. Enter wallet details:
   - Name
   - Threshold (e.g., 2)
   - Participants (e.g., 3)
   - Blockchain (Ethereum/Solana)
4. Share session ID with other participants
5. Wait for all participants to join
6. DKG process starts automatically
7. Wallet created and saved to keystore

### Distributed Key Generation (DKG)

```
Online DKG:
Device A ──┐
           ├──► Signal Server ──► DKG Protocol ──► Key Shares
Device B ──┘

Offline DKG:
Device A ──► Export ──► USB ──► Device B
                         ↓
                    Process Offline
                         ↓
Device B ──► Export ──► USB ──► Device A
```

### Transaction Signing

1. Select **[3] Sign** from menu
2. Choose wallet to use
3. Enter transaction details or load from file
4. Initiate signing session
5. Wait for threshold participants
6. Review and confirm transaction
7. Signature generated and displayed

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