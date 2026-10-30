//! Transport-agnostic, synchronous driver over round_based's state machine.
//!
//! cggmp24 protocols are async functions; `round_based::state_machine`
//! wraps them into a poll-free sync [`StateMachine`] (no executor, no
//! threads, no timers), so the same code runs natively and on
//! `wasm32-unknown-unknown`. [`EcdsaCeremony`] owns one such machine plus
//! everything it borrows (RNG, key share, execution id), and exposes a plain
//! byte-level API:
//!
//! ```text
//! loop {
//!     for m in ceremony.take_outgoing() { transport.send(m.recipient, m.payload) }
//!     if let Some(out) = ceremony.proceed()? { break out }
//!     let (sender, bytes) = transport.recv();
//!     ceremony.receive(sender, &bytes)?;
//! }
//! ```
//!
//! Parties are always addressed by their **keygen index** (position in
//! [`super::EcdsaKeyShare::participants`]), also during signing, where
//! cggmp24 itself uses signer-local indices — the driver translates.
//!
//! # Wire format (version 1)
//!
//! ```text
//! byte 0      wire version (= 1)
//! byte 1      protocol     (1 = aux-info, 2 = keygen, 3 = signing)
//! byte 2      kind         (0 = broadcast, 1 = p2p)
//! bytes 3..35 execution id (32 bytes, see [`super::execution_id`])
//! bytes 35..  CBOR (RFC 8949, via `ciborium`) of the cggmp24 protocol message
//! ```
//!
//! The header lets a receiver drop a message from another ceremony before
//! decoding it; the execution id is ALSO bound inside every cggmp24 message
//! (commitments, ZK-proof transcripts), so a message replayed with a
//! rewritten header still aborts the protocol. Exact duplicates (same sender,
//! same bytes — e.g. a transport retry) are dropped silently; a *different*
//! second message for the same round from one sender aborts, as cggmp24
//! requires.

use super::Protocol;
use crate::errors::{FrostError, Result};
use cggmp24::round_based::state_machine::{ProceedResult, StateMachine};
use cggmp24::round_based::{Incoming, MessageDestination, MessageType};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};
use std::fmt::Display;

/// Current wire version (byte 0 of every payload).
pub const WIRE_VERSION: u8 = 1;
const HEADER_LEN: usize = 3 + 32;
const KIND_BROADCAST: u8 = 0;
const KIND_P2P: u8 = 1;

/// Who an outgoing message is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipient {
    /// Every other party of the ceremony ([`EcdsaCeremony::parties`]).
    Broadcast,
    /// One party, by keygen index. Must travel over an encrypted channel.
    P2P(u16),
}

/// One message to hand to the transport.
#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    pub recipient: Recipient,
    /// Self-contained wire payload (header + CBOR body); deliver verbatim.
    pub payload: Vec<u8>,
}

/// Type-erased typed state machine: decodes/encodes the protocol's message
/// type so [`EcdsaCeremony`] only sees bytes and signer-local indices.
trait Machine<O> {
    fn push(&mut self, sender: u16, broadcast: bool, body: &[u8]) -> Result<()>;
    /// Drive until blocked on input (`Ok(None)`) or finished.
    fn run(&mut self, out: &mut Vec<(Option<u16>, Vec<u8>)>) -> Result<Option<O>>;
}

struct Typed<SM: StateMachine> {
    sm: SM,
    inbox: VecDeque<Incoming<SM::Msg>>,
    next_id: u64,
    awaiting_msg: bool,
}

impl<SM, O, E> Machine<O> for Typed<SM>
where
    SM: StateMachine<Output = std::result::Result<O, E>>,
    SM::Msg: Serialize + DeserializeOwned,
    E: Display,
{
    fn push(&mut self, sender: u16, broadcast: bool, body: &[u8]) -> Result<()> {
        let msg: SM::Msg = ciborium::from_reader(body).map_err(|e| {
            FrostError::EcdsaError(format!("undecodable message from party {sender}: {e}"))
        })?;
        self.inbox.push_back(Incoming {
            id: self.next_id,
            sender,
            msg_type: if broadcast {
                MessageType::Broadcast
            } else {
                MessageType::P2P
            },
            msg,
        });
        self.next_id += 1;
        Ok(())
    }

    fn run(&mut self, out: &mut Vec<(Option<u16>, Vec<u8>)>) -> Result<Option<O>> {
        loop {
            if self.awaiting_msg {
                let Some(msg) = self.inbox.pop_front() else {
                    return Ok(None);
                };
                self.sm.received_msg(msg).map_err(|_| {
                    FrostError::EcdsaError("state machine refused a message".into())
                })?;
                self.awaiting_msg = false;
            }
            match self.sm.proceed() {
                ProceedResult::SendMsg(msg) => {
                    let mut body = Vec::new();
                    ciborium::into_writer(&msg.msg, &mut body)
                        .map_err(|e| FrostError::EcdsaError(format!("encode message: {e}")))?;
                    let to = match msg.recipient {
                        MessageDestination::AllParties => None,
                        MessageDestination::OneParty(j) => Some(j),
                    };
                    out.push((to, body));
                }
                ProceedResult::NeedsOneMoreMessage => self.awaiting_msg = true,
                ProceedResult::Yielded => {}
                ProceedResult::Output(Ok(output)) => return Ok(Some(output)),
                ProceedResult::Output(Err(e)) => {
                    return Err(FrostError::EcdsaError(format!("protocol aborted: {e}")));
                }
                ProceedResult::Error(e) => {
                    return Err(FrostError::EcdsaError(format!("state machine: {e}")));
                }
            }
        }
    }
}

/// One running aux-info / keygen / signing protocol instance for one party.
///
/// Not `Send` (round_based's state machine shares state through `Rc`):
/// drive it from one thread / one wasm worker. Calls to [`Self::proceed`]
/// can be CPU-heavy (Paillier/ZK math), so native callers should run the
/// ceremony off their async executor (a dedicated thread or
/// `spawn_blocking`-style task that owns it).
pub struct EcdsaCeremony<O> {
    protocol: Protocol,
    eid: [u8; 32],
    me: u16,
    /// Keygen indices of the ceremony's parties; position = cggmp24 index.
    parties: Vec<u16>,
    machine: Box<dyn Machine<O>>,
    outbox: Vec<OutgoingMessage>,
    seen: HashSet<(u16, [u8; 32])>,
    finished: bool,
}

impl<O> EcdsaCeremony<O> {
    /// `parties[j]` is the keygen index of cggmp24's party `j`.
    pub(crate) fn new<SM, E>(
        protocol: Protocol,
        eid: [u8; 32],
        me: u16,
        parties: Vec<u16>,
        sm: SM,
    ) -> Self
    where
        SM: StateMachine<Output = std::result::Result<O, E>> + 'static,
        SM::Msg: Serialize + DeserializeOwned + 'static,
        E: Display + 'static,
    {
        Self {
            protocol,
            eid,
            me,
            parties,
            machine: Box::new(Typed {
                sm,
                inbox: VecDeque::new(),
                next_id: 0,
                awaiting_msg: false,
            }),
            outbox: Vec::new(),
            seen: HashSet::new(),
            finished: false,
        }
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub fn execution_id(&self) -> &[u8; 32] {
        &self.eid
    }

    /// Our keygen index.
    pub fn index(&self) -> u16 {
        self.me
    }

    /// Keygen indices of all parties of this ceremony (us included).
    pub fn parties(&self) -> &[u16] {
        &self.parties
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Queue a payload received from party `sender` (keygen index). Rejects
    /// payloads for another ceremony (protocol / execution id mismatch),
    /// from a non-party or from ourselves, and undecodable bodies. Exact
    /// duplicates and anything arriving after completion are ignored.
    pub fn receive(&mut self, sender: u16, payload: &[u8]) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        if payload.len() < HEADER_LEN {
            return Err(FrostError::EcdsaError(format!(
                "message from party {sender} is too short"
            )));
        }
        if payload[0] != WIRE_VERSION {
            return Err(FrostError::EcdsaError(format!(
                "unsupported ECDSA wire version {}",
                payload[0]
            )));
        }
        if payload[1] != self.protocol.wire_tag() || payload[3..HEADER_LEN] != self.eid {
            return Err(FrostError::EcdsaError(format!(
                "message from party {sender} belongs to another execution"
            )));
        }
        let broadcast = match payload[2] {
            KIND_BROADCAST => true,
            KIND_P2P => false,
            other => {
                return Err(FrostError::EcdsaError(format!(
                    "bad message kind {other} from party {sender}"
                )));
            }
        };
        if sender == self.me {
            return Err(FrostError::EcdsaError("message from ourselves".into()));
        }
        let local = self.local_index(sender).ok_or_else(|| {
            FrostError::EcdsaError(format!("party {sender} is not part of this ceremony"))
        })?;
        if !self.seen.insert((sender, Sha256::digest(payload).into())) {
            return Ok(());
        }
        self.machine.push(local, broadcast, &payload[HEADER_LEN..])
    }

    /// Advance as far as possible with the messages received so far.
    /// Returns the output exactly once; outgoing messages produced along the
    /// way are collected for [`Self::take_outgoing`].
    pub fn proceed(&mut self) -> Result<Option<O>> {
        if self.finished {
            return Err(FrostError::InvalidState(
                "ECDSA ceremony already finished".into(),
            ));
        }
        let mut produced = Vec::new();
        let result = self.machine.run(&mut produced);
        for (to, body) in produced {
            let (recipient, kind) = match to {
                None => (Recipient::Broadcast, KIND_BROADCAST),
                Some(j) => {
                    let index = *self.parties.get(usize::from(j)).ok_or_else(|| {
                        FrostError::EcdsaError(format!("protocol addressed unknown party {j}"))
                    })?;
                    (Recipient::P2P(index), KIND_P2P)
                }
            };
            let mut payload = Vec::with_capacity(HEADER_LEN + body.len());
            payload.extend_from_slice(&[WIRE_VERSION, self.protocol.wire_tag(), kind]);
            payload.extend_from_slice(&self.eid);
            payload.extend_from_slice(&body);
            self.outbox.push(OutgoingMessage { recipient, payload });
        }
        let output = result?;
        if output.is_some() {
            self.finished = true;
        }
        Ok(output)
    }

    /// Drain the messages to send (in production order).
    pub fn take_outgoing(&mut self) -> Vec<OutgoingMessage> {
        std::mem::take(&mut self.outbox)
    }

    fn local_index(&self, keygen_index: u16) -> Option<u16> {
        self.parties
            .iter()
            .position(|&p| p == keygen_index)
            .and_then(|j| u16::try_from(j).ok())
    }
}
