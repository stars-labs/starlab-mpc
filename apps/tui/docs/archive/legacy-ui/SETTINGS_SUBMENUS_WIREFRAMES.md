# Settings & Configuration Submenu Wireframes

This document contains detailed wireframes for all settings and configuration submenus in the MPC wallet TUI application.

## Table of Contents

1. [Settings Main Menu](#settings-main-menu)
2. [Network Settings](#network-settings)
3. [WebRTC Configuration](#webrtc-configuration)
4. [Security Policies](#security-policies)
5. [Connection Profiles](#connection-profiles)
6. [Display Preferences](#display-preferences)
7. [Keyboard Shortcuts](#keyboard-shortcuts)
8. [Notifications](#notifications)
9. [Data Management](#data-management)
10. [Logging & Diagnostics](#logging--diagnostics)
11. [Enterprise Policies](#enterprise-policies)

---

## Settings Main Menu

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

---

## Network Settings

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

### Network Diagnostics Screen

```
┌─ Network Diagnostics ────────────────────────────────────────────┐
│                                                                  │
│ Running Network Tests...                                         │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Test Results:                                               │  │
│ │                                                             │  │
│ │ Primary Server (wss://auto-life.tech):                     │  │
│ │ ✅ Connection: Success (45ms)                               │  │
│ │ ✅ WebSocket handshake: Success                             │  │
│ │ ✅ Authentication: Success                                  │  │
│ │ ✅ Keep-alive: Working                                      │  │
│ │                                                             │  │
│ │ Fallback Server 1 (backup.auto-life.tech):                 │  │
│ │ ✅ Connection: Success (52ms)                               │  │
│ │ ✅ Failover test: Working                                   │  │
│ │                                                             │  │
│ │ Network Quality:                                            │  │
│ │ • Latency: 45ms (Good)                                     │  │
│ │ • Packet loss: 0% (Excellent)                              │  │
│ │ • Bandwidth: 10.2 Mbps (Sufficient)                        │  │
│ │ • Jitter: 2ms (Stable)                                     │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Overall Status: ✅ All systems operational                      │
│                                                                  │
│ [R] Re-run tests  [E] Export report  [Esc] Back               │
└──────────────────────────────────────────────────────────────────┘
```

---

## WebRTC Configuration

```
┌─ WebRTC Configuration ───────────────────────────────────────────┐
│                                                                  │
│ STUN/TURN Server Settings:                                      │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ STUN Servers:                                               │  │
│ │ [✓] stun:stun.l.google.com:19302                          │  │
│ │ [✓] stun:stun1.l.google.com:19302                         │  │
│ │ [ ] stun:stun.stunprotocol.org:3478                       │  │
│ │ [+] Add custom STUN server                                  │  │
│ │                                                             │  │
│ │ TURN Server Configuration:                                  │  │
│ │ URL: [turn:turn.auto-life.tech:3478________]               │  │
│ │ Username: [user123_________________________]               │  │
│ │ Password: [••••••••••••••••••••____________]               │  │
│ │ [ ] Use long-term credentials                              │  │
│ │                                                             │  │
│ │ ICE Configuration:                                          │  │
│ │ [✓] Enable ICE trickle                                     │  │
│ │ [✓] Use aggressive nomination                              │  │
│ │ [ ] Force relay (TURN only)                                │  │
│ │ Gathering timeout: [10s____]                               │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Connection Quality Settings:                                     │
│ • Video codec: Disabled (MPC only)                              │
│ • Audio codec: Disabled (MPC only)                              │
│ • Data channel: Enabled (Required)                              │
│ • Max packet size: [16384] bytes                                │
│                                                                  │
│ [T] Test configuration  [S] Save  [R] Reset  [Esc] Cancel      │
└──────────────────────────────────────────────────────────────────┘
```

---

## Security Policies

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

### IP Whitelist Configuration

```
┌─ IP Whitelist Configuration ─────────────────────────────────────┐
│                                                                  │
│ Allowed IP Addresses and Ranges:                                │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Current Whitelist:                                          │  │
│ │                                                             │  │
│ │ [✓] 192.168.1.0/24      Local network                     │  │
│ │ [✓] 10.0.0.0/8          Corporate VPN                     │  │
│ │ [✓] 203.0.113.45        Office static IP                  │  │
│ │ [ ] 0.0.0.0/0           Allow all (NOT recommended)       │  │
│ │                                                             │  │
│ │ Add New IP/Range:                                           │  │
│ │ IP/CIDR: [_____________________] Description: [__________] │  │
│ │ [+] Add to whitelist                                       │  │
│ │                                                             │  │
│ │ Your Current IP: 192.168.1.100 ✅ (Allowed)               │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Security Warning: Enabling IP whitelist may lock you out if     │
│ your IP changes. Ensure you have recovery access configured.    │
│                                                                  │
│ [S] Save whitelist  [T] Test current IP  [D] Disable          │
│ [Esc] Cancel                                                     │
└──────────────────────────────────────────────────────────────────┘
```

---

## Connection Profiles

```
┌─ Connection Profiles ────────────────────────────────────────────┐
│                                                                  │
│ Manage Network Profiles:                                         │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Available Profiles:                                         │  │
│ │                                                             │  │
│ │ ● Production (Active)                                       │  │
│ │   Server: wss://auto-life.tech                            │  │
│ │   Security: High, Timeouts: Standard                       │  │
│ │                                                             │  │
│ │ ○ Development                                               │  │
│ │   Server: ws://localhost:8080                              │  │
│ │   Security: Relaxed, Timeouts: Extended                    │  │
│ │                                                             │  │
│ │ ○ Offline/Air-gapped                                       │  │
│ │   Server: None                                             │  │
│ │   Security: Maximum, Manual coordination                   │  │
│ │                                                             │  │
│ │ ○ Custom Profile 1                                          │  │
│ │   Server: wss://private.company.com                        │  │
│ │   Security: Custom, Corporate policies                     │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Profile Actions:                                                 │
│ [S] Switch profile  [E] Edit  [N] New profile  [D] Delete      │
│ [I] Import profile  [X] Export  [Esc] Back                     │
└──────────────────────────────────────────────────────────────────┘
```

### Edit Profile Screen

```
┌─ Edit Profile: Production ───────────────────────────────────────┐
│                                                                  │
│ Profile Configuration:                                           │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Profile Name: [Production_______________]                   │  │
│ │ Description: [Main production environment]                  │  │
│ │                                                             │  │
│ │ Network Settings:                                           │  │
│ │ Primary server: [wss://auto-life.tech____]                 │  │
│ │ Backup server: [wss://backup.auto-life.tech]               │  │
│ │ Connection timeout: [30s___]                                │  │
│ │                                                             │  │
│ │ Security Level:                                             │  │
│ │ ○ Low (Development only)                                    │  │
│ │ ● Standard (Recommended)                                    │  │
│ │ ○ High (Enterprise)                                         │  │
│ │ ○ Maximum (Air-gapped)                                      │  │
│ │                                                             │  │
│ │ Special Settings:                                           │  │
│ │ [✓] Auto-reconnect on failure                             │  │
│ │ [✓] Enable connection pooling                             │  │
│ │ [ ] Require VPN connection                                │  │
│ │ [ ] Restrict to office hours                              │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [S] Save changes  [T] Test profile  [R] Reset  [Esc] Cancel    │
└──────────────────────────────────────────────────────────────────┘
```

---

## Display Preferences

```
┌─ Display Preferences ────────────────────────────────────────────┐
│                                                                  │
│ Visual Settings:                                                 │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Theme:                                                      │  │
│ │ ● Dark (Default)    ○ Light    ○ High Contrast            │  │
│ │                                                             │  │
│ │ Color Scheme:                                               │  │
│ │ ● Professional Blue   ○ Matrix Green   ○ Monochrome       │  │
│ │                                                             │  │
│ │ Interface Density:                                          │  │
│ │ ○ Compact    ● Normal    ○ Comfortable                    │  │
│ │                                                             │  │
│ │ Font Settings:                                              │  │
│ │ Size: [12pt ▼]  Family: [Monospace ▼]                     │  │
│ │                                                             │  │
│ │ Display Options:                                            │  │
│ │ [✓] Show status icons                                     │  │
│ │ [✓] Enable animations                                      │  │
│ │ [✓] Show tooltips                                          │  │
│ │ [ ] Transparent background                                 │  │
│ │ [✓] Show connection status                                 │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Preview: [Your interface preview appears here]                  │
│                                                                  │
│ [A] Apply  [P] Preview  [R] Reset to defaults  [Esc] Cancel    │
└──────────────────────────────────────────────────────────────────┘
```

---

## Keyboard Shortcuts

```
┌─ Keyboard Shortcuts ─────────────────────────────────────────────┐
│                                                                  │
│ Customize Key Bindings:                                          │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Global Shortcuts:                                           │  │
│ │                                                             │  │
│ │ Quit application:        [Ctrl+Q____] (Default: Ctrl+Q)    │  │
│ │ Show help:              [F1________] (Default: F1)         │  │
│ │ Refresh data:           [F5________] (Default: F5)         │  │
│ │ Toggle debug mode:      [Ctrl+D____] (Default: Ctrl+D)     │  │
│ │                                                             │  │
│ │ Navigation:                                                 │  │
│ │ Main menu:              [M_________] (Default: M)          │  │
│ │ Back/Cancel:            [Esc_______] (Default: Esc)        │  │
│ │ Confirm/Select:         [Enter_____] (Default: Enter)      │  │
│ │                                                             │  │
│ │ Wallet Operations:                                          │  │
│ │ Quick sign:             [Ctrl+S____] (Default: Ctrl+S)     │  │
│ │ Create wallet:          [Ctrl+N____] (Default: Ctrl+N)     │  │
│ │ Export wallet:          [Ctrl+E____] (Default: Ctrl+E)     │  │
│ │                                                             │  │
│ │ ⚠️  Conflicts: None detected                                │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [S] Save bindings  [R] Reset all  [I] Import  [X] Export       │
│ [Esc] Cancel                                                     │
└──────────────────────────────────────────────────────────────────┘
```

---

## Notifications

```
┌─ Notification Settings ──────────────────────────────────────────┐
│                                                                  │
│ Alert Preferences:                                               │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Desktop Notifications:                                      │  │
│ │ [✓] Enable desktop notifications                           │  │
│ │ [✓] Play sound alerts                                      │  │
│ │ Sound: [Default beep ▼]  Volume: [████████░░] 80%         │  │
│ │                                                             │  │
│ │ Notification Types:                                         │  │
│ │ [✓] Session invitations         Priority: ● High           │  │
│ │ [✓] Signing requests           Priority: ● High           │  │
│ │ [✓] Connection status changes   Priority: ○ Medium         │  │
│ │ [✓] Wallet operations complete  Priority: ○ Medium         │  │
│ │ [ ] Debug messages             Priority: ○ Low            │  │
│ │                                                             │  │
│ │ Do Not Disturb:                                             │  │
│ │ [ ] Enable DND mode                                        │  │
│ │ Schedule: From [22:00] to [08:00]                          │  │
│ │ [ ] DND during signing operations                          │  │
│ │                                                             │  │
│ │ External Integrations:                                      │  │
│ │ [ ] Send to email: [____________________]                  │  │
│ │ [ ] Webhook URL: [_______________________]                 │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [T] Test notification  [S] Save  [R] Reset  [Esc] Cancel       │
└──────────────────────────────────────────────────────────────────┘
```

---

## Data Management

```
┌─ Data Management ────────────────────────────────────────────────┐
│                                                                  │
│ Storage Configuration:                                           │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Data Locations:                                             │  │
│ │                                                             │  │
│ │ Keystore directory:                                         │  │
│ │ [~/.starlab-mpc/keystore_______________] [Browse]           │  │
│ │ Current size: 45.2 MB                                       │  │
│ │                                                             │  │
│ │ Log directory:                                              │  │
│ │ [~/.starlab-mpc/logs__________________] [Browse]           │  │
│ │ Current size: 128.5 MB                                      │  │
│ │                                                             │  │
│ │ Cache directory:                                            │  │
│ │ [~/.starlab-mpc/cache_________________] [Browse]           │  │
│ │ Current size: 512.3 MB                                      │  │
│ │                                                             │  │
│ │ Cleanup Settings:                                           │  │
│ │ [✓] Auto-cleanup logs older than [30] days                │  │
│ │ [✓] Limit cache size to [1GB___]                          │  │
│ │ [ ] Compress old backups                                   │  │
│ │                                                             │  │
│ │ Database Maintenance:                                       │  │
│ │ Last cleanup: 2025-01-10 (2 days ago)                     │  │
│ │ [C] Clean now  [O] Optimize database  [V] Verify integrity │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Total disk usage: 686.0 MB    Free space: 45.2 GB             │
│                                                                  │
│ [S] Save settings  [B] Backup data  [R] Reset  [Esc] Cancel   │
└──────────────────────────────────────────────────────────────────┘
```

---

## Logging & Diagnostics

```
┌─ Logging & Diagnostics ──────────────────────────────────────────┐
│                                                                  │
│ Logging Configuration:                                           │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Log Level:                                                  │  │
│ │ ○ Error (Minimal)                                           │  │
│ │ ○ Warning                                                   │  │
│ │ ● Info (Recommended)                                        │  │
│ │ ○ Debug (Verbose)                                           │  │
│ │ ○ Trace (Everything)                                        │  │
│ │                                                             │  │
│ │ Log Categories:                                             │  │
│ │ [✓] Network operations      Level: [Info ▼]               │  │
│ │ [✓] Cryptographic ops       Level: [Warning ▼]            │  │
│ │ [✓] Session management      Level: [Info ▼]               │  │
│ │ [✓] UI events              Level: [Error ▼]               │  │
│ │ [ ] Performance metrics     Level: [Debug ▼]              │  │
│ │                                                             │  │
│ │ Output Settings:                                            │  │
│ │ [✓] Log to file            Max size: [100MB___]           │  │
│ │ [ ] Log to console         (Development only)              │  │
│ │ [ ] Send to syslog         Server: [__________]           │  │
│ │                                                             │  │
│ │ Privacy:                                                    │  │
│ │ [✓] Redact sensitive data (keys, addresses)               │  │
│ │ [✓] Anonymize IP addresses                                │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [V] View logs  [E] Export logs  [C] Clear logs  [S] Save       │
│ [D] Run diagnostics  [Esc] Cancel                               │
└──────────────────────────────────────────────────────────────────┘
```

### Diagnostics Report Screen

```
┌─ System Diagnostics Report ──────────────────────────────────────┐
│                                                                  │
│ Running System Diagnostics...                                    │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ System Information:                                         │  │
│ │ • OS: Linux 5.15.0                                         │  │
│ │ • Architecture: x86_64                                      │  │
│ │ • Memory: 16.0 GB (4.2 GB available)                      │  │
│ │ • CPU: Intel Core i7-9700K @ 3.60GHz                      │  │
│ │                                                             │  │
│ │ Application Status:                                         │  │
│ │ • Version: 2.0.0                                           │  │
│ │ • Uptime: 2 days, 14:32:15                                │  │
│ │ • Active sessions: 2                                        │  │
│ │ • Memory usage: 245 MB                                      │  │
│ │                                                             │  │
│ │ Dependencies Check:                                         │  │
│ │ ✅ Rust runtime: 1.75.0                                    │  │
│ │ ✅ OpenSSL: 3.0.2                                          │  │
│ │ ✅ libsodium: 1.0.18                                       │  │
│ │ ✅ Network stack: Operational                              │  │
│ │                                                             │  │
│ │ Performance Metrics:                                        │  │
│ │ • Average response time: 45ms                              │  │
│ │ • Cryptographic ops/sec: 1,250                             │  │
│ │ • Network throughput: 2.5 MB/s                             │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ All systems: ✅ Operational                                     │
│                                                                  │
│ [E] Export report  [S] Send to support  [R] Re-run  [Esc] Back │
└──────────────────────────────────────────────────────────────────┘
```

---

## Enterprise Policies

```
┌─ Enterprise Policies ────────────────────────────────────────────┐
│                                                                  │
│ Organization Settings:                                           │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Company: [ACME Corporation_______________]                 │  │
│ │ Policy Server: [https://policy.acme.com__]                 │  │
│ │ Last sync: 2025-01-12 08:00:00 UTC                        │  │
│ │                                                             │  │
│ │ Enforced Policies:                                          │  │
│ │ ✅ Minimum threshold: 3-of-5                               │  │
│ │ ✅ Session timeout: Max 24 hours                           │  │
│ │ ✅ Mandatory audit logging                                 │  │
│ │ ✅ IP whitelist required                                   │  │
│ │ ⚠️  Backup frequency: Every 7 days (Due in 2 days)         │  │
│ │                                                             │  │
│ │ Compliance Requirements:                                    │  │
│ │ [✓] SOC 2 Type II compliance mode                         │  │
│ │ [✓] FIPS 140-2 cryptography                               │  │
│ │ [✓] Data residency: US-only                               │  │
│ │                                                             │  │
│ │ User Restrictions:                                          │  │
│ │ • Max wallets per user: 10                                 │  │
│ │ • Signing limit: $100,000/day                              │  │
│ │ • Require 2FA for all operations                          │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Policy Status: ✅ Compliant    Next audit: 2025-02-01         │
│                                                                  │
│ [S] Sync policies  [V] Verify compliance  [R] Report  [Esc] Back│
└──────────────────────────────────────────────────────────────────┘
```

This comprehensive settings submenu wireframe document provides detailed layouts for all configuration options, maintaining consistency with the enterprise-grade BitGo-like interface while ensuring accessibility for both technical and non-technical users.