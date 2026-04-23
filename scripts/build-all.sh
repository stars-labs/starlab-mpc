#!/bin/bash
# Build all packages in the monorepo

set -e

echo "🔨 Building MPC Wallet Monorepo..."

# Build WASM package first
echo "📦 Building @mpc-wallet/core-wasm..."
cd packages/@mpc-wallet/core-wasm
bun run build
cd ../../..

# Build TypeScript types package
echo "📦 Building @mpc-wallet/types..."
cd packages/@mpc-wallet/types
bun run build
cd ../../..

# Note: `@mpc-wallet/utils` used to be listed here but the package
# was never created in the monorepo transform; the previous script
# would error out at `cd packages/@mpc-wallet/utils`.

# Build browser extension
echo "🌐 Building browser extension..."
cd apps/browser-extension
bun run build
cd ../..

# Build Rust workspace members (excluding native-node — optional
# GUI target that's OK to skip in CI-style "build everything" runs).
echo "🦀 Building Rust workspace (tui-node + frost-core + signal-server)..."
cargo build --workspace --exclude mpc-wallet-native

echo "✅ Build complete!"