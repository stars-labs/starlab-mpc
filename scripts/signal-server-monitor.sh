#!/bin/bash

# Signal Server Health Monitor
# This script continuously monitors the signal server health and connections

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Configuration
DEFAULT_PORT=9000
DEFAULT_HOST="localhost"
REFRESH_INTERVAL=2
MONITOR_LOG="/tmp/signal-server-monitor.log"

# Repo root — scripts/ is at the top level, so the parent of this
# script is the repo root regardless of where it was cloned.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
LOG_DIR="$REPO_ROOT/logs"

# Clear screen and move cursor
clear_screen() { printf "\033c"; }
move_cursor() { printf "\033[%d;%dH" "$1" "$2"; }
hide_cursor() { printf "\033[?25l"; }
show_cursor() { printf "\033[?25h"; }

# Print formatted output
print_header() {
    echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}${CYAN}          WebRTC Signal Server Monitor - Real-time Status             ${NC}"
    echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════════════════════${NC}"
}

# Get server process info
get_server_info() {
    local pid=$(pgrep -f "starlab-signal-server" | head -1)
    if [ ! -z "$pid" ]; then
        echo "$pid"
    else
        echo ""
    fi
}

# Get memory usage
get_memory_info() {
    local pid=$1
    if [ ! -z "$pid" ]; then
        ps -o pid,vsz,rss,pmem,comm -p "$pid" --no-headers 2>/dev/null || echo "N/A"
    else
        echo "N/A"
    fi
}

# Get CPU usage
get_cpu_usage() {
    local pid=$1
    if [ ! -z "$pid" ]; then
        ps -o pid,pcpu -p "$pid" --no-headers 2>/dev/null | awk '{print $2}' || echo "0"
    else
        echo "0"
    fi
}

# Count WebSocket connections
count_connections() {
    local port=$1
    ss -tn state established "( dport = :$port or sport = :$port )" 2>/dev/null | grep -c ESTAB || echo "0"
}

# Get connection details
get_connection_details() {
    local port=$1
    ss -tn state established "( dport = :$port or sport = :$port )" 2>/dev/null | grep ESTAB | head -10
}

# Check if port is listening
check_listening() {
    local port=$1
    if ss -tln | grep -q ":$port "; then
        echo "YES"
    else
        echo "NO"
    fi
}

# Test WebSocket connection
test_websocket() {
    local host=$1
    local port=$2

    # Try to connect with timeout
    timeout 2 bash -c "exec 3<>/dev/tcp/$host/$port" 2>/dev/null
    if [ $? -eq 0 ]; then
        echo "REACHABLE"
    else
        echo "UNREACHABLE"
    fi
}

# Parse log for errors
check_recent_errors() {
    local log_dir="$LOG_DIR"
    if [ -d "$log_dir" ]; then
        local latest_log=$(ls -t "$log_dir"/signal-server_*.log 2>/dev/null | head -1)
        if [ ! -z "$latest_log" ]; then
            tail -100 "$latest_log" 2>/dev/null | grep -i "error\|panic\|fatal" | tail -5
        fi
    fi
}

# Monitor main loop
monitor_loop() {
    local host=${1:-$DEFAULT_HOST}
    local port=${2:-$DEFAULT_PORT}

    hide_cursor
    trap 'show_cursor; exit' INT TERM

    while true; do
        clear_screen

        # Get current data
        local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
        local pid=$(get_server_info)
        local listening=$(check_listening "$port")
        local reachable=$(test_websocket "$host" "$port")
        local connections=$(count_connections "$port")

        # Header
        print_header
        echo ""

        # Timestamp
        echo -e "${BOLD}Last Update:${NC} $timestamp"
        echo ""

        # Server Status
        echo -e "${BOLD}${BLUE}▶ Server Status${NC}"
        echo -e "  ${CYAN}PID:${NC} $([ ! -z "$pid" ] && echo -e "${GREEN}$pid${NC}" || echo -e "${RED}Not Running${NC}")"
        echo -e "  ${CYAN}Port $port:${NC} $([ "$listening" = "YES" ] && echo -e "${GREEN}LISTENING${NC}" || echo -e "${RED}NOT LISTENING${NC}")"
        echo -e "  ${CYAN}WebSocket:${NC} $([ "$reachable" = "REACHABLE" ] && echo -e "${GREEN}REACHABLE${NC}" || echo -e "${RED}UNREACHABLE${NC}")"
        echo ""

        # Resource Usage
        if [ ! -z "$pid" ]; then
            echo -e "${BOLD}${BLUE}▶ Resource Usage${NC}"
            local mem_info=$(get_memory_info "$pid")
            local cpu_usage=$(get_cpu_usage "$pid")

            if [ "$mem_info" != "N/A" ]; then
                echo -e "  ${CYAN}Memory:${NC}"
                echo "    $(echo "$mem_info" | awk '{printf "RSS: %.1fMB (%.1f%%)", $3/1024, $4}')"
                echo -e "  ${CYAN}CPU:${NC} ${cpu_usage}%"
            fi
            echo ""
        fi

        # Connection Statistics
        echo -e "${BOLD}${BLUE}▶ Connection Statistics${NC}"
        echo -e "  ${CYAN}Active Connections:${NC} $connections"

        if [ "$connections" -gt 0 ]; then
            echo -e "  ${CYAN}Connection Details:${NC}"
            get_connection_details "$port" | while read line; do
                echo "    $line"
            done
        fi
        echo ""

        # Recent Errors
        echo -e "${BOLD}${BLUE}▶ Recent Errors${NC}"
        local errors=$(check_recent_errors)
        if [ ! -z "$errors" ]; then
            echo "$errors" | while read line; do
                echo -e "  ${RED}$line${NC}"
            done
        else
            echo -e "  ${GREEN}No recent errors${NC}"
        fi
        echo ""

        # Status Summary
        echo -e "${BOLD}${BLUE}▶ Health Summary${NC}"
        if [ ! -z "$pid" ] && [ "$listening" = "YES" ] && [ "$reachable" = "REACHABLE" ]; then
            echo -e "  ${GREEN}● Server is healthy and operational${NC}"
        elif [ ! -z "$pid" ] && [ "$listening" = "YES" ]; then
            echo -e "  ${YELLOW}● Server is running but may have connectivity issues${NC}"
        elif [ ! -z "$pid" ]; then
            echo -e "  ${YELLOW}● Server process exists but not listening on port $port${NC}"
        else
            echo -e "  ${RED}● Server is not running${NC}"
        fi

        echo ""
        echo -e "${CYAN}Press Ctrl+C to exit | Refreshing every ${REFRESH_INTERVAL}s...${NC}"

        # Log to file
        {
            echo "[$timestamp] PID=$pid LISTENING=$listening CONNECTIONS=$connections"
        } >> "$MONITOR_LOG"

        sleep "$REFRESH_INTERVAL"
    done
}

# WebSocket test with actual handshake
test_websocket_detailed() {
    local host=$1
    local port=$2

    echo -e "${BOLD}${BLUE}Testing WebSocket Connection to ws://$host:$port${NC}"
    echo ""

    # Test TCP connection
    echo -n "Testing TCP connection... "
    timeout 2 bash -c "exec 3<>/dev/tcp/$host/$port" 2>/dev/null
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}SUCCESS${NC}"
    else
        echo -e "${RED}FAILED${NC}"
        echo "  Server is not reachable on $host:$port"
        return 1
    fi

    # Test with curl if available
    if command -v curl &> /dev/null; then
        echo -n "Testing HTTP upgrade... "
        response=$(curl -s -o /dev/null -w "%{http_code}" \
            -H "Connection: Upgrade" \
            -H "Upgrade: websocket" \
            -H "Sec-WebSocket-Version: 13" \
            -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
            "http://$host:$port" 2>/dev/null)

        if [ "$response" = "101" ] || [ "$response" = "426" ]; then
            echo -e "${GREEN}WebSocket endpoint detected${NC}"
        else
            echo -e "${YELLOW}HTTP $response - May not be a WebSocket endpoint${NC}"
        fi
    fi

    echo ""
    echo -e "${GREEN}✓ Server is running and accessible${NC}"
}

# Watch logs in real-time with filtering
watch_logs() {
    local filter=${1:-""}
    local log_dir="$LOG_DIR"
    local latest_log=$(ls -t "$log_dir"/signal-server_*.log 2>/dev/null | head -1)

    if [ -z "$latest_log" ]; then
        echo -e "${RED}No log files found${NC}"
        return 1
    fi

    echo -e "${BOLD}${CYAN}Watching: $latest_log${NC}"
    echo -e "${CYAN}Filter: ${filter:-none}${NC}"
    echo -e "${CYAN}Press Ctrl+C to stop${NC}"
    echo ""

    if [ -z "$filter" ]; then
        tail -f "$latest_log" | while read line; do
            case "$line" in
                *ERROR*|*error*|*panic*|*fatal*)
                    echo -e "${RED}$line${NC}"
                    ;;
                *WARN*|*warning*)
                    echo -e "${YELLOW}$line${NC}"
                    ;;
                *connected*|*Connected*|*SUCCESS*)
                    echo -e "${GREEN}$line${NC}"
                    ;;
                *INFO*|*info*)
                    echo -e "${BLUE}$line${NC}"
                    ;;
                *)
                    echo "$line"
                    ;;
            esac
        done
    else
        tail -f "$latest_log" | grep -i "$filter" | while read line; do
            echo -e "${YELLOW}$line${NC}"
        done
    fi
}

# Show statistics
show_stats() {
    local port=${1:-$DEFAULT_PORT}

    echo -e "${BOLD}${CYAN}Signal Server Statistics${NC}"
    echo ""

    # Connection stats
    echo -e "${BOLD}Connection Statistics:${NC}"
    echo "  Total connections: $(count_connections $port)"
    echo "  Unique IPs:"
    ss -tn state established "( dport = :$port or sport = :$port )" 2>/dev/null | \
        awk '{print $4}' | cut -d: -f1 | sort -u | while read ip; do
        [ ! -z "$ip" ] && echo "    - $ip"
    done

    # Log analysis
    echo ""
    echo -e "${BOLD}Log Analysis (last 1000 lines):${NC}"
    local log_dir="$LOG_DIR"
    local latest_log=$(ls -t "$log_dir"/signal-server_*.log 2>/dev/null | head -1)

    if [ ! -z "$latest_log" ]; then
        echo "  Connections: $(tail -1000 "$latest_log" 2>/dev/null | grep -c "connected" || echo 0)"
        echo "  Disconnections: $(tail -1000 "$latest_log" 2>/dev/null | grep -c "disconnected" || echo 0)"
        echo "  Errors: $(tail -1000 "$latest_log" 2>/dev/null | grep -ci "error" || echo 0)"
        echo "  Warnings: $(tail -1000 "$latest_log" 2>/dev/null | grep -ci "warn" || echo 0)"
    fi

    # Resource history from monitor log
    if [ -f "$MONITOR_LOG" ]; then
        echo ""
        echo -e "${BOLD}Connection History (last hour):${NC}"
        tail -60 "$MONITOR_LOG" 2>/dev/null | \
            awk -F'CONNECTIONS=' '{print $2}' | \
            awk 'BEGIN {min=999; max=0; sum=0; count=0}
                 {if ($1 > max) max=$1; if ($1 < min) min=$1; sum+=$1; count++}
                 END {if (count > 0) printf "  Min: %d, Max: %d, Avg: %.1f\n", min, max, sum/count}'
    fi
}

# Show help
show_help() {
    echo "Signal Server Monitor"
    echo ""
    echo "Usage: $0 [command] [options]"
    echo ""
    echo "Commands:"
    echo "  monitor [host] [port]     Monitor server health (default: localhost 9000)"
    echo "  test [host] [port]        Test WebSocket connectivity"
    echo "  logs [filter]             Watch logs in real-time with optional filter"
    echo "  stats [port]              Show connection statistics"
    echo "  help                      Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 monitor                # Monitor localhost:9000"
    echo "  $0 monitor 0.0.0.0 8080  # Monitor specific host:port"
    echo "  $0 test localhost 9000    # Test WebSocket connection"
    echo "  $0 logs error             # Watch logs filtering for 'error'"
    echo "  $0 stats                  # Show statistics"
}

# Main script
case "${1:-monitor}" in
    monitor)
        monitor_loop "${2:-$DEFAULT_HOST}" "${3:-$DEFAULT_PORT}"
        ;;
    test)
        test_websocket_detailed "${2:-$DEFAULT_HOST}" "${3:-$DEFAULT_PORT}"
        ;;
    logs)
        watch_logs "$2"
        ;;
    stats)
        show_stats "${2:-$DEFAULT_PORT}"
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo -e "${RED}Unknown command: $1${NC}"
        show_help
        exit 1
        ;;
esac