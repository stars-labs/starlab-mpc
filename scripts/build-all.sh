#!/bin/bash
# Build all packages in the monorepo

set -e

echo "🔨 Building MPC Wallet Monorepo..."

# Build WASM package first
echo "📦 Building @stars-labs/core-wasm..."
cd packages/@starlab/core-wasm
bun run build
cd ../../..

# Build TypeScript types package
echo "📦 Building @stars-labs/types..."
cd packages/@starlab/types
bun run build
cd ../../..

# Note: `@stars-labs/utils` used to be listed here but the package
# was never created in the monorepo transform; the previous script
# would error out at `cd packages/@starlab/utils`.

# Build the Rust workspace (engine + cli + tui + signal-server; the GUI
# products live in their own repos).
echo "🦀 Building Rust workspace (cli + starlab-client + frost-core + signal-server)..."
cargo build --workspace

echo "✅ Build complete!"