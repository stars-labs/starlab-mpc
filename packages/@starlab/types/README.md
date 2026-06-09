# @starlab/types

Shared TypeScript type definitions for the MPC Wallet ecosystem.

## Installation

This is a **workspace-only package** — it isn't published to
npm. The monorepo's root `package.json` is marked `"private":
true` and workspace members consume it via

```json
"@starlab/types": "workspace:*"
```

in their own `package.json`, resolved by Bun's workspace
linker (see `apps/browser-extension/package.json:42` for the
reference pattern).

Earlier drafts of this README showed `bun add @starlab/types`
/ `npm install @starlab/types` as install commands; both
would fail for external consumers because the package isn't
on any registry. To use these types outside the monorepo
you would need to copy the source or publish a fork yourself.

## Usage

### Import specific types

```typescript
import { AppState, SessionInfo, DkgState } from '@starlab/types';
```

### Import message types

```typescript
import { 
    PopupToBackgroundMessage,
    BackgroundToOffscreenMessage,
    MESSAGE_TYPES 
} from '@starlab/types';
```

### Import constants and utilities

```typescript
import { 
    INITIAL_APP_STATE,
    MeshStatusType,
    validateSessionProposal 
} from '@starlab/types';
```

## Available Types

### Core Types
- `AppState` - Central application state
- `SessionInfo` - MPC session information
- `DkgState` - Distributed Key Generation states
- `MeshStatus` - WebRTC mesh network status

### Message Types
- `PopupToBackgroundMessage` - Messages from popup to background
- `BackgroundToOffscreenMessage` - Messages to offscreen document
- `WebRTCAppMessage` - Application messages over WebRTC

### Keystore Types
- `KeyShareData` - FROST key share data structure
- `ExtensionWalletMetadata` - Wallet metadata for extension
- `KeystoreBackup` - Backup format for keystores

### Network Types
- `Chain` - Blockchain network configuration
- `Account` - User account information

## Type Organization

Types are organized by domain (10 domain modules + `index.ts`
as the re-export aggregator):
- `account.ts` - Account management types
- `appstate.ts` - Application state types, `SupportedChain`,
  `CURVE_COMPATIBLE_CHAINS`
- `dkg.ts` - DKG protocol types
- `keystore.ts` - Keystore and wallet types
- `mesh.ts` - WebRTC mesh network types, `MeshStatus`
- `messages.ts` - Inter-component message types (the big one —
  `PopupToBackgroundMessage`, `BackgroundToOffscreenMessage`,
  `OffscreenToBackgroundMessage`, `BackgroundToPopupMessage`,
  plus `MESSAGE_TYPES` const + validation helpers)
- `network.ts` - Blockchain network types
- `session.ts` - MPC session types
- `webrtc.ts` - WebRTC communication types
  (`WebRTCAppMessage` with `webrtc_msg_type` tag)
- `websocket.ts` - WebSocket signaling types
- `index.ts` - re-exports everything for the `@starlab/types`
  root import (earlier drafts of this list omitted `index.ts`)

## Development

```bash
# Build the package
bun run build

# Watch mode
bun run dev

# Clean build artifacts
bun run clean
```