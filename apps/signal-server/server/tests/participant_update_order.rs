//! `participant_update` frames must reach every device in the order the
//! server applied the joins. A client treats each update as the authoritative
//! roster, so a stale snapshot delivered after a newer one shrinks the session
//! back below `total` — the 3-of-5 DKG then waits forever for a peer it has
//! just dropped (CI: "DKG/SIGN 3-of-5 timed out after 160s").

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

const JOINERS: usize = 8;
const ROUNDS: usize = 25;

async fn start_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(starlab_signal_server::run(listener));
    format!("ws://{addr}")
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

fn is_update(v: &Value) -> bool {
    v["data"]["type"] == "participant_update"
}

fn roster_len(v: &Value) -> usize {
    v["data"]["session_info"]["participants"]
        .as_array()
        .map_or(0, Vec::len)
}

/// Many devices join one session at once; the creator must see the roster
/// grow monotonically (1 join = 1 update, never a smaller roster after a
/// larger one).
async fn concurrent_joins_deliver_updates_in_order(url: &str, round: usize) {
    let session_id = format!("s{round}");
    let (mut creator, _) = connect_async(url).await.unwrap();
    send(
        &mut creator,
        json!({"type": "register", "device_id": format!("creator-{round}")}),
    )
    .await;

    let mut joiners = Vec::new();
    for j in 0..JOINERS {
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(
            &mut ws,
            json!({"type": "register", "device_id": format!("joiner-{round}-{j}")}),
        )
        .await;
        joiners.push(ws);
    }
    // Every joiner is registered once its own `devices` list names it; only
    // then announce, so each one receives the live `session_available`.
    for (j, ws) in joiners.iter_mut().enumerate() {
        let me = format!("joiner-{round}-{j}");
        wait_for(ws, |v| {
            v["type"] == "devices"
                && v["devices"]
                    .as_array()
                    .is_some_and(|d| d.iter().any(|x| x == me.as_str()))
        })
        .await;
    }

    send(
        &mut creator,
        json!({"type": "announce_session", "session_info": {
            "session_id": session_id, "proposer_id": format!("creator-{round}"),
            "participants": [format!("creator-{round}")],
            "total": JOINERS + 1, "threshold": 2, "session_type": "dkg"
        }}),
    )
    .await;
    for ws in &mut joiners {
        wait_for(ws, |v| v["type"] == "session_available").await;
    }

    // All joins go out back to back so the server handles them concurrently.
    let sends = joiners.iter_mut().enumerate().map(|(j, ws)| {
        let frame = json!({"type": "session_status_update", "session_info": {
            "session_id": session_id,
            "participant_joined": format!("joiner-{round}-{j}")
        }});
        async move { send(ws, frame).await }
    });
    futures_util::future::join_all(sends).await;

    let mut seen = Vec::new();
    for _ in 0..JOINERS {
        seen.push(roster_len(&wait_for(&mut creator, is_update).await));
    }
    let expected: Vec<usize> = (2..=JOINERS + 1).collect();
    assert_eq!(
        seen, expected,
        "round {round}: participant_update rosters arrived out of order"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_joins_reach_the_creator_in_join_order() {
    let url = start_server().await;
    for round in 0..ROUNDS {
        concurrent_joins_deliver_updates_in_order(&url, round).await;
    }
}
