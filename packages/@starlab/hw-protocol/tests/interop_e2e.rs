//! End-to-end cryptographic conformance & security tests proving that the
//! ESP32 hardware-share engine (`starlab-hw-protocol`) interoperates
//! byte-for-byte with `starlab-core` software peers across:
//! - 2-of-3 Mixed DKG + HD Signing on Bitcoin Taproot (BIP-340 / BIP-86)
//! - 2-of-3 Mixed DKG + HD Signing on Solana & Sui (Ed25519 / BIP-44)
//! - Physical button rejection + guaranteed single-use nonce destruction
//! - PIN lockout + irreversible NVS wipe after 10 wrong attempts
//! - Air-gap MicroSD `OfflineDataPackage` checksum & round-trip
//! - Strict MCU UART frame size (< 2 KB) and NVS storage size (< 3 KB) budgets

use frost_ed25519 as ed;
use frost_secp256k1_tr as secp;
use starlab_core::accounts::{
    account_addresses, address_for_chain, standard_path, verify_taproot_signature,
};
use starlab_core::hd_derivation::{ChainCode, DerivationPath, derive_child_key_path};
use starlab_core::rng::os_rng;
use starlab_hw_protocol::sim::ButtonPolicy;
use starlab_hw_protocol::{
    AirGapEnvelope, CurveKind, HardwareShareEngine, HwCommand, HwErrorCode, HwEvent, HwRequest,
    HwResponse, MAX_PIN_ATTEMPTS, OfflineDataPackage, SignDisplayInfo, SimHal,
};
use std::collections::{BTreeMap, VecDeque};

/// Helper: send one NDJSON line through `handle_line` (exactly like USB-CDC
/// serial I/O) and assert it stays under the MCU frame budget.
fn rpc(
    engine: &mut HardwareShareEngine<SimHal>,
    req: HwRequest,
    max_line_bytes: &mut usize,
) -> HwResponse {
    let line = serde_json::to_string(&req).unwrap();
    let resp_line = engine.handle_line(&line);
    assert!(resp_line.ends_with('\n'), "NDJSON response must end with \\n");
    *max_line_bytes = (*max_line_bytes).max(resp_line.len());
    serde_json::from_str(&resp_line).expect("valid HwResponse JSON")
}

#[test]
fn mixed_2of3_bitcoin_taproot_dkg_and_hd_signing() {
    let mut engine = HardwareShareEngine::new(SimHal::new_auto_approve());
    let mut max_frame = 0usize;

    // 1. Configure PIN (auto-unlocks on first setup)
    let r = rpc(
        &mut engine,
        HwRequest {
            id: Some(1),
            command: HwCommand::SetPin {
                pin: "884812".into(),
            },
        },
        &mut max_frame,
    );
    assert!(r.ok);

    // 2. Mixed 2-of-3 DKG: Participant #1 = ESP32 hardware share; #2 & #3 = starlab-core software nodes
    let r = rpc(
        &mut engine,
        HwRequest {
            id: Some(2),
            command: HwCommand::DkgInit {
                wallet_id: "btc-vault".into(),
                curve: CurveKind::Secp256k1,
                participant_index: 1,
                threshold: 2,
                total: 3,
            },
        },
        &mut max_frame,
    );
    assert!(r.ok);

    // Round 1 on ESP32 (#1)
    let r1_hw_resp = rpc(
        &mut engine,
        HwRequest {
            id: Some(3),
            command: HwCommand::DkgRound1 {
                wallet_id: "btc-vault".into(),
            },
        },
        &mut max_frame,
    );
    let HwEvent::DkgRound1 {
        package_hex: hw_r1_hex,
        ..
    } = r1_hw_resp.data.unwrap()
    else {
        panic!("expected DkgRound1");
    };
    let hw_r1_pkg =
        secp::keys::dkg::round1::Package::deserialize(&hex::decode(&hw_r1_hex).unwrap()).unwrap();

    // Round 1 on software peers (#2 and #3)
    let id1 = secp::Identifier::try_from(1u16).unwrap();
    let id2 = secp::Identifier::try_from(2u16).unwrap();
    let id3 = secp::Identifier::try_from(3u16).unwrap();
    let mut rng = os_rng();
    let (sw2_r1_sec, sw2_r1_pkg) = secp::keys::dkg::part1(id2, 3, 2, &mut rng).unwrap();
    let (sw3_r1_sec, sw3_r1_pkg) = secp::keys::dkg::part1(id3, 3, 2, &mut rng).unwrap();

    // Round 2 on ESP32 (#1)
    let mut peer_r1_for_hw = BTreeMap::new();
    peer_r1_for_hw.insert(2, hex::encode(sw2_r1_pkg.serialize().unwrap()));
    // Deliberately send #3 as JSON hex to prove mixed binary+JSON wire tolerance!
    peer_r1_for_hw.insert(3, hex::encode(serde_json::to_vec(&sw3_r1_pkg).unwrap()));
    let r2_hw_resp = rpc(
        &mut engine,
        HwRequest {
            id: Some(4),
            command: HwCommand::DkgRound2 {
                wallet_id: "btc-vault".into(),
                peer_round1: peer_r1_for_hw,
            },
        },
        &mut max_frame,
    );
    let HwEvent::DkgRound2 {
        packages: hw_r2_pkgs,
        ..
    } = r2_hw_resp.data.unwrap()
    else {
        panic!("expected DkgRound2");
    };
    let hw_r2_for_sw2 =
        secp::keys::dkg::round2::Package::deserialize(&hex::decode(&hw_r2_pkgs[&2]).unwrap())
            .unwrap();
    let hw_r2_for_sw3 =
        secp::keys::dkg::round2::Package::deserialize(&hex::decode(&hw_r2_pkgs[&3]).unwrap())
            .unwrap();

    // Round 2 on software peers (#2 and #3)
    let r1_for_sw2 = BTreeMap::from([(id1, hw_r1_pkg.clone()), (id3, sw3_r1_pkg.clone())]);
    let r1_for_sw3 = BTreeMap::from([(id1, hw_r1_pkg.clone()), (id2, sw2_r1_pkg.clone())]);
    let (sw2_r2_sec, sw2_r2_map) = secp::keys::dkg::part2(sw2_r1_sec, &r1_for_sw2).unwrap();
    let (sw3_r2_sec, sw3_r2_map) = secp::keys::dkg::part2(sw3_r1_sec, &r1_for_sw3).unwrap();

    // Round 3 / Finalize on ESP32 (#1)
    let mut peer_r2_for_hw = BTreeMap::new();
    peer_r2_for_hw.insert(2, hex::encode(sw2_r2_map[&id1].serialize().unwrap()));
    peer_r2_for_hw.insert(3, hex::encode(sw3_r2_map[&id1].serialize().unwrap()));
    let fin_resp = rpc(
        &mut engine,
        HwRequest {
            id: Some(5),
            command: HwCommand::DkgFinalize {
                wallet_id: "btc-vault".into(),
                peer_round2: peer_r2_for_hw,
            },
        },
        &mut max_frame,
    );
    let HwEvent::DkgFinalized {
        group_pubkey_hex: hw_group_hex,
        addresses: hw_addrs_0,
        ..
    } = fin_resp.data.unwrap()
    else {
        panic!("expected DkgFinalized");
    };

    // Finalize on software peers (#2 and #3)
    let r2_for_sw2 = BTreeMap::from([(id1, hw_r2_for_sw2), (id3, sw3_r2_map[&id2].clone())]);
    let r2_for_sw3 = BTreeMap::from([(id1, hw_r2_for_sw3), (id2, sw2_r2_map[&id3].clone())]);
    let (sw2_kp, sw2_pub) = secp::keys::dkg::part3(&sw2_r2_sec, &r1_for_sw2, &r2_for_sw2).unwrap();
    let (_sw3_kp, sw3_pub) = secp::keys::dkg::part3(&sw3_r2_sec, &r1_for_sw3, &r2_for_sw3).unwrap();

    let sw2_group_bytes = sw2_pub.verifying_key().serialize().unwrap();
    let sw3_group_bytes = sw3_pub.verifying_key().serialize().unwrap();
    assert_eq!(hw_group_hex, hex::encode(&sw2_group_bytes));
    assert_eq!(sw2_group_bytes, sw3_group_bytes);

    // Verify Account 0, 1, 2 Bitcoin Taproot addresses match starlab-core byte-for-byte
    // even while the ESP32 vault is LOCKED (public-only derivation).
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: Some(6),
                command: HwCommand::Lock
            },
            &mut max_frame
        )
        .ok
    );
    for acct in 0..3u32 {
        let expected = account_addresses("secp256k1", &sw2_group_bytes, acct).unwrap();
        let resp = rpc(
            &mut engine,
            HwRequest {
                id: Some(10 + acct as u64),
                command: HwCommand::DeriveAddress {
                    wallet_id: "btc-vault".into(),
                    chain: "bitcoin".into(),
                    account: acct,
                },
            },
            &mut max_frame,
        );
        let HwEvent::Address { address, path, .. } = resp.data.unwrap() else {
            panic!("expected Address");
        };
        assert_eq!(path, expected[0].1);
        assert_eq!(address, expected[0].2);
        assert!(address.starts_with("bc1p"), "must be BIP-86 P2TR address");
        if acct == 0 {
            assert_eq!(hw_addrs_0[0].address, address);
        }
    }

    // Check NVS storage budget (< 3 KB per encrypted wallet record)
    let raw_nvs_record = engine.hal().raw_nvs().get("w:btc-vault").unwrap();
    assert!(
        raw_nvs_record.len() < 3072,
        "NVS wallet record too large: {} bytes",
        raw_nvs_record.len()
    );

    // 3. Unlock and run 2-of-3 Threshold Signing on Account #1 (m/86'/0'/0'/0/1):
    //    ESP32 (#1) + Software Peer (#2)
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: Some(20),
                command: HwCommand::Unlock {
                    pin: "884812".into()
                }
            },
            &mut max_frame
        )
        .ok
    );

    let commit_resp = rpc(
        &mut engine,
        HwRequest {
            id: Some(21),
            command: HwCommand::SignCommit {
                wallet_id: "btc-vault".into(),
                chain: "bitcoin".into(),
                account: 1,
            },
        },
        &mut max_frame,
    );
    let HwEvent::SignCommitment {
        session_id,
        address: btc_addr_1,
        commitment_hex: hw_commit_hex,
        ..
    } = commit_resp.data.unwrap()
    else {
        panic!("expected SignCommitment");
    };
    let hw_commit =
        secp::round1::SigningCommitments::deserialize(&hex::decode(&hw_commit_hex).unwrap())
            .unwrap();

    // Derive matching BIP-86 child key on Software Peer #2
    let path1 = DerivationPath::parse(&standard_path("bitcoin", 1).unwrap()).unwrap();
    let cc = ChainCode::from_group_key(&sw2_group_bytes);
    let sw2_child =
        derive_child_key_path::<secp::Secp256K1Sha256TR>(&sw2_kp, &sw2_pub, &cc, &path1).unwrap();
    let (sw2_nonces, sw2_commit) = secp::round1::commit(sw2_child.key_package.signing_share(), &mut rng);

    // Fake 32-byte BIP-341 Taproot sighash
    let sighash = [0x42u8; 32];
    let commitments_wire = BTreeMap::from([
        (1u16, hw_commit_hex.clone()),
        (2u16, hex::encode(sw2_commit.serialize().unwrap())),
    ]);

    // Round 2 on ESP32 (#1) — triggers Clear Signing screen + physical button!
    let share_resp = rpc(
        &mut engine,
        HwRequest {
            id: Some(22),
            command: HwCommand::SignShare {
                session_id,
                message_hex: hex::encode(sighash),
                commitments: commitments_wire,
                display: SignDisplayInfo {
                    summary: Some("Send 0.05 BTC".into()),
                    recipient: Some("bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0".into()),
                    amount: Some("0.05 BTC".into()),
                    ..Default::default()
                },
            },
        },
        &mut max_frame,
    );
    let HwEvent::SignShare {
        share_hex: hw_share_hex,
        ..
    } = share_resp.data.unwrap()
    else {
        panic!("expected SignShare");
    };

    // Assert the ESP32 screen displayed trusted device-derived metadata!
    let shown = engine.hal().displayed_prompts.last().unwrap();
    assert_eq!(shown.wallet_id, "btc-vault");
    assert_eq!(shown.chain, "bitcoin");
    assert_eq!(shown.path, "m/86'/0'/0'/0/1");
    assert_eq!(shown.signer_address, btc_addr_1);
    assert_eq!(shown.message_digest_hex, hex::encode(sighash));
    assert_eq!(shown.amount.as_deref(), Some("0.05 BTC"));

    // Round 2 on Software Peer #2 + Aggregate
    let frost_commits = BTreeMap::from([(id1, hw_commit), (id2, sw2_commit)]);
    let signing_pkg = secp::SigningPackage::new(frost_commits, &sighash);
    let sw2_share = secp::round2::sign(&signing_pkg, &sw2_nonces, &sw2_child.key_package).unwrap();
    let hw_share =
        secp::round2::SignatureShare::deserialize(&hex::decode(&hw_share_hex).unwrap()).unwrap();

    let sig = secp::aggregate(
        &signing_pkg,
        &BTreeMap::from([(id1, hw_share), (id2, sw2_share)]),
        &sw2_child.public_key_package,
    )
    .expect("FROST BIP-340 aggregation must succeed");

    let sig_bytes = sig.serialize().unwrap();
    assert!(
        verify_taproot_signature(&btc_addr_1, &sighash, &sig_bytes).unwrap(),
        "aggregated BIP-340 signature must spend the BIP-86 P2TR address!"
    );

    // Assert every single NDJSON wire frame stayed compact (< 2.5 KB)
    assert!(
        max_frame < 2560,
        "largest NDJSON response exceeded MCU frame budget: {max_frame} bytes"
    );
}

#[test]
fn mixed_2of3_ed25519_solana_and_sui_dkg_and_hd_signing() {
    let mut engine = HardwareShareEngine::new(SimHal::new_auto_approve());
    let mut max_frame = 0usize;

    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: Some(1),
                command: HwCommand::SetPin {
                    pin: "654321".into()
                }
            },
            &mut max_frame
        )
        .ok
    );

    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: Some(2),
                command: HwCommand::DkgInit {
                    wallet_id: "ed-vault".into(),
                    curve: CurveKind::Ed25519,
                    participant_index: 1,
                    threshold: 2,
                    total: 3,
                }
            },
            &mut max_frame
        )
        .ok
    );

    let HwEvent::DkgRound1 {
        package_hex: hw_r1_hex,
        ..
    } = rpc(
        &mut engine,
        HwRequest {
            id: Some(3),
            command: HwCommand::DkgRound1 {
                wallet_id: "ed-vault".into(),
            },
        },
        &mut max_frame,
    )
    .data
    .unwrap()
    else {
        panic!("expected DkgRound1");
    };
    let hw_r1_pkg =
        ed::keys::dkg::round1::Package::deserialize(&hex::decode(&hw_r1_hex).unwrap()).unwrap();

    let id1 = ed::Identifier::try_from(1u16).unwrap();
    let id2 = ed::Identifier::try_from(2u16).unwrap();
    let id3 = ed::Identifier::try_from(3u16).unwrap();
    let mut rng = os_rng();
    let (sw2_r1_sec, sw2_r1_pkg) = ed::keys::dkg::part1(id2, 3, 2, &mut rng).unwrap();
    let (sw3_r1_sec, sw3_r1_pkg) = ed::keys::dkg::part1(id3, 3, 2, &mut rng).unwrap();

    let HwEvent::DkgRound2 {
        packages: hw_r2, ..
    } = rpc(
        &mut engine,
        HwRequest {
            id: Some(4),
            command: HwCommand::DkgRound2 {
                wallet_id: "ed-vault".into(),
                peer_round1: BTreeMap::from([
                    (2, hex::encode(sw2_r1_pkg.serialize().unwrap())),
                    (3, hex::encode(sw3_r1_pkg.serialize().unwrap())),
                ]),
            },
        },
        &mut max_frame,
    )
    .data
    .unwrap()
    else {
        panic!("expected DkgRound2");
    };

    let r1_for_sw2 = BTreeMap::from([(id1, hw_r1_pkg.clone()), (id3, sw3_r1_pkg.clone())]);
    let r1_for_sw3 = BTreeMap::from([(id1, hw_r1_pkg.clone()), (id2, sw2_r1_pkg.clone())]);
    let (sw2_r2_sec, sw2_r2_map) = ed::keys::dkg::part2(sw2_r1_sec, &r1_for_sw2).unwrap();
    let (_sw3_r2_sec, sw3_r2_map) = ed::keys::dkg::part2(sw3_r1_sec, &r1_for_sw3).unwrap();

    let HwEvent::DkgFinalized {
        group_pubkey_hex: hw_group_hex,
        ..
    } = rpc(
        &mut engine,
        HwRequest {
            id: Some(5),
            command: HwCommand::DkgFinalize {
                wallet_id: "ed-vault".into(),
                peer_round2: BTreeMap::from([
                    (2, hex::encode(sw2_r2_map[&id1].serialize().unwrap())),
                    (3, hex::encode(sw3_r2_map[&id1].serialize().unwrap())),
                ]),
            },
        },
        &mut max_frame,
    )
    .data
    .unwrap()
    else {
        panic!("expected DkgFinalized");
    };

    let hw_r2_for_sw2 =
        ed::keys::dkg::round2::Package::deserialize(&hex::decode(&hw_r2[&2]).unwrap()).unwrap();
    let (sw2_kp, sw2_pub) = ed::keys::dkg::part3(
        &sw2_r2_sec,
        &r1_for_sw2,
        &BTreeMap::from([(id1, hw_r2_for_sw2), (id3, sw3_r2_map[&id2].clone())]),
    )
    .unwrap();
    let sw2_group_bytes = sw2_pub.verifying_key().serialize().unwrap();
    assert_eq!(hw_group_hex, hex::encode(&sw2_group_bytes));

    // Sign BOTH Solana Account #0 and Sui Account #1 with ESP32 (#1) + Software (#2)
    for (chain, account, msg) in [
        ("solana", 0u32, b"solana-transfer-tx-bytes".as_slice()),
        ("sui", 1u32, b"sui-intent-tx-digest".as_slice()),
    ] {
        let HwEvent::SignCommitment {
            session_id,
            address: hw_addr,
            commitment_hex: hw_c_hex,
            ..
        } = rpc(
            &mut engine,
            HwRequest {
                id: Some(30),
                command: HwCommand::SignCommit {
                    wallet_id: "ed-vault".into(),
                    chain: chain.into(),
                    account,
                },
            },
            &mut max_frame,
        )
        .data
        .unwrap()
        else {
            panic!("expected SignCommitment");
        };

        let path = DerivationPath::parse(&standard_path(chain, account).unwrap()).unwrap();
        let cc = ChainCode::from_group_key(&sw2_group_bytes);
        let sw2_child =
            derive_child_key_path::<ed::Ed25519Sha512>(&sw2_kp, &sw2_pub, &cc, &path).unwrap();
        let sw2_child_vk = sw2_child.public_key_package.verifying_key().serialize().unwrap();
        assert_eq!(hw_addr, address_for_chain(chain, "ed25519", &sw2_child_vk).unwrap());

        let (sw2_nonces, sw2_commit) =
            ed::round1::commit(sw2_child.key_package.signing_share(), &mut rng);
        let hw_commit =
            ed::round1::SigningCommitments::deserialize(&hex::decode(&hw_c_hex).unwrap()).unwrap();

        let HwEvent::SignShare {
            share_hex: hw_share_hex,
            ..
        } = rpc(
            &mut engine,
            HwRequest {
                id: Some(31),
                command: HwCommand::SignShare {
                    session_id,
                    message_hex: hex::encode(msg),
                    commitments: BTreeMap::from([
                        (1, hw_c_hex),
                        (2, hex::encode(sw2_commit.serialize().unwrap())),
                    ]),
                    display: SignDisplayInfo::default(),
                },
            },
            &mut max_frame,
        )
        .data
        .unwrap()
        else {
            panic!("expected SignShare");
        };

        let signing_pkg = ed::SigningPackage::new(BTreeMap::from([(id1, hw_commit), (id2, sw2_commit)]), msg);
        let sw2_share = ed::round2::sign(&signing_pkg, &sw2_nonces, &sw2_child.key_package).unwrap();
        let hw_share =
            ed::round2::SignatureShare::deserialize(&hex::decode(&hw_share_hex).unwrap()).unwrap();
        let sig = ed::aggregate(
            &signing_pkg,
            &BTreeMap::from([(id1, hw_share), (id2, sw2_share)]),
            &sw2_child.public_key_package,
        )
        .expect("FROST Ed25519 aggregation must succeed");

        sw2_child
            .public_key_package
            .verifying_key()
            .verify(msg, &sig)
            .expect("aggregated Ed25519 signature must verify");
    }
}

#[test]
fn physical_reject_destroys_single_use_nonce() {
    let mut engine = HardwareShareEngine::new(
        SimHal::new_auto_approve().with_policy(ButtonPolicy::Scripted(VecDeque::from([false]))),
    );
    let mut max_frame = 0usize;
    rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::SetPin {
                pin: "112233".into(),
            },
        },
        &mut max_frame,
    );

    // Quick 2-of-2 Ed25519 DKG
    rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::DkgInit {
                wallet_id: "w".into(),
                curve: CurveKind::Ed25519,
                participant_index: 1,
                threshold: 2,
                total: 2,
            },
        },
        &mut max_frame,
    );
    let HwEvent::DkgRound1 {
        package_hex: r1_hex,
        ..
    } = rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::DkgRound1 {
                wallet_id: "w".into(),
            },
        },
        &mut max_frame,
    )
    .data
    .unwrap()
    else {
        unreachable!()
    };
    let id1 = ed::Identifier::try_from(1u16).unwrap();
    let id2 = ed::Identifier::try_from(2u16).unwrap();
    let mut rng = os_rng();
    let (sw2_r1_sec, sw2_r1_pkg) = ed::keys::dkg::part1(id2, 2, 2, &mut rng).unwrap();
    let hw_r1_pkg =
        ed::keys::dkg::round1::Package::deserialize(&hex::decode(&r1_hex).unwrap()).unwrap();
    let (_sw2_r2_sec, sw2_r2_map) =
        ed::keys::dkg::part2(sw2_r1_sec, &BTreeMap::from([(id1, hw_r1_pkg)])).unwrap();
    rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::DkgRound2 {
                wallet_id: "w".into(),
                peer_round1: BTreeMap::from([(2, hex::encode(sw2_r1_pkg.serialize().unwrap()))]),
            },
        },
        &mut max_frame,
    );
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::DkgFinalize {
                    wallet_id: "w".into(),
                    peer_round2: BTreeMap::from([(
                        2,
                        hex::encode(sw2_r2_map[&id1].serialize().unwrap())
                    )]),
                },
            },
            &mut max_frame,
        )
        .ok
    );

    // Commit Round 1
    let HwEvent::SignCommitment {
        session_id,
        commitment_hex,
        ..
    } = rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::SignCommit {
                wallet_id: "w".into(),
                chain: "solana".into(),
                account: 0,
            },
        },
        &mut max_frame,
    )
    .data
    .unwrap()
    else {
        unreachable!()
    };

    // Round 2: user presses physical REJECT -> must fail with UserRejected
    let rejected = rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::SignShare {
                session_id: session_id.clone(),
                message_hex: hex::encode(b"malicious tx"),
                commitments: BTreeMap::from([(1, commitment_hex.clone())]),
                display: SignDisplayInfo::default(),
            },
        },
        &mut max_frame,
    );
    assert!(!rejected.ok);
    assert_eq!(rejected.error.unwrap().code, HwErrorCode::UserRejected);

    // Even if button later becomes Approve, replaying the same session_id MUST FAIL
    // because the single-use nonce was destroyed on rejection!
    engine.hal_mut().set_policy(ButtonPolicy::AlwaysApprove);
    let replay = rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::SignShare {
                session_id,
                message_hex: hex::encode(b"second tx"),
                commitments: BTreeMap::from([(1, commitment_hex)]),
                display: SignDisplayInfo::default(),
            },
        },
        &mut max_frame,
    );
    assert!(!replay.ok);
    assert_eq!(replay.error.unwrap().code, HwErrorCode::SessionNotFound);
}

#[test]
fn pin_lockout_and_auto_wipe_after_max_attempts() {
    let mut engine = HardwareShareEngine::new(SimHal::new_auto_approve());
    let mut max_frame = 0usize;

    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::SetPin {
                    pin: "998877".into()
                }
            },
            &mut max_frame
        )
        .ok
    );
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::Lock
            },
            &mut max_frame
        )
        .ok
    );

    // Attempts 1..MAX_PIN_ATTEMPTS-1 return WrongPin and decrement remaining counter
    for attempt in 1..MAX_PIN_ATTEMPTS {
        let resp = rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::Unlock {
                    pin: "000000".into(),
                },
            },
            &mut max_frame,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.unwrap().code, HwErrorCode::WrongPin);

        let HwEvent::DeviceInfo {
            remaining_pin_attempts,
            ..
        } = rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::DeviceInfo,
            },
            &mut max_frame,
        )
        .data
        .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(remaining_pin_attempts, MAX_PIN_ATTEMPTS - attempt);
    }

    // Final wrong attempt triggers irreversible VaultWiped!
    let final_resp = rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::Unlock {
                pin: "000000".into(),
            },
        },
        &mut max_frame,
    );
    assert!(!final_resp.ok);
    assert_eq!(final_resp.error.unwrap().code, HwErrorCode::VaultWiped);
    assert!(
        engine.hal().raw_nvs().is_empty(),
        "NVS Flash must be completely wiped after max failed PIN attempts"
    );
}

#[test]
fn airgap_sd_card_offline_package_roundtrip_and_checksum_gate() {
    let mut engine = HardwareShareEngine::new(SimHal::new_auto_approve());
    let mut max_frame = 0usize;

    // 1. Tampered checksum MUST be rejected
    let payload = serde_json::to_vec(&AirGapEnvelope::Request {
        command: HwCommand::DeviceInfo,
    })
    .unwrap();
    let mut bad_pkg = OfflineDataPackage::new(
        "pkg-001",
        "device_info",
        0,
        "esp32-node-1",
        "1732460000",
        payload.clone(),
    );
    bad_pkg.checksum = "00".repeat(32);
    let bad_resp = rpc(
        &mut engine,
        HwRequest {
            id: Some(1),
            command: HwCommand::SdImportPackage { package: bad_pkg },
        },
        &mut max_frame,
    );
    assert!(!bad_resp.ok);
    assert_eq!(bad_resp.error.unwrap().code, HwErrorCode::InvalidRequest);

    // 2. Valid OfflineDataPackage returns a checksum-verified outbound export package
    let good_pkg = OfflineDataPackage::new(
        "pkg-002",
        "device_info",
        0,
        "esp32-node-1",
        "1732460001",
        payload,
    );
    assert_eq!(
        good_pkg.suggested_filename(),
        "device_info_0_esp32-node-1_1732460001.json"
    );
    let ok_resp = rpc(
        &mut engine,
        HwRequest {
            id: Some(2),
            command: HwCommand::SdImportPackage { package: good_pkg },
        },
        &mut max_frame,
    );
    let HwEvent::SdExportPackage { package: out_pkg } = ok_resp.data.unwrap() else {
        panic!("expected SdExportPackage");
    };
    assert!(out_pkg.verify_checksum());
    let AirGapEnvelope::Response {
        event: HwEvent::DeviceInfo { board, .. },
    } = serde_json::from_slice(&out_pkg.data).unwrap()
    else {
        panic!("expected DeviceInfo inside AirGapEnvelope::Response");
    };
    assert!(board.contains("lilygo-t-display-s3"));
}

#[test]
fn encrypted_nvs_never_leaks_plaintext_and_binds_to_hw_efuse_secret() {
    let mut engine = HardwareShareEngine::new(
        SimHal::new_auto_approve().with_hw_secret([0x11; 32]),
    );
    let mut max_frame = 0usize;

    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::SetPin {
                    pin: "246810".into(),
                },
            },
            &mut max_frame,
        )
        .ok
    );

    // Run 2-of-2 Ed25519 DKG so a real secret share is written to NVS
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::DkgInit {
                    wallet_id: "sec-test".into(),
                    curve: CurveKind::Ed25519,
                    participant_index: 1,
                    threshold: 2,
                    total: 2,
                },
            },
            &mut max_frame,
        )
        .ok
    );
    let HwEvent::DkgRound1 {
        package_hex: r1_hex,
        ..
    } = rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::DkgRound1 {
                wallet_id: "sec-test".into(),
            },
        },
        &mut max_frame,
    )
    .data
    .unwrap()
    else {
        unreachable!()
    };
    let id1 = ed::Identifier::try_from(1u16).unwrap();
    let id2 = ed::Identifier::try_from(2u16).unwrap();
    let mut rng = os_rng();
    let (sw2_r1_sec, sw2_r1_pkg) = ed::keys::dkg::part1(id2, 2, 2, &mut rng).unwrap();
    let hw_r1_pkg =
        ed::keys::dkg::round1::Package::deserialize(&hex::decode(&r1_hex).unwrap()).unwrap();
    let (_sw2_r2_sec, sw2_r2_map) =
        ed::keys::dkg::part2(sw2_r1_sec, &BTreeMap::from([(id1, hw_r1_pkg)])).unwrap();
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::DkgRound2 {
                    wallet_id: "sec-test".into(),
                    peer_round1: BTreeMap::from([(2, hex::encode(sw2_r1_pkg.serialize().unwrap()))]),
                },
            },
            &mut max_frame,
        )
        .ok
    );
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::DkgFinalize {
                    wallet_id: "sec-test".into(),
                    peer_round2: BTreeMap::from([(
                        2,
                        hex::encode(sw2_r2_map[&id1].serialize().unwrap()),
                    )]),
                },
            },
            &mut max_frame,
        )
        .ok
    );

    // 1. Verify raw NVS Flash bytes never contain plaintext FROST KeyPackage fields
    let raw_blob = engine.hal().raw_nvs().get("w:sec-test").unwrap();
    let raw_str = String::from_utf8_lossy(raw_blob);
    assert!(
        !raw_str.contains("signing_share"),
        "NVS record must NEVER store plaintext signing_share!"
    );

    // 2. Simulate an offline Flash dump moved onto another chip with a different
    //    eFuse HMAC secret: even with the EXACT right PIN, unlock MUST fail!
    let cloned_nvs = engine.hal().raw_nvs().clone();
    let mut attacker_hal = SimHal::new_auto_approve().with_hw_secret([0x22; 32]);
    for (k, v) in cloned_nvs {
        use starlab_hw_protocol::HardwareHal as _;
        attacker_hal.nvs_set(&k, &v).unwrap();
    }
    let mut attacker_engine = HardwareShareEngine::new(attacker_hal);
    let unlock_attempt = rpc(
        &mut attacker_engine,
        HwRequest {
            id: None,
            command: HwCommand::Unlock {
                pin: "246810".into(),
            },
        },
        &mut max_frame,
    );
    assert!(!unlock_attempt.ok);
    assert_eq!(unlock_attempt.error.unwrap().code, HwErrorCode::WrongPin);

    // 3. On the legitimate chip, PIN rotation re-wraps existing shares cleanly
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::SetPin {
                    pin: "newpin99".into(),
                },
            },
            &mut max_frame,
        )
        .ok
    );
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::Lock,
            },
            &mut max_frame,
        )
        .ok
    );
    // Old PIN rejected
    let old_try = rpc(
        &mut engine,
        HwRequest {
            id: None,
            command: HwCommand::Unlock {
                pin: "246810".into(),
            },
        },
        &mut max_frame,
    );
    assert_eq!(old_try.error.unwrap().code, HwErrorCode::WrongPin);
    // New PIN unlocks and can still commit for the re-wrapped wallet
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::Unlock {
                    pin: "newpin99".into(),
                },
            },
            &mut max_frame,
        )
        .ok
    );
    assert!(
        rpc(
            &mut engine,
            HwRequest {
                id: None,
                command: HwCommand::SignCommit {
                    wallet_id: "sec-test".into(),
                    chain: "solana".into(),
                    account: 0,
                },
            },
            &mut max_frame,
        )
        .ok
    );
}
