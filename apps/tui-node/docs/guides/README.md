# TUI Node User Guides

This directory contains user guides and how-to documentation for the MPC Wallet TUI application.

## Contents

- `USER_GUIDE.md` - Comprehensive user guide covering all TUI features and operations
- `keystore_sessions_user_guide.md` - Detailed guide for managing keystore sessions
- `offline-mode.md` - Instructions for using the TUI in offline mode

## Quick Start

The TUI node provides a terminal-based interface for:

1. **Creating/Joining Sessions**: Participate in DKG sessions to generate threshold keys
2. **Managing Keystores**: Import, export, and manage encrypted keystores
3. **Signing Transactions**: Participate in threshold signing operations
4. **Network Operations**: Monitor WebRTC/WebSocket connections

## Navigation

- Use keyboard shortcuts (displayed at bottom of screen)
- Tab through different UI sections
- Follow on-screen prompts for operations

## Known Issues & Recent Fixes

### Recent Fixes (2025-09-02)
1. **DKG Process Stuck Issue**: Fixed SimpleMessage parsing in WebRTC handler to properly process DKG Round 1/2 messages
2. **Compilation Warnings**: Fixed all unused variable warnings by properly prefixing with underscore
3. **UI Message Clarity**: Updated wallet creation success message to remove unimplemented 'v' key option
4. **DKG Round 2 Package Counting**: Fixed package counting logic to expect packages from (total - 1) participants, not including self
5. **User-Friendly DKG UI**: Redesigned DKG progress screen in tui.rs (correct file) to replace technical "Progress Log" with clear stage indicators and helpful tips
6. **Enter Key Workaround**: Added 'v' key as alternative to Enter for viewing wallet details after DKG completion
7. **Esc Key Fix**: Fixed Esc key to properly return to main menu when DKG is complete
8. **Duplicate UI Section**: Removed duplicate "Current Stage" section that was appearing twice
9. **Meaningful Wallet Info**: Now displays wallet ID, threshold, curve type, and Ethereum address when DKG completes
10. **Enhanced Debugging**: Added comprehensive logging for key events to diagnose input issues
11. **CRITICAL FIX - DKG Address Consistency**: Fixed critical bug where each MPC node was generating different wallet addresses. Now all nodes derive the same group public key and Ethereum address from the DKG session, ensuring proper threshold wallet functionality

## Related Documentation

- For technical architecture, see [architecture docs](../architecture/)
- For historical UI/UX wireframes (pre-componentization), see [archive/legacy-ui/](../archive/legacy-ui/)