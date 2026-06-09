#!/usr/bin/env bash
# Run script for MPC Wallet Signal Server

set -euo pipefail

# Configuration
SIGNAL_PORT="${SIGNAL_PORT:-9000}"
BIND_ADDRESS="${BIND_ADDRESS:-0.0.0.0:$SIGNAL_PORT}"
RUST_LOG="${RUST_LOG:-info}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SIGNAL_SERVER_DIR="$PROJECT_ROOT/../signal-server/server"
WORKSPACE_ROOT="$(dirname "$(dirname "$PROJECT_ROOT")")"
BINARY_PATH="$WORKSPACE_ROOT/target/release/starlab-signal-server"

# Check if binary exists, build if not
if [ ! -f "$BINARY_PATH" ]; then
    echo "Signal server binary not found. Building..."
    "$SCRIPT_DIR/build-signal-server.sh"
fi

echo "Starting MPC Wallet Signal Server..."
echo "Bind address: $BIND_ADDRESS"
echo "Log level: $RUST_LOG"

# Export environment variables
export RUST_LOG
export BIND_ADDRESS

# Start the signal server
echo "Starting signal server at ws://$BIND_ADDRESS"
exec "$BINARY_PATH" --bind "$BIND_ADDRESS"