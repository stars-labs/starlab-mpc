#!/usr/bin/env python3
"""serve_autojoin.py — drive one `starlab-cli serve` node hands-off.

A reactive JSONL driver for the rehearsal harness (#30). It spawns a CLI
`serve` daemon and:

  - on `ready`            → sends `connect`
  - every POLL seconds    → sends `list_sessions` (server replay)
  - on `session_available`(type=dkg, unseen) → sends `join_session`
  - signing is handled by `serve --auto-approve` itself (it joins the
    signing session and contributes a share), so we don't double-join.

It mirrors `DkgComplete` / `SignatureComplete` / `Error` to its own stdout
with a node prefix so the orchestrator can tail for success.

The wallet password is read from an env var (never argv): it is passed to
`serve` via `--approve-password-env` and inlined into the `join_session`
JSON (which travels over stdin, not the process argv).

Usage:
  MPC_PW_VAR=MPC_REHEARSAL_PW \
  serve_autojoin.py --device-id cli-a --keystore /tmp/ks-a \
      --signal wss://host --room <ROOM> --curve secp256k1
"""
import argparse
import json
import os
import sys
import threading
import time
import subprocess

POLL_SECS = 2.0


def log(node, msg):
    print(f"[{node}] {msg}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--device-id", required=True)
    ap.add_argument("--keystore", required=True)
    ap.add_argument("--signal", required=True)
    ap.add_argument("--room", default="")
    ap.add_argument("--curve", default="secp256k1")
    ap.add_argument("--cli", default="./target/release/starlab-cli")
    ap.add_argument("--pw-var", default=os.environ.get("MPC_PW_VAR", "MPC_REHEARSAL_PW"),
                    help="env var name holding the wallet password")
    args = ap.parse_args()

    node = args.device_id
    password = os.environ.get(args.pw_var, "")
    if not password:
        log(node, f"ERROR: password env ${args.pw_var} is empty")
        sys.exit(2)

    cmd = [
        args.cli, "serve",
        "--device-id", args.device_id,
        "--keystore", args.keystore,
        "--signal-server", args.signal,
        "--curve", args.curve,
        "--auto-approve",
        "--approve-password-env", args.pw_var,
        "--log-level", "warn",
    ]
    if args.room:
        cmd += ["--room", args.room]

    log(node, f"spawning: {' '.join(cmd)}")
    proc = subprocess.Popen(
        cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        text=True, bufsize=1,
    )

    joined = set()
    lock = threading.Lock()
    stop = threading.Event()

    def send(obj):
        with lock:
            try:
                proc.stdin.write(json.dumps(obj) + "\n")
                proc.stdin.flush()
            except (BrokenPipeError, ValueError):
                stop.set()

    def poller():
        # Periodic session replay so we discover the extension's DKG session
        # even if it was announced before we connected.
        while not stop.wait(POLL_SECS):
            send({"cmd": "list_sessions"})

    threading.Thread(target=poller, daemon=True).start()

    try:
        for line in proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue
            kind = ev.get("event")
            if kind == "ready":
                log(node, "ready → connect")
                send({"cmd": "connect"})
            elif kind == "session_available":
                s = ev.get("session", {})
                sid = s.get("session_id", "")
                stype = s.get("type", "")
                if stype == "dkg" and sid and sid not in joined:
                    joined.add(sid)
                    log(node, f"joining DKG session {sid} "
                              f"({s.get('threshold')}/{s.get('total')})")
                    send({"cmd": "join_session", "session_id": sid,
                          "password": password})
            elif kind == "dkg_complete":
                gk = ev.get("group_public_key", "")
                log(node, f"✅ DKG COMPLETE wallet={ev.get('wallet_id')} "
                          f"addr={ev.get('address')} group_key={gk[:16]}…")
                # Machine-parseable line for harnesses (group key is public).
                print(f"RESULT {node} dkg_complete "
                      f"{ev.get('wallet_id')} {ev.get('address')} {gk}",
                      flush=True)
            elif kind == "signature_complete":
                sig = ev.get("signature", "")
                log(node, f"✅ SIGNATURE COMPLETE sig={sig[:24]}…")
                print(f"RESULT {node} signature_complete "
                      f"{ev.get('message_hash','')} {sig}", flush=True)
            elif kind == "signing_request":
                log(node, f"signing request for {ev.get('wallet')} "
                          f"(auto-approve handles it)")
            elif kind == "error":
                log(node, f"⚠ error[{ev.get('code')}]: {ev.get('message')}")
    except KeyboardInterrupt:
        pass
    finally:
        stop.set()
        try:
            send({"cmd": "quit"})
            proc.wait(timeout=5)
        except Exception:
            proc.kill()


if __name__ == "__main__":
    main()
