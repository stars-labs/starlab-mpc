/**
 * Regression guards for four core-wasm FROST contracts that silently
 * broke in earlier iterations. Each test pins ONE invariant so a
 * regression points at the specific layer that slipped.
 *
 * Companion to the full ceremony flows in
 * src/entrypoints/offscreen/webrtc.test.ts and
 * tests/entrypoints/offscreen/webrtc.signing.test.ts — those
 * exercise the same contracts end-to-end but through enough plumbing
 * that a failure doesn't immediately localise.
 */

import { describe, test, expect, beforeAll } from 'bun:test';
import wasmInit, { FrostDkgEd25519, FrostDkgSecp256k1 } from '@starlab/core-wasm';

beforeAll(async () => {
    await wasmInit();
});

describe('FROST WASM contracts', () => {
    test('can_start_round2 matches frost-core n-1 contract (not n)', () => {
        // frost-core dkg::part2 at keys/dkg.rs:505 requires exactly
        // max_signers - 1 round-1 packages. Our WASM's
        // can_start_round2 must reflect that threshold, not `total`.
        // An off-by-one here would either block progress forever
        // (stuck at round 1) or let aggregate reach part2 with a
        // bad map that frost-core then rejects.
        const a = new FrostDkgEd25519();
        const b = new FrostDkgEd25519();
        const c = new FrostDkgEd25519();
        try {
            a.init_dkg(1, 3, 2);
            b.init_dkg(2, 3, 2);
            c.init_dkg(3, 3, 2);
            const r1a = a.generate_round1();
            const r1b = b.generate_round1();
            const r1c = c.generate_round1();

            // After generate_round1 but before any peer packages, a's
            // signing_commitments map is empty — NOT ready.
            expect(a.can_start_round2()).toBe(false);

            a.add_round1_package(2, r1b);
            // After n-2 peer packages, still not ready.
            expect(a.can_start_round2()).toBe(false);

            a.add_round1_package(3, r1c);
            // After n-1 peer packages, NOW ready (frost-core contract).
            expect(a.can_start_round2()).toBe(true);
        } finally {
            a.free();
            b.free();
            c.free();
        }
    });

    test('generate_round2 returns hex-encoded JSON, not raw JSON', () => {
        // generate_round1 returns hex-encoded JSON. generate_round2
        // USED TO return raw JSON, which broke consumers that uniformly
        // hex-decode before parsing (see webrtc.ts
        // _generateAndBroadcastRound2). Both round functions must emit
        // the same wire format.
        const a = new FrostDkgSecp256k1();
        const b = new FrostDkgSecp256k1();
        const c = new FrostDkgSecp256k1();
        try {
            a.init_dkg(1, 3, 2);
            b.init_dkg(2, 3, 2);
            c.init_dkg(3, 3, 2);
            const r1a = a.generate_round1();
            const r1b = b.generate_round1();
            const r1c = c.generate_round1();
            a.add_round1_package(2, r1b);
            a.add_round1_package(3, r1c);

            const r2 = a.generate_round2();
            // Must be pure hex — no JSON braces at the outer level.
            expect(r2).toMatch(/^[0-9a-f]+$/);
            // Decoding as hex must produce valid JSON with numeric-string keys.
            const decoded = Buffer.from(r2, 'hex').toString('utf8');
            const map = JSON.parse(decoded);
            expect(typeof map).toBe('object');
            // Keys are stringified participant indices; a (id 1) sent packages to b (2) and c (3).
            expect(Object.keys(map).sort()).toEqual(['2', '3']);
        } finally {
            a.free();
            b.free();
            c.free();
        }
    });

    test('signing_commit auto-registers own commitment so sign() succeeds', () => {
        // frost-core round2::sign at round2.rs:135 returns
        // Error::MissingCommitment if key_package.identifier isn't in
        // signing_commitments. Our WASM's signing_commit MUST insert
        // the own commitment there; otherwise the caller would need
        // explicit add_signing_commitment(self_idx, own_hex) which
        // complicates the contract.
        //
        // Indirectly asserted: a 2-of-3 DKG + threshold sign
        // ceremony reaches aggregate_signature without error.
        const participants = [new FrostDkgEd25519(), new FrostDkgEd25519(), new FrostDkgEd25519()];
        try {
            for (let i = 0; i < 3; i++) participants[i].init_dkg(i + 1, 3, 2);
            const r1 = participants.map((p) => p.generate_round1());
            for (let i = 0; i < 3; i++) {
                for (let j = 0; j < 3; j++) {
                    if (i !== j) participants[i].add_round1_package(j + 1, r1[j]);
                }
            }
            const r2Maps = participants.map((p) => p.generate_round2());
            for (let sender = 0; sender < 3; sender++) {
                const map = JSON.parse(Buffer.from(r2Maps[sender], 'hex').toString());
                for (let recipient = 0; recipient < 3; recipient++) {
                    if (sender === recipient) continue;
                    participants[recipient].add_round2_package(sender + 1, map[String(recipient + 1)]);
                }
            }
            participants.forEach((p) => p.finalize_dkg());

            // 2-of-3 sign: participant 1 + participant 2.
            const messageHex = Buffer.from('hello', 'utf8').toString('hex');
            const c1 = participants[0].signing_commit();
            const c2 = participants[1].signing_commit();
            participants[0].add_signing_commitment(2, c2);
            participants[1].add_signing_commitment(1, c1);

            // If signing_commit didn't self-register, these .sign()
            // calls would throw MissingCommitment here.
            const s1 = participants[0].sign(messageHex);
            const s2 = participants[1].sign(messageHex);
            expect(s1).toMatch(/^[0-9a-f]+$/);
            expect(s2).toMatch(/^[0-9a-f]+$/);
        } finally {
            participants.forEach((p) => p.free());
        }
    });

    test('sign auto-registers own share so aggregate_signature succeeds', () => {
        // frost-core aggregate enforces that signing_commitments and
        // signature_shares contain the same identifier set. Our WASM's
        // sign() must insert the own share into signature_shares so
        // aggregate_signature sees it alongside the peer shares added
        // via add_signature_share. Otherwise aggregate would throw
        // UnknownIdentifier.
        const participants = [new FrostDkgSecp256k1(), new FrostDkgSecp256k1(), new FrostDkgSecp256k1()];
        try {
            for (let i = 0; i < 3; i++) participants[i].init_dkg(i + 1, 3, 2);
            const r1 = participants.map((p) => p.generate_round1());
            for (let i = 0; i < 3; i++) {
                for (let j = 0; j < 3; j++) {
                    if (i !== j) participants[i].add_round1_package(j + 1, r1[j]);
                }
            }
            const r2Maps = participants.map((p) => p.generate_round2());
            for (let sender = 0; sender < 3; sender++) {
                const map = JSON.parse(Buffer.from(r2Maps[sender], 'hex').toString());
                for (let recipient = 0; recipient < 3; recipient++) {
                    if (sender === recipient) continue;
                    participants[recipient].add_round2_package(sender + 1, map[String(recipient + 1)]);
                }
            }
            participants.forEach((p) => p.finalize_dkg());

            const messageHex = Buffer.from('aggregate-contract', 'utf8').toString('hex');
            const c1 = participants[0].signing_commit();
            const c2 = participants[1].signing_commit();
            participants[0].add_signing_commitment(2, c2);
            participants[1].add_signing_commitment(1, c1);
            const s1 = participants[0].sign(messageHex);
            const s2 = participants[1].sign(messageHex);
            // Participant 0 acts as aggregator — receives s2 from peer.
            participants[0].add_signature_share(2, s2);

            // If sign() didn't self-register share for participant 0,
            // aggregate_signature would throw here.
            const aggregated = participants[0].aggregate_signature(messageHex);
            expect(aggregated).toMatch(/^[0-9a-f]+$/);
        } finally {
            participants.forEach((p) => p.free());
        }
    });
});
