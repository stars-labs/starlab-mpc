import { describe, it, expect } from "bun:test";
import { guideError } from "../../src/utils/error-guidance";

describe("guideError", () => {
    it("keeps the raw error and appends a hint", () => {
        const out = guideError("boom", "connection");
        expect(out).toContain("boom");
        expect(out).toContain("→");
    });

    it("connection: a 400/rejected points at the room", () => {
        expect(guideError("WebSocket 400", "connection").toLowerCase()).toContain("room");
        expect(guideError("connection rejected", "connection").toLowerCase()).toContain("room");
    });

    it("dkg: connection-ish errors point at server+room; timeouts at participants", () => {
        expect(guideError("signal server offline", "dkg").toLowerCase()).toContain("signal server");
        expect(guideError("timed out after 90s", "dkg").toLowerCase()).toContain("same room");
    });

    it("signing: timeout → quorum; password → wrong password", () => {
        expect(guideError("timed out", "signing").toLowerCase()).toContain("quorum");
        expect(guideError("invalid password", "signing").toLowerCase()).toContain("wrong password");
    });

    it("keystore: bad password is recognised", () => {
        expect(guideError("decrypt failed", "keystore").toLowerCase()).toContain("password");
    });

    it("empty error yields just the hint (no leading separator)", () => {
        const out = guideError("", "connection");
        expect(out.startsWith("→")).toBe(false);
        expect(out.length).toBeGreaterThan(0);
    });

    it("null/undefined are handled", () => {
        expect(() => guideError(null, "dkg")).not.toThrow();
        expect(() => guideError(undefined, "signing")).not.toThrow();
    });
});
