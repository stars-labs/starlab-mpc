#!/usr/bin/env bash
# preflight.sh — investor-demo health check. Run this 10 minutes before the
# demo (and once on each device). It proves the WHOLE MPC stack — DKG +
# threshold signing + the cryptography — works end to end in seconds, using
# the SAME real path the demo uses: every node is its own `mpc-wallet-cli`
# process with its own on-disk keystore, talking over real WebRTC through a
# real signal server (a local one for the self-contained checks). Nothing is
# faked in a single process. If this is green, the stack is healthy; if it's
# red, you find out BEFORE you're in front of anyone.
#
# It also checks reachability of the live signal server you'll demo against,
# and runs one real ceremony THROUGH it.
#
# Usage:
#   scripts/demo/preflight.sh                 # default checks
#   SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh
set -euo pipefail

cd "$(dirname "$0")/../.."

SIGNAL="${SIGNAL:-wss://panda.qzz.io}"
CEREMONY="scripts/demo/ceremony.sh"
PASS=0; FAIL=0
ok()   { printf '  \033[32m✅ %s\033[0m\n' "$1"; PASS=$((PASS+1)); }
bad()  { printf '  \033[31m❌ %s\033[0m\n' "$1"; FAIL=$((FAIL+1)); }
hdr()  { printf '\n\033[1m%s\033[0m\n' "$1"; }

hdr "0. Build the binaries (release)"
if cargo build --release --quiet -p mpc-wallet-cli -p webrtc-signal-server 2>/tmp/preflight_build.log; then
  ok "mpc-wallet-cli + signal server built"
else
  bad "build failed — see /tmp/preflight_build.log"; exit 1
fi

# Each ceremony spins a LOCAL signal server, runs a full N-process DKG (each
# node a separate mpc-wallet-cli process) and, with --sign, a real threshold
# signing — exiting 0 only if every node agreed and the signature was produced.
hdr "1. Online DKG (the core MPC ceremony, real multi-process)"
for spec in "2 2" "2 3" "3 5"; do
  set -- $spec; t=$1; n=$2
  if "$CEREMONY" --nodes "$n" --threshold "$t" --timeout 90 >/dev/null 2>&1; then
    ok "DKG ${t}-of-${n} — all nodes agree on one group key"
  else
    bad "DKG ${t}-of-${n} FAILED"
  fi
done

hdr "2. Threshold signing (sign together, real multi-process)"
for spec in "2 2" "2 3"; do
  set -- $spec; t=$1; n=$2
  if "$CEREMONY" --nodes "$n" --threshold "$t" --curve ed25519 --sign "investor demo" --timeout 120 >/dev/null 2>&1; then
    ok "Sign ${t}-of-${n} — quorum produced a signature for the group key"
  else
    bad "Sign ${t}-of-${n} FAILED"
  fi
done

hdr "3. Live signal server reachability ($SIGNAL)"
host="$(printf '%s' "$SIGNAL" | sed -E 's#^wss?://##; s#[:/].*$##')"
REACHABLE=0
if command -v curl >/dev/null && curl -sS --max-time 8 -o /dev/null "https://${host}" 2>/dev/null; then
  ok "reachable: $host (TLS responds)"; REACHABLE=1
elif ping -c1 -W3 "$host" >/dev/null 2>&1; then
  ok "reachable: $host (ping)"; REACHABLE=1
else
  bad "could NOT reach $host — use the LOCAL signal-server fallback (see runbook)"
fi

# A ping isn't a ceremony. This runs the REAL demo path: a full multi-process
# DKG + threshold signing THROUGH $SIGNAL with a strong room (the hosted worker
# REQUIRES one — #31). If green, the exact thing you'll do on stage works end
# to end; if red, you find out now, not in front of anyone.
hdr "4. Live ceremony through the server (real multi-process DKG + sign)"
if [ "$REACHABLE" = "1" ]; then
  ROOM="preflight-$(date +%s)-${RANDOM}${RANDOM}${RANDOM}"
  if "$CEREMONY" --nodes 2 --threshold 2 --curve ed25519 --sign "preflight check" \
        --signal "$SIGNAL" --room "$ROOM" --timeout 90 >/dev/null 2>&1; then
    ok "real 2-of-2 DKG + signing through $SIGNAL (room-scoped)"
  else
    bad "ceremony through $SIGNAL FAILED — the live path is broken; fall back to a local server (rung 1) or the local ceremony (rung 3)"
  fi
else
  printf '  (skipped — server unreachable; the local stack above already passed)\n'
fi

hdr "Summary"
printf '  %d passed, %d failed\n' "$PASS" "$FAIL"
if [ "$FAIL" -eq 0 ]; then
  printf '\033[32m\n  STACK HEALTHY — safe to demo. Keep the fallback ladder handy anyway.\033[0m\n'
  exit 0
else
  printf '\033[31m\n  NOT READY — fix the red items, or fall back (scripts/demo/demo-local.sh).\033[0m\n'
  exit 1
fi
