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

1. **Threshold trust**: No fewer than `t`-of-`n` participants together can produce a signature — compromise of `< t` key shares is insufficient
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

### TLS / DTLS scope

This application does NOT configure TLS cipher suites, versions,
or OCSP settings directly — both layers are delegated:

- **Signal-server TLS (WebSocket)**: clients connect via `wss://`
  and trust the system CA store. `tokio-tungstenite` uses the
  native TLS stack on each platform. The public Cloudflare Worker
  endpoint at `wss://xiongchenyu.dpdns.org` gets its TLS from
  Cloudflare's edge. Operators running the self-hosted signal
  server terminate TLS in their own reverse proxy (nginx / caddy /
  Cloudflare Tunnel — see `docs/deployment/README.md`).
- **WebRTC DTLS-SRTP**: version + cipher selection happens inside
  the `webrtc` crate / browser WebRTC implementation. Earlier
  drafts claimed "DTLS 1.3" specifically — the protocol version
  is whatever the underlying library negotiates, not something
  this app pins. Data channels (no media tracks) ride DTLS-SRTP
  as usual.

No certificate pinning, no HSTS enforcement, no OCSP stapling,
no TURN-server authentication (no TURN infra ships — clients rely
STUN only in the browser extension — TUI currently ships with
empty ICE-server config; STUN for the TUI is open work at
`src/network/webrtc.rs:285`), no SDP sanitization layer. Earlier drafts
of this section enumerated all of those as features — they're not
implemented.

### Network Isolation

For operators who want to deploy the self-hosted signal server in
a segmented network, standard network-engineering practice applies
(DMZ for the WS listener, separate management interface, air-gap
for offline participants). The application itself doesn't ship
any tooling that enforces this segmentation — it's an operator-side
concern.

## Local Security

### Keystore Protection

Values below are the real constants in
`apps/tui-node/src/keystore/encryption.rs`:

```
Key Derivation:
  Algorithm:  PBKDF2-HMAC-SHA256
  Iterations: 100_000  (PBKDF2_ITERATIONS constant)
  Salt:       16 bytes (SALT_LEN = 16, fresh per wallet)

Encryption:
  Algorithm:  AES-256-GCM
  Nonce:      12 bytes (NONCE_LEN = 12, fresh per encryption)
  Auth tag:   16 bytes (standard GCM tag, appended to ciphertext
              by the aes-gcm crate — no separate storage field)

Internal framing of the ciphertext produced by
`encrypt_data_with_method` (no version prefix):
  ┌─────────────────────────────────────────────┐
  │ salt       (16 B)                           │
  │ nonce      (12 B)                           │
  │ ciphertext + GCM auth tag (variable bytes)  │
  └─────────────────────────────────────────────┘
```

Earlier drafts of this section claimed a 32-byte salt and a
leading `Version (4 bytes)` field — neither is true (verified
against `encryption.rs:20-21` for the constants and `:99` for
the write format).

**On-disk shape**: the framed ciphertext above is NOT written to
its own `.dat` file — it's base64-encoded and embedded inside the
single `<wallet_id>.json` wallet file (the `data` field of the
`WalletFile` struct, `src/keystore/models.rs:438-453`). Earlier
drafts of this section labeled the framing above as the "`.dat`
on-disk layout" and showed a two-file directory tree (`.json`
metadata + `.dat` ciphertext); that layout never shipped.

### Memory Protection

Today only `packages/@mpc-wallet/frost-core/src/root_secret.rs`
zeros sensitive material on drop, and it does so via a manual
`self.0.fill(0)` inside `impl Drop for RootSecret` at
`root_secret.rs:62-67` — NOT via the `zeroize` crate (`zeroize`
is not a workspace dependency; `grep -rn zeroize` returns a
single match, and that's inside a code comment on the `.fill(0)`
line). Key shares, decrypted keystore blobs, passwords entered
into the PasswordPrompt screen, and signing intermediate state
are NOT zeroed on drop. Earlier drafts of this doc showed a
`#[zeroize(drop)] SensitiveData` struct with automatic wiping
and even asserted `zeroize::Zeroize` was in use — neither is
true in the current tree.

Adding systematic zeroization (at minimum to: `key_package`,
`group_public_key`, `frost_nonces`, `frost_commitments`,
`frost_signature_shares`, `signing_message`, and the
PasswordPrompt draft buffers) is tracked as open hardening work.

### File System Security

```
┌─────────────────────────────────────────────────────────┐
│ File Permissions (Unix, recommended)                   │
├─────────────────────────────────────────────────────────┤
│ ~/.frost_keystore/                                      │
│ ├── index.json           (600) Legacy wallet index     │
│ ├── device_id            (600) Device identity         │
│ └── <device_id>/                                        │
│     ├── ed25519/         (700) Curve-scoped dir        │
│     │   └── <wid>.json   (600) WalletFile (plaintext   │
│     │                          metadata + base64       │
│     │                          encrypted share)        │
│     └── secp256k1/       (700)                         │
│         └── <wid>.json   (600) same format             │
└─────────────────────────────────────────────────────────┘
```

The current implementation calls `fs::create_dir_all` for the
directories but does not explicitly `chmod` them — inheriting the
user's umask. Hardening steps below still apply as a defence-in-depth
recommendation.

## Operational Security

### Access Control Matrix

```
┌─────────────────────────────────────────────────────────┐
│ Role-Based Access Control                               │
├─────────────────────────────────────────────────────────┤
│ No RBAC model is implemented. The only access-control primitive │
│ is the threshold itself: any `t`-of-`n` participants who hold   │
│ the decrypted key shares (obtained by entering the correct      │
│ keystore password for each) can sign. There's no Coordinator    │
│ vs Auditor vs Administrator distinction; the "coordinator" is   │
│ just whichever participant initiated a given DKG / signing      │
│ ceremony and has no elevated privileges afterward.              │
│                                                                 │
│ Earlier drafts of this section showed a                         │
│ Participant / Coordinator / Auditor / Administrator matrix      │
│ with permissions for Create / Sign / Admin / Audit / Backup —   │
│ that RBAC scheme does not exist in code.                        │
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
│ ☐ Application Configuration (operator-side)             │
│   ☐ Strong keystore password chosen (no policy is      │
│     enforced in code — pick a strong one yourself)     │
│   ☐ Signal-server URL trusted / under your control     │
│     (--signal-server flag; default is the upstream     │
│     Cloudflare Worker)                                 │
│   ☐ Log file location set (--log-location) + log       │
│     rotation/retention managed by operator tooling     │
│   Note: the TUI does not enforce password complexity,  │
│   emit structured audit logs, configure network        │
│   timeouts, or rate-limit connections. Earlier drafts  │
│   of this checklist listed those as "enabled /         │
│   configured" items — none are application features.   │
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

This is an open-source project with no paid on-call response team
and no defined SLA. Earlier drafts of this section listed an
"Incident Classification" severity matrix with response-time
commitments (< 15 min for P1, < 1 hr for P2, etc.), an
8-step DETECT → ASSESS → CONTAIN … workflow, and an Emergency
Contacts table with fabricated phone numbers (`+1-555-0100`) and
email prefixes (`security@frost`, `tech@frost`). None of that
scaffolding exists. The real reporting path is:

- **Security vulnerabilities**: [GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new)
- **Operational bugs**: [GitHub Issues](https://github.com/hecoinfo/mpc-wallet/issues)

Each report is handled by whoever is maintaining the project;
best-effort response, not a contractual SLA.

For self-hosted operators, standard incident-response practice
applies to your deployment: detect via your own monitoring,
contain by stopping the affected signal-server / removing
suspect devices from the mesh, investigate through application
logs + `chrome://webrtc-internals`, remediate in your own
deployment, and document internally. The application itself
doesn't generate incident-response artefacts.

## Compliance and Auditing

### Audit logs

The application does not emit structured audit log events.
Earlier drafts of this section showed a JSON format with
`actor`, `action`, `resource`, `ip_address`, `session_id`,
`correlation_id` fields — no such output is produced. All
runtime information surfaces via `tracing` / `RUST_LOG`
diagnostic logs, which operators can ship through their own
pipeline but which are NOT structured as a tamper-evident audit
trail.

If an audit-log schema is needed for a regulated deployment,
adding one is open work — the natural hook points are in the
`Command::execute` path (side-effect emissions) and the
signal-server message loop.

### Compliance Framework

This codebase is NOT certified against SOC 2 Type II, ISO 27001,
GDPR, or any other regulatory standard. Earlier drafts of this
section listed a three-row compliance matrix with "Implementation"
columns citing features ("RBAC, MFA", "Immutable logs", "24/7
team", "Annual review", "DR procedures") that don't exist.

What this codebase actually does:

- **Encryption at rest**: AES-256-GCM + PBKDF2 keystore (real,
  documented above)
- **No RBAC**: there are no roles or permission scopes; a
  participant either has a key share or doesn't
- **No MFA**: access is gated by password for the keystore unlock
- **No immutable logs, no 24/7 response, no audited risk
  assessment, no DR runbook** — these are deliberately not
  claimed

Operators deploying this for a regulated use case are
responsible for their own compliance layer (key-management
policies, audit wrappers, incident response, etc.).

### Security Metrics

No operational metrics are currently collected by this codebase
(no `/metrics` endpoint, no Prometheus integration, no KPI
dashboard — see the tech doc's Monitoring section, 41d5ca0).
Earlier drafts of this section listed a "KPI Dashboard" table
with specific percentages (`Failed Login Rate 0.3%`,
`Patch Compliance 98%`, `Incident Response Time 45min`,
`Uptime 99.97%`). Those numbers had no source and have been
removed.

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
chmod -R go-rwx ~/.frost_keystore

# Enable audit logging
auditctl -w ~/.frost_keystore -p wa -k frost_keystore_changes
```

### Application Hardening

The TUI does NOT read a `config.toml` file — all runtime settings
are CLI flags (see `docs/README.md` § Configuration, rewritten in
6f896ad). Earlier drafts of this section showed a `[security]`
TOML block with:

  - `min_password_length`, `require_special_chars`, `password_history`,
    `max_password_age_days` — no password policy is enforced beyond
    "the keystore unlock must match"
  - `session_timeout_minutes`, `max_concurrent_sessions`, `require_mfa`
    — no session timeout, no session counter, no MFA
  - `allowed_ips`, `rate_limit_per_minute`, `connection_timeout_seconds`
    — no IP allowlist, no rate limiter (the signal server is an
    unauthenticated relay), connection timeouts are transport
    defaults
  - `min_key_share_entropy_bits`, `require_secure_random`,
    `key_derivation_iterations` — the first two are not settings
    (the code unconditionally uses OS-entropy ChaCha20Rng for
    FROST); PBKDF2 iterations are the hardcoded `PBKDF2_ITERATIONS`
    constant (100_000)

None of those config keys are read anywhere in source. Hardening
today is entirely in-code constants + the operator's own
deployment posture (firewall, kernel sysctls — see the System
Hardening subsection above).

## Conclusion

Security is a continuous process. This codebase gives you threshold
cryptography (FROST t-of-n, upstream ZCash `frost-core 2.2`) and
encrypted keystores (AES-256-GCM + PBKDF2). It does not give you
RBAC, MFA, audit logs, a compliance framework, or a formal
response SLA — those live outside this doc and, if needed, must
be added by the operator.

For vulnerability reports, open a private advisory via
[GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new).
For operational bugs, use [GitHub Issues](https://github.com/hecoinfo/mpc-wallet/issues).