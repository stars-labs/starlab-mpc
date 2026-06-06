#!/usr/bin/env bash
# preflight.sh — investor-demo health check. Run this 10 minutes before the
# demo (and once on each device). It proves the WHOLE MPC stack — DKG +
# threshold signing + the cryptography — works end to end in seconds, fully
# self-contained (embedded signal server, real WebRTC over loopback, real
# FROST). If this is green, the crypto/network stack is healthy; if it's red,
# you find out BEFORE you're in front of anyone.
#
# It also checks reachability of the live signal server you'll demo against.
#
# Usage:
#   scripts/demo/preflight.sh                 # default checks
#   SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh
set -euo pipefail

cd "$(dirname "$0")/../.."

SIGNAL="${SIGNAL:-wss://panda.qzz.io}"
CLI="cargo run --release --quiet -p mpc-wallet-cli --"
PASS=0; FAIL=0
ok()   { printf '  \033[32m✅ %s\033[0m\n' "$1"; PASS=$((PASS+1)); }
bad()  { printf '  \033[31m❌ %s\033[0m\n' "$1"; FAIL=$((FAIL+1)); }
hdr()  { printf '\n\033[1m%s\033[0m\n' "$1"; }

hdr "0. Build the CLI (release)"
if cargo build --release --quiet -p mpc-wallet-cli 2>/tmp/preflight_build.log; then
  ok "mpc-wallet-cli built"
else
  bad "build failed — see /tmp/preflight_build.log"; exit 1
fi

# Each simulate runs a full N-node DKG (+ optional sign) in one process and
# exits 0 only if every node agreed / the signature verified.
hdr "1. Online DKG (the core MPC ceremony)"
for spec in "2 2" "2 3" "3 5"; do
  set -- $spec; t=$1; n=$2
  if $CLI simulate --nodes "$n" --threshold "$t" --timeout 90 >/dev/null 2>&1; then
    ok "DKG ${t}-of-${n} — all nodes agree on one group key"
  else
    bad "DKG ${t}-of-${n} FAILED"
  fi
done

hdr "2. Threshold signing (sign together + verify)"
for spec in "2 2" "2 3"; do
  set -- $spec; t=$1; n=$2
  if $CLI simulate --nodes "$n" --threshold "$t" --sign "investor demo" --timeout 120 >/dev/null 2>&1; then
    ok "Sign ${t}-of-${n} — signature verifies against the group key"
  else
    bad "Sign ${t}-of-${n} FAILED"
  fi
done

hdr "3. Live signal server reachability ($SIGNAL)"
host="$(printf '%s' "$SIGNAL" | sed -E 's#^wss?://##; s#[:/].*$##')"
if command -v curl >/dev/null && curl -sS --max-time 8 -o /dev/null "https://${host}" 2>/dev/null; then
  ok "reachable: $host (TLS responds)"
elif ping -c1 -W3 "$host" >/dev/null 2>&1; then
  ok "reachable: $host (ping)"
else
  bad "could NOT reach $host — use the LOCAL signal-server fallback (see runbook)"
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
