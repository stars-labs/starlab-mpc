# MPC Wallet TUI Screen Specifications

This document provides detailed specifications for each screen in the MPC wallet TUI application.

## Screen Components Overview

### Common Elements

All screens share these common elements:

1. **Header Bar**
   - Application title and current screen name
   - Navigation breadcrumbs (where applicable)
   - Mode indicator (Online/Offline)
   - Additional context (curve type, session info)

2. **Status Bar**
   - Device ID
   - Connection status indicator
   - Network mode
   - Additional real-time metrics

3. **Footer Bar**
   - Context-sensitive keyboard shortcuts
   - Navigation hints
   - Help access

4. **Color Scheme**
   - Primary: Cyan (#00CED1) - Headers, selections
   - Success: Green (#00FF00) - Connected, complete
   - Warning: Yellow (#FFD700) - In progress, warnings
   - Error: Red (#FF6B6B) - Errors, disconnected
   - Info: White (#FFFFFF) - General text
   - Muted: Gray (#808080) - Help text, disabled

---

## Screen Specifications

### 1. Welcome Screen

**Purpose**: Entry point for all user interactions

**Components**:
- Logo/branding display
- Main menu with numbered options
- Device ID display
- Connection status indicator

**States**:
- Connected (green indicator)
- Connecting (yellow indicator)
- Disconnected (red indicator)

**Interactions**:
- Number keys (1-5): Quick selection
- Arrow keys: Navigate menu
- Enter: Confirm selection
- Q: Quit application
- ?: Show help

**Validation**: None required

**Error Handling**: 
- Show connection errors in status area
- Maintain menu functionality even when disconnected

---

### 2. Mode Selection Screen

**Purpose**: Choose between online and offline operation modes

**Components**:
- Two card-style selection panels
- Feature comparison lists
- Current mode indicator
- Back navigation

**States**:
- Online mode selected (default)
- Offline mode selected
- Transitioning between modes

**Interactions**:
- Left/Right arrows: Switch between modes
- Enter: Confirm selection
- Esc: Return to previous screen

**Validation**:
- Check network connectivity before allowing online mode
- Warn if switching from online to offline with active sessions

**Error Handling**:
- Gracefully handle network disconnection
- Suggest offline mode if network unavailable

---

### 3. Curve Selection Screen

**Purpose**: Select cryptographic curve for new wallet

**Components**:
- List of supported curves with descriptions
- Use case examples for each curve
- Warning about permanence of choice
- Selected curve indicator

**States**:
- No selection (initial)
- Curve selected
- Confirmation pending

**Interactions**:
- Up/Down arrows: Navigate curves
- Enter: Select and continue
- Esc: Return to mode selection

**Validation**:
- Ensure curve selection before proceeding
- Display compatibility warnings if needed

**Error Handling**:
- Handle unsupported curve gracefully
- Show clear error if curve initialization fails

---

### 4. Create Session Screen

**Purpose**: Configure and initiate new DKG session

**Components**:
- Form fields:
  - Session name (text input)
  - Total participants (numeric input, 2-10)
  - Threshold (numeric input, 1 to total)
  - Participant IDs (list input)
  - Description (optional text)
- Checkbox options:
  - Auto-accept participants
  - Enable timeout
- Validation status display

**States**:
- Empty form (initial)
- Partially filled
- Valid (all required fields correct)
- Invalid (validation errors shown)
- Submitting

**Interactions**:
- Tab/Shift+Tab: Navigate fields
- Text input in active field
- Space: Toggle checkboxes
- Enter: Submit when valid
- Esc: Cancel and return

**Validation**:
- Session name: Required, unique, alphanumeric
- Total participants: 2-10
- Threshold: 1 to total participants
- Participant count must match total
- All participant IDs must be unique

**Error Handling**:
- Inline validation messages
- Prevent submission of invalid data
- Handle duplicate session names

---

### 5. Join Session Screen

**Purpose**: Browse and join available sessions

**Components**:
- List of available sessions with:
  - Session ID
  - Type (DKG/Signing)
  - Participant count
  - Status
  - Age
- Manual session ID input field
- Auto-refresh indicator

**States**:
- Loading sessions
- Sessions displayed
- No sessions available
- Manual entry mode
- Joining session

**Interactions**:
- Up/Down arrows: Navigate session list
- Enter: Join selected session
- Tab: Switch to manual entry
- R: Refresh list
- Esc: Return to menu

**Validation**:
- Verify session exists before joining
- Check if already participant
- Validate manual session ID format

**Error Handling**:
- Handle session not found
- Display if session is full
- Show if session has expired

---

### 6. DKG Progress Screen

**Purpose**: Show real-time progress of key generation

**Components**:
- Phase progress bars (4 phases)
- Participant status list
- Time elapsed/remaining
- Connection indicators
- Message counter

**States**:
- Initializing
- Phase 1: Connection setup
- Phase 2: Mesh formation
- Phase 3: FROST round 1
- Phase 4: FROST round 2
- Completed
- Failed

**Interactions**:
- L: View detailed logs
- A: Abort process (with confirmation)
- ?: Show help for current phase

**Validation**: None (display only)

**Error Handling**:
- Show participant disconnections
- Display protocol errors clearly
- Offer recovery options on failure

---

### 7. Signing Progress Screen

**Purpose**: Monitor transaction signing process

**Components**:
- Transaction details display
- Signing progress bar
- Required signatures tracker
- Participant signature status
- Timeout countdown

**States**:
- Awaiting signatures
- Collecting signatures
- Threshold reached
- Completed
- Failed/Timeout

**Interactions**:
- D: Show raw transaction data
- C: Cancel signing (with confirmation)
- ?: Show help

**Validation**: None (display only)

**Error Handling**:
- Handle participant timeouts
- Show signature validation errors
- Provide clear failure reasons

---

### 8. Wallet Management Screen

**Purpose**: View and manage all wallets

**Components**:
- Wallet list with:
  - Name
  - Type (threshold config)
  - Curve
  - Address
  - Creation date
  - Last used
  - Status
  - Balance (if available)
- Action buttons

**States**:
- Loading wallets
- Wallets displayed
- No wallets
- Action in progress

**Interactions**:
- Up/Down arrows: Select wallet
- Enter: View details
- S: Initiate signing
- E: Export wallet
- D: Delete (with confirmation)
- Esc: Return to menu

**Validation**:
- Confirm destructive actions
- Verify wallet accessibility

**Error Handling**:
- Handle missing wallet files
- Show if wallet is corrupted
- Display network errors for balance

---

### 9. Error Screens

#### Connection Error Screen

**Components**:
- Error icon/indicator
- Error message
- Technical details
- Troubleshooting steps
- Action buttons
- Retry counter

**Interactions**:
- R: Retry connection
- O: Switch to offline mode
- L: View detailed logs
- Q: Quit application

#### Session Recovery Screen

**Components**:
- Session information
- Progress snapshot
- Recovery options
- Participant status

**Interactions**:
- Number keys: Select recovery option
- Enter: Confirm choice
- ?: Show recovery help

#### Critical Error Screen

**Components**:
- Error type and code
- User-friendly message
- Technical stack trace
- Timestamp
- Report generation option

**Interactions**:
- S: Save error report
- L: View full logs
- R: Restart application
- Q: Quit

---

## Accessibility Features

1. **Keyboard Navigation**
   - Full keyboard control
   - Consistent shortcut keys
   - Tab order follows visual flow

2. **Visual Indicators**
   - High contrast colors
   - Clear status symbols
   - Progress indicators

3. **Help System**
   - Context-sensitive help
   - Inline hints
   - Comprehensive documentation

4. **Error Messages**
   - Clear, actionable language
   - Technical details on demand
   - Recovery suggestions

---

## Performance Considerations

1. **Refresh Rates**
   - Status updates: 1 second
   - Progress bars: 500ms
   - Network status: 2 seconds

2. **Resource Usage**
   - Minimal CPU when idle
   - Efficient screen redraws
   - Bounded log storage

3. **Responsiveness**
   - Immediate key response
   - Non-blocking operations
   - Progress indicators for long tasks