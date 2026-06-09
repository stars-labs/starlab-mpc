# MPC TUI Wallet - Comprehensive Submenu Design

This document provides detailed submenu hierarchies and UI/UX design for all main menu options in the MPC TUI wallet, targeting professional enterprise users with both technical and non-technical backgrounds.

## Design Philosophy

### Core Principles
1. **Professional Enterprise Interface**: BitGo-inspired design with data-dense, clean layouts
2. **Progressive Disclosure**: Simple top-level options expanding to expert features
3. **Keyboard-First Navigation**: Consistent shortcuts and navigation patterns
4. **Security-Critical Confirmations**: Multiple confirmations for destructive actions
5. **Real-Time Status Feedback**: Live updates and progress indicators
6. **Accessibility**: Clear labeling, high contrast, logical tab order

### Navigation Patterns
- **Number Keys (1-9)**: Direct menu selection
- **Letter Keys (A-Z)**: Quick actions and shortcuts
- **Arrow Keys**: Navigation within lists/forms
- **Enter**: Confirm/Submit
- **Escape**: Back/Cancel (with confirmation if needed)
- **Tab/Shift+Tab**: Field navigation
- **F-Keys**: Function shortcuts (F1=Help, F5=Refresh, etc.)

---

## [1] Create New Wallet (Start DKG Process)

### Main Screen Layout
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

### 1.1 Quick DKG Session
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

### 1.2 Custom DKG Setup
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

### 1.3 Multi-Chain Wallet
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

---

## [2] Join Wallet Session (Participate in DKG/Signing)

### Main Screen Layout
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

### 2.1 Session Details View
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

### 2.2 Manual Session Entry
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

---

## [3] Select Existing Wallet (Access Stored Wallets)

### Main Screen Layout
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

### 3.1 Wallet Details View
```
┌─ Wallet Details: company_treasury ───────────────────────────────┐
│                                                                  │
│ General Information:                                             │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Name: company_treasury                                       │  │
│ │ Description: Main treasury for company operations            │  │
│ │ Created: 2025-01-10 09:30:15 UTC                           │  │
│ │ Last Modified: 2025-01-12 14:22:03 UTC                     │  │
│ │ Status: Active                                               │  │
│ │                                                              │  │
│ │ Cryptographic Configuration:                                 │  │
│ │ Curve: secp256k1                                            │  │
│ │ Threshold: 2-of-3                                           │  │
│ │ Your Index: 1                                               │  │
│ │ Public Key: 0x04a1b2c3d4e5f6...                            │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Participants:                                                    │
│ [1] You (mpc-node-alice)      Index: 1    Status: Active       │
│ [2] mpc-node-bob              Index: 2    Status: Active       │
│ [3] mpc-node-carol            Index: 3    Status: Inactive     │
│                                                                  │
│ Blockchain Addresses:                                            │
│ Ethereum: 0x742d35Cc6Eb6fC6D...    Balance: 15.7 ETH          │
│ Bitcoin:  bc1qxy2kgdx3s8t7v...      Balance: 0.25 BTC         │
│ Polygon:  0x742d35Cc6Eb6fC6D...    Balance: 2.1 MATIC        │
│                                                                  │
│ [Enter] Use wallet  [T] Test connection  [E] Export            │
│ [R] Rename  [L] Lock wallet  [Esc] Back                        │
└──────────────────────────────────────────────────────────────────┘
```

### 3.2 Wallet Operations Menu
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

## [4] Backup & Recovery (Import/Export/Restore)

### Main Screen Layout
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

### 4.1 Full Keystore Backup
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

### 4.2 Import Operations
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

## [5] Settings & Configuration

### Main Screen Layout
```
┌─ Settings & Configuration ───────────────────────────────────────┐
│                                                                  │
│ System Configuration:                                            │
│                                                                  │
│ Network & Connectivity:                                          │
│ [1] 🌐 Network Settings         Servers, ports, protocols       │
│ [2] 🔗 WebRTC Configuration     P2P connection settings          │
│ [3] 🛡️  Security Policies       Encryption and auth settings   │
│ [4] 🎯 Connection Profiles      Different network environments   │
│                                                                  │
│ User Interface:                                                  │
│ [5] 🎨 Display Preferences      Colors, layout, fonts           │
│ [6] ⌨️  Keyboard Shortcuts      Customize key bindings          │
│ [7] 🔔 Notifications           Alert preferences                │
│ [8] 🌍 Language & Region       Localization settings            │
│                                                                  │
│ Application Behavior:                                            │
│ [9] 💾 Data Management         Storage locations, cleanup       │
│ [A] 🔄 Auto-Update Settings    Software update preferences      │
│ [B] 📊 Logging & Diagnostics   Debug and audit configuration    │
│ [C] 🏢 Enterprise Policies     Organization-wide settings       │
│                                                                  │
│ Current Profile: Production  Status: ✅ Configured             │
│                                                                  │
│ [Enter] Configure  [R] Reset to defaults  [Esc] Back           │
└──────────────────────────────────────────────────────────────────┘
```

### 5.1 Network Settings
```
┌─ Network Settings ───────────────────────────────────────────────┐
│                                                                  │
│ Signaling Server Configuration:                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Primary Server:                                             │  │
│ │ URL: [wss://auto-life.tech________________] (WebSocket)     │  │
│ │ Port: [8080____] Timeout: [30s____] Retries: [3___]        │  │
│ │                                                             │  │
│ │ Fallback Servers:                                           │  │
│ │ [✓] wss://backup.auto-life.tech:8080                       │  │
│ │ [ ] wss://eu.signaling-service.com:8080                    │  │
│ │ [ ] wss://us-west.mpc-relay.net:8080                       │  │
│ │                                                             │  │
│ │ Connection Options:                                         │  │
│ │ [✓] Enable automatic failover                              │  │
│ │ [✓] Use compression                                         │  │
│ │ [ ] Force secure connections only                          │  │
│ │ [✓] Enable connection pooling                              │  │
│ │                                                             │  │
│ │ Advanced Settings:                                          │  │
│ │ Keep-alive interval: [25s____]                             │  │
│ │ Max message size: [1MB____]                                │  │
│ │ Heartbeat timeout: [5s____]                                │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Connection Status: 🟢 Connected (ping: 45ms, uptime: 2h 15m)   │
│                                                                  │
│ [T] Test connection  [D] Diagnostics  [S] Save                 │
│ [R] Reset defaults   [Esc] Cancel                              │
└──────────────────────────────────────────────────────────────────┘
```

### 5.2 Security Policies
```
┌─ Security Policies ──────────────────────────────────────────────┐
│                                                                  │
│ Cryptographic Settings:                                          │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Key Derivation:                                             │  │
│ │ PBKDF2 iterations: [100000_____]                           │  │
│ │ Salt size: [32 bytes] Memory cost: [64MB___]               │  │
│ │                                                             │  │
│ │ Session Security:                                           │  │
│ │ Message encryption: ● AES-256-GCM  ○ ChaCha20-Poly1305    │  │
│ │ Key exchange: ● X25519  ○ P-256                            │  │
│ │ [✓] Perfect forward secrecy                                │  │
│ │ [✓] Message replay protection                              │  │
│ │                                                             │  │
│ │ Session Timeouts:                                           │  │
│ │ DKG session: [24 hours____] Signing: [1 hour____]         │  │
│ │ Idle timeout: [30 minutes_] Max duration: [8 hours___]    │  │
│ │                                                             │  │
│ │ Access Control:                                             │  │
│ │ [✓] Require device authentication                          │  │
│ │ [ ] Enable IP whitelist                                    │  │
│ │ [✓] Lock after failed attempts (3 tries)                  │  │
│ │ [ ] Require hardware security module                      │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Security Level: ● High     Compliance: SOC 2, ISO 27001        │
│                                                                  │
│ [A] Apply changes  [T] Test configuration  [P] Policy export   │
│ [Esc] Cancel                                                     │
└──────────────────────────────────────────────────────────────────┘
```

---

## [6] Audit & Compliance

### Main Screen Layout
```
┌─ Audit & Compliance ─────────────────────────────────────────────┐
│                                                                  │
│ Compliance & Audit Management:                                   │
│                                                                  │
│ Audit Trail Management:                                          │
│ [1] 📋 View Audit Logs         Review all system activities     │
│ [2] 📊 Generate Reports        Compliance and activity reports   │
│ [3] 🔍 Search & Filter Logs    Find specific events/timeframes  │
│ [4] 📤 Export Audit Data       Download logs for analysis       │
│                                                                  │
│ Compliance Frameworks:                                           │
│ [5] 🛡️  SOC 2 Compliance       Service Organization Control 2    │
│ [6] 🌍 ISO 27001 Standards     Information Security Management   │
│ [7] 📜 GDPR Requirements       Data protection compliance        │
│ [8] 🏦 Financial Regulations   Banking and fintech standards     │
│                                                                  │
│ Security Monitoring:                                             │
│ [9] 🚨 Security Events         Failed attempts, anomalies       │
│ [A] 📈 Risk Assessment         Current security posture         │
│ [B] 🔐 Access Review           User permissions and roles        │
│ [C] 📝 Incident Documentation  Security incident tracking        │
│                                                                  │
│ Status: ✅ Compliant  Last Review: 2025-01-10  Next: 2025-04-10 │
│                                                                  │
│ [Enter] Select function  [R] Generate summary  [Esc] Back       │
└──────────────────────────────────────────────────────────────────┘
```

### 6.1 Audit Log Viewer
```
┌─ Audit Log Viewer ───────────────────────────────────────────────┐
│                                                                  │
│ Filters: [All Events ▼] [Last 7 days ▼] [All Users ▼]          │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ 2025-01-12 14:30:15  INFO   SESSION_JOIN                   │  │
│ │   User: mpc-node-alice  Session: company_treasury           │  │
│ │   Details: Successfully joined DKG session                 │  │
│ │   Result: SUCCESS  Duration: 234ms                         │  │
│ │                                                             │  │
│ │ 2025-01-12 14:28:42  WARN   AUTH_RETRY                     │  │
│ │   User: mpc-node-bob  Attempts: 2/3                        │  │
│ │   Details: Authentication failed, invalid signature        │  │
│ │   Result: RETRY  Source: 192.168.1.100                     │  │
│ │                                                             │  │
│ │ 2025-01-12 14:25:01  INFO   WALLET_CREATE                  │  │
│ │   User: mpc-node-alice  Wallet: project_alpha              │  │
│ │   Details: Wallet exported to backup location              │  │
│ │   Result: SUCCESS  Size: 1.2MB                             │  │
│ │                                                             │  │
│ │ 2025-01-12 14:20:33  ERROR  CONNECTION_FAILED              │  │
│ │   User: mpc-node-carol  Target: signaling-server           │  │
│ │   Details: Network timeout after 30s                       │  │
│ │   Result: FAILURE  Error: TIMEOUT                          │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ 📊 Summary: 1,247 events (1 error, 3 warnings, 1,243 info)    │
│                                                                  │
│ [D] Details  [F] Advanced filter  [E] Export  [Esc] Back       │
└──────────────────────────────────────────────────────────────────┘
```

### 6.2 Compliance Dashboard
```
┌─ Compliance Dashboard ───────────────────────────────────────────┐
│                                                                  │
│ Overall Compliance Status: 🟢 98.5% Compliant                   │
│                                                                  │
│ Framework Status:                                                │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ SOC 2 Type II:           ✅ Compliant   Last: Jan 2025     │  │
│ │ • Access Controls:       ✅ 100%        15/15 controls      │  │
│ │ • System Operations:     ✅ 100%        12/12 controls      │  │
│ │ • Change Management:     ✅ 100%        8/8 controls        │  │
│ │ • Risk Management:       ⚠️  95%         19/20 controls     │  │
│ │                                                             │  │
│ │ ISO 27001:               ✅ Compliant   Last: Dec 2024     │  │
│ │ • Information Security:  ✅ 100%        25/25 controls      │  │
│ │ • Risk Assessment:       ✅ 100%        10/10 controls      │  │
│ │ • Incident Management:   ✅ 100%        8/8 controls        │  │
│ │ • Business Continuity:   ⚠️  90%         9/10 controls     │  │
│ │                                                             │  │
│ │ GDPR:                    ✅ Compliant   Last: Jan 2025     │  │
│ │ • Data Protection:       ✅ 100%        Privacy by design   │  │
│ │ • User Rights:           ✅ 100%        Right to be forgotten│  │
│ │ • Breach Notification:   ✅ 100%        72-hour compliance  │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Action Items (2):                                                │
│ • Update business continuity documentation (Due: Jan 20)        │
│ • Complete risk management assessment (Due: Jan 25)             │
│                                                                  │
│ [R] Generate report  [A] View action items  [S] Schedule review │
│ [Esc] Back                                                       │
└──────────────────────────────────────────────────────────────────┘
```

---

## [7] Key Rotation & Management

### Main Screen Layout
```
┌─ Key Rotation & Management ──────────────────────────────────────┐
│                                                                  │
│ Key Lifecycle Management:                                        │
│                                                                  │
│ Rotation Operations:                                             │
│ [1] 🔄 Rotate Key Shares       Generate new threshold shares    │
│ [2] 👥 Update Participants     Add/remove/replace signers       │
│ [3] ⚙️  Change Threshold        Modify signature requirements    │
│ [4] 🔀 Migrate Curves          Change cryptographic curves      │
│                                                                  │
│ Participant Management:                                          │
│ [5] ➕ Add Participant         Expand signing group             │
│ [6] ➖ Remove Participant      Reduce signing group             │
│ [7] 🔄 Replace Participant     Substitute signer                │
│ [8] 🔍 Verify Participants     Check signer authenticity        │
│                                                                  │
│ Advanced Operations:                                             │
│ [9] 🚨 Emergency Key Freeze    Immediately disable all keys     │
│ [A] 🆘 Emergency Recovery      Restore from backup trustees     │
│ [B] 📊 Key Health Analysis     Assess cryptographic integrity   │
│ [C] 📋 Rotation History        Review past key changes          │
│                                                                  │
│ Next Scheduled Rotation: 2025-07-12 (6 months)                 │
│ Last Rotation: 2025-01-12 (Successful)                         │
│                                                                  │
│ [Enter] Select operation  [S] Schedule rotation  [Esc] Back     │
└──────────────────────────────────────────────────────────────────┘
```

### 7.1 Key Rotation Wizard
```
┌─ Key Rotation Wizard ────────────────────────────────────────────┐
│                                                                  │
│ Step 1 of 4: Rotation Planning                                  │
│                                                                  │
│ Select Wallet for Rotation:                                     │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ ● company_treasury                                           │  │
│ │   Current: 2-of-3, secp256k1, created 3 days ago          │  │
│ │   Status: Active, 15.7 ETH, last used 2 hours ago         │  │
│ │   Participants: alice, bob, carol                           │  │
│ │                                                             │  │
│ │ ○ project_alpha                                             │  │
│ │   Current: 3-of-5, secp256k1, created 5 days ago          │  │
│ │   Status: Active, 2.1 ETH, last used yesterday            │  │
│ │   Participants: alice, bob, carol, dave, eve               │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Rotation Type:                                                   │
│ ● Key Material Refresh (Keep same participants & threshold)     │
│ ○ Participant Update (Change signers)                          │
│ ○ Threshold Modification (Change m-of-n)                       │
│ ○ Complete Restructure (Change everything)                     │
│                                                                  │
│ Scheduling:                                                      │
│ ● Execute immediately                                           │
│ ○ Schedule for specific time: [2025-01-13 02:00] UTC          │
│ ○ During next maintenance window                               │
│                                                                  │
│ [Next] Continue  [S] Save as draft  [Esc] Cancel              │
└──────────────────────────────────────────────────────────────────┘
```

### 7.2 Participant Management
```
┌─ Participant Management ─────────────────────────────────────────┐
│                                                                  │
│ Current Participants for: company_treasury                       │
│                                                                  │
│ Active Participants (3):                                         │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ [1] ✅ mpc-node-alice (You)    Index: 1   Status: Online    │  │
│ │     Role: Administrator         Last seen: Now               │  │
│ │     Public Key: 0x04a1b2c3...  Joined: 2025-01-10          │  │
│ │                                                             │  │
│ │ [2] ✅ mpc-node-bob             Index: 2   Status: Online    │  │
│ │     Role: Participant           Last seen: 5 min ago        │  │
│ │     Public Key: 0x04d5e6f7...  Joined: 2025-01-10          │  │
│ │                                                             │  │
│ │ [3] ❌ mpc-node-carol           Index: 3   Status: Offline   │  │
│ │     Role: Participant           Last seen: 2 days ago       │  │
│ │     Public Key: 0x0489abcd...  Joined: 2025-01-10          │  │
│ │     ⚠️  Extended offline - consider replacement             │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Pending Invitations (1):                                        │
│ • mpc-node-dave (Invited: 2025-01-12 14:00, Expires: 24h)      │
│                                                                  │
│ Operations:                                                      │
│ [A] Add participant    [R] Remove participant                   │
│ [I] Send invitation    [C] Cancel invitation                    │
│ [T] Test connections   [V] Verify signatures                    │
│                                                                  │
│ [Enter] Select participant  [Esc] Back                          │
└──────────────────────────────────────────────────────────────────┘
```

---

## [8] Emergency Response

### Main Screen Layout
```
┌─ Emergency Response System ──────────────────────────────────────┐
│                                                                  │
│ 🚨 EMERGENCY RESPONSE CENTER 🚨                                  │
│                                                                  │
│ Immediate Actions:                                               │
│ [1] 🔒 EMERGENCY LOCKDOWN       Freeze all wallet operations    │
│ [2] ⚠️  SECURITY INCIDENT        Report and track security event │
│ [3] 🚫 REVOKE ACCESS            Immediately disable participant  │
│ [4] 📞 EMERGENCY CONTACTS       Notify security team            │
│                                                                  │
│ Threat Response:                                                 │
│ [5] 🕵️  FORENSIC ANALYSIS       Investigate security breach     │
│ [6] 🛡️  THREAT ASSESSMENT       Evaluate current risk level     │
│ [7] 📋 INCIDENT DOCUMENTATION   Record emergency procedures      │
│ [8] 🔄 RECOVERY PROCEDURES      Restore after emergency         │
│                                                                  │
│ Business Continuity:                                             │
│ [9] 💾 BACKUP ACTIVATION        Switch to backup systems        │
│ [A] 🌐 DISASTER RECOVERY        Full system recovery procedures │
│ [B] 📊 SYSTEM HEALTH CHECK     Verify all components            │
│ [C] 📞 STAKEHOLDER NOTIFY       Inform relevant parties         │
│                                                                  │
│ Current Status: 🟢 Normal Operations                            │
│ Threat Level: LOW    Last Check: 2025-01-12 14:30:00 UTC       │
│                                                                  │
│ ⚠️  Emergency procedures require additional authorization       │
│ [Enter] Select action  [S] System status  [Esc] Back           │
└──────────────────────────────────────────────────────────────────┘
```

### 8.1 Emergency Lockdown
```
┌─ EMERGENCY LOCKDOWN PROCEDURE ───────────────────────────────────┐
│                                                                  │
│ ⚠️  CRITICAL SECURITY OPERATION ⚠️                              │
│                                                                  │
│ LOCKDOWN SCOPE:                                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ What will be locked down:                                   │  │
│ │ [✓] All signing operations                                  │  │
│ │ [✓] New session creation                                    │  │
│ │ [✓] Wallet access                                           │  │
│ │ [✓] Key export/import                                       │  │
│ │ [ ] Read-only operations (view balances, etc.)             │  │
│ │                                                             │  │
│ │ Duration:                                                   │  │
│ │ ● Indefinite (manual unlock required)                      │  │
│ │ ○ Time-limited: [1 hour____] ○ [4 hours___] ○ [24 hours]  │  │
│ │                                                             │  │
│ │ Reason (required):                                          │  │
│ │ [Suspected security breach - unusual transaction patterns_] │  │
│ │ [_____________________________________________________]    │  │
│ │                                                             │  │
│ │ Notification:                                               │  │
│ │ [✓] Notify all participants immediately                    │  │
│ │ [✓] Send alert to security team                            │  │
│ │ [✓] Log to audit trail                                     │  │
│ │ [ ] Notify external authorities                            │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ ⚠️  THIS ACTION CANNOT BE UNDONE WITHOUT ADMIN APPROVAL         │
│                                                                  │
│ Authorization Required:                                          │
│ Admin Password: [________________] or Hardware Token            │
│                                                                  │
│ [L] EXECUTE LOCKDOWN  [C] Cancel  [H] Help                     │
└──────────────────────────────────────────────────────────────────┘
```

### 8.2 Incident Management
```
┌─ Security Incident Management ───────────────────────────────────┐
│                                                                  │
│ Active Incidents (1):                                            │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ INC-2025-001  🔴 HIGH SEVERITY                              │  │
│ │ Title: Unauthorized access attempts detected               │  │
│ │ Opened: 2025-01-12 14:15:00  Reporter: System Monitor     │  │
│ │ Status: Under Investigation   Assigned: Security Team      │  │
│ │                                                             │  │
│ │ Details: Multiple failed authentication attempts from      │  │
│ │ IP 192.168.1.999 targeting multiple participant accounts  │  │
│ │                                                             │  │
│ │ Actions Taken:                                              │  │
│ │ • IP blocked automatically                                 │  │
│ │ • Affected accounts notified                               │  │
│ │ • Enhanced monitoring activated                            │  │
│ │                                                             │  │
│ │ Next Steps: Forensic analysis in progress                  │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Recent Incidents (Closed):                                       │
│ • INC-2025-002: Network connectivity issues (RESOLVED)          │
│ • INC-2024-045: Failed DKG session (RESOLVED)                  │
│                                                                  │
│ Incident Management:                                             │
│ [N] New incident     [V] View details     [U] Update status     │
│ [A] Assign           [E] Escalate         [C] Close incident    │
│ [R] Generate report  [S] Search history   [Esc] Back           │
└──────────────────────────────────────────────────────────────────┘
```

---

## [9] Multi-Wallet Operations

### Main Screen Layout
```
┌─ Multi-Wallet Operations ────────────────────────────────────────┐
│                                                                  │
│ Portfolio Management:                                            │
│                                                                  │
│ Batch Operations:                                                │
│ [1] 📦 Batch Signing          Sign multiple transactions        │
│ [2] 🔄 Portfolio Rebalancing  Cross-wallet asset management     │
│ [3] 📊 Consolidated Reporting Generate combined reports         │
│ [4] 🔐 Batch Key Rotation     Rotate keys across wallets        │
│                                                                  │
│ Portfolio Analysis:                                              │
│ [5] 📈 Portfolio Dashboard    Overview of all wallet assets     │
│ [6] 💰 Total Asset Valuation  USD/crypto value calculations     │
│ [7] 📋 Transaction History    Unified activity across wallets   │
│ [8] 🎯 Risk Assessment        Portfolio risk analysis           │
│                                                                  │
│ Cross-Wallet Operations:                                         │
│ [9] 🔄 Cross-Chain Transfers  Move assets between chains        │
│ [A] 💱 DEX Aggregation        Multi-wallet DeFi operations      │
│ [B] 🏦 Yield Farming          Manage DeFi positions             │
│ [C] 📊 Tax Reporting          Generate tax documents            │
│                                                                  │
│ Portfolio Summary:                                               │
│ Total Wallets: 5    Total Value: $45,237.82    Change: +2.3%   │
│ Assets: ETH (65%), BTC (25%), SOL (8%), Stablecoins (2%)       │
│                                                                  │
│ [Enter] Select operation  [D] Dashboard view  [Esc] Back        │
└──────────────────────────────────────────────────────────────────┘
```

### 9.1 Portfolio Dashboard
```
┌─ Portfolio Dashboard ────────────────────────────────────────────┐
│                                                                  │
│ Total Portfolio Value: $45,237.82 USD (+2.3% / +$1,012.45)     │
│                                                                  │
│ Wallet Breakdown:                                                │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ 🏦 company_treasury    $28,450.23  (62.9%)  [2-of-3]       │  │
│ │    ETH: 15.7 ($25,120.50)  BTC: 0.25 ($10,500.00)         │  │
│ │    Last activity: 2 hours ago                               │  │
│ │                                                             │  │
│ │ 🚀 project_alpha       $8,234.15   (18.2%)  [3-of-5]       │  │
│ │    ETH: 2.1 ($3,360.30)  USDC: 4,873.85                   │  │
│ │    Last activity: Yesterday                                 │  │
│ │                                                             │  │
│ │ 💼 personal_backup     $5,678.90   (12.6%)  [2-of-2]       │  │
│ │    SOL: 45.2 ($4,517.80)  USDT: 1,161.10                  │  │
│ │    Last activity: 1 week ago                               │  │
│ │                                                             │  │
│ │ 🔒 emergency_funds     $2,874.54   (6.4%)   [4-of-7]       │  │
│ │    BTC: 0.065 ($2,730.00)  ETH: 0.09 ($144.54)            │  │
│ │    Status: 🔒 Locked                                        │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Asset Allocation:                                                │
│ ETH ████████████████████ 65%    BTC ████████ 25%               │
│ SOL ████ 8%    Stablecoins ██ 2%                               │
│                                                                  │
│ Recent Activity (24h):                                           │
│ • Received 0.5 ETH in company_treasury                          │
│ • Sent USDC 100 from project_alpha                              │
│                                                                  │
│ [R] Refresh prices  [T] Transaction details  [E] Export data    │
│ [S] Settings  [Esc] Back                                        │
└──────────────────────────────────────────────────────────────────┘
```

### 9.2 Batch Operations
```
┌─ Batch Signing Operations ───────────────────────────────────────┐
│                                                                  │
│ Select Operations to Execute:                                    │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ [✓] company_treasury → Send 1.0 ETH to 0x742d35Cc...       │  │
│ │     Gas: 21000  Fee: $12.45  Priority: Standard            │  │
│ │                                                             │  │
│ │ [✓] project_alpha → Send 500 USDC to 0xa1b2c3d4...         │  │
│ │     Gas: 65000  Fee: $8.23   Priority: Standard            │  │
│ │                                                             │  │
│ │ [ ] personal_backup → Claim staking rewards                 │  │
│ │     Gas: 120000  Fee: $15.67  Priority: Low                │  │
│ │                                                             │  │
│ │ [✓] emergency_funds → Unlock wallet (Admin required)       │  │
│ │     Operation: Administrative  Requires: 3-of-4 signatures │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Execution Settings:                                              │
│ Execution Order: ● Sequential  ○ Parallel (where possible)      │
│ Failure Handling: ● Stop on first failure  ○ Continue all      │
│ Confirmation: ● Required for each  ○ Batch confirmation        │
│                                                                  │
│ Total Estimated Cost: $36.35  Estimated Time: 15-30 minutes    │
│                                                                  │
│ Participants Required:                                           │
│ • mpc-node-alice (You): Required for all operations            │
│ • mpc-node-bob: Required for 3 operations                      │
│ • mpc-node-carol: Required for 2 operations                    │
│                                                                  │
│ [Enter] Execute batch  [P] Preview all  [S] Save as template   │
│ [Esc] Cancel                                                     │
└──────────────────────────────────────────────────────────────────┘
```

---

## [H] Help & Documentation

### Main Screen Layout
```
┌─ Help & Documentation ───────────────────────────────────────────┐
│                                                                  │
│ MPC Wallet TUI Help System:                                     │
│                                                                  │
│ Quick Start:                                                     │
│ [1] 🚀 Getting Started       First-time user walkthrough        │
│ [2] 📖 User Guide            Complete user manual               │
│ [3] 💡 Quick Tips            Common operations and shortcuts     │
│ [4] 🎯 Keyboard Shortcuts    Complete key binding reference     │
│                                                                  │
│ Advanced Topics:                                                 │
│ [5] 🔧 Technical Reference   Cryptographic details              │
│ [6] 🔒 Security Best Practices  Security recommendations       │
│ [7] 🌐 Network Configuration   Setup and troubleshooting       │
│ [8] 🏢 Enterprise Features    Business-specific functionality   │
│                                                                  │
│ Troubleshooting:                                                 │
│ [9] 🔍 Diagnostic Tools      System health and problem solving  │
│ [A] 📞 Support Resources     Contact information and community   │
│ [B] 🐛 Report Issue          Bug reporting and feedback         │
│ [C] 📚 FAQ                   Frequently asked questions         │
│                                                                  │
│ About:                                                           │
│ [D] ℹ️  About MPC Wallet     Version and license information    │
│ [E] 📄 Legal & Compliance    Terms, privacy, and regulations    │
│                                                                  │
│ Search Help: [____________________] 🔍                          │
│                                                                  │
│ [Enter] Select topic  [/] Search  [F1] Context help  [Esc] Back │
└──────────────────────────────────────────────────────────────────┘
```

### H.1 Interactive Tutorial
```
┌─ Getting Started Tutorial ───────────────────────────────────────┐
│                                                                  │
│ Step 1 of 8: Welcome to MPC Wallet                              │
│                                                                  │
│ 🎯 What you'll learn in this tutorial:                          │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ ✓ Understanding MPC and threshold signatures               │  │
│ │ • Creating your first wallet (next)                        │  │
│ │ • Inviting participants                                     │  │
│ │ • Completing the DKG process                               │  │
│ │ • Signing your first transaction                           │  │
│ │ • Backup and security practices                            │  │
│ │ • Advanced features overview                               │  │
│ │ • Getting help and support                                 │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ 📚 MPC (Multi-Party Computation) allows multiple parties to     │
│ jointly control a wallet without any single party having        │
│ access to the complete private key. This provides enhanced      │
│ security through distributed trust.                             │
│                                                                  │
│ Example: A 2-of-3 wallet requires 2 out of 3 participants      │
│ to agree and sign any transaction.                              │
│                                                                  │
│ Tutorial Options:                                                │
│ ● Interactive walkthrough (recommended for beginners)          │
│ ○ Skip tutorial and explore freely                             │
│ ○ Advanced user - show key features only                       │
│                                                                  │
│ [Enter] Continue  [S] Skip tutorial  [Q] Quick tour  [Esc] Exit │
└──────────────────────────────────────────────────────────────────┘
```

### H.2 Keyboard Reference
```
┌─ Keyboard Shortcuts Reference ───────────────────────────────────┐
│                                                                  │
│ Global Shortcuts (Available on all screens):                    │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Navigation:                                                 │  │
│ │ ↑↓←→          Navigate menus and lists                      │  │
│ │ Tab/Shift+Tab Form field navigation                         │  │
│ │ Enter         Confirm/Select/Submit                         │  │
│ │ Escape        Back/Cancel/Close                             │  │
│ │ Home/End      Jump to first/last item                       │  │
│ │ Page Up/Down  Navigate large lists                          │  │
│ │                                                             │  │
│ │ Quick Actions:                                              │  │
│ │ Ctrl+Q        Quit application                              │  │
│ │ Ctrl+R        Refresh/Reload current view                   │  │
│ │ Ctrl+L        Clear screen/Redraw                           │  │
│ │ F1            Context-sensitive help                        │  │
│ │ F5            Refresh data                                  │  │
│ │ ?             Show help overlay                             │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Main Menu Shortcuts:                                             │
│ 1-9: Select menu items    M: Main menu    H: Help               │
│ W: Wallets    S: Settings    A: Audit    E: Emergency            │
│                                                                  │
│ Advanced Shortcuts:                                              │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Alt+1-9       Switch between open tabs/views                │  │
│ │ Ctrl+S        Save current state                            │  │
│ │ Ctrl+E        Export current data                           │  │
│ │ Ctrl+F        Find/Search                                   │  │
│ │ Ctrl+D        Toggle debug mode                             │  │
│ │ Ctrl+T        Open new session                              │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [P] Print reference  [C] Customize shortcuts  [Esc] Back        │
└──────────────────────────────────────────────────────────────────┘
```

---

## [Q] Quit Application

### Quit Confirmation
```
┌─ Quit Application ───────────────────────────────────────────────┐
│                                                                  │
│ Confirm Application Exit                                         │
│                                                                  │
│ Current Activity Status:                                         │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Active Sessions: 1                                          │  │
│ │ • company_treasury DKG (Round 2 of 2, 85% complete)        │  │
│ │                                                             │  │
│ │ Pending Operations: 2                                       │  │
│ │ • Signing request from mpc-node-bob                         │  │
│ │ • Background sync in progress                               │  │
│ │                                                             │  │
│ │ Unsaved Changes: None                                       │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ ⚠️  Warning: Exiting now may interrupt active operations        │
│                                                                  │
│ Exit Options:                                                    │
│ ● Safe Exit - Complete current operations first (recommended)   │
│ ○ Force Exit - Terminate immediately (may cause data loss)     │
│ ○ Background Mode - Continue operations in background          │
│ ○ Save and Exit - Save state for later resumption             │
│                                                                  │
│ Auto-save: ✅ Enabled    Session backup: ✅ Enabled            │
│                                                                  │
│ [Enter] Confirm exit  [B] Background mode  [C] Cancel          │
│ [S] Save and exit     [F] Force quit                           │
└──────────────────────────────────────────────────────────────────┘
```

---

## Common UI Patterns and Components

### Status Indicators
- 🟢 Online/Active/Success
- 🟡 Warning/Pending/In Progress  
- 🔴 Error/Failed/Critical
- ⚫ Offline/Disabled/Unknown
- 🔒 Locked/Secured
- ✅ Completed/Verified
- ⚠️  Warning/Attention Required

### Progress Indicators
```
████████░░░░░░░ 50% (Text Progress Bar)
[████████████████████████████████████████] 100%
Processing... (spinner equivalent in text)
Step 3 of 7 (Step Indicator)
```

### Form Validation
- Real-time validation with inline error messages
- Required fields marked with (required) or *
- Success indicators for valid inputs
- Progressive disclosure of advanced options

### Confirmation Dialogs
- Multiple levels for destructive actions
- Clear explanations of consequences
- Default to safe options (No/Cancel)
- Require explicit confirmation for critical operations

### Navigation Breadcrumbs
```
Main Menu > Settings > Network Settings > Advanced Configuration
```

### Contextual Help
- F1 key always shows context-specific help
- ? key shows help overlay on current screen
- Inline hints for complex operations
- Links to relevant documentation sections

This comprehensive submenu design provides enterprise-grade functionality while maintaining usability for both technical and non-technical users. The consistent navigation patterns, clear visual hierarchy, and progressive disclosure ensure that users can efficiently access both simple and advanced features while maintaining security and compliance requirements.