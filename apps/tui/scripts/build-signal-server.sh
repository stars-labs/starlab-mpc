#!/usr/bin/env bash
# Build script for MPC Wallet Signal Server

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SIGNAL_SERVER_DIR="$PROJECT_ROOT/../signal-server/server"

echo "Building MPC Wallet Signal Server..."
echo "Signal server directory: $SIGNAL_SERVER_DIR"

# Check if signal server directory exists
if [ ! -d "$SIGNAL_SERVER_DIR" ]; then
    echo "Error: Signal server directory not found at $SIGNAL_SERVER_DIR"
    exit 1
fi

# Build the signal server
cd "$SIGNAL_SERVER_DIR"
echo "Building signal server in release mode..."
cargo build --release

# Check if binary was created (it's in workspace root target directory)
WORKSPACE_ROOT="$(dirname "$(dirname "$PROJECT_ROOT")")"
BINARY_PATH="$WORKSPACE_ROOT/target/release/starlab-signal-server"
if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Signal server binary not found at $BINARY_PATH"
    exit 1
fi

echo "Signal server built successfully at: $BINARY_PATH"

# Optionally copy to deployment directory
DEPLOY_DIR="/opt/starlab-mpc"
if [ -d "$DEPLOY_DIR" ] && [ -w "$DEPLOY_DIR" ]; then
    echo "Copying signal server to deployment directory..."
    cp "$BINARY_PATH" "$DEPLOY_DIR/signal-server"
    chmod +x "$DEPLOY_DIR/signal-server"
    echo "Signal server deployed to: $DEPLOY_DIR/signal-server"
fi

echo "Build complete!"