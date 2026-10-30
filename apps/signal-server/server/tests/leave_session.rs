//! `LeaveSession`: a creator withdrawing its invite removes the session for
//! everyone; a joiner leaving drops out of the participant list.

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(starlab_signal_server::run(listener));
    format!("ws://{addr}")
}

async fn client(url: &str, device_id: &str) -> Ws {
    let (mut ws, _) = connect_async(url).await.unwrap();
    send(&mut ws, json!({"type": "register", "device_id": device_id})).await;
    ws
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}

/// Read frames until one matches `pred`; panics after 5s.
async fn wait_for(ws: &mut Ws, pred: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let Some(Ok(Message::Text(t))) = ws.next().await else {
                continue;
            };
            let v: Value = serde_json::from_str(&t).unwrap();
            if pred(&v) {
                return v;
            }
        }
    })
    .await
    .expect("expected frame never arrived")
}

async fn announce_and_join(url: &str) -> (Ws, Ws) {
    let mut creator = client(url, "creator").await;
    let mut joiner = client(url, "joiner").await;
    send(
        &mut creator,
        json!({"type": "announce_session", "session_info": {
            "session_id": "s1", "proposer_id": "creator", "participants": ["creator"],
            "total": 3, "threshold": 2, "session_type": "dkg"
        }}),
    )
    .await;
    wait_for(&mut joiner, |v| v["type"] == "session_available").await;
    send(
        &mut joiner,
        json!({"type": "session_status_update", "session_info": {
            "session_id": "s1", "participant_joined": "joiner"
        }}),
    )
    .await;
    wait_for(&mut creator, |v| v["data"]["type"] == "participant_update").await;
    (creator, joiner)
}

#[tokio::test]
async fn creator_leaving_removes_session_for_everyone() {
    let url = start_server().await;
    let (mut creator, mut joiner) = announce_and_join(&url).await;

    send(
        &mut creator,
        json!({"type": "leave_session", "session_id": "s1"}),
    )
    .await;

    let removed = wait_for(&mut joiner, |v| v["type"] == "session_removed").await;
    assert_eq!(removed["session_id"], "s1");
}

#[tokio::test]
async fn joiner_leaving_is_dropped_from_participants() {
    let url = start_server().await;
    let (mut creator, mut joiner) = announce_and_join(&url).await;

    send(
        &mut joiner,
        json!({"type": "leave_session", "session_id": "s1"}),
    )
    .await;

    let update = wait_for(&mut creator, |v| v["data"]["type"] == "participant_update").await;
    assert_eq!(
        update["data"]["session_info"]["participants"],
        json!(["creator"])
    );
}

/// Completing an invite must stop restart replay without interrupting the
/// transport used by slower signers, or the next ceremony on the same mesh.
#[tokio::test]
async fn completed_invite_is_not_replayed_and_peers_can_keep_relaying() {
    let url = start_server().await;
    let (mut creator, mut joiner) = announce_and_join(&url).await;
    let mut outsider = client(&url, "outsider").await;
    wait_for(&mut creator, |v| {
        v["type"] == "devices"
            && v["devices"]
                .as_array()
                .is_some_and(|ds| ds.iter().any(|d| d == "outsider"))
    })
    .await;
    send(
        &mut creator,
        json!({"type": "leave_session", "session_id": "s1"}),
    )
    .await;
    wait_for(&mut joiner, |v| v["type"] == "session_removed").await;
    wait_for(&mut outsider, |v| v["type"] == "session_removed").await;
    send(&mut joiner, json!({"type": "query_my_active_sessions"})).await;
    let replay = wait_for(&mut joiner, |v| v["type"] == "sessions_for_device").await;
    assert_eq!(replay["sessions"], json!([]));
    // A late final-signature delivery to an outsider is still routable.
    send(
        &mut creator,
        json!({"type": "relay", "to": "outsider", "data": {"final_signature": "verified"}}),
    )
    .await;
    let delivered = wait_for(&mut outsider, |v| {
        v["data"]["final_signature"] == "verified"
    })
    .await;
    assert_eq!(delivered["from"], "creator");
    send(
        &mut creator,
        json!({"type": "announce_session", "session_info": {
            "session_id": "s2", "proposer_id": "creator", "participants": ["creator", "joiner"],
            "total": 3, "threshold": 2, "session_type": "signing"
        }}),
    )
    .await;
    let next = wait_for(&mut joiner, |v| v["type"] == "session_available").await;
    assert_eq!(next["session_info"]["session_id"], "s2");
    send(&mut joiner, json!({"type": "query_my_active_sessions"})).await;
    let replay = wait_for(&mut joiner, |v| v["type"] == "sessions_for_device").await;
    assert_eq!(replay["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(replay["sessions"][0]["session_id"], "s2");
}
