# MPC Wallet TUI Documentation

Terminal UI for the MPC Wallet — a t-of-n FROST threshold wallet
with distributed key generation and threshold signing, built on
Ratatui + tui-realm in the Elm architecture.

## What it is

- Full keyboard-driven TUI — no REPL, no typed commands. Navigate
  with arrow keys, confirm with Enter, back out with Esc.
- Runs as the standalone `mpc-wallet-tui` binary. Reuses the same
  `frost-core` backend as the browser extension and native desktop
  app, so TUI participants can co-sign with extension or native
  participants in the same mesh.
- Supports both online (WebRTC mesh) and offline (SD-card air-gap)
  DKG and signing flows.

## Documentation Structure

### [User Guides](./guides/)
- **[User Guide](./guides/USER_GUIDE.md)** — walkthrough
- **[Offline Mode Guide](./guides/offline-mode.md)** — air-gapped ceremony procedure

### [Architecture Documentation](./architecture/)
- **[Architecture Overview](./architecture/ARCHITECTURE.md)**
- **[Elm Architecture](./architecture/ELM_ARCHITECTURE.md)**
- **[DKG Flows](./architecture/DKG_FLOWS.md)**
- **[Security Model](./architecture/SECURITY.md)**
- **[Keystore Design](./architecture/keystore_design.md)**

### [Protocol Specifications](./protocol/)
- **[WebRTC Signaling](./protocol/01_webrtc_signaling.md)**

### Other references in this directory
- [Top-level README](./README.md) — quick-start + CLI flags
- [`KEYBOARD_NAVIGATION_GUIDE.md`](./KEYBOARD_NAVIGATION_GUIDE.md)
- [`KEYBOARD_HANDLING_GUIDE.md`](./KEYBOARD_HANDLING_GUIDE.md) — for developers adding new screens
- [`MPC_WALLET_TUI_ARCHITECTURE.md`](./MPC_WALLET_TUI_ARCHITECTURE.md) — deeper state-machine writeup
- [`WEBRTC_MESH_IMPLEMENTATION.md`](./WEBRTC_MESH_IMPLEMENTATION.md)
- [`OFFLINE_DKG_GUIDE.md`](./OFFLINE_DKG_GUIDE.md) — manual ceremony procedure
- [`COMPLETE_OFFLINE_WORKFLOW.md`](./COMPLETE_OFFLINE_WORKFLOW.md)
- [`DEPLOYMENT_GUIDE.md`](./DEPLOYMENT_GUIDE.md)
- Test-design specs: [`E2E_TEST_DESIGN.md`](./E2E_TEST_DESIGN.md), [`HYBRID_MODE_TEST_DESIGN.md`](./HYBRID_MODE_TEST_DESIGN.md), [`WEBRTC_MESH_TEST_DESIGN.md`](./WEBRTC_MESH_TEST_DESIGN.md), [`KEYSTORE_E2E_TEST_PLAN.md`](./KEYSTORE_E2E_TEST_PLAN.md)
- [`archive/`](./archive/) — historical design docs, dev-journal retrospectives

## Quick Start

```bash
# From the repo root
cargo run --bin mpc-wallet-tui -p tui-node -- --device-id alice
```

Keyboard basics:

| Keys              | Action                     |
|-------------------|----------------------------|
| ↑ / ↓             | Navigate menu items        |
| Enter             | Select / confirm           |
| Esc               | Go back / cancel           |
| Tab               | Move focus within a screen |
| Ctrl+Q / Ctrl+C   | Quit                       |
| Ctrl+R            | Refresh                    |
| Ctrl+H            | Navigate to home (main menu) |

The full key map per screen is in [`KEYBOARD_NAVIGATION_GUIDE.md`](./KEYBOARD_NAVIGATION_GUIDE.md).
There is no global help key (`?`) — the shortcut listed in earlier
drafts of this page was not actually implemented. The four Ctrl
globals above are handled in `src/elm/app.rs:851-866` before
per-component dispatch.

## Feature overview

### Multi-party computation

- **DKG**: FROST distributed key generation for t-of-n schemes
  (2-of-3, 3-of-5, etc.) via the ZCash `frost-core 2.2` crates.
- **Threshold signing**: any `t`-subset of participants can
  collaboratively produce a valid signature — no single device ever
  holds the complete private key.
- **Multi-chain**: secp256k1 → Ethereum (+ L2s that share the
  address format), ed25519 → Solana.

### Online mode

- WebRTC full-mesh between participants, using the signal server at
  `wss://xiongchenyu.dpdns.org` (Cloudflare Worker — see
  `docs/deployment/CLOUDFLARE_DEPLOYMENT.md`) to bootstrap.
- Peer-to-peer DKG / signing traffic rides DTLS-encrypted WebRTC
  data channels. The signal server only sees session announcements
  and opaque relay envelopes once the mesh is up.

### Offline mode

- Complete air-gap — no network interfaces consulted. DKG / signing
  rounds exchanged via SD card (or any physical media). Each round
  exports a JSON bundle on the coordinator, imports on participants,
  then re-exports for the next round.
- See [`OFFLINE_DKG_GUIDE.md`](./OFFLINE_DKG_GUIDE.md) for the
  operator procedure and [`COMPLETE_OFFLINE_WORKFLOW.md`](./COMPLETE_OFFLINE_WORKFLOW.md)
  for the combined DKG + signing flow.

### Keystore

- Key shares at rest are AES-256-GCM with password-derived keys
  (PBKDF2-HMAC-SHA256 at 100k iterations or Argon2id, selectable
  per wallet). Stored as a single JSON file per wallet at
  `~/.frost_keystore/<device_id>/<curve>/<wallet_id>.json` —
  plaintext metadata plus the base64-encoded ciphertext inside a
  `WalletFile` JSON wrapper (earlier drafts of this section
  described a two-file `.json` + `.dat` split; no `.dat` file is
  ever written).
- Import/export round-trips with the browser extension keystore
  (same format), covered by the interop tests under
  `apps/browser-extension/tests/`.

## Security

Security claims are deliberately conservative. Threshold cryptography
gives you one strong property (compromise of fewer than `t` shares
can't sign), and DTLS+AES-GCM cover transport and at-rest encryption.
No third-party audit has been performed on this codebase. See
[`SECURITY.md`](./architecture/SECURITY.md) for what the implementation
actually provides vs. what's open hardening work.

## Support

- **Issues**: [GitHub Issues](https://github.com/hecoinfo/mpc-wallet/issues)
- **Security**: [GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new)
