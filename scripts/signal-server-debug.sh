#!/bin/bash

# Signal Server Debug Runner Script
# This script helps run the signal server with comprehensive debugging capabilities

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SIGNAL_SERVER_DIR="$PROJECT_ROOT/apps/signal-server/server"
LOG_DIR="$PROJECT_ROOT/logs"
PID_FILE="/tmp/signal-server.pid"
DEFAULT_PORT=9000
DEFAULT_HOST="0.0.0.0"

# Create logs directory if it doesn't exist
mkdir -p "$LOG_DIR"

# Function to print colored messages
print_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }
print_debug() { echo -e "${PURPLE}[DEBUG]${NC} $1"; }

# Function to check if port is in use
check_port() {
    local port=$1
    if ss -tuln | grep -q ":$port "; then
        return 0  # Port is in use
    else
        return 1  # Port is free
    fi
}

# Function to stop existing signal server
stop_server() {
    print_info "Checking for existing signal server..."

    # Check if PID file exists
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            print_warning "Found running signal server (PID: $pid), stopping..."
            kill "$pid"
            sleep 2
            if kill -0 "$pid" 2>/dev/null; then
                print_warning "Server didn't stop gracefully, forcing..."
                kill -9 "$pid"
            fi
            rm -f "$PID_FILE"
            print_success "Signal server stopped"
        else
            print_info "PID file exists but process is not running"
            rm -f "$PID_FILE"
        fi
    fi

    # Also check for any running signal server processes
    local pids=$(pgrep -f "starlab-signal-server" || true)
    if [ ! -z "$pids" ]; then
        print_warning "Found signal server processes: $pids"
        for pid in $pids; do
            print_info "Stopping process $pid..."
            kill "$pid" 2>/dev/null || true
        done
        sleep 2
        # Force kill if still running
        pids=$(pgrep -f "starlab-signal-server" || true)
        if [ ! -z "$pids" ]; then
            for pid in $pids; do
                print_warning "Force stopping process $pid..."
                kill -9 "$pid" 2>/dev/null || true
            done
        fi
    fi
}

# Function to start the signal server
start_server() {
    local port=${1:-$DEFAULT_PORT}
    local host=${2:-$DEFAULT_HOST}
    local log_level=${3:-debug}

    print_info "Starting signal server..."
    print_info "Configuration:"
    echo -e "  ${CYAN}Host:${NC} $host"
    echo -e "  ${CYAN}Port:${NC} $port"
    echo -e "  ${CYAN}Log Level:${NC} $log_level"
    echo -e "  ${CYAN}Log Directory:${NC} $LOG_DIR"

    # Check if port is available
    if check_port "$port"; then
        print_error "Port $port is already in use!"
        echo "Current processes using port $port:"
        ss -tlnp | grep ":$port" || true
        return 1
    fi

    # Build the signal server
    print_info "Building signal server..."
    cd "$SIGNAL_SERVER_DIR"

    if [ "$log_level" = "release" ]; then
        cargo build --release
        BINARY="$PROJECT_ROOT/target/release/starlab-signal-server"
    else
        cargo build
        BINARY="$PROJECT_ROOT/target/debug/starlab-signal-server"
    fi

    if [ ! -f "$BINARY" ]; then
        print_error "Failed to build signal server!"
        return 1
    fi

    # Prepare log files
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local log_file="$LOG_DIR/signal-server_${timestamp}.log"
    local error_log="$LOG_DIR/signal-server_${timestamp}_error.log"

    print_info "Starting server with logging..."
    echo "  Output log: $log_file"
    echo "  Error log: $error_log"

    # Set environment variables for enhanced logging
    export RUST_LOG=${log_level}
    export RUST_BACKTRACE=1

    # Start the server
    nohup "$BINARY" \
        --host "$host" \
        --port "$port" \
        > "$log_file" \
        2> "$error_log" &

    local server_pid=$!
    echo $server_pid > "$PID_FILE"

    print_info "Server started with PID: $server_pid"

    # Wait a moment and check if it's still running
    sleep 2
    if kill -0 "$server_pid" 2>/dev/null; then
        print_success "Signal server is running!"
        echo ""
        echo "Server Details:"
        echo "  PID: $server_pid"
        echo "  WebSocket URL: ws://$host:$port"
        echo "  Logs: $log_file"
        echo ""
        echo "To monitor logs in real-time:"
        echo "  tail -f $log_file"
        echo ""
        echo "To stop the server:"
        echo "  $0 stop"
        return 0
    else
        print_error "Server failed to start! Check logs:"
        echo "  cat $error_log"
        return 1
    fi
}

# Function to show server status
show_status() {
    print_info "Signal Server Status"
    echo ""

    # Check PID file
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            print_success "Signal server is running (PID: $pid)"

            # Show process info
            echo ""
            echo "Process Information:"
            ps -fp "$pid" || true

            # Show memory usage
            echo ""
            echo "Memory Usage:"
            ps -o pid,vsz,rss,comm -p "$pid" || true

            # Show network connections
            echo ""
            echo "Network Connections:"
            ss -tnp | grep "$pid" 2>/dev/null || echo "  (requires root to show process names)"

            # Show recent log entries
            echo ""
            echo "Recent Log Entries:"
            local latest_log=$(ls -t "$LOG_DIR"/signal-server_*.log 2>/dev/null | head -1)
            if [ ! -z "$latest_log" ]; then
                tail -n 10 "$latest_log"
            else
                echo "  No log files found"
            fi
        else
            print_warning "PID file exists but server is not running"
            rm -f "$PID_FILE"
        fi
    else
        print_info "Signal server is not running (no PID file)"
    fi

    # Check if port is in use
    echo ""
    if check_port "$DEFAULT_PORT"; then
        print_warning "Port $DEFAULT_PORT is in use by:"
        ss -tlnp | grep ":$DEFAULT_PORT" || ss -tln | grep ":$DEFAULT_PORT"
    else
        print_info "Port $DEFAULT_PORT is available"
    fi

    # Check for any signal server processes
    echo ""
    local pids=$(pgrep -f "starlab-signal-server" || true)
    if [ ! -z "$pids" ]; then
        print_warning "Found signal server processes: $pids"
    fi
}

# Function to monitor server logs
monitor_logs() {
    print_info "Monitoring signal server logs..."

    local latest_log=$(ls -t "$LOG_DIR"/signal-server_*.log 2>/dev/null | head -1)
    if [ -z "$latest_log" ]; then
        print_error "No log files found in $LOG_DIR"
        return 1
    fi

    print_info "Tailing: $latest_log"
    print_info "Press Ctrl+C to stop monitoring"
    echo ""

    # Use tail with color highlighting for different log levels
    tail -f "$latest_log" | while read line; do
        case "$line" in
            *ERROR*|*error*)
                echo -e "${RED}$line${NC}"
                ;;
            *WARN*|*warning*)
                echo -e "${YELLOW}$line${NC}"
                ;;
            *INFO*|*info*)
                echo -e "${BLUE}$line${NC}"
                ;;
            *DEBUG*|*debug*)
                echo -e "${PURPLE}$line${NC}"
                ;;
            *SUCCESS*|*connected*|*Connected*)
                echo -e "${GREEN}$line${NC}"
                ;;
            *)
                echo "$line"
                ;;
        esac
    done
}

# Function to clean old logs
clean_logs() {
    print_info "Cleaning old signal server logs..."

    # Keep only the last 10 log files
    local count=$(ls -1 "$LOG_DIR"/signal-server_*.log 2>/dev/null | wc -l)
    if [ "$count" -gt 10 ]; then
        local to_delete=$((count - 10))
        print_info "Removing $to_delete old log files..."
        ls -t "$LOG_DIR"/signal-server_*.log | tail -n "$to_delete" | xargs rm -f
        print_success "Cleaned old logs"
    else
        print_info "No cleanup needed (only $count log files)"
    fi
}

# Function to run with verbose debugging
debug_mode() {
    print_info "Starting signal server in debug mode..."

    stop_server

    print_info "Building debug version..."
    cd "$SIGNAL_SERVER_DIR"
    cargo build

    print_info "Starting with maximum verbosity..."
    export RUST_LOG=trace,starlab_signal_server=trace,tokio=debug,tungstenite=debug
    export RUST_BACKTRACE=full

    local timestamp=$(date +%Y%m%d_%H%M%S)
    local debug_log="$LOG_DIR/signal-server_debug_${timestamp}.log"

    print_info "Debug log: $debug_log"
    print_info "Press Ctrl+C to stop"
    echo ""

    "$PROJECT_ROOT/target/debug/starlab-signal-server" 2>&1 | tee "$debug_log"
}

# Function to show help
show_help() {
    echo "Signal Server Debug Runner"
    echo ""
    echo "Usage: $0 [command] [options]"
    echo ""
    echo "Commands:"
    echo "  start [port] [host] [log_level]  Start the signal server (default: 9000 0.0.0.0 debug)"
    echo "  stop                              Stop the running signal server"
    echo "  restart                           Restart the signal server"
    echo "  status                            Show server status and statistics"
    echo "  monitor                           Monitor server logs in real-time"
    echo "  debug                             Run in interactive debug mode"
    echo "  clean                             Clean old log files"
    echo "  help                              Show this help message"
    echo ""
    echo "Log Levels:"
    echo "  error    - Only errors"
    echo "  warn     - Errors and warnings"
    echo "  info     - Informational messages (default)"
    echo "  debug    - Debug messages"
    echo "  trace    - Everything including trace data"
    echo "  release  - Build in release mode"
    echo ""
    echo "Examples:"
    echo "  $0 start                  # Start with defaults"
    echo "  $0 start 8080            # Start on port 8080"
    echo "  $0 start 9000 0.0.0.0 trace  # Start with trace logging"
    echo "  $0 monitor               # Watch logs in real-time"
    echo "  $0 debug                 # Run in debug mode"
}

# Main script logic
case "${1:-help}" in
    start)
        stop_server
        start_server "${2:-$DEFAULT_PORT}" "${3:-$DEFAULT_HOST}" "${4:-debug}"
        ;;
    stop)
        stop_server
        ;;
    restart)
        stop_server
        sleep 1
        start_server "${2:-$DEFAULT_PORT}" "${3:-$DEFAULT_HOST}" "${4:-debug}"
        ;;
    status)
        show_status
        ;;
    monitor)
        monitor_logs
        ;;
    debug)
        debug_mode
        ;;
    clean)
        clean_logs
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        print_error "Unknown command: $1"
        show_help
        exit 1
        ;;
esac