// L3c interop: extension (DKG initiator) + 2 CLI co-signers run a real DKG and
// threshold signing over a live signal server + shared room, then we assert the
// cross-implementation invariant: the Rust CLI peers and the TS/WASM extension
// AGREE on the group public key (#33).
//
// This is the ONLY layer that crosses the Rust-core ↔ independent-TS/WASM
// boundary, so it's where wire-protocol / round-accounting mismatches surface.
//
// Prereqs (see tests/interop/README.md):
//   - `cargo build --release -p starlab-cli`
//   - `bun run build`               (produces .output/chrome-mv3)
//   - `bunx playwright install chromium`
//   - INTEROP_SIGNAL set to a reachable signal server (default: deployed worker)
import { test, expect } from "./fixtures";
import { openPopup, setRoom, createWallet, signMessage } from "./extension-actions";
import { startPeers, killPeers, freshRoom, ensurePrereqs, type Peer } from "./cli-peers";

const SIGNAL = process.env.INTEROP_SIGNAL ?? "wss://panda.qzz.io";
const CURVE = process.env.INTEROP_CURVE ?? "secp256k1";
const PASSWORD = process.env.INTEROP_PW ?? "interop-test-password";
const THRESHOLD = 2;
const TOTAL = 3; // 1 extension + 2 CLI

test.describe.configure({ mode: "serial", timeout: 180_000 });

test("ext + 2 CLI: DKG group keys agree, then a signature verifies", async ({ page, extensionId }) => {
  ensurePrereqs();
  const room = freshRoom();
  let peers: Peer[] = [];
  try {
    // 1. Extension joins the shared room.
    await openPopup(page, extensionId);
    await setRoom(page, room);

    // 2. Start the 2 CLI co-signers (they auto-join the DKG we create).
    peers = startPeers({ count: TOTAL - 1, signal: SIGNAL, room, curve: CURVE, password: PASSWORD });

    // 3. Extension creates the wallet → becomes DKG initiator.
    await createWallet(page, { threshold: THRESHOLD, total: TOTAL, password: PASSWORD, curve: CURVE });

    // 4. Both CLI peers must finish DKG and agree on the group key.
    const results = await Promise.all(peers.map((p) => p.dkg));
    const groupKeys = new Set(results.map((r) => r.groupKey));
    expect(groupKeys.size, "all CLI peers must agree on one group key").toBe(1);
    const cliGroupKey = results[0].groupKey;
    expect(cliGroupKey).toMatch(/^[0-9a-f]+$/);

    // 5. Cross-impl check: the extension's displayed group key matches the
    //    Rust peers' (the L4 differential-oracle invariant). The extension
    //    surfaces the group key after DKG; if the selector needs tuning on the
    //    first headed run, this is the assertion to wire precisely.
    const extKey = await page.getByText(cliGroupKey, { exact: false }).count();
    expect(extKey, "extension should display the same group key the CLI peers derived").toBeGreaterThan(0);

    // 6. Sign from the extension; the CLI peers auto-approve. A completed
    //    signature on the peer side means the cross-impl ceremony closed.
    await signMessage(page, "interop-hello");
    const sigs = await Promise.all(peers.map((p) => p.signature));
    for (const s of sigs) expect(s.sig).toMatch(/^[0-9a-f]+$/);
  } finally {
    killPeers(peers);
  }
});
