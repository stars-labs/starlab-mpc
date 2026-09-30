// WASM-level test of the threshold-ECDSA bindings (the contract the browser
// extension consumes): a 2-of-3 aux info + keygen + signing through the real
// pkg, parties driven in-process over a shuffled network, fixture primes
// from starlab-core. Prints wasm timings.
//
//   bun run build:wasm && bun test packages/@starlab/core-wasm/tests
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  initSync,
  EcdsaAuxInfo,
  EcdsaKeygen,
  EcdsaSigning,
  ecdsa_key_share_from_parts,
  ecdsa_share_info,
  ecdsa_ethereum_address,
  ecdsa_recover_ethereum_address,
  ecdsa_execution_id,
  ecdsa_payload_execution_id,
  ecdsa_primes_from_parts,
  derive_account_addresses,
} from "../pkg/starlab_core_wasm.js";

const pkgDir = join(import.meta.dir, "../pkg");
initSync({ module: readFileSync(join(pkgDir, "starlab_core_wasm_bg.wasm")) });

const primeSets: { primes: unknown[] }[] = JSON.parse(
  readFileSync(join(import.meta.dir, "../../core/src/ecdsa/testdata/primes.json"), "utf8"),
);

type Out = { recipient: "broadcast" | number; payload: Uint8Array };
interface Ceremony {
  receive(sender: number, payload: Uint8Array): void;
  take_outgoing(): Out[];
  proceed(): string | undefined;
  parties(): Uint16Array;
  index(): number;
}

/** Drive ceremonies to completion; deterministic shuffle of deliveries. */
function run(parties: Ceremony[]): string[] {
  const results: (string | undefined)[] = parties.map(() => undefined);
  const byIndex = new Map(parties.map((c, i) => [c.index(), i]));
  let inFlight: [number, number, Uint8Array][] = [];
  let seed = 7;
  const next = () => (seed = (seed * 1103515245 + 12345) % 2 ** 31);
  const step = (i: number) => {
    const c = parties[i];
    const out = c.proceed();
    if (out !== undefined) results[i] = out;
    for (const m of c.take_outgoing()) {
      const to =
        m.recipient === "broadcast"
          ? [...c.parties()].filter((p) => p !== c.index())
          : [m.recipient];
      for (const t of to) inFlight.push([c.index(), t, m.payload]);
    }
  };
  parties.forEach((_, i) => step(i));
  while (inFlight.length > 0) {
    const k = next() % inFlight.length;
    const [from, to, payload] = inFlight.splice(k, 1)[0];
    const i = byIndex.get(to)!;
    if (results[i] !== undefined) continue;
    parties[i].receive(from, payload);
    step(i);
  }
  expect(results.every((r) => r !== undefined)).toBe(true);
  return results as string[];
}

const ids = ["dev-a", "dev-b", "dev-c"];
const primes = primeSets.map((s) => ecdsa_primes_from_parts(JSON.stringify(s.primes)));

describe("core-wasm threshold ECDSA", () => {
  let shares: string[] = [];

  test(
    "2-of-3 aux info + keygen yields shares with one Ethereum address",
    () => {
      let t = performance.now();
      const aux = run(ids.map((_, i) => new EcdsaAuxInfo("sess-1", ids, i, primes[i])));
      console.log(`wasm aux-info (3 parties, one thread): ${((performance.now() - t) / 1000).toFixed(1)}s`);
      t = performance.now();
      const kg = run(ids.map((_, i) => new EcdsaKeygen("sess-1", ids, i, 2)));
      console.log(`wasm keygen (3 parties, one thread): ${((performance.now() - t) / 1000).toFixed(1)}s`);
      shares = ids.map((_, i) => ecdsa_key_share_from_parts(ids, kg[i], aux[i]));
      const infos = shares.map((s) => JSON.parse(ecdsa_share_info(s)));
      expect(new Set(infos.map((x) => x.group_public_key)).size).toBe(1);
      expect(infos.map((x) => x.index)).toEqual([0, 1, 2]);
      expect(infos[0].threshold).toBe(2);
      const group = infos[0].group_public_key;
      const listed = JSON.parse(derive_account_addresses("secp256k1-ecdsa", group, 0));
      expect(listed).toEqual([
        { chain: "Ethereum", path: "m/44'/60'/0'/0/0", address: ecdsa_ethereum_address(group, 0) },
      ]);
    },
    30 * 60_000,
  );

  test(
    "signers {0,2} sign a prehash that ecrecovers to account 1",
    () => {
      const prehash = new Uint8Array(32).map((_, i) => i + 1);
      const t = performance.now();
      const sigs = run(
        [0, 2].map((i) => new EcdsaSigning("sign_1", shares[i], new Uint16Array([0, 2]), "m/44'/60'/0'/0/1", prehash)),
      );
      console.log(`wasm signing (2 signers, one thread): ${((performance.now() - t) / 1000).toFixed(1)}s`);
      expect(sigs[0]).toBe(sigs[1]);
      const sig = Uint8Array.from(Buffer.from(sigs[0], "hex"));
      expect(sig.length).toBe(65);
      expect([27, 28]).toContain(sig[64]);
      const group = JSON.parse(ecdsa_share_info(shares[0])).group_public_key;
      expect(ecdsa_recover_ethereum_address(prehash, sig)).toBe(ecdsa_ethereum_address(group, 1));
    },
    10 * 60_000,
  );

  test("payload headers carry the execution id", () => {
    const c = new EcdsaKeygen("sess-2", ids, 1, 2);
    c.proceed();
    const out = c.take_outgoing();
    expect(out.length).toBeGreaterThan(0);
    expect(ecdsa_payload_execution_id(out[0].payload)).toBe(ecdsa_execution_id("sess-2", "keygen", ["dev-c", "dev-a", "dev-b"]));
    expect(c.proceed()).toBeUndefined();
  });

  test("a payload of another execution is refused", () => {
    const a = new EcdsaKeygen("sess-3", ids, 0, 2);
    const b = new EcdsaKeygen("sess-4", ids, 1, 2);
    b.proceed();
    const [m] = b.take_outgoing();
    expect(() => a.receive(1, m.payload)).toThrow();
  });
});
