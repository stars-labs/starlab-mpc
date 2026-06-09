#!/usr/bin/env bash
# ceremony.sh — run a REAL multi-process MPC ceremony end to end.
#
# This drives the ACTUAL `starlab-cli` as separate OS processes: a creator
# (`wallet create`) plus N-1 joiners (`session join`) for DKG, then —
#   --sign     an initiator (`sign`) + a quorum of co-signers (`serve
#              --auto-approve`) produce a threshold signature;
#   --reshare  the wallet holder (`reshare`) + the retained signers refresh
#              every share — same address, fresh shares, old shares dead.
# Every node is its own process with its own on-disk keystore, talking over real
# WebRTC through a real signal server. Nothing is simulated in one address space,
# so there is nothing hard-coded to fake — which is exactly what makes it
# convincing on stage (and a faithful smoke test of the live demo path).
#
# By default it spins up a LOCAL signal server on loopback, so the whole thing
# is self-contained and needs no network — the "cannot fail" fallback. Point it
# at a hosted server to prove the live path:
#   scripts/demo/ceremony.sh --signal wss://panda.qzz.io --room <strong-id>
#
# Usage:
#   scripts/demo/ceremony.sh [--nodes N] [--threshold T]
#                            [--curve secp256k1|ed25519]
#                            [--sign "message to sign"] [--reshare]
#                            [--signal ws[s]://host[:port]] [--room ID]
#                            [--timeout SECONDS] [--keep]
#
# Exit 0 iff every node agreed on ONE group key (and, with --sign, the quorum
# produced a signature; with --reshare, the refresh preserved the address).
# Prints a JSON summary on success.
set -euo pipefail

NODES=3 THRESH=2 CURVE=secp256k1 SIGN_MSG="" RESHARE=0 SIGNAL="" ROOM="" TIMEOUT=90 KEEP=0
while [ $# -gt 0 ]; do
  case "$1" in
    --nodes)     NODES="$2"; shift 2 ;;
    --threshold) THRESH="$2"; shift 2 ;;
    --curve)     CURVE="$2"; shift 2 ;;
    --sign)      SIGN_MSG="$2"; shift 2 ;;
    --reshare)   RESHARE=1; shift ;;
    --signal)    SIGNAL="$2"; shift 2 ;;
    --room)      ROOM="$2"; shift 2 ;;
    --timeout)   TIMEOUT="$2"; shift 2 ;;
    --keep)      KEEP=1; shift ;;
    -h|--help)   grep '^#' "$0" | sed 's/^#\s\?//'; exit 0 ;;
    *) echo "ceremony.sh: unknown argument '$1' (try --help)" >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")/../.."
CLI=./target/release/starlab-cli
SIG_BIN=./target/release/starlab-signal-server

c_ok()  { printf '\033[32m%s\033[0m\n' "$1"; }
c_bad() { printf '\033[31m%s\033[0m\n' "$1" >&2; }
c_dim() { printf '\033[2m%s\033[0m\n' "$1"; }

# --- build the binaries we need ---
c_dim "▸ building (release)…"
cargo build --release -q -p starlab-cli
[ -z "$SIGNAL" ] && cargo build --release -q -p starlab-signal-server

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

# Co-signer daemons (`serve --auto-approve`) approve BOTH signing and reshare
# requests. `serve` is a JSONL daemon: it connects only when told, then exits on
# stdin EOF — so we feed it one `connect` command and HOLD stdin open with a
# trailing sleep. start_cosigners <indices…> launches one per node index;
# stop_cosigners tears them down so the next phase can reuse those device-ids
# (the signal server rejects a duplicate id, so phases must not overlap).
COSIGNER_PIDS=()
start_cosigners() {
  COSIGNER_PIDS=()
  for ci in "$@"; do
    "$CLI" serve --auto-approve --approve-password-file "$PW" --curve "$CURVE" \
      --device-id "$ci" --keystore "$WORK/ks-$ci" \
      --signal-server "$SIGNAL" "${ROOM_FLAG[@]}" \
      >"$WORK/serve-$ci.out" 2>"$WORK/serve-$ci.log" \
      < <(printf '{"cmd":"connect"}\n'; sleep "$((TIMEOUT + 15))") &
    COSIGNER_PIDS+=("$!"); PIDS+=("$!")
  done
  sleep 2  # let the daemons connect + replay the active session list
}
stop_cosigners() {
  for cp in "${COSIGNER_PIDS[@]:-}"; do kill "$cp" 2>/dev/null || true; done
  COSIGNER_PIDS=()
  sleep 1  # let the server register the disconnects before ids are reused
}

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
# --- Phase 2: threshold signing (optional) — initiator + (T-1) co-signers ---
if [ -n "$SIGN_MSG" ]; then
  echo
  c_dim "▸ signing \"${SIGN_MSG}\" — initiator + $((THRESH - 1)) co-signer(s) (serve --auto-approve)"
  start_cosigners $(seq 2 "$THRESH")
  "$CLI" sign --wallet-id "$WID" --message "$SIGN_MSG" --curve "$CURVE" \
    --device-id 1 --keystore "$WORK/ks-1" --password-file "$PW" \
    --signal-server "$SIGNAL" "${ROOM_FLAG[@]}" --timeout "$TIMEOUT" \
    >"$WORK/sign.out" 2>"$WORK/sign.log" || true
  stop_cosigners
  SIG_HEX="$(field "$WORK/sign.out" signature)"
  MSG_HASH="$(field "$WORK/sign.out" message_hash)"
  [ -z "$SIG_HEX" ] && { c_bad "✗ signing FAILED"; tail -5 "$WORK/sign.log" >&2; exit 1; }
  c_ok "  ✅ signature produced by a ${THRESH}-of-${NODES} quorum"
fi

RESHARE_GROUP=""
# --- Phase 3: share refresh / resharing (optional) — holder + retained signers ---
# Same-set refresh: the wallet holder (node 1) initiates, every other node
# approves via `serve --auto-approve` (which auto-joins the reshare request).
# The group key — and therefore the address — must be UNCHANGED afterwards.
if [ "$RESHARE" = "1" ]; then
  echo
  c_dim "▸ resharing — holder + $((NODES - 1)) retained signer(s) refresh every share (address preserved)"
  start_cosigners $(seq 2 "$NODES")
  "$CLI" reshare --wallet-id "$WID" --curve "$CURVE" \
    --device-id 1 --keystore "$WORK/ks-1" --password-file "$PW" \
    --signal-server "$SIGNAL" "${ROOM_FLAG[@]}" --timeout "$TIMEOUT" \
    >"$WORK/reshare.out" 2>"$WORK/reshare.log" || true
  stop_cosigners
  RESHARE_GROUP="$(field "$WORK/reshare.out" group_public_key)"
  [ -z "$RESHARE_GROUP" ] && { c_bad "✗ reshare FAILED"; tail -5 "$WORK/reshare.log" >&2; exit 1; }
  if [ "$RESHARE_GROUP" != "$GROUP" ]; then
    c_bad "✗ reshare CHANGED the group key ($GROUP → $RESHARE_GROUP) — the address would move!"; exit 1
  fi
  c_ok "  ✅ shares refreshed — same group key, address unchanged"
fi

# --- summary ---
echo
{
  printf '{\n  "ok": true,\n  "nodes": %s,\n  "threshold": %s,\n  "curve": "%s",\n' "$NODES" "$THRESH" "$CURVE"
  printf '  "wallet_id": "%s",\n  "address": "%s",\n  "group_public_key": "%s"' "$WID" "$ADDR" "$GROUP"
  [ -n "$SIG_HEX" ] && printf ',\n  "message_hash": "%s",\n  "signature": "%s"' "$MSG_HASH" "$SIG_HEX"
  [ -n "$RESHARE_GROUP" ] && printf ',\n  "reshared": true,\n  "group_key_preserved": true'
  printf '\n}\n'
}
