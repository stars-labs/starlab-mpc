//! ElmApp - Main application using tui-realm with Elm Architecture
//!
//! This is the main application that brings together the Model, Update, View, and Commands
//! to create a fully functional TUI application following the Elm Architecture pattern.

use crate::elm::model::{Model, Screen};
use crate::elm::message::Message;
use crate::elm::update::update;
use crate::elm::components::{Id, MainMenu, WalletList, WalletDetail, ModalComponent, NotificationBar};
use crate::utils::appstate_compat::AppState;

use tuirealm::application::Application;
use tuirealm::listener::EventListenerCfg;
// `TerminalBridge` was removed in tuirealm 4.0; use the adapter directly —
// its `TerminalAdapter` impl exposes raw-mode / alt-screen / draw methods.
use tuirealm::terminal::{CrosstermTerminalAdapter, TerminalAdapter};
use ratatui::layout::{Constraint, Direction, Layout};
use crossterm::event::Event as CrosstermEvent;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, debug, error};

/// The main Elm application
pub struct ElmApp<C: frost_core::Ciphersuite> {
    /// The application model (state)
    model: Model,
    
    /// The tui-realm application
    app: Application<Id, Message, crate::elm::components::UserEvent>,
    
    /// Terminal adapter for rendering (tuirealm 4.0 removed TerminalBridge)
    terminal: CrosstermTerminalAdapter,
    
    /// Channel for sending messages
    message_tx: UnboundedSender<Message>,
    
    /// Channel for receiving messages
    message_rx: UnboundedReceiver<Message>,
    
    /// Reference to the shared app state (for compatibility with existing code)
    app_state: Arc<Mutex<AppState<C>>>,
    
    /// Whether the app should quit
    should_quit: bool,
}

impl<C: frost_core::Ciphersuite + Send + Sync + 'static> ElmApp<C>
where
    <<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Element: Send + Sync,
    <<<C as frost_core::Ciphersuite>::Group as frost_core::Group>::Field as frost_core::Field>::Scalar: Send + Sync,
    // Required transitively by `Command::execute`, which feeds
    // `process_dkg_round2` — that's the single call site that needs the
    // real curve name for address derivation.
    C: crate::utils::curve_traits::CurveIdentifier,
{
    /// Create a new Elm application
    pub fn new(
        device_id: String,
        app_state: Arc<Mutex<AppState<C>>>,
    ) -> anyhow::Result<Self> {
        // Create message channels
        let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        
        // Initialize model. We seed `wallet_state.curve_type` here because
        // `update()` is plain-data (no generic `C`) — every update-layer
        // site that used to emit `"unified"` now reads this instead, which
        // keeps the session's stored curve in sync with the ciphersuite
        // actually running.
        let mut model = Model::new(device_id);
        model.wallet_state.curve_type =
            <C as crate::utils::curve_traits::CurveIdentifier>::curve_type();
        
        // Initialize terminal (adapter directly, no bridge in tuirealm 4.0)
        let mut terminal = CrosstermTerminalAdapter::new()?;
        terminal.enable_raw_mode()?;
        terminal.enter_alternate_screen()?;
        
        // Initialize tui-realm application
        let app = Application::init(
            EventListenerCfg::default()
        );
        
        let mut elm_app = Self {
            model,
            app,
            terminal,
            message_tx: message_tx.clone(),
            message_rx,
            app_state,
            should_quit: false,
        };
        
        // Mount initial components
        elm_app.mount_components()?;
        
        // Send initialization message
        let _ = message_tx.send(Message::Initialize);
        
        Ok(elm_app)
    }
    
    /// Mount components based on current screen
    fn mount_components(&mut self) -> anyhow::Result<()> {
        debug!("🔧 Mounting components for screen: {:?}", self.model.current_screen);
        
        // Log state before mounting
        if matches!(self.model.current_screen, Screen::ThresholdConfig) {
            let selected = self.model.ui_state.selected_indices
                .get(&crate::elm::model::ComponentId::ThresholdConfig)
                .copied()
                .unwrap_or(0);
            info!("🔄 PRE-MOUNT: ThresholdConfig selected_field in model = {}", selected);
        }
        
        // Clear all components first
        self.app.umount_all();
        
        // Mount components based on current screen
        match self.model.current_screen {
            Screen::Welcome | Screen::MainMenu => {
                // Create main menu with actual wallet count
                let wallet_count = self.model.wallet_state.wallets.len();
                let mut main_menu = MainMenu::with_wallet_count(wallet_count);
                
                // Set the selected index from the model
                let selected = self.model.ui_state.selected_indices
                    .get(&crate::elm::model::ComponentId::MainMenu)
                    .copied()
                    .unwrap_or(0);
                    
                debug!("Setting MainMenu selected index to: {}, wallet count: {}", selected, wallet_count);
                main_menu.set_selected(selected);
                
                self.app.mount(
                    Id::MainMenu,
                    Box::new(main_menu),
                    vec![]
                )?;
                self.app.active(&Id::MainMenu)?;
            }
            Screen::ManageWallets => {
                let mut wallet_list = WalletList::new();
                wallet_list.set_wallets(self.model.wallet_state.wallets.clone());
                // Re-apply the row selection from model state so
                // ScrollUp/ScrollDown mutations (which update
                // `selected_indices[WalletList]`) are reflected after
                // the remount triggered by those same messages.
                let selected = self
                    .model
                    .ui_state
                    .selected_indices
                    .get(&crate::elm::model::ComponentId::WalletList)
                    .copied()
                    .unwrap_or(0);
                wallet_list.set_selected(selected);

                self.app.remount(
                    Id::WalletList,
                    Box::new(wallet_list),
                    vec![]
                )?;
                self.app.active(&Id::WalletList)?;
            }
            Screen::WalletDetail { .. } => {
                self.app.mount(
                    Id::WalletDetail,
                    Box::new(WalletDetail::default()),
                    vec![]
                )?;
                self.app.active(&Id::WalletDetail)?;
            }
            Screen::CreateWallet(_) => {
                info!("🔨 Mounting CreateWallet component with state: {:?}", 
                     self.model.wallet_state.creating_wallet);
                
                // Pass the wallet state to the component
                let mut create_wallet = crate::elm::components::CreateWalletComponent::with_state(
                    self.model.wallet_state.creating_wallet.clone()
                );
                
                // Set the selected index from the model
                let selected = self.model.ui_state.selected_indices
                    .get(&crate::elm::model::ComponentId::CreateWallet)
                    .copied()
                    .unwrap_or(0);
                    
                debug!("🔧 Mounting CreateWallet component:");
                debug!("   - Model focus: {:?}", self.model.ui_state.focus);
                debug!("   - Model selected indices: {:?}", self.model.ui_state.selected_indices);
                debug!("   - Setting component selected index to: {}", selected);
                debug!("   - Wallet state: {:?}", self.model.wallet_state.creating_wallet);
                
                create_wallet.set_selected(selected);
                
                self.app.mount(
                    Id::CreateWallet,
                    Box::new(create_wallet),
                    vec![]
                )?;
                self.app.active(&Id::CreateWallet)?;
                
                debug!("✅ CreateWallet component mounted and activated");
            }
            Screen::ModeSelection => {
                debug!("🔧 Mounting ModeSelection component");
                // Get the selected index for ModeSelection
                let selected = self.model.ui_state.selected_indices
                    .get(&self.model.ui_state.focus)
                    .cloned()
                    .unwrap_or(0);
                debug!("ModeSelection selected index: {}", selected);
                let mut mode_selection =
                    crate::elm::components::ModeSelectionComponent::with_selected(selected);
                mode_selection.set_websocket_connected(self.model.network_state.connected);
                mode_selection.set_websocket_url(self.model.network_state.websocket_url.clone());
                self.app.mount(
                    Id::ModeSelection,
                    Box::new(mode_selection),
                    vec![]
                )?;
                self.app.active(&Id::ModeSelection)?;
            }
            Screen::ThresholdConfig => {
                debug!("🔧 Mounting ThresholdConfig component");
                
                // ALWAYS get the selected field from the correct place
                let selected_field = self.model.ui_state.selected_indices
                    .get(&crate::elm::model::ComponentId::ThresholdConfig)
                    .copied()
                    .unwrap_or(0);
                    
                info!("🎯 ThresholdConfig selected_field from model: {}", selected_field);
                
                // Get values from model if available
                let (participants, threshold) = if let Some(ref creating_wallet) = self.model.wallet_state.creating_wallet {
                    if let Some(ref config) = creating_wallet.custom_config {
                        debug!("Using custom_config values");
                        (config.total_participants, config.threshold)
                    } else {
                        debug!("ThresholdConfig mounting with default values (no custom_config)");
                        (3, 2) // Default values
                    }
                } else {
                    debug!("ThresholdConfig mounting with default values (no creating_wallet)");
                    (3, 2) // Default values
                };
                
                info!("🎯 FINAL: Mounting ThresholdConfig with participants={}, threshold={}, selected_field={}", 
                     participants, threshold, selected_field);
                
                // First unmount if already mounted to force recreation
                if self.app.mounted(&Id::ThresholdConfig) {
                    debug!("Unmounting existing ThresholdConfig component first");
                    let _ = self.app.umount(&Id::ThresholdConfig);
                }
                
                self.app.mount(
                    Id::ThresholdConfig,
                    Box::new(crate::elm::components::ThresholdConfigComponent::with_values(
                        participants, threshold, selected_field
                    )),
                    vec![]
                )?;
                self.app.active(&Id::ThresholdConfig)?;
            }
            Screen::JoinSession => {
                debug!("🔧 Mounting JoinSession component");
                
                // Create component and update it with real sessions from model
                let mut component = crate::elm::components::JoinSessionComponent::new();
                
                // Convert model sessions to UI format
                let ui_sessions: Vec<crate::elm::components::join_session::SessionInfo> = self.model.session_invites
                    .iter()
                    .map(|s| {
                        use crate::elm::components::join_session::{SessionInfo, SessionStatus, SessionType};
                        SessionInfo {
                            id: s.session_id.clone(),
                            session_type: match s.session_type {
                                crate::protocal::signal::SessionType::DKG => SessionType::DKG,
                                crate::protocal::signal::SessionType::Signing { .. } => SessionType::Signing,
                            },
                            creator: s.proposer_id.clone(),
                            status: SessionStatus::Waiting,
                            participants: s.participants.clone(),
                            required: s.total as usize,
                            joined: s.participants.len(),
                            curve: s.curve_type.clone(),
                            mode: s.coordination_type.clone(),
                            created_at: "Just now".to_string(),
                            expires_in: "30 mins".to_string(),
                        }
                    })
                    .collect();
                
                component.update_sessions(ui_sessions);
                
                // Set the selected tab from model
                component.set_selected_tab(self.model.ui_state.join_session_tab);
                debug!("🎯 JoinSession tab set to: {}", if self.model.ui_state.join_session_tab == 0 { "DKG" } else { "Signing" });
                
                // Set the selected index from model
                if let Some(selected_idx) = self.model.ui_state.selected_indices.get(&crate::elm::model::ComponentId::JoinSession) {
                    component.set_selected_index(*selected_idx);
                    debug!("🎯 JoinSession selected index set to: {}", selected_idx);
                }
                
                self.app.mount(
                    Id::JoinSession,
                    Box::new(component),
                    vec![]
                )?;
                self.app.active(&Id::JoinSession)?;
            }
            Screen::DKGProgress { ref session_id } => {
                info!("🔧 Mounting DKGProgress component for session: {}", session_id);
                
                // Get config values from creating_wallet state
                let (total_participants, threshold) = if let Some(ref creating_wallet) = self.model.wallet_state.creating_wallet {
                    if let Some(ref config) = creating_wallet.custom_config {
                        (config.total_participants, config.threshold)
                    } else {
                        (3, 2) // Default values
                    }
                } else {
                    (3, 2) // Default values
                };
                
                // Create the DKG progress component with proper state
                let mut dkg_progress = crate::elm::components::DKGProgressComponent::new(
                    session_id.clone(),
                    total_participants,
                    threshold
                );

                // Sync the DKG phase label from Model. Update::Message handlers
                // (StartDKGProtocol → Round1, first ProcessDKGRound2 → Round2,
                // DKGKeyGenerated → Finalization) own the transitions; we just
                // copy the current phase onto the freshly-mounted component so
                // the user sees the real round instead of the Initialization
                // default during every remount.
                dkg_progress.set_round(self.model.wallet_state.dkg_round.clone());

                // Update WebSocket connection status
                dkg_progress.set_websocket_connected(self.model.network_state.connected);
                
                // Add participants from active session if available (excluding self)
                if let Some(ref session) = self.model.active_session {
                    info!("📋 Session participants: {:?}, self: {}", session.participants, self.model.device_id);
                    for participant in &session.participants {
                        // Skip self - we don't need to show our own status
                        if participant == &self.model.device_id {
                            info!("  Skipping self: {}", participant);
                            continue;
                        }
                        
                        // Check if we have WebRTC status for this participant
                        if let Some(&(webrtc_connected, data_channel_open)) = 
                            self.model.network_state.participant_webrtc_status.get(participant) {
                            info!("  {} - WebRTC: {}, DataChannel: {}", participant, webrtc_connected, data_channel_open);
                            // Use the actual WebRTC status
                            dkg_progress.update_webrtc_status(
                                participant.clone(),
                                webrtc_connected,
                                data_channel_open
                            );
                        } else {
                            info!("  {} - No status, defaulting to Waiting", participant);
                            // Default to waiting for other participants
                            dkg_progress.update_participant(
                                participant.clone(),
                                crate::elm::components::dkg_progress::ParticipantStatus::Waiting
                            );
                        }
                    }
                }
                
                // Calculate and update mesh status if we have an active session
                if let Some(ref session) = self.model.active_session {
                    // Count how many participants have data channels open (excluding self)
                    let mesh_ready_count = session.participants.iter()
                        .filter(|p| **p != self.model.device_id) // Exclude self
                        .filter(|p| {
                            self.model.network_state.participant_webrtc_status.get(*p)
                                .is_some_and(|(_, data_channel_open)| *data_channel_open)
                        })
                        .count();
                    
                    // Check if all expected participants have data channels open
                    let expected_other_participants = (total_participants as usize).saturating_sub(1);
                    let all_connected = mesh_ready_count == expected_other_participants;
                    
                    info!("🔗 Mesh status calculation: ready_count={}, expected={}, all_connected={}", 
                          mesh_ready_count, expected_other_participants, all_connected);
                    
                    // Update mesh status in the component
                    dkg_progress.update_mesh_status(mesh_ready_count, all_connected);
                }
                
                // Set the selected action from the model
                if let Some(selected_action) = self.model.ui_state.selected_indices.get(&crate::elm::model::ComponentId::DKGProgress) {
                    dkg_progress.set_selected_action(*selected_action);
                }
                
                self.app.mount(
                    Id::DKGProgress,
                    Box::new(dkg_progress),
                    vec![]
                )?;
                self.app.active(&Id::DKGProgress)?;
            }
            Screen::PasswordPrompt => {
                // Sync the live draft (lengths only) from Model so the
                // freshly-mounted component matches whatever the user has
                // typed so far. The component is remounted after every
                // keystroke — see `needs_component_update` — so this is
                // the single place we have to keep in sync.
                let mut password_prompt =
                    crate::elm::components::PasswordPromptComponent::new();
                password_prompt.set_from_model(&self.model.wallet_state);
                self.app.mount(
                    Id::PasswordPrompt,
                    Box::new(password_prompt),
                    vec![]
                )?;
                self.app.active(&Id::PasswordPrompt)?;
            }
            Screen::WalletComplete { .. } => {
                // The wallet-id is on the Screen variant for routing
                // purposes, but the full snapshot (group key + addresses)
                // is on `wallet_state.last_finalized_wallet`. Pass that
                // into the component via `set_from_model` — same pattern
                // as PasswordPrompt / DKGProgress.
                let mut complete =
                    crate::elm::components::WalletCompleteComponent::new();
                complete.set_from_model(&self.model.wallet_state);
                self.app.mount(
                    Id::WalletComplete,
                    Box::new(complete),
                    vec![]
                )?;
                self.app.active(&Id::WalletComplete)?;
            }
            Screen::SignTransaction { ref wallet_id } => {
                // Phase C.3: the screen itself is view-only. Every keystroke
                // flows through `handle_key_event` → `SignTypeChar` /
                // `SignBackspace` / `SignSubmit`, mutating
                // `wallet_state.sign_message_draft` which the component
                // reads at mount time.
                let mut sign = crate::elm::components::SignTransactionComponent::new(
                    wallet_id.clone(),
                );
                sign.set_from_model(&self.model.wallet_state);
                self.app.mount(
                    Id::SignTransaction,
                    Box::new(sign),
                    vec![]
                )?;
                self.app.active(&Id::SignTransaction)?;
            }
            Screen::SignatureComplete { .. } => {
                // Phase C.5: terminal signing screen. Same view-only
                // pattern as WalletComplete — data flows from
                // `wallet_state.last_completed_signature`.
                let mut sc =
                    crate::elm::components::SignatureCompleteComponent::new();
                sc.set_from_model(&self.model.wallet_state);
                self.app.mount(Id::SignatureComplete, Box::new(sc), vec![])?;
                self.app.active(&Id::SignatureComplete)?;
            }
            Screen::SigningProgress { ref request_id } => {
                // Reuse DKGProgressComponent — it already renders the
                // participant mesh + a round indicator, which is
                // exactly the view we want during signing. Override
                // the title via `set_ceremony_label` so it reads
                // "🖊️  Signing Progress" instead of "🔐 DKG Progress";
                // without this override, users running a post-DKG
                // signing ceremony see a confusing "DKG" label.
                //
                // The round label stays at the inherited DKGRound
                // because signing has its own stages (commit / share /
                // aggregate) which the enum doesn't model — showing
                // them would need a separate component. That's a
                // Phase D polish task.
                let (total_participants, threshold) = self
                    .model
                    .active_session
                    .as_ref()
                    .map(|s| (s.total, s.threshold))
                    .unwrap_or((3, 2));

                // Resolve a chain label for the header. Joiners get the
                // proposer-supplied `blockchain` from the announce
                // (`SessionType::Signing`); creators don't have a
                // session yet of that variant, so fall back to the
                // running curve type. Either way we surface
                // *something* — "no chain shown" was the visible
                // half of this bug report.
                let chain = self
                    .model
                    .active_session
                    .as_ref()
                    .and_then(|s| match &s.session_type {
                        crate::protocal::signal::SessionType::Signing {
                            blockchain,
                            ..
                        } => Some(blockchain.clone()),
                        _ => None,
                    })
                    .or_else(|| {
                        let c = self.model.wallet_state.curve_type;
                        if c.is_empty() { None } else { Some(c.to_string()) }
                    });

                let mut progress = crate::elm::components::DKGProgressComponent::new(
                    request_id.clone(),
                    total_participants,
                    threshold,
                );
                progress.set_ceremony(
                    crate::elm::components::dkg_progress::Ceremony::Signing { chain },
                );
                progress.set_round(self.model.wallet_state.dkg_round.clone());
                progress.set_websocket_connected(self.model.network_state.connected);
                if let Some(ref session) = self.model.active_session {
                    for p in &session.participants {
                        if p == &self.model.device_id {
                            continue;
                        }
                        if let Some(&(wc, dc)) =
                            self.model.network_state.participant_webrtc_status.get(p)
                        {
                            progress.update_webrtc_status(p.clone(), wc, dc);
                        } else {
                            progress.update_participant(
                                p.clone(),
                                crate::elm::components::dkg_progress::ParticipantStatus::Waiting,
                            );
                        }

                        // Stage 4: overlay signing-ceremony progress on
                        // top of the WebRTC-status row. Commitments win
                        // over mesh state because once a commitment
                        // arrived, "mesh ready" is implied. Shares win
                        // over commitments for the same reason.
                        use crate::elm::components::dkg_progress::ParticipantStatus;
                        if self
                            .model
                            .wallet_state
                            .signing_shares_received
                            .contains(p)
                        {
                            progress.update_participant(
                                p.clone(),
                                ParticipantStatus::Round2Complete,
                            );
                        } else if self
                            .model
                            .wallet_state
                            .signing_commitments_received
                            .contains(p)
                        {
                            progress.update_participant(
                                p.clone(),
                                ParticipantStatus::Round1Complete,
                            );
                        }
                    }
                }
                self.app.mount(Id::DKGProgress, Box::new(progress), vec![])?;
                self.app.active(&Id::DKGProgress)?;
            }
            _ => {
                // Default to main menu for unimplemented screens
                let wallet_count = self.model.wallet_state.wallets.len();
                self.app.mount(
                    Id::MainMenu,
                    Box::new(MainMenu::with_wallet_count(wallet_count)),
                    vec![]
                )?;
                self.app.active(&Id::MainMenu)?;
            }
        }
        
        // Always mount modal and notification components (they control
        // their own visibility). `set_from_model` populates each with
        // the current Model state so their `view()` renders the live
        // data — without this step both would draw empty even though
        // the Model had pending toasts / modal.
        let mut modal_component = ModalComponent::default();
        modal_component.set_from_model(&self.model);
        self.app.mount(Id::Modal, Box::new(modal_component), vec![])?;

        let mut notification_bar = NotificationBar::default();
        notification_bar.set_from_model(&self.model);
        self.app.mount(
            Id::NotificationBar,
            Box::new(notification_bar),
            vec![]
        )?;
        
        Ok(())
    }
    
    /// Process a message through the update function
    async fn process_message(&mut self, msg: Message) {
        info!("📨 Processing message: {:?}", msg);
        
        // Special debug for NavigateBack
        if matches!(msg, Message::NavigateBack) {
            debug!("🚨 PROCESSING NavigateBack MESSAGE!");
        }
        
        // Log the current screen before processing
        debug!("Current screen before: {:?}", self.model.current_screen);
        
        // Check for quit message
        if matches!(msg, Message::Quit) {
            info!("Quit message received, exiting...");
            self.should_quit = true;
            return;
        }
        
        // Check if this is a scroll message that needs component update
        // Remount after any message that mutates state the component reads
        // from Model at mount time. For PasswordPrompt that's every
        // keystroke — the component's rendered bullets/focus/error come
        // from `wallet_state.*_draft` / `password_error`, which are only
        // copied into the component via its setter during mount.
        let needs_component_update = matches!(
            msg,
            Message::ScrollUp
                | Message::ScrollDown
                | Message::ScrollLeft
                | Message::ScrollRight
                | Message::PasswordTypeChar(_)
                | Message::PasswordBackspace
                | Message::PasswordToggleField
                | Message::PasswordSubmitDraft
                | Message::SignTypeChar(_)
                | Message::SignBackspace
                // Stage 4: signing-round messages mutate the
                // acceptance roster on WalletState; the SigningProgress
                // component reads that roster at mount time, so each
                // commitment/share needs a remount for the row to
                // actually flip to Round1/Round2Complete.
                | Message::ProcessSigningRound1 { .. }
                | Message::ProcessSigningRound2 { .. }
        );
        
        // Check if this is a force remount message
        let force_remount = matches!(msg, Message::ForceRemount);
        if force_remount {
            info!("🔄 ForceRemount detected in app.rs");
        }
        
        // Update the model and get command
        if let Some(command) = update(&mut self.model, msg) {
            debug!("Update produced command: {:?}", command);
            // Execute the command
            let tx = self.message_tx.clone();
            let app_state = self.app_state.clone();
            
            tokio::spawn(async move {
                if let Err(e) = command.execute(tx, &app_state).await {
                    error!("Command execution failed: {}", e);
                }
            });
        } else {
            debug!("Update produced no command");
        }
        
        // Log the current screen after processing
        debug!("Current screen after: {:?}", self.model.current_screen);
        
        // Check if we need to remount
        let need_remount = self.should_remount() || needs_component_update || force_remount;
        if need_remount {
            info!("🔁 Need remount: {} (should_remount: {}, needs_update: {}, force: {})", 
                   need_remount, self.should_remount(), needs_component_update, force_remount);
        }
        
        // Enhanced debug logging for CreateWallet state sync
        if matches!(self.model.current_screen, Screen::CreateWallet(_)) {
            debug!("🔍 CreateWallet post-update state:");
            debug!("   - Current focus: {:?}", self.model.ui_state.focus);
            debug!("   - Selected indices: {:?}", self.model.ui_state.selected_indices);
            debug!("   - Component mounted: {}", self.app.mounted(&Id::CreateWallet));
            if let Some(selected) = self.model.ui_state.selected_indices.get(&self.model.ui_state.focus) {
                debug!("   - Current selection for focused component: {}", selected);
            }
        }
        
        // Remount components if screen changed or selection updated
        if need_remount {
            debug!("Remounting components for screen: {:?}", self.model.current_screen);
            
            // Add specific debug for ThresholdConfig
            if matches!(self.model.current_screen, Screen::ThresholdConfig) {
                let selected_field = self.model.ui_state.selected_indices
                    .get(&crate::elm::model::ComponentId::ThresholdConfig)
                    .copied()
                    .unwrap_or(0);
                info!("🔄 REMOUNTING ThresholdConfig with selected_field={} from selected_indices", selected_field);
            }
            
            // Add specific debug for DKGProgress
            if matches!(self.model.current_screen, Screen::DKGProgress { .. }) {
                let selected_action = self.model.ui_state.selected_indices
                    .get(&crate::elm::model::ComponentId::DKGProgress)
                    .copied()
                    .unwrap_or(0);
                info!("🔄 REMOUNTING DKGProgress with selected_action={} from selected_indices", selected_action);
            }
            
            if let Err(e) = self.mount_components() {
                error!("Failed to mount components: {}", e);
            }
            
            // Force a render after remounting to ensure UI updates
            if let Err(e) = self.render() {
                error!("Failed to render after remount: {}", e);
            }
        }
        
        // Update component states
        self.update_component_states();
    }
    
    /// Check if components need to be remounted  
    fn should_remount(&self) -> bool {
        // Check if the mounted component matches current screen
        match self.model.current_screen {
            Screen::MainMenu | Screen::Welcome => !self.app.mounted(&Id::MainMenu),
            Screen::ManageWallets => !self.app.mounted(&Id::WalletList),
            Screen::WalletDetail { .. } => !self.app.mounted(&Id::WalletDetail),
            Screen::CreateWallet(_) => !self.app.mounted(&Id::CreateWallet),
            Screen::ModeSelection => !self.app.mounted(&Id::ModeSelection),
            Screen::ThresholdConfig => !self.app.mounted(&Id::ThresholdConfig),
            Screen::JoinSession => !self.app.mounted(&Id::JoinSession),
            Screen::DKGProgress { .. } => !self.app.mounted(&Id::DKGProgress),
            Screen::PasswordPrompt => !self.app.mounted(&Id::PasswordPrompt),
            Screen::WalletComplete { .. } => !self.app.mounted(&Id::WalletComplete),
            Screen::SignTransaction { .. } => !self.app.mounted(&Id::SignTransaction),
            // SigningProgress reuses the DKGProgress component, so remount
            // when the DKGProgress slot is empty.
            Screen::SigningProgress { .. } => !self.app.mounted(&Id::DKGProgress),
            Screen::SignatureComplete { .. } => !self.app.mounted(&Id::SignatureComplete),
            _ => false,
        }
    }
    
    /// Update component states with latest model data
    fn update_component_states(&mut self) {
        // Update MainMenu selection if it's mounted
        if self.app.mounted(&Id::MainMenu)
            && let Some(selected_idx) = self.model.ui_state.selected_indices.get(&self.model.ui_state.focus) {
                // Unfortunately tuirealm doesn't expose a way to update component state directly
                // We'll need to handle this in the render or via messages
                debug!("Would update MainMenu selected index to: {}", selected_idx);
            }
    }
    
    /// Main event loop
    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("Starting Elm application event loop");
        
        // Initial render
        self.render()?;
        
        loop {
            // Check if we should quit
            if self.should_quit {
                info!("Quitting application");
                break;
            }
            
            // Poll for events with a small timeout
            tokio::select! {
                // Handle terminal events
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    // Check for crossterm events with a proper timeout
                    if crossterm::event::poll(Duration::from_millis(10))? {
                        match crossterm::event::read() {
                            Ok(event) => {
                                debug!("Read terminal event: {:?}", event);
                                self.handle_terminal_event(event).await?;
                            }
                            Err(e) => {
                                debug!("Error reading terminal event: {:?}", e);
                            }
                        }
                    }
                }
                
                // Handle messages from the update loop
                Some(msg) = self.message_rx.recv() => {
                    self.process_message(msg).await;
                    self.render()?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle terminal events
    async fn handle_terminal_event(&mut self, event: CrosstermEvent) -> anyhow::Result<()> {
        match event {
            CrosstermEvent::Key(key_event) => {
                info!("📺 Received key event: {:?}", key_event);
                
                // Special debug for Enter and Esc keys at terminal level
                if matches!(key_event.code, crossterm::event::KeyCode::Enter) {
                    info!("🔥 ENTER KEY RECEIVED AT TERMINAL LEVEL!");
                }
                if matches!(key_event.code, crossterm::event::KeyCode::Esc) {
                    debug!("🚨 ESC KEY RECEIVED AT TERMINAL LEVEL!");
                }
                
                let msg = self.handle_key_event(key_event);
                if let Some(msg) = msg {
                    debug!("🎯 Key event produced message: {:?}", msg);
                    self.process_message(msg).await;
                }
                // Always render after key events to show component updates
                self.render()?;
            }
            CrosstermEvent::Resize(_, _) => {
                debug!("Terminal resized");
                self.render()?;
            }
            _ => {
                debug!("Other terminal event: {:?}", event);
            }
        }
        
        Ok(())
    }
    
    /// Handle key events - KISS approach, direct crossterm handling
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Option<Message> {
        debug!("🔑 Key pressed: {:?}", key.code);
        
        use crossterm::event::KeyCode;
        
        // Check if modal is open first - modal keys take priority.
        // For Modal::Confirm we must distinguish the two keys: Enter
        // fires ConfirmModal (→ on_confirm), Esc fires CancelModal (→
        // on_cancel). Prior to this, both keys dispatched CloseModal,
        // which silently dropped both handlers — a real bug that hid
        // for as long as no confirm-modal was in play. For non-Confirm
        // modals (Error/Success/Progress) either handler is fine since
        // they fall through to the plain modal-clear path.
        if self.model.ui_state.modal.is_some() {
            match key.code {
                KeyCode::Enter => {
                    debug!("✅ Modal Enter → ConfirmModal");
                    return Some(Message::ConfirmModal);
                }
                KeyCode::Esc => {
                    debug!("🔙 Modal Esc → CancelModal");
                    return Some(Message::CancelModal);
                }
                _ => return None, // Ignore other keys when modal is open
            }
        }
        
        // Global keys first - work everywhere
        match key.code {
            KeyCode::Esc => {
                debug!("🔙 Esc -> NavigateBack");
                return Some(Message::NavigateBack);
            }
            KeyCode::Char('q') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                debug!("🚪 Ctrl+Q -> Quit");
                return Some(Message::Quit);
            }
            KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                debug!("🚪 Ctrl+C -> Quit");
                return Some(Message::Quit);
            }
            KeyCode::Char('r') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                debug!("🔄 Ctrl+R -> Refresh");
                return Some(Message::Refresh);
            }
            KeyCode::Char('h') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                debug!("🏠 Ctrl+H -> Home");
                return Some(Message::NavigateHome);
            }
            _ => {}
        }
        
        // For ThresholdConfig screen, we need to update the component's state and then remount
        // Since tuirealm doesn't provide a direct way to send commands to components,
        // we'll update our model and remount the component
        if matches!(self.model.current_screen, Screen::ThresholdConfig) {
            match key.code {
                KeyCode::Left | KeyCode::Right => {
                    // These are handled by ScrollLeft/ScrollRight which update the model
                    // and trigger a remount
                    if key.code == KeyCode::Left {
                        info!("⬅️ ThresholdConfig LEFT -> ScrollLeft");
                        return Some(Message::ScrollLeft);
                    } else {
                        info!("➡️ ThresholdConfig RIGHT -> ScrollRight");  
                        return Some(Message::ScrollRight);
                    }
                }
                KeyCode::Up | KeyCode::Down => {
                    // For up/down, we need to update the values
                    // Let the normal ScrollUp/ScrollDown handle it
                    if key.code == KeyCode::Up {
                        info!("🔼 ThresholdConfig UP -> ScrollUp");
                        return Some(Message::ScrollUp);
                    } else {
                        info!("🔽 ThresholdConfig DOWN -> ScrollDown");
                        return Some(Message::ScrollDown);
                    }
                }
                KeyCode::Enter => {
                    info!("🔥 ThresholdConfig ENTER -> SelectItem");
                    let selected_index = self.model.ui_state.selected_indices
                        .get(&crate::elm::model::ComponentId::ThresholdConfig)
                        .copied()
                        .unwrap_or(0);
                    return Some(Message::SelectItem { index: selected_index });
                }
                _ => {}
            }
        }
        
        // WalletComplete is a terminal success screen — both Enter and
        // Esc mean "I'm done, go home". Esc is already handled by the
        // global arm above (→ NavigateBack, which pops our stack frame
        // back to MainMenu), so we only need Enter here. Any other key
        // is ignored so accidental typing doesn't drop the user into a
        // stale selection model.
        if matches!(self.model.current_screen, Screen::WalletComplete { .. }) {
            match key.code {
                KeyCode::Enter => {
                    info!("✅ WalletComplete Enter → NavigateBack → MainMenu");
                    return Some(Message::NavigateBack);
                }
                // `c` / `C` → copy the group pubkey hex to the system
                // clipboard. Some terminals don't forward mouse highlight
                // to a selectable clipboard, so a keyboard shortcut is
                // the only reliable way to grab this value.
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if let Some(ref info) = self.model.wallet_state.last_finalized_wallet {
                        return Some(Message::CopyToClipboard {
                            text: info.group_pubkey_hex.clone(),
                            label: "group verifying key".to_string(),
                        });
                    }
                    return None;
                }
                _ => return None,
            }
        }

        // SignatureComplete mirrors WalletComplete — Enter dismisses
        // back to MainMenu, Esc does the same via the global arm.
        if matches!(self.model.current_screen, Screen::SignatureComplete { .. }) {
            match key.code {
                KeyCode::Enter => {
                    info!("✅ SignatureComplete Enter → NavigateBack → MainMenu");
                    return Some(Message::NavigateBack);
                }
                // `c` / `C` → copy the hex-encoded FROST signature.
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if let Some(ref info) = self.model.wallet_state.last_completed_signature {
                        return Some(Message::CopyToClipboard {
                            text: hex::encode(&info.signature),
                            label: "FROST signature".to_string(),
                        });
                    }
                    return None;
                }
                _ => return None,
            }
        }

        // SignTransaction screen (Phase C.3): free-text message input.
        // Same pattern as PasswordPrompt — Esc is globally handled,
        // printable chars/backspace/Enter route through dedicated
        // Messages that mutate `Model.wallet_state.sign_message_draft`.
        if matches!(self.model.current_screen, Screen::SignTransaction { .. }) {
            match key.code {
                KeyCode::Char(c) => return Some(Message::SignTypeChar(c)),
                KeyCode::Backspace => return Some(Message::SignBackspace),
                KeyCode::Enter => return Some(Message::SignSubmit),
                _ => return None,
            }
        }

        // PasswordPrompt screen — this is a text-entry screen, so every
        // printable character, backspace, tab, and Enter routes through
        // dedicated messages that mutate `Model.wallet_state.*_draft`.
        // The component renders from that state; there is no per-component
        // `on()` handler in play (the architecture pushes everything
        // through `handle_key_event` → Message → update → render). Esc is
        // handled globally above, so we don't match it here.
        if matches!(self.model.current_screen, Screen::PasswordPrompt) {
            match key.code {
                KeyCode::Char(c) => {
                    return Some(Message::PasswordTypeChar(c));
                }
                KeyCode::Backspace => {
                    return Some(Message::PasswordBackspace);
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    return Some(Message::PasswordToggleField);
                }
                KeyCode::Enter => {
                    return Some(Message::PasswordSubmitDraft);
                }
                _ => return None, // ignore arrows / fn keys here
            }
        }

        // For DKGProgress screen, handle Left/Right to switch between action buttons
        if matches!(self.model.current_screen, Screen::DKGProgress { .. }) {
            match key.code {
                KeyCode::Left => {
                    info!("⬅️ DKGProgress LEFT -> ScrollLeft");
                    return Some(Message::ScrollLeft);
                }
                KeyCode::Right => {
                    info!("➡️ DKGProgress RIGHT -> ScrollRight");
                    return Some(Message::ScrollRight);
                }
                KeyCode::Enter => {
                    info!("🔥 DKGProgress ENTER -> SelectItem");
                    let selected_index = self.model.ui_state.selected_indices
                        .get(&crate::elm::model::ComponentId::DKGProgress)
                        .copied()
                        .unwrap_or(0);
                    return Some(Message::SelectItem { index: selected_index });
                }
                _ => {}
            }
        }

        // Screen-specific keys for other screens
        match key.code {
            KeyCode::Up => {
                info!("🔼 UP ARROW KEY PRESSED! -> ScrollUp");
                Some(Message::ScrollUp)
            }
            KeyCode::Down => {
                info!("🔽 DOWN ARROW KEY PRESSED! -> ScrollDown");
                Some(Message::ScrollDown)
            }
            KeyCode::Left => {
                info!("⬅️ LEFT ARROW KEY PRESSED! -> ScrollLeft");
                Some(Message::ScrollLeft)
            }
            KeyCode::Right => {
                info!("➡️ RIGHT ARROW KEY PRESSED! -> ScrollRight");
                Some(Message::ScrollRight)
            }
            KeyCode::Enter => {
                info!("🔥 ENTER KEY PRESSED! Screen: {:?}, Focus: {:?}", 
                     self.model.current_screen, self.model.ui_state.focus);
                
                // Get the current selected index from the model for the focused component
                let selected_index = self.model.ui_state.selected_indices
                    .get(&self.model.ui_state.focus)
                    .copied()
                    .unwrap_or(0);
                    
                info!("✅ Enter -> SelectItem with current selected index: {} (focus: {:?})", 
                       selected_index, self.model.ui_state.focus);
                Some(Message::SelectItem { index: selected_index })
            }
            _ => {
                debug!("❓ Unhandled key: {:?}", key.code);
                None
            }
        }
    }
    
    /// Render the UI
    fn render(&mut self) -> anyhow::Result<()> {
        debug!("🎨 Rendering UI - Current screen: {:?}", self.model.current_screen);

        // Dynamic overlays (notifications, modals) are populated at mount
        // time but their source-of-truth is `Model.ui_state.*`, which
        // can change without triggering a full screen remount. Refresh
        // both slots here so a newly-pushed toast or modal is visible
        // on the very next frame instead of waiting for the next
        // unrelated remount.
        //
        // IMPORTANT: use `remount` not `mount`. `mount` errors out
        // silently if the component is already mounted (returns
        // `ApplicationError::AlreadyMounted`) — a silent failure that
        // cost us a full round of debugging. `remount` is the
        // idempotent-replacement variant.
        let mut fresh_modal = ModalComponent::default();
        fresh_modal.set_from_model(&self.model);
        let _ = self
            .app
            .remount(Id::Modal, Box::new(fresh_modal), vec![]);

        let mut fresh_notifs = NotificationBar::default();
        fresh_notifs.set_from_model(&self.model);
        let _ = self
            .app
            .remount(Id::NotificationBar, Box::new(fresh_notifs), vec![]);

        self.terminal.raw_mut().draw(|f| {
            // Create main layout
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(if self.model.ui_state.notifications.is_empty() { 0 } else { 3 }),
                    Constraint::Min(0),
                ])
                .split(f.area());
            
            // Render notification bar if there are notifications
            if !self.model.ui_state.notifications.is_empty() {
                self.app.view(&Id::NotificationBar, f, chunks[0]);
            }
            
            // Render main content based on screen
            let main_area = if self.model.ui_state.notifications.is_empty() {
                f.area()
            } else {
                chunks[1]
            };
            
            // Render active component
            match self.model.current_screen {
                Screen::MainMenu | Screen::Welcome => {
                    self.app.view(&Id::MainMenu, f, main_area);
                }
                Screen::ManageWallets => {
                    self.app.view(&Id::WalletList, f, main_area);
                }
                Screen::WalletDetail { .. } => {
                    self.app.view(&Id::WalletDetail, f, main_area);
                }
                Screen::CreateWallet(_) => {
                    self.app.view(&Id::CreateWallet, f, main_area);
                }
                Screen::ModeSelection => {
                    self.app.view(&Id::ModeSelection, f, main_area);
                }
                Screen::ThresholdConfig => {
                    self.app.view(&Id::ThresholdConfig, f, main_area);
                }
                Screen::JoinSession => {
                    self.app.view(&Id::JoinSession, f, main_area);
                }
                Screen::DKGProgress { .. } => {
                    self.app.view(&Id::DKGProgress, f, main_area);
                }
                Screen::PasswordPrompt => {
                    self.app.view(&Id::PasswordPrompt, f, main_area);
                }
                Screen::WalletComplete { .. } => {
                    self.app.view(&Id::WalletComplete, f, main_area);
                }
                Screen::SignTransaction { .. } => {
                    self.app.view(&Id::SignTransaction, f, main_area);
                }
                Screen::SigningProgress { .. } => {
                    // Reuses DKGProgress slot — see mount_components.
                    self.app.view(&Id::DKGProgress, f, main_area);
                }
                Screen::SignatureComplete { .. } => {
                    self.app.view(&Id::SignatureComplete, f, main_area);
                }
                _ => {
                    // Fallback to main menu
                    self.app.view(&Id::MainMenu, f, main_area);
                }
            }
            
            // Render modal if present
            if self.model.ui_state.modal.is_some() {
                // Calculate modal area (centered, smaller than full screen)
                let modal_area = centered_rect(60, 20, main_area);
                self.app.view(&Id::Modal, f, modal_area);
            }
        })?;
        
        Ok(())
    }
    
    /// Get a message sender for external use
    pub fn get_message_sender(&self) -> UnboundedSender<Message> {
        self.message_tx.clone()
    }
}

use std::time::Duration;
use ratatui::layout::Rect;

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Removed unnecessary convert_key_event function - KISS approach!