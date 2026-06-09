# Archived Legacy Keystore Documentation

This directory contains archived documentation from the previous keystore design that stored redundant blockchain information in wallet files.

## Why These Were Archived

As of September 2024, the keystore was simplified following KISS (Keep It Simple, Stupid) and Orthogonal principles:

- **Old Design**: Stored blockchain addresses, networks, chain IDs in wallet files
- **New Design**: Derives all addresses from FROST group public key + curve type

## Archived Documents

- `01_keystore_design.md` - Original complex keystore architecture
- `keystore_session_*.md` - Session-based keystore management (overcomplicated)
- `simplified_keystore_session_design.md` - Intermediate simplification attempt
- `02_keystore_sessions.md` - Protocol specification for sessions
- `keystore_sessions_user_guide.md` - User guide for session management
- `keystore_session_ux_flow.md` - UX flow for session management

## Current Design

See `/apps/tui/docs/architecture/keystore_design.md` for the current simplified design.

## Key Improvements in New Design

1. **Single Source of Truth**: Group public key determines all addresses
2. **No Redundancy**: Addresses are derived, not stored
3. **Smaller Files**: ~50% reduction in file size
4. **Future-Proof**: New blockchains supported without schema changes
5. **Simpler Code**: Less complexity to maintain

These documents are kept for historical reference only.