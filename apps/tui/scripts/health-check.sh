#!/usr/bin/env bash
# Health check script for MPC Wallet cluster

set -euo pipefail

# Configuration
SIGNAL_SERVER_URL="${SIGNAL_SERVER_URL:-http://localhost:9000}"
BASE_DATA_DIR="${BASE_DATA_DIR:-./data}"
NODES=("mpc-1" "mpc-2" "mpc-3")

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Health check functions
check_signal_server() {
    echo "Checking signal server..."
    
    if curl -s "$SIGNAL_SERVER_URL/health" >/dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} Signal server is healthy"
        return 0
    else
        echo -e "  ${RED}✗${NC} Signal server is not responding"
        return 1
    fi
}

check_node_process() {
    local node="$1"
    local log_file="$BASE_DATA_DIR/$node/node.log"
    
    echo "Checking node: $node"
    
    # Check if log file exists
    if [ ! -f "$log_file" ]; then
        echo -e "  ${RED}✗${NC} Log file not found: $log_file"
        return 1
    fi
    
    # Check if process is running (look for recent log entries)
    if tail -n 5 "$log_file" 2>/dev/null | grep -q "$(date +%Y-%m-%d)" 2>/dev/null || \
       tail -n 5 "$log_file" 2>/dev/null | grep -q "running" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Node $node appears to be running"
        return 0
    else
        echo -e "  ${YELLOW}?${NC} Node $node status unclear (check logs)"
        return 1
    fi
}

check_connectivity() {
    echo "Checking cluster connectivity..."
    
    # Try to get session information from signal server
    if curl -s "$SIGNAL_SERVER_URL/sessions" >/dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} Can query sessions from signal server"
        
        # Show active sessions
        sessions=$(curl -s "$SIGNAL_SERVER_URL/sessions" | jq '.sessions | length' 2>/dev/null || echo "unknown")
        echo "  Active sessions: $sessions"
        return 0
    else
        echo -e "  ${RED}✗${NC} Cannot query sessions from signal server"
        return 1
    fi
}

show_recent_logs() {
    echo ""
    echo "Recent log entries:"
    
    # Signal server logs
    local signal_log="$BASE_DATA_DIR/signal-server.log"
    if [ -f "$signal_log" ]; then
        echo "--- Signal Server (last 3 lines) ---"
        tail -n 3 "$signal_log" 2>/dev/null | sed 's/^/  /'
    fi
    
    # Node logs
    for node in "${NODES[@]}"; do
        local node_log="$BASE_DATA_DIR/$node/node.log"
        if [ -f "$node_log" ]; then
            echo "--- $node (last 3 lines) ---"
            tail -n 3 "$node_log" 2>/dev/null | sed 's/^/  /'
        fi
    done
}

# Main health check
main() {
    echo "=== MPC Wallet Cluster Health Check ==="
    echo "Timestamp: $(date)"
    echo ""
    
    local overall_health=0
    
    # Check signal server
    if ! check_signal_server; then
        overall_health=1
    fi
    
    echo ""
    
    # Check each node
    for node in "${NODES[@]}"; do
        if ! check_node_process "$node"; then
            overall_health=1
        fi
    done
    
    echo ""
    
    # Check connectivity
    if ! check_connectivity; then
        overall_health=1
    fi
    
    # Show recent logs if verbose mode or there are issues
    if [ "$overall_health" -ne 0 ] || [ "${1:-}" == "--verbose" ] || [ "${1:-}" == "-v" ]; then
        show_recent_logs
    fi
    
    echo ""
    
    if [ "$overall_health" -eq 0 ]; then
        echo -e "${GREEN}✓ Overall cluster health: GOOD${NC}"
    else
        echo -e "${RED}✗ Overall cluster health: ISSUES DETECTED${NC}"
        echo "Run with --verbose for more details"
    fi
    
    return $overall_health
}

# Handle command line arguments
case "${1:-}" in
    --help|-h)
        echo "Usage: $0 [--verbose|-v] [--help|-h]"
        echo ""
        echo "Health check script for MPC Wallet cluster"
        echo ""
        echo "Options:"
        echo "  --verbose, -v    Show detailed logs even when healthy"
        echo "  --help, -h       Show this help message"
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac