# FROST MPC TUI Wallet Documentation

Welcome to the documentation for the FROST MPC TUI Wallet - a professional Terminal User Interface based Multi-Party Computation wallet for secure threshold signatures and distributed key management.

## 🎯 What is FROST MPC TUI Wallet?

The FROST MPC TUI Wallet transforms traditional command-line cryptocurrency operations into an intuitive, menu-driven experience. Built with enterprise-grade security similar to BitGo, it provides distributed key management through a beautiful terminal interface that requires zero command-line expertise.

### Key Differentiators

- **Full Terminal UI**: Navigate with arrow keys, no commands to memorize
- **Enterprise MPC**: True multi-party computation with threshold signatures
- **Visual Workflows**: Clear progress indicators and status updates
- **Dual Mode Operation**: 
  - 🌐 **Online Mode**: Real-time coordination via WebSocket/WebRTC
  - 🔒 **Offline Mode**: Air-gapped security with SD card data exchange
- **Professional Grade**: Audit trails, compliance features, and enterprise security

## 📚 Documentation Structure

### 📁 [User Guides](./guides/)
- **[User Guide](./guides/USER_GUIDE.md)** - Comprehensive user manual with visual examples
- **[Offline Mode Guide](./guides/offline-mode.md)** - Air-gapped operation procedures

### 📁 [Architecture Documentation](./architecture/)
- **[Architecture Overview](./architecture/ARCHITECTURE.md)** - Technical design and system components
- **[Elm Architecture](./architecture/ELM_ARCHITECTURE.md)** - Model/Update/View pattern used in the TUI
- **[DKG Flows](./architecture/DKG_FLOWS.md)** - Distributed key generation procedures
- **[Security Model](./architecture/SECURITY.md)** - Security analysis and best practices
- **[Keystore Design](./architecture/keystore_design.md)** - Keystore architecture details

### 📁 [Protocol Specifications](./protocol/)
- **[WebRTC Signaling](./protocol/01_webrtc_signaling.md)** - P2P communication protocol

## 🚀 Quick Start

```bash
# Build and run from the monorepo
cargo run --bin mpc-wallet-tui -p tui-node -- --device-id alice

# Navigate the TUI
↑/↓     Navigate menus
Enter   Select option
Esc     Go back
?       Show help
```

## 🔑 Core Features

### Multi-Party Computation
- **Distributed Key Generation**: No single party ever has the complete private key
- **Threshold Signatures**: Flexible schemes (2-of-3, 3-of-5, etc.)
- **Multi-Blockchain**: Native support for Ethereum and Solana

### Professional UI
- **Menu-Driven Interface**: No command memorization required
- **Real-Time Status**: Live updates on participant connectivity
- **Visual Progress**: Clear indicators for all operations
- **Context Help**: Press `?` anywhere for guidance

### Enterprise Security

Choose your security posture based on your requirements:

#### 🌐 Online/Hot-Wallet Mode
- **WebRTC Mesh Network**: Secure peer-to-peer communication
- **TLS 1.3 Encryption**: End-to-end encrypted channels
- **Real-time Coordination**: Instant multi-party operations
- **Best For**: Daily operations, trading, regular transactions

#### 🔒 Offline/Cold-Wallet Mode  
- **Complete Air-Gap**: No network interfaces active
- **SD Card Transfer**: Physical media for data exchange
- **Maximum Security**: Eliminates network attack vectors
- **Best For**: Cold storage, high-value assets, regulatory compliance

#### Both Modes Feature
- **Encrypted Storage**: AES-256-GCM encryption for key shares
- **Audit Trails**: Complete logging for compliance
- **Same Cryptography**: Identical FROST protocol implementation
- **Interoperability**: Seamless switching between modes

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────┐
│          Terminal UI Layer              │
│    (Ratatui + Event System)            │
├─────────────────────────────────────────┤
│        Business Logic Layer             │
│  (Session, Wallet, Transaction Mgmt)    │
├─────────────────────────────────────────┤
│          Network Layer                  │
│   (WebSocket, WebRTC, Offline)         │
├─────────────────────────────────────────┤
│       Cryptographic Core                │
│    (FROST Protocol, Keystore)          │
└─────────────────────────────────────────┘
```

## 🔐 Security Highlights

- **Zero Trust Architecture**: No single point of failure
- **Threshold Security**: Minimum participants required for any operation
- **Defense in Depth**: Multiple layers of security controls
- **Compliance Ready**: SOC 2, ISO 27001, GDPR support

## 🤝 Use Cases

### Enterprise Treasury
- Secure multi-signature wallets
- Distributed approval workflows
- Complete audit trails

### Institutional Custody
- Cold wallet operations
- Regulatory compliance
- Disaster recovery

### DeFi Operations
- Protocol governance
- Treasury management
- Cross-chain operations

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/hecoinfo/mpc-wallet/issues)
- **Security**: [GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new)

---

*Building the future of secure, distributed cryptocurrency management through beautiful terminal interfaces.*