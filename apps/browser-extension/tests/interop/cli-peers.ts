// Orchestrate CLI co-signer peers for the L3c interop harness (#33).
//
// Reuses the verified `scripts/demo/serve_autojoin.py` reactive driver: each
// peer spawns a `mpc-wallet-cli serve` node that auto-joins the DKG session the
// EXTENSION creates and auto-approves signing. We parse the driver's
// machine-readable `RESULT <node> dkg_complete <wallet> <addr> <groupkey>` /
// `RESULT <node> signature_complete <hash> <sig>` lines.
import { spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import fs from "node:fs";
import os from "node:os";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");
const DRIVER = path.join(REPO_ROOT, "scripts/demo/serve_autojoin.py");
const CLI_BIN = path.join(REPO_ROOT, "target/release/mpc-wallet-cli");

export type DkgResult = { wallet: string; address: string; groupKey: string };

export type Peer = {
  deviceId: string;
  proc: ChildProcess;
  /** Resolves when this peer reports DKG complete. */
  dkg: Promise<DkgResult>;
  /** Resolves when this peer reports a completed signature. */
  signature: Promise<{ hash: string; sig: string }>;
  kill(): void;
};

/** A fresh strong room id (≥16 chars, the worker's requirement). */
export function freshRoom(): string {
  return "interop-" + crypto.randomBytes(12).toString("hex");
}

export function ensurePrereqs(): void {
  if (!fs.existsSync(CLI_BIN)) {
    throw new Error(
      `CLI binary not found at ${CLI_BIN}. Run \`cargo build --release -p mpc-wallet-cli\`.`,
    );
  }
  if (!fs.existsSync(DRIVER)) {
    throw new Error(`serve_autojoin.py not found at ${DRIVER}.`);
  }
}

/**
 * Spawn `count` CLI co-signer peers on `room`/`signal`, each with an isolated
 * keystore and the shared `password`. They auto-join the extension's DKG.
 */
export function startPeers(opts: {
  count: number;
  signal: string;
  room: string;
  curve: string;
  password: string;
}): Peer[] {
  ensurePrereqs();
  const peers: Peer[] = [];
  for (let i = 1; i <= opts.count; i++) {
    const deviceId = `interop-cli-${i}`;
    const keystore = fs.mkdtempSync(path.join(os.tmpdir(), `interop-ks-${i}-`));
    const pwVar = `MPC_INTEROP_PW_${i}`;

    const proc = spawn(
      "python3",
      [
        DRIVER,
        "--device-id", deviceId,
        "--keystore", keystore,
        "--signal", opts.signal,
        "--room", opts.room,
        "--curve", opts.curve,
        "--cli", CLI_BIN,
        "--pw-var", pwVar,
      ],
      { env: { ...process.env, [pwVar]: opts.password }, stdio: ["ignore", "pipe", "pipe"] },
    );

    let resolveDkg!: (r: DkgResult) => void;
    let resolveSig!: (r: { hash: string; sig: string }) => void;
    const dkg = new Promise<DkgResult>((r) => (resolveDkg = r));
    const signature = new Promise<{ hash: string; sig: string }>((r) => (resolveSig = r));

    let buf = "";
    proc.stdout!.on("data", (d: Buffer) => {
      buf += d.toString();
      let nl: number;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl).trim();
        buf = buf.slice(nl + 1);
        // Mirror to test output for debugging.
        if (line) console.log(line);
        const parts = line.split(/\s+/);
        if (parts[0] === "RESULT" && parts[2] === "dkg_complete") {
          resolveDkg({ wallet: parts[3], address: parts[4], groupKey: parts[5] });
        } else if (parts[0] === "RESULT" && parts[2] === "signature_complete") {
          resolveSig({ hash: parts[3], sig: parts[4] });
        }
      }
    });
    proc.stderr!.on("data", (d: Buffer) => console.error(`[${deviceId}:err] ${d}`));

    peers.push({
      deviceId,
      proc,
      dkg,
      signature,
      kill() {
        proc.kill("SIGTERM");
        try { fs.rmSync(keystore, { recursive: true, force: true }); } catch { /* best effort */ }
      },
    });
  }
  return peers;
}

export function killPeers(peers: Peer[]): void {
  for (const p of peers) p.kill();
}
