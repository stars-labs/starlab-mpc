# TUI Node Architecture Documentation

This directory contains technical architecture and design
documentation for the MPC Wallet TUI application.

## Contents

- `ARCHITECTURE.md` — Overall system architecture
- `DKG_FLOWS.md` — DKG protocol flows and state machines
- `ELM_ARCHITECTURE.md` — Elm-architecture app loop (Model /
  Update / View / Command) as implemented in `src/elm/`
- `SECURITY.md` — Security considerations (see the scope note
  inside — separates real controls from aspirational ones)
- `keystore_design.md` — Keystore format + metadata schema

Earlier drafts of this README listed five additional files
(`01_keystore_design.md`, `keystore_session_final_design.md`,
`keystore_session_recovery.md`, `keystore_sessions_implementation.md`,
`simplified_keystore_session_design.md`) — those are archived
under [`../archive/legacy-keystore-docs/`](../archive/legacy-keystore-docs/),
not present here.

## Architecture Overview

- **Rust** (edition 2024) — core implementation
- **Ratatui + tui-realm** — terminal UI with Elm architecture
- **Tokio** — async runtime
- **webrtc-rs** — WebRTC mesh for peer-to-peer ceremony traffic
- **tokio-tungstenite** — WebSocket client for signal server
- **frost-core 2.2** (plus `frost-ed25519` / `frost-secp256k1`) —
  threshold signature primitives from ZCash Foundation

## Key Components

1. **UI Layer** (`src/elm/`) — tui-realm app + per-screen
   Components in `src/elm/components/`
2. **Business Logic** (`src/core/`) — `*Manager` types shared
   with native-node
3. **Network Layer** — WebRTC mesh + WS signal-server client:
   - `src/network/webrtc.rs` — one of the two real production
     RTCPeerConnection construction sites
   - `src/elm/webrtc_signaling.rs` — the other real runtime driver
     (offer/answer/ICE exchange through the Elm loop)
   - `src/elm/ws_runtime.rs` — WebSocket client for the signal
     server
   - `src/webrtc/` — test-harness library (`WebRTCMeshManager` +
     `ConnectionMonitor` + `RejoinCoordinator` + `MeshSimulator`)
     consumed only by `examples/webrtc_mesh_e2e_test.rs`; NOT
     wired into the production Elm runtime despite its suggestive
     module name
4. **Storage Layer** (`src/keystore/`) — encrypted keystore
5. **Protocol Layer** (`src/protocal/`) — wire types + DKG/signing
   state machines (note: `protocal` is an intentional misspelling)
6. **Offline Layer** (`src/offline/`, `src/hybrid/`) — SD-card
   air-gap + mixed-mode

## Related Documentation

- [User guides](../guides/)
- [Parent docs index](../index.md)
- [Historical UI wireframes (pre-componentization)](../archive/legacy-ui/)
- [Legacy keystore design docs (archived)](../archive/legacy-keystore-docs/)
- [Dev-journal archive](../archive/dev-journal/)
