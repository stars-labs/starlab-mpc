#!/usr/bin/env bash
# room-test.sh — exhaustive CLI test of the multi-tenant ROOM feature against a
# running signal worker (the room requirement lives in the Cloudflare worker;
# the standalone/embedded server has none, so this must hit a real worker).
#
# Default target is the deployed worker; override for a local `wrangler dev`:
#   SIGNAL=wss://panda.qzz.io        scripts/demo/room-test.sh   # deployed
#   SIGNAL=ws://127.0.0.1:8787       scripts/demo/room-test.sh   # local wrangler dev
#
# Cases:
#   T1 strong room  → full DKG+sign completes
#   T2 no room      → connection rejected
#   T3 weak room    → connection rejected
#   T4 isolation    → two rooms, SAME device-ids, both succeed (per-room namespace)
#   T5 invisibility → a session announced in room A is NOT visible in room B
#                     (and IS visible to a second node in room A)
set -uo pipefail   # NOT -e: several cases expect non-zero exits
cd "$(dirname "$0")/../.."

BASE="${SIGNAL:-wss://panda.qzz.io}"
CLI="./target/release/mpc-wallet-cli"
PASS=0; FAIL=0
ok()  { printf '  \033[32m✅ %s\033[0m\n' "$1"; PASS=$((PASS+1)); }
bad() { printf '  \033[31m❌ %s\033[0m\n' "$1"; FAIL=$((FAIL+1)); }
uuid(){ python3 -c 'import uuid;print(uuid.uuid4())'; }

echo "Building CLI (release)…"; cargo build --release --quiet -p mpc-wallet-cli || { echo build failed; exit 1; }
echo "Target signal server: $BASE"

# --- helpers ---------------------------------------------------------------
_parse() { python3 -c 'import sys,json
try: print("ok" if json.load(sys.stdin).get("verified") else "no")
except Exception: print("no")' 2>/dev/null; }

# One 2-of-2 DKG+sign attempt through $BASE; echo ok/no. Empty room ⇒ no --room.
sim_once() { # $1=room  $2=timeout
  local room="$1" t="${2:-40}" args=(simulate --nodes 2 --threshold 2 --sign x --timeout "$t" --signal-server "$BASE")
  [ -n "$room" ] && args+=(--room "$room")
  timeout "$((t+10))" "$CLI" "${args[@]}" 2>/dev/null | _parse
}

# Positive helper: retry the SAME room up to 3x. A brand-new room is a cold
# Durable Object; the first attempt may exceed the in-process connect/discovery
# timeouts over the internet, but a retry hits a now-warm DO and completes. (For
# hermetic, flake-free runs, point SIGNAL at a local `wrangler dev` instead.)
sim_ok() { # $1=room  $2=timeout
  local room="$1" t="${2:-40}" i
  for i in 1 2 3; do
    [ "$(sim_once "$room" "$t")" = "ok" ] && { echo ok; return 0; }
    sleep 1
  done
  echo no
}

# Drive a serve node over JSONL: connect, run the given commands, linger.
# $1=device $2=room $3=linger_secs $4=outfile  (remaining: JSONL command lines)
serve_drive() {
  local dev="$1" room="$2" linger="$3" out="$4"; shift 4
  local ks; ks="$(mktemp -d)"
  { printf '%s\n' '{"cmd":"connect"}'; sleep 2; for c in "$@"; do printf '%s\n' "$c"; done; sleep "$linger"; } \
    | "$CLI" serve --device-id "$dev" --keystore "$ks" --signal-server "$BASE" --room "$room" --log-level warn \
    > "$out" 2>/dev/null
}

echo; echo "T1 — strong room → DKG+sign completes"
[ "$(sim_ok "$(uuid)" 40)" = "ok" ] && ok "strong room ceremony verified" || bad "strong room ceremony FAILED"

echo; echo "T2 — no room → rejected"
[ "$(sim_once "" 20)" = "ok" ] && bad "no-room UNEXPECTEDLY succeeded" || ok "no room rejected"

echo; echo "T3 — weak room ('acme') → rejected"
[ "$(sim_once "acme" 20)" = "ok" ] && bad "weak room UNEXPECTEDLY succeeded" || ok "weak room rejected"

echo; echo "T4 — isolation: two rooms, identical device-ids, both succeed"
# (simulate uses fixed device-ids sim-node-0/1; a SHARED namespace would reject
#  the duplicate Register and fail at least one.) Run sequentially so each room's
# cold DO can warm via retry without competing for the same wall-clock window.
T4A="$(sim_ok "iso-$(uuid)" 40)"
T4B="$(sim_ok "iso-$(uuid)" 40)"
if [ "$T4A" = "ok" ] && [ "$T4B" = "ok" ]; then
  ok "both rooms completed with identical device-ids (per-room namespace)"
else
  bad "isolation: a same-id run failed ($T4A/$T4B)"
fi

echo; echo "T5 — cross-room session invisibility"
RA="inv-$(uuid)"; RB="inv-$(uuid)"
# A announces a DKG session in room A and lingers.
serve_drive roomtest-a "$RA" 22 /tmp/rt_a.jsonl \
  '{"id":1,"cmd":"create_wallet","name":"inv","threshold":2,"total":2,"password":"pw"}' &
sleep 6  # let A announce
# B (room B) and C (room A) each ask for the session list.
serve_drive roomtest-b "$RB" 5 /tmp/rt_b.jsonl '{"cmd":"list_sessions"}'
serve_drive roomtest-c "$RA" 5 /tmp/rt_c.jsonl '{"cmd":"list_sessions"}'
wait
SID="$(python3 -c 'import sys,json
for l in open("/tmp/rt_a.jsonl"):
    try:
        d=json.loads(l)
        if d.get("event")=="session_announced": print(d["session_id"]); break
    except Exception: pass' 2>/dev/null)"
if [ -z "$SID" ]; then
  bad "T5 setup: node A never announced a session"
else
  grep -q "$SID" /tmp/rt_b.jsonl && bad "room B SAW room A's session (leak!)" || ok "room B cannot see room A's session"
  grep -q "$SID" /tmp/rt_c.jsonl && ok "room A peer sees the session (sanity)" || bad "room A peer did NOT see the session"
fi

echo; echo "=== room test: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ]
