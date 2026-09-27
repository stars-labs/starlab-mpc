//! Weak-network recovery for the peer-to-peer WebRTC mesh.
//!
//! The browser extension implements the same rules, so keep the two in step:
//!
//! 1. **Grace** — a peer connection that goes `Disconnected` gets
//!    [`DISCONNECT_GRACE`] to come back to `Connected` on its own. `Failed`,
//!    an unexpected `Closed`, or an expired grace rebuilds it.
//! 2. **Rebuild** — close and forget the peer connection and its data channel.
//!    The side with the smaller device id (the usual offerer) builds a fresh
//!    connection and sends a new offer; the other side waits for that offer.
//!    Rebuilds are rate-limited per peer ([`MIN_REBUILD_INTERVAL`] doubling to
//!    [`MAX_REBUILD_INTERVAL`]) and stop once the peer is no longer in the
//!    session.
//! 3. **Offer replacement** — an offer from a peer we already have a
//!    connection with replaces that connection (the peer rebuilt or
//!    restarted). Glare exception: when we are the offerer for the pair and our
//!    own offer is still unanswered, their offer is ignored.
//! 4. **Ceremony resend** — every ceremony frame sent to a peer is remembered
//!    until the ceremony completes, fails or a new one starts; when a data
//!    channel to that peer (re)opens, the remembered frames are sent again.
//!    Resends carry the exact bytes sent before (never a recomputed share, so
//!    FROST nonces are never reused), and receivers drop exact duplicates
//!    ([`SeenFrames`]).

use crate::elm::message::Message;
use crate::utils::appstate_compat::AppState;
use frost_core::{Ciphersuite, Field, Group};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};
use webrtc::data_channel::DataChannel;
use webrtc::peer_connection::{PeerConnection, RTCPeerConnectionState};

/// How long a `Disconnected` peer connection may take to recover by itself.
pub const DISCONNECT_GRACE: Duration = Duration::from_secs(8);
/// Minimum spacing between two rebuilds of the same peer.
pub const MIN_REBUILD_INTERVAL: Duration = Duration::from_secs(5);
/// Upper bound of the per-peer rebuild backoff.
pub const MAX_REBUILD_INTERVAL: Duration = Duration::from_secs(30);
/// A rebuilt connection we offered on that is not `Connected` after this long
/// (e.g. the offer or answer was lost) is rebuilt again.
pub const OFFER_WATCHDOG: Duration = Duration::from_secs(20);

/// `SimpleMessage` prefixes of the frames that make up a DKG, signing or
/// reshare ceremony. Only these are remembered for resend and de-duplicated.
pub const CEREMONY_FRAME_PREFIXES: &[&str] = &[
    "DKG_ROUND1:",
    "DKG_ROUND2:",
    crate::protocal::signing::SIGN_COMMIT_PREFIX,
    crate::protocal::signing::SIGN_SHARE_PREFIX,
    "RESHARE_ROUND1:",
    "RESHARE_ROUND2:",
    crate::elm::command::UNIFIED_DKG_ROUND1_PREFIX,
    crate::elm::command::UNIFIED_DKG_ROUND2_PREFIX,
];

pub fn is_ceremony_frame(text: &str) -> bool {
    CEREMONY_FRAME_PREFIXES.iter().any(|p| text.starts_with(p))
}

/// The side with the smaller device id offers (same rule as the initial mesh
/// setup in `network::webrtc::initiate_webrtc_with_channel`).
pub fn we_offer(self_device_id: &str, peer: &str) -> bool {
    self_device_id < peer
}

#[derive(Debug, PartialEq, Eq)]
pub enum StateAction {
    CancelGrace,
    StartGrace,
    Rebuild,
    Nothing,
}

/// Rule 1: what a connection-state change of the *current* peer connection
/// asks for. (Events from a connection we already replaced never get here.)
pub fn action_for_state(state: RTCPeerConnectionState) -> StateAction {
    match state {
        RTCPeerConnectionState::Connected => StateAction::CancelGrace,
        RTCPeerConnectionState::Disconnected => StateAction::StartGrace,
        RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed => StateAction::Rebuild,
        _ => StateAction::Nothing,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum OfferDecision {
    /// No connection for this peer yet: build one and answer.
    Accept,
    /// Close the existing connection, build a new one and answer.
    Replace,
    /// Glare: our own offer to this peer is still outstanding and we are the
    /// designated offerer, so theirs loses.
    IgnoreGlare,
}

/// Rule 3. `existing` is `None` without a connection for the peer, otherwise
/// whether that connection has an unanswered local offer (`have-local-offer`).
pub fn offer_decision(self_device_id: &str, peer: &str, existing: Option<bool>) -> OfferDecision {
    match existing {
        None => OfferDecision::Accept,
        Some(true) if we_offer(self_device_id, peer) => OfferDecision::IgnoreGlare,
        Some(_) => OfferDecision::Replace,
    }
}

/// Per-peer grace timer + rebuild rate limiter.
#[derive(Debug, Default)]
pub struct PeerRecovery {
    grace_epoch: u64,
    last_rebuild: Option<Instant>,
    interval: Duration,
    rebuild_pending: bool,
}

impl PeerRecovery {
    /// Arm a grace timer; the returned epoch identifies it.
    pub fn start_grace(&mut self) -> u64 {
        self.grace_epoch += 1;
        self.grace_epoch
    }

    pub fn cancel_grace(&mut self) {
        self.grace_epoch += 1;
    }

    /// Whether the grace timer armed as `epoch` is still the live one.
    pub fn grace_armed(&self, epoch: u64) -> bool {
        self.grace_epoch == epoch
    }

    /// The connection is up again: cancel the grace timer and reset the
    /// backoff.
    pub fn connected(&mut self) {
        self.cancel_grace();
        self.last_rebuild = None;
        self.interval = Duration::ZERO;
    }

    /// Ask for a rebuild at `now`. `None` when one is already scheduled;
    /// otherwise how long to wait before calling [`Self::begin_rebuild`].
    pub fn request_rebuild(&mut self, now: Instant) -> Option<Duration> {
        if self.rebuild_pending {
            return None;
        }
        self.rebuild_pending = true;
        Some(match self.last_rebuild {
            None => Duration::ZERO,
            Some(last) => (last + self.interval).saturating_duration_since(now),
        })
    }

    /// Record that a rebuild runs now and widen the backoff for the next one.
    pub fn begin_rebuild(&mut self, now: Instant) {
        self.rebuild_pending = false;
        self.interval = if self.last_rebuild.is_none() {
            MIN_REBUILD_INTERVAL
        } else {
            (self.interval * 2).clamp(MIN_REBUILD_INTERVAL, MAX_REBUILD_INTERVAL)
        };
        self.last_rebuild = Some(now);
    }
}

/// Ceremony frames sent per peer, as the exact serialized envelopes.
#[derive(Debug, Default)]
pub struct CeremonyOutbox {
    frames: HashMap<String, Vec<String>>,
}

impl CeremonyOutbox {
    pub fn record(&mut self, peer: &str, frame: &str) {
        let frames = self.frames.entry(peer.to_string()).or_default();
        if !frames.iter().any(|f| f == frame) {
            frames.push(frame.to_string());
        }
    }

    pub fn frames_for(&self, peer: &str) -> Vec<String> {
        self.frames.get(peer).cloned().unwrap_or_default()
    }

    pub fn clear(&mut self) {
        self.frames.clear();
    }
}

/// Ceremony frames already delivered to the protocol layer, keyed by sender +
/// payload. FROST packages are fresh randomness every ceremony, so an exact
/// repeat is always a resend — including a late resend of a ceremony that has
/// already finished, which would otherwise pollute the next one.
#[derive(Debug, Default)]
pub struct SeenFrames {
    seen: HashSet<u64>,
}

impl SeenFrames {
    const CAPACITY: usize = 4096;

    /// `true` the first time `(from, text)` is seen.
    pub fn first_delivery(&mut self, from: &str, text: &str) -> bool {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (from, text).hash(&mut hasher);
        if self.seen.len() >= Self::CAPACITY {
            self.seen.clear();
        }
        self.seen.insert(hasher.finish())
    }
}

// ---------------------------------------------------------------------------
// Async orchestration
// ---------------------------------------------------------------------------

/// Identity of one peer connection, filled in right after it is built. Lets
/// its event handler tell whether it still is the connection on record for the
/// peer, so events from a replaced connection are ignored.
pub type PcSlot = Arc<OnceLock<Weak<dyn PeerConnection>>>;

fn same_pc(pc: &Arc<dyn PeerConnection>, slot: &PcSlot) -> bool {
    slot.get()
        .is_some_and(|weak| std::ptr::addr_eq(Arc::as_ptr(pc), weak.as_ptr()))
}

async fn device_connections<C: Ciphersuite>(
    app_state: &Arc<Mutex<AppState<C>>>,
) -> Arc<Mutex<HashMap<String, Arc<dyn PeerConnection>>>> {
    app_state.lock().await.device_connections.clone()
}

/// Whether the connection in `slot` is the one on record for `peer`.
pub async fn is_current<C: Ciphersuite>(
    app_state: &Arc<Mutex<AppState<C>>>,
    peer: &str,
    slot: &PcSlot,
) -> bool {
    let conns = device_connections(app_state).await;
    let conns = conns.lock().await;
    conns.get(peer).is_some_and(|pc| same_pc(pc, slot))
}

/// Rules 1 + 2 for a state change of the current connection to `peer`.
pub async fn handle_state_change<C>(
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: UnboundedSender<Message>,
    peer: String,
    slot: PcSlot,
    state: RTCPeerConnectionState,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    match action_for_state(state) {
        StateAction::CancelGrace => {
            let mut guard = app_state.lock().await;
            guard
                .peer_recovery
                .entry(peer.clone())
                .or_default()
                .connected();
        }
        StateAction::StartGrace => {
            let epoch = {
                let mut guard = app_state.lock().await;
                guard
                    .peer_recovery
                    .entry(peer.clone())
                    .or_default()
                    .start_grace()
            };
            info!(
                "⏳ {} disconnected; waiting {:?} for it to recover",
                peer, DISCONNECT_GRACE
            );
            tokio::spawn(async move {
                tokio::time::sleep(DISCONNECT_GRACE).await;
                let still_down = {
                    let guard = app_state.lock().await;
                    guard
                        .peer_recovery
                        .get(&peer)
                        .is_some_and(|r| r.grace_armed(epoch))
                        && guard.device_statuses.get(&peer)
                            != Some(&RTCPeerConnectionState::Connected)
                };
                if still_down && is_current(&app_state, &peer, &slot).await {
                    warn!("⚠️ {} did not recover within the grace period", peer);
                    schedule_rebuild(app_state, tx_msg, peer);
                }
            });
        }
        StateAction::Rebuild => {
            {
                let mut guard = app_state.lock().await;
                guard
                    .peer_recovery
                    .entry(peer.clone())
                    .or_default()
                    .cancel_grace();
            }
            schedule_rebuild(app_state, tx_msg, peer);
        }
        StateAction::Nothing => {}
    }
}

/// Queue a rebuild of `peer`, honouring the per-peer rate limit. A plain fn
/// that spawns, so the rebuild → watchdog → rebuild chain is not a recursive
/// async type.
pub fn schedule_rebuild<C>(
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: UnboundedSender<Message>,
    peer: String,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    tokio::spawn(async move {
        let wait = {
            let mut guard = app_state.lock().await;
            guard
                .peer_recovery
                .entry(peer.clone())
                .or_default()
                .request_rebuild(Instant::now())
        };
        let Some(wait) = wait else {
            return; // one is already scheduled
        };
        info!("🔁 Rebuilding connection to {} in {:?}", peer, wait);
        tokio::time::sleep(wait).await;
        rebuild_peer(app_state, tx_msg, peer).await;
    });
}

/// Rule 2: drop the connection to `peer` and, if we are its offerer, build a
/// fresh one and send a new offer.
async fn rebuild_peer<C>(
    app_state: Arc<Mutex<AppState<C>>>,
    tx_msg: UnboundedSender<Message>,
    peer: String,
) where
    C: Ciphersuite + Send + Sync + 'static,
    <<C as Ciphersuite>::Group as Group>::Element: Send + Sync,
    <<<C as Ciphersuite>::Group as Group>::Field as Field>::Scalar: Send + Sync,
{
    let (self_device_id, in_session, conns) = {
        let mut guard = app_state.lock().await;
        guard
            .peer_recovery
            .entry(peer.clone())
            .or_default()
            .begin_rebuild(Instant::now());
        let in_session = guard
            .session
            .as_ref()
            .is_some_and(|s| s.participants.contains(&peer));
        (
            guard.device_id.clone(),
            in_session,
            guard.device_connections.clone(),
        )
    };

    teardown_peer(&app_state, &tx_msg, &peer, true).await;
    if !in_session {
        info!("🛑 {} is no longer in the session; not reconnecting", peer);
        app_state.lock().await.peer_recovery.remove(&peer);
        return;
    }
    if !we_offer(&self_device_id, &peer) {
        info!(
            "🔁 Dropped connection to {}; waiting for its new offer",
            peer
        );
        return;
    }

    info!("🔁 Re-offering to {}", peer);
    crate::network::webrtc::initiate_webrtc_with_channel(
        self_device_id,
        vec![peer.clone()],
        conns.clone(),
        app_state.clone(),
        Some(tx_msg.clone()),
    )
    .await;

    // If the offer or its answer is lost the new connection never changes
    // state, so nothing else would notice. Check back and rebuild again.
    let Some(new_pc) = conns.lock().await.get(&peer).cloned() else {
        return;
    };
    tokio::spawn(async move {
        tokio::time::sleep(OFFER_WATCHDOG).await;
        let still_ours = conns
            .lock()
            .await
            .get(&peer)
            .is_some_and(|pc| Arc::ptr_eq(pc, &new_pc));
        let connected = app_state.lock().await.device_statuses.get(&peer)
            == Some(&RTCPeerConnectionState::Connected);
        if still_ours && !connected {
            warn!(
                "⚠️ Re-offered connection to {} not up after {:?}",
                peer, OFFER_WATCHDOG
            );
            schedule_rebuild(app_state, tx_msg, peer);
        }
    });
}

/// Close and forget the peer connection + data channel for `peer`.
/// `drop_pending_ice` also discards buffered remote candidates (kept when an
/// incoming offer triggered the teardown: they may belong to that offer).
async fn teardown_peer<C>(
    app_state: &Arc<Mutex<AppState<C>>>,
    tx_msg: &UnboundedSender<Message>,
    peer: &str,
    drop_pending_ice: bool,
) where
    C: Ciphersuite + Send + Sync + 'static,
{
    let conns = {
        let mut guard = app_state.lock().await;
        guard.data_channels.remove(peer);
        guard.device_statuses.remove(peer);
        guard.remote_description_set.remove(peer);
        if drop_pending_ice {
            guard.pending_ice_candidates.remove(peer);
        }
        guard.device_connections.clone()
    };
    let old = conns.lock().await.remove(peer);
    let _ = tx_msg.send(Message::UpdateParticipantWebRTCStatus {
        device_id: peer.to_string(),
        webrtc_connected: false,
        data_channel_open: false,
    });
    if let Some(old) = old {
        // Never close from inside an event handler; this runs on its own task.
        tokio::spawn(async move {
            let _ = old.close().await;
        });
    }
}

/// Rule 3, run before answering an offer from `peer`. Returns whether the
/// offer should be answered; on replacement the old connection is already
/// gone so the answer path builds a fresh one.
pub async fn prepare_for_offer<C>(
    app_state: &Arc<Mutex<AppState<C>>>,
    tx_msg: &UnboundedSender<Message>,
    self_device_id: &str,
    peer: &str,
) -> bool
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let existing = device_connections(app_state)
        .await
        .lock()
        .await
        .get(peer)
        .cloned();
    let mid_negotiation = match &existing {
        Some(pc) => Some(pc.pending_local_description().await.is_some()),
        None => None,
    };
    match offer_decision(self_device_id, peer, mid_negotiation) {
        OfferDecision::Accept => true,
        OfferDecision::IgnoreGlare => {
            info!(
                "↔️ Glare with {}: our offer is outstanding and we are the offerer; ignoring theirs",
                peer
            );
            false
        }
        OfferDecision::Replace => {
            info!(
                "🔁 New offer from {}; replacing the existing connection",
                peer
            );
            teardown_peer(app_state, tx_msg, peer, false).await;
            true
        }
    }
}

/// Rule 4: a data channel to `peer` just opened; resend the ceremony frames
/// we already sent it.
pub async fn resend_ceremony_frames<C: Ciphersuite>(
    app_state: &Arc<Mutex<AppState<C>>>,
    peer: &str,
    dc: &Arc<dyn DataChannel>,
) {
    let frames = app_state.lock().await.ceremony_outbox.frames_for(peer);
    if frames.is_empty() {
        return;
    }
    info!(
        "📮 Resending {} ceremony frame(s) to {} on the reopened channel",
        frames.len(),
        peer
    );
    for frame in frames {
        if let Err(e) = dc.send_text(&frame).await {
            warn!("❌ Resend to {} failed: {}", peer, e);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_starts_grace_and_connected_cancels_it() {
        assert_eq!(
            action_for_state(RTCPeerConnectionState::Disconnected),
            StateAction::StartGrace
        );
        assert_eq!(
            action_for_state(RTCPeerConnectionState::Connected),
            StateAction::CancelGrace
        );
    }

    #[test]
    fn failed_or_closed_rebuilds() {
        assert_eq!(
            action_for_state(RTCPeerConnectionState::Failed),
            StateAction::Rebuild
        );
        assert_eq!(
            action_for_state(RTCPeerConnectionState::Closed),
            StateAction::Rebuild
        );
        assert_eq!(
            action_for_state(RTCPeerConnectionState::Connecting),
            StateAction::Nothing
        );
    }

    #[test]
    fn reconnect_within_grace_disarms_the_timer() {
        let mut r = PeerRecovery::default();
        let epoch = r.start_grace();
        assert!(r.grace_armed(epoch));
        r.connected();
        assert!(!r.grace_armed(epoch));
    }

    #[test]
    fn a_new_disconnect_supersedes_the_old_grace_timer() {
        let mut r = PeerRecovery::default();
        let first = r.start_grace();
        let second = r.start_grace();
        assert!(!r.grace_armed(first));
        assert!(r.grace_armed(second));
    }

    #[test]
    fn first_rebuild_is_immediate_then_backs_off_to_the_cap() {
        let mut r = PeerRecovery::default();
        let t0 = Instant::now();
        assert_eq!(r.request_rebuild(t0), Some(Duration::ZERO));
        r.begin_rebuild(t0);
        let mut waits = Vec::new();
        let mut now = t0;
        for _ in 0..5 {
            let wait = r.request_rebuild(now).unwrap();
            waits.push(wait.as_secs());
            now += wait;
            r.begin_rebuild(now);
        }
        assert_eq!(waits, vec![5, 10, 20, 30, 30]);
    }

    #[test]
    fn only_one_rebuild_is_scheduled_at_a_time() {
        let mut r = PeerRecovery::default();
        let now = Instant::now();
        assert!(r.request_rebuild(now).is_some());
        assert_eq!(r.request_rebuild(now), None);
    }

    #[test]
    fn a_working_connection_resets_the_backoff() {
        let mut r = PeerRecovery::default();
        let now = Instant::now();
        r.request_rebuild(now);
        r.begin_rebuild(now);
        r.connected();
        assert_eq!(r.request_rebuild(now), Some(Duration::ZERO));
    }

    #[test]
    fn smaller_id_offers() {
        assert!(we_offer("mpc-1", "mpc-2"));
        assert!(!we_offer("mpc-2", "mpc-1"));
    }

    #[test]
    fn offer_without_connection_is_accepted() {
        assert_eq!(offer_decision("b", "a", None), OfferDecision::Accept);
    }

    #[test]
    fn offer_replaces_an_established_connection() {
        assert_eq!(
            offer_decision("b", "a", Some(false)),
            OfferDecision::Replace
        );
        assert_eq!(
            offer_decision("a", "b", Some(false)),
            OfferDecision::Replace
        );
    }

    #[test]
    fn glare_offerer_keeps_its_own_pending_offer() {
        assert_eq!(
            offer_decision("a", "b", Some(true)),
            OfferDecision::IgnoreGlare
        );
    }

    #[test]
    fn glare_answerer_yields_to_the_peer_offer() {
        assert_eq!(offer_decision("b", "a", Some(true)), OfferDecision::Replace);
    }

    #[test]
    fn ceremony_frames_are_recognised_by_prefix() {
        assert!(is_ceremony_frame("DKG_ROUND1:abc"));
        assert!(is_ceremony_frame("SIGN_SHARE:abc"));
        assert!(is_ceremony_frame("RESHARE_ROUND2:abc"));
        assert!(!is_ceremony_frame("hello"));
    }

    #[test]
    fn outbox_keeps_frames_per_peer_without_duplicates() {
        let mut outbox = CeremonyOutbox::default();
        outbox.record("b", "f1");
        outbox.record("b", "f2");
        outbox.record("b", "f1");
        outbox.record("c", "f3");
        assert_eq!(outbox.frames_for("b"), vec!["f1", "f2"]);
        assert_eq!(outbox.frames_for("c"), vec!["f3"]);
    }

    #[test]
    fn cleared_outbox_resends_nothing() {
        let mut outbox = CeremonyOutbox::default();
        outbox.record("b", "f1");
        outbox.clear();
        assert!(outbox.frames_for("b").is_empty());
    }

    use crate::protocal::signal::WebRTCMessage;
    use frost_secp256k1_tr::Secp256K1Sha256TR as Secp;

    fn app_state_with_ws() -> (
        Arc<Mutex<AppState<Secp>>>,
        tokio::sync::mpsc::UnboundedReceiver<String>,
    ) {
        let (ws_tx, ws_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let mut state = AppState::<Secp>::new();
        state.websocket_msg_tx = Some(ws_tx);
        (Arc::new(Mutex::new(state)), ws_rx)
    }

    /// A frame sent while the peer's channel is down is still remembered for
    /// the resend; non-ceremony traffic is not.
    #[tokio::test]
    async fn ceremony_frames_are_remembered_even_when_the_send_fails() {
        let (app_state, _ws) = app_state_with_ws();
        let frame = WebRTCMessage::<Secp>::SimpleMessage {
            text: "SIGN_COMMIT:abc".into(),
        };
        let chatter = WebRTCMessage::<Secp>::SimpleMessage {
            text: "hello".into(),
        };
        assert!(
            crate::utils::device::send_webrtc_message("peer", &frame, app_state.clone())
                .await
                .is_err()
        );
        let _ =
            crate::utils::device::send_webrtc_message("peer", &chatter, app_state.clone()).await;
        let frames = app_state.lock().await.ceremony_outbox.frames_for("peer");
        assert_eq!(frames, vec![serde_json::to_string(&frame).unwrap()]);
    }

    /// A resent frame reaches the protocol layer only once.
    #[tokio::test]
    async fn resent_frame_is_dispatched_once() {
        let (app_state, _ws) = app_state_with_ws();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let frame = serde_json::to_vec(&WebRTCMessage::<Secp>::SimpleMessage {
            text: "SIGN_COMMIT:AAAA".into(),
        })
        .unwrap();
        for _ in 0..2 {
            crate::network::webrtc::dispatch_data_channel_msg::<Secp>(
                frame.clone(),
                "peer".into(),
                app_state.clone(),
                Some(tx.clone()),
            )
            .await;
        }
        let mut commits = 0;
        while let Ok(msg) = rx.try_recv() {
            if matches!(msg, Message::ProcessSigningRound1 { .. }) {
                commits += 1;
            }
        }
        assert_eq!(commits, 1);
    }

    /// Build a connection to `peer` with an outstanding local offer, as the
    /// offerer path leaves it.
    async fn pc_with_pending_offer(
        app_state: &Arc<Mutex<AppState<Secp>>>,
        self_id: &str,
        peer: &str,
    ) {
        let conns = app_state.lock().await.device_connections.clone();
        crate::network::webrtc::initiate_webrtc_with_channel(
            self_id.to_string(),
            vec![peer.to_string()],
            conns,
            app_state.clone(),
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn glare_offerer_ignores_the_peer_offer_and_keeps_its_connection() {
        let (app_state, _ws) = app_state_with_ws();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        pc_with_pending_offer(&app_state, "a", "b").await;
        assert!(!prepare_for_offer(&app_state, &tx, "a", "b").await);
        let conns = app_state.lock().await.device_connections.clone();
        assert!(conns.lock().await.contains_key("b"));
    }

    #[tokio::test]
    async fn offer_for_an_existing_connection_replaces_it() {
        let (app_state, _ws) = app_state_with_ws();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        pc_with_pending_offer(&app_state, "a", "b").await;
        app_state
            .lock()
            .await
            .remote_description_set
            .insert("b".into());
        // Seen from a device whose id is larger than the peer's, so it is the
        // answerer for the pair and must take the peer's offer.
        assert!(prepare_for_offer(&app_state, &tx, "c", "b").await);
        let guard = app_state.lock().await;
        assert!(!guard.remote_description_set.contains("b"));
        assert!(!guard.device_connections.lock().await.contains_key("b"));
    }

    #[test]
    fn duplicate_frame_from_same_sender_is_dropped() {
        let mut seen = SeenFrames::default();
        assert!(seen.first_delivery("b", "SIGN_COMMIT:x"));
        assert!(!seen.first_delivery("b", "SIGN_COMMIT:x"));
        assert!(seen.first_delivery("c", "SIGN_COMMIT:x"));
    }
}
