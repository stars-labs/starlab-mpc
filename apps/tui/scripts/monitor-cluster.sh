#!/usr/bin/env bash
# Continuous monitoring script for MPC Wallet cluster

set -euo pipefail

# Configuration
MONITOR_INTERVAL="${MONITOR_INTERVAL:-30}"
BASE_DATA_DIR="${BASE_DATA_DIR:-./data}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Monitoring state
CONSECUTIVE_FAILURES=0
MAX_FAILURES=3
LAST_CHECK_TIME=""

# Log file for monitoring
MONITOR_LOG="$BASE_DATA_DIR/monitor.log"

log_message() {
    local message="$1"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] $message" >> "$MONITOR_LOG"
    echo -e "[$timestamp] $message"
}

check_and_alert() {
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    LAST_CHECK_TIME="$timestamp"
    
    # Run health check
    if "$SCRIPT_DIR/health-check.sh" >/dev/null 2>&1; then
        if [ "$CONSECUTIVE_FAILURES" -gt 0 ]; then
            log_message "${GREEN}✓ Cluster recovered after $CONSECUTIVE_FAILURES failures${NC}"
        fi
        CONSECUTIVE_FAILURES=0
        return 0
    else
        CONSECUTIVE_FAILURES=$((CONSECUTIVE_FAILURES + 1))
        log_message "${RED}✗ Health check failed (failure #$CONSECUTIVE_FAILURES)${NC}"
        
        if [ "$CONSECUTIVE_FAILURES" -ge "$MAX_FAILURES" ]; then
            log_message "${RED}⚠ CRITICAL: $CONSECUTIVE_FAILURES consecutive failures detected!${NC}"
            
            # Could trigger alerts here (email, Slack, etc.)
            # send_alert "MPC Wallet cluster has $CONSECUTIVE_FAILURES consecutive failures"
        fi
        
        return 1
    fi
}

show_status_dashboard() {
    clear
    echo -e "${BLUE}=== MPC Wallet Cluster Monitor ===${NC}"
    echo "Monitor started: $(cat "$MONITOR_LOG" 2>/dev/null | head -n 1 | cut -d']' -f1 | cut -c2- || date '+%Y-%m-%d %H:%M:%S')"
    echo "Last check: ${LAST_CHECK_TIME:-Never}"
    echo "Check interval: ${MONITOR_INTERVAL}s"
    echo "Consecutive failures: $CONSECUTIVE_FAILURES/$MAX_FAILURES"
    echo ""
    
    # Show recent health check results
    echo -e "${BLUE}--- Recent Health Check ---${NC}"
    if "$SCRIPT_DIR/health-check.sh"; then
        echo ""
    else
        echo ""
        echo -e "${YELLOW}See detailed logs with: tail -f $MONITOR_LOG${NC}"
    fi
    
    # Show resource usage
    echo -e "${BLUE}--- System Resources ---${NC}"
    echo "Memory usage:"
    ps aux | grep -E "(starlab-mpc|webrtc-signal)" | grep -v grep | awk '{print "  " $11 ": " $4"% RAM, " $3"% CPU"}' || echo "  No processes found"
    
    echo ""
    echo "Disk usage (data directory):"
    if [ -d "$BASE_DATA_DIR" ]; then
        du -sh "$BASE_DATA_DIR" 2>/dev/null | awk '{print "  " $0}' || echo "  Unable to check disk usage"
    fi
    
    echo ""
    echo -e "${BLUE}--- Recent Log Activity ---${NC}"
    echo "Signal server:"
    if [ -f "$BASE_DATA_DIR/signal-server.log" ]; then
        tail -n 2 "$BASE_DATA_DIR/signal-server.log" 2>/dev/null | sed 's/^/  /' || echo "  No recent activity"
    else
        echo "  Log file not found"
    fi
    
    for node in mpc-1 mpc-2 mpc-3; do
        echo "$node:"
        if [ -f "$BASE_DATA_DIR/$node/node.log" ]; then
            tail -n 1 "$BASE_DATA_DIR/$node/node.log" 2>/dev/null | sed 's/^/  /' || echo "  No recent activity"
        else
            echo "  Log file not found"
        fi
    done
    
    echo ""
    echo -e "${YELLOW}Press Ctrl+C to stop monitoring${NC}"
}

# Cleanup function
cleanup() {
    log_message "Monitor stopped by user"
    echo ""
    echo "Monitor stopped. Check $MONITOR_LOG for history."
}

trap cleanup EXIT INT TERM

# Main monitoring loop
main() {
    # Create data directory and log file
    mkdir -p "$BASE_DATA_DIR"
    
    log_message "Cluster monitoring started (interval: ${MONITOR_INTERVAL}s)"
    
    # Initial check
    check_and_alert
    
    while true; do
        show_status_dashboard
        
        # Wait for next check
        sleep "$MONITOR_INTERVAL"
        
        # Perform health check
        check_and_alert
    done
}

# Handle command line arguments
case "${1:-}" in
    --help|-h)
        echo "Usage: $0 [--help|-h]"
        echo ""
        echo "Continuous monitoring script for MPC Wallet cluster"
        echo ""
        echo "Environment variables:"
        echo "  MONITOR_INTERVAL    Check interval in seconds (default: 30)"
        echo "  BASE_DATA_DIR       Data directory path (default: ./data)"
        echo ""
        echo "Options:"
        echo "  --help, -h          Show this help message"
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac