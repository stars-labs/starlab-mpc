# MPC Wallet TUI Navigation Flow

This document describes the navigation patterns and state transitions in the MPC wallet TUI application.

## Navigation Flow Diagram

```
┌─────────────────┐
│  Welcome Screen │ ◄─────────────────────────────────────────┐
└────────┬────────┘                                           │
         │                                                    │
    ┌────┴────┬──────┬──────┬──────┬──────┬──────┬────────┬─┴────┐
    │         │      │      │      │      │      │        │      │
    ▼         ▼      ▼      ▼      ▼      ▼      ▼        ▼      ▼
┌────────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐
│ Create │ │ Join │ │Select│ │Backup│ │ Set- │ │Audit │ │ Key  │ │Emerg.│
│ Wallet │ │ Sess │ │Wallet│ │ Rec. │ │tings │ │Compl.│ │ Rot. │ │ Resp.│
└────┬───┘ └───┬──┘ └───┬──┘ └───┬──┘ └───┬──┘ └───┬──┘ └───┬──┘ └───┬──┘
     │         │        │        │        │        │        │        │
     ▼         │        │        │        │        │        │        │
┌─────────┐    │        │        │        │        │        │        │
│ Submenu │    │        │        │        │        │        │        │
│ Options │    │        │        │        │        │        │        │
└────┬────┘    │        │        │        │        │        │        │
     │         │        │        │        │        │        │        │
     ▼         ▼        ▼        ▼        ▼        ▼        ▼        ▼
┌──────────────────────────────────────────────────────────────────────┐
│                    Context-Specific Submenus                         │
│  (Quick DKG, Custom Setup, Portfolio View, Network Settings, etc.)   │
└──────────┬───────────────────────────────────────────────────────────┘
           │
           ▼
┌──────────────────────┐
│  Action/Progress     │
│  (DKG/Sign/Config)   │
└──────────┬───────────┘
           │
     ┌─────┴─────┐
     ▼           ▼
┌─────────┐ ┌─────────┐
│ Success │ │  Error  │
│ (Return)│ │(Recovery)│
└─────────┘ └─────────┘
```

## Comprehensive Menu Structure

### Main Menu (Welcome Screen)
```
[1] Create New Wallet
    ├─ [1] Quick DKG Session
    ├─ [2] Custom DKG Setup
    ├─ [3] Multi-Chain Wallet
    ├─ [4] Enterprise Setup
    └─ [5] Offline DKG

[2] Join Wallet Session
    ├─ Available Sessions List
    ├─ [M] Manual Entry
    ├─ [F] Filter Sessions
    └─ [D] Session Details

[3] Select Existing Wallet
    ├─ Wallet Portfolio View
    └─ Wallet Operations:
        ├─ [1] Send Transaction
        ├─ [2] Sign Message
        ├─ [3] Sign Typed Data
        ├─ [4] Multi-Chain Sign
        ├─ [5] Manage Participants
        ├─ [6] Rotate Keys
        ├─ [7] Lock/Unlock Wallet
        ├─ [8] View Activity Log
        ├─ [9] Test Connections
        ├─ [A] Export Details
        └─ [B] Advanced Settings

[4] Backup & Recovery
    ├─ [1] Full Keystore Backup
    ├─ [2] Individual Wallet Export
    ├─ [3] Configuration Export
    ├─ [4] Encrypted Backup
    ├─ [5] Import Keystore
    ├─ [6] Import Single Wallet
    ├─ [7] Import from CLI
    ├─ [8] Import from Browser
    ├─ [9] Disaster Recovery
    └─ [A] Repair Corrupted Data

[5] Settings & Configuration
    ├─ [1] Network Settings
    ├─ [2] WebRTC Configuration
    ├─ [3] Security Policies
    ├─ [4] Connection Profiles
    ├─ [5] Display Preferences
    ├─ [6] Keyboard Shortcuts
    ├─ [7] Notifications
    ├─ [8] Language & Region
    ├─ [9] Data Management
    ├─ [A] Auto-Update Settings
    ├─ [B] Logging & Diagnostics
    └─ [C] Enterprise Policies

[6] Audit & Compliance
    ├─ [1] View Audit Logs
    ├─ [2] Generate Reports
    ├─ [3] Search & Filter Logs
    ├─ [4] Export Audit Data
    ├─ [5] SOC 2 Compliance
    ├─ [6] ISO 27001 Standards
    ├─ [7] GDPR Requirements
    ├─ [8] Financial Regulations
    ├─ [9] Security Events
    ├─ [A] Risk Assessment
    ├─ [B] Access Review
    └─ [C] Incident Documentation

[7] Key Rotation & Management
    ├─ [1] Rotate Key Shares
    ├─ [2] Update Participants
    ├─ [3] Change Threshold
    ├─ [4] Migrate Curves
    ├─ [5] Add Participant
    ├─ [6] Remove Participant
    ├─ [7] Replace Participant
    ├─ [8] Verify Participants
    ├─ [9] Emergency Key Freeze
    ├─ [A] Emergency Recovery
    ├─ [B] Key Health Analysis
    └─ [C] Rotation History

[8] Emergency Response
    ├─ [1] EMERGENCY LOCKDOWN
    ├─ [2] SECURITY INCIDENT
    ├─ [3] REVOKE ACCESS
    ├─ [4] EMERGENCY CONTACTS
    ├─ [5] FORENSIC ANALYSIS
    ├─ [6] THREAT ASSESSMENT
    ├─ [7] INCIDENT DOCUMENTATION
    ├─ [8] RECOVERY PROCEDURES
    ├─ [9] BACKUP ACTIVATION
    ├─ [A] DISASTER RECOVERY
    ├─ [B] SYSTEM HEALTH CHECK
    └─ [C] STAKEHOLDER NOTIFY

[9] Multi-Wallet Operations
    ├─ [1] Batch Signing
    ├─ [2] Portfolio Rebalancing
    ├─ [3] Consolidated Reporting
    ├─ [4] Batch Key Rotation
    ├─ [5] Portfolio Dashboard
    ├─ [6] Total Asset Valuation
    ├─ [7] Transaction History
    ├─ [8] Risk Assessment
    ├─ [9] Cross-Chain Transfers
    ├─ [A] DEX Aggregation
    ├─ [B] Yield Farming
    └─ [C] Tax Reporting

[H] Help & Documentation
    ├─ [1] Getting Started
    ├─ [2] User Guide
    ├─ [3] Quick Tips
    ├─ [4] Keyboard Shortcuts
    ├─ [5] Technical Reference
    ├─ [6] Security Best Practices
    ├─ [7] Network Configuration
    ├─ [8] Enterprise Features
    ├─ [9] Diagnostic Tools
    ├─ [A] Support Resources
    ├─ [B] Report Issue
    ├─ [C] FAQ
    ├─ [D] About MPC Wallet
    └─ [E] Legal & Compliance

[Q] Quit Application
```

## Screen Transition Rules

### From Welcome Screen

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Select [1] "Create New Wallet" | Create Wallet Submenu | Always |
| Select [2] "Join Wallet Session" | Join Session Screen | Network connected |
| Select [3] "Select Existing Wallet" | Wallet Portfolio | Always |
| Select [4] "Backup & Recovery" | Backup/Recovery Menu | Always |
| Select [5] "Settings & Configuration" | Settings Menu | Always |
| Select [6] "Audit & Compliance" | Audit Menu | Always |
| Select [7] "Key Rotation & Management" | Key Management Menu | Always |
| Select [8] "Emergency Response" | Emergency Menu | Always |
| Select [9] "Multi-Wallet Operations" | Multi-Wallet Menu | Always |
| Select [H] "Help & Documentation" | Help Menu | Always |
| Press Q | Quit Confirmation | Always |

### From Create Wallet Submenu

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Select [1] "Quick DKG Session" | Quick DKG Setup | Always |
| Select [2] "Custom DKG Setup" | Custom DKG Form | Always |
| Select [3] "Multi-Chain Wallet" | Chain Selection | Always |
| Select [4] "Enterprise Setup" | Enterprise Config | Always |
| Select [5] "Offline DKG" | Offline Instructions | Always |
| Press Esc | Welcome Screen | Always |

### From Wallet Portfolio

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Select wallet + Enter | Wallet Operations Menu | Wallet exists |
| Press [D] | Wallet Details View | Wallet selected |
| Press [S] | Sort Options | Multiple wallets |
| Press [F] | Filter Dialog | Multiple wallets |
| Press [N] | Create Wallet Submenu | Always |
| Press [I] | Import Wallet | Always |
| Press Esc | Welcome Screen | Always |

### From Wallet Operations Menu

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Select [1-9,A,B] + Enter | Specific Operation Screen | Valid selection |
| Press [Q] | Quick Sign Dialog | Signing available |
| Press Esc | Wallet Portfolio | Always |

### From Mode Selection

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Select Online + Enter | Curve Selection | Network available |
| Select Offline + Enter | Curve Selection | Always |
| Press Esc | Welcome Screen | Always |

### From Curve Selection

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Select Curve + Enter | Create Session | Valid selection |
| Press Esc | Mode Selection | Always |

### From Create Session

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Fill Form + Enter | DKG Progress | All fields valid |
| Press Esc | Curve Selection | Confirm if data entered |

### From Join Session

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Select Session + Enter | Progress Screen | Session available |
| Enter Manual ID | Progress Screen | Valid session ID |
| Press Esc | Welcome Screen | Always |

### From Progress Screens

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Process Completes | Success → Welcome | Automatic |
| Process Fails | Error Recovery | Automatic |
| Press A (Abort) | Confirm → Welcome | User confirmation |

### From Error Screens

| User Action | Destination | Condition |
|------------|-------------|-----------|
| Retry | Previous Screen | Network available |
| Switch to Offline | Mode Selection | From connection error |
| Resume Session | Progress Screen | Session recoverable |
| Abandon | Welcome Screen | Always |

## State Management

### Application States

```rust
enum AppState {
    Welcome,
    ModeSelection { previous: Box<AppState> },
    CurveSelection { mode: OperationMode },
    CreateSession { mode: OperationMode, curve: CurveType },
    JoinSession,
    SessionProgress { session: Session },
    WalletManagement,
    ImportExport,
    Error { error: ErrorInfo, recovery: RecoveryOptions },
}
```

### Session States

```rust
enum SessionState {
    Initializing,
    AwaitingParticipants,
    MeshForming,
    DkgInProgress { phase: u8, round: u8 },
    SigningInProgress { collected: u8, required: u8 },
    Completed,
    Failed { reason: String },
}
```

### Navigation Stack

The application maintains a navigation stack for proper back button behavior:

```
NavigationStack: Vec<AppState>

Operations:
- push(): Add new state when navigating forward
- pop(): Remove state when going back
- peek(): View current state without modifying
- clear(): Reset to welcome screen
```

## Keyboard Navigation Map

### Global Keys (Available on all screens)

| Key | Action | Notes |
|-----|--------|-------|
| ? | Show contextual help | Overlay on current screen |
| Esc | Go back/Cancel | Context sensitive |
| Ctrl+C | Force quit | With confirmation if active session |
| Ctrl+L | Clear screen | Redraw current view |

### Navigation Keys by Screen

#### Welcome Screen
- `1-5`: Quick number selection
- `↑↓`: Navigate menu items
- `Enter`: Confirm selection
- `Q`: Quit application

#### Form Screens (Create Session, etc.)
- `Tab`: Next field
- `Shift+Tab`: Previous field
- `Space`: Toggle checkbox
- `Enter`: Submit (when valid)
- `Esc`: Cancel

#### List Screens (Join Session, Wallet Management)
- `↑↓`: Navigate items
- `Page Up/Down`: Navigate pages
- `Enter`: Select item
- `R`: Refresh list
- `Esc`: Back

#### Progress Screens
- `L`: Toggle log view
- `D`: Show details
- `A`: Abort (with confirmation)
- `P`: Pause/Resume (if supported)

## Modal Dialogs and Popups

### Confirmation Dialogs

Used for destructive actions:
```
┌─────────────────────────┐
│    Confirm Action       │
├─────────────────────────┤
│ Abort DKG process?      │
│                         │
│ This cannot be undone.  │
│                         │
│ [Y]es  [N]o (default)   │
└─────────────────────────┘
```

### Input Dialogs

For manual data entry:
```
┌─────────────────────────┐
│   Enter Session ID      │
├─────────────────────────┤
│ Session ID:             │
│ [________________]      │
│                         │
│ [Enter] OK  [Esc] Cancel│
└─────────────────────────┘
```

### Progress Overlays

Non-blocking status updates:
```
┌─────────────────────────┐
│   Connecting...         │
├─────────────────────────┤
│ ████████░░░░░░░ 50%     │
│                         │
│ Establishing connection │
│ to signaling server     │
└─────────────────────────┘
```

## State Persistence

### Saved States

The following states are persisted between sessions:
- Device ID
- Preferred mode (online/offline)
- Recent sessions
- Window preferences
- Completed wallet list

### Session Recovery

On unexpected exit, the application can recover:
1. Active DKG sessions (within timeout)
2. Pending signing requests
3. Partial form data
4. Connection state

### Recovery Flow

```
Start Application
       │
       ▼
Check for saved state
       │
   ┌───┴───┐
   │Found? │
   └───┬───┘
     Y │ N
   ┌───┴───┐
   ▼       ▼
Recovery  Welcome
Screen    Screen
```

## Navigation Best Practices

1. **Consistency**
   - Same keys perform same actions across screens
   - Esc always goes back or cancels
   - Enter always confirms or submits

2. **Feedback**
   - Show navigation breadcrumbs where helpful
   - Indicate current position in lists
   - Show loading states during transitions

3. **Safety**
   - Confirm destructive actions
   - Save form data on navigation
   - Warn before losing unsaved changes

4. **Efficiency**
   - Provide keyboard shortcuts for common actions
   - Remember last selections
   - Allow quick navigation to frequent screens

5. **Error Handling**
   - Always provide a way back to safety
   - Show clear error messages
   - Offer recovery options when possible