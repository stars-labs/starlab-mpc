#!/usr/bin/env bash
#
# smoke-dkg.sh — 3-node FROST DKG end-to-end smoke test.
#
# Drives (or observes) three `starlab-tui` instances and asserts they
# converge on the same group verifying key. Designed to give a PASS/FAIL
# verdict in <60s so iterations on the Elm/DKG code get a tight loop.
#
# Modes:
#   --manual (default)  — You open 3 terminals and drive each TUI by hand.
#                         The script tails the log files and prints PASS/FAIL
#                         when all 3 converge. Useful while keystroke
#                         sequences are still changing.
#   --tmux              — Script spawns a 3-pane tmux session and runs one
#                         TUI per pane. You still drive the UI; the script
#                         handles layout + cleanup. (Auto-keystroke-injection
#                         is a TODO — punt until the CreateWallet flow is
#                         stable across stages.)
#
# The script never kills your nodes on success — it just declares victory
# and exits. On failure/timeout it prints the last 30 lines of each log
# and exits 1. Cleanup of the tmux session on --tmux is your
# responsibility (`tmux kill-session -t mpc-smoke`) so you can inspect
# panes after a failure.
#
# Requires: bash, grep, awk, tail, timeout. Optional: tmux (for --tmux).
set -u
set -o pipefail

# Intentionally not `set -e` — the script does explicit error checks and
# benefits from continuing through individual failures to print diagnostics.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT" || {
    echo "FAIL: unable to cd into repo root" >&2
    exit 1
}

MODE="manual"
TIMEOUT_SECS="${SMOKE_TIMEOUT:-60}"
DEVICE_IDS=("mpc-1" "mpc-2" "mpc-3")

while [[ $# -gt 0 ]]; do
    case "$1" in
        --manual) MODE="manual"; shift ;;
        --tmux)   MODE="tmux"; shift ;;
        --timeout) TIMEOUT_SECS="$2"; shift 2 ;;
        -h|--help)
            sed -n '/^# smoke-dkg.sh/,/^$/p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

# ---------------------------------------------------------------
# Prereqs
# ---------------------------------------------------------------
BINARY="$REPO_ROOT/target/debug/starlab-tui"
if [[ ! -x "$BINARY" ]]; then
    echo ">> Binary not found, building…"
    cargo build -p starlab-client --bin starlab-tui 2>&1 | tail -5
    [[ -x "$BINARY" ]] || { echo "FAIL: build produced no binary" >&2; exit 1; }
fi

if [[ "$MODE" == "tmux" ]] && ! command -v tmux >/dev/null; then
    echo "FAIL: tmux not installed (required for --tmux mode)" >&2
    exit 1
fi

# ---------------------------------------------------------------
# Log file setup — truncate so old runs don't false-positive the watcher
# ---------------------------------------------------------------
LOG_FILES=()
for id in "${DEVICE_IDS[@]}"; do
    log="$REPO_ROOT/starlab-mpc-$id.log"
    : > "$log"  # truncate
    LOG_FILES+=("$log")
done

# ---------------------------------------------------------------
# Mode: tmux — spawn 3-pane session
# ---------------------------------------------------------------
if [[ "$MODE" == "tmux" ]]; then
    SESSION="mpc-smoke"
    tmux kill-session -t "$SESSION" 2>/dev/null || true
    tmux new-session -d -s "$SESSION" -x 220 -y 60 \
        "$BINARY --device-id ${DEVICE_IDS[0]}"
    tmux split-window -h -t "$SESSION" "$BINARY --device-id ${DEVICE_IDS[1]}"
    tmux split-window -v -t "$SESSION" "$BINARY --device-id ${DEVICE_IDS[2]}"
    tmux select-layout -t "$SESSION" tiled

    echo ">> tmux session '$SESSION' started with 3 panes."
    echo "   Attach: tmux attach -t $SESSION"
    echo "   Drive the creator (mpc-1) through CreateWallet → ThresholdConfig → …"
    echo "   Drive mpc-2/mpc-3 through JoinSession."
    echo "   This watcher will wait up to ${TIMEOUT_SECS}s for DKG to converge."
    echo
fi

if [[ "$MODE" == "manual" ]]; then
    cat <<EOF
>> Manual mode. Open 3 terminals and in each run (from this repo root):

    target/debug/starlab-tui --device-id mpc-1
    target/debug/starlab-tui --device-id mpc-2
    target/debug/starlab-tui --device-id mpc-3

   Drive mpc-1 through CreateWallet → ThresholdConfig → …; drive mpc-2 and
   mpc-3 through JoinSession. This watcher will wait up to ${TIMEOUT_SECS}s
   for DKG to converge before declaring FAIL.

EOF
fi

# ---------------------------------------------------------------
# Log watcher — poll the three log files for the success markers.
# Returns 0 on success (all 3 agree), 1 on timeout/mismatch.
# ---------------------------------------------------------------
SUCCESS_MARKER="🎉 DKG completed successfully"
# We extract the group verifying key from `DKGKeyGenerated { group_pubkey_hex: "…" }`.
KEY_REGEX='DKGKeyGenerated \{ group_pubkey_hex: "([0-9a-f]+)"'

start=$(date +%s)
declare -A KEYS   # device_id → group key hex

echo ">> Watching: ${LOG_FILES[*]}"
while :; do
    elapsed=$(($(date +%s) - start))
    if (( elapsed > TIMEOUT_SECS )); then
        echo
        echo "FAIL: ${TIMEOUT_SECS}s elapsed without all 3 nodes reporting DKG success." >&2
        echo ">> Last 30 lines per log:" >&2
        for log in "${LOG_FILES[@]}"; do
            echo "===== $(basename "$log") =====" >&2
            tail -30 "$log" >&2
        done
        exit 1
    fi

    all_have_key=true
    for id in "${DEVICE_IDS[@]}"; do
        if [[ -z "${KEYS[$id]:-}" ]]; then
            log="$REPO_ROOT/starlab-mpc-$id.log"
            # Skip if log file still empty (process not writing yet).
            [[ -s "$log" ]] || { all_have_key=false; continue; }
            if grep -q "$SUCCESS_MARKER" "$log"; then
                # Pull the group pubkey out of the DKGKeyGenerated log line.
                key=$(grep -oE "$KEY_REGEX" "$log" | head -1 | \
                      sed -E 's/.*group_pubkey_hex: "([0-9a-f]+)".*/\1/')
                if [[ -n "$key" ]]; then
                    KEYS[$id]="$key"
                    echo "   ✓ $id → ${key:0:16}… (at ${elapsed}s)"
                else
                    all_have_key=false
                fi
            else
                all_have_key=false
            fi
        fi
    done

    if $all_have_key; then
        break
    fi

    sleep 1
done

# ---------------------------------------------------------------
# All 3 nodes have a group key. Assert they agree.
# ---------------------------------------------------------------
mismatches=0
ref_key="${KEYS[${DEVICE_IDS[0]}]}"
for id in "${DEVICE_IDS[@]}"; do
    if [[ "${KEYS[$id]}" != "$ref_key" ]]; then
        echo "FAIL: $id key ${KEYS[$id]} != reference $ref_key" >&2
        ((mismatches++))
    fi
done

if (( mismatches > 0 )); then
    echo "FAIL: $mismatches nodes disagree on group verifying key." >&2
    exit 1
fi

elapsed=$(($(date +%s) - start))
echo
echo "PASS: 3-node DKG converged on $ref_key in ${elapsed}s."
echo
echo "Leftover artifacts:"
echo "  - Per-device logs: starlab-mpc-mpc-{1,2,3}.log"
if [[ "$MODE" == "tmux" ]]; then
    echo "  - tmux session 'mpc-smoke' (kill with: tmux kill-session -t mpc-smoke)"
fi
