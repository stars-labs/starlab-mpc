/**
 * Tolerant parser for `session_info` payloads received over the
 * WebSocket signal server. Mirrors TUI's behaviour in
 * `apps/tui/src/elm/command.rs::parse_session_info`:
 *
 * - Required fields: session_id, total, threshold.
 * - Defaulted fields (TUI fills in when absent):
 *     proposer_id → "unknown"
 *     participants → []
 *     curve_type → "secp256k1"
 *     coordination_type → "Network"
 *     session_type → "dkg"
 * - Signing-only fields pulled through if present.
 * - Extension-local `accepted_devices` synthesised as [] — TUI
 *   announcements don't carry this, but the rest of the extension
 *   code treats it as always defined.
 *
 * Returns `null` if any required field is missing or mistyped. The
 * caller (webSocketManager) logs and drops malformed payloads rather
 * than throwing.
 */
import type { SessionInfo, SessionTypeTag } from "@starlab/types/session";

export function parseSessionInfoFromWire(raw: unknown): SessionInfo | null {
    if (typeof raw !== "object" || raw === null) {
        return null;
    }
    const r = raw as Record<string, unknown>;

    const session_id = typeof r.session_id === "string" ? r.session_id : null;
    if (!session_id) return null;

    const total = typeof r.total === "number" ? r.total : null;
    if (total === null || !Number.isFinite(total) || total < 0) return null;

    const threshold = typeof r.threshold === "number" ? r.threshold : null;
    if (threshold === null || !Number.isFinite(threshold) || threshold < 0) {
        return null;
    }

    const proposer_id =
        typeof r.proposer_id === "string" ? r.proposer_id : "unknown";

    const participants = Array.isArray(r.participants)
        ? r.participants.filter((v): v is string => typeof v === "string")
        : [];

    const curve_type =
        typeof r.curve_type === "string" ? r.curve_type : "secp256k1";

    const coordination_type =
        typeof r.coordination_type === "string" ? r.coordination_type : "Network";

    // session_type on the wire is a flat lowercase string ("dkg" /
    // "signing"). We accept any string but narrow to the union when
    // the match succeeds; unknown tags degrade to undefined so
    // consumers treat them as "legacy payload" rather than DKG.
    const tagRaw = typeof r.session_type === "string" ? r.session_type : "dkg";
    const session_type: SessionTypeTag | undefined =
        tagRaw === "dkg" || tagRaw === "signing" ? tagRaw : undefined;

    const wallet_name =
        typeof r.wallet_name === "string" ? r.wallet_name : undefined;
    const group_public_key =
        typeof r.group_public_key === "string" ? r.group_public_key : undefined;
    const blockchain =
        typeof r.blockchain === "string" ? r.blockchain : undefined;

    // TUI also accepts the legacy alias `message_hex`.
    const signing_message_hex =
        typeof r.signing_message_hex === "string"
            ? r.signing_message_hex
            : typeof r.message_hex === "string"
              ? (r.message_hex as string)
              : undefined;

    // accepted_devices: extension-local bookkeeping. If the sender
    // included one (another extension peer, or a re-sent
    // session_status_update), honour it; otherwise synthesise [].
    const accepted_devices = Array.isArray(r.accepted_devices)
        ? r.accepted_devices.filter((v): v is string => typeof v === "string")
        : [];

    const status = typeof r.status === "string" ? r.status : undefined;

    return {
        session_id,
        proposer_id,
        total,
        threshold,
        participants,
        session_type,
        curve_type,
        coordination_type,
        wallet_name,
        group_public_key,
        blockchain,
        signing_message_hex,
        accepted_devices,
        status,
    };
}

/**
 * Build a TUI-compatible `session_info` JSON payload for the
 * `announce_session` outbound message. Shape follows TUI's announcer
 * in `command.rs` near line 555: all DKG-common fields top-level;
 * signing-specific fields appended top-level if `session_type ===
 * "signing"`. Does NOT include extension-local fields like
 * `accepted_devices` or `status` — keep the wire lean.
 */
export function buildWireSessionInfo(s: SessionInfo): Record<string, unknown> {
    const base: Record<string, unknown> = {
        session_id: s.session_id,
        proposer_id: s.proposer_id,
        total: s.total,
        threshold: s.threshold,
        participants: s.participants,
        session_type: s.session_type ?? "dkg",
        curve_type: s.curve_type ?? "secp256k1",
        coordination_type: s.coordination_type ?? "Network",
    };
    if (s.session_type === "signing") {
        if (s.wallet_name !== undefined) base.wallet_name = s.wallet_name;
        if (s.group_public_key !== undefined) {
            base.group_public_key = s.group_public_key;
        }
        if (s.blockchain !== undefined) base.blockchain = s.blockchain;
        if (s.signing_message_hex !== undefined) {
            base.signing_message_hex = s.signing_message_hex;
        }
    }
    return base;
}
