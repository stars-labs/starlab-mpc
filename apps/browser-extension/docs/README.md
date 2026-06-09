# Browser Extension Documentation

## Overview

The MPC Wallet Browser Extension is a Manifest V3 Chrome/Firefox extension that provides secure multi-party computation wallet functionality directly in the browser. Built with TypeScript, Svelte, and WebAssembly, it enables threshold signatures without any single party having access to complete private keys.

## Documentation Structure

- [User Interface](ui/) — popup flow, DKG progress, signing UX
- Extension architecture, message flow, and FROST-WASM details live
  directly in comments inside the offscreen / background source
  (start at `src/entrypoints/offscreen/webrtc.ts` and the top-of-repo
  `CLAUDE.md`).

## Quick Start

### Prerequisites
- Bun runtime (`curl -fsSL https://bun.sh/install | bash`) —
  this is a Bun workspace, not npm/yarn/Node.js
- Chrome/Firefox browser
- Rust toolchain with the `wasm32-unknown-unknown` target
  (wasm-pack is a devDependency pulled in by `bun install`)

### Installation

```bash
# Navigate to extension directory
cd apps/browser-extension

# Install dependencies
bun install

# Build WASM module (from root)
bun run build:wasm

# Start development server
bun run dev
```

### Loading the Extension

#### Chrome
1. Navigate to `chrome://extensions/`
2. Enable "Developer mode"
3. Click "Load unpacked"
4. Select `.output/chrome-mv3` directory

#### Firefox
1. Navigate to `about:debugging`
2. Click "This Firefox"
3. Click "Load Temporary Add-on"
4. Select `.output/firefox-mv2/manifest.json`

## Architecture Overview

```
┌──────────────────────────────────────────────────┐
│                   Web Page                        │
│    (dApp — discovers us via EIP-6963, talks to   │
│     window.starlabEthereum, NOT window.ethereum) │
└────────────────────┬─────────────────────────────┘
                     │
              Content Script
         (injects provider, proxies RPC)
                     │
┌────────────────────┼─────────────────────────────┐
│   Extension        │                             │
├────────────────────┼─────────────────────────────┤
│  Popup UI     Background Worker    Offscreen     │
│  (Svelte 5   (Service Worker —     Document      │
│   legacy)    StateManager,       (WebRTC + WASM  │
│              SessionManager,      FROST host)    │
│              WebSocketManager,                   │
│              RpcHandler,                         │
│              OffscreenManager,                   │
│              KeepaliveController)                │
└───────────────────────────────────────────────────┘
```

## Key Features

### Security
- **Threshold Signatures**: t-of-n participants required for signing
- **No Single Point of Failure**: Private keys never exist in complete form
- **Encrypted Storage**: AES-256-GCM encryption for local storage
- **Secure Communication**: WebRTC with DTLS encryption

### Functionality
- **Multi-Chain Support**: Ethereum (secp256k1) and Solana (ed25519)
- **Web3 Provider**: EIP-1193 compatible provider for dApps
- **P2P Communication**: Direct WebRTC connections between participants
- **Keystore Management**: Import/export compatible with the TUI wallet

### User Experience
- **Simple Setup**: One-click wallet creation
- **Visual DKG Flow**: Guided distributed key generation
- **Transaction Preview**: Clear transaction details before signing
- **Account Management**: Multiple wallets and accounts

## Development

### Project Structure

```
apps/browser-extension/
├── src/
│   ├── entrypoints/      # Extension entry points
│   │   ├── background/   # Service worker
│   │   ├── content/      # Content script
│   │   ├── injected/     # Page-context EIP-1193 provider
│   │   ├── offscreen/    # Offscreen document (WebRTC + WASM)
│   │   └── popup/        # Svelte popup UI
│   ├── components/       # Svelte components
│   ├── services/         # AccountService, NetworkService, etc.
│   ├── utils/
│   └── config/           # signal-server.ts + other config
├── tests/                # Bun test suite
├── public/               # Static assets
├── wxt.config.ts         # WXT framework config
└── package.json          # Bun workspace member

# Types and shared schemas live in the workspace `@starlab/types`
# package, NOT under the extension's own `src/`:
packages/@starlab/types/src/
├── messages.ts           # All cross-context message types
├── appstate.ts           # SupportedChain + app state
└── session.ts            # SessionInfo
```

### Key Technologies

- **WXT**: Extension framework for cross-browser development
- **Svelte**: Reactive UI framework for popup interface
- **TypeScript**: Type-safe JavaScript
- **WebAssembly**: High-performance cryptographic operations
- **WebRTC**: Peer-to-peer communication

### Testing

From `apps/browser-extension/` (all Bun test invocations — see
[`docs/testing/TESTING.md`](../../../docs/testing/TESTING.md)):

```bash
bun test                    # full suite
bun run test:watch          # watch mode
bun run test:coverage       # coverage report
bun run test:unit           # tests/services + tests/config
bun run test:integration    # tests/integration
bun run test:webrtc         # tests/entrypoints/offscreen/webrtc.*
bun test tests/services/walletClient.test.ts   # a specific file
```

No `test:e2e` script exists in `package.json` — earlier drafts
of this section listed it. Automated full-mesh E2E is open work
(see `docs/testing/E2E_TEST_IMPLEMENTATION_PLAN.md`).

### Building for Production

```bash
# Build for Chrome (default target)
bun run build

# Build for Firefox
bun run build:firefox

# Build for Edge
bun run build:edge
```

## API Integration

### For dApp Developers

dApps discover the wallet via **EIP-6963**, not by checking
`window.ethereum` directly (see
[`docs/implementation/EIP-6963-IMPLEMENTATION.md`](../../../docs/implementation/EIP-6963-IMPLEMENTATION.md)
— fixed in 6ecd63a). The extension injects ONLY as
`window.starlabEthereum`, never `window.ethereum`, to coexist with
other wallet extensions.

```javascript
// EIP-6963 discovery (recommended)
window.addEventListener("eip6963:announceProvider", (event) => {
  const { info, provider } = event.detail;
  if (info.rdns === "org.starlab.wallet") {
    // Found us; use `provider` for RPC
    const accounts = await provider.request({ method: "eth_requestAccounts" });
  }
});
window.dispatchEvent(new Event("eip6963:requestProvider"));

// Or, if you know the extension is installed:
const provider = window.starlabEthereum;
const accounts = await provider.request({ method: "eth_requestAccounts" });
```

Real RPC method list backing the provider: see the injected
provider's method switch in
`src/entrypoints/injected/index.ts` (EIP-1193 methods including
`eth_requestAccounts`, `eth_accounts`, `eth_chainId`,
`personal_sign`, `eth_sendTransaction`, …).

### Extension APIs

Internal `chrome.runtime.sendMessage` types are consts in
`MESSAGE_TYPES` (see the dispatch table in
`src/entrypoints/background/messageHandlers.ts`). Real type names
differ from earlier drafts:

```typescript
// Earlier drafts of this doc showed:
//   type: 'CREATE_WALLET', payload: { name, threshold, participants }
// Real type: CREATE_DKG_WALLET (see MESSAGE_TYPES enum in
// packages/@starlab/types/src/messages.ts:314 — earlier drafts
// of this note cited :303, which was stale; the block has grown
// as MESSAGE_TYPES entries accumulated)

chrome.runtime.sendMessage({
  type: "CREATE_DKG_WALLET",  // real MESSAGE_TYPES.CREATE_DKG_WALLET
  session_id: "my-wallet",
  total: 3,
  threshold: 2,
  participants: ["alice", "bob", "charlie"],
});
```

For the full list of real message types see the tech doc's API
Reference section (fixed in c9417e5).

## Troubleshooting

### Common Issues

#### Extension not loading
- Ensure developer mode is enabled
- Check console for errors
- Verify WASM module is built

#### WebRTC connection failures
- Check firewall settings
- Verify STUN/TURN servers
- Enable WebRTC debugging in chrome://webrtc-internals

#### Transaction signing errors
- Verify all participants are online
- Check threshold requirements
- Review gas settings

## Resources

- [Main Project Documentation](../../../docs/README.md)
- [**ARCHITECTURE.md**](ARCHITECTURE.md) — ~1000-line deep
  technical reference (verified `wc -l`, earlier drafts said
  700 before this sweep expanded it): 4-context runtime
  architecture (popup / background SW / offscreen / content),
  message system + flow patterns, WebSocket + WebRTC
  implementation details, API reference, error recovery.
- [UI Documentation](ui/)
- [Chrome Extension Docs](https://developer.chrome.com/docs/extensions/mv3/)
- [WXT Framework](https://wxt.dev/)

For the FROST-over-WebRTC wire protocol specifically, the top-of-repo
`CLAUDE.md` (§ "Browser extension: threshold signing architecture")
has a condensed version covering the critical sequence.

## Support

For issues and questions:
- [GitHub Issues](https://github.com/hecoinfo/starlab-mpc/issues)
- [Documentation](../../../docs/)