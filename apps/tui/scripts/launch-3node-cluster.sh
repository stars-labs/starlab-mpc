#!/usr/bin/env bash
# Launch script for 3-node MPC Wallet cluster

set -euo pipefail

# Configuration
SIGNAL_SERVER_URL="${SIGNAL_SERVER_URL:-ws://localhost:9000}"
SIGNAL_SERVER_PORT="${SIGNAL_SERVER_PORT:-9000}"
RUST_LOG="${RUST_LOG:-info}"
BASE_DATA_DIR="${BASE_DATA_DIR:-./data}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Node configuration
NODES=("mpc-1" "mpc-2" "mpc-3")
NODE_PIDS=()
SIGNAL_SERVER_PID=""

# Cleanup function
cleanup() {
    echo "Shutting down cluster..."
    
    # Kill node processes
    for pid in "${NODE_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            echo "Stopping node with PID: $pid"
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    
    # Kill signal server
    if [ -n "$SIGNAL_SERVER_PID" ] && kill -0 "$SIGNAL_SERVER_PID" 2>/dev/null; then
        echo "Stopping signal server with PID: $SIGNAL_SERVER_PID"
        kill -TERM "$SIGNAL_SERVER_PID" 2>/dev/null || true
    fi
    
    # Wait a bit for graceful shutdown
    sleep 2
    
    # Force kill if still running
    for pid in "${NODE_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            echo "Force killing node with PID: $pid"
            kill -KILL "$pid" 2>/dev/null || true
        fi
    done
    
    if [ -n "$SIGNAL_SERVER_PID" ] && kill -0 "$SIGNAL_SERVER_PID" 2>/dev/null; then
        echo "Force killing signal server with PID: $SIGNAL_SERVER_PID"
        kill -KILL "$SIGNAL_SERVER_PID" 2>/dev/null || true
    fi
    
    echo "Cluster shutdown complete."
}

# Set up signal handlers
trap cleanup EXIT INT TERM

echo "=== MPC Wallet 3-Node Cluster Launcher ==="
echo "Signal server: $SIGNAL_SERVER_URL"
echo "Data directory: $BASE_DATA_DIR"
echo "Log level: $RUST_LOG"

# Create data directories
mkdir -p "$BASE_DATA_DIR"
for node in "${NODES[@]}"; do
    mkdir -p "$BASE_DATA_DIR/$node"
done

# Build components if needed
echo "Building signal server..."
"$SCRIPT_DIR/build-signal-server.sh"

echo "Building TUI node..."
"$SCRIPT_DIR/build-starlab-client.sh"

# Start signal server
echo "Starting signal server on port $SIGNAL_SERVER_PORT..."
RUST_LOG=$RUST_LOG "$SCRIPT_DIR/run-signal-server.sh" > "$BASE_DATA_DIR/signal-server.log" 2>&1 &
SIGNAL_SERVER_PID=$!

echo "Signal server started with PID: $SIGNAL_SERVER_PID"

# Wait for signal server to be ready
echo "Waiting for signal server to be ready..."
for i in {1..30}; do
    if curl -s "http://localhost:$SIGNAL_SERVER_PORT/health" >/dev/null 2>&1; then
        echo "Signal server is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "Signal server failed to start within 30 seconds"
        exit 1
    fi
    sleep 1
done

TUI_BINARY_PATH="$PROJECT_ROOT/target/release/starlab-tui"
if [ ! -f "$TUI_BINARY_PATH" ]; then
    echo "Error: TUI binary not found at $TUI_BINARY_PATH." >&2
    echo "Run ./scripts/build-starlab-client.sh first." >&2
    exit 1
fi

echo "Using TUI binary: $TUI_BINARY_PATH"

# Start MPC nodes
for node in "${NODES[@]}"; do
    echo "Starting node: $node"
    
    NODE_DATA_DIR="$BASE_DATA_DIR/$node"
    LOG_FILE="$NODE_DATA_DIR/node.log"
    
    # Set environment variables for the node
    env RUST_LOG=$RUST_LOG \
        DEVICE_ID="$node" \
        DATA_DIR="$NODE_DATA_DIR" \
        "$TUI_BINARY_PATH" \
        --signal-server "$SIGNAL_SERVER_URL" \
        --device-id "$node" \
        > "$LOG_FILE" 2>&1 &
    
    NODE_PID=$!
    NODE_PIDS+=("$NODE_PID")
    echo "Node $node started with PID: $NODE_PID (log: $LOG_FILE)"
    
    # Small delay between node starts
    sleep 1
done

echo ""
echo "=== Cluster Status ==="
echo "Signal server PID: $SIGNAL_SERVER_PID"
echo "Node PIDs: ${NODE_PIDS[*]}"
echo ""
echo "=== Monitoring Cluster ==="
echo "Signal server URL: http://localhost:$SIGNAL_SERVER_PORT"
echo "Log files:"
echo "  Signal server: $BASE_DATA_DIR/signal-server.log"
for i in "${!NODES[@]}"; do
    node="${NODES[$i]}"
    echo "  Node $node: $BASE_DATA_DIR/$node/node.log"
done

echo ""
echo "Press Ctrl+C to shutdown the cluster..."

# Monitor processes
while true; do
    # Check if signal server is still running
    if ! kill -0 "$SIGNAL_SERVER_PID" 2>/dev/null; then
        echo "Signal server died unexpectedly!"
        exit 1
    fi
    
    # Check if nodes are still running
    for i in "${!NODE_PIDS[@]}"; do
        pid="${NODE_PIDS[$i]}"
        node="${NODES[$i]}"
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "Node $node (PID: $pid) died unexpectedly!"
            exit 1
        fi
    done
    
    sleep 5
done