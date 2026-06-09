# Emergency Response Path Wireframes

This document contains comprehensive wireframes for emergency response scenarios in the MPC wallet TUI, designed for critical security incidents and business continuity.

## Table of Contents

1. [Emergency Dashboard](#emergency-dashboard)
2. [Threat Detection Screen](#threat-detection-screen)
3. [Wallet Lockdown](#wallet-lockdown)
4. [Alert System](#alert-system)
5. [Forensic Analysis](#forensic-analysis)
6. [Recovery Procedures](#recovery-procedures)
7. [Incident Report](#incident-report)

---

## Emergency Dashboard

Real-time monitoring and threat assessment dashboard.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    🚨 EMERGENCY RESPONSE DASHBOARD 🚨                       │
├─────────────────────────────────────────────────────────────────────────────┤
│ Status: ⚠ ELEVATED THREAT              Last Check: 14:55:23 (5 sec ago)   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Active Threats & Anomalies:                                    │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ 🔴 CRITICAL | Unauthorized Access Attempt                │   │     │
│   │  │    Time: 14:52:15 | Source: 185.220.101.45 (TOR)       │   │     │
│   │  │    Target: company_treasury wallet                       │   │     │
│   │  │    Status: BLOCKED | Action: Investigating              │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ 🟡 WARNING | Unusual Transaction Pattern                 │   │     │
│   │  │    Time: 14:48:00 | Wallet: defi_operations            │   │     │
│   │  │    Details: 5 rapid transactions in 2 minutes           │   │     │
│   │  │    Status: MONITORING | Risk Score: 7/10               │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ 🟡 WARNING | Participant Disconnection                   │   │     │
│   │  │    Time: 14:45:00 | Node: mpc-node-003                 │   │     │
│   │  │    Duration: 10 minutes | Last Location: NYC Office    │   │     │
│   │  │    Status: UNREACHABLE | Attempts: 12                   │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  System Health:          Quick Actions:                         │     │
│   │  ┌──────────────────┐   ┌─────────────────────────────────┐  │     │
│   │  │ Wallets:    3/5 ●│   │ [1] Lock All Wallets           │  │     │
│   │  │ Network:    ████ │   │ [2] Emergency Alert            │  │     │
│   │  │ Security:   ███░ │   │ [3] Start Forensics            │  │     │
│   │  │ Backup:     ████ │   │ [4] Initiate Recovery          │  │     │
│   │  └──────────────────┘   │ [5] Contact Security Team       │  │     │
│   │                          └─────────────────────────────────┘  │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Active Incidents: 3        Response Team: On Call       Mode: Auto-Defense │
├─────────────────────────────────────────────────────────────────────────────┤
│ [R] Refresh  [1-5] Quick Action  [D] Details  [E] Export  [Esc] Exit       │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Threat Detection Screen

Detailed threat analysis and response options.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         THREAT ANALYSIS                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ Incident ID: INC-2024-0125-001          Severity: CRITICAL                │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Threat Details:                                                 │     │
│   │                                                                   │     │
│   │  Attack Vector: Brute Force Authentication                      │     │
│   │  Target: Administrative Access                                   │     │
│   │  Duration: 12 minutes (14:40:00 - 14:52:00)                    │     │
│   │  Attempts: 347                                                   │     │
│   │                                                                   │     │
│   │  Attack Timeline:                                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ 14:40 ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │   │     │
│   │  │ 14:42 ▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │   │     │
│   │  │ 14:44 ▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │   │     │
│   │  │ 14:46 ▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │   │     │
│   │  │ 14:48 ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░   │   │     │
│   │  │ 14:50 ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░   │   │     │
│   │  │ 14:52 ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ BLOCKED│   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Source Analysis:                                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ IP Address:     185.220.101.45                          │   │     │
│   │  │ Location:       TOR Exit Node (Netherlands)             │   │     │
│   │  │ ISP:            Anonymous VPN Service                   │   │     │
│   │  │ Reputation:     MALICIOUS (Score: 95/100)              │   │     │
│   │  │ Previous:       17 incidents reported globally          │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Recommended Actions:                                            │     │
│   │  > [1] Immediate wallet lockdown (all wallets)                  │     │
│   │    [2] Force password reset for all users                       │     │
│   │    [3] Enable enhanced authentication (hardware keys)           │     │
│   │    [4] Initiate full security audit                            │     │
│   │    [5] Report to authorities                                    │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Auto-response: ENABLED              Manual override available              │
├─────────────────────────────────────────────────────────────────────────────┤
│ [1-5] Execute Action  [A] Auto-respond  [M] Manual  [E] Escalate          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Wallet Lockdown

Emergency wallet lockdown interface.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    🔒 EMERGENCY WALLET LOCKDOWN 🔒                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ Authorization Level: CRITICAL            Initiated By: Security System      │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Lockdown Configuration:                                         │     │
│   │                                                                   │     │
│   │  Scope:                                                          │     │
│   │  [⚫] All Wallets - Complete lockdown                           │     │
│   │  [○] Selected Wallets - Choose specific wallets                 │     │
│   │  [○] Transaction Limit - Allow small transactions only          │     │
│   │                                                                   │     │
│   │  Affected Wallets:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] company_treasury     $31,250     → LOCKING...       │   │     │
│   │  │ [✓] treasury_cold        $1,125,500  → LOCKING...       │   │     │
│   │  │ [✓] defi_operations      $85,420     → LOCKING...       │   │     │
│   │  │ [✓] solana_ops           $2,580      → LOCKED ✓         │   │     │
│   │  │ [✓] btc_reserves         $367,500    → LOCKED ✓         │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Lockdown Progress:                                             │     │
│   │  [██████████████████████████░░░░░░░░░░░] 70% Complete        │     │
│   │                                                                   │     │
│   │  Duration Settings:                                             │     │
│   │  [○] 1 Hour    [○] 24 Hours    [⚫] Until Manual Release      │     │
│   │                                                                   │     │
│   │  Additional Security Measures:                                  │     │
│   │  [✓] Disable all API access                                    │     │
│   │  [✓] Revoke all active sessions                               │     │
│   │  [✓] Enable transaction monitoring                            │     │
│   │  [✓] Alert all stakeholders                                   │     │
│   │  [✓] Create encrypted backup                                  │     │
│   │                                                                   │     │
│   │  Override Code Required: [________________]                    │     │
│   │  (Contact CTO or Security Lead for override code)             │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Total Value Protected: $1,612,250         Time Elapsed: 00:01:23          │
├─────────────────────────────────────────────────────────────────────────────┤
│ [C] Confirm Lockdown  [A] Abort  [O] Override  [?] Help                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Alert System

Emergency notification and alert management.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         EMERGENCY ALERT SYSTEM                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ Alert Level: 🔴 CRITICAL                   Protocol: DEFCON-2              │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Alert Configuration:                                            │     │
│   │                                                                   │     │
│   │  Message Template:                                               │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Subject: [CRITICAL] Security Incident - Immediate Action │   │     │
│   │  │                                                           │   │     │
│   │  │ A critical security incident has been detected:          │   │     │
│   │  │ - Type: Unauthorized Access Attempt                      │   │     │
│   │  │ - Time: 2024-01-25 14:52:15 UTC                        │   │     │
│   │  │ - Severity: CRITICAL                                     │   │     │
│   │  │ - Action Taken: All wallets locked                      │   │     │
│   │  │                                                           │   │     │
│   │  │ Required Response: Acknowledge within 15 minutes         │   │     │
│   │  │ Dashboard: https://starlab-mpc.company.com/emergency      │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Recipient Groups:                                               │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] Executive Team (3)         Status: ⟳ Sending...     │   │     │
│   │  │     • ceo@company.com         ✓ Delivered              │   │     │
│   │  │     • cto@company.com         ⟳ Sending...            │   │     │
│   │  │     • ciso@company.com        ⟳ Sending...            │   │     │
│   │  │                                                           │   │     │
│   │  │ [✓] Security Team (5)         Status: ✓ Sent           │   │     │
│   │  │     All members notified via SMS + Email               │   │     │
│   │  │                                                           │   │     │
│   │  │ [✓] Operations (8)            Status: ⟳ In Progress    │   │     │
│   │  │     6/8 acknowledged                                    │   │     │
│   │  │                                                           │   │     │
│   │  │ [✓] External Auditor          Status: ◐ Queued        │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Communication Channels:                                         │     │
│   │  [✓] Email (Primary)    [✓] SMS    [✓] Slack    [✓] Phone     │     │
│   │                                                                   │     │
│   │  Escalation Timeline:                                            │     │
│   │  T+0min: Initial alert → T+15min: Follow-up → T+30min: Call   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Sent: 16/19        Acknowledged: 9/19        Failed: 0                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ [S] Send All  [R] Resend Failed  [U] Update Message  [C] Call Tree         │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Forensic Analysis

Deep forensic investigation interface.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         FORENSIC ANALYSIS MODE                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ Case ID: FOR-2024-0125-001              Analyst: security-admin            │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Evidence Collection:                                            │     │
│   │                                                                   │     │
│   │  System Logs:                    Blockchain Analysis:            │     │
│   │  ┌─────────────────────┐        ┌──────────────────────┐       │     │
│   │  │ Access Logs   45 MB │        │ TX History    12 MB  │       │     │
│   │  │ Network Logs  127 MB│        │ Smart Contr.  3 MB   │       │     │
│   │  │ Auth Logs     23 MB │        │ Gas Analysis  1 MB   │       │     │
│   │  │ Error Logs    8 MB  │        │ Address Book  0.5 MB │       │     │
│   │  └─────────────────────┘        └──────────────────────┘       │     │
│   │                                                                   │     │
│   │  Timeline Reconstruction:                                         │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ 14:35:00  Normal wallet operation                        │   │     │
│   │  │ 14:40:12  First suspicious connection from TOR          │   │     │
│   │  │ 14:42:30  Authentication attempts begin                  │   │     │
│   │  │ 14:45:00  mpc-node-003 loses connection                 │   │     │
│   │  │ 14:48:00  Unusual transaction pattern detected           │   │     │
│   │  │ 14:52:15  Brute force attack reaches critical threshold │   │     │
│   │  │ 14:52:16  Automatic lockdown initiated                   │   │     │
│   │  │ 14:52:45  All wallets secured                           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Network Trace:                                                  │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Source         → Gateway      → Target       │ Proto   │   │     │
│   │  │ 185.220.101.45 → 10.0.0.1    → 10.0.0.100  │ HTTPS   │   │     │
│   │  │ Packets: 3,847  Data: 45.2MB  Duration: 12min         │   │     │
│   │  │                                                         │   │     │
│   │  │ [View Packet Details] [Export PCAP] [Analyze Pattern] │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Indicators of Compromise (IoCs):                               │     │
│   │  • IP: 185.220.101.45 (Known malicious)                       │     │
│   │  • Pattern: Sequential password attempts                       │     │
│   │  • Timing: Automated tool signature detected                   │     │
│   │  • Target: Admin accounts exclusively                          │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Evidence: 211 MB collected         Analysis: 67% complete                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ [E] Export Evidence  [G] Generate Report  [S] Share Findings  [C] Continue │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Recovery Procedures

Post-incident recovery workflow.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         INCIDENT RECOVERY                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ Incident: INC-2024-0125-001            Status: Recovery In Progress        │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Recovery Checklist:                                            │     │
│   │                                                                   │     │
│   │  Security Verification:                                          │     │
│   │  [✓] Threat neutralized and contained                          │     │
│   │  [✓] All attack vectors identified                             │     │
│   │  [✓] Security patches applied                                  │     │
│   │  [⟳] Penetration test in progress...                           │     │
│   │  [ ] External security audit scheduled                          │     │
│   │                                                                   │     │
│   │  System Restoration:                                             │     │
│   │  [✓] Clean system backup verified                              │     │
│   │  [✓] Wallet integrity confirmed                                │     │
│   │  [⟳] Restoring normal operations...                            │     │
│   │  [ ] Performance benchmarks pending                            │     │
│   │                                                                   │     │
│   │  Access Control:                                                 │     │
│   │  [✓] All passwords reset                                       │     │
│   │  [✓] 2FA enforcement updated                                   │     │
│   │  [✓] Access logs reviewed                                      │     │
│   │  [⟳] New security policies being deployed...                   │     │
│   │                                                                   │     │
│   │  Wallet Status:                                                  │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Wallet              Status         Next Action          │   │     │
│   │  │ company_treasury    🟡 Restricted  Verify transactions  │   │     │
│   │  │ treasury_cold       🟢 Unlocked    Normal operations    │   │     │
│   │  │ defi_operations     🟡 Restricted  Pending approval     │   │     │
│   │  │ solana_ops          🟢 Unlocked    Normal operations    │   │     │
│   │  │ btc_reserves        🔴 Locked      Awaiting audit       │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Recovery Timeline:                                              │     │
│   │  Phase 1 (Complete): Immediate response and containment         │     │
│   │  Phase 2 (Current):  System verification and restoration        │     │
│   │  Phase 3 (Next):     Gradual service restoration               │     │
│   │  Phase 4 (Planned):  Full operational capacity                  │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Progress: 65%              Estimated Full Recovery: 2 hours               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [U] Unlock Wallet  [V] Verify  [T] Test Transaction  [R] Report Status    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Incident Report

Comprehensive incident report generation.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         INCIDENT REPORT                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ Report ID: RPT-2024-0125-001           Generated: 2024-01-25 16:30:00     │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Executive Summary:                                              │     │
│   │  ────────────────────────────────────────────────────────────   │     │
│   │  On January 25, 2024, at 14:52 UTC, our MPC wallet system      │     │
│   │  detected and successfully defended against a sophisticated     │     │
│   │  brute-force attack targeting administrative access.           │     │
│   │                                                                   │     │
│   │  Key Metrics:                                                    │     │
│   │  • Attack Duration: 12 minutes                                  │     │
│   │  • Funds at Risk: $1,612,250                                   │     │
│   │  • Funds Lost: $0                                              │     │
│   │  • System Downtime: 45 minutes                                 │     │
│   │  • Recovery Time: 2 hours                                      │     │
│   │                                                                   │     │
│   │  Technical Details:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Attack Vector:    Brute Force via TOR Network          │   │     │
│   │  │ Target:           Admin Authentication Endpoint         │   │     │
│   │  │ Attempts:         347 login attempts                   │   │     │
│   │  │ Source IPs:       185.220.101.45 (primary)            │   │     │
│   │  │ Tools Used:       Automated password cracking tool     │   │     │
│   │  │ Vulnerabilities:  None exploited (attack failed)       │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Response Timeline:                                              │     │
│   │  14:40 - First suspicious activity detected                     │     │
│   │  14:52 - Automatic lockdown triggered                           │     │
│   │  14:55 - Security team notified                                 │     │
│   │  15:10 - Forensic analysis initiated                            │     │
│   │  16:30 - System recovery completed                              │     │
│   │                                                                   │     │
│   │  Recommendations:                                                │     │
│   │  1. Implement hardware security keys for admin access           │     │
│   │  2. Enhance rate limiting on authentication endpoints           │     │
│   │  3. Deploy additional network monitoring tools                  │     │
│   │  4. Conduct quarterly security drills                           │     │
│   │                                                                   │     │
│   │  [Preview Full Report] [Export PDF] [Send to Board]             │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Report Status: DRAFT               Distribution: Restricted                │
├─────────────────────────────────────────────────────────────────────────────┤
│ [E] Edit  [F] Finalize  [S] Sign Report  [D] Distribute  [X] Export       │
└─────────────────────────────────────────────────────────────────────────────┘
```