// User-facing error guidance for the popup.
//
// Turns a raw/internal error string into something an investor fumbling the live
// demo can act on: the original error + the most likely cause + the next step.
// Keyword-based and conservative — when we recognise a cause we say something
// specific, otherwise we keep the raw error and add general "what to check"
// guidance. We never drop the original text.

export type ErrorContext = "connection" | "dkg" | "signing" | "keystore";

const ROOM_HINT =
    "Set a strong room (≥16 chars) in Settings ⚙ — the SAME on every device — then reconnect.";

/** Append actionable guidance to a raw error, based on `context` + keywords. */
export function guideError(raw: unknown, context: ErrorContext): string {
    const err = (raw == null ? "" : String(raw)).trim();
    const lc = err.toLowerCase();
    let hint = "";

    switch (context) {
        case "connection":
            if (lc.includes("room") || lc.includes("400") || lc.includes("rejected") || lc.includes("unauthor")) {
                hint = `No / weak room. ${ROOM_HINT}`;
            } else {
                hint =
                    "Check your network and the signal server in Settings ⚙. The hosted server also " +
                    "requires a strong room (≥16 chars).";
            }
            break;
        case "dkg":
            if (lc.includes("connect") || lc.includes("offline") || lc.includes("signal") || lc.includes("websocket")) {
                hint = `Couldn't reach the signal server. ${ROOM_HINT}`;
            } else if (lc.includes("timeout") || lc.includes("timed out") || lc.includes("waiting")) {
                hint =
                    "All participants must be online together in the SAME room, each with a unique device id.";
            } else if (lc.includes("room")) {
                hint = ROOM_HINT;
            } else {
                hint = "Make sure every participant is online in the same room with a unique device id.";
            }
            break;
        case "signing":
            if (lc.includes("timeout") || lc.includes("timed out") || lc.includes("waiting")) {
                hint =
                    "A quorum (the threshold) must approve. Make sure enough co-signers are online in " +
                    "the same room and approved the request.";
            } else if (lc.includes("password") || lc.includes("unlock") || lc.includes("decrypt")) {
                hint = "Wrong password for this wallet — enter the password you set on this device.";
            } else if (lc.includes("not found")) {
                hint = "Wallet/account not found on this device — pick the right one.";
            } else if (lc.includes("connect") || lc.includes("offline") || lc.includes("signal")) {
                hint = `Couldn't reach the signal server / co-signers. ${ROOM_HINT}`;
            } else {
                hint =
                    "Check at least the threshold number of co-signers are online in the same room and approved.";
            }
            break;
        case "keystore":
            if (lc.includes("password") || lc.includes("decrypt") || lc.includes("mac") || lc.includes("invalid")) {
                hint = "Wrong password, or the file isn't a matching keystore. Re-check the password.";
            } else {
                hint = "Check the file is a valid keystore export and the password is correct.";
            }
            break;
    }

    if (!err) return hint;
    return hint ? `${err}\n→ ${hint}` : err;
}
