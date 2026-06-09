# Audit & Compliance Submenu Wireframes

This document contains detailed wireframes for all audit and compliance submenus in the MPC wallet TUI application.

## Table of Contents

1. [Audit & Compliance Main Menu](#audit--compliance-main-menu)
2. [Transaction History](#transaction-history)
3. [Access Logs](#access-logs)
4. [Generate Reports](#generate-reports)
5. [Risk Assessment](#risk-assessment)
6. [Export Audit Trail](#export-audit-trail)
7. [Compliance Dashboard](#compliance-dashboard)
8. [Security Events](#security-events)
9. [Incident Documentation](#incident-documentation)

---

## Audit & Compliance Main Menu

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

---

## Transaction History

### Main Transaction History View

```
┌─ Transaction History ────────────────────────────────────────────┐
│                                                                  │
│ Filters: [All Wallets ▼] [Last 30 days ▼] [All Types ▼]       │
│ Search: [____________________________] 🔍                       │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Date/Time          Type    Amount      To/From     Status  │  │
│ ├─────────────────────────────────────────────────────────────┤  │
│ │ 2025-01-12 14:32   Send    1.5 ETH    0x742d...   ✅      │  │
│ │ 14:32:15           Gas: 0.003 ETH     Block: 18976543      │  │
│ │                    Wallet: company_treasury (2/3)           │  │
│ │                                                             │  │
│ │ 2025-01-12 13:45   Receive 5.2 ETH    0x8B3D...   ✅      │  │
│ │ 13:45:22           From: External     Block: 18976234      │  │
│ │                    Wallet: company_treasury                 │  │
│ │                                                             │  │
│ │ 2025-01-12 12:10   Failed  0.5 ETH    0xA4B1...   ❌      │  │
│ │ 12:10:33           Error: Insufficient signatures (1/3)     │  │
│ │                    Wallet: project_alpha                    │  │
│ │                                                             │  │
│ │ 2025-01-12 11:22   Signing 10.0 SOL   7dHbW...    ⏳      │  │
│ │ 11:22:44           Signatures: 1/2     Time left: 3:27      │  │
│ │                    Wallet: personal_backup                  │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Showing 4 of 1,247 transactions         Page 1 of 312 [◀][▶]   │
│                                                                  │
│ [D] Details  [F] Filter  [E] Export  [S] Stats  [Esc] Back     │
└──────────────────────────────────────────────────────────────────┘
```

### Transaction Details View

```
┌─ Transaction Details ────────────────────────────────────────────┐
│                                                                  │
│ Transaction Information:                                         │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Transaction Hash: 0x3f4e5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f │  │
│ │ Status: ✅ Confirmed (321 confirmations)                    │  │
│ │ Type: Send ETH                                              │  │
│ │ Amount: 1.5 ETH ($3,750.00 USD)                           │  │
│ │ Gas Used: 21,000 units                                     │  │
│ │ Gas Price: 142.86 Gwei                                     │  │
│ │ Total Cost: 1.503 ETH                                      │  │
│ │                                                             │  │
│ │ From: 0x742d35Cc6634C0532925a3b844Bc9e7595f2bd (You)     │  │
│ │ To: 0x8B3D5C9A89F0E1D2C3B4A5968776543210FEDCBA            │  │
│ │                                                             │  │
│ │ Block Number: 18,976,543                                    │  │
│ │ Block Time: 2025-01-12 14:32:15 UTC                       │  │
│ │ Nonce: 42                                                   │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Signing Information:                                             │
│ • Wallet: company_treasury (2-of-3)                             │
│ • Signers: mpc-node-alice (1), mpc-node-bob (2)               │
│ • Initiated by: mpc-node-alice                                  │
│ • Signing duration: 2 minutes 34 seconds                        │
│                                                                  │
│ [V] View on explorer  [C] Copy hash  [P] Print  [Esc] Back     │
└──────────────────────────────────────────────────────────────────┘
```

### Transaction Statistics

```
┌─ Transaction Statistics ─────────────────────────────────────────┐
│                                                                  │
│ Period: Last 30 days              Wallets: All                  │
│                                                                  │
│ Overview:                                                        │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Total Transactions: 1,247                                   │  │
│ │ • Successful: 1,201 (96.3%)                                │  │
│ │ • Failed: 23 (1.8%)                                        │  │
│ │ • Pending: 23 (1.8%)                                       │  │
│ │                                                             │  │
│ │ Volume Statistics:                                          │  │
│ │ • Total Volume: 542.3 ETH ($1,355,750)                    │  │
│ │ • Average Transaction: 0.45 ETH                            │  │
│ │ • Largest Transaction: 25.0 ETH                            │  │
│ │ • Smallest Transaction: 0.001 ETH                          │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Transaction Volume (Daily):                                      │
│ 25 ETH ┤                                    ╱─────            │
│        │                               ╱───╯                    │
│ 15 ETH ┤                      ╱───────╯                        │
│        │                 ╱───╯                                  │
│  5 ETH ┤────────────────╯                                      │
│        └──────────────────────────────────────────────────►     │
│         Jan 1                  Jan 15                    Jan 30  │
│                                                                  │
│ [E] Export stats  [P] Print report  [C] Change period  [Esc]    │
└──────────────────────────────────────────────────────────────────┘
```

---

## Access Logs

### Access Log Viewer

```
┌─ Access Logs ────────────────────────────────────────────────────┐
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

### Advanced Filter Screen

```
┌─ Advanced Log Filter ────────────────────────────────────────────┐
│                                                                  │
│ Filter Criteria:                                                 │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Time Range:                                                 │  │
│ │ From: [2025-01-01 00:00] To: [2025-01-12 23:59]           │  │
│ │ ○ Last hour  ○ Last 24h  ● Last 7d  ○ Last 30d  ○ Custom │  │
│ │                                                             │  │
│ │ Event Types:                                                │  │
│ │ [✓] Authentication     [✓] Session Management              │  │
│ │ [✓] Wallet Operations  [✓] Network Events                  │  │
│ │ [ ] Debug Events       [✓] Security Alerts                 │  │
│ │                                                             │  │
│ │ Severity:                                                   │  │
│ │ [✓] Error  [✓] Warning  [✓] Info  [ ] Debug  [ ] Trace   │  │
│ │                                                             │  │
│ │ Users:                                                      │  │
│ │ [All users_______________▼]  or specific: [__________]     │  │
│ │                                                             │  │
│ │ Additional Filters:                                         │  │
│ │ IP Address: [_________________]                            │  │
│ │ Session ID: [_________________]                            │  │
│ │ Contains text: [______________]                            │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [A] Apply filter  [C] Clear all  [S] Save filter  [Esc] Cancel │
└──────────────────────────────────────────────────────────────────┘
```

---

## Generate Reports

### Report Generation Menu

```
┌─ Generate Reports ───────────────────────────────────────────────┐
│                                                                  │
│ Select Report Type:                                              │
│                                                                  │
│ Compliance Reports:                                              │
│ [1] 📊 SOC 2 Compliance Report                                  │
│ [2] 🔒 Security Audit Report                                    │
│ [3] 📋 Access Control Review                                    │
│ [4] 🏦 Financial Compliance Summary                             │
│                                                                  │
│ Activity Reports:                                                │
│ [5] 💰 Transaction Summary Report                               │
│ [6] 👥 User Activity Report                                     │
│ [7] 🔑 Key Management Report                                    │
│ [8] 🌐 Network Operations Report                                │
│                                                                  │
│ Custom Reports:                                                  │
│ [9] 📝 Custom Report Builder                                    │
│ [A] 📅 Scheduled Reports                                        │
│                                                                  │
│ Recent Reports:                                                  │
│ • SOC2_Compliance_2025Q1.pdf (Generated: 2025-01-10)           │
│ • Monthly_Activity_Jan2025.csv (Generated: 2025-01-01)         │
│                                                                  │
│ [Enter] Generate  [V] View recent  [S] Schedule  [Esc] Back    │
└──────────────────────────────────────────────────────────────────┘
```

### Report Configuration

```
┌─ Configure Report: SOC 2 Compliance ─────────────────────────────┐
│                                                                  │
│ Report Parameters:                                               │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Report Period:                                              │  │
│ │ From: [2025-01-01] To: [2025-01-31]                       │  │
│ │ ○ Current month  ● Custom period  ○ Last quarter          │  │
│ │                                                             │  │
│ │ Include Sections:                                           │  │
│ │ [✓] Executive Summary                                      │  │
│ │ [✓] Access Control Assessment                              │  │
│ │ [✓] System Operations Review                               │  │
│ │ [✓] Change Management Log                                  │  │
│ │ [✓] Risk Assessment Matrix                                 │  │
│ │ [✓] Incident Response Summary                              │  │
│ │ [ ] Detailed Transaction Logs                              │  │
│ │                                                             │  │
│ │ Output Format:                                              │  │
│ │ ● PDF  ○ CSV  ○ JSON  ○ HTML                             │  │
│ │                                                             │  │
│ │ Recipients:                                                 │  │
│ │ Email to: [compliance@company.com_____________]            │  │
│ │ [✓] Include in audit archive                              │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Estimated generation time: 2-3 minutes                          │
│                                                                  │
│ [G] Generate report  [P] Preview  [S] Save template  [Esc] Back │
└──────────────────────────────────────────────────────────────────┘
```

---

## Risk Assessment

### Risk Assessment Dashboard

```
┌─ Risk Assessment Dashboard ──────────────────────────────────────┐
│                                                                  │
│ Overall Risk Score: 72/100 (Medium)          Trend: ↓ Improving │
│                                                                  │
│ Risk Categories:                                                 │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Security Risks:                          Score: 65/100     │  │
│ │ ████████████████████░░░░░░░░░░░░░░░░░░                   │  │
│ │ • 2 critical findings                                      │  │
│ │ • 5 medium findings                                        │  │
│ │ • 12 low findings                                          │  │
│ │                                                             │  │
│ │ Operational Risks:                       Score: 78/100     │  │
│ │ ███████████████████████░░░░░░░░░░░░░░                    │  │
│ │ • Key person dependency (High)                             │  │
│ │ • Backup frequency (Medium)                                │  │
│ │                                                             │  │
│ │ Compliance Risks:                        Score: 85/100     │  │
│ │ █████████████████████████████░░░░░░░░                    │  │
│ │ • All frameworks compliant                                 │  │
│ │ • Next audit in 89 days                                   │  │
│ │                                                             │  │
│ │ Financial Risks:                         Score: 92/100     │  │
│ │ ████████████████████████████████████░░                   │  │
│ │ • Transaction limits enforced                              │  │
│ │ • Multi-sig properly configured                            │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [D] Detailed analysis  [M] Mitigation plan  [E] Export  [Esc]  │
└──────────────────────────────────────────────────────────────────┘
```

### Risk Mitigation Plan

```
┌─ Risk Mitigation Plan ───────────────────────────────────────────┐
│                                                                  │
│ Critical Risks Requiring Immediate Action:                      │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ 1. Weak Authentication on Admin Accounts                    │  │
│ │    Risk Level: 🔴 Critical                                  │  │
│ │    Impact: Unauthorized access to system settings           │  │
│ │    Mitigation:                                              │  │
│ │    • [ ] Enable 2FA for all admin accounts                 │  │
│ │    • [ ] Implement IP whitelisting                         │  │
│ │    • [ ] Review and revoke unused access                   │  │
│ │    Timeline: Immediate (within 24 hours)                   │  │
│ │                                                             │  │
│ │ 2. Outdated Key Rotation Policy                             │  │
│ │    Risk Level: 🔴 Critical                                  │  │
│ │    Impact: Compromised keys remain valid too long          │  │
│ │    Mitigation:                                              │  │
│ │    • [ ] Implement 90-day rotation policy                  │  │
│ │    • [ ] Schedule automated rotation reminders             │  │
│ │    • [ ] Document rotation procedures                      │  │
│ │    Timeline: Within 7 days                                 │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Progress: 0/2 critical items addressed                          │
│                                                                  │
│ [S] Start mitigation  [P] Print plan  [A] Assign  [Esc] Back   │
└──────────────────────────────────────────────────────────────────┘
```

---

## Export Audit Trail

### Export Configuration

```
┌─ Export Audit Trail ─────────────────────────────────────────────┐
│                                                                  │
│ Configure Audit Export:                                          │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Export Scope:                                               │  │
│ │ Date Range: [2025-01-01] to [2025-01-12]                  │  │
│ │ ○ All data  ● Date range  ○ Last audit period            │  │
│ │                                                             │  │
│ │ Data Types to Include:                                     │  │
│ │ [✓] Transaction logs      (45.2 MB)                       │  │
│ │ [✓] Access logs          (12.3 MB)                       │  │
│ │ [✓] Configuration changes (2.1 MB)                        │  │
│ │ [✓] Security events      (5.7 MB)                        │  │
│ │ [✓] User activities      (8.9 MB)                        │  │
│ │ [ ] Debug logs           (156.2 MB)                       │  │
│ │                                                             │  │
│ │ Export Format:                                              │  │
│ │ ● Structured JSON  ○ CSV files  ○ XML  ○ SQLite DB       │  │
│ │                                                             │  │
│ │ Compression:                                                │  │
│ │ ● ZIP archive  ○ TAR.GZ  ○ No compression                │  │
│ │                                                             │  │
│ │ Security:                                                   │  │
│ │ [✓] Encrypt export with password                          │  │
│ │ [✓] Include integrity checksums                           │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Total size: 74.2 MB (23.4 MB compressed)                        │
│                                                                  │
│ [E] Export  [T] Test export  [S] Save config  [Esc] Cancel     │
└──────────────────────────────────────────────────────────────────┘
```

---

## Compliance Dashboard

### Main Compliance View

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

### Control Details View

```
┌─ Control Details: SOC 2 - Access Controls ──────────────────────┐
│                                                                  │
│ Control Overview:                                                │
│ Status: ✅ Fully Compliant                                      │
│ Last Assessment: 2025-01-10                                      │
│ Next Review: 2025-04-10                                          │
│                                                                  │
│ Control Requirements:                                            │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ AC-1: Access Control Policy                                 │  │
│ │ Status: ✅ Implemented                                      │  │
│ │ Evidence: Policy document v2.3, approved 2024-12-01        │  │
│ │                                                             │  │
│ │ AC-2: Account Management                                    │  │
│ │ Status: ✅ Implemented                                      │  │
│ │ Evidence: User provisioning logs, quarterly reviews        │  │
│ │                                                             │  │
│ │ AC-3: Access Enforcement                                    │  │
│ │ Status: ✅ Implemented                                      │  │
│ │ Evidence: RBAC configuration, access matrix                │  │
│ │                                                             │  │
│ │ AC-4: Information Flow Enforcement                          │  │
│ │ Status: ✅ Implemented                                      │  │
│ │ Evidence: Network segmentation, firewall rules             │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [V] View evidence  [T] Run test  [U] Update status  [Esc] Back │
└──────────────────────────────────────────────────────────────────┘
```

---

## Security Events

### Security Event Monitor

```
┌─ Security Events Monitor ────────────────────────────────────────┐
│                                                                  │
│ Real-time Security Monitoring         Status: 🟢 Active         │
│                                                                  │
│ Recent Security Events:                                          │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ 🔴 14:45:23  CRITICAL  Multiple failed auth attempts       │  │
│ │    Source: 45.32.164.22  Target: admin@company.com         │  │
│ │    Action: IP blocked, admin notified                      │  │
│ │                                                             │  │
│ │ 🟡 14:32:10  WARNING   Unusual transaction pattern         │  │
│ │    Wallet: project_alpha  Pattern: Rapid small transfers   │  │
│ │    Action: Additional verification required                │  │
│ │                                                             │  │
│ │ 🟡 13:21:45  WARNING   Session timeout exceeded            │  │
│ │    User: mpc-node-carol  Duration: 25 hours               │  │
│ │    Action: Session terminated                              │  │
│ │                                                             │  │
│ │ 🟢 12:15:33  INFO      Security scan completed            │  │
│ │    Result: No vulnerabilities found                        │  │
│ │    Next scan: 2025-01-13 12:00:00                         │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Statistics (Last 24h):                                           │
│ • Critical: 1  • High: 0  • Medium: 3  • Low: 12               │
│                                                                  │
│ [R] Refresh  [F] Filter  [A] Acknowledge  [I] Investigate      │
│ [E] Export events  [Esc] Back                                    │
└──────────────────────────────────────────────────────────────────┘
```

### Security Investigation

```
┌─ Security Investigation: Auth Attack ────────────────────────────┐
│                                                                  │
│ Incident ID: SEC-2025-0142                                       │
│ Severity: 🔴 Critical                                            │
│ Status: Under Investigation                                      │
│                                                                  │
│ Attack Timeline:                                                 │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ 14:40:15 - First failed attempt from 45.32.164.22         │  │
│ │ 14:40:32 - Second attempt (different password)             │  │
│ │ 14:40:48 - Third attempt (different password)              │  │
│ │ 14:41:05 - Fourth attempt (pattern suggests brute force)   │  │
│ │ 14:41:22 - Fifth attempt                                   │  │
│ │ 14:41:23 - Automatic IP block triggered                    │  │
│ │ 14:45:23 - Alert generated and sent                        │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ Attack Analysis:                                                 │
│ • Source IP: 45.32.164.22 (VPS provider, suspicious)           │
│ • Target: admin@company.com                                      │
│ • Method: Password brute force                                   │
│ • Passwords tried: Common patterns detected                      │
│                                                                  │
│ Recommended Actions:                                             │
│ [ ] Reset admin password                                         │
│ [ ] Enable 2FA if not already active                           │
│ [ ] Review all recent admin activities                          │
│ [ ] Check for other accounts targeted                          │
│                                                                  │
│ [M] Mark resolved  [E] Escalate  [R] Generate report  [Esc]    │
└──────────────────────────────────────────────────────────────────┘
```

---

## Incident Documentation

### Incident Report Form

```
┌─ Document Security Incident ─────────────────────────────────────┐
│                                                                  │
│ Create Incident Report:                                          │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Incident Details:                                           │  │
│ │                                                             │  │
│ │ Title: [Unauthorized access attempt on admin account_____] │  │
│ │                                                             │  │
│ │ Severity: ● Critical  ○ High  ○ Medium  ○ Low             │  │
│ │                                                             │  │
│ │ Date/Time: [2025-01-12 14:45:23] (auto-filled)           │  │
│ │                                                             │  │
│ │ Category:                                                   │  │
│ │ ● Authentication  ○ Data Breach  ○ Malware  ○ Other       │  │
│ │                                                             │  │
│ │ Description:                                                │  │
│ │ [Multiple failed authentication attempts detected from     │  │
│ │  IP 45.32.164.22 targeting admin@company.com account.     │  │
│ │  Pattern suggests automated brute force attack.           │  │
│ │  IP automatically blocked after 5 attempts._____________] │  │
│ │                                                             │  │
│ │ Impact Assessment:                                          │  │
│ │ [✓] No successful breach                                  │  │
│ │ [ ] Data potentially compromised                          │  │
│ │ [ ] Service disruption                                    │  │
│ │ [✓] Security controls worked as designed                  │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [S] Save draft  [N] Next: Response actions  [Esc] Cancel       │
└──────────────────────────────────────────────────────────────────┘
```

### Incident Response Actions

```
┌─ Incident Response Actions ──────────────────────────────────────┐
│                                                                  │
│ Document Response Actions:                                       │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Immediate Actions Taken:                                    │  │
│ │ [✓] IP address blocked (14:41:23)                         │  │
│ │ [✓] Admin notified via email (14:45:23)                   │  │
│ │ [✓] Security team alerted (14:45:30)                      │  │
│ │ [ ] Password reset enforced                                │  │
│ │ [ ] Additional monitoring enabled                          │  │
│ │                                                             │  │
│ │ Investigation Findings:                                     │  │
│ │ [Source IP traced to known VPS provider commonly used     │  │
│ │  for attacks. No other accounts targeted. Attack stopped  │  │
│ │  by automatic security controls._______________________] │  │
│ │                                                             │  │
│ │ Follow-up Actions Required:                                │  │
│ │ [ ] Review and update security policies                    │  │
│ │ [✓] Implement 2FA for all admin accounts                  │  │
│ │ [ ] Conduct security training                              │  │
│ │ [ ] Update incident response procedures                    │  │
│ │                                                             │  │
│ │ Lessons Learned:                                           │  │
│ │ [Security controls effectively prevented breach. Consider  │  │
│ │  implementing rate limiting at application level._______] │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [F] Finalize report  [P] Print  [A] Attach evidence  [Esc]     │
└──────────────────────────────────────────────────────────────────┘
```

This comprehensive audit and compliance submenu wireframe document provides detailed layouts for all audit, compliance, and security monitoring functions, maintaining the professional enterprise-grade interface while ensuring thorough tracking and reporting capabilities.