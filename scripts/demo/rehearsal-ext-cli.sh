#!/usr/bin/env bash
# rehearsal-ext-cli.sh — rehearse a live "1 browser extension + 2 CLI" ceremony
# (DKG then threshold signing) over a real signal server + shared room (#30).
#
# Two modes:
#
#   (default)  guided    — start 2 hands-off CLI co-signers that auto-join the
#                          DKG session the EXTENSION creates, then auto-approve
#                          signing. You drive only the extension UI; the script
#                          prints the exact settings + step-by-step and tails the
#                          CLI nodes for ✅ DKG / ✅ signature.
#
#   --cli-only           fully automated 3-party smoke (no browser): a 3-node
#                          `simulate` DKG+sign through the same room. Use this to
#                          prove the server/room/pipeline before a live run, or
#                          in CI.
#
# Config (env):
#   SIGNAL   signal server base   (default wss://panda.qzz.io — the deployed worker)
#   ROOM     shared room id       (default: a fresh uuid; ≥16 chars required by the worker)
#   CURVE    secp256k1 | ed25519  (default secp256k1)
#   THRESHOLD / TOTAL             (default 2 / 3 → 1 ext + 2 CLI)
#   MPC_REHEARSAL_PW             wallet password (prompted if unset; never on argv)
#
# Examples:
#   scripts/demo/rehearsal-ext-cli.sh --cli-only          # automated smoke
#   ROOM=team-alpha-demo-001 scripts/demo/rehearsal-ext-cli.sh   # guided live run
set -uo pipefail
cd "$(dirname "$0")/../.."

MODE="guided"; [ "${1:-}" = "--cli-only" ] && MODE="cli-only"

BASE="${SIGNAL:-wss://panda.qzz.io}"
CURVE="${CURVE:-secp256k1}"
THRESHOLD="${THRESHOLD:-2}"
TOTAL="${TOTAL:-3}"
CLI="./target/release/starlab-cli"
uuid() { python3 -c 'import uuid;print("rehearsal-"+uuid.uuid4().hex)'; }
ROOM="${ROOM:-$(uuid)}"

# Room must satisfy the worker's strong-room rule (≥16 chars, [A-Za-z0-9_-]).
if [ "${#ROOM}" -lt 16 ]; then
  echo "ROOM '$ROOM' is too short (<16 chars); the worker will reject it." >&2
  exit 1
fi

echo "Building CLI (release)…"
cargo build --release --quiet -p starlab-cli || { echo "build failed"; exit 1; }

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
echo
bold "Signal server : $BASE"
bold "Room          : $ROOM"
bold "Ceremony      : ${THRESHOLD}-of-${TOTAL}  curve=${CURVE}"
echo

# ---------------------------------------------------------------------------
if [ "$MODE" = "cli-only" ]; then
  bold "Mode: --cli-only (automated 3-party DKG + sign smoke)"
  MSG="deadbeefdeadbeefdeadbeefdeadbeef"
  OUT=$(timeout 120 "$CLI" simulate \
        --nodes "$TOTAL" --threshold "$THRESHOLD" --curve "$CURVE" \
        --sign "$MSG" --signal-server "$BASE" --room "$ROOM" 2>/dev/null)
  echo "$OUT"
  if echo "$OUT" | python3 -c 'import sys,json;sys.exit(0 if json.load(sys.stdin).get("verified") else 1)' 2>/dev/null; then
    printf '\033[32m✅ cli-only rehearsal PASSED (signature verified)\033[0m\n'
    exit 0
  else
    printf '\033[31m❌ cli-only rehearsal FAILED\033[0m\n'
    exit 1
  fi
fi

# ---------------------------------------------------------------------------
# Guided mode: 2 hands-off CLI co-signers + you drive the extension.
NUM_CLI=$((TOTAL - 1))
bold "Mode: guided ($NUM_CLI CLI co-signers + 1 extension)"

# Password (never on argv): from env or prompted.
if [ -z "${MPC_REHEARSAL_PW:-}" ]; then
  read -r -s -p "Wallet password (shared by all participants): " MPC_REHEARSAL_PW; echo
fi
export MPC_REHEARSAL_PW
[ -n "$MPC_REHEARSAL_PW" ] || { echo "empty password; aborting" >&2; exit 1; }

WORK="$(mktemp -d -t rehearsal.XXXXXX)"
PIDS=()
cleanup() {
  echo; echo "Stopping CLI nodes…"
  for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done
  wait 2>/dev/null
  echo "Logs kept in $WORK"
}
trap cleanup EXIT INT TERM

for i in $(seq 1 "$NUM_CLI"); do
  dev="cli-cosigner-$i"
  ks="$WORK/ks-$i"; mkdir -p "$ks"
  log="$WORK/$dev.log"
  python3 scripts/demo/serve_autojoin.py \
      --device-id "$dev" --keystore "$ks" \
      --signal "$BASE" --room "$ROOM" --curve "$CURVE" --cli "$CLI" \
      >"$log" 2>&1 &
  PIDS+=("$!")
  echo "  started $dev (log: $log)"
done

cat <<RUNBOOK

$(bold "── Extension runbook ──────────────────────────────────────────")
In your browser extension (the 3rd participant):

  1. Open Settings (⚙) → "Signal server room":
        Signal server : $BASE
        Room          : $ROOM
     Click Save. (Both CLI co-signers are already on this room.)

  2. Create a ${THRESHOLD}-of-${TOTAL} wallet on curve ${CURVE}:
        Wallets → Create → threshold=${THRESHOLD}, total=${TOTAL}, curve=${CURVE}
     Use the SAME password you entered here.
     The extension announces a DKG session; the 2 CLI nodes auto-join.
     Watch for "✅ DKG COMPLETE" from both CLI nodes below, and confirm the
     group key / address matches what the extension shows.

  3. Sign a message:
        Wallet → Sign Message → type anything → confirm.
     The CLI co-signers auto-approve. Watch for "✅ SIGNATURE COMPLETE".

$(bold "── Live CLI co-signer output (Ctrl-C to stop) ─────────────────")
RUNBOOK

# Tail all node logs until both DKG + signature are seen, or the user stops.
touch "$WORK"/*.log
tail -n +1 -f "$WORK"/cli-cosigner-*.log &
PIDS+=("$!")

# Wait until every co-signer has reported both completions (best-effort gate).
done_gate() {
  for i in $(seq 1 "$NUM_CLI"); do
    grep -q "DKG COMPLETE" "$WORK/cli-cosigner-$i.log" 2>/dev/null || return 1
    grep -q "SIGNATURE COMPLETE" "$WORK/cli-cosigner-$i.log" 2>/dev/null || return 1
  done
  return 0
}
while ! done_gate; do sleep 2; done
echo
printf '\033[32m✅ Rehearsal complete: all %d CLI co-signers finished DKG + signing.\033[0m\n' "$NUM_CLI"
