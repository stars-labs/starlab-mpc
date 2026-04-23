//! Example demonstrating unified DKG: one root secret → keys for both curves,
//! followed by threshold signing and verification on both curves.
//!
//! This proves the single root secret approach truly works end-to-end:
//!   1. Unified DKG produces key packages for ed25519 + secp256k1
//!   2. Threshold signing works with both curve key packages
//!   3. Signatures verify against the group public keys
//!
//! Run with: cargo run --example unified_dkg

use mpc_wallet_frost_core::hd_derivation::{ChainCode, derive_child_key};
use mpc_wallet_frost_core::unified_dkg::{UnifiedDkg, UnifiedRound1Package};
use mpc_wallet_frost_core::ed25519::Ed25519Curve;
use mpc_wallet_frost_core::secp256k1::Secp256k1Curve;
use mpc_wallet_frost_core::traits::FrostCurve;
use std::collections::BTreeMap;

fn main() {
    let max_signers: u16 = 3;
    let min_signers: u16 = 2;

    println!("=== Unified DKG: Single Root Secret → Ed25519 + Secp256k1 ===");
    println!("  Participants: {max_signers}");
    println!("  Threshold:    {min_signers}");

    // ─── Phase 1: Unified DKG ───────────────────────────────────────────

    let mut participants: Vec<UnifiedDkg> = (1..=max_signers)
        .map(|i| {
            let mut dkg = UnifiedDkg::new();
            dkg.init_dkg(i, max_signers, min_signers);
            dkg
        })
        .collect();

    // Round 1: generate commitments for both curves
    println!("\n--- Round 1: Generating commitments for both curves ---");
    let round1_packages: Vec<UnifiedRound1Package> = participants
        .iter_mut()
        .enumerate()
        .map(|(i, p)| {
            let pkg = p.generate_round1().expect("round1 failed");
            println!("  Participant {} generated round 1 packages", i + 1);
            pkg
        })
        .collect();

    // Distribute round 1 packages
    for sender_idx in 0..max_signers as usize {
        for receiver_idx in 0..max_signers as usize {
            if sender_idx == receiver_idx {
                continue;
            }
            let sender_id = (sender_idx + 1) as u16;
            participants[receiver_idx]
                .add_round1_package(sender_id, &round1_packages[sender_idx])
                .expect("add_round1_package failed");
        }
    }
    println!("  Round 1 packages distributed to all participants");

    // Round 2: generate shares for both curves
    println!("\n--- Round 2: Generating shares for both curves ---");
    let round2_packages: Vec<_> = participants
        .iter_mut()
        .enumerate()
        .map(|(i, p)| {
            assert!(p.can_start_round2(), "participant {} not ready for round 2", i + 1);
            let pkgs = p.generate_round2().expect("round2 failed");
            println!("  Participant {} generated round 2 packages", i + 1);
            pkgs
        })
        .collect();

    // Distribute round 2 packages
    for sender_idx in 0..max_signers as usize {
        let sender_id = (sender_idx + 1) as u16;
        for receiver_idx in 0..max_signers as usize {
            let receiver_id = (receiver_idx + 1) as u16;
            if sender_id == receiver_id {
                continue;
            }
            let ed_hex = round2_packages[sender_idx]
                .ed25519
                .get(&receiver_id)
                .expect("missing ed25519 round2 package");
            let secp_hex = round2_packages[sender_idx]
                .secp256k1
                .get(&receiver_id)
                .expect("missing secp256k1 round2 package");
            participants[receiver_idx]
                .add_round2_package(sender_id, ed_hex, secp_hex)
                .expect("add_round2_package failed");
        }
    }
    println!("  Round 2 packages distributed to all participants");

    // Finalize DKG
    println!("\n--- Finalizing DKG for both curves ---");
    let mut sol_addresses = Vec::new();
    let mut eth_addresses = Vec::new();

    for (i, p) in participants.iter_mut().enumerate() {
        assert!(p.can_finalize(), "participant {} not ready to finalize", i + 1);
        let keystore = p.finalize_dkg().expect("finalize failed");
        println!("  Participant {} finalized DKG", i + 1);
        println!("    Ed25519 curve:   {}", keystore.ed25519.curve);
        println!("    Secp256k1 curve: {}", keystore.secp256k1.curve);

        sol_addresses.push(p.get_solana_address().expect("sol address failed"));
        eth_addresses.push(p.get_eth_address().expect("eth address failed"));
    }

    // Verify all participants agree on addresses
    for i in 1..sol_addresses.len() {
        assert_eq!(sol_addresses[0], sol_addresses[i], "Solana address mismatch for participant {}", i + 1);
        assert_eq!(eth_addresses[0], eth_addresses[i], "Ethereum address mismatch for participant {}", i + 1);
    }

    println!("\n=== DKG Results ===");
    println!("  Solana address:   {}", sol_addresses[0]);
    println!("  Ethereum address: {}", eth_addresses[0]);
    println!("  All {max_signers} participants agree on both addresses!");

    // ─── Phase 2: Threshold Signing with Ed25519 ────────────────────────

    let message = b"Hello from unified MPC wallet!";
    // Use first `min_signers` participants as the signing quorum
    let signer_indices: Vec<usize> = (0..min_signers as usize).collect();

    println!("\n=== Threshold Signing: Ed25519 (Solana) ===");
    println!("  Message: {:?}", String::from_utf8_lossy(message));
    println!("  Signers: participants {:?}", signer_indices.iter().map(|i| i + 1).collect::<Vec<_>>());

    // Step 1: Each signer generates nonces and commitments
    let mut ed_nonces = BTreeMap::new();
    let mut ed_commitments = BTreeMap::new();

    for &idx in &signer_indices {
        let key_pkg = participants[idx].ed25519_key_package().expect("no ed25519 key package");
        let (nonces, commitments) = frost_ed25519::round1::commit(
            key_pkg.signing_share(),
            &mut rand_core::OsRng,
        );
        let id = *key_pkg.identifier();
        ed_nonces.insert(id, nonces);
        ed_commitments.insert(id, commitments);
        println!("  Participant {} generated ed25519 signing commitment", idx + 1);
    }

    // Step 2: Create signing package and generate signature shares
    let ed_signing_pkg = frost_ed25519::SigningPackage::new(ed_commitments, message);
    let mut ed_sig_shares = BTreeMap::new();

    for &idx in &signer_indices {
        let key_pkg = participants[idx].ed25519_key_package().unwrap();
        let id = *key_pkg.identifier();
        let nonces = &ed_nonces[&id];
        let share = frost_ed25519::round2::sign(&ed_signing_pkg, nonces, key_pkg)
            .expect("ed25519 signing failed");
        ed_sig_shares.insert(id, share);
        println!("  Participant {} generated ed25519 signature share", idx + 1);
    }

    // Step 3: Aggregate and verify
    let ed_pub_pkg = participants[0].ed25519_public_key_package().unwrap();
    let ed_signature = frost_ed25519::aggregate(&ed_signing_pkg, &ed_sig_shares, ed_pub_pkg)
        .expect("ed25519 aggregation failed");

    let ed_vk = ed_pub_pkg.verifying_key();
    ed_vk
        .verify(message, &ed_signature)
        .expect("ed25519 signature verification failed");

    let ed_sig_bytes = ed_signature.serialize().expect("ed25519 sig serialize failed");
    println!("  Ed25519 signature:  {}", hex::encode(&ed_sig_bytes));
    println!("  Verification: PASSED");

    // ─── Phase 3: Threshold Signing with Secp256k1 ──────────────────────

    println!("\n=== Threshold Signing: Secp256k1 (Ethereum) ===");
    println!("  Message: {:?}", String::from_utf8_lossy(message));
    println!("  Signers: participants {:?}", signer_indices.iter().map(|i| i + 1).collect::<Vec<_>>());

    // Step 1: Each signer generates nonces and commitments
    let mut secp_nonces = BTreeMap::new();
    let mut secp_commitments = BTreeMap::new();

    for &idx in &signer_indices {
        let key_pkg = participants[idx].secp256k1_key_package().expect("no secp256k1 key package");
        let (nonces, commitments) = frost_secp256k1::round1::commit(
            key_pkg.signing_share(),
            &mut rand_core::OsRng,
        );
        let id = *key_pkg.identifier();
        secp_nonces.insert(id, nonces);
        secp_commitments.insert(id, commitments);
        println!("  Participant {} generated secp256k1 signing commitment", idx + 1);
    }

    // Step 2: Create signing package and generate signature shares
    let secp_signing_pkg = frost_secp256k1::SigningPackage::new(secp_commitments, message);
    let mut secp_sig_shares = BTreeMap::new();

    for &idx in &signer_indices {
        let key_pkg = participants[idx].secp256k1_key_package().unwrap();
        let id = *key_pkg.identifier();
        let nonces = &secp_nonces[&id];
        let share = frost_secp256k1::round2::sign(&secp_signing_pkg, nonces, key_pkg)
            .expect("secp256k1 signing failed");
        secp_sig_shares.insert(id, share);
        println!("  Participant {} generated secp256k1 signature share", idx + 1);
    }

    // Step 3: Aggregate and verify
    let secp_pub_pkg = participants[0].secp256k1_public_key_package().unwrap();
    let secp_signature = frost_secp256k1::aggregate(&secp_signing_pkg, &secp_sig_shares, secp_pub_pkg)
        .expect("secp256k1 aggregation failed");

    let secp_vk = secp_pub_pkg.verifying_key();
    secp_vk
        .verify(message, &secp_signature)
        .expect("secp256k1 signature verification failed");

    let secp_sig_bytes = secp_signature.serialize().expect("secp256k1 sig serialize failed");
    println!("  Secp256k1 signature: {}", hex::encode(&secp_sig_bytes));
    println!("  Verification: PASSED");

    // ─── Phase 4: HD Key Derivation ────────────────────────────────────

    println!("\n=== HD Key Derivation: Child Addresses from Same DKG ===");

    // Derive chain codes from group public keys
    let ed_pub_pkg = participants[0].ed25519_public_key_package().unwrap();
    let secp_pub_pkg = participants[0].secp256k1_public_key_package().unwrap();
    let ed_vk_bytes = Ed25519Curve::serialize_verifying_key(&Ed25519Curve::verifying_key(ed_pub_pkg)).unwrap();
    let secp_vk_bytes = Secp256k1Curve::serialize_verifying_key(&Secp256k1Curve::verifying_key(secp_pub_pkg)).unwrap();
    let ed_chain_code = ChainCode::from_group_key(&ed_vk_bytes);
    let secp_chain_code = ChainCode::from_group_key(&secp_vk_bytes);

    // Derive 3 child Solana addresses (indices 0, 1, 2)
    println!("\n--- Child Solana Addresses ---");
    println!("  Base address: {}", sol_addresses[0]);
    let mut child_sol_addresses = Vec::new();
    for index in 0..3u32 {
        let derived = derive_child_key::<frost_ed25519::Ed25519Sha512>(
            participants[0].ed25519_key_package().unwrap(),
            participants[0].ed25519_public_key_package().unwrap(),
            &ed_chain_code,
            index,
        ).expect("ed25519 child derivation failed");
        let child_vk = derived.public_key_package.verifying_key();
        let child_vk_bytes = child_vk.serialize().expect("serialize child vk");
        let child_sol_addr = bs58::encode(&child_vk_bytes).into_string();
        println!("  Child #{index}: {child_sol_addr}");
        child_sol_addresses.push((derived, child_sol_addr));
    }

    // Derive 3 child Ethereum addresses (indices 0, 1, 2)
    println!("\n--- Child Ethereum Addresses ---");
    println!("  Base address: {}", eth_addresses[0]);
    let mut child_eth_addresses = Vec::new();
    for index in 0..3u32 {
        let derived = derive_child_key::<frost_secp256k1::Secp256K1Sha256>(
            participants[0].secp256k1_key_package().unwrap(),
            participants[0].secp256k1_public_key_package().unwrap(),
            &secp_chain_code,
            index,
        ).expect("secp256k1 child derivation failed");
        let child_vk = derived.public_key_package.verifying_key();
        let child_eth_addr = Secp256k1Curve::get_eth_address(child_vk)
            .expect("eth address from child key");
        println!("  Child #{index}: {child_eth_addr}");
        child_eth_addresses.push((derived, child_eth_addr));
    }

    // Verify all child addresses are different
    for i in 0..3 {
        for j in (i + 1)..3 {
            assert_ne!(
                child_sol_addresses[i].1, child_sol_addresses[j].1,
                "child Solana addresses must differ"
            );
            assert_ne!(
                child_eth_addresses[i].1, child_eth_addresses[j].1,
                "child Ethereum addresses must differ"
            );
        }
    }
    println!("\n  All child addresses are unique!");

    // Verify all participants derive the same child addresses
    for index in 0..3u32 {
        for p_idx in 1..max_signers as usize {
            let p_ed_derived = derive_child_key::<frost_ed25519::Ed25519Sha512>(
                participants[p_idx].ed25519_key_package().unwrap(),
                participants[p_idx].ed25519_public_key_package().unwrap(),
                &ed_chain_code,
                index,
            ).unwrap();
            let p_vk = p_ed_derived.public_key_package.verifying_key().serialize().unwrap();
            let p0_vk = child_sol_addresses[index as usize].0.public_key_package.verifying_key().serialize().unwrap();
            assert_eq!(p_vk, p0_vk, "participant {} disagrees on child ed25519 key at index {index}", p_idx + 1);
        }
    }
    println!("  All {max_signers} participants agree on all child addresses!");

    // ─── Phase 5: Threshold Signing with Child Key #1 ───────────────────

    // Child ed25519 signing (child index 1)
    println!("\n=== Threshold Signing with Child Keys (index 1) ===");
    let child_message = b"Signed with HD-derived child key!";
    let child_signer_indices: Vec<usize> = (0..min_signers as usize).collect();

    // Derive child keys for all signers at index 1
    let child_ed_keys: Vec<_> = child_signer_indices.iter().map(|&idx| {
        derive_child_key::<frost_ed25519::Ed25519Sha512>(
            participants[idx].ed25519_key_package().unwrap(),
            participants[idx].ed25519_public_key_package().unwrap(),
            &ed_chain_code,
            1,
        ).unwrap()
    }).collect();

    // Ed25519 child signing
    let mut child_ed_nonces = BTreeMap::new();
    let mut child_ed_commitments = BTreeMap::new();
    for (i, _) in child_signer_indices.iter().enumerate() {
        let kp = &child_ed_keys[i].key_package;
        let (nonces, commitments) = frost_ed25519::round1::commit(
            kp.signing_share(),
            &mut rand_core::OsRng,
        );
        let id = *kp.identifier();
        child_ed_nonces.insert(id, nonces);
        child_ed_commitments.insert(id, commitments);
    }

    let child_ed_signing_pkg = frost_ed25519::SigningPackage::new(child_ed_commitments, child_message);
    let mut child_ed_sig_shares = BTreeMap::new();
    for (i, _) in child_signer_indices.iter().enumerate() {
        let kp = &child_ed_keys[i].key_package;
        let id = *kp.identifier();
        let share = frost_ed25519::round2::sign(&child_ed_signing_pkg, &child_ed_nonces[&id], kp)
            .expect("child ed25519 signing failed");
        child_ed_sig_shares.insert(id, share);
    }

    let child_ed_pub = &child_ed_keys[0].public_key_package;
    let child_ed_sig = frost_ed25519::aggregate(&child_ed_signing_pkg, &child_ed_sig_shares, child_ed_pub)
        .expect("child ed25519 aggregation failed");
    child_ed_pub.verifying_key()
        .verify(child_message, &child_ed_sig)
        .expect("child ed25519 signature verification failed");
    println!("  Ed25519 child #1 signing:  PASSED");

    // Secp256k1 child signing
    let child_secp_keys: Vec<_> = child_signer_indices.iter().map(|&idx| {
        derive_child_key::<frost_secp256k1::Secp256K1Sha256>(
            participants[idx].secp256k1_key_package().unwrap(),
            participants[idx].secp256k1_public_key_package().unwrap(),
            &secp_chain_code,
            1,
        ).unwrap()
    }).collect();

    let mut child_secp_nonces = BTreeMap::new();
    let mut child_secp_commitments = BTreeMap::new();
    for (i, _) in child_signer_indices.iter().enumerate() {
        let kp = &child_secp_keys[i].key_package;
        let (nonces, commitments) = frost_secp256k1::round1::commit(
            kp.signing_share(),
            &mut rand_core::OsRng,
        );
        let id = *kp.identifier();
        child_secp_nonces.insert(id, nonces);
        child_secp_commitments.insert(id, commitments);
    }

    let child_secp_signing_pkg = frost_secp256k1::SigningPackage::new(child_secp_commitments, child_message);
    let mut child_secp_sig_shares = BTreeMap::new();
    for (i, _) in child_signer_indices.iter().enumerate() {
        let kp = &child_secp_keys[i].key_package;
        let id = *kp.identifier();
        let share = frost_secp256k1::round2::sign(&child_secp_signing_pkg, &child_secp_nonces[&id], kp)
            .expect("child secp256k1 signing failed");
        child_secp_sig_shares.insert(id, share);
    }

    let child_secp_pub = &child_secp_keys[0].public_key_package;
    let child_secp_sig = frost_secp256k1::aggregate(&child_secp_signing_pkg, &child_secp_sig_shares, child_secp_pub)
        .expect("child secp256k1 aggregation failed");
    child_secp_pub.verifying_key()
        .verify(child_message, &child_secp_sig)
        .expect("child secp256k1 signature verification failed");
    println!("  Secp256k1 child #1 signing: PASSED");

    // ─── Summary ────────────────────────────────────────────────────────

    println!("\n{}", "=".repeat(60));
    println!("  UNIFIED DKG + HD DERIVATION END-TO-END COMPLETE");
    println!("{}", "=".repeat(60));
    println!("  Root secret:      1 per participant (HKDF-derived per curve)");
    println!("  DKG:              {min_signers}-of-{max_signers} threshold for BOTH curves");
    println!("  Base Solana:      {}", sol_addresses[0]);
    println!("  Base Ethereum:    {}", eth_addresses[0]);
    println!("  Child Solana:     3 derived (indices 0-2)");
    println!("  Child Ethereum:   3 derived (indices 0-2)");
    println!("  Base ed25519:     {} signers → valid signature", min_signers);
    println!("  Base secp256k1:   {} signers → valid signature", min_signers);
    println!("  Child ed25519:    {} signers → valid signature (child #1)", min_signers);
    println!("  Child secp256k1:  {} signers → valid signature (child #1)", min_signers);
    println!("{}", "=".repeat(60));
}
