//! Native MPC Wallet node — Iced GUI.
//!
//! Ported from Slint to Iced (MIT-licensed) so the GUI stack is
//! compatible with a proprietary product. The app follows Iced's Elm
//! architecture, which maps cleanly onto the existing
//! `tui_node::core::*Manager` backend:
//!
//! * the old Slint global `AppState` becomes the [`State`] model,
//! * Slint `callback`s become [`Message`] variants handled in
//!   [`State::update`],
//! * each `.slint` screen/component becomes an Iced `view` helper
//!   returning `Element<Message>`,
//! * the async core managers push state through [`UICallback`] →
//!   [`UiEvent`] over an mpsc channel, drained by a [`Subscription`]
//!   that turns each event into [`Message::Ui`].

mod core_adapter;
mod ui_callback;

use std::sync::{Arc, Mutex as StdMutex};

use iced::futures::stream;
use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Column, Space,
};
use iced::{Color, Element, Length, Subscription, Task, Theme};

use core_adapter::CoreAdapter;
use tui_node::core::{ConnectionInfo, SessionInfo, SessionStatus, SigningState, WalletInfo};
use ui_callback::UiEvent;

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    iced::application(State::new, State::update, State::view)
        .title("FROST MPC Wallet — Native")
        .theme(State::theme)
        .subscription(State::subscription)
        .run()
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Which right-panel tab is showing (mirrors the old Slint `TabWidget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Operations,
    Logs,
    Settings,
}

#[derive(Debug, Clone)]
pub enum Message {
    // --- UI events pushed from the async core ---
    Ui(UiEvent),

    // --- Navigation / input ---
    TabSelected(Tab),
    NewWalletNameChanged(String),
    KeystorePasswordChanged(String),
    WsUrlChanged(String),
    SignHexChanged(String),
    SignChainChanged(String),
    JoinSessionIdChanged(String),
    WalletSelected(usize),

    // --- Actions (were Slint callbacks) ---
    ConnectWebsocket,
    CreateWallet,
    ImportWallet,
    ExportWallet,
    CreateSession,
    JoinSession,
    LeaveSession,
    RefreshSessions,
    ToggleOfflineMode,
    StartDkg,
    AbortDkg,
    SignMessage,
    ApproveSigning,
    RejectSigning,
    ExportToSdCard,
    ImportFromSdCard,
    ClearSdCard,

    /// Result of a fire-and-forget async action (the core reports the
    /// real outcome via `Ui` events; this just logs failures).
    ActionDone(Result<(), String>),
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The Iced model — formerly the Slint global `AppState`.
pub struct State {
    adapter: Arc<CoreAdapter>,

    // Connection state
    websocket_connected: bool,
    webrtc_connected: bool,
    mesh_connections: Vec<ConnectionInfo>,
    operation_mode: String,

    // Wallet state
    wallets: Vec<WalletInfo>,
    active_wallet_index: usize,
    has_keystore: bool,

    // Session state
    available_sessions: Vec<SessionInfo>,
    active_session: Option<SessionInfo>,

    // DKG state
    dkg_active: bool,
    dkg_current_round: u8,
    dkg_progress: f32,

    // Offline state
    offline_enabled: bool,
    sd_card_detected: bool,

    // Signing state
    has_signing_request: bool,
    signing_request_id: String,
    signing_message_preview: String,
    signing_chain: String,
    signing_label: String,
    signing_state: String,
    last_signature: String,

    // UI chrome
    device_id: String,
    status_message: String,
    log_messages: Vec<String>,
    active_tab: Tab,

    // Input fields
    new_wallet_name: String,
    dkg_password: String,
    ws_url: String,
    sign_hex: String,
    sign_chain: String,
    join_session_id: String,
}

impl State {
    fn new() -> (Self, Task<Message>) {
        // Build the UICallback → UI bridge channel. The sender goes into
        // the CoreAdapter (handed to NativeUICallback); the receiver is
        // stashed for the subscription to pull out exactly once.
        let (ui_tx, ui_rx) = tokio::sync::mpsc::unbounded_channel::<UiEvent>();
        BRIDGE_RX
            .lock()
            .expect("bridge mutex")
            .replace(ui_rx);

        let device_id =
            std::env::var("MPC_DEVICE_ID").unwrap_or_else(|_| "native-node-001".to_string());

        // Keystore lives alongside the TUI's (~/.frost_keystore).
        let keystore_path = format!(
            "{}/.frost_keystore",
            std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
        );

        // Signal server (env-configurable; hosted worker needs ?room=).
        let signal_base = std::env::var("MPC_SIGNAL_SERVER")
            .unwrap_or_else(|_| "wss://panda.qzz.io".to_string());
        let signal_url = match std::env::var("MPC_ROOM") {
            Ok(room) if !room.is_empty() && !signal_base.contains("room=") => {
                if signal_base.contains('?') {
                    format!("{signal_base}&room={room}")
                } else if signal_base
                    .splitn(2, "://")
                    .nth(1)
                    .unwrap_or(&signal_base)
                    .contains('/')
                {
                    format!("{signal_base}?room={room}")
                } else {
                    format!("{signal_base}/?room={room}")
                }
            }
            _ => signal_base,
        };

        let curve = std::env::var("MPC_CURVE").unwrap_or_else(|_| "secp256k1".to_string());

        let adapter = Arc::new(CoreAdapter::new(
            ui_tx,
            device_id.clone(),
            keystore_path,
            signal_url.clone(),
            curve,
        ));

        let state = State {
            adapter,
            websocket_connected: false,
            webrtc_connected: false,
            mesh_connections: Vec::new(),
            operation_mode: "online".to_string(),
            wallets: Vec::new(),
            active_wallet_index: 0,
            has_keystore: false,
            available_sessions: Vec::new(),
            active_session: None,
            dkg_active: false,
            dkg_current_round: 0,
            dkg_progress: 0.0,
            offline_enabled: false,
            sd_card_detected: false,
            has_signing_request: false,
            signing_request_id: String::new(),
            signing_message_preview: String::new(),
            signing_chain: String::new(),
            signing_label: String::new(),
            signing_state: "idle".to_string(),
            last_signature: String::new(),
            device_id,
            status_message: "Ready".to_string(),
            log_messages: Vec::new(),
            active_tab: Tab::Operations,
            new_wallet_name: String::new(),
            dkg_password: String::new(),
            ws_url: signal_url,
            sign_hex: String::new(),
            sign_chain: "ethereum".to_string(),
            join_session_id: String::new(),
        };

        (state, Task::none())
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    // -- Subscription: drain the UICallback bridge channel --------------

    fn subscription(&self) -> Subscription<Message> {
        // `Subscription::run` identifies the subscription by the type of
        // the worker fn, so it's spawned exactly once for the app's life.
        // The worker takes the receiver out of the global slot and folds
        // it into a stream of `Message::Ui`.
        Subscription::run(ui_bridge)
    }

    // -- Update ---------------------------------------------------------

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ui(event) => {
                self.apply_ui_event(event);
                Task::none()
            }

            Message::TabSelected(tab) => {
                self.active_tab = tab;
                Task::none()
            }
            Message::NewWalletNameChanged(v) => {
                self.new_wallet_name = v;
                Task::none()
            }
            Message::KeystorePasswordChanged(v) => {
                self.dkg_password = v;
                Task::none()
            }
            Message::WsUrlChanged(v) => {
                self.ws_url = v;
                Task::none()
            }
            Message::SignHexChanged(v) => {
                self.sign_hex = v;
                Task::none()
            }
            Message::SignChainChanged(v) => {
                self.sign_chain = v;
                Task::none()
            }
            Message::JoinSessionIdChanged(v) => {
                self.join_session_id = v;
                Task::none()
            }
            Message::WalletSelected(i) => {
                self.active_wallet_index = i;
                Task::none()
            }

            Message::ConnectWebsocket => {
                let adapter = self.adapter.clone();
                let url = self.ws_url.clone();
                action(async move { adapter.connect_websocket(url).await })
            }
            Message::CreateWallet => {
                let adapter = self.adapter.clone();
                let name = self.new_wallet_name.clone();
                let pw = self.dkg_password.clone();
                action(async move {
                    adapter.set_dkg_password(pw);
                    adapter.create_wallet(name).await
                })
            }
            Message::ImportWallet => {
                let adapter = self.adapter.clone();
                let pw = self.dkg_password.clone();
                action(async move { adapter.import_wallet(pw).await })
            }
            Message::ExportWallet => {
                let adapter = self.adapter.clone();
                let pw = self.dkg_password.clone();
                action(async move { adapter.export_wallet(pw).await })
            }
            Message::CreateSession => {
                let adapter = self.adapter.clone();
                action(async move { adapter.create_session().await })
            }
            Message::JoinSession => {
                let adapter = self.adapter.clone();
                let id = self.join_session_id.clone();
                let pw = self.dkg_password.clone();
                action(async move {
                    adapter.set_dkg_password(pw);
                    adapter.join_session(id).await
                })
            }
            Message::LeaveSession => {
                let adapter = self.adapter.clone();
                action(async move { adapter.leave_session().await })
            }
            Message::RefreshSessions => {
                let adapter = self.adapter.clone();
                action(async move { adapter.refresh_sessions().await })
            }
            Message::ToggleOfflineMode => {
                let adapter = self.adapter.clone();
                action(async move { adapter.toggle_offline_mode().await })
            }
            Message::StartDkg => {
                let adapter = self.adapter.clone();
                action(async move { adapter.start_dkg().await })
            }
            Message::AbortDkg => {
                let adapter = self.adapter.clone();
                action(async move { adapter.abort_dkg().await })
            }
            Message::SignMessage => {
                let adapter = self.adapter.clone();
                let hex = self.sign_hex.clone();
                let chain = self.sign_chain.clone();
                action(async move { adapter.request_signing(hex, chain, None).await.map(|_| ()) })
            }
            Message::ApproveSigning => {
                let adapter = self.adapter.clone();
                let id = self.signing_request_id.clone();
                action(async move { adapter.approve_signing(id).await })
            }
            Message::RejectSigning => {
                let adapter = self.adapter.clone();
                let id = self.signing_request_id.clone();
                action(async move { adapter.reject_signing(id).await })
            }
            Message::ExportToSdCard => {
                let adapter = self.adapter.clone();
                // The old UI passed a data_type label per button; the
                // single native "Export" button uses a generic stem.
                action(async move { adapter.export_to_sd_card("export".to_string()).await })
            }
            Message::ImportFromSdCard => {
                let adapter = self.adapter.clone();
                action(async move { adapter.import_from_sd_card().await })
            }
            Message::ClearSdCard => {
                let adapter = self.adapter.clone();
                action(async move { adapter.clear_sd_card().await })
            }

            Message::ActionDone(result) => {
                if let Err(e) = result {
                    tracing::warn!("native action failed: {e}");
                    self.push_log(format!("[ERROR] {e}"));
                    self.status_message = e;
                }
                Task::none()
            }
        }
    }

    /// Apply a single UICallback event onto the model.
    fn apply_ui_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::ConnectionStatus { websocket, webrtc } => {
                self.websocket_connected = websocket;
                self.webrtc_connected = webrtc;
            }
            UiEvent::MeshConnections(conns) => self.mesh_connections = conns,
            UiEvent::OperationMode(mode) => {
                self.operation_mode = match mode {
                    tui_node::core::OperationMode::Online => "online",
                    tui_node::core::OperationMode::Offline => "offline",
                    tui_node::core::OperationMode::Hybrid => "hybrid",
                }
                .to_string();
            }
            UiEvent::Wallets(wallets) => {
                self.has_keystore = !wallets.is_empty();
                self.wallets = wallets;
            }
            UiEvent::ActiveWallet(i) => self.active_wallet_index = i,
            UiEvent::AvailableSessions(sessions) => self.available_sessions = sessions,
            UiEvent::ActiveSession(session) => self.active_session = session,
            UiEvent::DkgStatus { active, round, progress } => {
                self.dkg_active = active;
                self.dkg_current_round = round;
                self.dkg_progress = progress;
            }
            UiEvent::DkgParticipants(_participants) => {
                // TODO(iced): render a per-participant round table; for
                // now the DKG progress bar (driven by DkgStatus) suffices.
            }
            UiEvent::OfflineStatus { enabled, sd_card_detected } => {
                self.offline_enabled = enabled;
                self.sd_card_detected = sd_card_detected;
            }
            UiEvent::SdOperations(_ops) => {
                // TODO(iced): list pending SD operations. Secondary polish.
            }
            UiEvent::SigningRequest(request) => match request {
                Some(r) => {
                    self.has_signing_request = true;
                    self.signing_request_id = r.id;
                    self.signing_message_preview = if r.message_hex.len() > 120 {
                        format!("{}…", &r.message_hex[..120])
                    } else {
                        r.message_hex
                    };
                    self.signing_chain = r.chain;
                    self.signing_label = r.display_label.unwrap_or_default();
                }
                None => {
                    self.has_signing_request = false;
                    self.signing_request_id.clear();
                    self.signing_message_preview.clear();
                    self.signing_chain.clear();
                    self.signing_label.clear();
                }
            },
            UiEvent::SigningState(state) => {
                self.signing_state = match state {
                    SigningState::Idle => "idle",
                    SigningState::AwaitingApproval => "awaiting_approval",
                    SigningState::Commitment => "commitment",
                    SigningState::Share => "share",
                    SigningState::Aggregating => "aggregating",
                    SigningState::Complete => "complete",
                    SigningState::Failed(_) => "failed",
                }
                .to_string();
            }
            UiEvent::SigningComplete(sig) => self.last_signature = sig,
            UiEvent::Message { log_line, status, is_error: _ } => {
                self.push_log(log_line);
                self.status_message = status;
            }
            UiEvent::Progress(status) => self.status_message = status,
        }
    }

    fn push_log(&mut self, line: String) {
        self.log_messages.push(line);
        let overflow = self.log_messages.len().saturating_sub(100);
        if overflow > 0 {
            self.log_messages.drain(0..overflow);
        }
    }

    // -- View -----------------------------------------------------------

    fn view(&self) -> Element<'_, Message> {
        column![
            self.header(),
            row![
                container(self.left_panel())
                    .width(Length::Fixed(400.0))
                    .padding(8),
                container(self.right_panel())
                    .width(Length::Fill)
                    .padding(8),
            ]
            .spacing(16)
            .height(Length::Fill),
        ]
        .into()
    }

    fn header(&self) -> Element<'_, Message> {
        let title = column![
            text("FROST MPC Wallet").size(24),
            text("Native Edition").size(12).color(muted()),
        ]
        .spacing(2);

        container(
            row![
                title,
                Space::new().width(Length::Fill),
                column![
                    text(format!("Device: {}", self.device_id)).size(13),
                    text(&self.status_message).size(12).color(accent()),
                ]
                .spacing(2)
                .align_x(iced::Alignment::End),
            ]
            .align_y(iced::Alignment::Center)
            .padding(16),
        )
        .width(Length::Fill)
        .into()
    }

    fn left_panel(&self) -> Element<'_, Message> {
        scrollable(
            column![
                self.connection_status_view(),
                self.wallet_selector_view(),
                self.offline_mode_view(),
                self.session_list_view(),
            ]
            .spacing(16),
        )
        .into()
    }

    fn right_panel(&self) -> Element<'_, Message> {
        let tabs = row![
            tab_button("Operations", Tab::Operations, self.active_tab),
            tab_button("Logs", Tab::Logs, self.active_tab),
            tab_button("Settings", Tab::Settings, self.active_tab),
        ]
        .spacing(8);

        let body: Element<'_, Message> = match self.active_tab {
            Tab::Operations => self.operations_tab(),
            Tab::Logs => self.logs_tab(),
            Tab::Settings => self.settings_tab(),
        };

        // The signing confirm "modal" is rendered inline at the top of
        // the right panel when a request is pending (Iced has no native
        // overlay stacking in this layout, so we surface it prominently).
        let mut content = column![self.dkg_progress_view(), tabs].spacing(16);
        if self.has_signing_request {
            content = content.push(self.signing_modal_view());
        }
        content = content.push(body);

        scrollable(content).into()
    }

    // ---- Components (formerly the .slint component files) ----

    /// `components/connection_status.slint`
    fn connection_status_view(&self) -> Element<'_, Message> {
        let ws = status_dot("WebSocket", self.websocket_connected);
        let rtc = status_dot("WebRTC", self.webrtc_connected);
        let mesh = text(format!(
            "Mesh peers: {} · mode: {}",
            self.mesh_connections.len(),
            self.operation_mode
        ))
        .size(12)
        .color(muted());

        card(
            "Connection",
            column![ws, rtc, mesh].spacing(6).into(),
        )
    }

    /// `components/wallet_selector.slint`
    fn wallet_selector_view(&self) -> Element<'_, Message> {
        let mut list = Column::new().spacing(6);
        if self.wallets.is_empty() {
            list = list.push(text("No wallets — create or import one.").size(12).color(muted()));
        } else {
            for (i, w) in self.wallets.iter().enumerate() {
                let selected = i == self.active_wallet_index;
                let label = format!(
                    "{} · {} · {} · {}",
                    w.name,
                    w.chain,
                    w.threshold,
                    short_addr(&w.address),
                );
                let mut b = button(text(label).size(13))
                    .width(Length::Fill)
                    .on_press(Message::WalletSelected(i));
                if selected {
                    b = b.style(button::primary);
                } else {
                    b = b.style(button::secondary);
                }
                list = list.push(b);
            }
        }

        let create_row = row![
            text_input("New wallet name", &self.new_wallet_name)
                .on_input(Message::NewWalletNameChanged)
                .width(Length::Fill),
            button(text("Create")).on_press(Message::CreateWallet),
        ]
        .spacing(8);

        let io_row = row![
            button(text("Import")).on_press(Message::ImportWallet),
            button(text("Export"))
                .on_press_maybe(self.has_keystore.then_some(Message::ExportWallet)),
        ]
        .spacing(8);

        card(
            "Wallets",
            column![list, create_row, io_row].spacing(10).into(),
        )
    }

    /// `components/offline_mode.slint`
    fn offline_mode_view(&self) -> Element<'_, Message> {
        let toggle = button(text(if self.offline_enabled {
            "Disable offline mode"
        } else {
            "Enable offline mode"
        }))
        .on_press(Message::ToggleOfflineMode);

        let detected = text(format!(
            "SD card: {}",
            if self.sd_card_detected { "detected" } else { "not detected" }
        ))
        .size(12)
        .color(muted());

        let sd_row = row![
            button(text("Export →SD")).on_press(Message::ExportToSdCard),
            button(text("Import ←SD")).on_press(Message::ImportFromSdCard),
            button(text("Clear SD")).on_press(Message::ClearSdCard),
        ]
        .spacing(8);

        card(
            "Offline mode",
            column![toggle, detected, sd_row].spacing(8).into(),
        )
    }

    /// `components/session_list.slint`
    fn session_list_view(&self) -> Element<'_, Message> {
        let mut list = Column::new().spacing(6);
        if self.available_sessions.is_empty() {
            list = list.push(text("No sessions available.").size(12).color(muted()));
        } else {
            for s in &self.available_sessions {
                let label = format!(
                    "{} · {}/{} · {} · {}",
                    s.session_id,
                    s.threshold.0,
                    s.threshold.1,
                    s.participants.len(),
                    session_status_str(&s.status),
                );
                let join = Message::JoinSessionIdChanged(s.session_id.clone());
                list = list.push(
                    row![
                        text(label).size(12).width(Length::Fill),
                        button(text("Join").size(12)).on_press(join),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
            }
        }

        let active = match &self.active_session {
            Some(s) => text(format!("Active: {} ({})", s.session_id, session_status_str(&s.status)))
                .size(12)
                .color(accent()),
            None => text("No active session.").size(12).color(muted()),
        };

        let join_row = row![
            text_input("Session id to join", &self.join_session_id)
                .on_input(Message::JoinSessionIdChanged)
                .width(Length::Fill),
            button(text("Join")).on_press(Message::JoinSession),
        ]
        .spacing(8);

        let controls = row![
            button(text("Create session")).on_press(Message::CreateSession),
            button(text("Leave")).on_press(Message::LeaveSession),
            button(text("Refresh")).on_press(Message::RefreshSessions),
        ]
        .spacing(8);

        card(
            "Sessions",
            column![list, active, join_row, controls].spacing(10).into(),
        )
    }

    /// `components/dkg_progress.slint`
    fn dkg_progress_view(&self) -> Element<'_, Message> {
        let header = if self.dkg_active {
            text(format!(
                "DKG in progress — round {} ({:.0}%)",
                self.dkg_current_round,
                self.dkg_progress * 100.0
            ))
            .color(accent())
        } else {
            text("DKG idle").color(muted())
        };

        let session_line = match &self.active_session {
            Some(s) => text(format!(
                "Session {} · {}/{}",
                s.session_id, s.threshold.0, s.threshold.1
            ))
            .size(12)
            .color(muted()),
            None => text("No active DKG session.").size(12).color(muted()),
        };

        let bar = iced::widget::progress_bar(0.0..=1.0, self.dkg_progress);

        let abort = button(text("Abort DKG"))
            .style(button::danger)
            .on_press_maybe(self.dkg_active.then_some(Message::AbortDkg));

        card(
            "DKG progress",
            column![header.size(14), session_line, bar, abort]
                .spacing(8)
                .into(),
        )
    }

    fn operations_tab(&self) -> Element<'_, Message> {
        let start = button(text("Start DKG")).on_press_maybe(
            (self.active_session.is_some() && !self.dkg_active).then_some(Message::StartDkg),
        );

        card(
            "Operations",
            column![
                text("DKG").size(14),
                start,
                Space::new().height(8),
                text("Signing").size(14),
                text("Use the Settings tab to request a signature.")
                    .size(12)
                    .color(muted()),
            ]
            .spacing(8)
            .into(),
        )
    }

    fn logs_tab(&self) -> Element<'_, Message> {
        let mut col = Column::new().spacing(2);
        if self.log_messages.is_empty() {
            col = col.push(text("No log messages yet.").size(11).color(muted()));
        } else {
            for line in &self.log_messages {
                col = col.push(text(line).size(11).color(faint()));
            }
        }
        card("Logs", scrollable(col).height(Length::Fixed(360.0)).into())
    }

    fn settings_tab(&self) -> Element<'_, Message> {
        let network = card(
            "Network",
            column![
                text("Signal server").size(12).color(muted()),
                row![
                    text_input("wss://…", &self.ws_url)
                        .on_input(Message::WsUrlChanged)
                        .width(Length::Fill),
                    button(text("Connect")).on_press(Message::ConnectWebsocket),
                ]
                .spacing(8),
            ]
            .spacing(8)
            .into(),
        );

        let keystore = card(
            "Keystore (encrypted import/export)",
            column![
                text_input("Keystore password", &self.dkg_password)
                    .secure(true)
                    .on_input(Message::KeystorePasswordChanged),
                row![
                    button(text("Import with password…")).on_press(Message::ImportWallet),
                    button(text("Export with password…"))
                        .on_press_maybe(self.has_keystore.then_some(Message::ExportWallet)),
                ]
                .spacing(8),
            ]
            .spacing(8)
            .into(),
        );

        let can_sign = self.has_keystore && self.signing_state == "idle";
        let last_sig: Element<'_, Message> = if self.last_signature.is_empty() {
            Space::new().into()
        } else {
            text(format!("Last signature: {}", short_addr(&self.last_signature)))
                .size(11)
                .color(muted())
                .into()
        };
        let signing = card(
            "Sign message (MPC)",
            column![
                text_input("Message hex (0x… or raw hex)", &self.sign_hex)
                    .on_input(Message::SignHexChanged),
                text_input("Chain (ethereum / solana / …)", &self.sign_chain)
                    .on_input(Message::SignChainChanged),
                button(text("Request signing"))
                    .on_press_maybe(can_sign.then_some(Message::SignMessage)),
                last_sig,
            ]
            .spacing(8)
            .into(),
        );

        column![network, keystore, signing].spacing(16).into()
    }

    /// The signing confirm modal (Slint `if has_signing_request`).
    fn signing_modal_view(&self) -> Element<'_, Message> {
        let label: Element<'_, Message> = if self.signing_label.is_empty() {
            Space::new().into()
        } else {
            text(&self.signing_label).size(13).into()
        };

        let awaiting = self.signing_state == "awaiting_approval";
        let approve_label = if awaiting { "Approve & Sign" } else { "Signing…" };

        card(
            "Confirm signing request",
            column![
                label,
                text(format!("Chain: {}", self.signing_chain)).size(12).color(muted()),
                text("Message:").size(12).color(muted()),
                text(&self.signing_message_preview).size(11),
                text(format!("Phase: {}", self.signing_state)).size(11).color(faint()),
                row![
                    button(text("Reject"))
                        .style(button::danger)
                        .on_press(Message::RejectSigning),
                    Space::new().width(Length::Fill),
                    button(text(approve_label))
                        .style(button::primary)
                        .on_press_maybe(awaiting.then_some(Message::ApproveSigning)),
                ]
                .spacing(12),
            ]
            .spacing(10)
            .into(),
        )
    }
}

// ---------------------------------------------------------------------------
// The UICallback → UI bridge
// ---------------------------------------------------------------------------

/// Holds the receiver end of the UICallback channel between `State::new`
/// (which creates it) and `ui_bridge` (the subscription worker that
/// consumes it). A `OnceLock`-style slot would work too, but the
/// subscription closure can't be `FnOnce`-move a captured receiver into a
/// `'static` stream, so we stash it here and take it out on first run.
static BRIDGE_RX: StdMutex<Option<tokio::sync::mpsc::UnboundedReceiver<UiEvent>>> =
    StdMutex::new(None);

/// Subscription worker: turns the mpsc receiver into a `Stream<Message>`.
/// `Subscription::run` calls this once and keeps the resulting stream
/// alive for the whole application lifetime.
fn ui_bridge() -> impl iced::futures::Stream<Item = Message> {
    let rx = BRIDGE_RX
        .lock()
        .expect("bridge mutex")
        .take()
        .expect("ui_bridge started before State::new, or started twice");

    stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|event| (Message::Ui(event), rx))
    })
}

/// Wrap a fallible async core action into a `Task` that reports the
/// outcome via `Message::ActionDone`.
fn action<F>(fut: F) -> Task<Message>
where
    F: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    Task::perform(fut, Message::ActionDone)
}

// ---------------------------------------------------------------------------
// Small view helpers
// ---------------------------------------------------------------------------

fn card<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(
        column![text(title).size(16), body].spacing(10),
    )
    .padding(16)
    .width(Length::Fill)
    .style(container::rounded_box)
    .into()
}

fn tab_button(label: &str, tab: Tab, active: Tab) -> Element<'_, Message> {
    let mut b = button(text(label)).on_press(Message::TabSelected(tab));
    b = if tab == active {
        b.style(button::primary)
    } else {
        b.style(button::secondary)
    };
    b.into()
}

fn status_dot(label: &str, connected: bool) -> Element<'_, Message> {
    let dot = if connected { "●" } else { "○" };
    let color = if connected { accent() } else { muted() };
    row![
        text(dot).color(color),
        text(format!(
            "{label}: {}",
            if connected { "connected" } else { "disconnected" }
        ))
        .size(13),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn short_addr(addr: &str) -> String {
    if addr.len() > 20 {
        format!("{}…{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

fn session_status_str(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Waiting => "waiting",
        SessionStatus::InProgress => "in_progress",
        SessionStatus::Completed => "completed",
        SessionStatus::Failed => "failed",
    }
}

fn accent() -> Color {
    Color::from_rgb(0.063, 0.725, 0.506) // #10B981
}

fn muted() -> Color {
    Color::from_rgb(0.58, 0.64, 0.72) // #94A3B8
}

fn faint() -> Color {
    Color::from_rgb(0.80, 0.84, 0.88) // #CBD5E1
}
