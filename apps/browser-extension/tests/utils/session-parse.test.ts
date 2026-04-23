/**
 * Regression tests for Ext-1a: wire-format parity with TUI.
 *
 * The TUI emits `session_info` JSON blobs with TUI-specific defaults
 * (e.g. no `accepted_devices`, `session_type` as a flat string). Our
 * parser must tolerate these and synthesise sensible defaults so the
 * extension's downstream code — which assumes every SessionInfo has
 * `accepted_devices: []` at minimum — doesn't crash on TUI-originated
 * invites.
 *
 * The inverse builder (`buildWireSessionInfo`) must emit shape that
 * TUI's `parse_session_info` (command.rs line 231) accepts. If this
 * test file diverges from that Rust function, interop silently
 * breaks — so the expectations here are load-bearing.
 */
import { describe, it, expect } from "bun:test";
import {
    parseSessionInfoFromWire,
    buildWireSessionInfo,
} from "../../src/utils/session-parse";
import type { SessionInfo } from "@mpc-wallet/types/session";

describe("parseSessionInfoFromWire", () => {
    it("accepts a minimal TUI-shaped DKG announcement", () => {
        const wire = {
            session_id: "dkg_abc",
            proposer_id: "tui-node-1",
            total: 3,
            threshold: 2,
            participants: ["tui-node-1"],
            session_type: "dkg",
            curve_type: "secp256k1",
            coordination_type: "Network",
        };
        const parsed = parseSessionInfoFromWire(wire);
        expect(parsed).not.toBeNull();
        expect(parsed!.session_id).toBe("dkg_abc");
        expect(parsed!.total).toBe(3);
        expect(parsed!.threshold).toBe(2);
        expect(parsed!.session_type).toBe("dkg");
        // Synthesised by the parser, not present on wire
        expect(parsed!.accepted_devices).toEqual([]);
    });

    it("synthesises defaults matching TUI's parse_session_info", () => {
        // Bare-minimum payload — only required fields. Every default
        // here must match the TUI defaults in command.rs line 231:
        // proposer_id "unknown", curve_type "secp256k1",
        // coordination_type "Network", session_type "dkg",
        // participants [].
        const parsed = parseSessionInfoFromWire({
            session_id: "minimal",
            total: 2,
            threshold: 2,
        });
        expect(parsed).not.toBeNull();
        expect(parsed!.proposer_id).toBe("unknown");
        expect(parsed!.participants).toEqual([]);
        expect(parsed!.curve_type).toBe("secp256k1");
        expect(parsed!.coordination_type).toBe("Network");
        expect(parsed!.session_type).toBe("dkg");
        expect(parsed!.accepted_devices).toEqual([]);
    });

    it("parses a TUI-shaped signing announcement with sibling fields", () => {
        // For signing sessions, TUI emits wallet_name, group_public_key,
        // blockchain, signing_message_hex as TOP-LEVEL siblings (not
        // nested under session_type). Parser must pick them up.
        const wire = {
            session_id: "sig_xyz",
            proposer_id: "alice",
            total: 3,
            threshold: 2,
            participants: ["alice", "bob", "charlie"],
            session_type: "signing",
            curve_type: "secp256k1",
            coordination_type: "Network",
            wallet_name: "wallet-dkg_4a3f",
            group_public_key: "02aabbcc",
            blockchain: "ethereum",
            signing_message_hex: "68656c6c6f",
        };
        const parsed = parseSessionInfoFromWire(wire);
        expect(parsed).not.toBeNull();
        expect(parsed!.session_type).toBe("signing");
        expect(parsed!.wallet_name).toBe("wallet-dkg_4a3f");
        expect(parsed!.group_public_key).toBe("02aabbcc");
        expect(parsed!.blockchain).toBe("ethereum");
        expect(parsed!.signing_message_hex).toBe("68656c6c6f");
    });

    it("accepts legacy alias message_hex for signing_message_hex", () => {
        // TUI's parser reads `message_hex` as a fallback; mirror that.
        const parsed = parseSessionInfoFromWire({
            session_id: "s",
            total: 2,
            threshold: 2,
            session_type: "signing",
            message_hex: "deadbeef",
        });
        expect(parsed!.signing_message_hex).toBe("deadbeef");
    });

    it("rejects payloads missing session_id", () => {
        expect(
            parseSessionInfoFromWire({ total: 3, threshold: 2 }),
        ).toBeNull();
    });

    it("rejects payloads with non-numeric total/threshold", () => {
        expect(
            parseSessionInfoFromWire({
                session_id: "x",
                total: "3",
                threshold: 2,
            }),
        ).toBeNull();
        expect(
            parseSessionInfoFromWire({
                session_id: "x",
                total: 3,
                threshold: "2",
            }),
        ).toBeNull();
    });

    it("rejects non-object inputs", () => {
        expect(parseSessionInfoFromWire(null)).toBeNull();
        expect(parseSessionInfoFromWire("string")).toBeNull();
        expect(parseSessionInfoFromWire(42)).toBeNull();
        expect(parseSessionInfoFromWire(undefined)).toBeNull();
    });

    it("drops non-string entries from participants", () => {
        const parsed = parseSessionInfoFromWire({
            session_id: "x",
            total: 3,
            threshold: 2,
            participants: ["alice", 42, null, "bob", { weird: true }],
        });
        expect(parsed!.participants).toEqual(["alice", "bob"]);
    });

    it("degrades unknown session_type tag to undefined", () => {
        // A payload tagged with something neither "dkg" nor "signing"
        // shouldn't be silently treated as DKG — parser returns
        // undefined so downstream can decide what to do (ignore /
        // log). This guards against a future `session_type` value
        // being mis-routed into DKG UI.
        const parsed = parseSessionInfoFromWire({
            session_id: "x",
            total: 3,
            threshold: 2,
            session_type: "future_variant",
        });
        expect(parsed!.session_type).toBeUndefined();
    });
});

describe("buildWireSessionInfo", () => {
    it("omits signing-only fields on DKG sessions", () => {
        const s: SessionInfo = {
            session_id: "dkg_1",
            proposer_id: "me",
            total: 3,
            threshold: 2,
            participants: ["me"],
            session_type: "dkg",
            curve_type: "secp256k1",
            coordination_type: "Network",
            accepted_devices: [],
        };
        const wire = buildWireSessionInfo(s);
        expect(wire.session_id).toBe("dkg_1");
        expect(wire.session_type).toBe("dkg");
        // These should NOT leak into a DKG wire payload.
        expect("wallet_name" in wire).toBe(false);
        expect("group_public_key" in wire).toBe(false);
        expect("signing_message_hex" in wire).toBe(false);
        // accepted_devices is local-only — must not hit the wire.
        expect("accepted_devices" in wire).toBe(false);
    });

    it("includes signing-only fields on signing sessions", () => {
        const s: SessionInfo = {
            session_id: "sig_1",
            proposer_id: "me",
            total: 3,
            threshold: 2,
            participants: ["me"],
            session_type: "signing",
            curve_type: "secp256k1",
            coordination_type: "Network",
            wallet_name: "w",
            group_public_key: "02aa",
            blockchain: "ethereum",
            signing_message_hex: "deadbeef",
            accepted_devices: [],
        };
        const wire = buildWireSessionInfo(s);
        expect(wire.session_type).toBe("signing");
        expect(wire.wallet_name).toBe("w");
        expect(wire.group_public_key).toBe("02aa");
        expect(wire.blockchain).toBe("ethereum");
        expect(wire.signing_message_hex).toBe("deadbeef");
    });

    it("defaults session_type, curve_type, coordination_type when unset", () => {
        // A caller that forgot to set defaults shouldn't trip TUI's
        // parser. We emit the same defaults TUI fills in.
        const s: SessionInfo = {
            session_id: "x",
            proposer_id: "me",
            total: 2,
            threshold: 2,
            participants: [],
            accepted_devices: [],
        };
        const wire = buildWireSessionInfo(s);
        expect(wire.session_type).toBe("dkg");
        expect(wire.curve_type).toBe("secp256k1");
        expect(wire.coordination_type).toBe("Network");
    });

    it("round-trips parse → build with no semantic loss for DKG", () => {
        // Interop guarantee: if the extension receives a TUI-shaped
        // DKG payload and immediately re-announces it (e.g. a
        // session_status_update flow), the wire bytes should match.
        const tuiWire = {
            session_id: "dkg_round_trip",
            proposer_id: "tui-1",
            total: 3,
            threshold: 2,
            participants: ["tui-1", "tui-2"],
            session_type: "dkg",
            curve_type: "secp256k1",
            coordination_type: "Network",
        };
        const parsed = parseSessionInfoFromWire(tuiWire)!;
        const rebuilt = buildWireSessionInfo(parsed);
        // Compare field-for-field on the keys TUI cares about.
        for (const k of Object.keys(tuiWire)) {
            expect(rebuilt[k]).toEqual((tuiWire as any)[k]);
        }
    });
});
