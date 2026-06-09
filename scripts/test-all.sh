#!/usr/bin/env bash
# Run tests for all packages in the monorepo

set -e

echo "🧪 Testing MPC Wallet Monorepo..."

# Test browser extension
echo "🌐 Testing browser extension..."
cd apps/browser-extension
bun test
cd ../..

# Test the Rust workspace — cli + starlab-client + frost-core + signal-server.
#
# `--lib --tests` covers both the per-crate unit tests (67 in
# starlab-client::lib) AND the separate integration-test binaries under
# apps/tui/tests/ (component_rendering.rs: 13 tests;
# update_transitions.rs: 88 tests). Without `--tests` those 101
# tests get silently skipped.
echo "🦀 Testing Rust workspace..."
cargo test --workspace --lib --tests

echo "✅ All tests complete!"