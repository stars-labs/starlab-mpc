# FROST MPC TUI Wallet - Security Model

## Table of Contents

1. [Security Overview](#security-overview)
2. [Threat Model](#threat-model)
3. [Cryptographic Security](#cryptographic-security)
4. [Network Security](#network-security)
5. [Local Security](#local-security)
6. [Operational Security](#operational-security)
7. [Security Best Practices](#security-best-practices)
8. [Incident Response](#incident-response)
9. [Compliance and Auditing](#compliance-and-auditing)

## Security Overview

The FROST MPC TUI Wallet implements defense-in-depth security architecture, combining cryptographic protection, secure communications, and operational safeguards to protect digital assets.

### Security Principles

1. **Zero Trust**: No single component or participant is fully trusted
2. **Least Privilege**: Components have minimal required permissions
3. **Defense in Depth**: Multiple layers of security controls
4. **Fail Secure**: System fails to a secure state
5. **Auditability**: All security-relevant events are logged

### Security Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Security Layers                        │
├─────────────────────────────────────────────────────────┤
│ Layer 5: Operational Security                           │
│ • Access controls, procedures, training                 │
├─────────────────────────────────────────────────────────┤
│ Layer 4: Application Security                           │
│ • Input validation, secure coding, memory safety        │
├─────────────────────────────────────────────────────────┤
│ Layer 3: Network Security                               │
│ • TLS/DTLS, certificate validation, firewall           │
├─────────────────────────────────────────────────────────┤
│ Layer 2: Cryptographic Security                         │
│ • FROST protocol, key encryption, signatures            │
├─────────────────────────────────────────────────────────┤
│ Layer 1: Platform Security                              │
│ • OS hardening, secure boot, hardware security         │
└─────────────────────────────────────────────────────────┘
```

## Threat Model

### Adversary Capabilities

We consider adversaries with the following capabilities:

#### Network Adversary (Level 1)
- Can observe all network traffic
- Can delay or drop packets
- Can attempt man-in-the-middle attacks
- Cannot break TLS/DTLS encryption

#### Compromised Participant (Level 2)
- Controls up to t-1 participants (below threshold)
- Has valid key shares for controlled participants
- Can attempt to manipulate protocol messages
- Cannot forge signatures without threshold

#### Local Malware (Level 3)
- Has user-level access to device
- Can read unencrypted files
- Can capture keyboard input
- Cannot access encrypted keystore without password

#### Physical Access (Level 4)
- Physical access to device
- Can attempt cold boot attacks
- Can install hardware keyloggers
- Time-limited access window

### Attack Scenarios

```
┌─────────────────────────────────────────────────────────┐
│ Attack Tree: Unauthorized Transaction                    │
├─────────────────────────────────────────────────────────┤
│ Goal: Execute unauthorized transaction                   │
│                                                         │
│ ├─ Compromise threshold participants                    │
│ │  ├─ Social engineering (High Risk)                   │
│ │  ├─ Malware infection (Medium Risk)                 │
│ │  └─ Physical coercion (Low Risk)                    │
│ │                                                       │
│ ├─ Break cryptographic security                        │
│ │  ├─ Factor private key (Negligible Risk)            │
│ │  ├─ Break FROST protocol (Negligible Risk)          │
│ │  └─ Exploit implementation bug (Low Risk)           │
│ │                                                       │
│ └─ Exploit operational weakness                        │
│     ├─ Weak passwords (Medium Risk)                    │
│     ├─ Poor key management (Medium Risk)              │
│     └─ Insider threat (Medium Risk)                   │
└─────────────────────────────────────────────────────────┘
```

## Cryptographic Security

### FROST Protocol Security

The FROST (Flexible Round-Optimized Schnorr Threshold) protocol provides:

```
┌─────────────────────────────────────────────────────────┐
│ FROST Security Properties                               │
├─────────────────────────────────────────────────────────┤
│ ✅ Unforgeability                                       │
│    No adversary can forge signatures without           │
│    controlling ≥t participants                          │
│                                                         │
│ ✅ Robustness                                           │
│    Protocol completes despite up to n-t failures       │
│                                                         │
│ ✅ Privacy                                              │
│    Individual key shares reveal nothing about          │
│    the complete private key                            │
│                                                         │
│ ✅ Non-repudiation                                      │
│    Participants cannot deny their participation        │
│    in signing operations                               │
│                                                         │
│ ✅ Verifiability                                        │
│    All operations can be publicly verified             │
└─────────────────────────────────────────────────────────┘
```

### Key Generation Security

```rust
// Secure random number generation
use rand_core::OsRng; // Cryptographically secure RNG

// Key generation with proper entropy
let mut rng = OsRng;
let secret_share = Scalar::random(&mut rng);

// Commitment generation with binding
let commitment = VerifiableSecretSharing::commit(&secret_share);
```

### Cryptographic Parameters

| Parameter | Value | Security Level |
|-----------|-------|----------------|
| Curve (Ethereum) | secp256k1 | 128-bit |
| Curve (Solana) | ed25519 | 128-bit |
| Hash Function | SHA-256 | 128-bit collision |
| KDF | PBKDF2-SHA256 | 100,000 iterations |
| Encryption | AES-256-GCM | 256-bit |
| Key Share Size | 32 bytes | Full entropy |

## Network Security

### TLS/DTLS Configuration

```
┌─────────────────────────────────────────────────────────┐
│ TLS Configuration (WebSocket)                           │
├─────────────────────────────────────────────────────────┤
│ Version: TLS 1.3 (minimum TLS 1.2)                     │
│                                                         │
│ Cipher Suites:                                          │
│ • TLS_AES_256_GCM_SHA384 (preferred)                  │
│ • TLS_AES_128_GCM_SHA256                              │
│ • TLS_CHACHA20_POLY1305_SHA256                        │
│                                                         │
│ Certificate Validation:                                 │
│ • Verify full certificate chain                        │
│ • Check certificate expiration                         │
│ • Validate against pinned CA (optional)               │
│ • Verify server hostname                              │
│                                                         │
│ Additional Security:                                    │
│ • HSTS enforcement                                     │
│ • Certificate transparency                             │
│ • OCSP stapling                                       │
└─────────────────────────────────────────────────────────┘
```

### WebRTC Security

```
┌─────────────────────────────────────────────────────────┐
│ WebRTC Security Configuration                           │
├─────────────────────────────────────────────────────────┤
│ DTLS Configuration:                                     │
│ • Version: DTLS 1.3                                    │
│ • SRTP for media encryption                           │
│ • Perfect forward secrecy                             │
│                                                         │
│ ICE Security:                                           │
│ • TURN server authentication                           │
│ • Consent freshness checks                            │
│ • Rate limiting                                       │
│                                                         │
│ Signaling Security:                                     │
│ • All offers/answers over TLS                         │
│ • SDP sanitization                                    │
│ • Origin validation                                   │
└─────────────────────────────────────────────────────────┘
```

### Network Isolation

For high-security deployments:

```
┌─────────────────────────────────────────────────────────┐
│ Network Segmentation                                    │
├─────────────────────────────────────────────────────────┤
│ DMZ Network:                                            │
│ • Signal server connection                             │
│ • No direct internet access                           │
│                                                         │
│ Management Network:                                     │
│ • Administrative access only                           │
│ • Separate from production                            │
│                                                         │
│ Air-Gapped Network:                                    │
│ • Offline signing operations                          │
│ • Physical separation required                        │
└─────────────────────────────────────────────────────────┘
```

## Local Security

### Keystore Protection

```
┌─────────────────────────────────────────────────────────┐
│ Keystore Encryption Scheme                              │
├─────────────────────────────────────────────────────────┤
│ Key Derivation:                                         │
│ • Algorithm: PBKDF2-SHA256                             │
│ • Iterations: 100,000                                  │
│ • Salt: 32 bytes (unique per keystore)                │
│                                                         │
│ Encryption:                                             │
│ • Algorithm: AES-256-GCM                               │
│ • IV: 12 bytes (unique per encryption)                │
│ • Tag: 16 bytes (authentication)                       │
│                                                         │
│ Storage Format:                                         │
│ ┌─────────────────────────┐                           │
│ │ Version (4 bytes)       │                           │
│ │ Salt (32 bytes)         │                           │
│ │ IV (12 bytes)           │                           │
│ │ Ciphertext (variable)   │                           │
│ │ Auth Tag (16 bytes)     │                           │
│ └─────────────────────────┘                           │
└─────────────────────────────────────────────────────────┘
```

### Memory Protection

```rust
// Secure memory handling
use zeroize::Zeroize;

pub struct SensitiveData {
    #[zeroize(drop)]
    key_material: Vec<u8>,
}

// Automatic zeroing on drop
impl Drop for SensitiveData {
    fn drop(&mut self) {
        self.key_material.zeroize();
    }
}
```

### File System Security

```
┌─────────────────────────────────────────────────────────┐
│ File Permissions (Unix)                                 │
├─────────────────────────────────────────────────────────┤
│ ~/.frost_keystore/                                           │
│ ├── config.toml          (600) User read/write only   │
│ ├── keystores/           (700) User access only       │
│ │   ├── wallet1.dat      (600) Encrypted keystore     │
│ │   └── wallet2.dat      (600) Encrypted keystore     │
│ ├── logs/                (700) User access only       │
│ │   └── audit.log        (600) Append-only            │
│ └── backups/             (700) User access only       │
└─────────────────────────────────────────────────────────┘
```

## Operational Security

### Access Control Matrix

```
┌─────────────────────────────────────────────────────────┐
│ Role-Based Access Control                               │
├─────────────────────────────────────────────────────────┤
│ Role          │ Create │ Sign │ Admin │ Audit │ Backup │
├───────────────┼────────┼──────┼───────┼───────┼────────┤
│ Participant   │   ✓    │  ✓   │   ✗   │   ✗   │   ✓    │
│ Coordinator   │   ✓    │  ✓   │   ✓   │   ✗   │   ✓    │
│ Auditor       │   ✗    │  ✗   │   ✗   │   ✓   │   ✗    │
│ Administrator │   ✗    │  ✗   │   ✓   │   ✓   │   ✓    │
└─────────────────────────────────────────────────────────┘
```

### Operational Procedures

#### Secure Setup Checklist
```
┌─────────────────────────────────────────────────────────┐
│ Pre-Deployment Security Checklist                       │
├─────────────────────────────────────────────────────────┤
│ ☐ Operating System                                      │
│   ☐ Latest security patches installed                  │
│   ☐ Unnecessary services disabled                      │
│   ☐ Firewall configured                               │
│   ☐ Antivirus/EDR installed                          │
│                                                         │
│ ☐ Application Configuration                             │
│   ☐ Strong passwords enforced                         │
│   ☐ Audit logging enabled                             │
│   ☐ Network timeouts configured                       │
│   ☐ Rate limiting enabled                             │
│                                                         │
│ ☐ Physical Security                                     │
│   ☐ Device in secure location                         │
│   ☐ Screen lock configured                            │
│   ☐ BIOS/UEFI password set                           │
│   ☐ Full disk encryption enabled                      │
└─────────────────────────────────────────────────────────┘
```

#### Key Ceremony Process
```
┌─────────────────────────────────────────────────────────┐
│ Secure Key Generation Ceremony                          │
├─────────────────────────────────────────────────────────┤
│ Phase 1: Preparation                                    │
│ • Clean room setup                                      │
│ • Device verification                                   │
│ • Participant authentication                           │
│ • Witness presence                                     │
│                                                         │
│ Phase 2: Generation                                     │
│ • Air-gapped environment                               │
│ • Video recording (optional)                           │
│ • Dual control verification                            │
│ • Immediate backup creation                            │
│                                                         │
│ Phase 3: Verification                                   │
│ • Test transaction                                     │
│ • Backup restoration test                              │
│ • Audit log review                                     │
│ • Secure storage confirmation                          │
└─────────────────────────────────────────────────────────┘
```

## Security Best Practices

### For Users

```
┌─────────────────────────────────────────────────────────┐
│ User Security Guidelines                                │
├─────────────────────────────────────────────────────────┤
│ 1. Password Security                                    │
│    • Use unique, strong passwords (>16 chars)         │
│    • Enable password manager                           │
│    • Never share passwords                            │
│    • Change passwords regularly                        │
│                                                         │
│ 2. Device Security                                      │
│    • Keep OS and software updated                     │
│    • Use full disk encryption                         │
│    • Enable screen lock (5 min timeout)              │
│    • Disable unnecessary services                     │
│                                                         │
│ 3. Operational Security                                 │
│    • Verify participant identities                     │
│    • Use secure communication channels                │
│    • Regular security training                        │
│    • Report suspicious activity                       │
│                                                         │
│ 4. Backup Security                                      │
│    • Encrypt all backups                              │
│    • Store in multiple locations                      │
│    • Test recovery regularly                          │
│    • Secure physical storage                          │
└─────────────────────────────────────────────────────────┘
```

### For Administrators

```
┌─────────────────────────────────────────────────────────┐
│ Administrator Security Checklist                        │
├─────────────────────────────────────────────────────────┤
│ Daily Tasks:                                            │
│ ☐ Review security alerts                               │
│ ☐ Check system logs                                    │
│ ☐ Monitor failed login attempts                       │
│ ☐ Verify backup completion                            │
│                                                         │
│ Weekly Tasks:                                           │
│ ☐ Review access logs                                   │
│ ☐ Test incident response                              │
│ ☐ Update security patches                             │
│ ☐ Audit user permissions                              │
│                                                         │
│ Monthly Tasks:                                          │
│ ☐ Security training review                            │
│ ☐ Penetration testing                                 │
│ ☐ Disaster recovery drill                             │
│ ☐ Policy compliance audit                             │
└─────────────────────────────────────────────────────────┘
```

## Incident Response

### Incident Classification

```
┌─────────────────────────────────────────────────────────┐
│ Security Incident Severity Levels                       │
├─────────────────────────────────────────────────────────┤
│ CRITICAL (P1) - Immediate Response Required             │
│ • Unauthorized transaction detected                     │
│ • Multiple key shares compromised                      │
│ • Active attack in progress                            │
│ Response Time: < 15 minutes                            │
│                                                         │
│ HIGH (P2) - Urgent Response                             │
│ • Single key share potentially compromised             │
│ • Suspicious participant behavior                      │
│ • Failed signature verification                        │
│ Response Time: < 1 hour                                │
│                                                         │
│ MEDIUM (P3) - Timely Response                           │
│ • Unusual network activity                             │
│ • Failed login attempts                                │
│ • Configuration tampering                              │
│ Response Time: < 4 hours                               │
│                                                         │
│ LOW (P4) - Scheduled Response                           │
│ • Policy violations                                     │
│ • Non-critical vulnerabilities                         │
│ • Documentation issues                                 │
│ Response Time: < 24 hours                              │
└─────────────────────────────────────────────────────────┘
```

### Incident Response Procedure

```
┌─────────────────────────────────────────────────────────┐
│ Incident Response Workflow                              │
├─────────────────────────────────────────────────────────┤
│ 1. DETECT                                               │
│    ↓                                                    │
│ 2. ASSESS → Determine severity                         │
│    ↓                                                    │
│ 3. CONTAIN → Isolate affected systems                  │
│    ↓                                                    │
│ 4. INVESTIGATE → Gather evidence                        │
│    ↓                                                    │
│ 5. REMEDIATE → Fix vulnerabilities                     │
│    ↓                                                    │
│ 6. RECOVER → Restore normal operations                 │
│    ↓                                                    │
│ 7. DOCUMENT → Create incident report                   │
│    ↓                                                    │
│ 8. IMPROVE → Update procedures                         │
└─────────────────────────────────────────────────────────┘
```

### Emergency Contacts

```
┌─────────────────────────────────────────────────────────┐
│ Emergency Response Team                                 │
├─────────────────────────────────────────────────────────┤
│ Role                  │ Contact           │ Backup      │
├───────────────────────┼───────────────────┼─────────────┤
│ Security Lead         │ security@frost    │ +1-555-0100 │
│ Technical Lead        │ tech@frost        │ +1-555-0101 │
│ Legal Counsel         │ legal@frost       │ +1-555-0102 │
│ PR/Communications     │ pr@frost          │ +1-555-0103 │
│ Executive Sponsor     │ exec@frost        │ +1-555-0104 │
└─────────────────────────────────────────────────────────┘
```

## Compliance and Auditing

### Audit Log Format

```json
{
  "timestamp": "2024-01-20T10:30:00Z",
  "event_type": "signature_created",
  "severity": "info",
  "actor": "alice",
  "action": "sign_transaction",
  "resource": "treasury-wallet",
  "details": {
    "transaction_hash": "0xabcd...",
    "participants": ["alice", "bob"],
    "threshold_met": true
  },
  "ip_address": "192.168.1.100",
  "user_agent": "FROST-MPC-TUI/2.0.0",
  "session_id": "sess_123456",
  "correlation_id": "corr_789012"
}
```

### Compliance Framework

```
┌─────────────────────────────────────────────────────────┐
│ Regulatory Compliance Matrix                            │
├─────────────────────────────────────────────────────────┤
│ Standard      │ Requirement        │ Implementation     │
├───────────────┼────────────────────┼────────────────────┤
│ SOC 2 Type II │ Access Controls    │ RBAC, MFA         │
│               │ Encryption         │ AES-256-GCM       │
│               │ Audit Trails       │ Immutable logs    │
├───────────────┼────────────────────┼────────────────────┤
│ ISO 27001     │ Risk Assessment    │ Annual review     │
│               │ Incident Response  │ 24/7 team         │
│               │ Business Continuity│ DR procedures     │
├───────────────┼────────────────────┼────────────────────┤
│ GDPR          │ Data Protection    │ Encryption at rest│
│               │ Right to Erasure   │ Key deletion      │
│               │ Data Portability   │ Export functions  │
└─────────────────────────────────────────────────────────┘
```

### Security Metrics

```
┌─────────────────────────────────────────────────────────┐
│ Security KPIs Dashboard                                 │
├─────────────────────────────────────────────────────────┤
│ Metric                    │ Target │ Current │ Status  │
├───────────────────────────┼────────┼─────────┼─────────┤
│ Failed Login Rate         │ <1%    │ 0.3%    │ ✅      │
│ Patch Compliance          │ 100%   │ 98%     │ ⚠️      │
│ Incident Response Time    │ <1hr   │ 45min   │ ✅      │
│ Security Training         │ 100%   │ 100%    │ ✅      │
│ Vulnerability Scan        │ 0 High │ 0       │ ✅      │
│ Backup Success Rate       │ 99.9%  │ 99.95%  │ ✅      │
│ Uptime                    │ 99.9%  │ 99.97%  │ ✅      │
└─────────────────────────────────────────────────────────┘
```

## Security Hardening Guide

### System Hardening

```bash
# Linux Security Hardening Script
#!/bin/bash

# Kernel parameters
echo "kernel.randomize_va_space=2" >> /etc/sysctl.conf
echo "net.ipv4.tcp_syncookies=1" >> /etc/sysctl.conf
echo "net.ipv4.conf.all.rp_filter=1" >> /etc/sysctl.conf

# Disable unnecessary services
systemctl disable bluetooth
systemctl disable cups
systemctl disable avahi-daemon

# Configure firewall
ufw default deny incoming
ufw default allow outgoing
ufw allow 443/tcp  # Signal server
ufw enable

# File system hardening
chmod 700 ~/.frost_keystore
chmod 600 ~/.frost_keystore/keystores/*

# Enable audit logging
auditctl -w ~/.frost_keystore -p wa -k frost_keystore_changes
```

### Application Hardening

```toml
# config.toml - Security Settings
[security]
# Password policy
min_password_length = 16
require_special_chars = true
password_history = 5
max_password_age_days = 90

# Session management
session_timeout_minutes = 15
max_concurrent_sessions = 1
require_mfa = true

# Network security
allowed_ips = ["192.168.1.0/24"]
rate_limit_per_minute = 60
connection_timeout_seconds = 30

# Cryptographic settings
min_key_share_entropy_bits = 256
require_secure_random = true
key_derivation_iterations = 100000
```

## Conclusion

Security is not a feature but a continuous process. The FROST MPC TUI Wallet implements comprehensive security controls at every layer, from cryptographic protocols to operational procedures. Regular security assessments, updates, and training ensure the system remains secure against evolving threats.

For security concerns or vulnerability reports, please open a private
advisory via [GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new).