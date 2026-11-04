use super::*;
use crate::protocal::signal::{SessionInfo, SessionType};
use frost_ed25519::Ed25519Sha512;

type State = Arc<Mutex<AppState<Ed25519Sha512>>>;
// Deliberately not sorted: wire addressing must use canonical identities.
const PEERS: [&str; 3] = ["charlie", "alice", "bob"];

fn new_state(peer: &str) -> State {
    let mut state = AppState::<Ed25519Sha512>::new();
    state.device_id = peer.into();
    state.dkg_in_progress = true;
    state.unified_mode = true;
    state.session = Some(SessionInfo {
        session_id: "private-round2-fixture".into(),
        proposer_id: "alice".into(),
        total: 3,
        threshold: 2,
        participants: PEERS.iter().map(|p| p.to_string()).collect(),
        session_type: SessionType::DKG,
        curve_type: "unified".into(),
        coordination_type: "Network".into(),
        signing_message_hex: None,
    });
    Arc::new(Mutex::new(state))
}

fn copy_round1(package: &UnifiedRound1Package) -> UnifiedRound1Package {
    serde_json::from_str(&serde_json::to_string(package).unwrap()).unwrap()
}

async fn round2_fixture() -> (Vec<State>, Vec<UnifiedRound2Message>) {
    let mut states = Vec::new();
    let mut round1 = Vec::new();
    for peer in PEERS {
        let state = new_state(peer);
        round1.push(start_round1(state.clone(), peer.into()).await.unwrap());
        states.push(state);
    }
    for (receiver, state) in states.iter().enumerate() {
        for (sender, package) in round1.iter().enumerate() {
            if sender != receiver {
                let decoded =
                    serde_json::from_str(&serde_json::to_string(package).unwrap()).unwrap();
                ingest_round1(state.clone(), PEERS[sender].into(), decoded).await;
            }
        }
    }
    let mut round2 = Vec::new();
    for state in &states {
        round2.push(start_round2(state.clone()).await.unwrap());
    }
    (states, round2)
}

fn index(peer: &str) -> u16 {
    canonical_index(
        &PEERS.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
        peer,
    )
    .unwrap()
}

#[tokio::test]
async fn private_round2_frames_contain_only_their_recipient_and_complete_both_curves() {
    let (states, round2) = round2_fixture().await;
    for (sender, message) in round2.iter().enumerate() {
        assert!(message.for_recipient(index(PEERS[sender])).is_none());
        for (receiver, state) in states.iter().enumerate() {
            if receiver == sender {
                continue;
            }
            let recipient = index(PEERS[receiver]);
            let targeted = message.for_recipient(recipient).unwrap();
            let wire = serde_json::to_string(&targeted).unwrap();
            let decoded: UnifiedRound2Message = serde_json::from_str(&wire).unwrap();
            assert!(decoded.ed25519.keys().copied().eq([recipient]));
            assert!(decoded.secp256k1.keys().copied().eq([recipient]));
            // Boolean assertions avoid dumping private packages on failure.
            assert!(decoded.ed25519.get(&recipient) == message.ed25519.get(&recipient));
            assert!(decoded.secp256k1.get(&recipient) == message.secp256k1.get(&recipient));
            ingest_round2(state.clone(), PEERS[sender].into(), decoded).await;
        }
    }
    let mut keys = Vec::new();
    for state in states {
        assert!(can_finalize(state.clone()).await);
        let mut state = state.lock().await;
        let dkg = state.unified_dkg.as_mut().unwrap();
        dkg.finalize_dkg().unwrap();
        keys.push((
            dkg.get_ed25519_group_public_key().unwrap(),
            dkg.get_secp256k1_group_public_key().unwrap(),
        ));
    }
    assert!(keys.iter().all(|key| key == &keys[0]));
}

#[tokio::test]
async fn two_of_three_dkg_still_needs_both_other_round2_contributions() {
    let (states, round2) = round2_fixture().await;
    let receiver = states[0].clone(); // charlie = canonical index 3
    assert!(
        !ingest_round2(
            receiver.clone(),
            "alice".into(),
            round2[1].for_recipient(3).unwrap()
        )
        .await
    );
    assert!(!can_finalize(receiver.clone()).await);
    assert!(
        ingest_round2(
            receiver.clone(),
            "bob".into(),
            round2[2].for_recipient(3).unwrap()
        )
        .await
    );
    assert!(can_finalize(receiver).await);
}

#[tokio::test]
async fn spoofed_unknown_self_or_overbroad_round2_cannot_advance_the_dkg() {
    let (states, round2) = round2_fixture().await;
    let receiver = states[0].clone();
    // Alice's valid private package delivered on Bob's or an outsider's channel.
    for sender in ["bob", "outsider"] {
        assert!(
            !ingest_round2(
                receiver.clone(),
                sender.into(),
                round2[1].for_recipient(3).unwrap()
            )
            .await
        );
    }
    let mut spoofed = round2[1].for_recipient(3).unwrap();
    spoofed.from_index = index("bob");
    assert!(!ingest_round2(receiver.clone(), "alice".into(), spoofed).await);
    let mut self_claim = round2[1].for_recipient(3).unwrap();
    self_claim.from_index = 3;
    assert!(!ingest_round2(receiver.clone(), "charlie".into(), self_claim).await);
    // Reject the old all-recipients broadcast format and a different target.
    let broadcast: UnifiedRound2Message =
        serde_json::from_str(&serde_json::to_string(&round2[1]).unwrap()).unwrap();
    assert!(!ingest_round2(receiver.clone(), "alice".into(), broadcast).await);
    assert!(
        !ingest_round2(
            receiver.clone(),
            "alice".into(),
            round2[1].for_recipient(2).unwrap()
        )
        .await
    );
    let mut incomplete = round2[1].for_recipient(3).unwrap();
    incomplete.secp256k1.clear();
    assert!(incomplete.for_recipient(3).is_none());
    assert!(!ingest_round2(receiver.clone(), "alice".into(), incomplete).await);
    // A real contribution from Bob alone must not become sufficient: none of
    // the invalid Alice/self messages above may have entered either curve.
    assert!(
        !ingest_round2(
            receiver.clone(),
            "bob".into(),
            round2[2].for_recipient(3).unwrap()
        )
        .await
    );
    assert!(!can_finalize(receiver.clone()).await);
    assert!(
        ingest_round2(
            receiver.clone(),
            "alice".into(),
            round2[1].for_recipient(3).unwrap()
        )
        .await
    );
    let mut state = receiver.lock().await;
    state.unified_dkg.as_mut().unwrap().finalize_dkg().unwrap();
}

#[tokio::test]
async fn early_round1_replays_through_start_command_and_early_part2_completes_without_network_retry()
 {
    use crate::elm::command::Command;
    use crate::utils::state::DkgState;
    let states: Vec<_> = PEERS.iter().map(|peer| new_state(peer)).collect();
    let mut packages = Vec::new();
    for i in 1..3 {
        packages.push(
            start_round1(states[i].clone(), PEERS[i].into())
                .await
                .unwrap(),
        );
        assert!(
            !ingest_round1(
                states[0].clone(),
                PEERS[i].into(),
                copy_round1(&packages[i - 1])
            )
            .await
        );
    }
    // Transport has already delivered/deduplicated these frames. Startup must
    // consume its local inbox and advance; no resend or reconnect is simulated.
    let storage = tempfile::tempdir().unwrap();
    states[0].lock().await.unified_finalize = Some((
        "fixture-password".into(),
        storage.path().to_string_lossy().into_owned(),
        None,
    ));
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    Command::StartFrostProtocol
        .execute(tx, &states[0])
        .await
        .unwrap();
    let (own_round1, own_round2) = {
        let state = states[0].lock().await;
        assert!(matches!(state.dkg_state, DkgState::Round2InProgress));
        assert!(state.pending_unified.round1.is_empty());
        let frames = state.ceremony_outbox.frames_for("alice");
        let text = |prefix: &str| -> String {
            frames
                .iter()
                .find_map(|frame| {
                    let json: serde_json::Value = serde_json::from_str(frame).unwrap();
                    json["text"]
                        .as_str()?
                        .strip_prefix(prefix)
                        .map(str::to_owned)
                })
                .expect("both round1 and round2 must remain in the replay outbox")
        };
        (
            serde_json::from_str::<UnifiedRound1Package>(&text("UNIFIED_DKG_ROUND1:")).unwrap(),
            serde_json::from_str::<UnifiedRound2Message>(&text("UNIFIED_DKG_ROUND2:")).unwrap(),
        )
    };
    let mut peer_round2 = std::collections::BTreeMap::<usize, UnifiedRound2Message>::new();
    for i in [2, 1] {
        ingest_round1(states[i].clone(), PEERS[0].into(), copy_round1(&own_round1)).await;
        let other = if i == 1 { 2 } else { 1 };
        ingest_round1(
            states[i].clone(),
            PEERS[other].into(),
            copy_round1(&packages[other - 1]),
        )
        .await;
        if i == 1 {
            // Both remote round2 packages arrive before Alice runs local part2.
            ingest_round2(
                states[1].clone(),
                "charlie".into(),
                own_round2.for_recipient(1).unwrap(),
            )
            .await;
            ingest_round2(
                states[1].clone(),
                "bob".into(),
                peer_round2[&2].for_recipient(1).unwrap(),
            )
            .await;
            assert!(!can_finalize(states[1].clone()).await);
        }
        peer_round2.insert(i, start_round2(states[i].clone()).await.unwrap());
        if i == 1 {
            assert!(can_finalize(states[1].clone()).await);
        }
    }
    for i in 1..3 {
        ingest_round2(
            states[0].clone(),
            PEERS[i].into(),
            peer_round2[&i].for_recipient(3).unwrap(),
        )
        .await;
    }
    assert!(can_finalize(states[0].clone()).await);
    // A repeated delivery cannot regenerate local part2 / erase round2 state.
    assert!(start_round2(states[0].clone()).await.is_none());
    let mut state = states[0].lock().await;
    state.unified_dkg.as_mut().unwrap().finalize_dkg().unwrap();
}

#[tokio::test]
async fn preinit_inbox_is_authenticated_bounded_and_cannot_cross_sessions() {
    let (states, packages) = round2_fixture().await;
    let receiver = states[0].clone();
    {
        let mut state = receiver.lock().await;
        state.unified_dkg = None;
        state.dkg_state = crate::utils::state::DkgState::Idle;
    }
    for _ in 0..3 {
        ingest_round2(
            receiver.clone(),
            "alice".into(),
            packages[1].for_recipient(3).unwrap(),
        )
        .await;
        ingest_round2(
            receiver.clone(),
            "outsider".into(),
            packages[1].for_recipient(3).unwrap(),
        )
        .await;
        ingest_round2(
            receiver.clone(),
            "bob".into(),
            packages[1].for_recipient(3).unwrap(),
        )
        .await;
        ingest_round2(
            receiver.clone(),
            "alice".into(),
            packages[1].for_recipient(2).unwrap(),
        )
        .await;
    }
    assert_eq!(receiver.lock().await.pending_unified.round2.len(), 1);
    let pending = take_pending(receiver.clone()).await;
    assert_eq!(pending.round2.len(), 1);
    assert!(pending.round2.contains_key("alice"));
    // A taken batch still carries its session tag: switching AFTER take must
    // not import old frames into the next ceremony, even with identical peers.
    receiver.lock().await.session.as_mut().unwrap().session_id = "replacement-session".into();
    replay_pending(receiver.clone(), pending).await;
    assert!(receiver.lock().await.pending_unified.round2.is_empty());
    receiver.lock().await.session.as_mut().unwrap().session_id = "private-round2-fixture".into();
    ingest_round2(
        receiver.clone(),
        "alice".into(),
        packages[1].for_recipient(3).unwrap(),
    )
    .await;
    receiver.lock().await.session.as_mut().unwrap().session_id = "next-session".into();
    assert!(take_pending(receiver.clone()).await.round2.is_empty());
    ingest_round2(
        receiver.clone(),
        "alice".into(),
        packages[1].for_recipient(3).unwrap(),
    )
    .await;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    crate::elm::command::Command::CancelDKG {
        session_id: Some("next-session".into()),
    }
    .execute(tx, &receiver)
    .await
    .unwrap();
    assert!(receiver.lock().await.pending_unified.round2.is_empty());
    ingest_round2(
        receiver.clone(),
        "alice".into(),
        packages[1].for_recipient(3).unwrap(),
    )
    .await;
    assert!(receiver.lock().await.pending_unified.round2.is_empty());
}

#[tokio::test]
async fn headless_unified_joiner_prepares_explicit_storage_and_password_decrypts_saved_curves() {
    use crate::elm::{Message, Model, command::Command, update::update};
    let storage = tempfile::tempdir().unwrap();
    let password = "joiner-explicit-password";
    let (states, packages) = round2_fixture().await;
    let state = states[0].clone();
    let invite = state.lock().await.session.clone().unwrap();
    let mut model = Model::new("charlie".into());
    model.wallet_state.keystore_path = storage.path().to_string_lossy().into_owned();
    model.wallet_state.unified = false; // `serve` has no local unified selector.
    model.session_invites.push(invite.clone());
    update(
        &mut model,
        Message::HeadlessJoinSession {
            session_id: invite.session_id,
            password: password.into(),
            label: String::new(),
        },
    );
    update(
        &mut model,
        Message::SubmitPassword {
            value: password.into(),
        },
    );
    assert!(model.wallet_state.unified);
    let batch = update(&mut model, Message::StartDKGProtocol).unwrap();
    assert!(
        matches!(&batch, Command::Batch(commands) if matches!(commands.first(), Some(Command::PrepareUnifiedFinalize { keystore_path, password: pw, .. }) if keystore_path == storage.path().to_str().unwrap() && pw == password))
    );
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    batch.execute(tx, &state).await.unwrap();
    for i in 1..3 {
        ingest_round2(
            state.clone(),
            PEERS[i].into(),
            packages[i].for_recipient(3).unwrap(),
        )
        .await;
    }
    let (saved_password, path, label) = state.lock().await.unified_finalize.clone().unwrap();
    let outcome = finalize(
        state,
        "explicit-storage-wallet".into(),
        saved_password,
        path,
        label,
    )
    .await
    .unwrap();
    let keystore = crate::keystore::Keystore::new(storage.path(), "charlie").unwrap();
    for curve in ["ed25519", "secp256k1"] {
        assert!(
            storage
                .path()
                .join("charlie")
                .join(curve)
                .join("explicit-storage-wallet.json")
                .is_file()
        );
        let share = keystore
            .load_wallet_file_for_curve(&outcome.wallet_id, curve, password)
            .unwrap();
        assert!(!share.is_empty());
        assert!(
            keystore
                .load_wallet_file_for_curve(&outcome.wallet_id, curve, "")
                .is_err()
        );
        assert!(
            keystore
                .load_wallet_file_for_curve(&outcome.wallet_id, curve, "wrong-password")
                .is_err()
        );
    }
}
