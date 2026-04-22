//! Update - The state transition function
//!
//! The update function is the heart of the Elm Architecture. It takes the current
//! model and a message, and returns an updated model along with optional commands
//! to execute side effects.

use crate::elm::model::{Model, Screen, Modal, Notification, NotificationKind, ConnectionStatus, Operation, ProgressInfo, WalletConfig, WalletMode, CreateWalletState};
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
            // frame.
            model.wallet_state.last_finalized_wallet = None;
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
            if model.wallet_state.password_focus_confirm {
                model.wallet_state.confirm_draft.push(c);
            } else {
                model.wallet_state.password_draft.push(c);
            }
            None
        }

        Message::PasswordBackspace => {
            model.wallet_state.password_error = None;
            if model.wallet_state.password_focus_confirm {
                model.wallet_state.confirm_draft.pop();
            } else {
                model.wallet_state.password_draft.pop();
            }
            None
        }

        Message::PasswordToggleField => {
            model.wallet_state.password_focus_confirm =
                !model.wallet_state.password_focus_confirm;
            None
        }

        Message::PasswordSubmitDraft => {
            // Validation lives here (not in the component) so the rules are
            // exercised by the same test harness as every other state
            // transition and can't silently drift if the component's
            // `on()` is ever re-enabled.
            const MIN_PW_LEN: usize = 8;
            let pw = model.wallet_state.password_draft.clone();
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

            // Valid: wipe the drafts immediately so the cleartext doesn't
            // outlive the handoff. `SubmitPassword` stashes it on
            // `pending_password` and drives the DKG flow forward.
            model.wallet_state.password_draft.clear();
            model.wallet_state.confirm_draft.clear();
            model.wallet_state.password_error = None;
            model.wallet_state.password_focus_confirm = false;
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
        // Stage 2 will introduce the actual keystore-write step that
        // reads `pending_password` and clears it after encryption.
        Message::SubmitPassword { value } => {
            info!(
                "Password submitted ({} chars) — routing to DKG",
                value.len()
            );
            model.wallet_state.pending_password = Some(value);

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
                        Some(Command::JoinDKG { session_id })
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
            // `curve_type` comes from the Model's boot-time snapshot of
            // `C::curve_type()` — no more "unified" placeholder.
            model.active_session = Some(SessionInfo {
                session_id: temp_session_id.clone(),
                proposer_id: model.device_id.clone(),
                total: config.total_participants,
                threshold: config.threshold,
                participants: participants.clone(),
                session_type: SessionType::DKG,
                curve_type: model.wallet_state.curve_type.to_string(),
                coordination_type: "online".to_string(),
                signing_message_hex: None,
            });
            
            // Navigate to DKG Progress screen with placeholder
            model.push_screen(Screen::DKGProgress { session_id: temp_session_id.clone() });

            // Set focus for DKGProgress screen
            model.ui_state.focus = crate::elm::model::ComponentId::DKGProgress;
            model.ui_state.selected_indices.entry(crate::elm::model::ComponentId::DKGProgress).or_insert(0);
            
            // Add to pending operations
            model.pending_operations.push(Operation::CreateWallet(config.clone()));
            
            // Start DKG process - this will generate the real session ID
            Some(Command::StartDKG { config })
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
            Some(Command::ProcessSigningRound2 {
                from_device,
                share_bytes,
            })
        }

        // Dispatched by the SignTransaction screen (C.3) once the user
        // confirms a message to sign. We assume the wallet is already
        // unlocked — UnlockWallet is dispatched upstream. Kick off the
        // ceremony.
        Message::InitiateSigning { request } => {
            info!(
                "Initiating signing for wallet '{}' ({} bytes of transaction data)",
                request.wallet_id,
                request.transaction_data.len()
            );
            Some(Command::StartSigning { request })
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
            // Surface as an inline notification for now (C.5 adds the
            // proper SignatureComplete screen).
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

            let message_bytes = model.wallet_state.sign_message_draft.as_bytes().to_vec();
            // Clear immediately so the sign button can't be re-triggered
            // for the same message by accident.
            model.wallet_state.clear_sign_draft();

            // Curve and chain_id fields aren't used by the protocol layer
            // but fill them in honestly for logs/future use.
            let request = crate::elm::message::SigningRequest {
                wallet_id,
                transaction_data: message_bytes,
                chain: model.wallet_state.curve_type.to_string(),
                metadata: None,
            };
            Some(Command::SendMessage(Message::InitiateSigning { request }))
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

            if let (Some(msg), Some(sid), Some(wid)) =
                (pending_msg, pending_sid, pending_wallet)
            {
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
                // SigningProgress screen: for Phase C we reuse
                // DKGProgress's mount route (the component renders
                // participant mesh status, which is exactly what we
                // want to show during signing too). A dedicated
                // component is a polish item for Phase D.
                model.push_screen(Screen::SigningProgress {
                    request_id: sid.clone(),
                });
                return Some(Command::JoinSigning {
                    session_id: sid,
                    message_bytes: msg,
                });
            }

            None
        }

        Message::WalletUnlockFailed { error } => {
            // Surface as a modal so the user has to acknowledge before
            // continuing — signing with a stale/missing KeyPackage would
            // produce nonsense, so we must block the flow until the user
            // either retries or navigates away.
            error!("Wallet unlock failed: {}", error);
            model.ui_state.modal = Some(Modal::Error {
                title: "Unlock Failed".to_string(),
                message: error,
            });
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
                on_confirm: Box::new(Message::WalletDeleted { wallet_id: wallet_id.clone() }),
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
                *session_id = real_session_id.clone();
            }
            
            // Force a remount to update the display
            Some(Command::SendMessage(Message::ForceRemount))
        }
        
        Message::UpdateParticipants { participants } => {
            info!("Updating participants list: {:?}", participants);
            
            // Update the active session with the current participants
            if let Some(ref mut session) = model.active_session {
                session.participants = participants.clone();
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
                .entry(device_id.clone())
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
                            .map_or(false, |(_, data_channel_open)| *data_channel_open)
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
                message: error.clone(),
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
            Some(Command::Batch(vec![
                Command::StartFrostProtocol,
                Command::SendMessage(Message::ForceRemount),
            ]))
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
            Some(Command::Batch(vec![
                Command::StartFrostProtocol,
                Command::SendMessage(Message::ForceRemount),
            ]))
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
                // signing coordination later. Same formula as `dkg.rs:661`
                // uses to seed `current_wallet_id`.
                let wallet_name = model
                    .active_session
                    .as_ref()
                    .map(|s| format!("wallet-{}", &s.session_id[..8.min(s.session_id.len())]));

                match (password, wallet_name) {
                    (Some(password), Some(wallet_name)) if !keystore_path.is_empty() => {
                        info!(
                            "Auto-dispatching FinalizeWalletFromDkg (keystore={}, name={})",
                            keystore_path, wallet_name
                        );
                        Some(Command::FinalizeWalletFromDkg {
                            password,
                            keystore_path,
                            wallet_name,
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
                    group_pubkey_hex: group_pubkey_hex.clone(),
                    curve_type: curve_type.clone(),
                    addresses: addresses.clone(),
                });

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
                wallet_id: wallet_id.clone(),
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
                            .map_or(false, |(_, data_channel_open)| *data_channel_open)
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
                    if let Some(ref mut cw) = model.wallet_state.creating_wallet {
                        if cw.custom_config.is_none() {
                            cw.custom_config = Some(WalletConfig {
                                name: "MPC Wallet".to_string(),
                                threshold: 2,
                                total_participants: 3,
                                mode: cw.mode.clone().unwrap_or(WalletMode::Online),
                            });
                        }
                    }

                    // Route through the password-capture screen before
                    // starting the DKG. `Message::SubmitPassword` picks
                    // the config back up and dispatches CreateWallet,
                    // which is where session announcement + StartDKG
                    // kick in.
                    info!("ThresholdConfig confirmed — routing to PasswordPrompt");
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
                        // Copy Session ID — actually place the string on the
                        // system clipboard (not just a toast), so the user can
                        // paste it into another TUI / chat client to invite
                        // the other participants. Previously this was a
                        // notification-only stub and the user saw nothing
                        // happen in their clipboard.
                        if let Some(ref session) = model.active_session {
                            let session_id = session.session_id.clone();
                            info!("DKGProgress: Copy Session ID: {}", session_id);
                            let (kind, text) = match arboard::Clipboard::new()
                                .and_then(|mut c| c.set_text(session_id.clone()))
                            {
                                Ok(()) => (
                                    NotificationKind::Success,
                                    format!("📋 Copied Session ID to clipboard: {}", session_id),
                                ),
                                Err(e) => {
                                    warn!("Clipboard copy failed: {}", e);
                                    (
                                        NotificationKind::Warning,
                                        format!(
                                            "Couldn't access clipboard ({}). Session ID: {}",
                                            e, session_id
                                        ),
                                    )
                                }
                            };
                            model.ui_state.notifications.push(Notification {
                                id: Uuid::new_v4().to_string(),
                                text,
                                kind,
                                timestamp: Utc::now(),
                                dismissible: true,
                            });
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
            if let Some(slot) = model
                .session_invites
                .iter_mut()
                .find(|s| s.session_id == session.session_id)
            {
                *slot = session;
            } else {
                model.session_invites.push(session);
            }

            if matches!(model.current_screen, Screen::JoinSession) {
                Some(Command::SendMessage(Message::ForceRemount))
            } else {
                None
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elm::model::WalletMode;
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