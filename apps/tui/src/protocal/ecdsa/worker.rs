//! Runs ECDSA ceremonies on a dedicated OS thread.
//!
//! [`EcdsaCeremony`] is `!Send` (round_based's state machine shares state
//! through `Rc`) and its `proceed()` calls are CPU-heavy (Paillier / ZK
//! proofs: seconds per round), so a ceremony never touches the tokio
//! runtime: [`spawn`] creates a thread that builds the ceremony, feeds it
//! the inbound payloads it receives on a std channel, and hands outgoing
//! payloads and the result back to async code over tokio channels.

use starlab_core::ecdsa::{
    AuxInfo, EcdsaCeremony, EcdsaKeyShare, EcdsaSignature, Primes, Protocol, Recipient,
    execution_id,
};
use starlab_core::hd_derivation::DerivationPath;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tracing::{debug, info, warn};

use super::primes::PrimeSupply;
use super::wire::execution_id_of;

/// How long a DKG waits for the background prime generator.
const PRIMES_WAIT: Duration = Duration::from_secs(15 * 60);

/// What a worker thread runs.
pub enum Job {
    /// Aux info, then keygen, among `participants` (sorted device ids;
    /// position = keygen index).
    Dkg {
        session_id: String,
        participants: Vec<String>,
        index: u16,
        threshold: u16,
        primes: PrimeSupply,
    },
    /// One signing by exactly `signers` (keygen indices, us included).
    Sign {
        session_id: String,
        share: Box<EcdsaKeyShare>,
        signers: Vec<u16>,
        path: DerivationPath,
        prehash: [u8; 32],
    },
}

pub enum Outcome {
    Dkg(Box<EcdsaKeyShare>),
    Sign(EcdsaSignature),
}

/// A payload for the transport: deliver verbatim to each device in `to`.
#[derive(Debug)]
pub struct Outgoing {
    pub to: Vec<String>,
    pub payload: Vec<u8>,
}

/// Progress notes for the UI.
pub type StatusSender = UnboundedSender<String>;

/// Inbound side of a running worker: `(sender device id, payload)`.
pub type Inbox = Sender<(String, Vec<u8>)>;

/// Start `job` on its own thread. Returns the inbox to feed with the
/// peers' payloads; the result arrives on `done`. Dropping every clone of
/// the inbox cancels the ceremony. `timeout` bounds the ceremony itself
/// (not the wait for primes).
pub fn spawn(
    job: Job,
    timeout: Duration,
    out: UnboundedSender<Outgoing>,
    status: StatusSender,
    done: oneshot::Sender<Result<Outcome, String>>,
) -> std::io::Result<Inbox> {
    let (inbox, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("ecdsa-ceremony".into())
        .spawn(move || {
            let result = run(job, timeout, &rx, &out, &status);
            let _ = done.send(result);
        })?;
    Ok(inbox)
}

fn run(
    job: Job,
    timeout: Duration,
    rx: &Receiver<(String, Vec<u8>)>,
    out: &UnboundedSender<Outgoing>,
    status: &StatusSender,
) -> Result<Outcome, String> {
    match job {
        Job::Dkg {
            session_id,
            participants,
            index,
            threshold,
            primes,
        } => {
            if !primes.is_ready() {
                let _ = status.send(
                    "Generating ECDSA safe primes for the Ethereum key (first run can take a \
                     few minutes)…"
                        .to_string(),
                );
            }
            let primes: Primes = primes
                .take(PRIMES_WAIT)
                .ok_or("timed out waiting for ECDSA safe primes")?;
            let deadline = Instant::now() + timeout;
            let rng = starlab_core::rng::os_rng();
            let keygen_eid = execution_id(&session_id, Protocol::Keygen, &participants);
            let mut stash = Vec::new();
            let started = Instant::now();
            let aux: AuxInfo = drive(
                EcdsaCeremony::aux_info(&session_id, &participants, index, primes, rng)
                    .map_err(|e| e.to_string())?,
                &participants,
                rx,
                out,
                &mut stash,
                &[keygen_eid],
                deadline,
            )?;
            info!(
                "ECDSA aux info done in {:.1}s",
                started.elapsed().as_secs_f64()
            );
            let incomplete = drive(
                EcdsaCeremony::keygen(
                    &session_id,
                    &participants,
                    index,
                    threshold,
                    starlab_core::rng::os_rng(),
                )
                .map_err(|e| e.to_string())?,
                &participants,
                rx,
                out,
                &mut stash,
                &[],
                deadline,
            )?;
            let share = EcdsaKeyShare::from_parts(participants, incomplete, aux)
                .map_err(|e| e.to_string())?;
            info!(
                "ECDSA keygen done in {:.1}s total",
                started.elapsed().as_secs_f64()
            );
            Ok(Outcome::Dkg(Box::new(share)))
        }
        Job::Sign {
            session_id,
            share,
            signers,
            path,
            prehash,
        } => {
            let deadline = Instant::now() + timeout;
            let participants = share.participants().to_vec();
            let ceremony = EcdsaCeremony::signing(
                &session_id,
                &share,
                &signers,
                &path,
                prehash,
                starlab_core::rng::os_rng(),
            )
            .map_err(|e| e.to_string())?;
            let sig = drive(
                ceremony,
                &participants,
                rx,
                out,
                &mut Vec::new(),
                &[],
                deadline,
            )?;
            Ok(Outcome::Sign(sig))
        }
    }
}

/// Run one ceremony to completion. Payloads for `later` executions (the
/// next ceremony of this job) are stashed for it; payloads of any other
/// execution (a finished or foreign ceremony, a late resend) are dropped.
fn drive<O>(
    mut ceremony: EcdsaCeremony<O>,
    participants: &[String],
    rx: &Receiver<(String, Vec<u8>)>,
    out: &UnboundedSender<Outgoing>,
    stash: &mut Vec<(String, Vec<u8>)>,
    later: &[[u8; 32]],
    deadline: Instant,
) -> Result<O, String> {
    let eid = *ceremony.execution_id();
    let (mine, keep): (Vec<_>, Vec<_>) = std::mem::take(stash)
        .into_iter()
        .partition(|(_, p)| execution_id_of(p) == Some(eid));
    *stash = keep;
    for (from, payload) in mine {
        feed(&mut ceremony, participants, &from, &payload);
    }
    loop {
        let result = ceremony.proceed().map_err(|e| e.to_string());
        flush(&mut ceremony, participants, out)?;
        if let Some(output) = result? {
            return Ok(output);
        }
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| format!("ECDSA {:?} timed out", ceremony.protocol()))?;
        let (from, payload) = match rx.recv_timeout(left) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => {
                return Err(format!("ECDSA {:?} timed out", ceremony.protocol()));
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("ECDSA ceremony cancelled".into());
            }
        };
        match execution_id_of(&payload) {
            Some(e) if e == eid => feed(&mut ceremony, participants, &from, &payload),
            Some(e) if later.contains(&e) => stash.push((from, payload)),
            _ => debug!("ECDSA: dropping a payload from {from} for another execution"),
        }
    }
}

fn feed<O>(ceremony: &mut EcdsaCeremony<O>, participants: &[String], from: &str, payload: &[u8]) {
    let Some(index) = participants
        .iter()
        .position(|p| p == from)
        .and_then(|i| u16::try_from(i).ok())
    else {
        warn!("ECDSA: payload from {from}, who is not a participant; dropped");
        return;
    };
    // Invalid input (not a party of this ceremony, undecodable) is dropped:
    // the driver has not changed state, and a missing message ends in the
    // ceremony timeout rather than letting one bad frame abort it.
    if let Err(e) = ceremony.receive(index, payload) {
        warn!("ECDSA: payload from {from} refused: {e}");
    }
}

fn flush<O>(
    ceremony: &mut EcdsaCeremony<O>,
    participants: &[String],
    out: &UnboundedSender<Outgoing>,
) -> Result<(), String> {
    for m in ceremony.take_outgoing() {
        let to: Vec<String> = match m.recipient {
            Recipient::Broadcast => ceremony
                .parties()
                .iter()
                .filter(|&&p| p != ceremony.index())
                .filter_map(|&p| participants.get(usize::from(p)).cloned())
                .collect(),
            Recipient::P2P(p) => participants
                .get(usize::from(p))
                .cloned()
                .into_iter()
                .collect(),
        };
        out.send(Outgoing {
            to,
            payload: m.payload,
        })
        .map_err(|_| "ECDSA transport closed".to_string())?;
    }
    Ok(())
}
