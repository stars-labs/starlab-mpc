#!/usr/bin/env bash
# ceremony.sh — run a REAL multi-process MPC ceremony end to end.
#
# This drives the ACTUAL `mpc-wallet-cli` as separate OS processes: a creator
# (`wallet create`) plus N-1 joiners (`session join`) for DKG, then — with
# --sign — an initiator (`sign`) plus a quorum of co-signers (`serve
# --auto-approve`) for threshold signing. Every node is its own process with
# its own on-disk keystore, talking over real WebRTC through a real signal
# server. Nothing is simulated in one address space, so there is nothing
# hard-coded to fake — which is exactly what makes it convincing on stage (and
# a faithful smoke test of the same path a live demo uses).
#
# By default it spins up a LOCAL signal server on loopback, so the whole thing
# is self-contained and needs no network — the "cannot fail" fallback. Point it
# at a hosted server to prove the live path:
#   scripts/demo/ceremony.sh --signal wss://panda.qzz.io --room <strong-id>
#
# Usage:
#   scripts/demo/ceremony.sh [--nodes N] [--threshold T]
#                            [--curve secp256k1|ed25519]
#                            [--sign "message to sign"]
#                            [--signal ws[s]://host[:port]] [--room ID]
#                            [--timeout SECONDS] [--keep]
#
# Exit 0 iff every node agreed on ONE group key (and, with --sign, the quorum
# produced a signature). Prints a JSON summary on success.
set -euo pipefail

NODES=3 THRESH=2 CURVE=secp256k1 SIGN_MSG="" SIGNAL="" ROOM="" TIMEOUT=90 KEEP=0
while [ $# -gt 0 ]; do
  case "$1" in
    --nodes)     NODES="$2"; shift 2 ;;
    --threshold) THRESH="$2"; shift 2 ;;
    --curve)     CURVE="$2"; shift 2 ;;
    --sign)      SIGN_MSG="$2"; shift 2 ;;
    --signal)    SIGNAL="$2"; shift 2 ;;
    --room)      ROOM="$2"; shift 2 ;;
    --timeout)   TIMEOUT="$2"; shift 2 ;;
    --keep)      KEEP=1; shift ;;
    -h|--help)   grep '^#' "$0" | sed 's/^#\s\?//'; exit 0 ;;
    *) echo "ceremony.sh: unknown argument '$1' (try --help)" >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")/../.."
CLI=./target/release/mpc-wallet-cli
SIG_BIN=./target/release/webrtc-signal-server

c_ok()  { printf '\033[32m%s\033[0m\n' "$1"; }
c_bad() { printf '\033[31m%s\033[0m\n' "$1" >&2; }
c_dim() { printf '\033[2m%s\033[0m\n' "$1"; }

# --- build the binaries we need ---
c_dim "▸ building (release)…"
cargo build --release -q -p mpc-wallet-cli
[ -z "$SIGNAL" ] && cargo build --release -q -p webrtc-signal-server

# --- isolated workspace + cleanup ---
WORK="$(mktemp -d "${TMPDIR:-/tmp}/mpc-ceremony.XXXXXX")"
PW="$WORK/password"; printf 'ceremony-demo-password' > "$PW"
PIDS=()
cleanup() {
  for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done
  if [ "$KEEP" = "1" ]; then c_dim "▸ artefacts kept in $WORK"; else rm -rf "$WORK"; fi
}
trap cleanup EXIT INT TERM

# --- signal server (local unless --signal given) ---
if [ -z "$SIGNAL" ]; then
  for _ in 1 2 3 4 5; do
    PORT=$(( (RANDOM % 20000) + 20000 ))
    MPC_SIGNAL_BIND="127.0.0.1:$PORT" "$SIG_BIN" >"$WORK/signal.log" 2>&1 &
    SIG_PID=$!; PIDS+=("$SIG_PID")
    up=0
    for _ in $(seq 1 50); do
      if (exec 3<>"/dev/tcp/127.0.0.1/$PORT") 2>/dev/null; then exec 3>&- 3<&-; up=1; break; fi
      kill -0 "$SIG_PID" 2>/dev/null || break   # server died (port taken) → retry
      sleep 0.1
    done
    [ "$up" = "1" ] && { SIGNAL="ws://127.0.0.1:$PORT"; break; }
    kill "$SIG_PID" 2>/dev/null || true
  done
  [ -z "$SIGNAL" ] && { c_bad "✗ could not start a local signal server"; exit 1; }
  c_dim "▸ local signal server: $SIGNAL"
fi

# --- room: hosted wss:// REQUIRES a strong one; local ws:// does not ---
ROOM_FLAG=()
if [ -n "$ROOM" ]; then
  ROOM_FLAG=(--room "$ROOM")
elif [[ "$SIGNAL" == wss://* ]]; then
  ROOM="ceremony$(date +%s)${RANDOM}${RANDOM}${RANDOM}"; ROOM_FLAG=(--room "$ROOM")
fi

# field <file> <json-key> — pull a string value out of an event line
field() { grep -oE "\"$2\": *\"[^\"]+\"" "$1" 2>/dev/null | head -1 | sed -E 's/.*: *"([^"]+)"/\1/'; }

echo
c_dim "▸ DKG ${THRESH}-of-${NODES} (${CURVE}) over ${SIGNAL} — ${NODES} separate processes"

# --- Phase 1: DKG — creator + (N-1) joiners ---
"$CLI" wallet create --total "$NODES" --threshold "$THRESH" --curve "$CURVE" \
  --device-id 1 --keystore "$WORK/ks-1" --password-file "$PW" \
  --signal-server "$SIGNAL" "${ROOM_FLAG[@]}" --timeout "$TIMEOUT" \
  >"$WORK/out-1" 2>"$WORK/log-1" &
DKG_PIDS=("$!"); PIDS+=("$!")

SID=""
for _ in $(seq 1 $((TIMEOUT * 2))); do
  SID="$(grep -oE 'session id = [0-9a-f-]+' "$WORK/log-1" 2>/dev/null | head -1 | sed 's/session id = //')" || true
  [ -n "$SID" ] && break
  sleep 0.5
done
[ -z "$SID" ] && { c_bad "✗ creator never announced a session"; tail -5 "$WORK/log-1" >&2; exit 1; }
c_dim "  session: $SID"

for i in $(seq 2 "$NODES"); do
  "$CLI" session join --session-id "$SID" --curve "$CURVE" \
    --device-id "$i" --keystore "$WORK/ks-$i" --password-file "$PW" \
    --signal-server "$SIGNAL" "${ROOM_FLAG[@]}" --timeout "$TIMEOUT" \
    >"$WORK/out-$i" 2>"$WORK/log-$i" &
  DKG_PIDS+=("$!"); PIDS+=("$!")
done

for p in "${DKG_PIDS[@]}"; do wait "$p" || true; done

# --- verify every node agreed on ONE group key ---
GROUP=""; ADDR=""; WID=""; agree=1
for i in $(seq 1 "$NODES"); do
  g="$(field "$WORK/out-$i" group_public_key)"
  if [ -z "$g" ]; then c_bad "✗ node $i did not finish DKG"; tail -3 "$WORK/log-$i" >&2; agree=0; continue; fi
  if [ -z "$GROUP" ]; then GROUP="$g"; ADDR="$(field "$WORK/out-1" address)"; WID="$(field "$WORK/out-1" wallet_id)"
  elif [ "$g" != "$GROUP" ]; then c_bad "✗ node $i disagrees on the group key"; agree=0; fi
done
[ "$agree" = "1" ] || { c_bad "✗ DKG FAILED — nodes did not converge"; exit 1; }
c_ok "  ✅ DKG complete — all ${NODES} nodes agree on one group key"

SIG_HEX=""; MSG_HASH=""
# --- Phase 2: threshold signing (optional) ---
if [ -n "$SIGN_MSG" ]; then
  echo
  c_dim "▸ signing \"${SIGN_MSG}\" — initiator + $((THRESH - 1)) co-signer(s) (serve --auto-approve)"
  # `serve` is a JSONL daemon: it reads commands on stdin, connects only when
  # told to, auto-approves the signing request on its own, and exits on EOF. So
  # we feed it a single `connect` command and then HOLD stdin open (the trailing
  # sleep) so it neither sits disconnected nor quits before the ceremony ends.
  for i in $(seq 2 "$THRESH"); do
    "$CLI" serve --auto-approve --approve-password-file "$PW" --curve "$CURVE" \
      --device-id "$i" --keystore "$WORK/ks-$i" \
      --signal-server "$SIGNAL" "${ROOM_FLAG[@]}" \
      >"$WORK/serve-$i.out" 2>"$WORK/serve-$i.log" \
      < <(printf '{"cmd":"connect"}\n'; sleep "$((TIMEOUT + 15))") &
    PIDS+=("$!")
  done
  sleep 2  # let the co-signer daemons connect + replay sessions
  "$CLI" sign --wallet-id "$WID" --message "$SIGN_MSG" --curve "$CURVE" \
    --device-id 1 --keystore "$WORK/ks-1" --password-file "$PW" \
    --signal-server "$SIGNAL" "${ROOM_FLAG[@]}" --timeout "$TIMEOUT" \
    >"$WORK/sign.out" 2>"$WORK/sign.log" || true
  SIG_HEX="$(field "$WORK/sign.out" signature)"
  MSG_HASH="$(field "$WORK/sign.out" message_hash)"
  [ -z "$SIG_HEX" ] && { c_bad "✗ signing FAILED"; tail -5 "$WORK/sign.log" >&2; exit 1; }
  c_ok "  ✅ signature produced by a ${THRESH}-of-${NODES} quorum"
fi

# --- summary ---
echo
if [ -n "$SIG_HEX" ]; then
  printf '{\n  "ok": true,\n  "nodes": %s,\n  "threshold": %s,\n  "curve": "%s",\n  "wallet_id": "%s",\n  "address": "%s",\n  "group_public_key": "%s",\n  "message_hash": "%s",\n  "signature": "%s"\n}\n' \
    "$NODES" "$THRESH" "$CURVE" "$WID" "$ADDR" "$GROUP" "$MSG_HASH" "$SIG_HEX"
else
  printf '{\n  "ok": true,\n  "nodes": %s,\n  "threshold": %s,\n  "curve": "%s",\n  "wallet_id": "%s",\n  "address": "%s",\n  "group_public_key": "%s"\n}\n' \
    "$NODES" "$THRESH" "$CURVE" "$WID" "$ADDR" "$GROUP"
fi
