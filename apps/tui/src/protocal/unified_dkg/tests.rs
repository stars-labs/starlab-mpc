use super::*;
use crate::protocal::signal::{SessionInfo, SessionType};
use frost_ed25519::Ed25519Sha512;

type State = Arc<Mutex<AppState<Ed25519Sha512>>>;
// Deliberately not sorted: wire addressing must use canonical identities.
const PEERS: [&str; 3] = ["charlie", "alice", "bob"];

async fn round2_fixture() -> (Vec<State>, Vec<UnifiedRound2Message>) {
    let mut states = Vec::new();
    let mut round1 = Vec::new();
    for peer in PEERS {
        let mut state = AppState::<Ed25519Sha512>::new();
        state.device_id = peer.into();
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
        let state = Arc::new(Mutex::new(state));
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
