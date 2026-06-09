//! Update - The state transition function
//!
//! The update function is the heart of the Elm Architecture. It takes the current
//! model and a message, and returns an updated model along with optional commands
//! to execute side effects.

use crate::elm::model::{Model, Screen, Modal, Notification, NotificationKind, ConnectionStatus, Operation, ProgressInfo, WalletConfig, WalletMode, CreateWalletState, PendingSignPreview};
use crate::elm::message::{Message, DKGRound};
use crate::elm::command::Command;
use crate::protocal::signal::{SessionInfo, SessionType};
use chrono::Utc;
use crossterm::event::{KeyCode, KeyModifiers};
use tracing::{info, debug, warn, error};
use uuid::Uuid;

/// Mark the local DKG state as "Round 1 in progress" so the DKGProgress
/// component renders the cyan "Round 1" label and a ~25% progress bar the
/// next time it's remounted. Also flips `dkg_in_progress` so idempotency
/// guards in callers work.
fn enter_round1(model: &mut Model) {
    model.wallet_state.dkg_in_progress = true;
    // Don't clobber Round2/Finalization if a concurrent path already advanced
    // us — this function runs on the Round 1 trigger edge only.
    if matches!(
        model.wallet_state.dkg_round,
        DKGRound::Initialization | DKGRound::WaitingForParticipants
    ) {
        model.wallet_state.dkg_round = DKGRound::Round1;
    }
}

/// Build the mesh-ready FROST-trigger batch. For a unified ceremony, prepend a
/// `PrepareUnifiedFinalize` that captures the password/keystore/label so the
/// round-2 completion can persist both curves. The password is taken (cleared)
/// from the model — it lives only on the in-flight command afterwards.
fn frost_trigger_batch(model: &mut Model) -> Command {
    let mut cmds = Vec::new();
    if model.wallet_state.unified {
        if let Some(password) = model.wallet_state.pending_password.take() {
            let label = {
                let s = model.wallet_state.wallet_name_draft.trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            };
            cmds.push(Command::PrepareUnifiedFinalize {
                password,
                keystore_path: model.wallet_state.keystore_path.clone(),
                label,
            });
        }
    }
    cmds.push(Command::StartFrostProtocol);
    cmds.push(Command::SendMessage(Message::ForceRemount));
    Command::Batch(cmds)
}

/// The main update function that handles all state transitions
pub fn update(model: &mut Model, msg: Message) -> Option<Command> {
    debug!("Processing message: {:?}", msg);
    
    match msg {
        // ============= Navigation Messages =============
        Message::Navigate(screen) => {
            info!("Navigating to screen: {:?}", screen);
            model.push_screen(screen.clone());
            
            // Update focus based on screen
            match screen {
                Screen::CreateWallet(_) => {
                    model.ui_state.focus = crate::elm::model::ComponentId::CreateWallet;
                    // Initialize selected index for CreateWallet if not exists
                    model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::CreateWallet).or_insert(0);
                    debug!("🎯 CreateWallet focus set, selected index: {}", 
                           model.ui_state.selected_indices[&crate::elm::model::ComponentId::CreateWallet]);
                }
                Screen::ModeSelection => {
                    model.ui_state.focus = crate::elm::model::ComponentId::ModeSelection;
                    model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::ModeSelection).or_insert(0);
                    debug!("🎯 ModeSelection focus set");
                }
                Screen::ManageWallets => {
                    model.ui_state.focus = crate::elm::model::ComponentId::WalletList;
                }
                Screen::JoinSession => {
                    model.ui_state.focus = crate::elm::model::ComponentId::JoinSession;
                    model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::JoinSession).or_insert(0);
                    debug!("🎯 JoinSession focus set");
                }
                Screen::MainMenu | Screen::Welcome => {
                    model.ui_state.focus = crate::elm::model::ComponentId::MainMenu;
                }
                Screen::DKGProgress { .. } => {
                    model.ui_state.focus = crate::elm::model::ComponentId::DKGProgress;
                    model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::DKGProgress).or_insert(0);
                    debug!("🎯 DKGProgress focus set");
                }
                Screen::ThresholdConfig => {
                    model.ui_state.focus = crate::elm::model::ComponentId::ThresholdConfig;
                    model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::ThresholdConfig).or_insert(0);
                    debug!("🎯 ThresholdConfig focus set");
                }
                _ => {}
            }

            // Load data for the new screen if needed
            match screen {
                Screen::ManageWallets => Some(Command::LoadWallets),
                Screen::JoinSession => Some(Command::LoadSessions),
                _ => None,
            }
        }
        
        Message::NavigateBack => {
            debug!("🔙 NavigateBack message received!");
            debug!("Current screen: {:?}", model.current_screen);
            debug!("Navigation stack length: {}", model.navigation_stack.len());

            // If the user is leaving PasswordPrompt, wipe the draft so the
            // cleartext password never outlives the screen that collected it.
            if matches!(model.current_screen, Screen::PasswordPrompt) {
                model.wallet_state.clear_password_draft();
            }
            // Similarly, clear any in-progress sign-message draft on exit
            // from SignTransaction. Message bytes aren't sensitive but a
            // stale half-typed message has no business surviving Esc.
            if matches!(model.current_screen, Screen::SignTransaction { .. }) {
                model.wallet_state.clear_sign_draft();
            }

            // Check if we're at the root screen (main menu with empty stack)
            if model.navigation_stack.is_empty() && matches!(model.current_screen, Screen::MainMenu | Screen::Welcome) {
                // At root level - Esc should quit the app
                debug!("🚪 At root screen, Esc should quit");
                return Some(Command::SendMessage(Message::Quit));
            }
            
            // Otherwise, navigate back normally
            if !model.pop_screen() {
                // Fallback - shouldn't happen after the check above
                debug!("🚨 Already at root screen, staying put");
            } else {
                debug!("✅ Successfully popped screen, new current screen: {:?}", model.current_screen);
                // Update focus based on new current screen
                match model.current_screen {
                    Screen::MainMenu | Screen::Welcome => {
                        model.ui_state.focus = crate::elm::model::ComponentId::MainMenu;
                        debug!("🎯 Focus set to MainMenu");
                    }
                    Screen::ManageWallets => {
                        model.ui_state.focus = crate::elm::model::ComponentId::WalletList;
                        debug!("🎯 Focus set to WalletList");
                    }
                    Screen::CreateWallet(_) => {
                        model.ui_state.focus = crate::elm::model::ComponentId::CreateWallet;
                        debug!("🎯 Focus set to CreateWallet");
                    }
                    Screen::ModeSelection => {
                        model.ui_state.focus = crate::elm::model::ComponentId::ModeSelection;
                        debug!("🎯 Focus set to ModeSelection");
                    }
                    Screen::ThresholdConfig => {
                        model.ui_state.focus = crate::elm::model::ComponentId::ThresholdConfig;
                        debug!("🎯 Focus set to ThresholdConfig");
                    }
                    Screen::DKGProgress { .. } => {
                        model.ui_state.focus = crate::elm::model::ComponentId::DKGProgress;
                        debug!("🎯 Focus set to DKGProgress");
                    }
                    Screen::JoinSession => {
                        model.ui_state.focus = crate::elm::model::ComponentId::JoinSession;
                        debug!("🎯 Focus set to JoinSession");
                    }
                    _ => {
                        debug!("🎯 No specific focus set for screen: {:?}", model.current_screen);
                    }
                }
            }
            None
        }
        
        Message::NavigateHome => {
            // Same invariant as NavigateBack: leaving PasswordPrompt must
            // wipe the draft, whichever route the user takes out.
            if matches!(model.current_screen, Screen::PasswordPrompt) {
                model.wallet_state.clear_password_draft();
            }
            // Clear the post-DKG snapshot — if the user is back on home
            // they're no longer looking at "that just-finished wallet".
            // Stops a second DKG from rendering stale data on its first
            // frame. Same invariant for the post-signing snapshot.
            model.wallet_state.last_finalized_wallet = None;
            model.wallet_state.last_completed_signature = None;
            // Don't carry unlocked-wallet state through home either —
            // the user navigated away, conservative default is "next
            // sign needs to re-unlock". Same logic as clearing the
            // DKG-time drafts.
            model.wallet_state.wallet_unlocked_id = None;
            // Stale pre-hash message shouldn't outlive the flow.
            model.wallet_state.pending_raw_message = None;
            // Any pending signing handoff state becomes stale the
            // moment the user is back home — clear so a future
            // DKG-creator PasswordPrompt submit doesn't misroute.
            // (See the WalletUnlockFailed comment for the specific
            // bug this prevents.)
            model.wallet_state.pending_sign_message = None;
            model.wallet_state.pending_sign_wallet_id = None;
            model.wallet_state.pending_sign_session_id = None;
            // Stage 4: signing-acceptance roster is ceremony-specific.
            // Leaving home means any in-flight ceremony is either done
            // or abandoned — either way the next mount should start empty.
            model.wallet_state.signing_commitments_received.clear();
            model.wallet_state.signing_shares_received.clear();
            model.go_home();
            None
        }

        Message::PushScreen(screen) => {
            model.push_screen(screen);
            None
        }

        Message::PopScreen => {
            if matches!(model.current_screen, Screen::PasswordPrompt) {
                model.wallet_state.clear_password_draft();
            }
            model.pop_screen();
            None
        }
        
        Message::ForceRemount => {
            // This message forces a remount of the current screen's components
            // Used when returning from sub-screens to ensure UI updates
            info!("ForceRemount triggered for screen: {:?}", model.current_screen);
            None
        }
        
        // ============= Wallet Management Messages =============

        // ----- PasswordPrompt draft input (keystroke-level) -----
        // These four messages own the live-edit state on PasswordPrompt.
        // They exist because `handle_key_event` in app.rs bypasses
        // tuirealm's per-component `on()` and routes keys through Messages
        // directly; the component renders from `Model.wallet_state` rather
        // than from internal state.
        Message::PasswordTypeChar(c) => {
            // Any typing clears the stale error so the user isn't left
            // staring at a complaint about input they've already changed.
            model.wallet_state.password_error = None;
            if model.wallet_state.wallet_name_focus {
                model.wallet_state.wallet_name_draft.push(c);
            } else if model.wallet_state.password_focus_confirm {
                model.wallet_state.confirm_draft.push(c);
            } else {
                model.wallet_state.password_draft.push(c);
            }
            None
        }

        Message::PasswordBackspace => {
            model.wallet_state.password_error = None;
            if model.wallet_state.wallet_name_focus {
                model.wallet_state.wallet_name_draft.pop();
            } else if model.wallet_state.password_focus_confirm {
                model.wallet_state.confirm_draft.pop();
            } else {
                model.wallet_state.password_draft.pop();
            }
            None
        }

        Message::PasswordToggleField => {
            // Unlock mode renders a single field — Tab / BackTab is a no-op
            // there. (Without this guard, toggling would route subsequent
            // keystrokes to a hidden buffer the unlock path never reads.)
            if matches!(
                model.wallet_state.password_prompt_purpose,
                crate::elm::model::PasswordPromptPurpose::Unlock
            ) {
                return None;
            }
            // SetNew renders three fields: cycle name → password → confirm.
            if model.wallet_state.wallet_name_focus {
                model.wallet_state.wallet_name_focus = false;
                model.wallet_state.password_focus_confirm = false;
            } else if !model.wallet_state.password_focus_confirm {
                model.wallet_state.password_focus_confirm = true;
            } else {
                model.wallet_state.password_focus_confirm = false;
                model.wallet_state.wallet_name_focus = true;
            }
            None
        }

        Message::PasswordSubmitDraft => {
            // Validation lives here (not in the component) so the rules are
            // exercised by the same test harness as every other state
            // transition and can't silently drift if the component's
            // `on()` is ever re-enabled.
            let pw = model.wallet_state.password_draft.clone();
            let purpose = model.wallet_state.password_prompt_purpose.clone();

            match purpose {
                crate::elm::model::PasswordPromptPurpose::SetNew => {
                    // Setting a brand-new password for DKG: enforce a
                    // minimum length and require the confirm field to
                    // match.
                    const MIN_PW_LEN: usize = 8;
                    let cf = model.wallet_state.confirm_draft.clone();
                    if pw.len() < MIN_PW_LEN {
                        model.wallet_state.password_error = Some(format!(
                            "Password must be at least {MIN_PW_LEN} characters"
                        ));
                        return None;
                    }
                    if pw != cf {
                        model.wallet_state.password_error =
                            Some("Confirm does not match password".to_string());
                        return None;
                    }
                }
                crate::elm::model::PasswordPromptPurpose::Unlock => {
                    // Unlocking an existing wallet — backend
                    // `UnlockWallet` returns "Invalid password" if it's
                    // wrong, so we only reject the obviously-empty
                    // case here. No length check (existing wallets'
                    // passwords were already validated at DKG time)
                    // and no confirm-match (the screen renders one
                    // field only).
                    if pw.is_empty() {
                        model.wallet_state.password_error =
                            Some("Enter the wallet password".to_string());
                        return None;
                    }
                }
            }

            // Valid: wipe the drafts immediately so the cleartext doesn't
            // outlive the handoff. `SubmitPassword` stashes it on
            // `pending_password` and drives the DKG-or-unlock flow
            // forward.
            model.wallet_state.password_draft.clear();
            model.wallet_state.confirm_draft.clear();
            model.wallet_state.password_error = None;
            model.wallet_state.password_focus_confirm = false;
            // Keep `wallet_name_draft` — it's consumed later at finalize
            // (DKGKeyGenerated). Just drop focus from the name field.
            model.wallet_state.wallet_name_focus = false;
            Some(Command::SendMessage(Message::SubmitPassword { value: pw }))
        }

        // Route the staged password to the right downstream flow. This
        // handler is the single join point where both the creator and the
        // joiner paths hand off to the DKG layer — upstream, they diverge
        // (ThresholdConfig-Enter vs JoinSession-Enter); downstream, they
        // both land on DKGProgress via their respective Commands.
        //
        // Disambiguation: at this point the joiner already has
        // `active_session` populated (set when they clicked a session on
        // the JoinSession screen); the creator does not (the session is
        // announced by `Command::StartDKG`, which runs from inside
        // `Message::CreateWallet`'s handler).
        //
        // The password stashed here is later consumed by the keystore-
        // write in the `DKGKeyGenerated` handler (line ~1382), which
        // clears `pending_password` after encryption.
        // Headless creator entry: seed the same state ThresholdConfig +
        // PasswordPrompt would have set, then hand off to SubmitPassword's
        // creator branch (which builds the config + announces the session).
        Message::SetUnifiedMode { unified } => {
            info!("Unified DKG mode set to {}", unified);
            model.wallet_state.unified = unified;
            None
        }

        Message::HeadlessCreateWallet { config, password, label } => {
            model.wallet_state.creating_wallet =
                Some(crate::elm::model::CreateWalletState {
                    mode: Some(config.mode.clone()),
                    template: None,
                    custom_config: Some(config),
                });
            model.wallet_state.wallet_name_draft = label;
            Some(Command::SendMessage(Message::SubmitPassword { value: password }))
        }

        // Headless joiner entry: pick the discovered invite by id, mark it
        // active (the joiner signal SubmitPassword keys off), then hand off.
        Message::HeadlessJoinSession { session_id, password, label } => {
            // Idempotent: the CLI joiner retries refresh+join on a short cadence
            // to beat the announce/connect race (the creator may announce before
            // we connect, and `announce_session` is a one-shot broadcast). Once
            // we've already joined this session, ignore repeat sends so we don't
            // re-fire SubmitPassword and clobber an in-progress DKG.
            if model
                .active_session
                .as_ref()
                .map(|s| s.session_id.as_str())
                == Some(session_id.as_str())
            {
                return None;
            }
            match model
                .session_invites
                .iter()
                .find(|s| s.session_id == session_id)
                .cloned()
            {
                Some(invite) => {
                    model.active_session = Some(invite);
                    model.wallet_state.wallet_name_draft = label;
                    Some(Command::SendMessage(Message::SubmitPassword {
                        value: password,
                    }))
                }
                None => {
                    // Not discovered yet — expected during the cold-start retry
                    // window before the server's `RequestActiveSessions` replay
                    // (or the live announce) arrives. Logged at debug so the
                    // retry cadence doesn't spam an investor's info-level run.
                    debug!(
                        "HeadlessJoinSession: session {} not discovered yet, awaiting replay",
                        session_id
                    );
                    None
                }
            }
        }

        // Headless initiator-side signing. Mirrors SignSubmit's cold path:
        // compute the payload (EIP-191 hash for secp256k1, raw otherwise),
        // stash pending_sign_* (no session_id → "we're announcing one"),
        // then hand off to SubmitPassword which unlocks + InitiateSigning.
        Message::HeadlessSign { wallet_id, message, encoding, password } => {
            let raw = if encoding.eq_ignore_ascii_case("hex") {
                match hex::decode(message.trim().trim_start_matches("0x")) {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("HeadlessSign: bad hex message: {}", e);
                        return None;
                    }
                }
            } else {
                message.into_bytes()
            };
            let curve = model.wallet_state.curve_type;
            let bytes_to_sign = if curve == "secp256k1" {
                crate::utils::eth_helper::eip191_hash(&raw).to_vec()
            } else {
                raw.clone()
            };
            model.wallet_state.pending_sign_message = Some(bytes_to_sign);
            model.wallet_state.pending_sign_wallet_id = Some(wallet_id);
            model.wallet_state.pending_sign_session_id = None;
            model.wallet_state.pending_raw_message =
                if curve == "secp256k1" { Some(raw) } else { None };
            // Clear any leftover DKG-creation/join state so SubmitPassword's
            // cold-start *sign* gate (creating_wallet.is_none() &&
            // active_session.is_none()) is taken — otherwise a wallet we just
            // created would re-route into the creator/joiner DKG branch.
            model.wallet_state.creating_wallet = None;
            model.active_session = None;
            model.wallet_state.password_prompt_purpose =
                crate::elm::model::PasswordPromptPurpose::Unlock;
            Some(Command::SendMessage(Message::SubmitPassword { value: password }))
        }

        // Headless cold-start session replay: ask the server to resend every
        // active session so a node that connected after an announcement can
        // still discover it. Maps straight to the same discovery command the
        // Join-Session screen uses.
        Message::HeadlessRefreshSessions => Some(Command::LoadSessions),

        Message::SubmitPassword { value } => {
            info!(
                "Password submitted ({} chars) — routing to DKG/sign",
                value.len()
            );
            model.wallet_state.pending_password = Some(value);

            // Creator-side cold-start signing: SignSubmit stashed
            // `pending_sign_message` + `pending_sign_wallet_id` but NO
            // `pending_sign_session_id` (there's no incoming session yet;
            // we're about to announce one). Unlock the wallet → on
            // success, `WalletUnlocked` re-dispatches as an InitiateSigning
            // which announces + starts the ceremony.
            //
            // **Gated on `creating_wallet.is_none() && active_session.is_none()`**
            // because those fields are the definitive "this is a DKG
            // flow" signal — and they dominate over stale
            // `pending_sign_*` state from a prior failed sign. Without
            // this gate, a failed cold-start sign (e.g. mesh timeout
            // that never cleared `pending_sign_message`) would hijack
            // the next legitimate creator/joiner DKG password submit
            // and try to UnlockWallet a wallet the user isn't signing
            // with — the exact bug that produced
            // "UnlockWallet: load_wallet_file failed: Invalid password"
            // on a DKG-creator PasswordPrompt.
            let is_creator_dkg = model.wallet_state.creating_wallet.is_some();
            let is_joiner = model.active_session.is_some();
            if !is_creator_dkg
                && !is_joiner
                && model.wallet_state.pending_sign_message.is_some()
                && model.wallet_state.pending_sign_session_id.is_none()
            {
                let wallet_id = match model.wallet_state.pending_sign_wallet_id.clone() {
                    Some(id) => id,
                    None => {
                        warn!(
                            "SubmitPassword with pending_sign_message but no \
                             pending_sign_wallet_id — SignSubmit navigation broke"
                        );
                        model.wallet_state.pending_password = None;
                        return None;
                    }
                };
                let password = model
                    .wallet_state
                    .pending_password
                    .take()
                    .unwrap_or_default();
                info!(
                    "SubmitPassword on cold-start sign for wallet '{}' — \
                     dispatching UnlockWallet",
                    wallet_id
                );
                return Some(Command::UnlockWallet {
                    wallet_id,
                    password,
                    keystore_path: model.wallet_state.keystore_path.clone(),
                });
            }

            if let Some(session) = model.active_session.clone() {
                // Joiner path — fork on session_type. DKG sessions go
                // through the existing JoinDKG flow → DKGProgress;
                // signing sessions unlock the wallet, decode the
                // embedded message, and kick off JoinSigning.
                let session_id = session.session_id.clone();
                match &session.session_type {
                    crate::protocal::signal::SessionType::DKG => {
                        model.push_screen(Screen::DKGProgress {
                            session_id: session_id.clone(),
                        });
                        model.ui_state.focus =
                            crate::elm::model::ComponentId::DKGProgress;
                        model
                            .ui_state
                            .selected_indices
                            .entry(crate::elm::model::ComponentId::DKGProgress)
                            .or_insert(0);
                        // Pass the discovered session's real shape so the
                        // joiner agrees on `total` immediately (no default-3 race).
                        Some(Command::JoinDKG {
                            session_id,
                            total: session.total,
                            threshold: session.threshold,
                            proposer_id: session.proposer_id.clone(),
                            curve_type: session.curve_type.clone(),
                        })
                    }
                    crate::protocal::signal::SessionType::Signing {
                        wallet_name, ..
                    } => {
                        // Pull the message bytes from the announcement
                        // that came with the session. Hex decode is
                        // safe — the creator encoded it cleanly.
                        let Some(ref hex_msg) = session.signing_message_hex else {
                            warn!(
                                "SubmitPassword on signing session {} but \
                                 signing_message_hex is missing — can't sign",
                                session_id
                            );
                            model.ui_state.modal = Some(Modal::Error {
                                title: "Corrupt signing session".to_string(),
                                message: "The signing announcement is missing the \
                                          message payload. Try rejoining."
                                    .to_string(),
                            });
                            model.wallet_state.pending_password = None;
                            return None;
                        };
                        let message_bytes = match hex::decode(hex_msg) {
                            Ok(b) => b,
                            Err(e) => {
                                warn!(
                                    "Signing announcement had invalid hex message: {}",
                                    e
                                );
                                model.ui_state.modal = Some(Modal::Error {
                                    title: "Corrupt signing session".to_string(),
                                    message: format!(
                                        "Bad message hex in announcement: {}",
                                        e
                                    ),
                                });
                                model.wallet_state.pending_password = None;
                                return None;
                            }
                        };

                        // Need the cleartext password once more to hand to
                        // UnlockWallet — then re-dispatch the signing kickoff
                        // after WalletUnlocked fires. We stash message_bytes
                        // + wallet_id on Model so the WalletUnlocked handler
                        // can pick it back up.
                        let password = model
                            .wallet_state
                            .pending_password
                            .take()
                            .unwrap_or_default();
                        model.wallet_state.pending_sign_message = Some(message_bytes);
                        model.wallet_state.pending_sign_wallet_id = Some(wallet_name.clone());
                        // Also stash session_id so JoinSigning knows which
                        // session we're joining.
                        model.wallet_state.pending_sign_session_id = Some(session_id.clone());

                        info!(
                            "SubmitPassword on signing session {}: dispatching \
                             UnlockWallet for wallet '{}'",
                            session_id, wallet_name
                        );
                        Some(Command::UnlockWallet {
                            wallet_id: wallet_name.clone(),
                            password,
                            keystore_path: model
                                .wallet_state
                                .keystore_path
                                .clone(),
                        })
                    }
                    crate::protocal::signal::SessionType::Reshare {
                        wallet_name,
                        curve_type,
                        group_public_key,
                    } => {
                        // Reshare joiner (#56): the user accepted a reshare invite
                        // and supplied the wallet password. Reuse the DKG progress
                        // screen (the mesh-formation UX is identical) and dispatch
                        // `JoinReshare`, which loads the OLD share, seeds the
                        // reshare context, and joins so the mesh forms. The refresh
                        // then fires via the shared `StartFrostProtocol` path.
                        let password = model
                            .wallet_state
                            .pending_password
                            .take()
                            .unwrap_or_default();
                        info!(
                            "SubmitPassword on reshare session {} (wallet '{}') — \
                             dispatching JoinReshare",
                            session_id, wallet_name
                        );
                        model.push_screen(Screen::DKGProgress {
                            session_id: session_id.clone(),
                        });
                        model.ui_state.focus =
                            crate::elm::model::ComponentId::DKGProgress;
                        model
                            .ui_state
                            .selected_indices
                            .entry(crate::elm::model::ComponentId::DKGProgress)
                            .or_insert(0);
                        Some(Command::JoinReshare {
                            session_id,
                            wallet_name: wallet_name.clone(),
                            total: session.total,
                            threshold: session.threshold,
                            proposer_id: session.proposer_id.clone(),
                            curve_type: curve_type.clone(),
                            group_public_key: group_public_key.clone(),
                            password,
                            keystore_path: model.wallet_state.keystore_path.clone(),
                        })
                    }
                }
            } else if let Some(cw) = model.wallet_state.creating_wallet.clone() {
                // Creator path — build the `WalletConfig` out of whatever
                // the user configured on `ThresholdConfig` (preferred) or
                // picked from a template, and hand off to
                // `Message::CreateWallet` which already handles the
                // session-announce + DKGProgress navigation dance.
                let config = cw.custom_config.unwrap_or_else(|| {
                    cw.template
                        .as_ref()
                        .map(|t| WalletConfig {
                            name: t.name.clone(),
                            threshold: t.threshold,
                            total_participants: t.total_participants,
                            mode: cw.mode.clone().unwrap_or(WalletMode::Online),
                        })
                        .unwrap_or_else(|| WalletConfig {
                            name: "MPC Wallet".to_string(),
                            threshold: 2,
                            total_participants: 3,
                            mode: WalletMode::Online,
                        })
                });
                Some(Command::SendMessage(Message::CreateWallet { config }))
            } else {
                // Neither creator nor joiner state is set — this shouldn't
                // happen because the only way to reach PasswordPrompt is
                // through one of those two flows. Log loudly rather than
                // silently drop so the regression is debuggable.
                warn!(
                    "SubmitPassword fired with no active_session and no \
                     creating_wallet — navigation edge regressed upstream"
                );
                model.go_home();
                None
            }
        }

        Message::CreateWallet { config } => {
            info!("Creating wallet with config: {:?}", config);
            
            // Don't generate session ID here - wait for StartDKG command to generate the real DKG session ID
            // Use a placeholder for now that will be updated by UpdateDKGSessionId message
            let temp_session_id = "pending".to_string();
            info!("Starting DKG process - session ID will be generated by StartDKG command");
            
            // Initialize session state with current device as first participant
            let participants = vec![model.device_id.clone()];
            info!("Added current device as participant: {}", model.device_id);
            
            // Create active session with placeholder session ID.
            // `curve_type` is the Model's boot-time snapshot of `C::curve_type()`,
            // UNLESS this is a unified ceremony — then it's the "unified" marker
            // that every node branches on (creator + joiners).
            let creator_curve = if model.wallet_state.unified {
                "unified".to_string()
            } else {
                model.wallet_state.curve_type.to_string()
            };
            model.active_session = Some(SessionInfo {
                session_id: temp_session_id.clone(),
                proposer_id: model.device_id.clone(),
                total: config.total_participants,
                threshold: config.threshold,
                participants,
                session_type: SessionType::DKG,
                curve_type: creator_curve,
                coordination_type: "online".to_string(),
                signing_message_hex: None,
            });
            
            // Navigate to DKG Progress screen with placeholder
            model.push_screen(Screen::DKGProgress { session_id: temp_session_id });

            // Set focus for DKGProgress screen
            model.ui_state.focus = crate::elm::model::ComponentId::DKGProgress;
            model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::DKGProgress).or_insert(0);
            
            // Add to pending operations
            model.pending_operations.push(Operation::CreateWallet(config.clone()));
            
            // Start DKG process - this will generate the real session ID
            Some(Command::StartDKG { config, unified: model.wallet_state.unified })
        }
        
        Message::SelectWallet { wallet_id } => {
            info!("Selected wallet: {}", wallet_id);
            model.selected_wallet = Some(wallet_id.clone());
            
            // Navigate to wallet detail
            model.push_screen(Screen::WalletDetail { wallet_id: wallet_id.clone() });
            
            // Load wallet details
            Some(Command::LoadWalletDetails { wallet_id })
        }
        
        Message::ListWallets => {
            Some(Command::LoadWallets)
        }
        
        // Phase C.2/C.3 bridge: peer signing frames routed through the
        // protocol-layer Commands. Pure pass-through on purpose — the
        // update layer doesn't mutate Model state for these; all the
        // state lives on AppState and `protocal::signing` handles it
        // directly. Keeping this thin means the UI remount logic
        // doesn't have to distinguish between "got a SIGN_COMMIT" and
        // "got a SIGN_SHARE" — both just advance the async driver.
        Message::ProcessSigningRound1 { from_device, commitment_bytes } => {
            debug!(
                "Routing SIGN_COMMIT from {} ({} bytes) to the protocol layer",
                from_device,
                commitment_bytes.len()
            );
            // Stage 4: record this device as "committed" so the
            // SigningProgress roster can flip their row to Round1Complete.
            // This is also the "they accepted" signal — a peer who
            // received the session invite but chose Decline / ignored it
            // never sends a commitment.
            model
                .wallet_state
                .signing_commitments_received
                .insert(from_device.clone());
            Some(Command::ProcessSigningRound1 {
                from_device,
                commitment_bytes,
            })
        }

        Message::ProcessSigningRound2 { from_device, share_bytes } => {
            debug!(
                "Routing SIGN_SHARE from {} ({} bytes) to the protocol layer",
                from_device,
                share_bytes.len()
            );
            // Stage 4: record share receipt → advances the roster row
            // from "✓ committed" to "✓✓ shared".
            model
                .wallet_state
                .signing_shares_received
                .insert(from_device.clone());
            Some(Command::ProcessSigningRound2 {
                from_device,
                share_bytes,
            })
        }

        // Dispatched by the SignTransaction screen (C.3) once the user
        // confirms a message to sign. We assume the wallet is already
        // unlocked — UnlockWallet is dispatched upstream. Kick off the
        // ceremony AND push SigningProgress so the user gets a "waiting
        // for peers" screen instead of sitting on the input form after
        // they've submitted. The session_id on AppState is reused as the
        // request_id for the progress screen so the joiner path lands on
        // the same screen key.
        Message::InitiateSigning { request } => {
            info!(
                "Initiating signing for wallet '{}' ({} bytes of transaction data)",
                request.wallet_id,
                request.transaction_data.len()
            );
            let request_id = model
                .active_session
                .as_ref()
                .map(|s| s.session_id.clone())
                .unwrap_or_else(|| "inline".to_string());
            // Stage 4: wipe any stale roster from a previous ceremony
            // before the new one starts recording commitments.
            model.wallet_state.signing_commitments_received.clear();
            model.wallet_state.signing_shares_received.clear();
            model.push_screen(Screen::SigningProgress {
                request_id,
            });
            Some(Command::StartSigning { request })
        }

        // ----- Phase C.5: signing ceremony terminal handlers -----
        //
        // `protocal::signing::try_aggregate` emits SigningComplete on
        // every participant that has accumulated threshold shares —
        // which means all nodes in a 2-of-2 ceremony, or the first
        // threshold-many to hear from each other in larger quorums.
        // Stash the snapshot + push SignatureComplete + clear all
        // transient signing state so the next ceremony starts clean.
        Message::SigningComplete { request_id, message, signature } => {
            info!(
                "🎉 Signing complete: request_id={} signature={} bytes",
                request_id,
                signature.len()
            );

            let wallet_id = model
                .wallet_state
                .last_finalized_wallet
                .as_ref()
                .map(|w| w.wallet_id.clone())
                .or_else(|| model.selected_wallet.clone())
                .or_else(|| {
                    model
                        .active_session
                        .as_ref()
                        .and_then(|s| match &s.session_type {
                            crate::protocal::signal::SessionType::Signing {
                                wallet_name,
                                ..
                            } => Some(wallet_name.clone()),
                            _ => None,
                        })
                })
                .unwrap_or_else(|| "(unknown)".to_string());

            // If the creator typed a message, `message` (passed in from
            // `Message::SigningComplete`) is the 32-byte EIP-191 hash
            // FROST actually signed. Use `pending_raw_message` for the
            // user-facing "Message: ..." display, and record the hash
            // separately as `signed_hash` so the SignatureComplete
            // screen can show both — and call out that this is
            // `personal_sign` compatible. Joiner sides never
            // populated `pending_raw_message`; they render the hash
            // only.
            let raw_message = model.wallet_state.pending_raw_message.take();
            let signed_hash = if raw_message.is_some() {
                Some(message.clone())
            } else {
                None
            };
            let display_message = raw_message.unwrap_or_else(|| message.clone());

            model.wallet_state.last_completed_signature =
                Some(crate::elm::model::CompletedSignatureInfo {
                    request_id: request_id.clone(),
                    wallet_id: wallet_id.clone(),
                    message: display_message,
                    signed_hash,
                    signature,
                    // `protocal::signing::try_aggregate` gates the emit
                    // on a successful verify — see the guard there.
                    // If that ever changes we'll need to re-verify
                    // here.
                    verified: true,
                });

            model.ui_state.notifications.push(Notification {
                id: Uuid::new_v4().to_string(),
                text: format!("🎉 Signature complete for '{}'", wallet_id),
                kind: NotificationKind::Success,
                timestamp: Utc::now(),
                dismissible: true,
            });

            // Drop any leftover pending-sign state — the flow is over.
            model.wallet_state.pending_sign_message = None;
            model.wallet_state.pending_sign_wallet_id = None;
            model.wallet_state.pending_sign_session_id = None;
            model.wallet_state.clear_sign_draft();
            // Stage 4: the acceptance roster has served its purpose;
            // retain nothing into the next ceremony.
            model.wallet_state.signing_commitments_received.clear();
            model.wallet_state.signing_shares_received.clear();

            // Same stack-reset pattern as DKGFinalized: go_home first so
            // pop_screen from SignatureComplete cleanly lands on MainMenu.
            model.go_home();
            model.push_screen(Screen::SignatureComplete { request_id });
            model.ui_state.focus = crate::elm::model::ComponentId::SignatureComplete;
            None
        }

        Message::SigningFailed { request_id, error } => {
            error!("Signing ceremony {} failed: {}", request_id, error);
            model.ui_state.modal = Some(Modal::Error {
                title: "Signing Failed".to_string(),
                message: crate::elm::error_help::signing(&error),
            });
            // Clear pending-sign state so a retry starts clean.
            model.wallet_state.pending_sign_message = None;
            model.wallet_state.pending_sign_wallet_id = None;
            model.wallet_state.pending_sign_session_id = None;
            model.wallet_state.clear_sign_draft();
            // Stage 4: wipe the acceptance roster too so a retry
            // doesn't render as if the prior commitments are still live.
            model.wallet_state.signing_commitments_received.clear();
            model.wallet_state.signing_shares_received.clear();
            None
        }

        // Generic clipboard copy — reused by WalletComplete /
        // SignatureComplete / anywhere else a single hero string is
        // worth grabbing with one keypress.
        //
        // Why the thread: on Linux X11, `arboard::Clipboard::set_text`
        // establishes this process as the X selection owner and then
        // blocks until something claims the selection (or X times out).
        // Calling it inline from the Elm update loop froze the whole
        // TUI for tens of seconds during full-flow testing. Spawning
        // a `std::thread` keeps the event loop responsive: the thread
        // holds the `Clipboard` handle open for a few seconds so
        // Wayland / X11 clipboard managers have time to latch the
        // contents, then drops it.
        //
        // Notification is pushed *optimistically* — we don't know from
        // inside the update loop whether the set succeeded. Better UX
        // than silence; the user will re-press `C` if they notice the
        // paste didn't work.
        Message::CopyToClipboard { text, label } => {
            // Build the notification text up front (while we still own
            // `text` + `label`) so the thread below can take everything
            // else by move without fighting the borrow checker.
            let preview = if text.len() > 80 {
                format!("{}…", &text[..80])
            } else {
                text.clone()
            };
            let notif_text = format!("📋 Copied {}: {}", label, preview);
            let log_label = label;
            let log_len = text.len();

            std::thread::spawn(move || {
                match arboard::Clipboard::new().and_then(|mut c| {
                    c.set_text(text)?;
                    // Hold the Clipboard handle alive long enough for a
                    // clipboard manager to grab the content. Two seconds
                    // is generous for interactive use and imperceptible
                    // from the user's side.
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    Ok(())
                }) {
                    Ok(()) => {
                        info!("Clipboard set: {} ({} bytes)", log_label, log_len);
                    }
                    Err(e) => {
                        warn!("Clipboard set failed ({}): {}", log_label, e);
                    }
                }
            });

            model.ui_state.notifications.push(Notification {
                id: Uuid::new_v4().to_string(),
                text: notif_text,
                kind: NotificationKind::Success,
                timestamp: Utc::now(),
                dismissible: true,
            });
            None
        }

        // ----- SignTransaction screen input (Phase C.3) -----
        Message::SignTypeChar(c) => {
            model.wallet_state.sign_message_draft.push(c);
            None
        }
        Message::SignBackspace => {
            model.wallet_state.sign_message_draft.pop();
            None
        }
        Message::SignSubmit => {
            // Empty-message check: FROST will happily sign an empty byte
            // string, but doing so accidentally would be awful UX.
            if model.wallet_state.sign_message_draft.is_empty() {
                model.ui_state.notifications.push(Notification {
                    id: Uuid::new_v4().to_string(),
                    text: "Message is empty — type something to sign".to_string(),
                    kind: NotificationKind::Warning,
                    timestamp: Utc::now(),
                    dismissible: true,
                });
                return None;
            }

            // Derive wallet_id from the current screen if we're on
            // SignTransaction; fall back to the selected wallet
            // otherwise. One or the other is always populated by the
            // navigation path (WalletDetail → Sign action → push
            // SignTransaction { wallet_id }).
            let wallet_id = match &model.current_screen {
                Screen::SignTransaction { wallet_id } => wallet_id.clone(),
                _ => match &model.wallet_state.selected_wallet {
                    Some(id) => id.clone(),
                    None => {
                        warn!(
                            "SignSubmit with no wallet_id available — navigation \
                             path broke upstream"
                        );
                        return None;
                    }
                },
            };

            let raw_message_bytes = model.wallet_state.sign_message_draft.as_bytes().to_vec();
            // For secp256k1 wallets, sign the EIP-191 hash of the message
            // rather than the raw bytes. That's what `ecrecover`
            // expects — so the resulting signature is directly usable
            // as an Ethereum `personal_sign` output. For ed25519 /
            // future curves, the raw bytes ARE the payload (Ed25519
            // signs variable-length input natively).
            let curve = model.wallet_state.curve_type;
            let (bytes_to_sign, raw_for_display) = if curve == "secp256k1" {
                let hash =
                    crate::utils::eth_helper::eip191_hash(&raw_message_bytes).to_vec();
                (hash, Some(raw_message_bytes))
            } else {
                (raw_message_bytes, None)
            };

            let warm = model.wallet_state.wallet_unlocked_id.as_deref() == Some(&wallet_id);

            // Stage 3: instead of firing the signing flow immediately,
            // stash the computed request in `pending_sign_preview` and
            // surface a confirmation modal. FROST signatures are
            // broadcast-then-irrevocable — a tap-Enter-once-too-many on
            // SignTransaction shouldn't silently announce to the mesh.
            // Build a UTF-8 / hex preview for the body. Chars + hash
            // are already computed above so the modal shows exactly
            // what will be signed.
            let preview_body = preview_lines(
                &wallet_id,
                curve,
                &bytes_to_sign,
                raw_for_display.as_deref(),
                &model.wallet_state.wallets,
            );

            model.wallet_state.pending_sign_preview = Some(PendingSignPreview {
                wallet_id,
                bytes_to_sign,
                raw_message: raw_for_display,
                warm,
            });
            model.ui_state.modal = Some(Modal::Confirm {
                title: "📝 Confirm Signing Request".to_string(),
                message: preview_body,
                on_confirm: Box::new(Message::ConfirmSigningRequest),
                on_cancel: Box::new(Message::CancelSigningRequest),
            });
            None
        }

        Message::ConfirmSigningRequest => {
            // Take() the preview so a double-fire can't dispatch twice.
            let preview = match model.wallet_state.pending_sign_preview.take() {
                Some(p) => p,
                None => {
                    warn!("ConfirmSigningRequest without a pending preview — dropped");
                    return None;
                }
            };
            // Tear the modal down now that the decision's been made;
            // the remaining flow (modal in cold path is the
            // PasswordPrompt screen itself) doesn't expect this one.
            model.ui_state.modal = None;

            // Clear the draft so an extra Enter doesn't re-trigger the
            // exact same submit. Keep pending_raw_message populated for
            // SignatureComplete's "what I typed vs what was signed" row.
            model.wallet_state.clear_sign_draft();
            model.wallet_state.pending_raw_message = preview.raw_message.clone();

            if preview.warm {
                let request = crate::elm::message::SigningRequest {
                    wallet_id: preview.wallet_id,
                    transaction_data: preview.bytes_to_sign,
                    chain: model.wallet_state.curve_type.to_string(),
                    metadata: None,
                    raw_message: preview.raw_message,
                };
                Some(Command::SendMessage(Message::InitiateSigning { request }))
            } else {
                info!(
                    "ConfirmSigningRequest: wallet '{}' not unlocked — routing through PasswordPrompt",
                    preview.wallet_id
                );
                model.wallet_state.pending_sign_message = Some(preview.bytes_to_sign);
                model.wallet_state.pending_sign_wallet_id = Some(preview.wallet_id);
                model.wallet_state.pending_sign_session_id = None;
                // The screen is shared with creator/joiner DKG, which set
                // a new password. Cold-start sign instead UNLOCKS an
                // existing wallet — flip the purpose so the component
                // renders single-field "Unlock Wallet" and validation
                // skips the confirm-match check.
                model.wallet_state.password_prompt_purpose =
                    crate::elm::model::PasswordPromptPurpose::Unlock;
                model.push_screen(Screen::PasswordPrompt);
                model.ui_state.focus = crate::elm::model::ComponentId::PasswordPrompt;
                None
            }
        }

        Message::CancelSigningRequest => {
            info!("CancelSigningRequest: dismissing preview modal, keeping draft");
            model.wallet_state.pending_sign_preview = None;
            model.ui_state.modal = None;
            None
        }

        // Phase C.1: signing-time keystore hydration results. These are
        // emitted by `Command::UnlockWallet`.
        //
        // If we're in the middle of a pending signing flow (joiner path
        // routed through PasswordPrompt → UnlockWallet), take the
        // stashed session_id + message_bytes and dispatch
        // `Command::JoinSigning` now that the key share is loaded. Also
        // push a SigningProgress screen so the user sees what's happening.
        //
        // For a plain "just unlocked, no queued action" flow we just log
        // + show a toast.
        Message::WalletUnlocked { wallet_id } => {
            info!("✅ Wallet unlocked: {}", wallet_id);
            // Mark the wallet as unlocked so subsequent SignSubmits on
            // the same wallet don't ask for the password again.
            model.wallet_state.wallet_unlocked_id = Some(wallet_id.clone());
            model.ui_state.notifications.push(Notification {
                id: Uuid::new_v4().to_string(),
                text: format!("🔓 Wallet '{}' unlocked", wallet_id),
                kind: NotificationKind::Success,
                timestamp: Utc::now(),
                dismissible: true,
            });

            let pending_msg = model.wallet_state.pending_sign_message.take();
            let pending_sid = model.wallet_state.pending_sign_session_id.take();
            let pending_wallet = model.wallet_state.pending_sign_wallet_id.take();

            match (pending_msg, pending_sid, pending_wallet) {
                // Joiner path — session_id is present because the user
                // accepted a signing session off the JoinSession screen,
                // the SubmitPassword handler stashed everything before
                // dispatching UnlockWallet.
                (Some(msg), Some(sid), Some(wid)) => {
                    if wid != wallet_id {
                        warn!(
                            "WalletUnlocked for {} but pending_sign_wallet_id was {} — \
                             proceeding with unlocked wallet",
                            wallet_id, wid
                        );
                    }
                    info!(
                        "🖊️  WalletUnlocked with pending signing ({} bytes) — \
                         dispatching JoinSigning on session {}",
                        msg.len(),
                        sid
                    );
                    model.push_screen(Screen::SigningProgress {
                        request_id: sid.clone(),
                    });
                    Some(Command::JoinSigning {
                        session_id: sid,
                        message_bytes: msg,
                    })
                }
                // Creator cold-start path — the user pressed Sign on a
                // wallet whose KeyPackage wasn't in AppState, so
                // SignSubmit stashed the message and routed through
                // PasswordPrompt. No session existed yet; InitiateSigning
                // announces the new one from scratch.
                (Some(msg), None, Some(wid)) => {
                    info!(
                        "🖊️  WalletUnlocked with pending cold-start sign ({} bytes) — \
                         dispatching InitiateSigning for wallet {}",
                        msg.len(),
                        wid
                    );
                    let request = crate::elm::message::SigningRequest {
                        wallet_id: wid,
                        transaction_data: msg,
                        chain: model.wallet_state.curve_type.to_string(),
                        metadata: None,
                        // Cold-start: raw_message was set by SignSubmit
                        // into pending_raw_message; re-surface it via
                        // the request so downstream paths treat this
                        // identically to the warm flow. (SigningComplete
                        // reads it back out of Model regardless; having
                        // it on the request is mostly for logging.)
                        raw_message: model.wallet_state.pending_raw_message.clone(),
                    };
                    Some(Command::SendMessage(Message::InitiateSigning { request }))
                }
                _ => None,
            }
        }

        Message::WalletUnlockFailed { error } => {
            // Surface as a modal so the user has to acknowledge before
            // continuing — signing with a stale/missing KeyPackage would
            // produce nonsense, so we must block the flow until the user
            // either retries or navigates away.
            error!("Wallet unlock failed: {}", error);
            let (title, message) = crate::elm::error_help::unlock(&error);
            model.ui_state.modal = Some(Modal::Error { title, message });
            // Critical: clear any pending signing state so a subsequent
            // DKG-creator PasswordPrompt submit isn't hijacked into
            // UnlockWallet on this-failed wallet_id. Without this,
            // a bad password here permanently wedges every future
            // SubmitPassword into the cold-sign branch.
            model.wallet_state.pending_sign_message = None;
            model.wallet_state.pending_sign_wallet_id = None;
            model.wallet_state.pending_sign_session_id = None;
            model.wallet_state.pending_raw_message = None;
            None
        }

        Message::WalletsLoaded { wallets } => {
            info!("Loaded {} wallets", wallets.len());
            let old_count = model.wallet_state.wallets.len();
            model.wallet_state.wallets = wallets;
            
            // If on main menu and wallet count changed, force remount to update menu
            if matches!(model.current_screen, Screen::MainMenu | Screen::Welcome) && 
               old_count != model.wallet_state.wallets.len() {
                info!("Wallet count changed from {} to {}, forcing menu update", 
                      old_count, model.wallet_state.wallets.len());
                Some(Command::SendMessage(Message::Refresh))
            } else {
                None
            }
        }
        
        Message::DeleteWallet { wallet_id } => {
            // Show confirmation modal
            model.ui_state.modal = Some(Modal::Confirm {
                title: "Delete Wallet".to_string(),
                message: format!("Are you sure you want to delete wallet '{}'? This action cannot be undone.", wallet_id),
                on_confirm: Box::new(Message::WalletDeleted { wallet_id }),
                on_cancel: Box::new(Message::CloseModal),
            });
            None
        }
        
        Message::WalletDeleted { wallet_id } => {
            info!("Deleting wallet: {}", wallet_id);
            model.ui_state.modal = None;
            Some(Command::DeleteWallet { wallet_id })
        }
        
        // ============= Wallet Creation Flow =============
        Message::SelectMode(mode) => {
            if let Screen::CreateWallet(ref mut state) = model.current_screen {
                state.mode = Some(mode);
                // Auto-navigate to next step (skip curve selection - unified DKG handles all curves)
                model.push_screen(Screen::ThresholdConfig);
            }
            None
        }
        
        Message::SelectTemplate(template) => {
            if let Screen::CreateWallet(ref mut state) = model.current_screen {
                let is_custom = template.name == "Custom";
                state.template = Some(template);
                // Auto-navigate to configuration if custom, otherwise start DKG
                if is_custom {
                    model.push_screen(Screen::WalletConfiguration(Default::default()));
                } else {
                    // Start DKG with template configuration
                    return Some(Command::SendMessage(Message::ConfirmWalletCreation));
                }
            }
            None
        }
        
        // ============= DKG Operations =============
        Message::UpdateDKGSessionId { real_session_id } => {
            info!("Updating DKG session ID to real ID: {}", real_session_id);
            
            // Update the active session with the real DKG session ID
            if let Some(ref mut session) = model.active_session {
                session.session_id = real_session_id.clone();
            }
            
            // Update the screen to show the real session ID
            if let Screen::DKGProgress { ref mut session_id } = model.current_screen {
                *session_id = real_session_id;
            }
            
            // Force a remount to update the display
            Some(Command::SendMessage(Message::ForceRemount))
        }
        
        Message::UpdateParticipants { participants } => {
            info!("Updating participants list: {:?}", participants);
            
            // Update the active session with the current participants
            if let Some(ref mut session) = model.active_session {
                session.participants = participants;
                info!("Updated session participants to: {:?}", session.participants);
            }
            
            // Force a remount to update the display with new participants
            Some(Command::SendMessage(Message::ForceRemount))
        }
        
        Message::UpdateParticipantWebRTCStatus { device_id, webrtc_connected, data_channel_open } => {
            info!("Updating WebRTC status for {}: WebRTC={}, DataChannel={}",
                 device_id, webrtc_connected, data_channel_open);

            // Store the WebRTC status in the model's network state
            model.network_state.participant_webrtc_status
                .entry(device_id)
                .and_modify(|status| {
                    status.0 = webrtc_connected;
                    status.1 = data_channel_open;
                })
                .or_insert((webrtc_connected, data_channel_open));

            // Check if all participants are now connected and trigger DKG if needed
            let should_start_dkg = if let Some(ref session) = model.active_session {
                // Count how many participants have data channels open (excluding self)
                let connected_count = session.participants.iter()
                    .filter(|p| **p != model.device_id)
                    .filter(|p| {
                        model.network_state.participant_webrtc_status.get(*p)
                            .is_some_and(|(_, data_channel_open)| *data_channel_open)
                    })
                    .count();
                
                // CRITICAL FIX: Use session.total, not current participant count!
                // We must wait for the CONFIGURED total number of participants
                let required_total_participants = session.total as usize;
                let current_total_participants = session.participants.len();
                let expected_other_participants = required_total_participants.saturating_sub(1);
                
                info!("🔍 DKG trigger check: connected={}/{}, current_participants={}/{}, dkg_in_progress={}", 
                      connected_count, expected_other_participants, current_total_participants, required_total_participants, model.wallet_state.dkg_in_progress);
                
                // ALL participants start DKG when:
                // 1. We have the configured total number of participants in the session
                // 2. All of them are connected via WebRTC
                // 3. DKG hasn't started yet
                current_total_participants >= required_total_participants &&  // Have enough participants
                connected_count == expected_other_participants &&  // All others connected
                expected_other_participants > 0 &&  // Make sure we have other participants
                !model.wallet_state.dkg_in_progress
            } else {
                false
            };
            
            // Force a remount to update the display with new WebRTC status
            if matches!(model.current_screen, Screen::DKGProgress { .. }) {
                if should_start_dkg {
                    info!("🎯 All participants connected! Starting DKG protocol...");
                    // DON'T set dkg_in_progress here - let the command handler do it
                    // to avoid race condition where it's set before the command runs
                }
                // ALWAYS force remount to update the UI with the new connection status
                Some(Command::SendMessage(Message::ForceRemount))
            } else {
                None
            }
        }

        Message::UpdateMeshStatus { ready_count, total_count, all_connected } => {
            info!("Mesh status update: {}/{} ready, all_connected={}",
                 ready_count, total_count, all_connected);

            // Force a remount to update the display
            if matches!(model.current_screen, Screen::DKGProgress { .. }) {
                Some(Command::SendMessage(Message::ForceRemount))
            } else {
                None
            }
        }

        Message::UpdateDKGProgress { round, progress } => {
            // Previously this opened a Modal::Progress popup that was
            // never dismissed. The Modal stays in `ui_state.modal = Some(..)`
            // for the rest of the session, and the key dispatcher at the
            // top of `handle_key_event` bails on any non-{Enter,Esc} key
            // while a modal is open → Left/Right arrows on the DKG Progress
            // screen silently return None. The DKG Progress screen already
            // has its own inline progress bar; we just log the transition
            // here and leave the UI to the dedicated component.
            let message = match round {
                DKGRound::Initialization => "Initializing DKG protocol...",
                DKGRound::WaitingForParticipants => "Waiting for participants to join...",
                DKGRound::Round1 => "Round 1: Generating commitments...",
                DKGRound::Round2 => "Round 2: Distributing shares...",
                DKGRound::Finalization => "Finalizing wallet creation...",
                DKGRound::Complete => "DKG complete!",
            };
            info!(
                "DKG progress: {} ({:.0}%): {}",
                format!("{:?}", round),
                progress * 100.0,
                message
            );
            let _ = progress;
            None
        }
        
        Message::DKGComplete { result } => {
            info!("DKG completed successfully: {:?}", result);
            
            // Clear modal
            model.ui_state.modal = None;
            
            // Show success notification
            let notification = Notification {
                id: Uuid::new_v4().to_string(),
                text: format!("Wallet '{}' created successfully!", result.wallet_id),
                kind: NotificationKind::Success,
                timestamp: Utc::now(),
                dismissible: true,
            };
            model.ui_state.notifications.push(notification);
            
            // Navigate back to main menu to show updated menu with Sign Transaction
            model.go_home();
            
            // Reload wallet list which will trigger menu update
            Some(Command::LoadWallets)
        }
        
        Message::DKGFailed { error } => {
            error!("DKG failed: {}", error);

            // Show error modal - always stay on current screen so user can retry or press Esc to go back
            model.ui_state.modal = Some(Modal::Error {
                title: "DKG Failed".to_string(),
                message: crate::elm::error_help::dkg(&error),
            });

            // Reset DKG-in-progress flag so user can retry
            model.pending_operations.clear();

            None
        }
        
        Message::CancelDKG => {
            info!("🛑 CancelDKG requested by user");

            // Clear DKG state
            model.active_session = None;
            model.pending_operations.clear();
            model.wallet_state.creating_wallet = None;
            model.ui_state.modal = None;

            // Navigate back to main menu
            model.navigation_stack.clear();
            model.current_screen = Screen::MainMenu;
            model.ui_state.focus = crate::elm::model::ComponentId::MainMenu;
            model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::MainMenu).or_insert(0);

            Some(Command::CancelDKG)
        }

        Message::StartDKGProtocol => {
            // Fired by the WebRTC layer in two places:
            //   1. `mesh_ready` control-frame handler (network/webrtc.rs) once
            //      every peer has acked mesh readiness.
            //   2. The data-channel open-count check once the local node has
            //      established every expected peer connection.
            // Either way, this is the authoritative "all channels ready" edge
            // for the joiner path. The creator has its own polling loop inside
            // Command::InitiateWebRTCConnections that produces Message::InitiateDKG.
            // Both converge on Command::StartFrostProtocol, which is idempotent
            // via the DkgState::Idle → Round1InProgress guard.
            info!("🚀 StartDKGProtocol — mesh ready, dispatching FROST Round 1 trigger");
            enter_round1(model);
            Some(frost_trigger_batch(model))
        }

        Message::InitiateDKG { params } => {
            // Fired when the WebRTC mesh becomes ready. This runs on EVERY
            // participant — creator and joiners alike — so it must not touch
            // session-announcement concerns. Previously this dispatched
            // `Command::StartDKG`, which had session announcement baked in;
            // when joiners hit that path they'd re-announce the session
            // under their own proposer_id and clobber the creator's record
            // server-side. We only want the FROST trigger here.
            info!("Mesh is ready — dispatching FROST Round 1 trigger. params={:?}", params);
            enter_round1(model);
            Some(frost_trigger_batch(model))
        }
        
        Message::ProcessDKGRound1 { from_device, package_bytes } => {
            info!(
                "Queueing DKG Round 1 package from {} ({} bytes) for FROST part1 processing",
                from_device,
                package_bytes.len()
            );
            Some(Command::ProcessDKGRound1 {
                from_device,
                package_bytes,
            })
        }

        Message::ProcessReshareRound1 { from_device, package_bytes } => {
            Some(Command::ProcessReshareRound1 { from_device, package_bytes })
        }
        Message::ProcessReshareRound2 { from_device, package_bytes } => {
            Some(Command::ProcessReshareRound2 { from_device, package_bytes })
        }
        Message::ProcessUnifiedDKGRound1 { from_device, package_json } => {
            Some(Command::ProcessUnifiedDKGRound1 { from_device, package_json })
        }
        Message::ProcessUnifiedDKGRound2 { from_device, message_json } => {
            Some(Command::ProcessUnifiedDKGRound2 { from_device, message_json })
        }
        Message::HeadlessReshare { wallet_id, password, keystore_path } => {
            Some(Command::StartReshare { wallet_id, password, keystore_path })
        }
        Message::ReshareComplete { .. } => {
            // Terminal event; tapped by the CLI bridge / simulate harness. No
            // model transition needed.
            None
        }

        Message::ProcessDKGRound2 { from_device, package_bytes } => {
            // First peer Round 2 package → we've clearly advanced past Round 1.
            // Update the UI label. Idempotent: we only transition on the first
            // one and stay at Round2 until DKGKeyGenerated bumps us to Finalization.
            let round2_edge = matches!(
                model.wallet_state.dkg_round,
                DKGRound::Initialization | DKGRound::WaitingForParticipants | DKGRound::Round1
            );
            if round2_edge {
                model.wallet_state.dkg_round = DKGRound::Round2;
            }
            info!(
                "Queueing DKG Round 2 package from {} ({} bytes) for FROST part3 processing",
                from_device,
                package_bytes.len()
            );
            let process_cmd = Command::ProcessDKGRound2 {
                from_device,
                package_bytes,
            };
            if round2_edge && matches!(model.current_screen, Screen::DKGProgress { .. }) {
                Some(Command::Batch(vec![
                    process_cmd,
                    Command::SendMessage(Message::ForceRemount),
                ]))
            } else {
                Some(process_cmd)
            }
        }

        Message::DKGKeyGenerated { group_pubkey_hex } => {
            info!("🎉 DKG finalised. Group verifying key: {}", group_pubkey_hex);
            // Terminal UI state: 100% and a "done" label. Previously we set
            // `Finalization` here, which has a hardcoded 95% progress bar and
            // a "Finalizing DKG..." label — making a successful DKG look as if
            // it were still in progress. `DKGRound::Complete` is the 100%
            // terminal variant added specifically for this transition.
            // `part3` has already populated `public_key_package` at the
            // protocol layer, so setting `dkg_in_progress = false` is safe —
            // a subsequent wallet-creation flow won't collide.
            model.wallet_state.dkg_round = DKGRound::Complete;
            model.wallet_state.dkg_in_progress = false;
            model.ui_state.notifications.push(Notification {
                id: Uuid::new_v4().to_string(),
                text: format!("🎉 DKG complete — group key {}…", &group_pubkey_hex[..16]),
                kind: NotificationKind::Success,
                timestamp: Utc::now(),
                dismissible: true,
            });

            // Auto-trigger the keystore persistence step. The password was
            // collected on `PasswordPrompt` and stashed on
            // `wallet_state.pending_password`; we hand it off to the Command
            // by value and immediately clear the Model-side copy so the
            // cleartext lives in exactly one place (the in-flight Command
            // arm) before being dropped. If the password is missing, we
            // log loudly but keep the UI responsive — this shouldn't
            // happen because both entry edges into `DKGProgress` come
            // through `Message::SubmitPassword`, but a panic here would
            // brick an otherwise-successful DKG.
            let finalize_cmd: Option<Command> = {
                let password = model.wallet_state.pending_password.take();
                let keystore_path = model.wallet_state.keystore_path.clone();

                // Wallet name is derived from the session so every participant
                // ends up with the same identifier — required for cross-device
                // signing coordination later. Shared derivation with `dkg.rs`
                // (`wallet_id_from_session`) so the persisted id matches the
                // in-memory `current_wallet_id` seeded post-part3.
                let wallet_name = model
                    .active_session
                    .as_ref()
                    .map(|s| crate::protocal::dkg::wallet_id_from_session(&s.session_id));

                // Optional user-chosen display label (creator typed it on the
                // password screen). Empty → None → UI falls back to the id.
                let wallet_label = {
                    let s = model.wallet_state.wallet_name_draft.trim().to_string();
                    model.wallet_state.wallet_name_draft.clear();
                    if s.is_empty() { None } else { Some(s) }
                };

                match (password, wallet_name) {
                    (Some(password), Some(wallet_name)) if !keystore_path.is_empty() => {
                        info!(
                            "Auto-dispatching FinalizeWalletFromDkg (keystore={}, name={}, label={:?})",
                            keystore_path, wallet_name, wallet_label
                        );
                        Some(Command::FinalizeWalletFromDkg {
                            password,
                            keystore_path,
                            wallet_name,
                            wallet_label,
                        })
                    }
                    (None, _) => {
                        warn!(
                            "DKGKeyGenerated with no pending_password — DKG produced a key \
                             but we have no password to encrypt it with. This usually means \
                             the user reached DKGProgress via a path that bypassed \
                             PasswordPrompt; the key share is in memory but won't be \
                             persisted this session."
                        );
                        None
                    }
                    (_, None) => {
                        warn!(
                            "DKGKeyGenerated with no active_session — cannot derive wallet \
                             name. Key share not persisted."
                        );
                        None
                    }
                    _ => {
                        warn!(
                            "DKGKeyGenerated with empty keystore_path — Model was never \
                             initialised with a keystore location. Key share not persisted."
                        );
                        None
                    }
                }
            };

            let remount_cmd = if matches!(model.current_screen, Screen::DKGProgress { .. }) {
                Some(Command::SendMessage(Message::ForceRemount))
            } else {
                None
            };

            match (finalize_cmd, remount_cmd) {
                (Some(fin), Some(rem)) => Some(Command::Batch(vec![rem, fin])),
                (Some(fin), None) => Some(fin),
                (None, Some(rem)) => Some(rem),
                (None, None) => None,
            }
        }

        // Fires after the keystore file has been written (see
        // `Command::FinalizeWalletFromDkg`). Stashes a snapshot onto
        // `wallet_state.last_finalized_wallet` for the WalletComplete
        // screen to render, clears all the transient DKG/password state,
        // and drops the user on that success screen with a refreshed
        // wallet list waiting behind it.
        Message::DKGFinalized {
            wallet_id,
            group_pubkey_hex,
            curve_type,
            addresses,
        } => {
            info!(
                "✅ Wallet finalized: id='{}' curve={} group={}… addresses={}",
                wallet_id,
                curve_type,
                &group_pubkey_hex[..16.min(group_pubkey_hex.len())],
                addresses.len()
            );

            // Belt-and-suspenders: the Command consumed `password` already,
            // but there's no harm in clearing `pending_password` here too —
            // the update layer is the source of truth for Model state and
            // this guarantees the field is None before any future screen
            // reads it.
            model.wallet_state.pending_password = None;
            model.wallet_state.creating_wallet = None;
            model.wallet_state.dkg_in_progress = false;

            // Snapshot for the WalletComplete component to read.
            // Stored BEFORE we push the screen so `mount_components` sees
            // the value on its first remount.
            model.wallet_state.last_finalized_wallet =
                Some(crate::elm::model::CompletedWalletInfo {
                    wallet_id: wallet_id.clone(),
                    group_pubkey_hex,
                    curve_type,
                    addresses: addresses.clone(),
                });
            // DKG leaves the KeyPackage live on AppState; mark the
            // wallet as unlocked so an immediate SignSubmit can skip
            // the PasswordPrompt roundtrip.
            model.wallet_state.wallet_unlocked_id = Some(wallet_id.clone());

            model.ui_state.notifications.push(Notification {
                id: Uuid::new_v4().to_string(),
                text: format!(
                    "✅ Wallet '{}' created with {} chain address{}",
                    wallet_id,
                    addresses.len(),
                    if addresses.len() == 1 { "" } else { "es" }
                ),
                kind: NotificationKind::Success,
                timestamp: Utc::now(),
                dismissible: true,
            });

            // We want Esc/Enter on WalletComplete to land on MainMenu,
            // not DKGProgress or PasswordPrompt (which are stale). Clear
            // the stack, pin current_screen to MainMenu first, and only
            // then push WalletComplete — this way pop_screen leaves us
            // on a fresh MainMenu.
            model.go_home();
            model.push_screen(Screen::WalletComplete {
                wallet_id,
            });
            model.ui_state.focus = crate::elm::model::ComponentId::WalletComplete;

            Some(Command::LoadWallets)
        }

        // ============= Network Events =============
        Message::InitiateWebRTCWithParticipants { participants } => {
            info!("Initiating WebRTC connections with {} participants", participants.len());
            Some(Command::InitiateWebRTCConnections { participants })
        }
        
        Message::CheckWebRTCConnections => {
            info!("Checking WebRTC connection status");
            Some(Command::VerifyWebRTCMesh)
        }
        
        Message::VerifyMeshConnectivity => {
            info!("🔍 Verifying WebRTC mesh connectivity");

            // Check current connection status from network state
            if let Some(session) = &model.active_session {
                let expected_connections = session.participants.len().saturating_sub(1);

                // Count how many participants have data channels open (excluding self)
                let connected_count = session.participants.iter()
                    .filter(|p| **p != model.device_id)
                    .filter(|p| {
                        model.network_state.participant_webrtc_status.get(*p)
                            .is_some_and(|(_, data_channel_open)| *data_channel_open)
                    })
                    .count();

                info!("📊 Mesh Status: {}/{} data channels open", connected_count, expected_connections);

                if connected_count < expected_connections {
                    info!("⚠️ Mesh incomplete, triggering re-initiation");
                    // Re-initiate with all participants
                    let other_participants: Vec<String> = session.participants.iter()
                        .filter(|p| **p != model.device_id)
                        .cloned()
                        .collect();

                    Some(Command::InitiateWebRTCConnections { participants: other_participants })
                } else {
                    info!("✅ Mesh complete! All participants connected");
                    // Trigger DKG start if not already in progress. StartFrostProtocol
                    // is idempotent — if FROST is already running it's a no-op.
                    if !model.wallet_state.dkg_in_progress {
                        enter_round1(model);
                        Some(Command::Batch(vec![
                            Command::StartFrostProtocol,
                            Command::SendMessage(Message::ForceRemount),
                        ]))
                    } else {
                        None
                    }
                }
            } else {
                Some(Command::EnsureFullMesh)
            }
        }
        
        Message::WebSocketConnected => {
            info!("WebSocket connected");
            model.network_state.connected = true;
            model.network_state.connection_status = ConnectionStatus::Connected;
            model.network_state.reconnect_attempts = 0;
            
            // Show success notification
            let notification = Notification {
                id: Uuid::new_v4().to_string(),
                text: "Connected to network".to_string(),
                kind: NotificationKind::Success,
                timestamp: Utc::now(),
                dismissible: true,
            };
            model.ui_state.notifications.push(notification);
            
            // Chain follow-up work: redraw the WS-status-aware screens, and if
            // the user is already on Join Session, re-run discovery over the
            // freshly-open primary channel (LoadSessions previously would have
            // no-op'd if the socket wasn't up yet).
            let mut follow_ups: Vec<Command> = Vec::new();
            if matches!(
                model.current_screen,
                Screen::DKGProgress { .. } | Screen::ModeSelection
            ) {
                follow_ups.push(Command::SendMessage(Message::ForceRemount));
            }
            if matches!(model.current_screen, Screen::JoinSession) {
                follow_ups.push(Command::LoadSessions);
            }
            match follow_ups.len() {
                0 => None,
                1 => Some(follow_ups.pop().unwrap()),
                _ => Some(Command::Batch(follow_ups)),
            }
        }

        Message::WebSocketDisconnected => {
            warn!("WebSocket disconnected");
            model.network_state.connected = false;
            model.network_state.connection_status = ConnectionStatus::Disconnected;
            model.network_state.reconnect_attempts += 1;

            // Show warning notification
            let notification = Notification {
                id: Uuid::new_v4().to_string(),
                text: "Disconnected from network".to_string(),
                kind: NotificationKind::Warning,
                timestamp: Utc::now(),
                dismissible: true,
            };
            model.ui_state.notifications.push(notification);

            // Remount UI immediately if we're on a screen that displays WebSocket status,
            // since the mounted component captured the old (connected) state at mount time.
            let remount_cmd = if matches!(
                model.current_screen,
                Screen::DKGProgress { .. } | Screen::ModeSelection
            ) {
                Some(Command::SendMessage(Message::ForceRemount))
            } else {
                None
            };

            // Attempt reconnection with exponential backoff
            let reconnect_cmd = if model.network_state.reconnect_attempts
                <= model.network_state.max_reconnect_attempts
            {
                let delay = 2000 * model.network_state.reconnect_attempts as u64;
                warn!(
                    "Scheduling reconnect attempt {} in {}ms",
                    model.network_state.reconnect_attempts, delay
                );
                Some(Command::ScheduleMessage {
                    delay_ms: delay,
                    message: Box::new(Message::TriggerReconnect),
                })
            } else {
                warn!(
                    "Max reconnect attempts ({}) reached, giving up",
                    model.network_state.max_reconnect_attempts
                );
                model.network_state.connection_status =
                    ConnectionStatus::Failed("Max reconnect attempts reached".to_string());
                None
            };

            match (remount_cmd, reconnect_cmd) {
                (Some(a), Some(b)) => Some(Command::Batch(vec![a, b])),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            }
        }

        Message::TriggerReconnect => {
            model.network_state.connection_status = ConnectionStatus::Reconnecting;
            Some(Command::ReconnectWebSocket)
        }
        
        // ============= UI Events =============
        Message::KeyPressed(key) => {
            // Global key handling
            match key.code {
                KeyCode::Esc => {
                    // ALWAYS navigate back, NEVER exit
                    if model.ui_state.modal.is_some() {
                        // Close modal first
                        model.ui_state.modal = None;
                        None
                    } else {
                        // Navigate back
                        Some(Command::SendMessage(Message::NavigateBack))
                    }
                }
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Only Ctrl+Q exits the application
                    Some(Command::SendMessage(Message::Quit))
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl+H goes home
                    Some(Command::SendMessage(Message::NavigateHome))
                }
                KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl+R refreshes
                    Some(Command::SendMessage(Message::Refresh))
                }
                _ => {
                    // Delegate to focused component
                    None
                }
            }
        }
        
        Message::FocusChanged { component } => {
            model.ui_state.focus = component;
            None
        }
        
        Message::InputChanged { value } => {
            model.ui_state.input_buffer = value;
            None
        }
        
        Message::ScrollUp => {
            info!("⬆️ ScrollUp: current screen = {:?}", model.current_screen);
            // Update selected index based on current screen
            match model.current_screen {
                Screen::MainMenu | Screen::Welcome => {
                    let current_idx = model.ui_state.selected_indices
                        .entry(model.ui_state.focus.clone())
                        .or_insert(0);
                    
                    // Get menu item count based on wallet state
                    let menu_item_count = if model.wallet_state.wallets.is_empty() {
                        4  // Create, Join, Settings, Exit
                    } else {
                        6  // Create, Join, Manage, Sign, Settings, Exit
                    };
                    
                    // Wrap around
                    if *current_idx == 0 {
                        *current_idx = menu_item_count - 1;  // Last item index
                    } else {
                        *current_idx = current_idx.saturating_sub(1);
                    }
                    info!("MainMenu selection moved up to: {}", current_idx);
                }
                Screen::CreateWallet(_) => {
                    // Handle CreateWallet navigation
                    debug!("🔼 ScrollUp on CreateWallet, focus: {:?}", model.ui_state.focus);
                    debug!("🔼 Before: selected_indices = {:?}", model.ui_state.selected_indices);
                    
                    let current_idx = model.ui_state.selected_indices
                        .entry(model.ui_state.focus.clone())
                        .or_insert(0);
                    
                    let old_idx = *current_idx;
                    if *current_idx == 0 {
                        *current_idx = 3;  // Wrap to bottom (4 items: 0-3)
                    } else {
                        *current_idx = current_idx.saturating_sub(1);
                    }
                    info!("🔼 CreateWallet selection moved up: {} -> {}", old_idx, current_idx);
                    debug!("🔼 After: selected_indices = {:?}", model.ui_state.selected_indices);
                }
                Screen::ModeSelection => {
                    // ModeSelection doesn't respond to Up - only Left/Right
                    debug!("ModeSelection: Ignoring Up arrow (use Left/Right to switch modes)");
                }
                Screen::ThresholdConfig => {
                    // Get which field is selected (0 = participants, 1 = threshold)
                    let selected_field = model.ui_state.selected_indices
                        .get(&crate::elm::model::ComponentId::ThresholdConfig)
                        .copied()
                        .unwrap_or(0);
                    
                    // Ensure we have the creating_wallet state with custom_config
                    if let Some(ref mut creating_wallet) = model.wallet_state.creating_wallet {
                        // Initialize custom_config if not present
                        if creating_wallet.custom_config.is_none() {
                            creating_wallet.custom_config = Some(WalletConfig {
                                name: "MPC Wallet".to_string(),
                                total_participants: 3,
                                threshold: 2,
                                mode: creating_wallet.mode.clone().unwrap_or_default(),
                            });
                        }

                        if let Some(ref mut config) = creating_wallet.custom_config {
                            if selected_field == 0 {
                                // Increase participants (max 10)
                                if config.total_participants < 10 {
                                    config.total_participants += 1;
                                    // Ensure threshold doesn't exceed participants
                                    config.threshold = config.threshold.min(config.total_participants);
                                    info!("ThresholdConfig: Participants increased to {}", config.total_participants);
                                }
                            } else {
                                // Increase threshold (max = participants)
                                if config.threshold < config.total_participants {
                                    config.threshold += 1;
                                    info!("ThresholdConfig: Threshold increased to {}", config.threshold);
                                }
                            }
                        }
                    }
                }
                Screen::JoinSession => {
                    // Handle JoinSession navigation for arrow up
                    let current_idx = model.ui_state.selected_indices
                        .entry(crate::elm::model::ComponentId::JoinSession)
                        .or_insert(0);

                    if *current_idx > 0 {
                        *current_idx = current_idx.saturating_sub(1);
                    }
                    info!("JoinSession selection moved up to: {}", current_idx);
                }
                Screen::ManageWallets => {
                    // Wallet list navigation — previously routed through the
                    // generic `_ =>` arm that only bumped `scroll_position`,
                    // which WalletList doesn't read. The component's `selected`
                    // is re-applied from this map at mount time via
                    // `WalletList::set_selected`.
                    let current_idx = model
                        .ui_state
                        .selected_indices
                        .entry(crate::elm::model::ComponentId::WalletList)
                        .or_insert(0);
                    if *current_idx > 0 {
                        *current_idx -= 1;
                    }
                    info!("WalletList selection moved up to: {}", current_idx);
                }
                _ => {
                    model.ui_state.scroll_position = model.ui_state.scroll_position.saturating_sub(1);
                }
            }
            None
        }

        Message::ScrollDown => {
            info!("⬇️ ScrollDown: current screen = {:?}", model.current_screen);
            // Update selected index based on current screen
            match model.current_screen {
                Screen::MainMenu | Screen::Welcome => {
                    let current_idx = model.ui_state.selected_indices
                        .entry(model.ui_state.focus.clone())
                        .or_insert(0);
                    
                    // Get menu item count based on wallet state
                    let menu_item_count = if model.wallet_state.wallets.is_empty() {
                        4  // Create, Join, Settings, Exit
                    } else {
                        6  // Create, Join, Manage, Sign, Settings, Exit
                    };
                    
                    // Wrap around
                    if *current_idx >= menu_item_count - 1 {
                        *current_idx = 0; // Back to top
                    } else {
                        *current_idx += 1;
                    }
                    info!("MainMenu selection moved down to: {}", current_idx);
                }
                Screen::CreateWallet(_) => {
                    // Handle CreateWallet navigation
                    debug!("🔽 ScrollDown on CreateWallet, focus: {:?}", model.ui_state.focus);
                    debug!("🔽 Before: selected_indices = {:?}", model.ui_state.selected_indices);
                    
                    let current_idx = model.ui_state.selected_indices
                        .entry(model.ui_state.focus.clone())
                        .or_insert(0);
                    
                    let old_idx = *current_idx;
                    if *current_idx >= 3 {
                        *current_idx = 0;  // Wrap to top
                    } else {
                        *current_idx += 1;
                    }
                    info!("🔽 CreateWallet selection moved down: {} -> {}", old_idx, current_idx);
                    debug!("🔽 After: selected_indices = {:?}", model.ui_state.selected_indices);
                }
                Screen::ModeSelection => {
                    // ModeSelection doesn't respond to Down - only Left/Right
                    debug!("ModeSelection: Ignoring Down arrow (use Left/Right to switch modes)");
                }
                Screen::ThresholdConfig => {
                    // Get which field is selected (0 = participants, 1 = threshold)
                    let selected_field = model.ui_state.selected_indices
                        .get(&crate::elm::model::ComponentId::ThresholdConfig)
                        .copied()
                        .unwrap_or(0);
                    
                    // Ensure we have the creating_wallet state with custom_config
                    if let Some(ref mut creating_wallet) = model.wallet_state.creating_wallet {
                        // Initialize custom_config if not present
                        if creating_wallet.custom_config.is_none() {
                            creating_wallet.custom_config = Some(WalletConfig {
                                name: "MPC Wallet".to_string(),
                                total_participants: 3,
                                threshold: 2,
                                mode: creating_wallet.mode.clone().unwrap_or_default(),
                            });
                        }

                        if let Some(ref mut config) = creating_wallet.custom_config {
                            if selected_field == 0 {
                                // Decrease participants (min 2)
                                if config.total_participants > 2 {
                                    config.total_participants -= 1;
                                    // Ensure threshold doesn't exceed participants
                                    config.threshold = config.threshold.min(config.total_participants);
                                    info!("ThresholdConfig: Participants decreased to {}", config.total_participants);
                                }
                            } else {
                                // Decrease threshold (min 2)
                                if config.threshold > 2 {
                                    config.threshold -= 1;
                                    info!("ThresholdConfig: Threshold decreased to {}", config.threshold);
                                }
                            }
                        }
                    }
                }
                Screen::JoinSession => {
                    // Handle JoinSession navigation for arrow down
                    // Note: The actual session count will be handled by the component itself
                    let current_idx = model.ui_state.selected_indices
                        .entry(crate::elm::model::ComponentId::JoinSession)
                        .or_insert(0);

                    // We don't know the actual count here, just increment
                    *current_idx += 1;
                    info!("JoinSession selection moved down to: {}", current_idx);
                }
                Screen::ManageWallets => {
                    let wallet_count = model.wallet_state.wallets.len();
                    if wallet_count == 0 {
                        return None;
                    }
                    let current_idx = model
                        .ui_state
                        .selected_indices
                        .entry(crate::elm::model::ComponentId::WalletList)
                        .or_insert(0);
                    if *current_idx + 1 < wallet_count {
                        *current_idx += 1;
                    }
                    info!("WalletList selection moved down to: {}", current_idx);
                }
                _ => {
                    model.ui_state.scroll_position = model.ui_state.scroll_position.saturating_add(1);
                }
            }
            None
        }

        Message::ScrollLeft => {
            info!("⬅️ ScrollLeft: current screen = {:?}", model.current_screen);
            match model.current_screen {
                Screen::ModeSelection => {
                    // Switch to Online mode (left side)
                    let current_idx = model.ui_state.selected_indices
                        .entry(model.ui_state.focus.clone())
                        .or_insert(0);
                    *current_idx = 0;
                    info!("ModeSelection switched to: Online");
                }
                Screen::ThresholdConfig => {
                    // Switch to participants field (left side) only if we're not already there
                    let current_idx = model.ui_state.selected_indices
                        .entry(crate::elm::model::ComponentId::ThresholdConfig)
                        .or_insert(0);
                    if *current_idx != 0 {
                        *current_idx = 0;
                        info!("ThresholdConfig switched to: Participants field");
                    } else {
                        debug!("Already on Participants field");
                    }
                }
                Screen::DKGProgress { .. } => {
                    // Switch between Cancel DKG and Copy Session ID buttons
                    let current_idx = model.ui_state.selected_indices
                        .entry(crate::elm::model::ComponentId::DKGProgress)
                        .or_insert(0);
                    if *current_idx > 0 {
                        *current_idx = 0;  // Switch to Cancel DKG
                        info!("DKGProgress switched to: Cancel DKG button");
                    } else {
                        debug!("Already on Cancel DKG button");
                    }
                }
                Screen::JoinSession => {
                    // Switch to DKG tab (left)
                    model.ui_state.join_session_tab = 0;
                    // Reset session selection when switching tabs
                    model.ui_state.selected_indices.insert(crate::elm::model::ComponentId::JoinSession, 0);
                    info!("JoinSession switched to DKG tab");
                }
                _ => {
                    debug!("ScrollLeft not handled for this screen");
                }
            }
            None
        }
        
        Message::ScrollRight => {
            info!("➡️ ScrollRight: current screen = {:?}", model.current_screen);
            match model.current_screen {
                Screen::ModeSelection => {
                    // Switch to Offline mode (right side)
                    let current_idx = model.ui_state.selected_indices
                        .entry(model.ui_state.focus.clone())
                        .or_insert(0);
                    *current_idx = 1;
                    info!("ModeSelection switched to: Offline");
                }
                Screen::ThresholdConfig => {
                    // Switch to threshold field (right side) only if we're not already there
                    let current_idx = model.ui_state.selected_indices
                        .entry(crate::elm::model::ComponentId::ThresholdConfig)
                        .or_insert(0);
                    if *current_idx != 1 {
                        *current_idx = 1;
                        info!("ThresholdConfig switched to: Threshold field");
                    } else {
                        debug!("Already on Threshold field");
                    }
                }
                Screen::DKGProgress { .. } => {
                    // Switch between Cancel DKG and Copy Session ID buttons
                    let current_idx = model.ui_state.selected_indices
                        .entry(crate::elm::model::ComponentId::DKGProgress)
                        .or_insert(0);
                    if *current_idx < 1 {
                        *current_idx = 1;  // Switch to Copy Session ID
                        info!("DKGProgress switched to: Copy Session ID button");
                    } else {
                        debug!("Already on Copy Session ID button");
                    }
                }
                Screen::JoinSession => {
                    // Switch to Signing tab (right)
                    model.ui_state.join_session_tab = 1;
                    // Reset session selection when switching tabs
                    model.ui_state.selected_indices.insert(crate::elm::model::ComponentId::JoinSession, 0);
                    info!("JoinSession switched to Signing tab");
                }
                _ => {
                    debug!("ScrollRight not handled for this screen");
                }
            }
            None
        }
        
        Message::SelectItem { index: _ } => {
            info!("SelectItem on screen: {:?}", model.current_screen);
            // Handle item selection based on current screen
            match model.current_screen {
                Screen::MainMenu | Screen::Welcome => {
                    // Get the current selected index
                    let selected_idx = model.ui_state.selected_indices
                        .get(&model.ui_state.focus)
                        .copied()
                        .unwrap_or(0);
                    
                    info!("MainMenu item selected: {}", selected_idx);
                    
                    // Check if we have wallets (affects menu structure)
                    let has_wallets = !model.wallet_state.wallets.is_empty();
                    
                    // Navigate based on menu selection
                    // Menu structure when no wallets: Create, Join, Settings, Exit (4 items)
                    // Menu structure with wallets: Create, Join, Manage, Sign, Settings, Exit (6 items)
                    match (selected_idx, has_wallets) {
                        (0, _) => {
                            // Create New Wallet - go directly to Mode Selection
                            info!("Navigating directly to Mode Selection");
                            // IMPORTANT: Reset the creating_wallet state to start fresh
                            model.wallet_state.creating_wallet = None;
                            info!("Reset creating_wallet state to None for fresh start");
                            model.push_screen(Screen::ModeSelection);
                            // Set focus to ModeSelection component
                            model.ui_state.focus = crate::elm::model::ComponentId::ModeSelection;
                            model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::ModeSelection).or_insert(0);
                            debug!("🎯 Focus set to ModeSelection");
                            None
                        }
                        (1, _) => {
                            // Join Session (always second)
                            info!("Navigating to Join Session");
                            model.push_screen(Screen::JoinSession);
                            // Set focus to JoinSession component
                            model.ui_state.focus = crate::elm::model::ComponentId::JoinSession;
                            model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::JoinSession).or_insert(0);
                            debug!("🎯 Focus set to JoinSession");
                            Some(Command::LoadSessions)
                        }
                        (2, false) => {
                            // Settings (when no wallets)
                            info!("Navigating to Settings");
                            model.push_screen(Screen::Settings);
                            None
                        }
                        (2, true) => {
                            // Manage Wallets (when wallets exist)
                            info!("Navigating to Manage Wallets");
                            model.push_screen(Screen::ManageWallets);
                            Some(Command::LoadWallets)
                        }
                        (3, false) => {
                            // Exit (when no wallets)
                            info!("Exiting application");
                            Some(Command::Quit)
                        }
                        (3, true) => {
                            // Sign Transaction (when wallets exist)
                            if let Some(ref wallet_id) = model.selected_wallet {
                                info!("Navigating to Sign Transaction");
                                model.push_screen(Screen::SignTransaction { wallet_id: wallet_id.clone() });
                                None
                            } else {
                                // Need to select a wallet first
                                info!("Navigating to Manage Wallets for wallet selection");
                                model.push_screen(Screen::ManageWallets);
                                Some(Command::LoadWallets)
                            }
                        }
                        (4, true) => {
                            // Settings (when wallets exist)
                            info!("Navigating to Settings");
                            model.push_screen(Screen::Settings);
                            None
                        }
                        (5, true) => {
                            // Exit (when wallets exist)
                            info!("Exiting application");
                            Some(Command::Quit)
                        }
                        _ => None,
                    }
                }
                Screen::CreateWallet(_) => {
                    debug!("✅ SelectItem on CreateWallet screen");
                    debug!("Current focus: {:?}", model.ui_state.focus);
                    debug!("Selected indices: {:?}", model.ui_state.selected_indices);
                    
                    // Get the current selected index
                    let selected_idx = model.ui_state.selected_indices
                        .get(&model.ui_state.focus)
                        .copied()
                        .unwrap_or(0);
                    
                    info!("✅ CreateWallet item selected: {} (focus: {:?})", selected_idx, model.ui_state.focus);
                    
                    // Handle selection based on current option
                    match selected_idx {
                        0 => {
                            // Option 1: Choose Mode (Online/Offline)
                            info!("Selected: Choose Mode - navigating to mode selection");
                            model.push_screen(Screen::ModeSelection);
                            model.ui_state.focus = crate::elm::model::ComponentId::ModeSelection;
                            model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::ModeSelection).or_insert(0);
                            None
                        }
                        1 => {
                            // Option 2: Configure Threshold
                            info!("Selected: Configure Threshold - navigating to threshold configuration");
                            model.push_screen(Screen::ThresholdConfig);
                            model.ui_state.focus = crate::elm::model::ComponentId::ThresholdConfig;
                            model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::ThresholdConfig).or_insert(0);
                            None
                        }
                        2 => {
                            // Option 3: Start DKG Process (unified multi-chain)
                            info!("Selected: Start DKG Process - initiating unified DKG");

                            // Use the wallet state if available, otherwise use defaults
                            let wallet_state = model.wallet_state.creating_wallet.as_ref();
                            let config = WalletConfig {
                                name: wallet_state
                                    .and_then(|s| s.template.as_ref())
                                    .map(|t| t.name.clone())
                                    .unwrap_or_else(|| "MPC Wallet".to_string()),
                                threshold: wallet_state
                                    .and_then(|s| s.template.as_ref())
                                    .map(|t| t.threshold)
                                    .unwrap_or(2),
                                total_participants: wallet_state
                                    .and_then(|s| s.template.as_ref())
                                    .map(|t| t.total_participants)
                                    .unwrap_or(3),
                                mode: wallet_state
                                    .and_then(|s| s.mode.clone())
                                    .unwrap_or(WalletMode::Online),
                            };
                            Some(Command::SendMessage(Message::CreateWallet { config }))
                        }
                        _ => {
                            debug!("Invalid selection index: {}", selected_idx);
                            None
                        }
                    }
                }
                Screen::ModeSelection => {
                    // Get the current selected mode (0 = Online, 1 = Offline)
                    let selected_mode = model.ui_state.selected_indices
                        .get(&model.ui_state.focus)
                        .copied()
                        .unwrap_or(0);

                    // Block progression if Online mode is chosen without an active
                    // signaling WebSocket — DKG/signing would fail immediately.
                    if selected_mode == 0 && !model.network_state.connected {
                        warn!(
                            "ModeSelection: Online selected but WebSocket is {:?} — blocking submit",
                            model.network_state.connection_status
                        );
                        model.ui_state.notifications.push(Notification {
                            id: Uuid::new_v4().to_string(),
                            text: "Online mode requires an active WebSocket connection. Wait for reconnection or switch to Offline mode.".to_string(),
                            kind: NotificationKind::Warning,
                            timestamp: Utc::now(),
                            dismissible: true,
                        });
                        return None;
                    }

                    info!("ModeSelection confirmed: {}", if selected_mode == 0 { "Online" } else { "Offline" });

                    // Initialize creating_wallet if needed
                    if model.wallet_state.creating_wallet.is_none() {
                        model.wallet_state.creating_wallet = Some(CreateWalletState::default());
                    }

                    // Update the create wallet state with the selected mode
                    if let Some(ref mut state) = model.wallet_state.creating_wallet {
                        state.mode = Some(if selected_mode == 0 {
                            WalletMode::Online
                        } else {
                            WalletMode::Offline
                        });
                    }

                    // Navigate to Threshold Config screen (skip curve - unified DKG handles all)
                    info!("Mode selected, navigating to Threshold Config");
                    model.push_screen(Screen::ThresholdConfig);

                    None
                }
                Screen::ThresholdConfig => {
                    // Get the threshold configuration and start DKG
                    info!("ThresholdConfig confirmed - starting DKG process");
                    
                    // Ensure the chosen config is persisted on the
                    // `creating_wallet` state so `Message::SubmitPassword`
                    // can retrieve it after password capture. The UI code
                    // already writes to `custom_config` on arrow-key
                    // edits; this is a belt-and-suspenders write in case
                    // someone landed here with defaults.
                    if model.wallet_state.creating_wallet.is_none() {
                        model.wallet_state.creating_wallet =
                            Some(CreateWalletState::default());
                    }
                    if let Some(ref mut cw) = model.wallet_state.creating_wallet
                        && cw.custom_config.is_none() {
                            cw.custom_config = Some(WalletConfig {
                                name: "MPC Wallet".to_string(),
                                threshold: 2,
                                total_participants: 3,
                                mode: cw.mode.clone().unwrap_or(WalletMode::Online),
                            });
                        }

                    // Route through the password-capture screen before
                    // starting the DKG. `Message::SubmitPassword` picks
                    // the config back up and dispatches CreateWallet,
                    // which is where session announcement + StartDKG
                    // kick in.
                    info!("ThresholdConfig confirmed — routing to PasswordPrompt");
                    // Creator sets a new password + optional wallet name.
                    model.wallet_state.password_prompt_purpose =
                        crate::elm::model::PasswordPromptPurpose::SetNew;
                    model.wallet_state.wallet_name_draft.clear();
                    model.wallet_state.wallet_name_focus = true;
                    model.push_screen(Screen::PasswordPrompt);
                    model.ui_state.focus = crate::elm::model::ComponentId::PasswordPrompt;
                    None
                }
                Screen::JoinSession => {
                    // Get the selected session index from the JoinSession component
                    let selected_idx = model.ui_state.selected_indices
                        .get(&crate::elm::model::ComponentId::JoinSession)
                        .copied()
                        .unwrap_or(0);
                    
                    let selected_tab = model.ui_state.join_session_tab;
                    info!("JoinSession: Selected session index: {}, tab: {}", selected_idx, 
                          if selected_tab == 0 { "DKG" } else { "Signing" });
                    
                    // Filter sessions by tab type, just like the component does
                    let filtered_sessions: Vec<_> = model.session_invites
                        .iter()
                        .filter(|s| {
                            if selected_tab == 0 {
                                // DKG tab
                                matches!(s.session_type, SessionType::DKG)
                            } else {
                                // Signing tab
                                matches!(s.session_type, SessionType::Signing { .. })
                            }
                        })
                        .cloned()
                        .collect();
                    
                    // Get the session from the filtered list
                    if let Some(session) = filtered_sessions.get(selected_idx).cloned() {
                        info!("Joining DKG session: {}", session.session_id);

                        // Stash the session so `Message::SubmitPassword` can
                        // recognize this as the joiner path and dispatch
                        // `Command::JoinDKG` with the right session_id.
                        model.active_session = Some(session);

                        // Route through the password-capture screen before
                        // actually joining. This ensures the local key-share
                        // encryption password is staged before the DKG
                        // starts producing a KeyPackage we need to persist.
                        // Joiners may also set a local wallet name (label).
                        model.wallet_state.password_prompt_purpose =
                            crate::elm::model::PasswordPromptPurpose::SetNew;
                        model.wallet_state.wallet_name_draft.clear();
                        model.wallet_state.wallet_name_focus = true;
                        model.push_screen(Screen::PasswordPrompt);
                        model.ui_state.focus = crate::elm::model::ComponentId::PasswordPrompt;
                        None
                    } else {
                        warn!("No session available at index {}", selected_idx);
                        None
                    }
                }
                Screen::DKGProgress { .. } => {
                    let selected_action = model.ui_state.selected_indices
                        .get(&crate::elm::model::ComponentId::DKGProgress)
                        .copied()
                        .unwrap_or(0);

                    if selected_action == 0 {
                        // Cancel DKG
                        info!("DKGProgress: Cancel DKG selected");
                        Some(Command::SendMessage(Message::CancelDKG))
                    } else {
                        // Copy Session ID — delegate to the shared
                        // `Message::CopyToClipboard` path so we get the
                        // background-thread handling that keeps the
                        // event loop responsive (see that handler for
                        // the rationale). Previously this was an
                        // inline arboard call and blocked the whole
                        // TUI for up to 47 seconds on X11.
                        if let Some(ref session) = model.active_session {
                            let session_id = session.session_id.clone();
                            info!("DKGProgress: Copy Session ID: {}", session_id);
                            return Some(Command::SendMessage(Message::CopyToClipboard {
                                text: session_id,
                                label: "session ID".to_string(),
                            }));
                        }
                        None
                    }
                }
                Screen::ManageWallets => {
                    // Phase C shortcut: Enter on a wallet row goes
                    // straight into the signing flow instead of
                    // WalletDetail (which is being iterated on
                    // separately). Populate `selected_wallet` so the
                    // SignTransaction screen has a wallet id to target.
                    let selected = model
                        .ui_state
                        .selected_indices
                        .get(&crate::elm::model::ComponentId::WalletList)
                        .copied()
                        .unwrap_or(0);
                    if let Some(wallet) = model.wallet_state.wallets.get(selected) {
                        let wallet_id = wallet.session_id.clone();
                        info!(
                            "ManageWallets SelectItem[{}] → SignTransaction({})",
                            selected, wallet_id
                        );
                        model.selected_wallet = Some(wallet_id.clone());
                        model.wallet_state.clear_sign_draft();
                        model.push_screen(Screen::SignTransaction {
                            wallet_id,
                        });
                        model.ui_state.focus =
                            crate::elm::model::ComponentId::SignTransaction;
                    } else {
                        warn!(
                            "ManageWallets SelectItem[{}] but list only has {} wallets",
                            selected,
                            model.wallet_state.wallets.len()
                        );
                    }
                    None
                }
                _ => None,
            }
        }

        // ============= Modal Management =============
        Message::ShowModal(modal) => {
            model.ui_state.modal = Some(modal);
            None
        }
        
        Message::CloseModal => {
            model.ui_state.modal = None;
            None
        }
        
        Message::ConfirmModal => {
            if let Some(Modal::Confirm { on_confirm, .. }) = &model.ui_state.modal {
                let msg = *on_confirm.clone();
                model.ui_state.modal = None;
                Some(Command::SendMessage(msg))
            } else {
                model.ui_state.modal = None;
                None
            }
        }
        
        Message::CancelModal => {
            if let Some(Modal::Confirm { on_cancel, .. }) = &model.ui_state.modal {
                let msg = *on_cancel.clone();
                model.ui_state.modal = None;
                Some(Command::SendMessage(msg))
            } else {
                model.ui_state.modal = None;
                None
            }
        }
        
        // ============= Notifications =============
        Message::ShowNotification { text, kind } => {
            let notification = Notification {
                id: Uuid::new_v4().to_string(),
                text,
                kind,
                timestamp: Utc::now(),
                dismissible: true,
            };
            
            // Clone the id before moving notification
            let id = notification.id.clone();
            model.ui_state.notifications.push(notification);
            
            // Auto-dismiss after 5 seconds
            Some(Command::ScheduleMessage {
                delay_ms: 5000,
                message: Box::new(Message::ClearNotification { id }),
            })
        }
        
        Message::ClearNotification { id } => {
            model.ui_state.notifications.retain(|n| n.id != id);
            None
        }
        
        // ============= Progress Updates =============
        Message::StartProgress { operation, message } => {
            model.ui_state.progress = Some(ProgressInfo {
                operation,
                progress: 0.0,
                message,
                started_at: Utc::now(),
                estimated_completion: None,
            });
            None
        }
        
        Message::UpdateProgress { progress, message } => {
            if let Some(ref mut info) = model.ui_state.progress {
                info.progress = progress;
                if let Some(msg) = message {
                    info.message = msg;
                }
            }
            None
        }
        
        Message::CompleteProgress => {
            model.ui_state.progress = None;
            None
        }
        
        // ============= System Messages =============
        Message::Initialize => {
            info!("Initializing application");
            
            // Initialize keystore
            let keystore_path = format!("{}/.frost_keystore", 
                std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
            
            Some(Command::InitializeKeystore { 
                path: keystore_path,
                device_id: model.device_id.clone(),
            })
        }
        
        Message::Quit => {
            info!("Quitting application");
            Some(Command::Quit)
        }
        
        Message::Refresh => {
            info!("Refreshing UI");
            Some(Command::RefreshUI)
        }
        
        Message::Error { message } => {
            error!("Error: {}", message);
            model.ui_state.error_message = Some(message.clone());
            
            let notification = Notification {
                id: Uuid::new_v4().to_string(),
                text: message,
                kind: NotificationKind::Error,
                timestamp: Utc::now(),
                dismissible: true,
            };
            model.ui_state.notifications.push(notification);
            
            None
        }
        
        Message::Success { message } => {
            info!("Success: {}", message);
            model.ui_state.success_message = Some(message.clone());
            
            let notification = Notification {
                id: Uuid::new_v4().to_string(),
                text: message,
                kind: NotificationKind::Success,
                timestamp: Utc::now(),
                dismissible: true,
            };
            model.ui_state.notifications.push(notification);
            
            None
        }
        
        // ============= Keystore Events =============
        Message::KeystoreInitialized { path } => {
            info!("Keystore initialized at: {}", path);
            model.wallet_state.keystore_initialized = true;
            model.wallet_state.keystore_path = path;
            
            // Load wallets after initialization
            Some(Command::LoadWallets)
        }
        
        Message::KeystoreError { error } => {
            error!("Keystore error: {}", error);
            Some(Command::SendMessage(Message::Error { 
                message: format!("Keystore error: {}", error) 
            }))
        }
        
        // ============= Session Discovery Events =============
        Message::SessionsLoaded { sessions } => {
            info!("Loaded {} sessions from discovery", sessions.len());
            // Store the discovered sessions
            model.session_invites = sessions.clone();

            // Log session details for debugging
            for session in &sessions {
                info!("Session discovered: {} ({}/{})", session.session_id, session.threshold, session.total);
            }

            // Force UI update if we're on the JoinSession screen
            if matches!(model.current_screen, Screen::JoinSession) {
                info!("On JoinSession screen, forcing remount to update session list");
                Some(Command::SendMessage(Message::ForceRemount))
            } else {
                None
            }
        }

        Message::SessionDiscovered { session } => {
            // Merge-update: replace the existing entry for this session_id if present,
            // otherwise append. This lets us pick up live `SessionAvailable` broadcasts
            // while `LoadSessions` is also running its bulk refresh.
            info!(
                "Session discovered via push: {} ({}/{})",
                session.session_id, session.threshold, session.total
            );

            // Signing-request push notification (Stage 1 of the signing-
            // network flow). If this is a Signing session *for us* — we're
            // listed as a participant, we're not the proposer, we're not
            // already on the JoinSession screen, and there's no modal
            // already commanding the user's attention — pop a
            // `Modal::Confirm` asking if they want to review it. This
            // closes the gap where co-signers on other screens (MainMenu,
            // ManageWallets, idle) would miss a signing request entirely
            // and the creator would time out waiting.
            let is_signing = matches!(session.session_type, SessionType::Signing { .. });
            let am_participant = session.participants.contains(&model.device_id);
            let am_proposer = session.proposer_id == model.device_id;
            let can_interrupt = !matches!(model.current_screen, Screen::JoinSession)
                && model.ui_state.modal.is_none();
            let should_notify =
                is_signing && am_participant && !am_proposer && can_interrupt;

            if let Some(slot) = model
                .session_invites
                .iter_mut()
                .find(|s| s.session_id == session.session_id)
            {
                *slot = session.clone();
            } else {
                model.session_invites.push(session.clone());
            }

            if should_notify {
                // Preview the message the creator is asking us to sign.
                // Show as UTF-8 when valid + printable, otherwise hex.
                // Note: for EIP-191 signatures the payload is already the
                // 32-byte keccak hash, which won't be valid UTF-8 — that's
                // fine, we fall through to hex.
                let preview = match session.signing_message_hex.as_ref() {
                    Some(hex_str) => match hex::decode(hex_str) {
                        Ok(bytes) => match std::str::from_utf8(&bytes) {
                            Ok(s) if s.chars().all(|c| !c.is_control() || c == '\n') => {
                                let truncated: String = s.chars().take(80).collect();
                                if truncated.chars().count() < s.chars().count() {
                                    format!("\"{}…\"", truncated)
                                } else {
                                    format!("\"{}\"", s)
                                }
                            }
                            _ => {
                                if hex_str.len() > 32 {
                                    format!("0x{}…", &hex_str[..32])
                                } else {
                                    format!("0x{}", hex_str)
                                }
                            }
                        },
                        Err(_) => format!("<invalid hex> {}", hex_str),
                    },
                    None => "<no payload>".to_string(),
                };
                let wallet_name = if let SessionType::Signing { wallet_name, .. } = &session.session_type {
                    wallet_name.clone()
                } else {
                    "wallet".to_string()
                };
                let message_body = format!(
                    "From {proposer}\nWallet: {wallet}\nThreshold: {k}-of-{n}\n\nMessage: {preview}\n\nReview and sign?",
                    proposer = session.proposer_id,
                    wallet = wallet_name,
                    k = session.threshold,
                    n = session.total,
                );
                model.ui_state.modal = Some(Modal::Confirm {
                    title: "📝 Signing Request".to_string(),
                    message: message_body,
                    on_confirm: Box::new(Message::ReviewSigningRequest {
                        session_id: session.session_id.clone(),
                    }),
                    on_cancel: Box::new(Message::DeclineSigningRequest {
                        session_id: session.session_id,
                    }),
                });
            }

            if matches!(model.current_screen, Screen::JoinSession) {
                Some(Command::SendMessage(Message::ForceRemount))
            } else {
                None
            }
        }

        Message::ReviewSigningRequest { session_id } => {
            // Co-signer accepted the push notification. Jump them to
            // JoinSession on the Signing tab with this session highlighted
            // so they can confirm the exact payload before unlocking.
            let signing_index = model
                .session_invites
                .iter()
                .filter(|s| matches!(s.session_type, SessionType::Signing { .. }))
                .position(|s| s.session_id == session_id);
            if signing_index.is_none() {
                warn!(
                    "ReviewSigningRequest: session {} no longer in invites; dropping",
                    session_id
                );
                return None;
            }
            info!(
                "ReviewSigningRequest: navigating to JoinSession (Signing tab, idx {})",
                signing_index.unwrap_or(0)
            );
            model.ui_state.modal = None;
            model.ui_state.join_session_tab = 1; // Signing tab
            model
                .ui_state
                .selected_indices
                .insert(
                    crate::elm::model::ComponentId::JoinSession,
                    signing_index.unwrap_or(0),
                );
            model.push_screen(Screen::JoinSession);
            model.ui_state.focus = crate::elm::model::ComponentId::JoinSession;
            None
        }

        Message::DeclineSigningRequest { session_id } => {
            // Co-signer clicked Cancel on the push-notification modal.
            // Drop the session locally + confirm the decline with a
            // toast. The creator won't learn about this automatically
            // yet — wire-level decline propagation is a follow-up. For
            // now the local ledger is correct and the user won't keep
            // getting re-prompted about a request they've already said
            // no to.
            let existed = model
                .session_invites
                .iter()
                .any(|s| s.session_id == session_id);
            model.session_invites.retain(|s| s.session_id != session_id);
            model.ui_state.modal = None;
            if existed {
                let notification = Notification {
                    id: Uuid::new_v4().to_string(),
                    text: format!("Declined signing request {}", short_session_id(&session_id)),
                    kind: NotificationKind::Info,
                    timestamp: Utc::now(),
                    dismissible: true,
                };
                model.ui_state.notifications.push(notification);
            }
            None
        }

        Message::RemoveSession { session_id } => {
            let before = model.session_invites.len();
            model.session_invites.retain(|s| s.session_id != session_id);
            if model.session_invites.len() != before {
                info!("Session removed: {}", session_id);
                if matches!(model.current_screen, Screen::JoinSession) {
                    return Some(Command::SendMessage(Message::ForceRemount));
                }
            }
            None
        }
        
        // ============= Default =============
        _ => {
            debug!("Unhandled message: {:?}", msg);
            None
        }
    }
}

/// Truncate a session_id for inline display — full UUIDs are 36
/// chars and dominate narrow notification lines. Keep the prefix so
/// ops-log cross-referencing still works.
fn short_session_id(id: &str) -> String {
    if id.len() > 16 {
        format!("{}…", &id[..16])
    } else {
        id.to_string()
    }
}

/// Render the multi-line body for the creator-side Confirm-Signing
/// modal. Shows the user-typed message (UTF-8 preview), the bytes
/// FROST will actually sign (EIP-191 hash for secp256k1, same as raw
/// for ed25519), and the wallet's threshold so the user knows how
/// many co-signers are being recruited.
fn preview_lines(
    wallet_id: &str,
    curve: &str,
    bytes_to_sign: &[u8],
    raw_message: Option<&[u8]>,
    wallets: &[crate::keystore::WalletMetadata],
) -> String {
    let (threshold, total) = wallets
        .iter()
        .find(|w| w.session_id == wallet_id)
        .map(|w| (w.threshold, w.total_participants))
        .unwrap_or((0, 0));

    let user_message_line = match raw_message {
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(s) if s.chars().all(|c| !c.is_control() || c == '\n') => {
                let truncated: String = s.chars().take(80).collect();
                if truncated.chars().count() < s.chars().count() {
                    format!("Message: \"{}…\"", truncated)
                } else {
                    format!("Message: \"{}\"", s)
                }
            }
            _ => format!("Message bytes ({}): 0x{}", bytes.len(), hex::encode(bytes)),
        },
        // ed25519 / raw-bytes signing: what FROST signs IS what the
        // user typed. Show a single line instead of two identical ones.
        None => match std::str::from_utf8(bytes_to_sign) {
            Ok(s) if s.chars().all(|c| !c.is_control() || c == '\n') => {
                format!("Message: \"{}\"", s)
            }
            _ => format!("Bytes ({}): 0x{}", bytes_to_sign.len(), hex::encode(bytes_to_sign)),
        },
    };

    let hash_line = if curve == "secp256k1" {
        // For secp256k1 we sign the EIP-191 hash; the user should see
        // what hash will actually go on-chain via ecrecover.
        format!("Hash (EIP-191): 0x{}", hex::encode(bytes_to_sign))
    } else {
        String::new()
    };

    let threshold_line = if total > 0 {
        format!("Threshold: {}-of-{}", threshold, total)
    } else {
        String::new()
    };

    let mut lines: Vec<String> = Vec::with_capacity(7);
    lines.push(format!("Wallet: {}", wallet_id));
    if !threshold_line.is_empty() {
        lines.push(threshold_line);
    }
    lines.push(String::new()); // spacer
    lines.push(user_message_line);
    if !hash_line.is_empty() {
        lines.push(hash_line);
    }
    lines.push(String::new());
    lines.push("Broadcast this signing request to the network?".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crossterm::event::KeyEvent;
    
    #[test]
    fn test_navigate_back() {
        let mut model = Model::new("test".to_string());
        model.current_screen = Screen::MainMenu;
        
        // Navigate to wallet list
        update(&mut model, Message::Navigate(Screen::ManageWallets));
        assert_eq!(model.current_screen, Screen::ManageWallets);
        assert_eq!(model.navigation_stack.len(), 1);
        
        // Navigate back
        update(&mut model, Message::NavigateBack);
        assert_eq!(model.current_screen, Screen::MainMenu);
        assert_eq!(model.navigation_stack.len(), 0);
    }
    
    #[test]
    fn test_esc_key_never_exits() {
        let mut model = Model::new("test".to_string());
        model.current_screen = Screen::ManageWallets;
        model.navigation_stack.push(Screen::MainMenu);
        
        // Press Esc
        let cmd = update(&mut model, Message::KeyPressed(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }));
        
        // Should return NavigateBack command, never quit
        assert!(matches!(cmd, Some(Command::SendMessage(Message::NavigateBack))));
    }
    
    #[test]
    fn password_submit_set_new_requires_min_length() {
        let mut model = Model::new("test".to_string());
        model.wallet_state.password_prompt_purpose =
            crate::elm::model::PasswordPromptPurpose::SetNew;
        model.wallet_state.password_draft = "short".to_string();
        model.wallet_state.confirm_draft = "short".to_string();

        let cmd = update(&mut model, Message::PasswordSubmitDraft);

        assert!(cmd.is_none(), "short password must not dispatch SubmitPassword");
        assert!(
            model
                .wallet_state
                .password_error
                .as_deref()
                .unwrap_or("")
                .contains("at least"),
            "expected min-length error, got {:?}",
            model.wallet_state.password_error,
        );
        // Drafts preserved so the user keeps what they typed.
        assert_eq!(model.wallet_state.password_draft, "short");
    }

    #[test]
    fn password_submit_set_new_requires_confirm_match() {
        let mut model = Model::new("test".to_string());
        model.wallet_state.password_prompt_purpose =
            crate::elm::model::PasswordPromptPurpose::SetNew;
        model.wallet_state.password_draft = "longenough1".to_string();
        model.wallet_state.confirm_draft = "differs!!!!".to_string();

        let cmd = update(&mut model, Message::PasswordSubmitDraft);

        assert!(cmd.is_none(), "mismatched confirm must not dispatch");
        assert_eq!(
            model.wallet_state.password_error.as_deref(),
            Some("Confirm does not match password"),
        );
    }

    #[test]
    fn password_submit_unlock_skips_confirm_match() {
        // Reproduces the reported bug: cold-start sign lands on
        // PasswordPrompt with only the Password field rendered. Without
        // the purpose discriminator the validator demanded confirm == pw
        // and silently rejected the submit.
        let mut model = Model::new("test".to_string());
        model.wallet_state.password_prompt_purpose =
            crate::elm::model::PasswordPromptPurpose::Unlock;
        model.wallet_state.password_draft = "hunter2".to_string();
        model.wallet_state.confirm_draft = String::new();

        let cmd = update(&mut model, Message::PasswordSubmitDraft);

        match cmd {
            Some(Command::SendMessage(Message::SubmitPassword { value })) => {
                assert_eq!(value, "hunter2");
            }
            other => panic!(
                "expected SubmitPassword command in Unlock mode, got {:?}",
                other
            ),
        }
        assert!(model.wallet_state.password_error.is_none());
        // Drafts cleared on successful handoff so cleartext doesn't
        // outlive the screen.
        assert!(model.wallet_state.password_draft.is_empty());
        assert!(model.wallet_state.confirm_draft.is_empty());
    }

    #[test]
    fn password_submit_unlock_rejects_empty() {
        let mut model = Model::new("test".to_string());
        model.wallet_state.password_prompt_purpose =
            crate::elm::model::PasswordPromptPurpose::Unlock;
        model.wallet_state.password_draft = String::new();

        let cmd = update(&mut model, Message::PasswordSubmitDraft);

        assert!(cmd.is_none());
        assert_eq!(
            model.wallet_state.password_error.as_deref(),
            Some("Enter the wallet password"),
        );
    }

    #[test]
    fn headless_create_wallet_seeds_state_and_hands_off() {
        let mut model = Model::new("dev".to_string());
        let config = WalletConfig {
            name: "Treasury".to_string(),
            total_participants: 3,
            threshold: 2,
            mode: WalletMode::Online,
        };
        let cmd = update(
            &mut model,
            Message::HeadlessCreateWallet {
                config,
                password: "hunter2hunter2".to_string(),
                label: "Treasury".to_string(),
            },
        );
        // Seeds the same state the interactive ThresholdConfig screen would.
        assert!(model.wallet_state.creating_wallet.is_some());
        assert_eq!(model.wallet_state.wallet_name_draft, "Treasury");
        // Hands off to the creator SubmitPassword path.
        match cmd {
            Some(Command::SendMessage(Message::SubmitPassword { value })) => {
                assert_eq!(value, "hunter2hunter2");
            }
            other => panic!("expected SendMessage(SubmitPassword), got {:?}", other),
        }
    }

    #[test]
    fn headless_sign_seeds_pending_state_and_hands_off() {
        let mut model = Model::new("dev".to_string());
        let cmd = update(
            &mut model,
            Message::HeadlessSign {
                wallet_id: "wallet-ab12".to_string(),
                message: "hello".to_string(),
                encoding: "utf8".to_string(),
                password: "pw".to_string(),
            },
        );
        assert!(model.wallet_state.pending_sign_message.is_some());
        assert_eq!(
            model.wallet_state.pending_sign_wallet_id.as_deref(),
            Some("wallet-ab12")
        );
        assert!(model.wallet_state.pending_sign_session_id.is_none());
        match cmd {
            Some(Command::SendMessage(Message::SubmitPassword { value })) => {
                assert_eq!(value, "pw");
            }
            other => panic!("expected SendMessage(SubmitPassword), got {:?}", other),
        }
    }

    #[test]
    fn headless_join_unknown_session_is_noop() {
        let mut model = Model::new("dev".to_string());
        // No discovered invites → join is a no-op (no command, no active session).
        let cmd = update(
            &mut model,
            Message::HeadlessJoinSession {
                session_id: "nope".to_string(),
                password: "pw".to_string(),
                label: String::new(),
            },
        );
        assert!(cmd.is_none());
        assert!(model.active_session.is_none());
    }

    #[test]
    fn password_toggle_field_is_no_op_in_unlock() {
        let mut model = Model::new("test".to_string());
        model.wallet_state.password_prompt_purpose =
            crate::elm::model::PasswordPromptPurpose::Unlock;
        model.wallet_state.password_focus_confirm = false;

        update(&mut model, Message::PasswordToggleField);

        assert!(
            !model.wallet_state.password_focus_confirm,
            "Tab in Unlock mode must not move focus — there is no second field"
        );
    }

    #[test]
    fn password_toggle_field_still_toggles_in_set_new() {
        let mut model = Model::new("test".to_string());
        model.wallet_state.password_prompt_purpose =
            crate::elm::model::PasswordPromptPurpose::SetNew;
        model.wallet_state.password_focus_confirm = false;

        update(&mut model, Message::PasswordToggleField);

        assert!(model.wallet_state.password_focus_confirm);
    }

    #[test]
    fn clear_password_draft_resets_purpose_to_default() {
        let mut model = Model::new("test".to_string());
        model.wallet_state.password_prompt_purpose =
            crate::elm::model::PasswordPromptPurpose::Unlock;
        model.wallet_state.password_draft = "x".into();

        model.wallet_state.clear_password_draft();

        assert_eq!(
            model.wallet_state.password_prompt_purpose,
            crate::elm::model::PasswordPromptPurpose::SetNew,
            "exiting the screen must reset purpose so the next push starts fresh"
        );
        assert!(model.wallet_state.password_draft.is_empty());
    }

    #[test]
    fn test_modal_closes_on_esc() {
        let mut model = Model::new("test".to_string());
        model.ui_state.modal = Some(Modal::Error {
            title: "Test".to_string(),
            message: "Test error".to_string(),
        });
        
        // Press Esc with modal open
        let cmd = update(&mut model, Message::KeyPressed(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }));
        
        // Modal should be closed, no navigation
        assert!(model.ui_state.modal.is_none());
        assert!(cmd.is_none());
    }
}