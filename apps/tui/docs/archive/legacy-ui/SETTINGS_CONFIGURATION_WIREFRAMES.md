# Settings & Configuration Path Wireframes

This document provides comprehensive wireframes for all settings and configuration screens in the MPC wallet TUI, enabling enterprise-grade customization and control.

## Table of Contents

1. [Settings Dashboard](#settings-dashboard)
2. [Network Configuration](#network-configuration)
3. [Security Settings](#security-settings)
4. [Notification Management](#notification-management)
5. [Advanced Options](#advanced-options)
6. [Display & Interface](#display--interface)
7. [Compliance Settings](#compliance-settings)
8. [Profile Management](#profile-management)

---

## Settings Dashboard

Main settings overview with quick access to all configuration categories.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SETTINGS & CONFIGURATION                            │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                    Profile: Production  [Switch Profile] │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  System Overview:                                                │     │
│   │  ┌──────────────────────────────────────────────────────────┐  │     │
│   │  │ Version:        2.0.1          Uptime: 45d 12h 23m      │  │     │
│   │  │ Environment:    Production     Last Update: 5 days ago  │  │     │
│   │  │ Config Hash:    0xA3F2...8B1   Auto-backup: ● Enabled  │  │     │
│   │  └──────────────────────────────────────────────────────────┘  │     │
│   │                                                                   │     │
│   │  Configuration Categories:                                       │     │
│   │                                                                   │     │
│   │  ┌─────────────────────┐  ┌─────────────────────┐            │     │
│   │  │ > 🌐 Network        │  │   🔒 Security       │            │     │
│   │  │   Connections: 3/3  │  │   Level: High       │            │     │
│   │  │   Status: ● Online  │  │   2FA: ✓ Enabled    │            │     │
│   │  │   [Configure →]     │  │   [Configure →]     │            │     │
│   │  └─────────────────────┘  └─────────────────────┘            │     │
│   │                                                                   │     │
│   │  ┌─────────────────────┐  ┌─────────────────────┐            │     │
│   │  │   🔔 Notifications  │  │   ⚙️ Advanced        │            │     │
│   │  │   Active: 12        │  │   Dev Mode: OFF     │            │     │
│   │  │   Pending: 3        │  │   Logging: INFO     │            │     │
│   │  │   [Configure →]     │  │   [Configure →]     │            │     │
│   │  └─────────────────────┘  └─────────────────────┘            │     │
│   │                                                                   │     │
│   │  ┌─────────────────────┐  ┌─────────────────────┐            │     │
│   │  │   🎨 Display        │  │   📋 Compliance     │            │     │
│   │  │   Theme: Dark      │  │   Status: ✓ OK      │            │     │
│   │  │   Font: 14px       │  │   Next Audit: 15d   │            │     │
│   │  │   [Configure →]     │  │   [Configure →]     │            │     │
│   │  └─────────────────────┘  └─────────────────────┘            │     │
│   │                                                                   │     │
│   │  Quick Actions:                                                 │     │
│   │  [E] Export Config  [I] Import  [R] Reset  [B] Backup         │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Last Change: 2024-01-20 14:32:00 by admin          Changes: 0 unsaved     │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Select  [S] Save All  [Q] Quick Settings           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Network Configuration

Detailed network settings including WebSocket, WebRTC, and blockchain connections.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         NETWORK CONFIGURATION                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings         Connection Status: ● All Systems Operational    │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  WebSocket Servers:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Primary Server Configuration:                            │   │     │
│   │  │ URL:      [wss://auto-life.tech________________]       │   │     │
│   │  │ Status:   ● Connected (latency: 23ms)                  │   │     │
│   │  │ Protocol: [v2] [v3] [Auto-negotiate]                   │   │     │
│   │  │                                                           │   │     │
│   │  │ Backup Servers:                                          │   │     │
│   │  │ 1. [wss://backup1.auto-life.tech_________] ● Ready     │   │     │
│   │  │ 2. [wss://backup2.auto-life.tech_________] ● Ready     │   │     │
│   │  │ 3. [_____________________________________] [+Add]      │   │     │
│   │  │                                                           │   │     │
│   │  │ Connection Parameters:                                   │   │     │
│   │  │ Timeout:        [30] seconds                           │   │     │
│   │  │ Retry Attempts: [5]  Retry Delay: [1000] ms           │   │     │
│   │  │ Heartbeat:      [30] seconds                           │   │     │
│   │  │ [✓] Auto-reconnect  [✓] Connection pooling            │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  WebRTC Configuration:                                           │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ ICE Servers:                                            │   │     │
│   │  │ STUN:                                                   │   │     │
│   │  │ • [stun:stun.l.google.com:19302__________] ✓          │   │     │
│   │  │ • [stun:stun1.l.google.com:19302_________] ✓          │   │     │
│   │  │ • [stun:stun.services.mozilla.com________] ✓          │   │     │
│   │  │                                                           │   │     │
│   │  │ TURN:                                                   │   │     │
│   │  │ URL:  [turn:turn.auto-life.tech:3478_____]             │   │     │
│   │  │ User: [turnuser123_____________________]               │   │     │
│   │  │ Pass: [••••••••••••••••••••___________]               │   │     │
│   │  │                                                           │   │     │
│   │  │ Advanced WebRTC:                                        │   │     │
│   │  │ [✓] Enable ICE trickle    Max Peers: [10]             │   │     │
│   │  │ [✓] Enable DTLS          Bundle Policy: [balanced]    │   │     │
│   │  │ [✓] IPv6 support         Codec: [opus] [VP8]         │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  [T] Test All Connections  [D] Diagnostics  [L] View Logs      │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Changes: 3 unsaved                    [S] Save  [R] Reset  [C] Cancel      │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Next Field  [T] Test  [S] Save  [R] Reset  [Esc] Back              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Security Settings

Comprehensive security configuration options.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SECURITY SETTINGS                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings                    Security Score: 92/100 (Excellent)   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Authentication Settings:                                        │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Password Policy:                                         │   │     │
│   │  │ Min Length:        [16] characters                      │   │     │
│   │  │ Complexity:        [✓] Uppercase  [✓] Numbers          │   │     │
│   │  │                    [✓] Lowercase  [✓] Special          │   │     │
│   │  │ Expiry:           [90] days                            │   │     │
│   │  │ History:          [12] (prevent reuse)                 │   │     │
│   │  │                                                           │   │     │
│   │  │ Multi-Factor Authentication:                             │   │     │
│   │  │ [✓] Enable 2FA (Required)                               │   │     │
│   │  │ Methods:          [✓] TOTP    [✓] Hardware Key        │   │     │
│   │  │                    [✓] SMS     [ ] Biometric           │   │     │
│   │  │ Backup Codes:     [Generate New Set]                   │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Encryption & Key Management:                                    │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Key Derivation Function:                                │   │     │
│   │  │ Algorithm:   [⚫ Argon2id] [○ PBKDF2] [○ Scrypt]       │   │     │
│   │  │ Memory:      [64] MB                                    │   │     │
│   │  │ Iterations:  [3]                                        │   │     │
│   │  │ Parallelism: [4]                                        │   │     │
│   │  │                                                           │   │     │
│   │  │ Storage Encryption:                                      │   │     │
│   │  │ Algorithm:   [⚫ AES-256-GCM] [○ ChaCha20-Poly1305]   │   │     │
│   │  │ Key Rotation: Every [30] days                          │   │     │
│   │  │ [✓] Encrypt at rest  [✓] Secure memory wipe           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Access Control:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Session Management:                                      │   │     │
│   │  │ Timeout:          [30] minutes (idle)                   │   │     │
│   │  │ Max Sessions:     [3] per user                          │   │     │
│   │  │ Lock After:       [3] failed attempts                   │   │     │
│   │  │ Lock Duration:    [30] minutes                          │   │     │
│   │  │                                                           │   │     │
│   │  │ IP Restrictions:                                         │   │     │
│   │  │ [✓] Enable IP allowlist                                │   │     │
│   │  │ Allowed Networks:                                       │   │     │
│   │  │ • 192.168.1.0/24  (Office Network)                     │   │     │
│   │  │ • 10.0.0.0/8      (VPN)                                │   │     │
│   │  │ • [Add New Range...]                                    │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ [A] Run Security Audit  [H] Hardening Wizard  [E] Export Policy           │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Navigate  [Space] Toggle  [S] Save  [A] Audit  [Esc] Back           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Notification Management

Configure alerts, notifications, and communication preferences.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         NOTIFICATION SETTINGS                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings                          Active Rules: 24  Muted: 3     │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Notification Channels:                                          │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] Email Notifications                                  │   │     │
│   │  │     Server: [smtp.company.com:587______________]        │   │     │
│   │  │     From:   [alerts@company.com_______________]         │   │     │
│   │  │     Auth:   [username_________] [••••••••••••]          │   │     │
│   │  │                                                           │   │     │
│   │  │ [✓] SMS Alerts                                           │   │     │
│   │  │     Provider: [Twilio] [Nexmo] [AWS SNS]                │   │     │
│   │  │     API Key:  [••••••••••••••••••••••••••••••]         │   │     │
│   │  │                                                           │   │     │
│   │  │ [✓] Webhook Integration                                  │   │     │
│   │  │     URL: [https://api.company.com/webhooks____]         │   │     │
│   │  │     Auth: [Bearer ••••••••••••••••••••••••••]          │   │     │
│   │  │                                                           │   │     │
│   │  │ [✓] In-App Notifications                                 │   │     │
│   │  │ [ ] Slack Integration                                    │   │     │
│   │  │ [ ] PagerDuty Integration                               │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Alert Rules:                                                    │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Event Type         Severity  Channels        Status     │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ > Failed Login     High      Email,SMS       ● Active   │   │     │
│   │  │   After [3] attempts, alert [security-team]            │   │     │
│   │  │                                                           │   │     │
│   │  │   Large Transfer   Critical  All             ● Active   │   │     │
│   │  │   Transfers over [$10,000] require approval            │   │     │
│   │  │                                                           │   │     │
│   │  │   New Device       Medium    Email           ● Active   │   │     │
│   │  │   Unknown device access attempts                        │   │     │
│   │  │                                                           │   │     │
│   │  │   Key Rotation     Info      Email           ○ Muted    │   │     │
│   │  │   Scheduled rotation reminders                          │   │     │
│   │  │                                                           │   │     │
│   │  │ [+] Add New Rule                                         │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Notification Schedule:                                          │     │
│   │  Business Hours: [09:00] - [18:00] [EST]                       │     │
│   │  [✓] Urgent alerts override schedule                           │     │
│   │  [✓] Batch non-urgent notifications                            │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Test Mode: [T] Send Test Alert         Last Alert: 2 hours ago            │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Space] Toggle  [E] Edit Rule  [T] Test  [S] Save          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Advanced Options

Developer settings, performance tuning, and system optimization.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ADVANCED OPTIONS                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings              ⚠ Warning: Expert settings - Use caution   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Developer Options:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [ ] Enable Developer Mode                                │   │     │
│   │  │     Unlock experimental features and debugging tools     │   │     │
│   │  │                                                           │   │     │
│   │  │ Logging Configuration:                                   │   │     │
│   │  │ Log Level:  [○ ERROR] [○ WARN] [⚫ INFO] [○ DEBUG]     │   │     │
│   │  │ Log Output: [✓] File  [✓] Console  [ ] Remote          │   │     │
│   │  │ Max Size:   [100] MB   Rotation: [7] days             │   │     │
│   │  │ Path:       [/var/log/starlab-mpc/____________]         │   │     │
│   │  │                                                           │   │     │
│   │  │ Debug Features:                                          │   │     │
│   │  │ [ ] Enable transaction simulation                       │   │     │
│   │  │ [ ] Show raw protocol messages                         │   │     │
│   │  │ [ ] Extended error reporting                           │   │     │
│   │  │ [ ] Performance profiling                              │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Performance Tuning:                                             │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Connection Pool:                                         │   │     │
│   │  │ Max Connections:    [100]                               │   │     │
│   │  │ Idle Timeout:       [300] seconds                       │   │     │
│   │  │ Connection Buffer:  [8192] bytes                        │   │     │
│   │  │                                                           │   │     │
│   │  │ Cache Settings:                                          │   │     │
│   │  │ [✓] Enable caching                                      │   │     │
│   │  │ Cache Size:    [512] MB                                 │   │     │
│   │  │ TTL:          [3600] seconds                           │   │     │
│   │  │ Strategy:     [⚫ LRU] [○ LFU] [○ FIFO]               │   │     │
│   │  │                                                           │   │     │
│   │  │ Thread Pool:                                             │   │     │
│   │  │ Worker Threads:     [8] (CPU cores: 8)                  │   │     │
│   │  │ Queue Size:         [1000]                              │   │     │
│   │  │ Priority Queues:    [✓] Enabled                        │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Database Optimization:                                          │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] Auto-vacuum      Interval: [Daily]                 │   │     │
│   │  │ [✓] Index optimization                                  │   │     │
│   │  │ [ ] Query logging    (Performance impact)              │   │     │
│   │  │ Connection Limit: [50]                                  │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ [B] Benchmark  [P] Profile  [O] Optimize  [R] Reset to Defaults           │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Navigate  [Space] Toggle  [S] Save  [B] Benchmark  [Esc] Back        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Display & Interface

UI customization and accessibility settings.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         DISPLAY & INTERFACE                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings                               Preview: [P] Show Preview │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Theme Settings:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Color Scheme:                                           │   │     │
│   │  │ [⚫ Dark] [○ Light] [○ Auto] [○ High Contrast]         │   │     │
│   │  │                                                           │   │     │
│   │  │ Color Customization:                                    │   │     │
│   │  │ Primary:     [#1E88E5] ████  Accent: [#FFC107] ████   │   │     │
│   │  │ Background:  [#121212] ████  Text:   [#FFFFFF] ████   │   │     │
│   │  │ Success:     [#4CAF50] ████  Error:  [#F44336] ████   │   │     │
│   │  │ Warning:     [#FF9800] ████  Info:   [#2196F3] ████   │   │     │
│   │  │                                                           │   │     │
│   │  │ [Load Preset: BitGo | Ledger | Custom]                  │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Typography:                                                     │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Font Family: [Fira Code] [Monaco] [Consolas]           │   │     │
│   │  │ Font Size:   [12] [14] [16] [18] px                    │   │     │
│   │  │ Line Height: [1.2] [1.5] [1.8] [2.0]                   │   │     │
│   │  │ [✓] Enable ligatures  [✓] Anti-aliasing               │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Layout Options:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Density:     [○ Compact] [⚫ Normal] [○ Comfortable]   │   │     │
│   │  │ Animations:  [✓] Enable  Speed: [Normal▼]              │   │     │
│   │  │ Sidebar:     [⚫ Left] [○ Right] [○ Hidden]           │   │     │
│   │  │ Status Bar:  [⚫ Bottom] [○ Top] [○ Hidden]           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Accessibility:                                                  │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] High contrast mode available                       │   │     │
│   │  │ [✓] Screen reader support                              │   │     │
│   │  │ [✓] Keyboard navigation hints                          │   │     │
│   │  │ [ ] Reduce motion                                      │   │     │
│   │  │ [ ] Large cursor                                       │   │     │
│   │  │ Focus indicator: [⚫ Default] [○ High visibility]      │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Current Theme: Dark Mode                    [P] Preview  [A] Apply Now     │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Navigate  [P] Preview  [S] Save  [R] Reset  [Esc] Back             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Compliance Settings

Regulatory compliance and audit configuration.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         COMPLIANCE SETTINGS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings                    Compliance Status: ✓ Fully Compliant │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Regulatory Frameworks:                                          │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] SOC 2 Type II      Status: ✓ Compliant             │   │     │
│   │  │     Last Audit: 2023-10-15                              │   │     │
│   │  │     Next Audit: 2024-04-15 (84 days)                   │   │     │
│   │  │     Auditor: Ernst & Young                              │   │     │
│   │  │                                                           │   │     │
│   │  │ [✓] ISO 27001          Status: ✓ Certified             │   │     │
│   │  │     Certificate: #ISO27001-2023-1847                    │   │     │
│   │  │     Expires: 2025-12-31                                 │   │     │
│   │  │                                                           │   │     │
│   │  │ [✓] GDPR               Status: ✓ Compliant             │   │     │
│   │  │     DPO: privacy@company.com                            │   │     │
│   │  │     Last Review: 2024-01-10                            │   │     │
│   │  │                                                           │   │     │
│   │  │ [ ] PCI DSS            Status: N/A                      │   │     │
│   │  │ [ ] HIPAA              Status: N/A                      │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Data Retention Policies:                                        │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Transaction Logs:    [7] years                          │   │     │
│   │  │ Access Logs:        [3] years                          │   │     │
│   │  │ Audit Trails:       [10] years                         │   │     │
│   │  │ User Data:          [As per GDPR]                      │   │     │
│   │  │ Backup Data:        [1] year                           │   │     │
│   │  │                                                           │   │     │
│   │  │ Deletion Policy:    [⚫ Automatic] [○ Manual]          │   │     │
│   │  │ Archive Location:   [s3://compliance-archive/____]      │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Audit Requirements:                                             │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] Enable comprehensive audit logging                  │   │     │
│   │  │ [✓] Immutable audit trail                              │   │     │
│   │  │ [✓] Real-time compliance monitoring                    │   │     │
│   │  │ [✓] Automated compliance reports                       │   │     │
│   │  │                                                           │   │     │
│   │  │ Report Schedule:                                        │   │     │
│   │  │ Daily:    [✓] Transaction summary                      │   │     │
│   │  │ Weekly:   [✓] Access report                            │   │     │
│   │  │ Monthly:  [✓] Compliance dashboard                     │   │     │
│   │  │ Quarterly:[✓] Full audit report                        │   │     │
│   │  │                                                           │   │     │
│   │  │ Recipients: [compliance@company.com__________]          │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ [G] Generate Report  [C] Check Compliance  [E] Export Settings            │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Navigate  [Space] Toggle  [S] Save  [G] Generate  [Esc] Back        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Profile Management

Manage different configuration profiles for various environments.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PROFILE MANAGEMENT                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Settings                          Active Profile: Production      │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Available Profiles:                                            │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ ⚫ Production                        Modified: 5d ago    │   │     │
│   │  │   High security, stable settings                        │   │     │
│   │  │   Wallets: 5  |  Users: 12  |  Uptime: 45d           │   │     │
│   │  │   [View] [Edit] [Export] [Clone]                       │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ ○ Development                       Modified: 2h ago    │   │     │
│   │  │   Debug enabled, relaxed security                       │   │     │
│   │  │   Wallets: 2  |  Users: 3   |  Last Used: 1d ago      │   │     │
│   │  │   [View] [Edit] [Export] [Delete]                      │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ ○ Testing                           Modified: 1w ago    │   │     │
│   │  │   Automated testing configuration                       │   │     │
│   │  │   Wallets: 0  |  Users: 1   |  Last Used: 3d ago      │   │     │
│   │  │   [View] [Edit] [Export] [Delete]                      │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ ○ Disaster Recovery                 Modified: 1m ago    │   │     │
│   │  │   Emergency fallback configuration                      │   │     │
│   │  │   Wallets: 5  |  Users: 12  |  Never Used             │   │     │
│   │  │   [View] [Edit] [Export] [Test]                        │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Profile Comparison:                                             │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Setting           Production    Development   Testing   │   │     │
│   │  │ ─────────────────────────────────────────────────────   │   │     │
│   │  │ Security Level    High          Low           Medium    │   │     │
│   │  │ 2FA Required      Yes           No            Yes       │   │     │
│   │  │ Log Level         INFO          DEBUG         WARN      │   │     │
│   │  │ Network           MainNet       TestNet       TestNet   │   │     │
│   │  │ Auto-backup       Daily         Never         Weekly    │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Quick Actions:                                                  │     │
│   │  [N] New Profile  [I] Import  [S] Switch  [C] Compare         │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Profile Lock: OFF           [L] Lock Current  [B] Backup All Profiles     │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Select  [Enter] Switch  [E] Edit  [N] New  [D] Delete  [Esc] Back   │
└─────────────────────────────────────────────────────────────────────────────┘
```