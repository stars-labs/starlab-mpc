# Terminal UI Node Documentation

## Overview

The MPC Wallet Terminal UI (TUI) Node is a keyboard-driven terminal application for distributed key generation (FROST DKG), threshold signing, and encrypted keystore management. Built in Rust with Ratatui via the tui-realm Elm-architecture framework; interoperates with the browser extension + native desktop app over the same wire protocol and keystore format.

## Documentation Structure

### Core Documentation
- [Architecture](architecture/) - System design, state management, network protocols
- [User Guide](guides/) - Complete user manual and tutorials
- [Protocol](protocol/) - WebRTC signaling and keystore session protocols

Earlier drafts of this section also linked to `ui/` for
"Interface wireframes and navigation flows" — that directory
does not exist under `apps/tui/docs/` (verified via `ls`;
only the legacy wireframes under `archive/legacy-ui/` remain).
The UI is keyboard-driven and documented inline via the
[`KEYBOARD_NAVIGATION_GUIDE.md`](./KEYBOARD_NAVIGATION_GUIDE.md)
+ per-component source under `src/elm/components/`.

## Features

### Core Capabilities
- **Interactive Terminal UI**: Menu-driven interface with keyboard navigation
- **FROST MPC Protocol**: Secure threshold signatures with distributed key generation
- **Multi-Blockchain Support**: Ethereum (secp256k1) and Solana (ed25519)
- **Offline Mode**: Air-gapped operations for maximum security
- **Encrypted Keystore**: Password-protected local key storage

### Additional Features
- **WebSocket Reconnect**: Signal-server connection is re-established
  automatically after drops; mid-ceremony resumption depends on the
  peers still holding their in-memory FROST state.
- **Multi-Wallet Support**: Manage multiple wallets per device. Each
  wallet is partitioned by curve (ed25519 / secp256k1) under the
  device_id in `~/.frost_keystore/`.
- **Import/Export**: Keystore round-trips with the browser extension
  are test-covered (same PBKDF2 + AES-256-GCM format).

## Quick Start

### Prerequisites
- Rust 1.85+ (workspace is edition 2024; see root `Cargo.toml`)
- Linux, macOS, or Windows
- Terminal with UTF-8 support

### Installation

```bash
# Clone repository
git clone https://github.com/hecoinfo/starlab-mpc
cd starlab-mpc/apps/tui

# Build from source
cargo build --release

# Run the application
cargo run --release -- --device-id Device-001
```

### Command Line Options

See § Configuration → CLI Flags below for the real flag list
(`--device-id` / `--offline` / `--signal-server` / `--log-location` /
`--log-level`). Earlier drafts of this section duplicated the list
with fabricated flags (`--config <PATH>`, `--keystore <PATH>`, a
`[required]` marker on `--device-id`, explicit `--help`). None of
those are real:

- `--config` / `--keystore` — do not exist; the TUI has no config
  file and the keystore path is hardcoded to `~/.frost_keystore`.
- `--device-id` is **optional**, not required — defaults to the
  machine hostname via `gethostname::gethostname()`.
- `--help` works because `clap::Parser` generates it automatically;
  it isn't an explicit flag in the `Args` struct.

Verified against `apps/tui/src/bin/starlab-tui.rs:13-39`.

## User Interface

### Main Screen Layout

Launch the binary and you see the main menu defined in
`src/elm/components/main_menu.rs:55-114`. Items vary with wallet
state:

- **Always shown**: `Create New Wallet`, `Join Session`,
  `Settings`, `Exit`
- **Added when `wallet_count > 0`**: `Manage Wallets` and
  `Sign Transaction` — both appear only once at least one wallet
  exists in the keystore. DKG-progress surfaces live inside
  sub-screens; `Sign Transaction` is the top-level entry point
  into the signing flow.

Earlier drafts of this section printed a `[1] Wallet / [2] DKG /
[3] Sign / [4] Session / [5] Network / [6] Settings / [Q] Quit`
numbered layout with a right-hand pane showing `Current Wallet` /
`Address` / `Balance: 1.234 ETH`. None of that is real — the menu
uses arrow-key navigation over an emoji-icon list with no number
hotkeys, there is no wallet-summary side pane, and the TUI does
NOT query on-chain balances (so the `1.234 ETH` figure was
fabricated).

### Navigation

Keys actually handled by `src/elm/update.rs` + per-component
`on(KeyEvent)` impls:

| Key | Action |
|-----|--------|
| ↑ / ↓ | Move selection within a menu or list |
| Enter | Select the highlighted item |
| Esc | Go back one screen / cancel the current operation |
| Tab | Move focus between fields inside a form |
| q | Quit from the main menu |

Earlier drafts listed `Space` / `?` bindings — neither is wired up,
and no context-help modal ships. See
[`KEYBOARD_NAVIGATION_GUIDE.md`](./KEYBOARD_NAVIGATION_GUIDE.md)
for the authoritative reference.

## Core Workflows

### Creating a Wallet

1. From the main menu (arrow keys, not number hotkeys), select
   **Create New Wallet**.
2. Fill the form: Name, Threshold, Total participants, Blockchain
   (Ethereum and/or Solana — unified DKG produces both address
   types from a single ceremony via
   `frost-core::unified_dkg`).
3. The creator's node mints a session id and broadcasts
   `AnnounceSession`; co-signers discover it via
   `SessionAvailable`.
4. Co-signers pick the session from **Join Session** and enter
   the session id.
5. When participants == total, DKG runs automatically
   (Round 1 → Round 2 → Finalization).
6. The wallet is persisted as a single JSON file at
   `~/.frost_keystore/<device_id>/<curve>/<wallet_id>.json` —
   plaintext metadata plus the base64-encoded encrypted share
   inside a `WalletFile` wrapper (no separate `.dat` blob, despite
   what earlier drafts of this step claimed).

Earlier drafts said "Select [1] Wallet from main menu" — no such
item exists; the top-level label is "Create New Wallet".

### Distributed Key Generation (DKG)

```
Online DKG:
Device A ──┐
           ├──► Signal Server ──► WebRTC Mesh ──► FROST DKG ──► Key Shares
Device B ──┘

Offline DKG (SD-card air-gap):
Device A ──► Export (.json) ──► USB ──► Device B
                                          ↓
                                   Process Offline
                                          ↓
Device B ──► Export (.json) ──► USB ──► Device A
```

### Transaction Signing

Live today the TUI supports **message signing**, not full
transaction construction-and-signing — see the "Phase C scope"
note in `src/elm/components/sign_transaction.rs`. The flow:

1. From the main menu select **Manage Wallets** and pick a wallet.
2. Choose **Sign Message** (EIP-191 `personal_sign` shape, so the
   resulting secp256k1 signature is `ecrecover`-compatible for
   Ethereum; ed25519 wallets produce raw signatures for Solana).
3. Broadcast a signing session; co-signers discover it via
   **Join Session** (same transport as DKG).
4. Each participant approves the message. Once threshold
   commitments + threshold shares arrive the aggregator emits
   the final signature.

Earlier drafts said "Select [3] Sign / load transaction from
file / 7-step flow". The numeric hotkey doesn't exist, and
transaction-from-file loading is not a shipped code path — signing
input is a hex-encoded message supplied in the SignTransaction
component.

## Configuration

The TUI has no config-file support today — all runtime settings are
passed as CLI flags. Keystore location is fixed at `~/.frost_keystore`.

### CLI Flags

```
--device-id <ID>            Device identity in the FROST mesh.
                            Defaults to the machine hostname.
--signal-server <URL>       WebSocket signal server.
                            Default: wss://xiongchenyu.dpdns.org
--offline                   Run without network (SD-card DKG mode).
--log-location <PATH>       Log file path.
                            Default: ~/.frost_keystore/logs/starlab-mpc.log
--log-level <LEVEL>         error | warn | info | debug | trace
                            Default: info
```

See `apps/tui/src/bin/starlab-tui.rs` for the authoritative
definitions.

### Environment Variables

Three env vars are consulted:

| Variable           | Effect                                       |
|--------------------|----------------------------------------------|
| `HOME`             | Used to compute the keystore path.           |
| `PERF_MONITORING`  | If set (any value), enables perf counters.   |
| `RUST_LOG`         | Standard tracing-subscriber directive.       |

## Advanced Features

### Offline Mode

For air-gapped operations, launch with `--offline` and navigate
via arrow keys + Enter:

```bash
starlab-tui --offline --device-id Device-001
```

The menu structure — no numeric hotkeys — is:

  Main Menu → Create New Wallet (select Offline mode)
            OR Join Session (enter a session id distributed out of band)

Per-round Export / Import buttons are surfaced by each offline
ceremony screen; bundles are JSON envelopes per `OfflineData`
(`src/offline/types.rs:12`). Earlier drafts of this section
showed `[4] Session → Export to File` — no numeric hotkeys
exist; the § Main Screen Layout scope note above enumerates
the real menu items.

### Multi-Wallet Management

- Multiple wallets per device, partitioned by curve
  (`ed25519` / `secp256k1`) under the device_id.
- HD key derivation via BIP-44-style additive scalar offsets
  (`frost-core::hd_derivation`), so child keys share the DKG
  group without extra rounds.
- Import/export via the Manage Wallets screen uses the
  `WalletFile` JSON envelope — round-trip with the browser
  extension is covered by the interop tests under
  `apps/browser-extension/tests/`.

### Performance notes (aspirational vs actual)

Earlier drafts of this section promised "Connection pooling for
WebRTC", "Message batching", "State caching", and "Lazy loading
of wallet data". `grep` for the corresponding symbols
(`ConnectionPool`, `connection_pool`, `message_batch`,
`state_cache`, `lazy_load`) in `apps/tui/src/` returns
zero hits — none shipped.

What IS real: WebRTC peer connections are held in a per-peer
`HashMap` so repeat messages reuse the same DataChannel, and
`tokio::mpsc` channels absorb bursty signal-server traffic.
That's the whole performance story today.

## Troubleshooting

### Common Issues

#### Connection Problems
```bash
# Check signal server connectivity (TLS + HTTP, not a health endpoint —
# the server is WebSocket-only, so this just verifies reachability)
curl -v https://xiongchenyu.dpdns.org/

# Enable debug logging
RUST_LOG=debug starlab-tui --device-id Device-001
```

#### DKG Failures
- Ensure all participants are online
- Check network firewall settings
- Verify matching threshold/participant counts
- Review logs for timeout issues

#### Performance Issues
- No config file exists — earlier drafts suggested "reduce UI
  refresh rate in config" and "disable animations", neither of
  which is a supported setting. The Ratatui render path is
  event-driven (no timer-based refresh loop) and there are no
  animations.
- Check system resources (CPU, memory).
- For WebRTC mesh performance: full-mesh degree is `n·(n-1)/2`
  peer connections, so keep cohorts small or pair with a
  TURN server (no TURN ships — see OFFLINE_DKG_GUIDE.md
  for the absent-infra list).

## Security Considerations

- **Never share** your device ID password
- **Always verify** session IDs with participants
- **Use offline mode** for high-security operations
- **Regular backups** of keystore recommended
- **Encrypt keystores** at rest

## Resources

- [Architecture Documentation](architecture/)
- [User Guide](guides/USER_GUIDE.md)
- [Protocol Specification](protocol/)
- [Main Project Docs](../../../docs/)

(Earlier drafts listed a `[UI Wireframes](ui/)` link — that
directory does not exist; the UI is documented inline in the
keyboard guides.)

## Support

For help and support:
- [GitHub Issues](https://github.com/hecoinfo/starlab-mpc/issues)
- [Documentation](../../../docs/)

---

[← Back to Apps](../../) | [→ Architecture](architecture/)