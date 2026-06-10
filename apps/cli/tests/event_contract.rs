//! L2 event-contract golden (Phase 2 of docs/cli-conformance-testing.md).
//!
//! Pins the normalized JSON *shape* of every `CliEvent` variant against a
//! checked-in golden. This is the spec the L4 differential oracle diffs the
//! browser extension's emitted protocol against; any accidental serde change
//! (rename, retag, field made optional, structural drift) breaks this test.
//!
//! Deterministic + offline → runs in the fast CI lane (not `#[ignore]`).
//!
//! Regenerate intentionally after a *reviewed* protocol change:
//!   BLESS=1 cargo test -p starlab-cli --test event_contract

use starlab_cli::protocol::{
    CliEvent, SessionEntry, WalletEntry, PROTOCOL_VERSION,
};
use starlab_cli::trace::normalize_event_line;

/// One representative instance of every `CliEvent` variant, in declaration
/// order. Volatile values are arbitrary — normalization redacts them — so this
/// list pins the *vocabulary and shape* of the protocol.
fn all_events() -> Vec<CliEvent> {
    vec![
        CliEvent::Ready {
            protocol: PROTOCOL_VERSION,
            device_id: "node-a".into(),
            curve: "secp256k1".into(),
        },
        CliEvent::Ack { correlates: 1 },
        CliEvent::Connection { connected: true },
        CliEvent::Status {
            connected: true,
            device_id: "node-a".into(),
            wallets: vec![sample_wallet()],
        },
        CliEvent::SessionAvailable { session: sample_session() },
        CliEvent::SessionAnnounced {
            correlates: Some(7),
            session_id: "dkg_x".into(),
        },
        CliEvent::Wallets { wallets: vec![sample_wallet()] },
        CliEvent::Sessions { sessions: vec![sample_session()] },
        CliEvent::DkgProgress {
            session_id: "dkg_x".into(),
            round: 1,
            received: 1,
            need: 2,
        },
        CliEvent::DkgComplete {
            correlates: Some(2),
            wallet_id: "wallet-dkg_x".into(),
            address: "0xabc".into(),
            group_public_key: "0299".into(),
        },
        CliEvent::SigningRequest {
            session_id: "sign_x".into(),
            wallet: "wallet-dkg_x".into(),
            threshold: 2,
            total: 3,
            proposer: "node-a".into(),
        },
        CliEvent::SignatureComplete {
            correlates: Some(3),
            signature: "0xdead".into(),
            message_hash: "0xbeef".into(),
        },
        CliEvent::Error {
            correlates: None,
            code: "bad_request".into(),
            message: "some dynamic detail".into(),
        },
    ]
}

fn sample_wallet() -> WalletEntry {
    WalletEntry {
        curves: vec!["secp256k1".into()],
        addresses: vec![],
        id: "w1".into(),
        name: "Wallet One".into(),
        address: "0xabc".into(),
        chain: "Ethereum".into(),
        threshold: "2/3".into(),
    }
}

fn sample_session() -> SessionEntry {
    SessionEntry {
        session_id: "dkg_x".into(),
        session_type: "dkg".into(),
        threshold: 2,
        total: 3,
        proposer: "node-a".into(),
        participants: vec!["node-a".into(), "node-b".into()],
    }
}

fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/event_contract.golden.jsonl")
}

#[test]
fn event_contract_matches_golden() {
    let actual: String = all_events()
        .iter()
        .map(normalize_event_line)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    let path = golden_path();

    if std::env::var("BLESS").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        eprintln!("blessed golden at {}", path.display());
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {} — generate it with BLESS=1 cargo test -p starlab-cli --test event_contract",
            path.display()
        )
    });

    assert_eq!(
        actual, expected,
        "event contract drifted from golden. If this change is intended, regenerate with \
         BLESS=1 cargo test -p starlab-cli --test event_contract and review the diff."
    );
}
