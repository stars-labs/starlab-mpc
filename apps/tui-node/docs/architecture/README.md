# TUI Node Architecture Documentation

This directory contains technical architecture and design documentation for the MPC Wallet TUI application.

## Contents

- `01_keystore_design.md` - Keystore architecture and design decisions
- `ARCHITECTURE.md` - Overall system architecture of the TUI application
- `DKG_FLOWS.md` - Detailed DKG protocol flows and state machines
- `SECURITY.md` - Security considerations and threat model
- `keystore_session_final_design.md` - Final design for keystore session management
- `keystore_session_recovery.md` - Session recovery mechanisms
- `keystore_sessions_implementation.md` - Implementation details for keystore sessions
- `simplified_keystore_session_design.md` - Simplified design approach

## Architecture Overview

The TUI node is built with:

- **Rust** - Core implementation language
- **Ratatui** - Terminal UI framework
- **Tokio** - Async runtime
- **WebRTC/WebSocket** - P2P communication
- **FROST** - Threshold signature scheme

## Key Components

1. **UI Layer** - Terminal interface using Ratatui
2. **Business Logic** - Session management, DKG, signing
3. **Network Layer** - WebRTC mesh and WebSocket signaling
4. **Storage Layer** - Encrypted keystore management

## Related Documentation

- For user guides, see [guides](../guides/)
- For historical UI/UX wireframes (pre-componentization), see [archive/legacy-ui/](../archive/legacy-ui/)