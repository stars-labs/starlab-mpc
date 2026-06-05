#!/usr/bin/env bash
# demo-local.sh — single-laptop fallback / rehearsal. Runs a LOCAL signal
# server plus N TUI nodes in a tmux grid, each with a UNIQUE device id, all
# pointed at the local server. It looks and behaves like a multi-party demo
# but depends on nothing but this one machine — your safety net if the live
# multi-device run wobbles on the network.
#
# Usage:
#   scripts/demo/demo-local.sh            # 3 nodes: alice, bob, carol
#   scripts/demo/demo-local.sh 5          # 5 nodes
#   NUCLEAR=1 scripts/demo/demo-local.sh  # skip the UI; just prove crypto via `simulate`
#
# In the tmux grid: alice creates a 2-of-3 wallet, bob + carol join, then any
# two sign. Detach with Ctrl-b d; kill with `tmux kill-session -t mpcdemo`.
set -euo pipefail
cd "$(dirname "$0")/../.."

N="${1:-3}"
PORT="${PORT:-9000}"
URL="ws://127.0.0.1:${PORT}"
NAMES=(alice bob carol dave erin frank)

# Nuclear fallback: no UI, no network — prove the whole ceremony in ~5s.
if [ "${NUCLEAR:-0}" = "1" ]; then
  echo "NUCLEAR fallback: full ${2:-2}-of-${N} DKG + signing, self-contained:"
  cargo run --release --quiet -p mpc-wallet-cli -- \
    simulate --nodes "$N" --threshold "${2:-2}" --sign "live investor demo"
  exit $?
fi

command -v tmux >/dev/null || { echo "tmux not found — use NUCLEAR=1 $0"; exit 1; }

echo "Building release binaries (tui + signal server)…"
cargo build --release --quiet -p tui-node -p webrtc-signal-server

SESSION=mpcdemo
tmux kill-session -t "$SESSION" 2>/dev/null || true

# Pane 0: the local signal server.
tmux new-session -d -s "$SESSION" -n demo \
  "MPC_SIGNAL_BIND=127.0.0.1:${PORT} cargo run --release --quiet -p webrtc-signal-server; read"
sleep 2  # let it bind before nodes dial

# One pane per node, each with a UNIQUE --device-id (duplicate ids collide on
# the server and break the mesh — the #1 manual-demo footgun).
for i in $(seq 0 $((N-1))); do
  dev="${NAMES[$i]:-node$i}"
  tmux split-window -t "$SESSION" \
    "cargo run --release --quiet --bin mpc-wallet-tui -p tui-node -- --device-id ${dev} --signal-server ${URL}; read"
  tmux select-layout -t "$SESSION" tiled
done

cat <<EOF

  Local demo up: 1 signal server + ${N} TUI nodes (${NAMES[*]:0:$N}) on ${URL}
  Flow:  alice → Create Wallet (2-of-${N}) → bob & carol → Join → all show the
         same address → alice Sign → bob Approve → signature.
  Attach:  tmux attach -t ${SESSION}      Detach: Ctrl-b d
  Kill:    tmux kill-session -t ${SESSION}
EOF
tmux attach -t "$SESSION"
