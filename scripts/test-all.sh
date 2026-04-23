#!/usr/bin/env bash
# Run tests for all packages in the monorepo

set -e

echo "🧪 Testing MPC Wallet Monorepo..."

# Test browser extension
echo "🌐 Testing browser extension..."
cd apps/browser-extension
bun test
cd ../..

# Test Rust workspace — tui-node + frost-core + signal-server.
# Exclude native-node: its binary pulls the graphics-stack feature
# set which is inappropriate for a headless test run. (The crate
# still gets `cargo build`'d in build-all.sh on a workstation.)
echo "🦀 Testing Rust workspace..."
cargo test --workspace --lib --exclude mpc-wallet-native

echo "✅ All tests complete!"