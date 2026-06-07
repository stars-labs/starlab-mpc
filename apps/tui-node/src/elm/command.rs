//! Command - Side effects to be executed
//!
//! Commands represent operations that have side effects and need to be executed
//! outside of the pure update function. They handle async operations, I/O, and
//! interactions with external systems.

use crate::elm::message::{Message, SigningRequest};
use crate::elm::model::WalletConfig;
use tokio::sync::mpsc::UnboundedSender;
use std::path::PathBuf;
use tracing::{info, error, warn};

/// Commands represent side effects to be executed
#[derive(Debug, Clone)]
pub enum Command {
    // Data loading commands
    LoadWallets,
    LoadSessions,
    LoadWalletDetails { wallet_id: String },
    LoadSigningRequests,
    
    // Network operations
    // (Intentionally no ConnectWebSocket: `ReconnectWebSocket` already handles
    // both the initial dial and every subsequent redial, so there's no "connect
    // once then reconnect" distinction at the command layer.)
    ReconnectWebSocket,
    DisconnectWebSocket,
    SendNetworkMessage { to: String, data: Vec<u8> },
    BroadcastMessage { data: Vec<u8> },
    InitiateWebRTCConnections { participants: Vec<String> },
    VerifyWebRTCMesh,
    EnsureFullMesh,
    
    // Keystore operations
    InitializeKeystore { path: String, device_id: String },
    SaveWallet { wallet_data: Vec<u8> },
    DeleteWallet { wallet_id: String },
    ExportWallet { wallet_id: String, path: PathBuf },
    ImportWallet { path: PathBuf },
    
    // DKG operations
    /// Creator-only: mint session id, persist to AppState, broadcast
    /// AnnounceSession over the signaling WebSocket. Does NOT trigger the
    /// FROST cryptographic protocol — that waits for `StartFrostProtocol`
    /// once the WebRTC mesh is actually established.
    StartDKG { config: WalletConfig },
    /// Everyone (creator + joiners): the WebRTC mesh is up and data channels
    /// are reachable; run FROST Round 1 against the participants captured in
    /// `AppState::session`. No session announcement happens here, which is
    /// crucial: previously this logic lived inside `Command::StartDKG` and
    /// caused joiners to re-announce the session under their own `proposer_id`,
    /// clobbering the creator's record server-side.
    StartFrostProtocol,
    /// Process a peer's Round 1 package received over a data channel.
    /// Calls `protocal::dkg::process_dkg_round1` which stores the package and
    /// auto-triggers Round 2 once all `session.total` packages have arrived.
    ProcessDKGRound1 { from_device: String, package_bytes: Vec<u8> },
    /// Process a peer's Round 2 package received over a data channel.
    /// Calls `protocal::dkg::process_dkg_round2` which finalises the key with
    /// `part3` once all Round 2 packages for us have arrived.
    ProcessDKGRound2 { from_device: String, package_bytes: Vec<u8> },
    /// Reshare **initiator** (#56): load the OLD share from the keystore, seed
    /// the reshare context, and announce a `SessionType::Reshare` session. The
    /// refresh itself fires later via the shared mesh-ready path
    /// (`StartFrostProtocol`), exactly like DKG. Triggered by `HeadlessReshare`.
    StartReshare { wallet_id: String, password: String, keystore_path: String },
    /// Reshare **joiner** (#56): load the OLD share, seed the reshare context,
    /// and send a `SessionStatusUpdate` to join the announced reshare session so
    /// the mesh forms. Refresh fires via `StartFrostProtocol`. Dispatched from
    /// the SubmitPassword `Reshare` arm once the joiner accepts the invite.
    JoinReshare {
        session_id: String,
        wallet_name: String,
        total: u16,
        threshold: u16,
        proposer_id: String,
        curve_type: String,
        group_public_key: String,
        password: String,
        keystore_path: String,
    },
    /// Process a peer's reshare round-1 / round-2 package received over a data
    /// channel — drives `protocal::reshare`.
    ProcessReshareRound1 { from_device: String, package_bytes: Vec<u8> },
    ProcessReshareRound2 { from_device: String, package_bytes: Vec<u8> },
    JoinDKG {
        session_id: String,
        /// Session shape known to the joiner from the discovered announcement.
        /// Seeded into `AppState.session` so the joiner agrees on `total`
        /// immediately, rather than defaulting to 3 and racing on a later
        /// `SessionAvailable` re-broadcast (the headless joiner has already
        /// consumed that broadcast to discover the session). Curve falls back
        /// to the `available_sessions` lookup when empty.
        total: u16,
        threshold: u16,
        proposer_id: String,
        curve_type: String,
    },
    CancelDKG,
    /// Encrypt the just-produced FROST key share with `password` and
    /// persist it to the keystore, then emit `Message::DKGFinalized`.
    /// Consumes the cleartext password — the update-layer handler that
    /// dispatches this command must also clear
    /// `Model.wallet_state.pending_password` so we don't keep the
    /// password in two places.
    ///
    /// `keystore_path` and `wallet_name` are passed in rather than read
    /// off AppState because (a) the password is already a constructor
    /// parameter so we're committed to "pure input" anyway, and (b) the
    /// writable `Keystore` is constructed fresh inside the executor —
    /// `AppState.keystore` only stores a read-only `Arc<Keystore>`.
    FinalizeWalletFromDkg {
        password: String,
        keystore_path: String,
        wallet_name: String,
        /// Optional user-chosen display label (creator only). Persisted as
        /// keystore `metadata.label`; `None` → UI falls back to wallet_name.
        wallet_label: Option<String>,
    },
    /// Hot-load an existing wallet: decrypt the keystore file with
    /// `password`, deserialize the `(KeyPackage, PublicKeyPackage)` tuple
    /// from the blob (see `encode_keystore_blob`), and stash both onto
    /// `AppState` so the signing protocol layer has what it needs.
    ///
    /// `password` is taken by value — same discipline as
    /// `FinalizeWalletFromDkg`. The update-layer dispatcher is expected
    /// to clear the Model-side copy before dispatching; this Command is
    /// the last place the plaintext exists in-process.
    UnlockWallet {
        wallet_id: String,
        password: String,
        keystore_path: String,
    },

    // Signing operations
    StartSigning { request: SigningRequest },
    ApproveSignature { request_id: String },
    RejectSignature { request_id: String },
    /// Forward a peer's Round-1 signing commitment to the protocol layer.
    /// Dispatched by the `Message::ProcessSigningRound1` handler after the
    /// primary WebRTC reader decoded the `SIGN_COMMIT:<b64>` frame.
    ProcessSigningRound1 { from_device: String, commitment_bytes: Vec<u8> },
    /// Same shape for Round-2 signature shares.
    ProcessSigningRound2 { from_device: String, share_bytes: Vec<u8> },
    /// Joiner-side counterpart of `StartSigning`: record the
    /// just-accepted signing session on AppState, then (after the wallet
    /// has been unlocked) kick off `handle_start_signing` on the joiner's
    /// node. Mesh setup is reused from the prior DKG when available;
    /// cold-start mesh establishment is deferred to a later phase.
    JoinSigning {
        session_id: String,
        message_bytes: Vec<u8>,
    },
    
    // UI operations
    SendMessage(Message),
    ScheduleMessage { delay_ms: u64, message: Box<Message> },
    /// Run several commands in sequence. Later commands don't depend on earlier ones completing —
    /// they're dispatched in order on the same task, so use this for fire-and-forget side effects.
    Batch(Vec<Command>),
    RefreshUI,
    
    // Settings operations
    SaveSettings { websocket_url: String, device_id: String },
    LoadSettings,
    
    // System operations
    Quit,
    None,
}

/// Serialize `(KeyPackage, PublicKeyPackage)` into the byte stream that
/// `Keystore::create_wallet_multi_chain` encrypts for us.
///
/// Framing is `[kp_len: u32 LE][kp_bytes][pkp_len: u32 LE][pkp_bytes]` so the
/// reader doesn't need a separator and both halves can be length-checked
/// cheaply. We use FROST's own `.serialize()` (compact binary) rather than
/// JSON/bincode because the frost-core types don't all gate their serde
/// derives on the same feature flag and `.serialize()` is the
/// canonical round-trip pairing with `.deserialize()`.
pub(crate) fn encode_keystore_blob<C: frost_core::Ciphersuite>(
    key_package: &frost_core::keys::KeyPackage<C>,
    public_key_package: &frost_core::keys::PublicKeyPackage<C>,
) -> Result<Vec<u8>, String> {
    let kp = key_package
        .serialize()
        .map_err(|e| format!("KeyPackage::serialize: {:?}", e))?;
    let pkp = public_key_package
        .serialize()
        .map_err(|e| format!("PublicKeyPackage::serialize: {:?}", e))?;
    let kp_len = u32::try_from(kp.len()).map_err(|_| "KeyPackage too large".to_string())?;
    let pkp_len = u32::try_from(pkp.len()).map_err(|_| "PublicKeyPackage too large".to_string())?;

    let mut out = Vec::with_capacity(8 + kp.len() + pkp.len());
    out.extend_from_slice(&kp_len.to_le_bytes());
    out.extend_from_slice(&kp);
    out.extend_from_slice(&pkp_len.to_le_bytes());
    out.extend_from_slice(&pkp);
    Ok(out)
}

/// Inverse of [`encode_keystore_blob`]. Fails with a descriptive error on
/// any truncation, over-read, or FROST deserialize failure — so
/// `UnlockWallet` can turn those into a `WalletUnlockFailed` rather than a
/// panic on a malformed file.
pub(crate) fn decode_keystore_blob<C: frost_core::Ciphersuite>(
    bytes: &[u8],
) -> Result<
    (
        frost_core::keys::KeyPackage<C>,
        frost_core::keys::PublicKeyPackage<C>,
    ),
    String,
> {
    fn read_u32_le(buf: &[u8], offset: usize) -> Result<(u32, usize), String> {
        let end = offset
            .checked_add(4)
            .ok_or_else(|| "length overflow".to_string())?;
        if end > buf.len() {
            return Err(format!(
                "truncated: need 4 bytes at offset {offset}, have {}",
                buf.len().saturating_sub(offset)
            ));
        }
        let mut arr = [0u8; 4];
        arr.copy_from_slice(&buf[offset..end]);
        Ok((u32::from_le_bytes(arr), end))
    }

    let (kp_len, mut pos) = read_u32_le(bytes, 0)?;
    let kp_len = kp_len as usize;
    let kp_end = pos
        .checked_add(kp_len)
        .ok_or_else(|| "kp length overflow".to_string())?;
    if kp_end > bytes.len() {
        return Err(format!(
            "truncated KeyPackage: need {kp_len} bytes, have {}",
            bytes.len().saturating_sub(pos)
        ));
    }
    let kp = frost_core::keys::KeyPackage::<C>::deserialize(&bytes[pos..kp_end])
        .map_err(|e| format!("KeyPackage::deserialize: {:?}", e))?;
    pos = kp_end;

    let (pkp_len, npos) = read_u32_le(bytes, pos)?;
    pos = npos;
    let pkp_len = pkp_len as usize;
    let pkp_end = pos
        .checked_add(pkp_len)
        .ok_or_else(|| "pkp length overflow".to_string())?;
    if pkp_end > bytes.len() {
        return Err(format!(
            "truncated PublicKeyPackage: need {pkp_len} bytes, have {}",
            bytes.len().saturating_sub(pos)
        ));
    }
    let pkp = frost_core::keys::PublicKeyPackage::<C>::deserialize(&bytes[pos..pkp_end])
        .map_err(|e| format!("PublicKeyPackage::deserialize: {:?}", e))?;

    // Intentionally ignore trailing bytes — a future format with an extra
    // field would still decode up to this point. For today, there should
    // be no trailing bytes.
    Ok((kp, pkp))
}

/// Load an existing wallet's OLD share + metadata from the keystore and seed the
/// reshare context on `AppState` (keys, ORIGINAL participant set, persist creds,
/// `reshare_in_progress`). Shared by the reshare initiator (`StartReshare`) and
/// joiner (`JoinReshare`) so that — before the mesh forms and the refresh runs —
/// finalize has the old `KeyPackage` and the ORIGINAL identifier set (design §3,
/// never the retained/mesh set).
///
/// Returns `(original_participants, threshold, total_participants, curve_type,
/// group_public_key)` from the persisted metadata, for the announce/join. On any
/// failure returns `Err` and leaves `AppState` untouched (no `reshare_in_progress`).
#[allow(clippy::type_complexity)]
async fn seed_reshare_context<C: frost_core::Ciphersuite>(
    app_state: &std::sync::Arc<tokio::sync::Mutex<crate::utils::appstate_compat::AppState<C>>>,
    wallet_id: &str,
    password: &str,
    keystore_path: &str,
) -> Result<(Vec<String>, u16, u16, String, String), String> {
    use crate::keystore::Keystore;
    let device_id = { app_state.lock().await.device_id.clone() };
    let ks = Keystore::new(keystore_path, &device_id)
        .map_err(|e| format!("reshare: keystore open ({keystore_path}): {e}"))?;
    let meta = ks
        .get_wallet(wallet_id)
        .ok_or_else(|| format!("reshare: wallet '{wallet_id}' not found in keystore"))?
        .clone();
    let blob = ks
        .load_wallet_file(wallet_id, password)
        .map_err(|e| format!("reshare: unlock '{wallet_id}' failed: {e}"))?;
    let (key_package, public_key_package) =
        decode_keystore_blob::<C>(&blob).map_err(|e| format!("reshare: decode blob: {e}"))?;

    let mut state = app_state.lock().await;
    state.key_package = Some(key_package);
    state.group_public_key = Some(*public_key_package.verifying_key());
    state.public_key_package = Some(public_key_package);
    state.current_wallet_id = Some(wallet_id.to_string());
    // ORIGINAL participant set (design §3) — survivors keep their original FROST
    // ids even when a device is removed, so we always canonicalise over this.
    state.reshare_original_participants = meta.participants.clone();
    state.reshare_wallet_id = Some(wallet_id.to_string());
    state.reshare_password = Some(password.to_string());
    state.reshare_keystore_path = Some(keystore_path.to_string());
    state.reshare_in_progress = true;
    Ok((
        meta.participants.clone(),
        meta.threshold,
        meta.total_participants,
        meta.curve_type.clone(),
        meta.group_public_key.clone(),
    ))
}

/// Parse a `session_info` JSON blob (as sent over the wire by the Cloudflare
/// signal Worker) into a strongly-typed `SessionInfo`. Returns `None` if any
/// of the required scalar fields is missing or has the wrong type — callers
/// should log the raw blob so protocol drifts are debuggable.
pub(crate) fn parse_session_info(
    session_info: &serde_json::Value,
) -> Option<crate::protocal::signal::SessionInfo> {
    use crate::protocal::signal::{SessionInfo, SessionType};

    let session_id = session_info.get("session_id")?.as_str()?.to_string();
    let total = session_info.get("total")?.as_u64()? as u16;
    let threshold = session_info.get("threshold")?.as_u64()? as u16;

    let participants = session_info
        .get("participants")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let proposer_id = session_info
        .get("proposer_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    // Fallback when a wire message omits `curve_type` (protocol
    // violation by the announcer, or a legacy frame pre-dating the Stage-5
    // sweep). Default to `"secp256k1"` — the only curve this TUI binary
    // actually runs — rather than the useless `"unified"` placeholder
    // which `CurveType::from_string` rejects downstream. An incoming
    // valid announce will override this default anyway.
    let curve_type = session_info
        .get("curve_type")
        .and_then(|v| v.as_str())
        .unwrap_or("secp256k1")
        .to_string();
    let coordination_type = session_info
        .get("coordination_type")
        .and_then(|v| v.as_str())
        .unwrap_or("Network")
        .to_string();

    // `session_type` on the wire is a flat string flag: `"dkg"`,
    // `"signing"`, or `"reshare"`. For signing/reshare sessions we also carry
    // `wallet_name` and `group_public_key` in the announcement JSON so joiners
    // can cross-check against their local keystore without a separate query.
    let session_type_tag = session_info
        .get("session_type")
        .and_then(|v| v.as_str())
        .unwrap_or("dkg");
    let session_type = if session_type_tag == "reshare" {
        // Reshare announce: the joiner needs `wallet_name` to know which local
        // wallet to unlock, and `group_public_key` to confirm it owns that
        // exact wallet before refreshing. Degrade to empty strings if absent —
        // downstream matches care about the variant shape.
        let wallet_name = session_info
            .get("wallet_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let group_public_key = session_info
            .get("group_public_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        SessionType::Reshare {
            wallet_name,
            curve_type: curve_type.clone(),
            group_public_key,
        }
    } else if session_type_tag == "signing" {
        // Degrade gracefully: if the signing-specific fields aren't
        // present we still produce a Signing variant with empty
        // strings, since the downstream matches care about the variant
        // shape more than the payload.
        let wallet_name = session_info
            .get("wallet_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let group_public_key = session_info
            .get("group_public_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let blockchain = session_info
            .get("blockchain")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        SessionType::Signing {
            wallet_name,
            curve_type: curve_type.clone(),
            blockchain,
            group_public_key,
        }
    } else {
        SessionType::DKG
    };

    // Pull `signing_message_hex` out of the signing announce if present —
    // joiners use this to run the same ceremony the creator started.
    let signing_message_hex = session_info
        .get("signing_message_hex")
        .or_else(|| session_info.get("message_hex"))
        .and_then(|v| v.as_str())
        .map(String::from);

    Some(SessionInfo {
        session_id,
        proposer_id,
        total,
        threshold,
        participants,
        session_type,
        curve_type,
        coordination_type,
        signing_message_hex,
    })
}

impl Command {
    /// Execute the command and send resulting messages back to the update loop
    pub async fn execute<C>(
        self,
        tx: UnboundedSender<Message>,
        app_state: &std::sync::Arc<tokio::sync::Mutex<crate::utils::appstate_compat::AppState<C>>>,
    ) -> anyhow::Result<()>
    where
        // Bounds all in the where-clause to avoid clippy's
        // `multiple_bound_locations` lint (prior form had
        // `<C: frost_core::Ciphersuite + ...>` on the fn header AND
        // `C: CurveIdentifier` in the where-clause).
        C: frost_core::Ciphersuite + Send + Sync + 'static,
        <<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
        <<<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
        // Needed by `process_dkg_round2` so the completion path can derive
        // the real curve name ("secp256k1" / "ed25519") from the generic
        // `C` for blockchain-address generation. Both ciphersuites the TUI
        // instantiates implement this trait.
        C: crate::utils::curve_traits::CurveIdentifier,
    {
        match self {
            Command::LoadWallets => {
                info!("Loading wallets from keystore");
                
                let state = app_state.lock().await;
                if let Some(ref keystore) = state.keystore {
                    let wallets = keystore.list_wallets();
                    // Convert Vec<&WalletMetadata> to Vec<WalletMetadata> by cloning
                    let wallets: Vec<crate::keystore::WalletMetadata> = wallets.into_iter()
                        .cloned()
                        .collect();
                    let _ = tx.send(Message::WalletsLoaded { wallets });
                } else {
                    let _ = tx.send(Message::Error { 
                        message: "Keystore not initialized".to_string() 
                    });
                }
            }
            
            Command::LoadSessions => {
                // Send `RequestActiveSessions` on the shared primary WebSocket. The
                // server now replies with one `SessionAvailable` frame per stored
                // session, and the primary reader converts each into a
                // `Message::SessionDiscovered` — so the UI fills in live as replies
                // arrive. No temp socket, no 2-second swallow.
                info!("Refreshing session list via primary WebSocket");

                // Optimistically clear the list so stale entries don't linger if a
                // session was removed while this TUI wasn't looking.
                let _ = tx.send(Message::SessionsLoaded { sessions: vec![] });

                let ws_tx_opt = {
                    let state = app_state.lock().await;
                    state.websocket_msg_tx.clone()
                };

                let Some(ws_tx) = ws_tx_opt else {
                    warn!(
                        "LoadSessions: primary WebSocket not connected yet; discovery \
                         will populate once `WebSocketConnected` fires"
                    );
                    let _ = tx.send(Message::Info {
                        message: "Waiting for signal server connection...".to_string(),
                    });
                    return Ok(());
                };

                let request = webrtc_signal_server::ClientMsg::RequestActiveSessions;
                match serde_json::to_string(&request) {
                    Ok(json) => {
                        if let Err(e) = ws_tx.send(json) {
                            warn!("LoadSessions: primary channel closed: {}", e);
                        }
                    }
                    Err(e) => error!("LoadSessions: failed to serialize request: {}", e),
                }
            }
            
            Command::LoadWalletDetails { wallet_id } => {
                info!("Loading details for wallet: {}", wallet_id);
                
                let state = app_state.lock().await;
                if let Some(ref keystore) = state.keystore {
                    if let Some(_wallet) = keystore.get_wallet(&wallet_id) {
                        // Wallet details loaded, update UI
                        let _ = tx.send(Message::Success { 
                            message: format!("Wallet {} loaded", wallet_id) 
                        });
                    } else {
                        let _ = tx.send(Message::Error { 
                            message: format!("Wallet {} not found", wallet_id) 
                        });
                    }
                }
            }
            
            Command::InitializeKeystore { path, device_id } => {
                info!("Initializing keystore at: {}", path);
                
                use crate::keystore::Keystore;
                match Keystore::new(&path, &device_id) {
                    Ok(keystore) => {
                        let mut state = app_state.lock().await;
                        state.keystore = Some(std::sync::Arc::new(keystore));
                        let _ = tx.send(Message::KeystoreInitialized { path });
                    }
                    Err(e) => {
                        error!("Failed to initialize keystore: {}", e);
                        let _ = tx.send(Message::KeystoreError { 
                            error: e.to_string() 
                        });
                    }
                }
            }
            
            Command::StartDKG { config } => {
                // Creator-only path. Responsibility: mint a session_id, store
                // it in AppState, broadcast AnnounceSession so joiners can
                // discover us. FROST Round 1 is NOT triggered here — it needs
                // the WebRTC mesh + populated session.participants, neither of
                // which exist yet. `Command::StartFrostProtocol` does that when
                // mesh-ready fires.
                info!("Creator path: create + announce session. config={:?}", config);

                {
                    let mut state = app_state.lock().await;
                    if state.dkg_in_progress {
                        info!("⚠️ DKG already in progress, skipping duplicate StartDKG");
                        let _ = tx.send(Message::Info {
                            message: "DKG already in progress, please wait...".to_string(),
                        });
                        return Ok(());
                    }
                    state.dkg_in_progress = true;
                }

                if config.mode == crate::elm::model::WalletMode::Online {
                    // For online mode, use the real DKG session manager
                    info!("Online mode - need {} participants with threshold {}", 
                          config.total_participants, config.threshold);
                    
                    // Send initial progress
                    let _ = tx.send(Message::UpdateDKGProgress { 
                        round: crate::elm::message::DKGRound::Initialization,
                        progress: 0.1,
                    });
                    
                    // Start the real DKG with session manager
                    let tx_clone = tx.clone();
                    let config_clone = config.clone();
                    
                    // Note: We can't use tokio::spawn here due to Send/Sync constraints
                    // with FROST cryptographic types. For now, show informative messages.

                    // CRITICAL FIX: Check if we already have an active session ID
                    // This prevents creating new sessions on WebSocket reconnection
                    let session_id = {
                        let state = app_state.lock().await;
                        if let Some(ref session) = state.session {
                            // Reuse existing session ID to prevent session chaos
                            info!("🔄 Reusing existing session ID: {}", session.session_id);
                            session.session_id.clone()
                        } else {
                            // Only generate new session ID if we don't have one
                            let new_id = format!("dkg_{}", uuid::Uuid::new_v4());
                            info!("🆕 Creating new session ID: {}", new_id);
                            new_id
                        }
                    };

                    let _ = tx_clone.send(Message::UpdateDKGSessionId {
                        real_session_id: session_id.clone()
                    });
                    
                    let _ = tx_clone.send(Message::Info { 
                        message: format!("📝 Created DKG session: {}", session_id)
                    });
                    
                    // Show instructions
                    let _ = tx_clone.send(Message::Info { 
                        message: "📋 To complete REAL DKG in online mode:".to_string()
                    });
                    let _ = tx_clone.send(Message::Info { 
                        message: format!("1. Share session ID '{}' with other participants", session_id)
                    });
                    let _ = tx_clone.send(Message::Info { 
                        message: "2. Each participant must run this TUI with 'Join Session'".to_string()
                    });
                    let _ = tx_clone.send(Message::Info { 
                        message: format!("3. Need {} total participants connected", config_clone.total_participants)
                    });
                    
                    // Acquire the shared primary-WebSocket handles (`websocket_msg_tx`
                    // for outbound JSON, `server_msg_broadcast_tx` for parsed-frame
                    // fan-out). These are installed exactly once, by
                    // `Command::ReconnectWebSocket`, at first connect. StartDKG does
                    // NOT dial or register — those already happened.
                    let device_id = {
                        let state = app_state.lock().await;
                        state.device_id.clone()
                    };
                    let (ws_tx, broadcast_tx) = {
                        let state = app_state.lock().await;
                        match (
                            state.websocket_msg_tx.clone(),
                            state.server_msg_broadcast_tx.clone(),
                        ) {
                            (Some(ws), Some(bt)) => (ws, bt),
                            _ => {
                                warn!("StartDKG: primary WebSocket not up — can't announce");
                                let _ = tx_clone.send(Message::DKGFailed {
                                    error: "Signal server not connected. Wait for reconnect and try again.".to_string(),
                                });
                                drop(state);
                                let mut s = app_state.lock().await;
                                s.dkg_in_progress = false;
                                return Ok(());
                            }
                        }
                    };

                    // Announce the session through the shared channel.
                    // `curve_type` comes from the ciphersuite type witness
                    // via `CurveIdentifier`, so joiners learn the real curve
                    // from the announce rather than the legacy "unified"
                    // placeholder.
                    let announced_curve =
                        <C as crate::utils::curve_traits::CurveIdentifier>::curve_type();
                    let session_info = serde_json::json!({
                        "session_id": session_id.clone(),
                        "total": config_clone.total_participants,
                        "threshold": config_clone.threshold,
                        "session_type": "dkg",
                        "proposer_id": device_id.clone(),
                        "participants": [device_id.clone()],
                        "curve_type": announced_curve,
                        "coordination_type": "Network",
                    });
                    let announce = webrtc_signal_server::ClientMsg::AnnounceSession {
                        session_info,
                    };
                    match serde_json::to_string(&announce) {
                        Ok(json) => {
                            info!("Announcing session: {}", json);
                            if ws_tx.send(json).is_err() {
                                let _ = tx_clone.send(Message::Error {
                                    message: "Primary WebSocket channel closed mid-announce".to_string(),
                                });
                            } else {
                                let _ = tx_clone.send(Message::Info {
                                    message: format!("📝 Session created: {}", session_id),
                                });
                            }
                        }
                        Err(e) => error!("Serialize AnnounceSession failed: {}", e),
                    }

                    // Record session state with the real curve — same
                    // source of truth as the announcement above.
                    {
                        let mut state = app_state.lock().await;
                        state.session = Some(crate::protocal::signal::SessionInfo {
                            session_id: session_id.clone(),
                            proposer_id: device_id.clone(),
                            participants: vec![device_id.clone()],
                            threshold: config_clone.threshold,
                            total: config_clone.total_participants,
                            session_type: crate::protocal::signal::SessionType::DKG,
                            curve_type: announced_curve.to_string(),
                            coordination_type: "Network".to_string(),
                            signing_message_hex: None,
                        });
                    }

                    let _ = tx_clone.send(Message::Info {
                        message: "⏳ Waiting for other participants to join...".to_string(),
                    });
                    let _ = tx_clone.send(Message::UpdateDKGProgress {
                        round: crate::elm::message::DKGRound::WaitingForParticipants,
                        progress: 0.2,
                    });


                            // Spawn a task to handle incoming WebSocket messages
                            let tx_msg = tx_clone.clone();
                            let session_id_clone = session_id.clone();
                            let total_participants = config_clone.total_participants;
                            let device_id_clone = device_id.clone();

                            // Subscribe to the shared server-message fan-out owned by
                            // `Command::ReconnectWebSocket`. We see every parsed frame
                            // without maintaining our own socket. Relay frames (peer
                            // WebRTC signals + participant_update) are handled by the
                            // always-on relay handler (spawn_relay_handler_task), not
                            // here — this loop only mirrors display/roster updates.
                            let mut broadcast_rx = broadcast_tx.subscribe();

                            tokio::spawn(async move {
                                let mut participants_seen = std::collections::HashSet::new();
                                participants_seen.insert(device_id_clone.clone());

                                loop {
                                    let shared = match broadcast_rx.recv().await {
                                        Ok(m) => m,
                                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                            warn!("DKG driver lagged {} messages; continuing", n);
                                            continue;
                                        }
                                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                            info!("DKG driver: broadcast channel closed, exiting");
                                            let _ = tx_msg.send(Message::WebSocketDisconnected);
                                            break;
                                        }
                                    };
                                    match &*shared {
                                                    webrtc_signal_server::ServerMsg::SessionAvailable { session_info } => {
                                                        // Another participant announced a session - check if it's us joining theirs
                                                        if let Some(sid) = session_info.get("session_id").and_then(|v| v.as_str())
                                                            && sid != session_id_clone {
                                                                // Different session
                                                                let _ = tx_msg.send(Message::Info { 
                                                                    message: format!("📢 Another session available: {}", sid)
                                                                });
                                                            }
                                                    }
                                                    webrtc_signal_server::ServerMsg::Devices { devices } => {
                                                        // Display-only: show the raw signal-server
                                                        // device roster. `Devices` fires on every WS
                                                        // register/deregister, which is NOT the same
                                                        // as "joined this session" — a fresh peer that
                                                        // just hit Welcome also shows up here. We used
                                                        // to use this as the trigger for WebRTC init,
                                                        // which fired before joiners had a broadcast
                                                        // subscriber alive, so offers vanished into a
                                                        // dead channel. The authoritative "all joined"
                                                        // signal is `participant_update` via Relay,
                                                        // handled in `webrtc_signaling::handle_server_frame`.
                                                        let _ = tx_msg.send(Message::Info {
                                                            message: format!("📡 Connected devices: {:?}", devices),
                                                        });
                                                        for device in devices.iter() {
                                                            participants_seen.insert(device.clone());
                                                        }
                                                        let participants_list: Vec<String> =
                                                            participants_seen.iter().cloned().collect();
                                                        let _ = tx_msg.send(Message::UpdateParticipants {
                                                            participants: participants_list,
                                                        });
                                                        let _ = &total_participants; // silence unused capture
                                                    }
                                        // Relay frames handled by the always-on
                                        // relay handler, not this loop.
                                        _ => {}
                                    }
                                }
                            });

                            // Show current participant count
                            let _ = tx_clone.send(Message::Info {
                                message: format!("👥 Current participants: 1/{}", config_clone.total_participants)
                            });
                            
                            // Update DKG progress to show we're waiting for participants  
                            let _ = tx_clone.send(Message::UpdateDKGProgress {
                                round: crate::elm::message::DKGRound::WaitingForParticipants,
                                progress: 0.2,
                            });
                            
                            // Keep the DKG progress screen open and wait for participants
                            // Don't automatically fail - let the user cancel if they want
                            let _ = tx_clone.send(Message::Info { 
                                message: format!("⏳ Waiting for {} more participants...", config_clone.total_participants - 1)
                            });
                            
                            let _ = tx_clone.send(Message::Info { 
                                message: format!("📋 Share this session ID with other participants: {}", session_id)
                            });
                            
                            // The broadcast subscriber task will continue listening
                            // for participants joining. User can press Esc to cancel.
                } else {
                    // Offline mode - use SD card exchange
                    info!("Offline mode selected - air-gapped DKG");
                    
                    let _ = tx.send(Message::Info { 
                        message: "🔒 Offline DKG Mode".to_string()
                    });
                    let _ = tx.send(Message::Info { 
                        message: "📋 Steps for offline DKG:".to_string()
                    });
                    let _ = tx.send(Message::Info { 
                        message: "1. Each participant generates their Round 1 commitment".to_string()
                    });
                    let _ = tx.send(Message::Info { 
                        message: "2. Export commitments to SD card".to_string()
                    });
                    let _ = tx.send(Message::Info { 
                        message: "3. Exchange SD cards physically".to_string()
                    });
                    let _ = tx.send(Message::Info { 
                        message: "4. Import other participants' commitments".to_string()
                    });
                    let _ = tx.send(Message::Info { 
                        message: "5. Generate and exchange Round 2 shares".to_string()
                    });
                    
                    // TODO: Implement offline DKG with SD card exchange
                    let _ = tx.send(Message::DKGFailed {
                        error: "Offline DKG implementation in progress. For now, please use online mode with multiple nodes.".to_string()
                    });
                }
            }

            Command::StartFrostProtocol => {
                // Triggered on every node when its WebRTC mesh is ready. Reads
                // the session that `StartDKG` (creator) / `JoinDKG` (joiner)
                // stashed in AppState, then kicks FROST Round 1. This is the
                // one and only place `handle_trigger_dkg_round1` is called —
                // previously it was baked into `Command::StartDKG`, so joiners
                // either didn't run it at all or ran it with stale state.
                //
                // Guard against double-fire: mesh-check can spawn multiple times
                // (one per `InitiateWebRTCWithParticipants`). Running Round 1
                // twice would regenerate the secret/package and break the
                // protocol mid-flight. We atomically transition dkg_state to
                // `Round1InProgress` only from `Idle` — subsequent calls bail.
                let (device_id, internal_cmd_tx, have_session, already_running, is_reshare, dkg_in_progress) = {
                    let mut state_guard = app_state.lock().await;
                    let already = !matches!(state_guard.dkg_state, crate::utils::state::DkgState::Idle);
                    if !already {
                        state_guard.dkg_state = crate::utils::state::DkgState::Round1InProgress;
                    }
                    (
                        state_guard.device_id.clone(),
                        state_guard.websocket_internal_cmd_tx.clone(),
                        state_guard.session.is_some(),
                        already,
                        state_guard.reshare_in_progress,
                        state_guard.dkg_in_progress,
                    )
                };
                if already_running {
                    info!("StartFrostProtocol: FROST already running — ignoring duplicate trigger");
                    return Ok(());
                }
                if !have_session {
                    warn!(
                        "StartFrostProtocol fired but AppState::session is None — ignoring"
                    );
                    // Roll back the state transition since we didn't actually run.
                    let mut state = app_state.lock().await;
                    state.dkg_state = crate::utils::state::DkgState::Idle;
                    return Ok(());
                }
                // Guard against a STALE-session mesh: when a node reconnects to a
                // signal server that still lists a previously-completed session
                // (e.g. after a process restart), that session's mesh can re-form
                // and fire mesh-ready even though this node has NO active ceremony
                // intent. Running DKG Round 1 then would (a) waste a ceremony and
                // (b) pin `dkg_state` to Round1InProgress, blocking a subsequent
                // *reshare* on the same node (its StartFrostProtocol would be
                // ignored as "already running"). Only proceed when a ceremony is
                // genuinely pending: a reshare (`reshare_in_progress`) or a DKG
                // (`dkg_in_progress`, set by StartDKG/JoinDKG). Signing is immune —
                // it never routes through this path.
                if !is_reshare && !dkg_in_progress {
                    warn!(
                        "StartFrostProtocol fired with no DKG/reshare in progress \
                         (stale-session mesh?) — ignoring"
                    );
                    let mut state = app_state.lock().await;
                    state.dkg_state = crate::utils::state::DkgState::Idle;
                    return Ok(());
                }

                // Reshare fork: the mesh-ready path is identical for DKG and
                // reshare (announce → join → mesh → StartFrostProtocol). The only
                // difference is which FROST ceremony runs. When the initiator
                // (`StartReshare`) / joiner (`JoinReshare`) seeded a reshare,
                // `reshare_in_progress` is set — refresh the share instead of
                // generating a fresh key. (The OLD share + ORIGINAL participant
                // set were loaded from the keystore by those commands.)
                if is_reshare {
                    info!(
                        "🔄 Triggering FROST reshare Round 1 for device_id={}",
                        device_id
                    );
                    crate::protocal::reshare::handle_trigger_reshare_round1(
                        app_state.clone(),
                        device_id.clone(),
                        tx.clone(),
                    )
                    .await;
                    let _ = tx.send(Message::Info {
                        message: "✅ Reshare Round 1 initiated - refreshing share...".to_string(),
                    });
                    return Ok(());
                }

                let internal_tx = internal_cmd_tx.unwrap_or_else(|| {
                    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
                    tx
                });

                info!(
                    "🌐 Triggering unified FROST DKG Round 1 for device_id={}",
                    device_id
                );
                crate::protocal::dkg::handle_trigger_dkg_round1(
                    app_state.clone(),
                    device_id.clone(),
                    internal_tx,
                )
                .await;
                info!("✅ FROST Round 1 trigger returned");

                let _ = tx.send(Message::Info {
                    message: "✅ DKG Round 1 initiated - exchanging commitments...".to_string(),
                });
            }

            Command::ProcessDKGRound1 {
                from_device,
                package_bytes,
            } => {
                info!(
                    "Calling process_dkg_round1 for {} ({} bytes)",
                    from_device,
                    package_bytes.len()
                );
                crate::protocal::dkg::process_dkg_round1(
                    app_state.clone(),
                    from_device,
                    package_bytes,
                )
                .await;
                // `process_dkg_round1` internally transitions to Round 2 and
                // calls `handle_trigger_dkg_round2`. On fast transports the
                // round2 re-feed inside that path can complete DKG right here
                // (peer round2 arrived before our part2), so check for the
                // finished key and notify the UI — same detection as
                // ProcessDKGRound2 below.
                let group_key_hex = {
                    let state = app_state.lock().await;
                    state
                        .public_key_package
                        .as_ref()
                        .and_then(|pkg| pkg.verifying_key().serialize().ok())
                        .map(hex::encode)
                };
                if let Some(hex) = group_key_hex {
                    let _ = tx.send(Message::DKGKeyGenerated {
                        group_pubkey_hex: hex,
                    });
                }
            }

            Command::StartReshare { wallet_id, password, keystore_path } => {
                // Reshare INITIATOR (#56). Mirrors `StartDKG`: load the OLD share
                // from the keystore, seed the reshare context, then announce a
                // `SessionType::Reshare` session so the retained signers can
                // discover + join. The refresh itself does NOT run here — it
                // fires on every node via the shared mesh-ready path
                // (`StartFrostProtocol`, which forks on `reshare_in_progress`),
                // exactly like DKG Round 1.
                info!("Reshare initiator: load '{}' + announce reshare session", wallet_id);
                let (orig_participants, threshold, total, curve_type, group_pk) =
                    match seed_reshare_context::<C>(app_state, &wallet_id, &password, &keystore_path)
                        .await
                    {
                        Ok(v) => v,
                        Err(e) => {
                            error!("{e}");
                            let _ = tx.send(Message::Error { message: e });
                            return Ok(());
                        }
                    };
                drop(password);

                // Same-set reshare: retain ALL original participants. (Reduced-set
                // / device-removal threads a keep-list in a follow-up; the §3
                // identifier logic + finalize already support non-contiguous ids.)
                let retained_total = if total as usize == orig_participants.len() && total > 0 {
                    total
                } else {
                    orig_participants.len() as u16
                };

                let device_id = { app_state.lock().await.device_id.clone() };
                let ws_tx = {
                    let state = app_state.lock().await;
                    match state.websocket_msg_tx.clone() {
                        Some(ws) => ws,
                        None => {
                            warn!("StartReshare: primary WebSocket not up — can't announce");
                            let _ = tx.send(Message::Error {
                                message: "Signal server not connected. Wait for reconnect."
                                    .to_string(),
                            });
                            let mut s = app_state.lock().await;
                            crate::protocal::reshare::clear_reshare_state(&mut s);
                            return Ok(());
                        }
                    }
                };

                let session_id = format!("reshare_{}", uuid::Uuid::new_v4());
                let announce = webrtc_signal_server::ClientMsg::AnnounceSession {
                    session_info: serde_json::json!({
                        "session_id": session_id.clone(),
                        "total": retained_total,
                        "threshold": threshold,
                        "session_type": "reshare",
                        "proposer_id": device_id.clone(),
                        "participants": [device_id.clone()],
                        "curve_type": curve_type.clone(),
                        "coordination_type": "Network",
                        "wallet_name": wallet_id.clone(),
                        "group_public_key": group_pk.clone(),
                    }),
                };
                match serde_json::to_string(&announce) {
                    Ok(json) => {
                        info!("Announcing reshare session: {}", json);
                        if ws_tx.send(json).is_err() {
                            let _ = tx.send(Message::Error {
                                message: "Primary WebSocket closed mid reshare-announce".to_string(),
                            });
                        }
                    }
                    Err(e) => error!("Serialize reshare AnnounceSession failed: {}", e),
                }

                {
                    let mut state = app_state.lock().await;
                    state.session = Some(crate::protocal::signal::SessionInfo {
                        session_id: session_id.clone(),
                        proposer_id: device_id.clone(),
                        participants: vec![device_id.clone()],
                        threshold,
                        total: retained_total,
                        session_type: crate::protocal::signal::SessionType::Reshare {
                            wallet_name: wallet_id.clone(),
                            curve_type: curve_type.clone(),
                            group_public_key: group_pk.clone(),
                        },
                        curve_type,
                        coordination_type: "Network".to_string(),
                        signing_message_hex: None,
                    });
                }
                let _ = tx.send(Message::Info {
                    message: format!("🔄 Reshare session announced: {} — waiting for signers", session_id),
                });
            }

            Command::JoinReshare {
                session_id,
                wallet_name,
                total,
                threshold,
                proposer_id,
                curve_type,
                group_public_key,
                password,
                keystore_path,
            } => {
                // Reshare JOINER (#56). Mirrors `JoinDKG`: load the OLD share,
                // seed the reshare context, then send a `SessionStatusUpdate` so
                // the server+initiator grow the participant set and the mesh
                // forms. Refresh fires via the shared `StartFrostProtocol` path.
                info!("Reshare joiner: load '{}' + join session {}", wallet_name, session_id);
                if let Err(e) =
                    seed_reshare_context::<C>(app_state, &wallet_name, &password, &keystore_path)
                        .await
                {
                    error!("{e}");
                    let _ = tx.send(Message::Error { message: e });
                    return Ok(());
                }
                drop(password);

                let device_id = { app_state.lock().await.device_id.clone() };
                let ws_tx = {
                    let state = app_state.lock().await;
                    match state.websocket_msg_tx.clone() {
                        Some(ws) => ws,
                        None => {
                            warn!("JoinReshare: primary WebSocket not up — can't join");
                            let _ = tx.send(Message::Error {
                                message: "Signal server not connected. Wait for reconnect."
                                    .to_string(),
                            });
                            let mut s = app_state.lock().await;
                            crate::protocal::reshare::clear_reshare_state(&mut s);
                            return Ok(());
                        }
                    }
                };

                let status_msg = webrtc_signal_server::ClientMsg::SessionStatusUpdate {
                    session_info: serde_json::json!({
                        "session_id": session_id.clone(),
                        "participant_joined": device_id.clone(),
                    }),
                };
                match serde_json::to_string(&status_msg) {
                    Ok(json) => {
                        if ws_tx.send(json).is_err() {
                            let _ = tx.send(Message::Error {
                                message: "Primary WS closed mid reshare-join".to_string(),
                            });
                        }
                    }
                    Err(e) => error!("Serialize reshare SessionStatusUpdate: {}", e),
                }

                {
                    let mut state = app_state.lock().await;
                    state.session = Some(crate::protocal::signal::SessionInfo {
                        session_id: session_id.clone(),
                        proposer_id: if proposer_id.is_empty() {
                            "unknown".to_string()
                        } else {
                            proposer_id
                        },
                        participants: vec![device_id.clone()],
                        threshold,
                        total,
                        session_type: crate::protocal::signal::SessionType::Reshare {
                            wallet_name,
                            curve_type: curve_type.clone(),
                            group_public_key,
                        },
                        curve_type,
                        coordination_type: "Network".to_string(),
                        signing_message_hex: None,
                    });
                }
                let _ = tx.send(Message::Info {
                    message: format!("🔄 Joined reshare session: {}", session_id),
                });
            }

            Command::ProcessReshareRound1 {
                from_device,
                package_bytes,
            } => {
                crate::protocal::reshare::process_reshare_round1(
                    app_state.clone(),
                    from_device,
                    package_bytes,
                    tx.clone(),
                )
                .await;
            }

            Command::ProcessReshareRound2 {
                from_device,
                package_bytes,
            } => {
                crate::protocal::reshare::process_reshare_round2(
                    app_state.clone(),
                    from_device,
                    package_bytes,
                    tx.clone(),
                )
                .await;
            }

            Command::ProcessDKGRound2 {
                from_device,
                package_bytes,
            } => {
                info!(
                    "Calling process_dkg_round2 for {} ({} bytes)",
                    from_device,
                    package_bytes.len()
                );
                crate::protocal::dkg::process_dkg_round2(
                    app_state.clone(),
                    from_device,
                    package_bytes,
                )
                .await;
                // `process_dkg_round2` runs `part3` internally and populates the
                // key_package / public_key_package on AppState once complete.
                // Check whether we've just crossed that threshold and notify UI.
                let group_key_hex = {
                    let state = app_state.lock().await;
                    state
                        .public_key_package
                        .as_ref()
                        .and_then(|pkg| pkg.verifying_key().serialize().ok())
                        .map(hex::encode)
                };
                if let Some(hex) = group_key_hex {
                    let _ = tx.send(Message::DKGKeyGenerated {
                        group_pubkey_hex: hex,
                    });
                }
            }

            Command::JoinDKG { session_id, total: known_total, threshold: known_threshold, proposer_id: known_proposer, curve_type: known_curve } => {
                info!("Joining DKG session: {} ({}-of-{})", session_id, known_threshold, known_total);
                let _ = tx.send(Message::Info {
                    message: format!("🔗 Joining DKG session: {}", session_id)
                });

                // Acquire the shared primary-WS handles. `ReconnectWebSocket`
                // already dialed and registered at app start — Join doesn't dial.
                // Also set `dkg_in_progress` so any stray `Command::StartDKG`
                // (e.g. from an accidentally re-triggered CreateWallet flow) bails
                // at the dedupe check and can't re-announce the session as us.
                let device_id = {
                    let mut state = app_state.lock().await;
                    state.dkg_in_progress = true;
                    state.device_id.clone()
                };
                let tx_clone = tx.clone();
                let (ws_tx, broadcast_tx) = {
                    let state = app_state.lock().await;
                    match (
                        state.websocket_msg_tx.clone(),
                        state.server_msg_broadcast_tx.clone(),
                    ) {
                        (Some(ws), Some(bt)) => (ws, bt),
                        _ => {
                            warn!("JoinDKG: primary WebSocket not up — can't join");
                            let _ = tx_clone.send(Message::DKGFailed {
                                error: "Signal server not connected. Wait for reconnect.".to_string(),
                            });
                            return Ok(());
                        }
                    }
                };

                // Send a SessionStatusUpdate so the server+creator learn we're in.
                let session_update = serde_json::json!({
                    "session_id": session_id.clone(),
                    "participant_joined": device_id.clone(),
                });
                let status_msg = webrtc_signal_server::ClientMsg::SessionStatusUpdate {
                    session_info: session_update,
                };
                match serde_json::to_string(&status_msg) {
                    Ok(json) => {
                        if ws_tx.send(json).is_err() {
                            let _ = tx_clone.send(Message::Error {
                                message: "Primary WS channel closed mid-join".to_string(),
                            });
                        } else {
                            let _ = tx_clone.send(Message::Info {
                                message: format!("✅ Joined session: {}", session_id)
                            });
                            let _ = tx_clone.send(Message::UpdateDKGProgress {
                                round: crate::elm::message::DKGRound::WaitingForParticipants,
                                progress: 0.2,
                            });
                        }
                    }
                    Err(e) => error!("Serialize SessionStatusUpdate: {}", e),
                }

                // Provisional session state — curve_type/threshold get overwritten
                // as soon as the creator's SessionAvailable arrives on the broadcast.
                {
                    let mut state = app_state.lock().await;
                    // Prefer the curve the joiner already learned from the
                    // announcement; fall back to the available_sessions lookup.
                    let curve_type = if !known_curve.is_empty() {
                        known_curve.clone()
                    } else {
                        state.available_sessions.iter()
                            .find(|s| s.session_code == session_id)
                            .map(|s| s.curve_type.clone())
                            .unwrap_or_else(|| "Ed25519".to_string())
                    };
                    info!("📊 Joining session with curve type: {}, total {}", curve_type, known_total);
                    state.session = Some(crate::protocal::signal::SessionInfo {
                        session_id: session_id.clone(),
                        proposer_id: if known_proposer.is_empty() {
                            "unknown".to_string()
                        } else {
                            known_proposer.clone()
                        },
                        participants: vec![device_id.clone()],
                        // Seed the REAL shape from the discovered announcement so
                        // `session.total` is correct from the start (no race on a
                        // later SessionAvailable re-broadcast).
                        threshold: known_threshold,
                        total: known_total,
                        session_type: crate::protocal::signal::SessionType::DKG,
                        curve_type,
                        coordination_type: "Network".to_string(),
                        signing_message_hex: None,
                    });
                }

                // Capture broadcast subscription + context for the driver task.
                let tx_msg = tx_clone.clone();
                let session_id_clone = session_id.clone();
                let device_id_clone = device_id.clone();
                let session_total = known_total; // Known from the discovered announcement.
                let app_state_clone = app_state.clone();
                let mut broadcast_rx = broadcast_tx.subscribe();

                        tokio::spawn(async move {
                            let mut participants_seen = std::collections::HashSet::new();
                            // Don't add ourselves yet - wait for server to confirm

                            loop {
                                let shared = match broadcast_rx.recv().await {
                                    Ok(m) => m,
                                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                        warn!("JoinDKG driver lagged {} messages; continuing", n);
                                        continue;
                                    }
                                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                        info!("JoinDKG driver: broadcast closed");
                                        let _ = tx_msg.send(Message::WebSocketDisconnected);
                                        break;
                                    }
                                };
                                match &*shared {
                                                webrtc_signal_server::ServerMsg::SessionAvailable { session_info } => {
                                                    // Check if this is our session being announced/updated
                                                    if let Some(sid) = session_info.get("session_id").and_then(|v| v.as_str())
                                                        && sid == session_id_clone {
                                                            // Our session - update full session info
                                                            let curve_type = session_info.get("curve_type")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("Ed25519")
                                                                .to_string();
                                                            
                                                            let _ = tx_msg.send(Message::Info { 
                                                                message: format!("📋 Session update - curve type: {}", curve_type)
                                                            });
                                                            
                                                            // Update the session in app state with correct curve type
                                                            {
                                                                let mut state = app_state_clone.lock().await;
                                                                if let Some(ref mut session) = state.session {
                                                                    session.curve_type = curve_type.clone();
                                                                    
                                                                    // Also update other session fields
                                                                    if let Some(total) = session_info.get("total").and_then(|v| v.as_u64()) {
                                                                        session.total = total as u16;
                                                                    }
                                                                    if let Some(threshold) = session_info.get("threshold").and_then(|v| v.as_u64()) {
                                                                        session.threshold = threshold as u16;
                                                                    }
                                                                }
                                                            }
                                                            
                                                            // Update participants list
                                                            if let Some(participants) = session_info.get("participants").and_then(|v| v.as_array()) {
                                                                let _ = tx_msg.send(Message::Info { 
                                                                    message: format!("📋 Session update - participants: {}", participants.len())
                                                                });
                                                                
                                                                participants_seen.clear();
                                                                for p in participants {
                                                                    if let Some(pid) = p.as_str() {
                                                                        participants_seen.insert(pid.to_string());
                                                                    }
                                                                }
                                                            }
                                                        }
                                                }
                                                webrtc_signal_server::ServerMsg::Devices { devices } => {
                                                    let _ = tx_msg.send(Message::Info { 
                                                        message: format!("📡 Connected devices: {:?}", devices)
                                                    });
                                                    
                                                    // Track previous count to detect new participants
                                                    let prev_count = participants_seen.len();
                                                    
                                                    // Count unique participants in our session (devices is &Vec<String>)
                                                    for device in devices.iter() {
                                                        participants_seen.insert(device.clone());
                                                    }
                                                    
                                                    // Send UpdateParticipants message to update the model
                                                    let participants_list: Vec<String> = participants_seen.iter().cloned().collect();
                                                    let _ = tx_msg.send(Message::UpdateParticipants { 
                                                        participants: participants_list.clone() 
                                                    });
                                                    
                                                    let participants_count = participants_seen.len();
                                                    
                                                    let _ = tx_msg.send(Message::Info { 
                                                        message: format!("👥 Current participants: {}/{}", 
                                                            participants_count, session_total)
                                                    });
                                                    
                                                    // Re-initiate WebRTC if we have new participants
                                                    if participants_count > prev_count && participants_count > 1 {
                                                        let _ = tx_msg.send(Message::Info { 
                                                            message: format!("🔄 New participant detected, re-initiating WebRTC with all {} participants", participants_count)
                                                        });
                                                        
                                                        // Get participants list WITHOUT self for WebRTC initiation
                                                        let self_device = device_id_clone.clone();
                                                        let other_participants: Vec<String> = participants_seen.iter()
                                                            .filter(|p| **p != self_device)
                                                            .cloned()
                                                            .collect();
                                                        
                                                        // Re-initiate WebRTC with OTHER participants only
                                                        let _ = tx_msg.send(Message::InitiateWebRTCWithParticipants {
                                                            participants: other_participants,
                                                        });
                                                    }
                                                    
                                                    if participants_count >= session_total as usize {
                                                        let _ = tx_msg.send(Message::Info { 
                                                            message: "🎉 All participants connected! Starting DKG...".to_string()
                                                        });
                                                        
                                                        // Final WebRTC initiation to ensure all connections
                                                        let _ = tx_msg.send(Message::Info { 
                                                            message: "🔗 Ensuring all peer-to-peer connections are established...".to_string()
                                                        });
                                                        
                                                        // Send with ALL participants to ensure full mesh
                                                        let _ = tx_msg.send(Message::InitiateWebRTCWithParticipants {
                                                            participants: participants_list,
                                                        });
                                                        
                                                        // Schedule mesh verification after a delay
                                                        let tx_verify = tx_msg.clone();
                                                        tokio::spawn(async move {
                                                            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                                                            let _ = tx_verify.send(Message::VerifyMeshConnectivity);
                                                        });
                                                        
                                                        // Update DKG progress
                                                        let _ = tx_msg.send(Message::UpdateDKGProgress {
                                                            round: crate::elm::message::DKGRound::Round1,
                                                            progress: 0.3,
                                                        });
                                                    }
                                                }
                                    // Relay frames (peer WebRTC signals +
                                    // participant_update) are handled by the
                                    // always-on relay handler, not this loop.
                                    _ => {}
                                }
                            }
                        });

                        // Show initial status
                        let _ = tx_clone.send(Message::Info {
                            message: "⏳ Waiting for other participants to join...".to_string()
                        });
                        let _ = tx_clone.send(Message::Info {
                            message: format!("📋 Session ID: {}", session_id)
                        });
            }
            
            Command::InitiateWebRTCConnections { participants } => {
                info!("Initiating WebRTC connections with {} participants", participants.len());
                
                // Store participants in app state for WebRTC handler to process
                let (self_device_id, device_connections_arc, _signal_server_url) = {
                    let mut state = app_state.lock().await;
                    // Update session participants
                    if let Some(ref mut session) = state.session {
                        // Merge new participants with existing ones
                        for p in &participants {
                            if !session.participants.contains(p) {
                                session.participants.push(p.clone());
                            }
                        }
                        info!("Updated session participants: {:?}", session.participants);
                    }
                    (state.device_id.clone(), state.device_connections.clone(), state.signal_server_url.clone())
                };
                
                // Send message to trigger WebRTC through the UI
                let _ = tx.send(Message::Info { 
                    message: format!("🚀 WebRTC mesh creation triggered for {} participants", participants.len())
                });
                
                let _ = tx.send(Message::Info { 
                    message: "⏳ Starting WebRTC connection process...".to_string()
                });
                
                // CRITICAL FIX: Actually initiate WebRTC connections NOW
                info!("🚀 Actually initiating WebRTC for participants: {:?}", participants);

                // Store participant count before moving the vector
                let expected_peer_connections = participants.len() - 1; // Exclude self

                // Call the WebRTC initiation directly with UI message sender
                crate::network::webrtc::initiate_webrtc_with_channel(
                    self_device_id,
                    participants,
                    device_connections_arc,
                    app_state.clone(),
                    Some(tx.clone()),  // Pass the UI message sender
                ).await;

                // Also update DKG progress to show we're connecting
                let _ = tx.send(Message::UpdateDKGProgress {
                    round: crate::elm::message::DKGRound::Round1,
                    progress: 0.35,
                });

                // KISS Fix: Start a simple periodic mesh status checker
                // This polls the connection state every 500ms until mesh is ready
                let tx_mesh = tx.clone();
                let app_state_mesh = app_state.clone();

                tokio::spawn(async move {
                    let mut attempts = 0;
                    const MAX_ATTEMPTS: u32 = 60; // 30 seconds max

                    loop {
                        attempts += 1;
                        if attempts > MAX_ATTEMPTS {
                            let _ = tx_mesh.send(Message::Error {
                                message: "Timeout waiting for WebRTC mesh to be ready".to_string()
                            });
                            break;
                        }

                        // Wait 500ms between checks
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                        // Check if all connections are established and in Connected state
                        let mesh_ready = {
                            let state = app_state_mesh.lock().await;

                            // Check device_connections to see if we have all peer connections
                            let device_connections = state.device_connections.clone();

                            let connections = device_connections.lock().await;
                            let total_connections = connections.len();

                            // Count how many are actually in Connected state
                            let mut connected_count = 0;
                            for (_device_id, pc) in connections.iter() {
                                let connection_state = pc.connection_state();
                                if connection_state == webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Connected {
                                    connected_count += 1;
                                }
                            }

                            info!("🔍 Mesh check: {}/{} peer connections in Connected state (total connections: {})",
                                  connected_count, expected_peer_connections, total_connections);

                            // Mesh is ready when we have connected to all other participants
                            connected_count >= expected_peer_connections
                        };

                        if mesh_ready {
                            info!("✅ WebRTC mesh is ready! Connected to all {} other participants", expected_peer_connections);

                            // Update UI that mesh is complete
                            let _ = tx_mesh.send(Message::Info {
                                message: "✅ WebRTC mesh established successfully!".to_string()
                            });

                            // Trigger DKG Round 1
                            let _ = tx_mesh.send(Message::Info {
                                message: "🚀 Starting DKG Round 1...".to_string()
                            });

                            // Update progress to show DKG is actually starting
                            let _ = tx_mesh.send(Message::UpdateDKGProgress {
                                round: crate::elm::message::DKGRound::Round1,
                                progress: 0.5,
                            });

                            // Actually start DKG protocol here
                            // Get session info to create wallet config
                            let wallet_config = {
                                let state = app_state_mesh.lock().await;
                                state.session.as_ref().map(|session| crate::elm::model::WalletConfig {
                                        name: format!("MPC Wallet {}", &session.session_id[..8]),
                                        total_participants: session.total,
                                        threshold: session.threshold,
                                        mode: crate::elm::model::WalletMode::Online,
                                    })
                            };

                            if let Some(config) = wallet_config {
                                // Trigger actual DKG using InitiateDKG message
                                let _ = tx_mesh.send(crate::elm::message::Message::InitiateDKG {
                                    params: crate::elm::message::DKGParams {
                                        wallet_config: config,
                                        session_id: None,
                                        coordinator: true, // Assume we're coordinator since we're triggering
                                    }
                                });

                                let _ = tx_mesh.send(crate::elm::message::Message::Info {
                                    message: "🚀 Mesh ready! Starting real DKG protocol...".to_string()
                                });
                            } else {
                                // Fallback if no session info available
                                let _ = tx_mesh.send(crate::elm::message::Message::Info {
                                    message: "⚠️ Mesh ready but no session info available for DKG".to_string()
                                });
                            }

                            // Mark that we're ready
                            {
                                let mut state = app_state_mesh.lock().await;
                                state.own_mesh_ready_sent = true;
                            }

                            break;
                        }
                    }
                });
            }
            
            Command::VerifyWebRTCMesh => {
                info!("🔍 Verifying WebRTC mesh connectivity");
                
                let (self_device_id, expected_connections) = {
                    let state = app_state.lock().await;
                    let expected = if let Some(ref session) = state.session {
                        session.participants.len() - 1  // Exclude self
                    } else {
                        0
                    };
                    (state.device_id.clone(), expected)
                };
                
                // Check current connection status
                let connections_status = {
                    let state = app_state.lock().await;
                    let device_connections = state.device_connections.clone();
                    let connections = device_connections.lock().await;
                    
                    let mut status_report = Vec::new();
                    let mut connected_count = 0;
                    let mut failed_count = 0;
                    
                    for (peer_id, pc) in connections.iter() {
                        let conn_state = pc.connection_state();
                        let is_connected = conn_state == webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Connected;
                        
                        if is_connected {
                            connected_count += 1;
                            status_report.push(format!("✅ {} -> {}: Connected", self_device_id, peer_id));
                        } else {
                            failed_count += 1;
                            status_report.push(format!("❌ {} -> {}: {:?}", self_device_id, peer_id, conn_state));
                        }
                    }
                    
                    (connected_count, failed_count, status_report, connections.len())
                };
                
                let (connected_count, failed_count, status_report, _total_connections) = connections_status;
                
                // Send status report
                let _ = tx.send(Message::Info {
                    message: format!("📊 Mesh Status: {}/{} connected ({} failed)", 
                                   connected_count, expected_connections, failed_count)
                });
                
                for status_line in status_report {
                    info!("{}", status_line);
                }
                
                // If not all connections are established, trigger re-initiation
                if connected_count < expected_connections {
                    warn!("⚠️ Incomplete mesh: only {}/{} connections established", connected_count, expected_connections);
                    
                    // Get participants and re-initiate for missing connections
                    let participants = {
                        let state = app_state.lock().await;
                        if let Some(ref session) = state.session {
                            session.participants.clone()
                        } else {
                            vec![]
                        }
                    };
                    
                    if !participants.is_empty() {
                        let _ = tx.send(Message::Info {
                            message: "🔄 Re-initiating WebRTC for missing connections...".to_string()
                        });
                        
                        let _ = tx.send(Message::InitiateWebRTCWithParticipants {
                            participants: participants.into_iter()
                                .filter(|p| p != &self_device_id)
                                .collect()
                        });
                    }
                } else {
                    let _ = tx.send(Message::Success {
                        message: format!("✅ Full mesh established: {} connections", connected_count)
                    });
                }
            }
            
            Command::EnsureFullMesh => {
                info!("🔗 Ensuring full mesh connectivity");
                
                let (self_device_id, participants) = {
                    let state = app_state.lock().await;
                    let participants = if let Some(ref session) = state.session {
                        session.participants.clone()
                    } else {
                        vec![]
                    };
                    (state.device_id.clone(), participants)
                };
                
                if participants.is_empty() {
                    let _ = tx.send(Message::Warning {
                        message: "No active session to verify mesh for".to_string()
                    });
                    return Ok(());
                }
                
                // Check each expected connection
                let mut missing_connections = Vec::new();
                {
                    let state = app_state.lock().await;
                    let device_connections = state.device_connections.clone();
                    let connections = device_connections.lock().await;
                    
                    for participant in &participants {
                        if participant == &self_device_id {
                            continue;
                        }
                        
                        match connections.get(participant) {
                            Some(pc) => {
                                let conn_state = pc.connection_state();
                                if conn_state != webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Connected {
                                    info!("⚠️ Connection to {} is in state: {:?}", participant, conn_state);
                                    missing_connections.push(participant.clone());
                                }
                            }
                            None => {
                                info!("❌ No connection exists to {}", participant);
                                missing_connections.push(participant.clone());
                            }
                        }
                    }
                }
                
                if !missing_connections.is_empty() {
                    let _ = tx.send(Message::Warning {
                        message: format!("Missing connections to: {:?}", missing_connections)
                    });
                    
                    // Re-initiate WebRTC for all participants to fix missing connections
                    let _ = tx.send(Message::Info {
                        message: "🔄 Re-establishing WebRTC connections...".to_string()
                    });
                    
                    let _ = tx.send(Message::InitiateWebRTCWithParticipants {
                        participants: participants.into_iter()
                            .filter(|p| p != &self_device_id)
                            .collect()
                    });
                    
                    // Schedule a verification check after a delay
                    let tx_check = tx.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
                        let _ = tx_check.send(Message::CheckWebRTCConnections);
                    });
                } else {
                    let _ = tx.send(Message::Success {
                        message: "✅ Full mesh connectivity confirmed".to_string()
                    });
                }
            }
            
            Command::DeleteWallet { wallet_id } => {
                info!("Deleting wallet: {}", wallet_id);

                // TODO: Implement wallet deletion in keystore
                // For now, just send an error message
                let _ = tx.send(Message::Error {
                    message: "Wallet deletion not yet implemented".to_string()
                });
            }

            Command::FinalizeWalletFromDkg { password, keystore_path, wallet_name, wallet_label } => {
                // Runs right after `Message::DKGKeyGenerated`. Pulls the
                // FROST output from AppState, serializes the key share,
                // encrypts it with `password`, and writes the wallet file.
                //
                // `password` is taken by value (not borrow) specifically so
                // the Drop at the end of this arm is the last place the
                // cleartext exists in-process. The update handler that
                // dispatched us has already cleared
                // `wallet_state.pending_password`.
                info!("Finalizing wallet '{}' from DKG output", wallet_name);

                // ---- 1. Pull what we need out of AppState in one lock acquisition.
                let (
                    device_id,
                    curve_type_str,
                    threshold,
                    total_participants,
                    participant_index,
                    participants_sorted,
                    key_share_data,
                    group_pubkey_hex,
                    addresses,
                ) = {
                    let state = app_state.lock().await;

                    let Some(session) = state.session.as_ref() else {
                        let err = "FinalizeWalletFromDkg: no active session on AppState".to_string();
                        error!("{}", err);
                        let _ = tx.send(Message::DKGFailed { error: err });
                        return Ok(());
                    };

                    let Some(key_package) = state.key_package.as_ref() else {
                        let err = "FinalizeWalletFromDkg: AppState has no key_package — DKG hasn't finished".to_string();
                        error!("{}", err);
                        let _ = tx.send(Message::DKGFailed { error: err });
                        return Ok(());
                    };

                    let Some(public_key_package) = state.public_key_package.as_ref() else {
                        let err = "FinalizeWalletFromDkg: AppState has no public_key_package".to_string();
                        error!("{}", err);
                        let _ = tx.send(Message::DKGFailed { error: err });
                        return Ok(());
                    };

                    // We need BOTH the KeyPackage (this node's secret share)
                    // AND the PublicKeyPackage (the group-wide map of verifying
                    // shares) to sign later — aggregate() takes the pubkey
                    // package. Can't reconstruct PublicKeyPackage from
                    // KeyPackage alone; KeyPackage only holds THIS node's
                    // verifying share, not the others'. Frame both
                    // into a single encrypted blob as:
                    //     [kp_len: u32 LE][kp_bytes][pkp_len: u32 LE][pkp_bytes]
                    // Stage C.1's UnlockWallet reverses this framing.
                    let key_share_data = match encode_keystore_blob(key_package, public_key_package) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            let err = format!("FinalizeWalletFromDkg: encode_keystore_blob failed: {}", e);
                            error!("{}", err);
                            let _ = tx.send(Message::DKGFailed { error: err });
                            return Ok(());
                        }
                    };

                    let group_pubkey_bytes = match public_key_package.verifying_key().serialize() {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            let err = format!(
                                "FinalizeWalletFromDkg: VerifyingKey::serialize failed: {:?}", e
                            );
                            error!("{}", err);
                            let _ = tx.send(Message::DKGFailed { error: err });
                            return Ok(());
                        }
                    };

                    // Canonical (sorted) order — this is what
                    // `protocal::dkg::canonical_identifier` uses to derive
                    // each node's FROST `Identifier`, so the keystore's
                    // `participant_index` needs to match. Using the raw
                    // `session.participants` order was a subtle bug: each
                    // node ends up seeing itself at position 2 of the wire
                    // ordering, so every single participant stored
                    // `participant_index = 3`.
                    let mut sorted = session.participants.clone();
                    sorted.sort();
                    let participant_index = match sorted
                        .iter()
                        .position(|p| p == &state.device_id)
                    {
                        Some(idx) => (idx as u16) + 1,
                        None => {
                            let err = format!(
                                "FinalizeWalletFromDkg: our device_id '{}' not found in session.participants {:?}",
                                state.device_id, session.participants
                            );
                            error!("{}", err);
                            let _ = tx.send(Message::DKGFailed { error: err });
                            return Ok(());
                        }
                    };

                    // Curve comes from the type-level witness — not from
                    // `session.curve_type`, which is still the legacy
                    // "unified" literal (see Stage 5 of the plan).
                    let curve_type_str = <C as crate::utils::curve_traits::CurveIdentifier>::curve_type().to_string();

                    let addresses: Vec<(String, String)> = state
                        .blockchain_addresses
                        .iter()
                        .map(|b| (b.blockchain.clone(), b.address.clone()))
                        .collect();

                    (
                        state.device_id.clone(),
                        curve_type_str,
                        session.threshold,
                        session.total,
                        participant_index,
                        // Canonical-sorted participant list — persisted
                        // so cold-start signing (after restart) can
                        // reconstruct the session metadata from disk.
                        sorted.clone(),
                        key_share_data,
                        hex::encode(&group_pubkey_bytes),
                        addresses,
                    )
                };

                // ---- 2. Write the wallet file via a fresh Keystore instance.
                // We deliberately don't reuse `state.keystore` (it's
                // `Arc<Keystore>` — read-only); instead we construct one
                // against the same `base_path` for the write, and later
                // re-hydrate the shared Arc so `Command::LoadWallets`
                // sees the new entry.
                use crate::keystore::Keystore;

                let mut ks = match Keystore::new(&keystore_path, &device_id) {
                    Ok(ks) => ks,
                    Err(e) => {
                        let err = format!(
                            "FinalizeWalletFromDkg: Keystore::new({}, {}) failed: {}",
                            keystore_path, device_id, e
                        );
                        error!("{}", err);
                        let _ = tx.send(Message::DKGFailed { error: err });
                        return Ok(());
                    }
                };

                let wallet_id = match ks.create_wallet_multi_chain(
                    &wallet_name,
                    &curve_type_str,
                    Vec::new(),          // blockchains: ignored by keystore (derived from group_pubkey)
                    threshold,
                    total_participants,
                    &group_pubkey_hex,
                    &key_share_data,
                    &password,
                    Vec::new(),          // tags (deprecated)
                    None,                // description (deprecated)
                    participant_index,
                    participants_sorted,
                    wallet_label,        // optional user-chosen display label
                ) {
                    Ok(id) => id,
                    Err(e) => {
                        let err = format!(
                            "FinalizeWalletFromDkg: create_wallet_multi_chain failed: {}",
                            e
                        );
                        error!("{}", err);
                        let _ = tx.send(Message::DKGFailed { error: err });
                        return Ok(());
                    }
                };

                // ---- 3. Drop cleartext password by shadowing; the plaintext
                // is no longer reachable after this block.
                drop(password);

                // ---- 4. Re-hydrate the shared read-only keystore so the UI's
                // next `LoadWallets` picks up the new wallet. We could
                // instead mutate the existing one via Arc::get_mut, but
                // `Arc::get_mut` returns None as soon as anyone else holds
                // a clone (and the Model does). `Keystore::new` rescans
                // all files, so the new instance is authoritative.
                match Keystore::new(&keystore_path, &device_id) {
                    Ok(fresh) => {
                        let mut state = app_state.lock().await;
                        state.keystore = Some(std::sync::Arc::new(fresh));
                    }
                    Err(e) => warn!(
                        "FinalizeWalletFromDkg: re-hydrating Keystore after write failed: {} \
                         (wallet file was still written OK; next LoadWallets will retry)",
                        e
                    ),
                }

                info!(
                    "✅ Wallet '{}' persisted (group={}…, addresses={})",
                    wallet_id,
                    &group_pubkey_hex[..16.min(group_pubkey_hex.len())],
                    addresses.len()
                );

                let _ = tx.send(Message::DKGFinalized {
                    wallet_id,
                    group_pubkey_hex,
                    curve_type: curve_type_str,
                    addresses,
                });
            }

            Command::UnlockWallet { wallet_id, password, keystore_path } => {
                // Counterpart to `FinalizeWalletFromDkg`: decrypt the
                // keystore file, deserialize the `(KeyPackage, PubKeyPackage)`
                // tuple, and write both onto AppState so the signing
                // protocol layer has everything FROST needs.
                //
                // `password` is taken by value; it goes out of scope at the
                // end of this arm, matching the single-plaintext-site
                // invariant from Phase A.
                info!("Unlocking wallet '{}'", wallet_id);

                use crate::keystore::Keystore;

                let device_id = {
                    let state = app_state.lock().await;
                    state.device_id.clone()
                };

                // Hydrate a local Keystore instance so we can call
                // `load_wallet_file` — the shared `Arc<Keystore>` is
                // read-only and the load path doesn't need the live
                // cache anyway.
                let ks = match Keystore::new(&keystore_path, &device_id) {
                    Ok(ks) => ks,
                    Err(e) => {
                        let err = format!(
                            "UnlockWallet: Keystore::new({}, {}) failed: {}",
                            keystore_path, device_id, e
                        );
                        error!("{}", err);
                        let _ = tx.send(Message::WalletUnlockFailed { error: err });
                        return Ok(());
                    }
                };

                // `load_wallet_file` returns the decrypted blob bytes.
                // Wrong-password / wrong-wallet_id errors bubble up as
                // `KeystoreError` — no panic.
                let blob = match ks.load_wallet_file(&wallet_id, &password) {
                    Ok(b) => b,
                    Err(e) => {
                        let err = format!("UnlockWallet: load_wallet_file failed: {}", e);
                        // Don't log the password in any form.
                        error!("{}", err);
                        let _ = tx.send(Message::WalletUnlockFailed { error: err });
                        return Ok(());
                    }
                };

                // Drop the password now — nothing else needs it. This is
                // the single end-of-lifetime point for the plaintext.
                drop(password);

                let (key_package, public_key_package) = match decode_keystore_blob::<C>(&blob) {
                    Ok(t) => t,
                    Err(e) => {
                        let err = format!("UnlockWallet: decode_keystore_blob failed: {}", e);
                        error!("{}", err);
                        let _ = tx.send(Message::WalletUnlockFailed { error: err });
                        return Ok(());
                    }
                };

                // Belt-and-suspenders: verify the `KeyPackage.verifying_key()`
                // matches `PublicKeyPackage.verifying_key()`. A mismatch
                // would mean someone wrote an inconsistent blob — better
                // to fail loudly here than during aggregate later.
                if key_package.verifying_key() != public_key_package.verifying_key() {
                    let err = "UnlockWallet: KeyPackage and PublicKeyPackage disagree on group key".to_string();
                    error!("{}", err);
                    let _ = tx.send(Message::WalletUnlockFailed { error: err });
                    return Ok(());
                }

                {
                    let mut state = app_state.lock().await;
                    state.key_package = Some(key_package);
                    state.group_public_key = Some(*public_key_package.verifying_key());
                    state.public_key_package = Some(public_key_package);
                    state.current_wallet_id = Some(wallet_id.clone());
                }

                info!("✅ Wallet '{}' unlocked — ready to sign", wallet_id);
                let _ = tx.send(Message::WalletUnlocked { wallet_id });
            }

            // ============= Phase C.2 — signing-protocol executors =============
            // These are the thin bridges that let UI-layer Messages reach
            // the async FROST driver in `protocal::signing`. Nothing substantive
            // happens here — the handlers own the real work.

            Command::StartSigning { request } => {
                info!(
                    "🖊️  StartSigning dispatch: wallet={} chain={} message_bytes={}",
                    request.wallet_id,
                    request.chain,
                    request.transaction_data.len()
                );

                // Snapshot what we need in one short lock: device id, the
                // primary WebSocket handle (may be absent on cold start —
                // treat as a no-op announce in that case), and the
                // `PublicKeyPackage` we need to announce the group key so
                // joiners can cross-check.
                let (self_device_id, ws_tx_opt, group_pubkey_hex) = {
                    let state = app_state.lock().await;
                    let group_pubkey_hex = state
                        .public_key_package
                        .as_ref()
                        .and_then(|pkp| pkp.verifying_key().serialize().ok())
                        .map(hex::encode)
                        .unwrap_or_default();
                    (
                        state.device_id.clone(),
                        state.websocket_msg_tx.clone(),
                        group_pubkey_hex,
                    )
                };

                // Record the session on AppState so the protocol layer's
                // broadcast helper has somewhere to read `participants`
                // from. Two paths:
                //
                // 1. Warm — a session already lives on AppState (from
                //    the prior DKG in this run, or a previous sign).
                //    Mutate its session_type to Signing + reuse the
                //    existing participant list + session_id.
                //
                // 2. Cold — no session. Rebuild one from the persisted
                //    wallet metadata we just unlocked: threshold,
                //    total_participants and the participant device_id
                //    list were written at DKG finalize time (see
                //    `WalletMetadata::participants`). Mint a fresh
                //    signing session_id and stamp it on AppState so
                //    joiners can discover us. If the metadata lacks a
                //    participant list (pre-participants-field wallet),
                //    degrade to empty — the announcement will go out
                //    with `participants=[]` and `canonical_identifier`
                //    on the peer side will reject, but we fail loudly
                //    later rather than silently producing a broken
                //    ceremony.
                let session_id = {
                    let mut state = app_state.lock().await;
                    if state.session.is_some() {
                        // Warm path
                        let sid = state.session.as_ref().unwrap().session_id.clone();
                        if let Some(ref mut session) = state.session {
                            use crate::protocal::signal::SessionType;
                            session.session_type = SessionType::Signing {
                                wallet_name: request.wallet_id.clone(),
                                curve_type: request.chain.clone(),
                                blockchain: request.chain.clone(),
                                group_public_key: group_pubkey_hex.clone(),
                            };
                        }
                        sid
                    } else {
                        // Cold path — reconstruct from keystore metadata.
                        let wallet_meta = state
                            .keystore
                            .as_ref()
                            .and_then(|ks| ks.get_wallet(&request.wallet_id).cloned());

                        let (cold_threshold, cold_total, cold_participants) = match wallet_meta {
                            Some(m) => {
                                let ps = m.participants.clone();
                                if ps.is_empty() {
                                    warn!(
                                        "StartSigning cold-start: wallet {} has no \
                                         participants list in metadata (pre-field-add \
                                         wallet?) — announce will have participants=[] \
                                         and peers can't join. Re-run DKG to regenerate.",
                                        request.wallet_id
                                    );
                                }
                                (m.threshold, m.total_participants, ps)
                            }
                            None => {
                                warn!(
                                    "StartSigning cold-start: wallet {} not in keystore \
                                     cache — announce will have empty metadata",
                                    request.wallet_id
                                );
                                (0, 0, Vec::new())
                            }
                        };

                        let sid = format!("sign_{}", uuid::Uuid::new_v4());
                        state.session = Some(crate::protocal::signal::SessionInfo {
                            session_id: sid.clone(),
                            proposer_id: self_device_id.clone(),
                            total: cold_total,
                            threshold: cold_threshold,
                            participants: cold_participants,
                            session_type: crate::protocal::signal::SessionType::Signing {
                                wallet_name: request.wallet_id.clone(),
                                curve_type: request.chain.clone(),
                                blockchain: request.chain.clone(),
                                group_public_key: group_pubkey_hex.clone(),
                            },
                            curve_type: request.chain.clone(),
                            coordination_type: "Network".to_string(),
                            signing_message_hex: None,
                        });
                        info!(
                            "StartSigning cold-start: rebuilt session {} with \
                             {}-of-{} + {} participants from wallet metadata",
                            sid,
                            cold_threshold,
                            cold_total,
                            state.session.as_ref().unwrap().participants.len()
                        );
                        sid
                    }
                };

                // Announce over the signal server so any peer that is
                // joining can discover this signing ceremony. Best-effort —
                // if the websocket isn't up we press on anyway; the
                // in-mesh broadcast still works for same-run ceremonies.
                if let Some(ws_tx) = ws_tx_opt {
                    let announced_curve =
                        <C as crate::utils::curve_traits::CurveIdentifier>::curve_type();
                    let n_participants = {
                        let state = app_state.lock().await;
                        state
                            .session
                            .as_ref()
                            .map(|s| s.total)
                            .unwrap_or(0)
                    };
                    let threshold = {
                        let state = app_state.lock().await;
                        state
                            .session
                            .as_ref()
                            .map(|s| s.threshold)
                            .unwrap_or(0)
                    };
                    let participants = {
                        let state = app_state.lock().await;
                        state
                            .session
                            .as_ref()
                            .map(|s| s.participants.clone())
                            .unwrap_or_default()
                    };
                    // The `blockchain` field must be a real chain name (the
                    // extension's session-parse.ts reads it), not the curve.
                    // `request.chain` is set to the curve upstream (the headless
                    // sign API has no per-chain context), so resolve the wallet's
                    // actual primary chain from keystore metadata, falling back
                    // to the curve's canonical chain. (#32)
                    let announce_blockchain = {
                        let state = app_state.lock().await;
                        state
                            .keystore
                            .as_ref()
                            .and_then(|ks| ks.get_wallet(&request.wallet_id))
                            .and_then(|w| {
                                w.blockchains
                                    .first()
                                    .map(|b| b.blockchain.clone())
                                    .or_else(|| w.blockchain.clone())
                            })
                            .unwrap_or_else(|| {
                                if announced_curve == "ed25519" {
                                    "solana".to_string()
                                } else {
                                    "ethereum".to_string()
                                }
                            })
                    };
                    let session_info = serde_json::json!({
                        "session_id": session_id.clone(),
                        "total": n_participants,
                        "threshold": threshold,
                        "session_type": "signing",
                        "proposer_id": self_device_id.clone(),
                        "participants": participants,
                        "curve_type": announced_curve,
                        "coordination_type": "Network",
                        "wallet_name": request.wallet_id.clone(),
                        "group_public_key": group_pubkey_hex,
                        "blockchain": announce_blockchain,
                        // Joiners need the exact bytes to sign — embed them in
                        // the announce rather than requiring an extra round.
                        "signing_message_hex": hex::encode(&request.transaction_data),
                    });
                    let announce = webrtc_signal_server::ClientMsg::AnnounceSession {
                        session_info,
                    };
                    match serde_json::to_string(&announce) {
                        Ok(json) => {
                            info!("Announcing signing session: {}", session_id);
                            if ws_tx.send(json).is_err() {
                                warn!(
                                    "Signing announcement dropped: primary \
                                     WebSocket channel closed"
                                );
                            }
                        }
                        Err(e) => error!("Serialize signing AnnounceSession: {}", e),
                    }
                }

                // Re-establish the WebRTC mesh for this signing
                // ceremony. On a warm run this is a no-op (peer
                // connections are still alive from the DKG that just
                // finished); on a cold run (post-restart) this is
                // essential — without it `SIGN_COMMIT` has no data
                // channel to flow through. The existing retry loop in
                // `broadcast_signing_frame` (10 × 500ms) absorbs the
                // few seconds the handshake takes.
                //
                // We dispatch via `Message::InitiateWebRTCWithParticipants`
                // rather than calling the WebRTC helper directly because
                // this Command runs on the same tokio task as the
                // in-line `handle_start_signing` below; the message
                // path keeps the initiation on a fresh task and lets
                // our commit broadcast proceed concurrently with
                // ongoing handshakes.
                {
                    let peers: Vec<String> = {
                        let state = app_state.lock().await;
                        state
                            .session
                            .as_ref()
                            .map(|s| {
                                s.participants
                                    .iter()
                                    .filter(|p| *p != &self_device_id)
                                    .cloned()
                                    .collect()
                            })
                            .unwrap_or_default()
                    };
                    if !peers.is_empty() {
                        let _ = tx.send(Message::InitiateWebRTCWithParticipants {
                            participants: peers,
                        });
                    }
                }

                // Kick off the local half of the ceremony. Peers race us —
                // whoever gathers threshold commitments first advances.
                crate::protocal::signing::handle_start_signing::<C>(
                    app_state.clone(),
                    self_device_id,
                    request.transaction_data,
                    tx.clone(),
                )
                .await;
            }

            Command::JoinSigning { session_id, message_bytes } => {
                // Joiner-side counterpart of `StartSigning`. The session
                // was already recorded on AppState by the accept-path
                // update handler (`SubmitPassword` / joiner branch).
                // Here we just kick off the same ceremony entry point —
                // the protocol layer is symmetric between creator and
                // joiner from Round 1 onward.
                info!(
                    "🖊️  JoinSigning: session={} message_bytes={}",
                    session_id,
                    message_bytes.len()
                );
                let self_device_id = {
                    let state = app_state.lock().await;
                    state.device_id.clone()
                };

                // Cold-joiner path: if AppState.session is None
                // (fresh boot, no prior DKG in this run), rebuild it
                // from the just-unlocked wallet's keystore metadata —
                // same shape as StartSigning's cold path. Without this
                // `protocal::signing::broadcast_signing_frame` has no
                // participant list to enumerate.
                //
                // Warm joiner path: session carries over from the DKG
                // that ran earlier in the same process; leave it alone
                // (its session_type may already be Signing from an
                // earlier ceremony; the protocol layer doesn't care).
                {
                    let mut state = app_state.lock().await;
                    if state.session.is_none() {
                        // Find the wallet by its session_id-derived id.
                        // The current_wallet_id was set by UnlockWallet.
                        let (meta, keystore_path) = (
                            state
                                .keystore
                                .as_ref()
                                .and_then(|ks| {
                                    state
                                        .current_wallet_id
                                        .as_ref()
                                        .and_then(|id| ks.get_wallet(id).cloned())
                                }),
                            state.signal_server_url.clone(),
                        );
                        let _ = keystore_path; // unused; kept for clarity
                        match meta {
                            Some(m) => {
                                info!(
                                    "JoinSigning cold-joiner: rebuilt session {} from \
                                     wallet metadata ({}-of-{}, {} participants)",
                                    session_id,
                                    m.threshold,
                                    m.total_participants,
                                    m.participants.len()
                                );
                                state.session = Some(crate::protocal::signal::SessionInfo {
                                    session_id: session_id.clone(),
                                    proposer_id: String::new(),
                                    total: m.total_participants,
                                    threshold: m.threshold,
                                    participants: m.participants.clone(),
                                    session_type: crate::protocal::signal::SessionType::Signing {
                                        wallet_name: m.session_id.clone(),
                                        curve_type: m.curve_type.clone(),
                                        blockchain: m.curve_type.clone(),
                                        group_public_key: m.group_public_key.clone(),
                                    },
                                    curve_type: m.curve_type,
                                    coordination_type: "Network".to_string(),
                                    signing_message_hex: None,
                                });
                            }
                            None => {
                                warn!(
                                    "JoinSigning cold-joiner: no wallet metadata for \
                                     current_wallet_id — peers list will be empty"
                                );
                            }
                        }
                    }
                }

                // Same mesh-re-establishment as StartSigning — joiners
                // typically come from a cold boot too (they pressed
                // Sign on an existing wallet list after restart), so
                // their WebRTC peer connections need to be (re)created.
                // Warm joiners (in the same run as the DKG) are a
                // no-op — the existing connections stay alive.
                {
                    let peers: Vec<String> = {
                        let state = app_state.lock().await;
                        state
                            .session
                            .as_ref()
                            .map(|s| {
                                s.participants
                                    .iter()
                                    .filter(|p| *p != &self_device_id)
                                    .cloned()
                                    .collect()
                            })
                            .unwrap_or_default()
                    };
                    if !peers.is_empty() {
                        let _ = tx.send(Message::InitiateWebRTCWithParticipants {
                            participants: peers,
                        });
                    }
                }

                crate::protocal::signing::handle_start_signing::<C>(
                    app_state.clone(),
                    self_device_id,
                    message_bytes,
                    tx.clone(),
                )
                .await;
            }

            Command::ProcessSigningRound1 { from_device, commitment_bytes } => {
                let self_device_id = {
                    let state = app_state.lock().await;
                    state.device_id.clone()
                };
                crate::protocal::signing::process_signing_round1::<C>(
                    app_state.clone(),
                    self_device_id,
                    from_device,
                    commitment_bytes,
                    tx.clone(),
                )
                .await;
            }

            Command::ProcessSigningRound2 { from_device, share_bytes } => {
                crate::protocal::signing::process_signing_round2::<C>(
                    app_state.clone(),
                    from_device,
                    share_bytes,
                    tx.clone(),
                )
                .await;
            }

            // Placeholder arms for the request-queue approval flow — that's a
            // Phase E concern; Phase C does synchronous "sign now" via
            // `Command::StartSigning`. Log if they fire so future regressions
            // (accidental dispatch) surface.
            Command::ApproveSignature { request_id } => {
                warn!(
                    "Command::ApproveSignature({}) dispatched but not implemented — \
                     Phase E will handle the approval queue",
                    request_id
                );
            }
            Command::RejectSignature { request_id } => {
                warn!(
                    "Command::RejectSignature({}) dispatched but not implemented — \
                     Phase E will handle the approval queue",
                    request_id
                );
            }

            Command::ReconnectWebSocket => {
                // One flat script. Each step has a narrow responsibility and
                // lives in `elm::ws_runtime`:
                //   1. snapshot state, flag as connecting, drop the stale sender
                //   2. dial the signal server
                //   3. mint the outbound mpsc + inbound broadcast, stash in state
                //   4. send Register, and re-announce our own session if any
                //   5. spawn the sender (mpsc → sink, with 30s ping)
                //   6. spawn the reader (stream → parse → broadcast + Elm dispatch)
                //   7. tell the Elm loop we're live
                use crate::elm::ws_runtime;

                info!("Attempting to reconnect WebSocket");
                let params = ws_runtime::read_connect_params(app_state).await;
                let _ = tx.send(Message::Info {
                    message: format!("🔄 Reconnecting to {}...", params.url),
                });

                let (mut sink, rx) = match ws_runtime::dial(&params.url).await {
                    Ok(split) => split,
                    Err(e) => {
                        ws_runtime::handle_dial_failure(e, &tx, app_state).await;
                        return Ok(());
                    }
                };

                let channels = ws_runtime::install_handles(app_state).await;

                ws_runtime::send_register(&mut sink, &params.device_id).await;
                if let Some(session) = &params.existing_session {
                    ws_runtime::send_reannounce(&mut sink, session, &tx).await;
                }

                // Always-on relay handler (peer WebRTC signals +
                // participant_update) for the whole connection — subscribe
                // BEFORE the reader publishes. This is what lets a cold-started
                // signer receive offers without a DKG driver loop running.
                ws_runtime::spawn_relay_handler_task(
                    channels.broadcast_tx.clone(),
                    app_state.clone(),
                    tx.clone(),
                    params.device_id.clone(),
                );
                ws_runtime::spawn_sender_task(sink, channels.ws_msg_rx);
                ws_runtime::spawn_reader_task(rx, tx.clone(), channels.broadcast_tx);

                let _ = tx.send(Message::WebSocketConnected);
                let _ = tx.send(Message::Info {
                    message: "✅ Reconnected to signal server".to_string(),
                });
            }
            
            Command::SendMessage(msg) => {
                // Forward the message
                let _ = tx.send(msg);
            }
            
            Command::ScheduleMessage { delay_ms, message } => {
                // Schedule a message to be sent after a delay
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    let _ = tx.send(*message);
                });
            }

            Command::Batch(commands) => {
                for cmd in commands {
                    // Recurse on the boxed future to avoid an infinitely-sized async type.
                    Box::pin(cmd.execute::<C>(tx.clone(), app_state)).await?;
                }
            }

            Command::RefreshUI => {
                // UI refresh handled by the view layer
                info!("UI refresh requested");
            }
            
            Command::Quit => {
                info!("Application quit requested");
                // Send quit message to trigger app shutdown
                let _ = tx.send(Message::Quit);
            }
            
            Command::None => {
                // No operation
            }
            
            _ => {
                info!("Command not yet implemented: {:?}", self);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_command_creation() {
        let cmd = Command::LoadWallets;
        assert!(matches!(cmd, Command::LoadWallets));

        let cmd = Command::StartDKG {
            config: WalletConfig {
                name: "Test".to_string(),
                total_participants: 3,
                threshold: 2,
                mode: crate::elm::model::WalletMode::Online,
            }
        };
        assert!(matches!(cmd, Command::StartDKG { .. }));
    }

    /// Round-trip a (KeyPackage, PublicKeyPackage) produced by a real
    /// FROST DKG through `encode_keystore_blob` + `decode_keystore_blob`
    /// and assert the deserialized key package's identifier + group
    /// verifying key match. Uses `trusted_dealer_keygen` to get real
    /// packages in-process without running a network DKG.
    #[test]
    fn encode_decode_keystore_blob_round_trips() {
        use frost_secp256k1::{
            keys::{
                generate_with_dealer, IdentifierList, KeyPackage as KP,
                PublicKeyPackage as PKP,
            },
            rand_core::OsRng,
            Identifier, Secp256K1Sha256,
        };
        let rng = OsRng;
        let (secret_shares, pubkey_package): (
            std::collections::BTreeMap<Identifier, _>,
            PKP,
        ) = generate_with_dealer(3, 2, IdentifierList::Default, rng)
            .expect("trusted-dealer keygen");

        let (id, secret_share) = secret_shares.iter().next().unwrap();
        let key_package: KP =
            secret_share.clone().try_into().expect("share → KeyPackage");

        let blob = encode_keystore_blob::<Secp256K1Sha256>(&key_package, &pubkey_package)
            .expect("encode");

        let (kp_back, pkp_back) =
            decode_keystore_blob::<Secp256K1Sha256>(&blob).expect("decode");

        assert_eq!(
            kp_back.identifier(),
            key_package.identifier(),
            "identifier must survive round-trip"
        );
        assert_eq!(
            pkp_back.verifying_key(),
            pubkey_package.verifying_key(),
            "group verifying key must survive round-trip"
        );
        assert_eq!(id, kp_back.identifier());
    }

    /// A garbage-byte blob must produce a descriptive Err, not a panic —
    /// this is the path that fires on a wrong-password decrypt (the
    /// plaintext that comes out is noise).
    #[test]
    fn decode_keystore_blob_handles_truncation_gracefully() {
        let truncated: Vec<u8> = vec![0xff, 0xff, 0xff, 0xff, 0x00]; // claims 4GB but has 1 byte
        let result = decode_keystore_blob::<frost_secp256k1::Secp256K1Sha256>(&truncated);
        assert!(result.is_err(), "truncated blob must surface an Err");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("truncated") || msg.contains("deserialize"),
            "error must describe the failure; got {msg:?}"
        );
    }

    #[test]
    fn decode_keystore_blob_rejects_empty_input() {
        let result = decode_keystore_blob::<frost_secp256k1::Secp256K1Sha256>(&[]);
        assert!(result.is_err(), "empty input must fail");
    }
}