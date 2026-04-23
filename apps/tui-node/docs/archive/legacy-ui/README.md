# TUI Node UI Documentation

This directory contains comprehensive user interface and user experience documentation for the MPC Wallet TUI (Terminal User Interface) application, designed to meet enterprise-grade requirements.

## Contents

### Core Flow Documentation
- `WALLET_FLOW_WIREFRAMES.md` - Main wallet flow wireframes including welcome, session management, DKG, and signing operations

The companion `keystore_session_ux_flow.md` wireframes live under
the sibling archive dir [`../legacy-keystore-docs/`](../legacy-keystore-docs/).

### Critical Enterprise Paths
- `KEY_ROTATION_WIREFRAMES.md` - Key rotation and participant management wireframes for security maintenance
- `EMERGENCY_RESPONSE_WIREFRAMES.md` - Emergency response system including threat detection, lockdown, and forensics
- `SETTINGS_CONFIGURATION_WIREFRAMES.md` - Comprehensive settings and configuration management screens

### Additional Documentation
- `NAVIGATION_FLOW.md` - Navigation patterns and keyboard shortcuts
- `SCREEN_SPECIFICATIONS.md` - Detailed screen specifications and layout guidelines
- `HELP_SYSTEM_WIREFRAMES.md` - Help and documentation system design
- `IMPORT_EXPORT_WIREFRAMES.md` - Import/export operations for wallets and configurations
- `OFFLINE_MODE_WIREFRAMES.md` - Offline mode operations and air-gapped functionality

## Overview

The TUI node provides a professional, enterprise-grade terminal-based interface for operating an MPC wallet node. The UI is built using the Ratatui framework and provides:

### Core Features
- Real-time status display with comprehensive monitoring
- Interactive command interface with intelligent navigation
- Advanced session management for DKG and signing operations
- Network status monitoring with connection health metrics
- Multi-wallet portfolio management

### Enterprise Features
- **Security & Compliance**: SOC 2, ISO 27001, GDPR compliance tracking
- **Emergency Response**: Threat detection, wallet lockdown, forensic analysis
- **Business Continuity**: Key rotation, backup/recovery, disaster recovery
- **Audit Trail**: Comprehensive logging and reporting for regulatory requirements
- **Multi-Environment**: Production, development, testing, and DR profiles

## Design Principles

1. **Professional Aesthetic**: BitGo-inspired design with clean, data-dense layouts
2. **Keyboard-First**: Full keyboard navigation with consistent shortcuts
3. **Information Hierarchy**: Progressive disclosure of complexity
4. **Real-Time Feedback**: Live status updates and progress indicators
5. **Error Prevention**: Confirmation dialogs for critical operations
6. **Accessibility**: Screen reader support and high-contrast modes

## Navigation Patterns

- Number keys (1-9) for menu selection
- Arrow keys for navigation
- Enter to confirm, Escape to go back
- Tab/Shift+Tab for form field navigation
- Function keys for quick actions (F1=Help, F5=Refresh, etc.)

## Related Documentation

- For implementation details, see the [architecture docs](../architecture/)
- For usage instructions, see the [user guides](../guides/)
- For API documentation, see the [API reference](../api/)