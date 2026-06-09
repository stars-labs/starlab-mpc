#!/bin/bash
# Clean all build artifacts in the monorepo

echo "🧹 Cleaning MPC Wallet Monorepo..."

# Clean root
echo "📁 Cleaning root..."
rm -rf node_modules dist target .wxt coverage

# Clean packages
echo "📦 Cleaning packages..."
rm -rf packages/@starlab/*/node_modules
rm -rf packages/@starlab/*/dist
rm -rf packages/@starlab/*/pkg
rm -rf packages/@starlab/*/target

# Clean apps
echo "📱 Cleaning apps..."
rm -rf apps/*/node_modules
rm -rf apps/*/dist
rm -rf apps/*/.wxt
# Catches apps/{starlab-client,browser-extension}/target
# when those crates were built in isolation. The workspace
# shares a top-level `target/` already wiped above.
rm -rf apps/*/target
# signal-server is a nested workspace (server/ + cloudflare-worker/)
# so needs one extra level.
rm -rf apps/signal-server/*/target

echo "✅ Clean complete!"