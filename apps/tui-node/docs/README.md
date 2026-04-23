# Terminal UI Node Documentation

## Overview

The MPC Wallet Terminal UI (TUI) Node is a professional-grade terminal application for managing multi-party computation wallets. Built with Rust and Ratatui, it provides an intuitive interface for distributed key generation, threshold signing, and secure wallet management without requiring command-line expertise.

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

### Enterprise Features
- **Session Management**: Persistent sessions with automatic reconnection
- **Audit Logging**: Complete activity and transaction history
- **Multi-Wallet Support**: Manage multiple wallets and accounts
- **Import/Export**: Compatible with browser extension and other implementations
- **Performance Optimization**: Connection pooling and message batching

## Quick Start

### Prerequisites
- Rust 1.70+ toolchain
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

### Configuration File

Location: `~/.mpc-wallet/config.toml`

```toml
[network]
signal_server = "wss://xiongchenyu.dpdns.org"
stun_servers = ["stun:stun.l.google.com:19302"]
enable_turn = false

[keystore]
path = "~/.mpc-wallet/keystores"
encryption = "aes-256-gcm"
auto_backup = true

[ui]
theme = "dark"
refresh_rate = 60
show_animations = true

[security]
require_password = true
session_timeout = 900
max_login_attempts = 3
```

### Environment Variables

```bash
export MPC_WALLET_CONFIG=/path/to/config.toml
export MPC_WALLET_KEYSTORE=/secure/location
export RUST_LOG=info
```

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