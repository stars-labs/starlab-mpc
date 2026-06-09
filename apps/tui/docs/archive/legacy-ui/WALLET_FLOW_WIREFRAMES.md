# MPC Wallet TUI Flow Wireframes

This document contains ASCII-art wireframes for all screens in the MPC wallet TUI application, organized by the main user flow.

## Table of Contents

1. [Welcome Screen](#welcome-screen)
2. [Select Wallet Screen](#select-wallet-screen)
3. [Mode Selection Screen](#mode-selection-screen)
4. [Curve Selection Screen](#curve-selection-screen)
5. [Session Configuration Screens](#session-configuration-screens)
   - [Create Session Screen](#create-session-screen)
   - [Join Session Screen - Online](#join-session-screen-online)
   - [Join Session Screen - Offline](#join-session-screen-offline)
6. [Progress Screens](#progress-screens)
   - [DKG Progress Screen](#dkg-progress-screen)
   - [Signing Progress Screen](#signing-progress-screen)
7. [Wallet Management Screen](#wallet-management-screen)
8. [Recovery & Backup Path](#recovery--backup-path)
9. [Settings & Configuration Path](#settings--configuration-path)
10. [Audit & Compliance Path](#audit--compliance-path)
11. [Emergency Response Path](#emergency-response-path)
12. [Multi-Wallet Operations Path](#multi-wallet-operations-path)
13. [Error and Recovery Screens](#error-and-recovery-screens)

---

## Welcome Screen

Initial screen where user selects their primary action.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          MPC WALLET - WELCOME                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                           ╔════════════════════╗                           │
│                           ║   MPC WALLET v2.0  ║                           │
│                           ╚════════════════════╝                           │
│                                                                             │
│                    Distributed Key Generation & Signing                     │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Please select an option:                                       │     │
│   │                                                                   │     │
│   │  > [1] Create New Wallet (Start DKG Process)                   │     │
│   │    [2] Join Wallet Session (Participate in DKG/Signing)        │     │
│   │    [3] Select Existing Wallet (Access stored wallets)          │     │
│   │    [4] Backup & Recovery (Import/Export/Restore)               │     │
│   │    [5] Settings & Configuration                                 │     │
│   │    [6] Audit & Compliance                                       │     │
│   │    [7] Key Rotation & Management                               │     │
│   │    [8] Emergency Response                                       │     │
│   │    [9] Multi-Wallet Operations                                  │     │
│   │    [H] Help & Documentation                                     │     │
│   │    [Q] Quit Application                                         │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Device ID: mpc-node-001                            Status: ● Connected      │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓/1-9] Navigate  [Enter] Select  [Q] Quit  [?] Help                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Create New Wallet Submenu

Submenu displayed when selecting "Create New Wallet" from the welcome screen.

```
┌─ Create New Wallet ──────────────────────────────────────────────┐
│                                                                  │
│ [1] Quick DKG Session     Create standard 2-of-3 threshold      │
│ [2] Custom DKG Setup      Advanced threshold configuration      │
│ [3] Multi-Chain Wallet    Support multiple blockchains         │
│ [4] Enterprise Setup      Batch operations and policies         │
│ [5] Offline DKG           Air-gapped key generation             │
│                                                                  │
│ Recent Sessions: wallet_2of3_20250112, company_keys_20250111    │
│                                                                  │
│ [R] Recent  [T] Templates  [H] Help  [Esc] Back                 │
└──────────────────────────────────────────────────────────────────┘
```

---

## Select Wallet Screen (Portfolio View)

Screen for selecting an existing wallet from stored wallets.

```
┌─ Wallet Portfolio ───────────────────────────────────────────────┐
│                                                                  │
│ Your Wallets (5 total):                         💾 Keystore OK  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ [1] 🏦 company_treasury        2-of-3    ETH: 15.7         │  │
│ │     Created: 2025-01-10        Active    BTC: 0.25         │  │
│ │     Last used: 2 hours ago     secp256k1                   │  │
│ │                                                             │  │
│ │ [2] 🚀 project_alpha           3-of-5    ETH: 2.1          │  │
│ │     Created: 2025-01-08        Active    USDC: 1000        │  │
│ │     Last used: Yesterday       secp256k1                   │  │
│ │                                                             │  │
│ │ [3] 💼 personal_backup         2-of-2    SOL: 45.2         │  │
│ │     Created: 2024-12-15        Active    ed25519           │  │
│ │     Last used: 1 week ago                                  │  │
│ │                                                             │  │
│ │ [4] 🔒 emergency_funds         4-of-7    BTC: 1.5          │  │
│ │     Created: 2024-11-20        Locked    secp256k1         │  │
│ │     Last used: 1 month ago     [Unlock required]           │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [Enter] Select  [D] Details  [S] Sort  [F] Filter              │
│ [N] New wallet  [I] Import  [Esc] Back                         │
└──────────────────────────────────────────────────────────────────┘
```

### Wallet Operations Submenu

Displayed after selecting a specific wallet.

```
┌─ Wallet Operations: company_treasury ────────────────────────────┐
│                                                                  │
│ Available Operations:                                            │
│                                                                  │
│ Signing Operations:                                              │
│ [1] 📤 Send Transaction         Initiate outbound transfer      │
│ [2] ✍️  Sign Message            Sign arbitrary message          │
│ [3] 📋 Sign Typed Data          EIP-712 structured signing      │
│ [4] 🔄 Multi-Chain Sign         Cross-chain transaction         │
│                                                                  │
│ Wallet Management:                                               │
│ [5] 👥 Manage Participants      Add/remove signers              │
│ [6] 🔑 Rotate Keys              Generate new key shares         │
│ [7] 🔒 Lock/Unlock Wallet       Security state management       │
│ [8] 📊 View Activity Log        Transaction history             │
│                                                                  │
│ Maintenance:                                                     │
│ [9] 🧪 Test Connections         Verify participant status       │
│ [A] 📋 Export Details           Backup wallet information       │
│ [B] ⚙️  Advanced Settings       Technical configuration         │
│                                                                  │
│ [Enter] Select operation  [Q] Quick sign  [Esc] Back           │
└──────────────────────────────────────────────────────────────────┘
```

---

## Mode Selection Screen

User chooses between online and offline mode for operations.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         MODE SELECTION                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                                                          │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Select operation mode:                                          │     │
│   │                                                                   │     │
│   │  ┌───────────────────────┐   ┌───────────────────────┐         │     │
│   │  │    > ONLINE MODE      │   │     OFFLINE MODE      │         │     │
│   │  │                       │   │                       │         │     │
│   │  │ • Real-time comms     │   │ • Air-gapped signing  │         │     │
│   │  │ • WebRTC connections  │   │ • QR code exchange    │         │     │
│   │  │ • Instant DKG/signing │   │ • Enhanced security   │         │     │
│   │  │ • Multi-party sync    │   │ • Manual coordination │         │     │
│   │  │                       │   │                       │         │     │
│   │  │ Recommended for most  │   │ For high-security     │         │     │
│   │  │ use cases             │   │ environments          │         │     │
│   │  └───────────────────────┘   └───────────────────────┘         │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Current Mode: Online                              Network: ● Connected      │
├─────────────────────────────────────────────────────────────────────────────┤
│ [←→] Switch Mode  [Enter] Confirm  [Esc] Back  [?] Help                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Curve Selection Screen

Only shown for wallet creation path - user selects cryptographic curve.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CURVE SELECTION                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Mode Selection                                    [Online Mode]   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Select cryptographic curve for your wallet:                     │     │
│   │                                                                   │     │
│   │  ┌───────────────────────────────────────────────────────────┐  │     │
│   │  │  > secp256k1 (Ethereum, Bitcoin)                          │  │     │
│   │  │    • Used by: Ethereum, Bitcoin, BSC, Polygon            │  │     │
│   │  │    • ECDSA signatures                                     │  │     │
│   │  │    • Most widely supported                                │  │     │
│   │  └───────────────────────────────────────────────────────────┘  │     │
│   │                                                                   │     │
│   │  ┌───────────────────────────────────────────────────────────┐  │     │
│   │  │    ed25519 (Solana, newer chains)                        │  │     │
│   │  │    • Used by: Solana, Near, Polkadot                     │  │     │
│   │  │    • EdDSA signatures                                     │  │     │
│   │  │    • Faster, more efficient                               │  │     │
│   │  └───────────────────────────────────────────────────────────┘  │     │
│   │                                                                   │     │
│   │  ⓘ This choice cannot be changed after wallet creation          │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Selected: secp256k1                                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Select Curve  [Enter] Continue  [Esc] Back  [?] Help                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Session Configuration Screens

### Quick DKG Session Screen

For quickly creating a standard 2-of-3 wallet.

```
┌─ Quick DKG Session ──────────────────────────────────────────────┐
│                                                                  │
│ Wallet Name: [_company_treasury_______] (auto-generated)        │
│                                                                  │
│ Participants (3 total, 2 required):                             │
│ ✓ You         [mpc-node-alice]     Status: Ready                │
│ ○ Participant [________________]   Add device ID                │
│ ○ Participant [________________]   Add device ID                │
│                                                                  │
│ Curve: ● secp256k1 (Ethereum)  ○ ed25519 (Solana)             │
│                                                                  │
│ Network: ● Online Mode  ○ Offline Mode                         │
│                                                                  │
│ [Enter] Start DKG  [A] Auto-discover  [L] Load Template        │
│ [Esc] Back                                                       │
└──────────────────────────────────────────────────────────────────┘
```

### Custom DKG Setup Screen

For advanced configuration of wallet parameters.

```
┌─ Custom DKG Setup ───────────────────────────────────────────────┐
│                                                                  │
│ Session Configuration                                            │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Wallet Name: [_________________________] (required)        │  │
│ │ Description: [_________________________] (optional)        │  │
│ │                                                             │  │
│ │ Threshold Scheme:                                           │  │
│ │ Total Participants: [3] ↕   Required Signatures: [2] ↕     │  │
│ │                                                             │  │
│ │ Advanced Options:                                           │  │
│ │ [✓] Enable session timeout (24 hours)                      │  │
│ │ [✓] Require all participants online                        │  │
│ │ [ ] Allow dynamic participant joining                      │  │
│ │ [ ] Enable session resumption                              │  │
│ │                                                             │  │
│ │ Security Level: ● Standard  ○ High  ○ Maximum              │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Participants Management:                                         │
│ [A] Auto-discover  [M] Manual entry  [I] Import list           │
│ [Enter] Continue  [S] Save template  [Esc] Back                │
└──────────────────────────────────────────────────────────────────┘
```

### Multi-Chain Wallet Creation Screen

For creating wallets that support multiple blockchains.

```
┌─ Multi-Chain Wallet Creation ────────────────────────────────────┐
│                                                                  │
│ Select Supported Chains:                                         │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ [✓] Ethereum (secp256k1)    Address: 0x742d35Cc...        │  │
│ │ [✓] Bitcoin (secp256k1)     Address: bc1qxy2kgd...        │  │
│ │ [ ] Solana (ed25519)        Address: (requires new DKG)    │  │
│ │ [✓] Polygon (secp256k1)     Address: 0x742d35Cc...        │  │
│ │ [ ] Avalanche (secp256k1)   Address: 0x742d35Cc...        │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ ⚠️  Note: Mixed curve types require separate DKG processes      │
│                                                                  │
│ Chain-Specific Settings:                                         │
│ [C] Configure chains  [G] Gas settings  [T] Test networks      │
│                                                                  │
│ [Enter] Continue with selection  [A] Select all secp256k1      │
│ [Esc] Back                                                       │
└──────────────────────────────────────────────────────────────────┘
```

### Join Wallet Session Screen

For joining an existing session with detailed information.

```
┌─ Join Wallet Session ────────────────────────────────────────────┐
│                                                                  │
│ Available Sessions (3):                              🟢 Online  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ [1] company_treasury    DKG    3/3 participants   Ready    │  │
│ │     Initiator: mpc-node-bob     Threshold: 2/3             │  │
│ │     Curve: secp256k1           Timeout: 23h 45m            │  │
│ │                                                             │  │
│ │ [2] project_alpha       Sign   2/3 participants   Waiting  │  │
│ │     Initiator: mpc-node-carol   Amount: 1.5 ETH            │  │
│ │     Transaction: 0xa1b2c3...    Gas: 21000                 │  │
│ │                                                             │  │
│ │ [3] backup_wallet       DKG    1/5 participants   Pending  │  │
│ │     Initiator: mpc-node-dave    Threshold: 3/5             │  │
│ │     Curve: ed25519             Enterprise Policy           │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [M] Manual entry  [F] Filter sessions  [R] Refresh             │
│ [Enter] Join selected  [D] Details  [Esc] Back                 │
└──────────────────────────────────────────────────────────────────┘
```

### Session Details View

Detailed view when pressing [D] on a session.

```
┌─ Session Details: company_treasury ──────────────────────────────┐
│                                                                  │
│ Session Information:                                             │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ ID: company_treasury_20250112_1430                         │  │
│ │ Type: DKG (Key Generation)                                  │  │
│ │ Initiator: mpc-node-bob                                     │  │
│ │ Created: 2025-01-12 14:30:15 UTC                           │  │
│ │ Timeout: 2025-01-13 14:30:15 UTC (23h 45m remaining)      │  │
│ │                                                             │  │
│ │ Threshold Configuration:                                    │  │
│ │ Total Participants: 3                                       │  │
│ │ Required Signatures: 2                                      │  │
│ │ Cryptographic Curve: secp256k1                             │  │
│ │                                                             │  │
│ │ Security Settings:                                          │  │
│ │ Session Encryption: AES-256-GCM                            │  │
│ │ Message Authentication: HMAC-SHA256                        │  │
│ │ Forward Secrecy: Enabled                                   │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Participants:                                                    │
│ ✓ mpc-node-bob (Initiator)   Status: Ready     Connected       │
│ ✓ mpc-node-alice             Status: Ready     Connected       │
│ ? Your participation         Status: Pending   Not joined      │
│                                                                  │
│ [Enter] Join Session  [C] Copy session ID  [Esc] Back          │
└──────────────────────────────────────────────────────────────────┘
```

### Manual Session Entry Screen

For manually entering a session ID.

```
┌─ Manual Session Entry ───────────────────────────────────────────┐
│                                                                  │
│ Enter Session Information:                                       │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Session ID: [_____________________________] (required)     │  │
│ │                                                             │  │
│ │ Optional Connection Info:                                   │  │
│ │ Signaling Server: [wss://auto-life.tech___] (default)      │  │
│ │ Custom Port:      [_____] (leave empty for default)        │  │
│ │                                                             │  │
│ │ Authentication (if required):                               │  │
│ │ Passcode:         [_____________________] (optional)       │  │
│ │                                                             │  │
│ │ Connection Mode:                                            │  │
│ │ ● Auto-detect     ○ Force WebRTC        ○ WebSocket only   │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Recent Sessions:                                                 │
│ [1] company_treasury_20250111  [2] project_alpha_20250110       │
│                                                                  │
│ [Enter] Connect  [P] Paste from clipboard  [Esc] Back          │
└──────────────────────────────────────────────────────────────────┘
```

### Join Session Screen - Offline

For joining an existing session offline via SD card data exchange.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         JOIN SESSION - OFFLINE                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Mode Selection                          SD Card: ● Mounted        │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Import session invitation from SD card:                         │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │                                                           │   │     │
│   │  │  SD Card Status:  [✓] Mounted at /mnt/sdcard            │   │     │
│   │  │  Files Found:     3 session invitations                 │   │     │
│   │  │                                                           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Available invitations on SD card:                               │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ > cold_wallet_3of5.json    DKG    Coordinator: alice    │   │     │
│   │  │   Threshold: 3 of 5         Curve: secp256k1 (Bitcoin) │   │     │
│   │  │   Your role: Participant #2                            │   │     │
│   │  │   Created: 1 hour ago      Expires: 23 hours          │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │   signing_request_01.json  SIGN   Wallet: treasury     │   │     │
│   │  │   Transaction: 10 BTC transfer to bc1qxy2...           │   │     │
│   │  │   Required: 3 of 5 signatures                          │   │     │
│   │  │   Created: 30 min ago      Round: 1 of 2              │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │   refresh_shares.json      REFRESH  Wallet: cold-stor  │   │     │
│   │  │   Operation: Key share refresh                         │   │     │
│   │  │   Participants: 5 of 7      Your role: Participant #4 │   │     │
│   │  │   Created: 2 hours ago                                │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Or scan QR code: [Press Q to activate QR scanner]              │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ [3 invitations]                               SD Card: 14.2 GB Free        │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Import  [R] Rescan SD  [Q] QR Scan  [Esc] Back     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Backup & Recovery Screen

Main screen for all backup, recovery, import, and export operations.

```
┌─ Backup & Recovery ──────────────────────────────────────────────┐
│                                                                  │
│ Data Protection & Recovery Tools:                                │
│                                                                  │
│ Backup Operations:                                               │
│ [1] 💾 Full Keystore Backup     Export all wallets/settings     │
│ [2] 📦 Individual Wallet Export Export specific wallet          │
│ [3] ⚙️  Configuration Export     Settings and preferences       │
│ [4] 🔐 Encrypted Backup          Password-protected archive     │
│                                                                  │
│ Recovery Operations:                                             │
│ [5] 📂 Import Keystore          Restore from backup file        │
│ [6] 🔗 Import Single Wallet     Add wallet from export          │
│ [7] 🖥️  Import from CLI          Cross-platform import          │
│ [8] 🌐 Import from Browser       Chrome extension import        │
│                                                                  │
│ Emergency Recovery:                                              │
│ [9] 🚨 Disaster Recovery        Restore from seed phrases       │
│ [A] 🔧 Repair Corrupted Data    Fix damaged keystores           │
│                                                                  │
│ Status: ✅ Last backup: 2025-01-12 08:00:00 UTC (6 hours ago)  │
│                                                                  │
│ [Enter] Select operation  [S] Schedule backup  [Esc] Back       │
└──────────────────────────────────────────────────────────────────┘
```

### Full Keystore Backup Screen

For backing up all wallets and settings.

```
┌─ Full Keystore Backup ───────────────────────────────────────────┐
│                                                                  │
│ Backup Configuration:                                            │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Backup Location:                                            │  │
│ │ [/home/user/mpc-backups/keystore_20250112_______] Browse   │  │
│ │                                                             │  │
│ │ What to Include:                                            │  │
│ │ [✓] All wallet key shares (5 wallets)                      │  │
│ │ [✓] Device configuration                                   │  │
│ │ [✓] Network settings                                       │  │
│ │ [✓] Security preferences                                   │  │
│ │ [ ] Session history and logs                               │  │
│ │ [ ] Cached blockchain data                                 │  │
│ │                                                             │  │
│ │ Security Options:                                           │  │
│ │ ● Password Protection    Strong encryption (recommended)    │  │
│ │ ○ Hardware Token         Require YubiKey/similar          │  │
│ │ ○ Split Backup           Distribute across multiple files  │  │
│ │                                                             │  │
│ │ Backup Format:                                              │  │
│ │ ● JSON Archive (.json)   ○ Binary Archive (.backup)       │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Estimated Size: 2.4 MB    Estimated Time: < 1 minute           │
│                                                                  │
│ [Enter] Start backup  [T] Test location  [A] Advanced          │
│ [Esc] Cancel                                                     │
└──────────────────────────────────────────────────────────────────┘
```

### Import Keystore Screen

For restoring wallets from backup.

```
┌─ Import Keystore ────────────────────────────────────────────────┐
│                                                                  │
│ Import Source Selection:                                         │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Select Import File:                                         │  │
│ │ [Browse...___________________________________] File        │  │
│ │                                                             │  │
│ │ Detected Format: MPC Wallet Keystore (JSON)                │  │
│ │ File Size: 2.1 MB                                          │  │
│ │ Created: 2025-01-10 15:30:00 UTC                           │  │
│ │ Contains: 3 wallets, 1 device profile                      │  │
│ │                                                             │  │
│ │ Authentication Required:                                    │  │
│ │ Password: [_____________________] (if encrypted)           │  │
│ │ Hardware Token: [ ] Require YubiKey                        │  │
│ │                                                             │  │
│ │ Import Options:                                             │  │
│ │ [✓] Merge with existing keystore                           │  │
│ │ [✓] Verify cryptographic integrity                         │  │
│ │ [ ] Import as read-only                                    │  │
│ │ [✓] Create backup before import                            │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Preview of Wallets to Import:                                    │
│ • company_treasury (2-of-3, secp256k1)                         │
│ • project_alpha (3-of-5, secp256k1)                            │
│ • emergency_backup (2-of-2, ed25519)                           │
│                                                                  │
│ [Enter] Import  [P] Preview details  [V] Verify file           │
│ [Esc] Cancel                                                     │
└──────────────────────────────────────────────────────────────────┘
```

---

## Progress Screens

### DKG Progress Screen

Shows real-time progress during Distributed Key Generation.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         DKG IN PROGRESS                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ Session: wallet_2of3                        [Online Mode] [secp256k1]      │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Distributed Key Generation Progress:                            │     │
│   │                                                                   │     │
│   │  Phase 1: Connection Setup                                       │     │
│   │  [████████████████████████████████████████] 100% Complete       │     │
│   │                                                                   │     │
│   │  Phase 2: WebRTC Mesh Formation                                  │     │
│   │  [████████████████████████████████████████] 100% Complete       │     │
│   │                                                                   │     │
│   │  Phase 3: FROST Protocol Round 1                                 │     │
│   │  [██████████████████████░░░░░░░░░░░░░░░░░░] 60% In Progress     │     │
│   │                                                                   │     │
│   │  Phase 4: FROST Protocol Round 2                                 │     │
│   │  [░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 0% Pending          │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Participants:                     Status:               │   │     │
│   │  │ • mpc-node-001 (You)            ✓ Round 1 Complete    │   │     │
│   │  │ • mpc-node-002                  ⟳ Processing Round 1   │   │     │
│   │  │ • mpc-node-003                  ✓ Round 1 Complete    │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Estimated time remaining: ~30 seconds                           │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ WebRTC: ● Connected (2/2)                        Messages: 42 sent/received │
├─────────────────────────────────────────────────────────────────────────────┤
│ [L] View Logs  [A] Abort Process  [?] Help                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Signing Progress Screen

Shows progress during transaction signing.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SIGNING IN PROGRESS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ Wallet: company_wallet                      [Online Mode] [secp256k1]      │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Transaction Signing Progress:                                   │     │
│   │                                                                   │     │
│   │  Transaction Details:                                            │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Chain:     Ethereum Mainnet                              │   │     │
│   │  │ To:        0x742d35Cc6634C0532925a3b844Bc9e7595f2bd  │   │     │
│   │  │ Value:     1.5 ETH                                       │   │     │
│   │  │ Gas Price: 25 Gwei                                       │   │     │
│   │  │ Nonce:     42                                             │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Signing Progress:                                              │     │
│   │  [████████████████░░░░░░░░░░░░░░░░░░░░░░░░] 40% Collecting    │     │
│   │                                                                   │     │
│   │  Required Signatures: 2 of 3                                    │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ • mpc-node-001 (You)     ✓ Signature provided          │   │     │
│   │  │ • mpc-node-002           ⟳ Computing signature         │   │     │
│   │  │ • mpc-node-003           ○ Not participating          │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Status: Waiting for 1 more signature...                        │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Time Elapsed: 00:45                              Auto-timeout: 4:15         │
├─────────────────────────────────────────────────────────────────────────────┤
│ [D] Show Raw Data  [C] Cancel Signing  [?] Help                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Wallet Management Screen

Shows all wallets and their status.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         WALLET MANAGEMENT                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                                   Total Wallets: 3        │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Your Wallets:                                                   │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ > company_wallet                                         │   │     │
│   │  │   Type: 2-of-3 multisig    Curve: secp256k1             │   │     │
│   │  │   Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f2bd    │   │     │
│   │  │   Created: 2024-01-15      Last Used: Today             │   │     │
│   │  │   Status: ● Active         Balance: 12.5 ETH            │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │   treasury_cold                                          │   │     │
│   │  │   Type: 5-of-7 multisig    Curve: secp256k1             │   │     │
│   │  │   Address: 0x8B3D5C9A7E2F6D1C4B5A9876543210FEDCBA9876    │   │     │
│   │  │   Created: 2023-12-01      Last Used: 2 weeks ago       │   │     │
│   │  │   Status: ● Active         Balance: 450.2 ETH           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │   solana_test_wallet                                     │   │     │
│   │  │   Type: 3-of-5 multisig    Curve: ed25519               │   │     │
│   │  │   Address: 7EYnBvD3HWqFgvFbDp8qKCgQqPK2mPwrAoKBr5BmL    │   │     │
│   │  │   Created: 2024-01-20      Last Used: 3 days ago        │   │     │
│   │  │   Status: ● Active         Balance: 25.8 SOL            │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Select  [Enter] Details  [S] Sign  [E] Export  [D] Delete  [Esc] Back │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Recovery & Backup Path

### Recovery Menu Screen

Main menu for all recovery and backup operations.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RECOVERY & BACKUP                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                                   Backup Status: ✓ Current │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Recovery & Backup Operations:                                   │     │
│   │                                                                   │     │
│   │  > [1] Restore from Backup                                      │     │
│   │      Restore wallet from encrypted backup file                  │     │
│   │                                                                   │     │
│   │    [2] Recover Lost Share                                       │     │
│   │      Initiate share recovery protocol (requires threshold)      │     │
│   │                                                                   │     │
│   │    [3] Emergency Access                                          │     │
│   │      Access emergency recovery options                          │     │
│   │                                                                   │     │
│   │    [4] Verify Backup Integrity                                   │     │
│   │      Test backup files without restoring                        │     │
│   │                                                                   │     │
│   │    [5] Create New Backup                                         │     │
│   │      Export current wallet state securely                       │     │
│   │                                                                   │     │
│   │    [6] Key Rotation                                              │     │
│   │      Rotate shares and update threshold                         │     │
│   │                                                                   │     │
│   │  Last Backup: 2024-01-20 14:32:00 (5 days ago)                 │     │
│   │  Recovery Shares: 3 of 5 available                              │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Recovery Kit: ● Ready                               Emergency Contact: Set   │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓/1-6] Navigate  [Enter] Select  [Esc] Back  [?] Help                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Restore from Backup Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RESTORE FROM BACKUP                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Recovery Menu                                                    │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Select backup source:                                           │     │
│   │                                                                   │     │
│   │  > [1] Local File                                               │     │
│   │      Browse for backup file on local filesystem                │     │
│   │                                                                   │     │
│   │    [2] SD Card / USB                                            │     │
│   │      Import from removable media                               │     │
│   │                                                                   │     │
│   │    [3] Cloud Storage                                            │     │
│   │      Restore from encrypted cloud backup                       │     │
│   │                                                                   │     │
│   │    [4] QR Code Series                                           │     │
│   │      Scan multiple QR codes containing backup                  │     │
│   │                                                                   │     │
│   │    [5] Manual Entry                                             │     │
│   │      Enter recovery phrases or seed manually                   │     │
│   │                                                                   │     │
│   │  Recent Backups:                                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ • wallet_backup_2024-01-20.enc    5 days ago    2.1 MB │   │     │
│   │  │ • treasury_backup_2024-01-15.enc  10 days ago   1.8 MB │   │     │
│   │  │ • cold_storage_2024-01-01.enc    25 days ago   2.4 MB │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ ⚠ Warning: Restoring will overwrite existing wallet data                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓/1-5] Select Source  [Enter] Continue  [Esc] Cancel                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Recover Lost Share Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RECOVER LOST SHARE                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Recovery Menu                          Protocol: FROST Share Recovery │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Share Recovery Protocol:                                        │     │
│   │                                                                   │     │
│   │  Selected Wallet: company_treasury (2-of-3)                     │     │
│   │  Missing Share: Participant #2 (mpc-node-002)                   │     │
│   │                                                                   │     │
│   │  Recovery Requirements:                                          │     │
│   │  • Minimum 2 participants needed (threshold)                    │     │
│   │  • All participants must be online                             │     │
│   │  • Process takes approximately 10-15 minutes                    │     │
│   │                                                                   │     │
│   │  Available Participants:                                         │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] mpc-node-001 (You)          Status: ● Ready        │   │     │
│   │  │ [ ] mpc-node-002 (Lost)         Status: ✗ Missing      │   │     │
│   │  │ [✓] mpc-node-003                Status: ● Ready        │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  New Participant Details:                                        │     │
│   │  Device ID: [mpc-node-002-new_______________]                   │     │
│   │  Public Key: [Will be generated during recovery]                │     │
│   │                                                                   │     │
│   │  [✓] I understand this will invalidate the old share           │     │
│   │  [✓] All participants have been notified                       │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Ready: 2/2 required participants                    Est. Time: 10-15 min    │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Space] Toggle Selection  [S] Start Recovery  [Esc] Cancel                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Verify Backup Integrity Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         VERIFY BACKUP INTEGRITY                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Recovery Menu                                                    │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Backup Verification:                                            │     │
│   │                                                                   │     │
│   │  Selected File: wallet_backup_2024-01-20.enc                    │     │
│   │  Location: /backups/encrypted/                                   │     │
│   │  Size: 2.1 MB                                                    │     │
│   │                                                                   │     │
│   │  Verification Progress:                                          │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [████████████████████████████████████████] Checksum     │   │     │
│   │  │ [████████████████████████████████████████] Encryption   │   │     │
│   │  │ [████████████████████████████████████████] Structure    │   │     │
│   │  │ [██████████████████░░░░░░░░░░░░░░░░░░░░] Key Shares    │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Verification Results:                                           │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ ✓ File Checksum:      Valid (SHA-256 match)            │   │     │
│   │  │ ✓ Encryption:         AES-256-GCM verified             │   │     │
│   │  │ ✓ Backup Structure:   Version 2.0 compatible           │   │     │
│   │  │ ⟳ Key Shares:        Verifying share 2 of 3...         │   │     │
│   │  │   Metadata:          Valid                              │   │     │
│   │  │   Timestamp:         2024-01-20 14:32:00 UTC           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Enter password to decrypt and verify contents:                 │     │
│   │  [••••••••••••••••________________]                            │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Status: Verification in progress...                 Time Elapsed: 00:23     │
├─────────────────────────────────────────────────────────────────────────────┤
│ [P] Pause  [V] View Details  [E] Export Report  [Esc] Cancel               │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Settings & Configuration Path

### Settings Menu Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SETTINGS & CONFIGURATION                            │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                                Profile: Production        │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Configuration Categories:                                       │     │
│   │                                                                   │     │
│   │  > [1] Network Configuration                                    │     │
│   │      WebSocket servers, WebRTC settings, timeouts              │     │
│   │                                                                   │     │
│   │    [2] Security Settings                                        │     │
│   │      Encryption, authentication, access controls                │     │
│   │                                                                   │     │
│   │    [3] Notifications & Alerts                                   │     │
│   │      Alert thresholds, contact methods, webhooks               │     │
│   │                                                                   │     │
│   │    [4] Advanced Options                                         │     │
│   │      Developer mode, logging, performance tuning               │     │
│   │                                                                   │     │
│   │    [5] Display & Interface                                      │     │
│   │      Theme, layout, keyboard shortcuts                         │     │
│   │                                                                   │     │
│   │    [6] Compliance Settings                                      │     │
│   │      Regulatory requirements, audit settings                   │     │
│   │                                                                   │     │
│   │  Current Profile: [Production] [Dev] [Testing]                  │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Config Version: 2.0.1                              Auto-save: ● Enabled     │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓/1-6] Navigate  [Enter] Select  [P] Switch Profile  [Esc] Back          │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Network Configuration Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         NETWORK CONFIGURATION                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings                                  Changes: 2 unsaved      │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  WebSocket Configuration:                                        │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Primary Server:   [wss://auto-life.tech______________]   │   │     │
│   │  │ Backup Server:    [wss://backup.auto-life.tech_______]   │   │     │
│   │  │ Connection Timeout: [30] seconds                         │   │     │
│   │  │ Retry Attempts:    [5]                                   │   │     │
│   │  │ [✓] Auto-reconnect on disconnect                        │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  WebRTC Configuration:                                           │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ STUN Servers:                                            │   │     │
│   │  │ • [stun:stun.l.google.com:19302_________________]      │   │     │
│   │  │ • [stun:stun1.l.google.com:19302________________]      │   │     │
│   │  │ TURN Server:      [turn:turn.auto-life.tech:3478_]      │   │     │
│   │  │ TURN Username:    [user123______________________]       │   │     │
│   │  │ TURN Password:    [••••••••••••••••••••________]       │   │     │
│   │  │ [✓] Enable ICE trickle                                  │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Blockchain RPC Endpoints:                                       │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Ethereum: [https://eth-mainnet.g.alchemy.com/v2/____]  │   │     │
│   │  │ Solana:   [https://api.mainnet-beta.solana.com______]  │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Connection Test: [T] Test Current Settings          Status: Not tested      │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Next Field  [S] Save  [R] Reset to Defaults  [T] Test  [Esc] Cancel  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Security Settings Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SECURITY SETTINGS                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings                              Security Level: ■■■■□ High  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Encryption Settings:                                            │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Key Derivation:    [Argon2id] [PBKDF2] [Scrypt]         │   │     │
│   │  │ Iterations:        [100000_____] (min: 10000)           │   │     │
│   │  │ Encryption:        [AES-256-GCM] [ChaCha20-Poly1305]   │   │     │
│   │  │ [✓] Require password for all operations                 │   │     │
│   │  │ [✓] Auto-lock after [5] minutes of inactivity          │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Authentication:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] Two-Factor Authentication (2FA)                     │   │     │
│   │  │     Method: [TOTP] [Hardware Key] [SMS]                 │   │     │
│   │  │ [✓] Biometric unlock (if available)                     │   │     │
│   │  │ [ ] Remember device for [30] days                       │   │     │
│   │  │ Session timeout: [30] minutes                           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Access Control:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Failed login attempts before lockout: [3]               │   │     │
│   │  │ Lockout duration: [30] minutes                          │   │     │
│   │  │ [✓] IP allowlist enabled                                │   │     │
│   │  │     Allowed IPs: 192.168.1.0/24, 10.0.0.0/8           │   │     │
│   │  │ [✓] Require approval for new devices                    │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Last security audit: 2024-01-15                    [A] Run Security Audit   │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Navigate  [Space] Toggle  [S] Save  [A] Audit  [Esc] Cancel          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Audit & Compliance Path

### Audit Menu Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AUDIT & COMPLIANCE                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                                  Compliance: ✓ Compliant  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Audit & Compliance Tools:                                      │     │
│   │                                                                   │     │
│   │  > [1] Transaction History                                      │     │
│   │      View detailed transaction logs and history                │     │
│   │                                                                   │     │
│   │    [2] Access Logs                                              │     │
│   │      User access and authentication history                    │     │
│   │                                                                   │     │
│   │    [3] Generate Reports                                         │     │
│   │      Create compliance and audit reports                       │     │
│   │                                                                   │     │
│   │    [4] Risk Assessment                                          │     │
│   │      Analyze wallet risk profile and recommendations           │     │
│   │                                                                   │     │
│   │    [5] Export Audit Trail                                       │     │
│   │      Export complete audit data for external review            │     │
│   │                                                                   │     │
│   │    [6] Compliance Dashboard                                     │     │
│   │      View regulatory compliance status                         │     │
│   │                                                                   │     │
│   │  Summary:                                                        │     │
│   │  • Total Transactions: 1,247                                    │     │
│   │  • Last Audit: 2024-01-20 14:00:00                            │     │
│   │  • Compliance Score: 98/100                                     │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Next scheduled audit: 2024-02-01                   Auto-export: ● Enabled   │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓/1-6] Navigate  [Enter] Select  [E] Quick Export  [Esc] Back            │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Transaction History Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TRANSACTION HISTORY                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Audit Menu           Filter: [All] [Sent] [Received] [Failed]    │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │ Date Range: [2024-01-01] to [2024-01-25]    Search: [_______]  │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │ Time        Type   Amount      From/To              Status      │     │
│   ├─────────────────────────────────────────────────────────────────┤     │
│   │ > 14:32:15  SEND   1.5 ETH    → 0x742d35...595f   ✓ Confirmed │     │
│   │   Gas: 0.003 ETH   Block: 18976543   Confirmations: 12        │     │
│   │   Signers: mpc-node-001, mpc-node-003 (2/3)                   │     │
│   │   Memo: "Payment for services - Invoice #1234"                 │     │
│   ├─────────────────────────────────────────────────────────────────┤     │
│   │   13:45:22  RECV   5.2 ETH    ← 0x8B3D5C...BA98   ✓ Confirmed │     │
│   │   Block: 18976234   Confirmations: 321                         │     │
│   │   Memo: "Treasury deposit"                                     │     │
│   ├─────────────────────────────────────────────────────────────────┤     │
│   │   12:10:33  SEND   0.5 ETH    → 0xA4B1C2...3D4E   ✗ Failed    │     │
│   │   Error: Insufficient signatures (1/3)                         │     │
│   │   Attempted by: mpc-node-002                                  │     │
│   ├─────────────────────────────────────────────────────────────────┤     │
│   │   11:22:44  SIGN   10.0 SOL   → 7dHbWXm...KTaM    ⟳ Pending   │     │
│   │   Waiting for signatures: 1/2 collected                        │     │
│   │   Time remaining: 3:27                                         │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Showing 4 of 1,247 transactions            Page 1 of 312  [◀] [▶]         │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Details  [F] Filter  [E] Export  [R] Refresh        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Access Logs Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ACCESS LOGS                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Audit Menu                    Filter: [All] [Success] [Failed]   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Access Event History:                                           │     │
│   │                                                                   │     │
│   │  ┌──────────────────────────────────────────────────────────┐  │     │
│   │  │ 2024-01-25 14:45:23  LOGIN SUCCESS                       │  │     │
│   │  │ User: admin@company.com   Device: mpc-node-001           │  │     │
│   │  │ IP: 192.168.1.100   Location: New York, US              │  │     │
│   │  │ 2FA: ✓ Verified   Session: 45min                         │  │     │
│   │  ├──────────────────────────────────────────────────────────┤  │     │
│   │  │ 2024-01-25 14:32:10  OPERATION: Create Backup            │  │     │
│   │  │ User: admin@company.com   Result: Success                │  │     │
│   │  │ Details: Encrypted backup created (2.1MB)                │  │     │
│   │  ├──────────────────────────────────────────────────────────┤  │     │
│   │  │ 2024-01-25 13:21:45  LOGIN FAILED                        │  │     │
│   │  │ User: unknown   Device: Unknown-Device-ID                │  │     │
│   │  │ IP: 45.32.164.22   Location: Unknown                     │  │     │
│   │  │ Reason: Invalid credentials (attempt 3/3)                │  │     │
│   │  │ Action: IP blocked for 30 minutes                        │  │     │
│   │  ├──────────────────────────────────────────────────────────┤  │     │
│   │  │ 2024-01-25 12:15:33  OPERATION: Sign Transaction         │  │     │
│   │  │ User: operator@company.com   Result: Success             │  │     │
│   │  │ Transaction: 0x742d...595f   Amount: 1.5 ETH            │  │     │
│   │  │ Approvers: 2/3 threshold met                             │  │     │
│   │  └──────────────────────────────────────────────────────────┘  │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Total events: 3,842                        Suspicious events: 12 ⚠         │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Details  [F] Filter  [E] Export  [A] Analyze        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Emergency Response Path

### Emergency Response Menu

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      🚨 EMERGENCY RESPONSE 🚨                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                                    Status: ⚠ ALERT MODE   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  ⚠ EMERGENCY ACTIONS - USE WITH CAUTION ⚠                       │     │
│   │                                                                   │     │
│   │  > [1] 🔒 LOCK ALL WALLETS                                      │     │
│   │      Immediately disable all wallet operations                  │     │
│   │                                                                   │     │
│   │    [2] ⏸ PAUSE OPERATIONS                                       │     │
│   │      Temporarily halt signing and transactions                  │     │
│   │                                                                   │     │
│   │    [3] 🚨 ALERT CONTACTS                                         │     │
│   │      Notify emergency contacts and administrators               │     │
│   │                                                                   │     │
│   │    [4] 🔍 FORENSIC MODE                                          │     │
│   │      Enable detailed logging for investigation                  │     │
│   │                                                                   │     │
│   │    [5] 🔄 INITIATE KEY ROTATION                                  │     │
│   │      Emergency key share rotation protocol                      │     │
│   │                                                                   │     │
│   │    [6] 📤 EMERGENCY EXPORT                                       │     │
│   │      Export all critical data immediately                       │     │
│   │                                                                   │     │
│   │  Current Threats:                                                │     │
│   │  • 3 failed login attempts from unknown IP                      │     │
│   │  • Unusual transaction pattern detected                         │     │
│   │  • Participant mpc-node-003 unreachable for 2 hours           │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Emergency Kit: ✓ Ready                    Last Drill: 2024-01-15           │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓/1-6] Select  [Enter] EXECUTE  [H] Emergency Hotline  [Esc] Cancel      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Lock All Wallets Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         🔒 LOCK ALL WALLETS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ ⚠ CRITICAL OPERATION                               Requires Authorization   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  ⚠ WARNING: This will immediately lock all wallet operations    │     │
│   │                                                                   │     │
│   │  Affected Wallets:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] company_treasury     Balance: 12.5 ETH              │   │     │
│   │  │ [✓] treasury_cold        Balance: 450.2 ETH             │   │     │
│   │  │ [✓] solana_test_wallet   Balance: 25.8 SOL              │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Lock Configuration:                                             │     │
│   │  Duration: [⚫ Indefinite] [○ 24 hours] [○ Custom]             │     │
│   │  Notify: [✓] All participants                                   │     │
│   │         [✓] Emergency contacts                                  │     │
│   │         [✓] Compliance officer                                  │     │
│   │                                                                   │     │
│   │  Authorization Required:                                         │     │
│   │  Admin Password: [••••••••••••••••________________]            │     │
│   │  2FA Code:       [______]                                       │     │
│   │                                                                   │     │
│   │  Reason for lock (required):                                    │     │
│   │  [Suspected security breach - investigating_____________]       │     │
│   │                                                                   │     │
│   │  [✓] I understand this action cannot be easily reversed        │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ ⚠ This action will be logged and audited                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ [L] LOCK NOW  [T] Test Mode First  [C] Cancel  [?] Help                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Forensic Mode Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         🔍 FORENSIC MODE                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Emergency                           Recording: ● ACTIVE           │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Forensic Data Collection:                                      │     │
│   │                                                                   │     │
│   │  Capture Settings:                                              │     │
│   │  [✓] All network traffic          [✓] System calls             │     │
│   │  [✓] File system access           [✓] Memory snapshots         │     │
│   │  [✓] Process activity             [✓] Cryptographic operations │     │
│   │  [✓] User interactions            [✓] External connections     │     │
│   │                                                                   │     │
│   │  Collection Status:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Duration:        00:15:42                                │   │     │
│   │  │ Events Captured: 3,847                                   │   │     │
│   │  │ Data Size:       127.3 MB                                │   │     │
│   │  │ Anomalies:       12 ⚠                                     │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Live Event Stream:                                             │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ 14:45:23.842 [NET] Connection from 192.168.1.105       │   │     │
│   │  │ 14:45:23.901 [AUTH] Login attempt: admin@company.com   │   │     │
│   │  │ 14:45:24.122 [CRYPTO] Key derivation started           │   │     │
│   │  │ 14:45:24.234 [FILE] Read: /wallets/company.json        │   │     │
│   │  │ 14:45:24.456 [ANOMALY] Unusual API call pattern ⚠      │   │     │
│   │  │ 14:45:24.678 [NET] Outbound to 45.32.164.22:443       │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Storage: 872.5 MB free                      Auto-upload: ● Enabled         │
├─────────────────────────────────────────────────────────────────────────────┤
│ [S] Stop Recording  [E] Export Now  [F] Filter  [A] Analyze  [Esc] Back    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Multi-Wallet Operations Path

### Multi-Wallet Dashboard

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         MULTI-WALLET OPERATIONS                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                          Total Portfolio: $2,847,392.50   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Portfolio Overview:                                            │     │
│   │                                                                   │     │
│   │  ┌──────────────────────────────────────────────────────────┐  │     │
│   │  │ Wallet              Balance       USD Value    24h Change│  │     │
│   │  ├──────────────────────────────────────────────────────────┤  │     │
│   │  │ > company_treasury  12.5 ETH      $31,250      ▲ +2.3%  │  │     │
│   │  │   treasury_cold     450.2 ETH     $1,125,500   ▼ -0.8%  │  │     │
│   │  │   solana_ops        25.8 SOL      $2,580       ▲ +5.2%  │  │     │
│   │  │   btc_reserves      10.5 BTC      $367,500     ▲ +1.1%  │  │     │
│   │  │   defi_wallet       1,234 ETH     $3,085,000   ▼ -1.5%  │  │     │
│   │  └──────────────────────────────────────────────────────────┘  │     │
│   │                                                                   │     │
│   │  Batch Operations:                                              │     │
│   │                                                                   │     │
│   │  > [1] Batch Transfer                                           │     │
│   │      Send from multiple wallets simultaneously                 │     │
│   │                                                                   │     │
│   │    [2] Portfolio Rebalancing                                    │     │
│   │      Automatically rebalance across wallets                    │     │
│   │                                                                   │     │
│   │    [3] Consolidated Reporting                                   │     │
│   │      Generate unified reports for all wallets                  │     │
│   │                                                                   │     │
│   │    [4] Risk Analysis                                            │     │
│   │      Analyze portfolio risk and correlations                   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Selected: 1 wallet                         Last sync: 2 minutes ago        │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Space] Select  [A] Select All  [Enter] Operation  [R] Refresh  [Esc] Back │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Batch Transfer Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BATCH TRANSFER                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Multi-Wallet                     Total to Transfer: 15.7 ETH     │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Configure Batch Transfer:                                      │     │
│   │                                                                   │     │
│   │  Source Wallets:                                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] company_treasury    12.5 ETH    Amount: [5.0____]  │   │     │
│   │  │ [✓] treasury_cold       450.2 ETH   Amount: [10.0___]  │   │     │
│   │  │ [ ] defi_wallet         1,234 ETH   Amount: [_______]  │   │     │
│   │  │ [✓] test_wallet         3.2 ETH     Amount: [0.7____]  │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Destination:                                                    │     │
│   │  Address: [0x742d35Cc6634C0532925a3b844Bc9e7595f2bd______]    │     │
│   │  ENS:     [payments.company.eth_____________________]           │     │
│   │                                                                   │     │
│   │  Transfer Summary:                                               │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Total Amount:    15.7 ETH (~$39,250)                    │   │     │
│   │  │ From Wallets:    3                                       │   │     │
│   │  │ Network Fees:    ~0.045 ETH (3 transactions)            │   │     │
│   │  │ Required Sigs:   7 total (2/3 + 3/5 + 2/3)             │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Schedule: [⚫ Execute Now] [○ Schedule for: ___________]      │     │
│   │  Priority: [○ Low] [⚫ Normal] [○ High]                        │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Validation: ✓ All amounts valid            [P] Preview  [S] Sign & Send    │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Next Field  [Space] Toggle  [P] Preview  [S] Send  [Esc] Cancel      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Portfolio Analytics Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PORTFOLIO ANALYTICS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Multi-Wallet              Time Range: [30D] [90D] [1Y] [All]    │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Portfolio Performance:                                         │     │
│   │                                                                   │     │
│   │  Total Value Over Time (30 days):                              │     │
│   │  $3M ┤                                           ╱─────        │     │
│   │      │                                      ╱───╯              │     │
│   │  $2M ┤                            ╱────────╯                   │     │
│   │      │                      ╱────╯                             │     │
│   │  $1M ┤────────────────────╯                                   │     │
│   │      └──────────────────────────────────────────────────►     │     │
│   │       Jan 1                Jan 15                    Jan 25    │     │
│   │                                                                   │     │
│   │  Asset Allocation:                Risk Metrics:                 │     │
│   │  ┌────────────────┐             ┌──────────────────────────┐  │     │
│   │  │ ETH    65% ████│             │ Sharpe Ratio:      1.82  │  │     │
│   │  │ BTC    25% ██  │             │ Max Drawdown:      -12%  │  │     │
│   │  │ SOL     8% █   │             │ Volatility:        28%   │  │     │
│   │  │ Other   2% ·   │             │ Risk Score:        6/10  │  │     │
│   │  └────────────────┘             └──────────────────────────┘  │     │
│   │                                                                   │     │
│   │  Top Performers:               Recommendations:                │     │
│   │  • defi_wallet    +15.2%       • Consider rebalancing ETH     │     │
│   │  • solana_ops     +12.8%       • Increase BTC allocation      │     │
│   │  • btc_reserves    +8.4%       • Review fee optimization      │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Export: [PDF] [CSV] [JSON]                 Auto-refresh: ● Every 5 min     │
├─────────────────────────────────────────────────────────────────────────────┤
│ [T] Timeframe  [E] Export  [R] Refresh  [D] Detailed View  [Esc] Back      │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Error and Recovery Screens

### Connection Error Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONNECTION ERROR                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  ⚠ Connection Lost                                               │     │
│   │                                                                   │     │
│   │  Unable to establish connection with signaling server:          │     │
│   │  wss://auto-life.tech                                            │     │
│   │                                                                   │     │
│   │  Error Details:                                                  │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Code: ETIMEDOUT                                          │   │     │
│   │  │ Message: Connection timed out after 30 seconds          │   │     │
│   │  │ Time: 2024-01-25 14:32:15                               │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Troubleshooting:                                                │     │
│   │  • Check your internet connection                               │     │
│   │  • Verify firewall settings allow WebSocket connections        │     │
│   │  • Try using a different network                               │     │
│   │  • Contact support if problem persists                         │     │
│   │                                                                   │     │
│   │  [R] Retry Connection   [O] Offline Mode   [Q] Quit            │     │
│   │                                                                   │     │
│   │  Retry attempts: 3/5                                             │     │
│   │  Next retry in: 15 seconds...                                   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ [R] Retry Now  [O] Switch to Offline  [L] View Logs  [Q] Quit              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Session Recovery Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SESSION RECOVERY                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  ⚠ Session Interrupted                                           │     │
│   │                                                                   │     │
│   │  The DKG session 'wallet_2of3' was interrupted.                 │     │
│   │  Would you like to attempt recovery?                             │     │
│   │                                                                   │     │
│   │  Session Details:                                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Session ID: wallet_2of3                                  │   │     │
│   │  │ Progress: Phase 3 - Round 1 (60% complete)              │   │     │
│   │  │ Participants Connected: 2/3                              │   │     │
│   │  │ Time Elapsed: 2:34                                       │   │     │
│   │  │ Last Activity: 30 seconds ago                           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Recovery Options:                                               │     │
│   │                                                                   │     │
│   │  > [1] Resume Session (Recommended)                             │     │
│   │      Continue from last checkpoint                              │     │
│   │                                                                   │     │
│   │    [2] Restart Session                                           │     │
│   │      Start the DKG process from beginning                       │     │
│   │                                                                   │     │
│   │    [3] Export Partial Data                                       │     │
│   │      Save current progress for manual recovery                  │     │
│   │                                                                   │     │
│   │    [4] Abandon Session                                           │     │
│   │      Cancel and return to main menu                             │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Select Option  [Enter] Confirm  [?] Help                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Critical Error Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CRITICAL ERROR                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  ⛔ Critical Error Occurred                                       │     │
│   │                                                                   │     │
│   │  An unrecoverable error has occurred during the operation.      │     │
│   │                                                                   │     │
│   │  Error Information:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Type: KeyGenerationFailure                               │   │     │
│   │  │ Code: FROST_PROTOCOL_ERROR                                │   │     │
│   │  │ Message: Invalid share verification in Round 2           │   │     │
│   │  │ Component: DKG Protocol Handler                          │   │     │
│   │  │ Timestamp: 2024-01-25 14:45:23 UTC                      │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Stack Trace (abbreviated):                                      │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ at frost_core::dkg::round2::verify_share()              │   │     │
│   │  │ at starlab_mpc::protocol::handle_dkg_round2()            │   │     │
│   │  │ at starlab_mpc::session::process_message()               │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Actions:                                                        │     │
│   │  [S] Save Error Report   [L] View Full Logs   [R] Restart      │     │
│   │                                                                   │     │
│   │  ⓘ This error has been logged. Please contact support if        │     │
│   │    this issue persists.                                         │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ [S] Save Report  [L] Full Logs  [R] Restart App  [Q] Quit                  │
└─────────────────────────────────────────────────────────────────────────────┘
```