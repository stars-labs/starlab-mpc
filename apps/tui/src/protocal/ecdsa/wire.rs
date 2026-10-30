//! Data-channel framing of the threshold-ECDSA engine (all pure, no I/O).
//!
//! Every frame is a `SimpleMessage` text on the existing WebRTC data channel
//! (`{"webrtc_msg_type":"SimpleMessage","text":"…"}`); the sender is the
//! channel's peer. Four kinds:
//!
//! ```text
//! ECDSA:<id>:<index>:<count>:<base64 chunk>      protocol message chunk
//! ECDSA_SIGN_READY:<base64 JSON {"session_id"}>  would-be signer → proposer
//! ECDSA_SIGN_SET:<base64 JSON {"session_id","signers":[device ids]}>
//!                                                proposer → every participant
//! ECDSA_SIGN_DONE:<base64 JSON {"session_id","signature":"<hex r‖s‖v>"}>
//!                                                proposer → participants outside the set
//! ```
//!
//! A protocol message is one `starlab_core::ecdsa` driver payload (35-byte
//! header: version, protocol, broadcast|p2p, execution id — then CBOR). Aux
//! info messages are ~200 KiB, above what browsers accept per data-channel
//! message, so every payload is split into [`CHUNK_SIZE`]-byte chunks:
//! `<id>` = lowercase hex of the first 8 bytes of SHA-256(payload), `<index>`
//! 0-based, `<count>` ≥ 1, both decimal; base64 is standard with padding. A
//! receiver reassembles per (sender, id) and checks the digest. A broadcast
//! message is sent to every other party of the ceremony, a p2p one to that
//! party only. base64 alphabet contains no `:`, so the text splits cleanly.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const FRAME_PREFIX: &str = "ECDSA:";
pub const SIGN_READY_PREFIX: &str = "ECDSA_SIGN_READY:";
pub const SIGN_SET_PREFIX: &str = "ECDSA_SIGN_SET:";
pub const SIGN_DONE_PREFIX: &str = "ECDSA_SIGN_DONE:";

/// Binary bytes per chunk (base64 → ~22 KB of text, far below every data
/// channel's message limit).
pub const CHUNK_SIZE: usize = 16 * 1024;
/// Largest message we reassemble (64 chunks = 1 MiB).
const MAX_CHUNKS: u16 = 64;
/// Incomplete messages kept at once (across senders) before we give up on
/// the oldest ones — bounds memory against a peer that never finishes one.
const MAX_PARTIAL: usize = 256;

/// Every frame kind the engine owns (ceremony frames: resent on reconnect,
/// de-duplicated on receipt — each one carries a session id or a payload
/// digest, so distinct frames never collide).
pub const PREFIXES: &[&str] = &[
    FRAME_PREFIX,
    SIGN_READY_PREFIX,
    SIGN_SET_PREFIX,
    SIGN_DONE_PREFIX,
];

fn message_id(payload: &[u8]) -> String {
    hex::encode(&Sha256::digest(payload)[..8])
}

/// Split one driver payload into its `ECDSA:` frames.
pub fn encode_frames(payload: &[u8]) -> Vec<String> {
    let id = message_id(payload);
    let chunks: Vec<&[u8]> = if payload.is_empty() {
        vec![&[]]
    } else {
        payload.chunks(CHUNK_SIZE).collect()
    };
    let count = chunks.len();
    chunks
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{FRAME_PREFIX}{id}:{i}:{count}:{}", BASE64.encode(c)))
        .collect()
}

/// One parsed `ECDSA:` frame (text after the prefix).
#[derive(Debug, PartialEq, Eq)]
pub struct Chunk {
    pub id: String,
    pub index: u16,
    pub count: u16,
    pub data: Vec<u8>,
}

pub fn parse_chunk(rest: &str) -> Result<Chunk, String> {
    let mut parts = rest.splitn(4, ':');
    let (Some(id), Some(index), Some(count), Some(b64)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err("malformed ECDSA frame".into());
    };
    if id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("bad ECDSA message id {id:?}"));
    }
    let index: u16 = index.parse().map_err(|_| "bad ECDSA chunk index")?;
    let count: u16 = count.parse().map_err(|_| "bad ECDSA chunk count")?;
    if count == 0 || count > MAX_CHUNKS || index >= count {
        return Err(format!("bad ECDSA chunk {index}/{count}"));
    }
    let data = BASE64
        .decode(b64)
        .map_err(|e| format!("ECDSA chunk base64: {e}"))?;
    Ok(Chunk {
        id: id.to_ascii_lowercase(),
        index,
        count,
        data,
    })
}

#[derive(Debug)]
struct Partial {
    count: u16,
    chunks: HashMap<u16, Vec<u8>>,
    seq: u64,
}

/// Reassembles chunked messages per (sender, message id).
#[derive(Debug, Default)]
pub struct Reassembler {
    partial: HashMap<(String, String), Partial>,
    seq: u64,
}

impl Reassembler {
    /// Add a chunk from `from`; returns the whole payload once its last
    /// chunk arrived and the digest matches. Repeated chunks are harmless.
    pub fn push(&mut self, from: &str, chunk: Chunk) -> Result<Option<Vec<u8>>, String> {
        let key = (from.to_string(), chunk.id.clone());
        if !self.partial.contains_key(&key) && self.partial.len() >= MAX_PARTIAL {
            // Drop the oldest incomplete message.
            if let Some(oldest) = self
                .partial
                .iter()
                .min_by_key(|(_, p)| p.seq)
                .map(|(k, _)| k.clone())
            {
                self.partial.remove(&oldest);
            }
        }
        self.seq += 1;
        let seq = self.seq;
        let entry = self.partial.entry(key.clone()).or_insert_with(|| Partial {
            count: chunk.count,
            chunks: HashMap::new(),
            seq,
        });
        if entry.count != chunk.count {
            self.partial.remove(&key);
            return Err(format!(
                "ECDSA message {} from {from}: inconsistent chunk count",
                chunk.id
            ));
        }
        entry.chunks.insert(chunk.index, chunk.data);
        if entry.chunks.len() < usize::from(entry.count) {
            return Ok(None);
        }
        let entry = self.partial.remove(&key).expect("present");
        let mut payload = Vec::new();
        for i in 0..entry.count {
            payload.extend_from_slice(&entry.chunks[&i]);
        }
        if message_id(&payload) != chunk.id {
            return Err(format!(
                "ECDSA message {} from {from}: digest mismatch",
                chunk.id
            ));
        }
        Ok(Some(payload))
    }
}

/// Body of the three signing control frames (JSON, then base64).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Control {
    pub session_id: String,
    /// `ECDSA_SIGN_SET` only: the fixed signer set (device ids).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signers: Vec<String>,
    /// `ECDSA_SIGN_DONE` only: hex of the 65-byte `r ‖ s ‖ v` signature.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
}

impl Control {
    pub fn encode(&self, prefix: &str) -> String {
        let json = serde_json::to_vec(self).expect("Control always serializes");
        format!("{prefix}{}", BASE64.encode(json))
    }

    pub fn decode(b64: &str) -> Result<Self, String> {
        let json = BASE64
            .decode(b64)
            .map_err(|e| format!("ECDSA control frame base64: {e}"))?;
        serde_json::from_slice(&json).map_err(|e| format!("ECDSA control frame JSON: {e}"))
    }
}

/// The 32-byte execution id in a driver payload's header, if it has one.
pub fn execution_id_of(payload: &[u8]) -> Option<[u8; 32]> {
    payload.get(3..35).and_then(|s| s.try_into().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_through_the_reassembler_in_any_order() {
        let payload: Vec<u8> = (0..(CHUNK_SIZE * 3 + 5)).map(|i| i as u8).collect();
        let mut frames = encode_frames(&payload);
        assert_eq!(frames.len(), 4);
        assert!(frames.iter().all(|f| f.starts_with(FRAME_PREFIX)));
        frames.reverse();
        let mut r = Reassembler::default();
        let mut out = None;
        for (i, f) in frames.iter().enumerate() {
            let chunk = parse_chunk(&f[FRAME_PREFIX.len()..]).unwrap();
            // A duplicate chunk changes nothing.
            if i == 1 {
                let dup = parse_chunk(&f[FRAME_PREFIX.len()..]).unwrap();
                assert_eq!(r.push("peer", dup).unwrap(), None);
            }
            let got = r.push("peer", chunk).unwrap();
            if i + 1 < frames.len() {
                assert_eq!(got, None);
            } else {
                out = got;
            }
        }
        assert_eq!(out.unwrap(), payload);
    }

    #[test]
    fn frame_text_format_is_pinned() {
        let frames = encode_frames(b"abc");
        // sha256("abc")[..8] = ba7816bf8f01cfea
        assert_eq!(frames, vec!["ECDSA:ba7816bf8f01cfea:0:1:YWJj".to_string()]);
    }

    #[test]
    fn senders_are_kept_apart_and_tampering_is_detected() {
        let payload = vec![7u8; CHUNK_SIZE + 1];
        let frames = encode_frames(&payload);
        let mut r = Reassembler::default();
        let c0 = parse_chunk(&frames[0][FRAME_PREFIX.len()..]).unwrap();
        let c1 = parse_chunk(&frames[1][FRAME_PREFIX.len()..]).unwrap();
        assert_eq!(r.push("a", c0).unwrap(), None);
        // The other chunk from ANOTHER sender doesn't complete a's message.
        assert_eq!(r.push("b", c1).unwrap(), None);
        let mut bad = parse_chunk(&frames[1][FRAME_PREFIX.len()..]).unwrap();
        bad.data[0] ^= 1;
        assert!(r.push("a", bad).is_err());
    }

    #[test]
    fn malformed_frames_are_rejected() {
        for bad in [
            "",
            "zz:0:1:AA==",
            "ba7816bf8f01cfea:1:1:AA==",
            "ba7816bf8f01cfea:0:0:AA==",
            "ba7816bf8f01cfea:0:65:AA==",
            "ba7816bf8f01cfea:0:1:!!",
        ] {
            assert!(parse_chunk(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn control_frames_round_trip() {
        let set = Control {
            session_id: "sign_1".into(),
            signers: vec!["a".into(), "b".into()],
            signature: String::new(),
        };
        let text = set.encode(SIGN_SET_PREFIX);
        let back = Control::decode(text.strip_prefix(SIGN_SET_PREFIX).unwrap()).unwrap();
        assert_eq!(back, set);
        // READY carries only the session id.
        let ready = Control {
            session_id: "sign_1".into(),
            signers: vec![],
            signature: String::new(),
        }
        .encode(SIGN_READY_PREFIX);
        assert_eq!(
            ready,
            format!(
                "{SIGN_READY_PREFIX}{}",
                BASE64.encode(br#"{"session_id":"sign_1"}"#)
            )
        );
    }
}
