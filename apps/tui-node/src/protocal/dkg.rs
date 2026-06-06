//! Real FROST DKG Implementation
//! 
//! This implementation uses the exact same FROST cryptographic logic as the dkg.rs example.
//! It properly implements all three phases of FROST DKG:
//! - Part 1: Generates and exchanges commitments
//! - Part 2: Generates and distributes secret shares 
//! - Part 3: Computes the real group public key from DKG output
//! 
//! The previous insecure implementation that derived group keys from session IDs
//! has been completely removed and replaced with proper FROST threshold cryptography.

use crate::protocal::signal::WebRTCMessage;
use crate::utils::appstate_compat::AppState;
use crate::utils::state::DkgState;
use frost_core::{Ciphersuite, Identifier};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};
use base64;
use tracing::{info, error, warn};

/// DKG execution mode for different coordination scenarios
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum DkgMode {
    #[default]
    Online,    // Real-time WebRTC mesh coordination
    Offline,   // Air-gapped with file/QR code exchange
    Hybrid,    // Online coordination, offline key generation
}

/// Compute a FROST `Identifier` for `device_id` that is **deterministic across
/// every node** in the session.
///
/// FROST's `part2` verifies a proof-of-knowledge tied to the sender's
/// identifier. If two nodes disagree on "who is identifier 0x01" because their
/// local `session.participants` ordering differs, part2 raises
/// `InvalidProofOfKnowledge`. Sorting the participant list first gives us a
/// canonical order (alphabetical over `device_id`) every node agrees on without
/// any extra signalling.
///
/// Returns `None` only if `device_id` is not in the list, or if the resulting
/// index is out of FROST's identifier range (which can't actually happen for
/// reasonable session sizes).
pub(crate) fn canonical_identifier<C: Ciphersuite>(
    participants: &[String],
    device_id: &str,
) -> Option<Identifier<C>> {
    let mut sorted: Vec<&String> = participants.iter().collect();
    sorted.sort();
    let idx = sorted.iter().position(|p| p.as_str() == device_id)?;
    let one_based = u16::try_from(idx).ok()?.checked_add(1)?;
    Identifier::<C>::try_from(one_based).ok()
}

// Removed insecure derive_group_key function - now using real FROST DKG output.
//
// `handle_trigger_dkg_round1_dynamic` used to live here as a dispatch helper
// that branched on `CurveType` and forwarded to the generic
// `handle_trigger_dkg_round1<C>`. Stage 5 of the wallet-persistence plan
// removed it: now that session announcements carry the real curve name,
// the creator/joiner type witness `C` is resolved at the binary boundary
// (in `mpc-wallet-tui.rs`) rather than at each DKG entry point, and the
// dynamic helper had no remaining callers.

/// Start DKG Round 1 - Real FROST implementation
pub async fn handle_trigger_dkg_round1<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
    _internal_cmd_tx: tokio::sync::mpsc::UnboundedSender<crate::utils::state::InternalCommand<C>>
)
where
    C: Ciphersuite + Send + Sync + 'static,
{
    info!("🎯🎯🎯 handle_trigger_dkg_round1 CALLED! Device: {}", self_device_id);
    info!("📊 About to acquire state lock...");

    let mut guard = state.lock().await;
    info!("✅ State lock acquired");

    // Check if we have a session
    let session = match &guard.session {
        Some(s) => {
            info!("✅ Session found: {} participants, threshold {}/{}",
                s.participants.len(), s.threshold, s.total);
            s.clone()
        },
        None => {
            error!("❌ No session available for DKG!");
            guard.dkg_state = DkgState::Failed("No session available".to_string());
            return;
        }
    };
    
    // Start DKG Round 1
    guard.dkg_state = DkgState::Round1InProgress;
    
    // Compute our FROST identifier from the canonicalised (sorted) participant
    // list, so every node assigns the same identifier to the same device_id
    // regardless of local arrival order. A `None` here means `self_device_id`
    // isn't in `session.participants` — a protocol-level desync that we should
    // surface via `DkgState::Failed` rather than panic the tokio task.
    let my_identifier = match canonical_identifier::<C>(&session.participants, &self_device_id) {
        Some(id) => id,
        None => {
            error!(
                "❌ self_device_id {} not in session.participants={:?}",
                self_device_id, session.participants
            );
            guard.dkg_state = DkgState::Failed(format!(
                "self_device_id {} not in session.participants",
                self_device_id
            ));
            return;
        }
    };
    info!(
        "🪪 DKG Round 1 identifier for {} = {:?} (canonical, sorted participants: {:?})",
        self_device_id,
        my_identifier,
        {
            let mut sorted = session.participants.clone();
            sorted.sort();
            sorted
        }
    );

    // Generate real FROST DKG round 1
    // Use the frost_ed25519 rand_core for compatibility
    use frost_ed25519::rand_core::OsRng;
    let rng = OsRng;
    let (round1_secret_package, round1_public_package) = match frost_core::keys::dkg::part1(
        my_identifier,
        session.total,
        session.threshold,
        rng,
    ) {
        Ok(pair) => pair,
        Err(e) => {
            error!("❌ DKG part1 failed: {:?}", e);
            guard.dkg_state = DkgState::Failed(format!("DKG part1 failed: {:?}", e));
            return;
        }
    };

    // Serialize once; `part1` gives us distinct secret + public packages and
    // we store both in `guard` for later rounds. Serialization is infallible
    // for valid FROST output but the API type is `Result`, so propagate.
    let round1_secret_bytes = match round1_secret_package.serialize() {
        Ok(b) => b,
        Err(e) => {
            error!("Round1 SecretPackage::serialize failed: {:?}", e);
            guard.dkg_state = DkgState::Failed(format!("Round1 secret serialize: {:?}", e));
            return;
        }
    };
    let round1_public_bytes = match round1_public_package.serialize() {
        Ok(b) => b,
        Err(e) => {
            error!("Round1 Package::serialize failed: {:?}", e);
            guard.dkg_state = DkgState::Failed(format!("Round1 public serialize: {:?}", e));
            return;
        }
    };

    guard.dkg_part1_secret_package = Some(round1_secret_bytes);
    // Store the public-package bytes once — previously we serialized twice
    // (into `dkg_part1_public_package` and then again for broadcast). Same
    // bytes every time, same cost avoided.
    guard.dkg_part1_public_package = Some(round1_public_bytes.clone());

    // Store our own round1 package
    guard.dkg_round1_packages.insert(my_identifier, round1_public_package.clone());

    // Reuse the already-serialized bytes for the broadcast payload.
    let package_bytes = round1_public_bytes;
    
    // Create WebRTC message for broadcasting
    let message = WebRTCMessage::SimpleMessage {
        text: {
            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
            format!("DKG_ROUND1:{}", BASE64.encode(&package_bytes))
        },
    };
    
    // Broadcast to session participants
    let participants = session.participants.clone();
    drop(guard);
    
    // Wait longer to ensure data channels are fully established
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await; // Increased from 500ms to 2s
    info!("📡 Broadcasting DKG Round 1 packages to {} participants", participants.len() - 1);
    
    // Verify data channels are ready before broadcasting
    let participants_to_check: Vec<String> = participants.iter()
        .filter(|&p| *p != self_device_id)
        .cloned()
        .collect();
    
    let mut all_ready = false;
    for attempt in 1..=10 {
        let state_guard = state.lock().await;
        let ready_count = participants_to_check.iter().filter(|&device_id| {
            state_guard.data_channels.get(device_id)
                .map(|dc| dc.ready_state() == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open)
                .unwrap_or(false)
        }).count();
        
        if ready_count == participants_to_check.len() {
            all_ready = true;
            info!("✅ All {} data channels verified ready for DKG broadcast", ready_count);
            drop(state_guard);
            break;
        } else {
            drop(state_guard);
            info!("⏳ Data channels readiness: {}/{} (attempt {}/10)", 
                         ready_count, participants_to_check.len(), attempt);
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }
    
    if !all_ready {
        warn!("⚠️ Not all data channels ready, proceeding with DKG anyway");
    }
    
    for device_id in participants {
        if device_id != self_device_id {
            // Enhanced retry logic for sending DKG packages with longer timeout
            let mut retry_count = 0;
            const MAX_RETRIES: u32 = 10; // Increased from 3 to 10
            const RETRY_DELAY_MS: u64 = 500; // Reduced from 1000ms to 500ms for more frequent retries
            
            while retry_count < MAX_RETRIES {
                match crate::utils::device::send_webrtc_message(&device_id, &message, state.clone()).await {
                    Ok(()) => {
                        info!("✅ Successfully sent DKG Round 1 package to {}", device_id);
                        break;
                    }
                    Err(e) if (e.contains("Data channel not found") || e.contains("Data channel for") || e.contains("is not open")) && retry_count < MAX_RETRIES - 1 => {
                        retry_count += 1;
                        info!("⏳ Data channel not ready for {}, retrying in {}ms (attempt {}/{})", 
                                     device_id, RETRY_DELAY_MS, retry_count, MAX_RETRIES);
                        tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                    }
                    Err(e) => {
                        warn!("❌ Failed to send DKG Round 1 package to {} after {} attempts: {}", 
                                     device_id, retry_count + 1, e);
                        break;
                    }
                }
            }
        }
    }
}

/// Process DKG Round 1 package - Real FROST implementation
pub async fn process_dkg_round1<C>(
    state: Arc<Mutex<AppState<C>>>,
    from_device_id: String,
    package_bytes: Vec<u8>,
)
where
    // CurveIdentifier flows through to `handle_trigger_dkg_round2`'s
    // post-part2 re-feed → `process_dkg_round2`.
    C: Ciphersuite + Send + Sync + 'static + crate::utils::curve_traits::CurveIdentifier,
{
    let mut guard = state.lock().await;
    
    // Get session to determine sender's identifier
    let session = match &guard.session {
        Some(s) => s.clone(),
        None => return,
    };
    
    // Determine sender's identifier from the canonicalised participant list —
    // must match the identifier the sender used in `part1`, otherwise part2
    // will raise InvalidProofOfKnowledge.
    let sender_identifier = match canonical_identifier::<C>(&session.participants, &from_device_id) {
        Some(id) => id,
        None => {
            error!(
                "DKG Round 1 package from unknown device {} — not in session.participants={:?}",
                from_device_id, session.participants
            );
            return;
        }
    };
    
    // Deserialize the real FROST round1 package
    let round1_package = match frost_core::keys::dkg::round1::Package::<C>::deserialize(&package_bytes) {
        Ok(pkg) => pkg,
        Err(e) => {
            error!("Failed to deserialize DKG Round 1 package: {}", e);
            return;
        }
    };
    
    // Store the round1 package
    guard.dkg_round1_packages.insert(sender_identifier, round1_package);
    
    // Check if we have enough packages to proceed (need all participants including ourselves)
    let required_count = session.total as usize;
    let received_count = guard.dkg_round1_packages.len();
    
    info!("DKG Round 1: received {}/{} packages total", received_count, required_count);
    
    if received_count >= required_count {
        // Move to Round 2
        guard.dkg_state = DkgState::Round1Complete;
        info!("All DKG Round 1 packages received, triggering Round 2");
        
        // Trigger Round 2 immediately
        let self_device_id = guard.device_id.clone();
        drop(guard);
        
        handle_trigger_dkg_round2(state, self_device_id).await;
    }
}

/// Start DKG Round 2 - Real FROST part2 implementation
pub async fn handle_trigger_dkg_round2<C>(
    state: Arc<Mutex<AppState<C>>>,
    self_device_id: String,
)
where
    // CurveIdentifier needed because the post-part2 re-feed path calls
    // `process_dkg_round2` (which derives the curve name for addresses).
    C: Ciphersuite + Send + Sync + 'static + crate::utils::curve_traits::CurveIdentifier,
{
    info!("🔁🔁🔁 handle_trigger_dkg_round2 ENTERED for device={}", self_device_id);

    let mut guard = state.lock().await;
    info!("  round2: state lock acquired, dkg_state = {:?}", guard.dkg_state);

    // Check state
    if !matches!(guard.dkg_state, DkgState::Round1Complete) {
        warn!(
            "  round2 bailing: dkg_state is {:?}, expected Round1Complete",
            guard.dkg_state
        );
        return;
    }
    guard.dkg_state = DkgState::Round2InProgress;

    let session = match &guard.session {
        Some(s) => s.clone(),
        None => {
            error!("  round2 bailing: no session in AppState");
            guard.dkg_state = DkgState::Failed("No session in Round 2".to_string());
            return;
        }
    };

    // Canonical (sorted-participants) identifier — must match the one used
    // during Round 1 generation and stored on every peer.
    let my_identifier = match canonical_identifier::<C>(&session.participants, &self_device_id) {
        Some(id) => id,
        None => {
            error!(
                "  round2 bailing: self_device_id={} not in session.participants={:?}",
                self_device_id, session.participants
            );
            guard.dkg_state = DkgState::Failed(
                format!("self_device_id {} not in session.participants", self_device_id),
            );
            return;
        }
    };
    info!("  round2: my_identifier = {:?} (canonical)", my_identifier);

    // Get our secret package from round 1
    let secret_package_bytes = match guard.dkg_part1_secret_package.clone() {
        Some(b) => b,
        None => {
            error!("  round2 bailing: dkg_part1_secret_package is None");
            guard.dkg_state = DkgState::Failed("Missing round 1 secret package".to_string());
            return;
        }
    };
    info!(
        "  round2: dkg_part1_secret_package has {} bytes, deserializing…",
        secret_package_bytes.len()
    );
    let secret_package =
        match frost_core::keys::dkg::round1::SecretPackage::<C>::deserialize(&secret_package_bytes) {
            Ok(sp) => sp,
            Err(e) => {
                error!(
                    "  round2 bailing: SecretPackage::deserialize failed: {:?} (bytes len={})",
                    e,
                    secret_package_bytes.len()
                );
                guard.dkg_state = DkgState::Failed(format!("Round1 SecretPackage deserialize: {:?}", e));
                return;
            }
        };
    info!("  round2: secret_package deserialized ✓");

    // Collect all round 1 packages EXCLUDING our own (like in dkg.rs example)
    let round1_packages = guard.dkg_round1_packages.clone();
    let round1_packages_from_others: std::collections::BTreeMap<_, _> = round1_packages
        .iter()
        .filter(|(id, _)| **id != my_identifier)
        .map(|(id, pkg)| (*id, pkg.clone()))
        .collect();
    info!(
        "  round2: {}/{} peer round1 packages gathered (excluding self)",
        round1_packages_from_others.len(),
        round1_packages.len() - 1
    );

    // Generate round 2 packages using FROST part2
    info!("  round2: calling frost_core::keys::dkg::part2");
    let (round2_secret_package, round2_public_packages) = match frost_core::keys::dkg::part2(
        secret_package,
        &round1_packages_from_others,
    ) {
        Ok(result) => {
            info!(
                "  round2: part2 OK, produced {} per-peer round2 packages",
                result.1.len()
            );
            result
        }
        Err(e) => {
            error!("  round2 bailing: part2 failed: {:?}", e);
            guard.dkg_state = DkgState::Failed(format!("DKG part2 failed: {:?}", e));
            return;
        }
    };

    // Store the round2 secret package for part3
    match round2_secret_package.serialize() {
        Ok(bytes) => {
            info!("  round2: stored round2_secret_package ({} bytes)", bytes.len());
            guard.dkg_part2_secret_package = Some(bytes);
        }
        Err(e) => {
            error!("  round2 bailing: round2_secret_package.serialize failed: {:?}", e);
            guard.dkg_state = DkgState::Failed(format!("Serialize round2 secret: {:?}", e));
            return;
        }
    }

    drop(guard);

    // Create identifier→device_id map using the canonical (sorted) ordering
    // that Round 1 used. Any deviation here would route Round 2 packages to
    // the wrong peer.
    let mut identifier_to_device_id = std::collections::HashMap::new();
    for device_id in session.participants.iter() {
        if let Some(identifier) =
            canonical_identifier::<C>(&session.participants, device_id)
        {
            identifier_to_device_id.insert(identifier, device_id.clone());
        }
    }

    info!("  round2: broadcasting {} packages", round2_public_packages.len());
    for (receiver_id, package) in round2_public_packages {
        let Some(receiver_device_id) = identifier_to_device_id.get(&receiver_id) else {
            warn!("  round2: no device_id for identifier {:?}", receiver_id);
            continue;
        };
        if receiver_device_id == &self_device_id {
            continue;
        }
        let package_bytes = match package.serialize() {
            Ok(b) => b,
            Err(e) => {
                error!("  round2: serialize per-peer package for {}: {:?}", receiver_device_id, e);
                continue;
            }
        };
        let message = WebRTCMessage::SimpleMessage {
            text: {
                use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
                format!("DKG_ROUND2:{}", BASE64.encode(&package_bytes))
            },
        };
        match crate::utils::device::send_webrtc_message(receiver_device_id, &message, state.clone()).await {
            Ok(()) => info!("  round2: ✅ sent Round2 package to {}", receiver_device_id),
            Err(e) => warn!("  round2: ❌ send Round2 package to {} failed: {:?}", receiver_device_id, e),
        }
    }

    // Race fix: a peer's Round 2 package can arrive BEFORE our local part2
    // ran (common on fast/loopback transports). Those packets were buffered
    // in `process_dkg_round2` without finalizing because our part2 secret
    // wasn't stored yet. Now that part2 is done, re-feed any buffered
    // packages so part3 can complete. `process_dkg_round2` re-inserts the
    // same package (idempotent) and runs part3 once everything is present.
    let buffered: Vec<(String, Vec<u8>)> = {
        let guard = state.lock().await;
        guard
            .dkg_round2_packages
            .iter()
            .filter_map(|(id, pkg)| {
                let dev = identifier_to_device_id.get(id)?.clone();
                let bytes = pkg.serialize().ok()?;
                Some((dev, bytes))
            })
            .collect()
    };
    if !buffered.is_empty() {
        info!(
            "  round2: re-feeding {} buffered round2 package(s) to finalize",
            buffered.len()
        );
        for (dev, bytes) in buffered {
            process_dkg_round2(state.clone(), dev, bytes).await;
        }
    }

    info!("🔁 handle_trigger_dkg_round2 RETURNING for device={}", self_device_id);
}

/// Process DKG Round 2 package - Real FROST implementation with part3
pub async fn process_dkg_round2<C>(
    state: Arc<Mutex<AppState<C>>>,
    from_device_id: String,
    package_bytes: Vec<u8>,
)
where
    // `CurveIdentifier` is what lets us translate `C` → `"secp256k1"` or
    // `"ed25519"` at runtime. We need the real curve name (not the session
    // blob's "unified") to route address derivation in the completion block
    // below. TUI only instantiates `AppState<Secp256K1Sha256>` today, and
    // both ciphersuites in use implement this trait — a future third curve
    // would need to implement it too.
    C: Ciphersuite + Send + Sync + 'static + crate::utils::curve_traits::CurveIdentifier,
{
    let mut guard = state.lock().await;

    // Idempotency: a peer's round2 package can still arrive after we've
    // already finalized (e.g. the re-feed path below also ran part3).
    // Re-running part3 is wasteful and would re-emit completion — skip.
    if matches!(guard.dkg_state, DkgState::Complete) {
        return;
    }

    // Get session to determine sender's identifier
    let session = match &guard.session {
        Some(s) => s.clone(),
        None => return,
    };

    // Canonical identifiers — see `canonical_identifier` docstring above.
    let my_identifier = match canonical_identifier::<C>(&session.participants, &guard.device_id) {
        Some(id) => id,
        None => {
            error!(
                "Round 2 process: self device_id {} not in session.participants={:?}",
                guard.device_id, session.participants
            );
            return;
        }
    };
    let sender_identifier = match canonical_identifier::<C>(&session.participants, &from_device_id) {
        Some(id) => id,
        None => {
            error!(
                "Round 2 process: sender {} not in session.participants={:?}",
                from_device_id, session.participants
            );
            return;
        }
    };
    
    // Deserialize the real FROST round2 package
    let round2_package = match frost_core::keys::dkg::round2::Package::<C>::deserialize(&package_bytes) {
        Ok(pkg) => pkg,
        Err(e) => {
            error!("Failed to deserialize DKG Round 2 package: {}", e);
            return;
        }
    };
    
    // Store the round2 package
    guard.dkg_round2_packages.insert(sender_identifier, round2_package);
    
    // Check if we have received round2 packages from all other participants
    let session = match &guard.session {
        Some(s) => s.clone(),
        None => return,
    };
    
    let expected_senders = session.total as usize - 1; // All participants except ourselves
    let received_count = guard.dkg_round2_packages.len();
    
    info!("DKG Round 2: received {}/{} packages from other participants", received_count, expected_senders);
    
    if received_count >= expected_senders {
        // Now run FROST part3 to complete DKG
        
        // Get our round1 packages EXCLUDING our own (like in dkg.rs example)
        let round1_packages = guard.dkg_round1_packages.clone();
        let round1_packages_from_others: std::collections::BTreeMap<_, _> = round1_packages
            .iter()
            .filter(|(id, _)| **id != my_identifier)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();
        
        // Round 2 secret package must have been stored by `handle_trigger_dkg_round2`
        // earlier in this flow. A deserialize failure here means either storage
        // corruption or a protocol-version mismatch between Round 2 part2 and
        // part3 — neither is recoverable, but we should surface the failure
        // through `DkgState::Failed` so the UI can render an error modal
        // instead of the tokio task going dark from a panic.
        let round2_secret_package = match &guard.dkg_part2_secret_package {
            Some(bytes) => match frost_core::keys::dkg::round2::SecretPackage::<C>::deserialize(bytes) {
                Ok(pkg) => pkg,
                Err(e) => {
                    error!(
                        "  round2 process: SecretPackage::<round2>::deserialize failed: {:?} ({} bytes)",
                        e,
                        bytes.len()
                    );
                    guard.dkg_state = DkgState::Failed(format!(
                        "Round2 SecretPackage deserialize: {:?}",
                        e
                    ));
                    return;
                }
            },
            None => {
                // Our local part2 hasn't run yet — on fast transports a
                // peer's round2 can land first. Don't fail: the package is
                // already buffered in `dkg_round2_packages` above, and
                // `handle_trigger_dkg_round2` re-feeds buffered packages once
                // part2 completes, which is when part3 will actually run.
                info!(
                    "  round2: all packages in but local part2 not done yet — \
                     buffering; will finalize after part2"
                );
                return;
            }
        };
        
        // Get our round2 package (this contains only packages sent TO us)
        let round2_packages_for_us = guard.dkg_round2_packages.clone();
        
        // Run FROST part3 to get the key package and public key package
        let (key_package, pubkey_package) = match frost_core::keys::dkg::part3(
            &round2_secret_package,
            &round1_packages_from_others,
            &round2_packages_for_us,
        ) {
            Ok(result) => result,
            Err(e) => {
                guard.dkg_state = DkgState::Failed(format!("DKG part3 failed: {:?}", e));
                return;
            }
        };
        
        // Store the real key package and public key package
        guard.key_package = Some(key_package.clone());
        guard.public_key_package = Some(pubkey_package.clone());
        
        // Get the real verifying key from the public key package
        let verifying_key = pubkey_package.verifying_key();
        guard.group_public_key = Some(*verifying_key);
        
        // Complete DKG
        guard.dkg_state = DkgState::Complete;
        
        // Generate wallet ID
        let wallet_id = if let Some(session) = &guard.session {
            format!("wallet-{}", &session.session_id[..8])
        } else {
            "wallet-default".to_string()
        };
        guard.current_wallet_id = Some(wallet_id.clone());
        
        // Log the real group public key
        info!("🎉 DKG completed successfully!");
        info!("Group Verifying Key: {:?}", verifying_key);
        info!("Key Package Identifier: {:?}", key_package.identifier());
        info!("Min signers: {:?}", key_package.min_signers());
        
        // Now we can use the real verifying key to generate addresses.
        // VerifyingKey serialization can fail in principle (per the FROST API
        // it returns `Result`), so handle it rather than panicking — a failure
        // here would otherwise kill the tokio task silently and leave the UI
        // pinned on Round2 forever.
        let group_public_key_bytes = match verifying_key.serialize() {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("VerifyingKey::serialize failed after part3: {:?}", e);
                guard.dkg_state = DkgState::Failed(format!(
                    "VerifyingKey serialize: {:?}",
                    e
                ));
                return;
            }
        };
        
        // Generate appropriate blockchain addresses based on curve type.
        // `CurveIdentifier` is brought into scope by the `C:` bound on this
        // function; we call `C::curve_type()` directly below.
        use crate::blockchain_config::{CurveType, get_compatible_chains, generate_address_for_chain};

        // NOTE: `session.curve_type` is the string the *session* was
        // announced with — the TUI currently publishes "unified" regardless
        // of which curve actually ran. That's fine for signaling but useless
        // for address derivation because `CurveType::from_string("unified")`
        // returns `None` and `generate_address_for_chain` refuses to run.
        // The ciphersuite `C` carries the real curve identity at the type
        // level; `CurveIdentifier::curve_type()` materialises it as the
        // "secp256k1" / "ed25519" strings the chain helpers expect.
        let curve_type = C::curve_type().to_string();

        // Get ALL compatible chains for this curve and generate addresses
        let compatible_chains = get_compatible_chains(
            &CurveType::from_string(&curve_type).unwrap_or(CurveType::Secp256k1)
        );
        
        let mut generated_addresses = Vec::new();
        let mut blockchain_addresses = Vec::new();
        
        for (chain_id, _) in compatible_chains.iter() {
            match generate_address_for_chain(&group_public_key_bytes, &curve_type, chain_id) {
                Ok(address) => {
                    generated_addresses.push(format!("{}: {}", chain_id, address));
                    info!("Generated {} address: {}", chain_id, address);
                    
                    // Create BlockchainInfo for UI display
                    // Map chain_id to proper chain ID for EVM chains.
                    // `chain_id` is &&String here (from iterator of
                    // &(String, _)); `.as_ref()` is needed to deref down
                    // to &str for matching against the string literals
                    // below. Clippy's `useless_asref` misses this.
                    #[allow(clippy::useless_asref)]
                    let chain_id_num = match chain_id.as_ref() {
                        "ethereum" => Some(1u64),
                        "bsc" => Some(56u64),
                        "polygon" => Some(137u64),
                        "avalanche" => Some(43114u64),
                        "arbitrum" => Some(42161u64),
                        "optimism" => Some(10u64),
                        _ => None,
                    };
                    
                    // Determine address format based on chain
                    let addr_format = if chain_id == &"bitcoin" {
                        "P2WPKH".to_string()
                    } else if chain_id == &"solana" || chain_id == &"sui" || chain_id == &"aptos" {
                        "base58".to_string()
                    } else {
                        "EIP-55".to_string() // Ethereum and EVM chains
                    };
                    
                    let blockchain_info = crate::keystore::BlockchainInfo {
                        blockchain: chain_id.to_string(),
                        network: "mainnet".to_string(),
                        chain_id: chain_id_num,
                        address: address.clone(),
                        address_format: addr_format,
                        enabled: true,
                        rpc_endpoint: None,
                        metadata: None,
                    };
                    blockchain_addresses.push(blockchain_info);
                }
                Err(e) => {
                    warn!("Could not generate {} address: {}", chain_id, e);
                }
            }
        }
        
        // Store blockchain addresses for UI
        guard.blockchain_addresses = blockchain_addresses.clone();
        
        // Store the first compatible address for backward compatibility
        if let Some(first_address) = generated_addresses.first() {
            // Extract just the address part (after the ": ")
            if let Some(addr_part) = first_address.split(": ").nth(1) {
                guard.etherum_public_key = Some(addr_part.to_string());
            }
        }
        
        // Log successful DKG completion with real FROST key
        let display_address = guard.etherum_public_key.as_deref().unwrap_or("no address");
        info!("🎉 DKG completed successfully with REAL FROST!");
        info!("Wallet ID: {}, Primary Address: {}", wallet_id, display_address);
        info!("DKG State set to: {:?}", guard.dkg_state);
        info!("Generated {} blockchain addresses", guard.blockchain_addresses.len());
        for blockchain_info in &guard.blockchain_addresses {
            info!("  - {}: {}", blockchain_info.blockchain, blockchain_info.address);
        }
    }
}

/// Handle DKG finalization - simplified
pub async fn handle_dkg_finalization<C>(state: Arc<Mutex<AppState<C>>>) 
where
    C: Ciphersuite + Send + Sync + 'static,
{
    let mut guard = state.lock().await;
    
    if !matches!(guard.dkg_state, DkgState::Round2Complete) {
        return;
    }
    
    // Simple finalization
    guard.dkg_state = DkgState::Complete;
    
    info!("DKG finalization completed for device: {}", guard.device_id);
}

/// Finalize DKG - alias for compatibility
pub async fn finalize_dkg<C>(
    state: Arc<Mutex<AppState<C>>>,
    _device_id: String,  // Accept device_id parameter for compatibility
) 
where
    C: Ciphersuite + Send + Sync + 'static,
{
    handle_dkg_finalization(state).await;
}

/// Check if device is selected as signer - simplified helper
pub fn is_device_selected<C: Ciphersuite>(
    device_identifier: &Identifier<C>,
    selected_signers: &[Identifier<C>],
) -> bool {
    selected_signers.contains(device_identifier)
}

/// Create device ID to identifier map - simplified
pub fn create_device_id_map<C: Ciphersuite>(
    identifier_map: &std::collections::HashMap<String, Identifier<C>>
) -> std::collections::HashMap<Identifier<C>, String> {
    identifier_map.iter().map(|(k, v)| (*v, k.clone())).collect()
}

/// Map selected signers - stub
pub fn map_selected_signers<C: Ciphersuite>(
    _signers: Vec<String>
) -> Vec<Identifier<C>> {
    Vec::new()
}

/// Create signing package - stub
pub fn create_signing_package<C: Ciphersuite>(
    _message: &[u8],
    _signing_commitments: Vec<frost_core::round1::SigningCommitments<C>>,
) -> Result<frost_core::SigningPackage<C>, Box<dyn std::error::Error + Send + Sync>> {
    Err("Signing package creation is temporarily stubbed".into())
}

/// Generate signature share - stub
pub fn generate_signature_share<C: Ciphersuite>(
    _signing_package: &frost_core::SigningPackage<C>,
    _nonces: &frost_core::round1::SigningNonces<C>,
    _key_package: &frost_core::keys::KeyPackage<C>,
) -> Result<frost_core::round2::SignatureShare<C>, Box<dyn std::error::Error + Send + Sync>> {
    Err("Signature share generation is temporarily stubbed".into())
}

/// Aggregate signature - stub
pub fn aggregate_signature<C: Ciphersuite>(
    _signing_package: &frost_core::SigningPackage<C>,
    _signature_shares: &std::collections::BTreeMap<frost_core::Identifier<C>, frost_core::round2::SignatureShare<C>>,
    _group_public_key: &frost_core::VerifyingKey<C>,
) -> Result<frost_core::Signature<C>, Box<dyn std::error::Error + Send + Sync>> {
    Err("Signature aggregation is temporarily stubbed".into())
}

/// Generate signing commitment - stub
pub fn generate_signing_commitment<C: Ciphersuite>(
) -> Result<frost_core::round1::SigningCommitments<C>, Box<dyn std::error::Error + Send + Sync>> {
    Err("Signing commitment generation is temporarily stubbed".into())
}