// Generate real FROST keystores for test fixtures by running a
// complete 2-of-3 DKG ceremony through the WASM and dumping each
// participant's export_keystore() output.
//
// Re-run with: `bun run scripts/gen-frost-fixtures.ts` from the
// browser-extension directory. The output is stable across runs
// for a given WASM build — regenerate whenever the on-disk
// keystore schema in packages/@mpc-wallet/frost-core changes.

import wasmInit, { FrostDkgSecp256k1, FrostDkgEd25519 } from '@mpc-wallet/core-wasm';
import { writeFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

await wasmInit();

interface DkgInstance {
    init_dkg(idx: number, total: number, threshold: number): unknown;
    generate_round1(): string;
    add_round1_package(idx: number, hex: string): unknown;
    can_start_round2(): boolean;
    generate_round2(): string;
    add_round2_package(idx: number, hex: string): unknown;
    finalize_dkg(): string;
    export_keystore(): string;
}

function runDkg<T extends DkgInstance>(
    newInstance: () => T,
    totalParticipants: number,
    threshold: number,
): string[] {
    const instances = Array.from({ length: totalParticipants }, () => newInstance());
    for (let i = 0; i < totalParticipants; i++) {
        instances[i].init_dkg(i + 1, totalParticipants, threshold);
    }
    const round1 = instances.map((inst) => inst.generate_round1());
    for (let i = 0; i < totalParticipants; i++) {
        for (let j = 0; j < totalParticipants; j++) {
            if (i !== j) instances[i].add_round1_package(j + 1, round1[j]);
        }
    }
    const round2Maps = instances.map((inst) => inst.generate_round2());
    for (let sender = 0; sender < totalParticipants; sender++) {
        const mapJson = Buffer.from(round2Maps[sender], 'hex').toString();
        const map = JSON.parse(mapJson) as Record<string, string>;
        for (let recipient = 0; recipient < totalParticipants; recipient++) {
            if (sender === recipient) continue;
            const pkg = map[String(recipient + 1)];
            if (!pkg) throw new Error(`no pkg for ${recipient + 1} from ${sender + 1}`);
            instances[recipient].add_round2_package(sender + 1, pkg);
        }
    }
    instances.forEach((inst) => inst.finalize_dkg());
    return instances.map((inst) => inst.export_keystore());
}

const secp = runDkg(() => new FrostDkgSecp256k1() as unknown as DkgInstance, 3, 2);
const ed = runDkg(() => new FrostDkgEd25519() as unknown as DkgInstance, 3, 2);

const here = dirname(fileURLToPath(import.meta.url));
const outDir = resolve(here, '..', 'test-data');
writeFileSync(resolve(outDir, 'real-secp256k1-keystore-p2.json'), secp[1]);
writeFileSync(resolve(outDir, 'real-ed25519-keystore-p1.json'), ed[0]);
console.log('Wrote:');
console.log(`  ${outDir}/real-secp256k1-keystore-p2.json`);
console.log(`  ${outDir}/real-ed25519-keystore-p1.json`);
