//! DKG Progress Component - Real-time DKG status display
//!
//! Professional component for displaying the progress of the Distributed Key Generation
//! process in online mode with WebRTC mesh networking.

use crate::elm::components::{Id, UserEvent, MpcWalletComponent};
use crate::elm::message::{Message, DKGRound};

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::event::{Event, Key, KeyEvent};
use ratatui::layout::{Rect, Constraint, Direction, Layout, Alignment};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, Gauge, List, ListItem};
use tuirealm::component::{AppComponent, Component};
use tuirealm::ratatui::Frame;
use tuirealm::props::Props;
use tuirealm::state::{State, StateValue};

/// What ceremony this progress component is rendering. Drives three
/// pieces of behavior that diverge between DKG and signing:
///
/// 1. **Title** — `🔐 DKG Progress - Online Mode` vs
///    `🖊️ Signing Progress - Online Mode`.
/// 2. **Expected-other-participants math** — DKG genuinely needs every
///    party (`total_participants - 1` others), but a t-of-n signing
///    ceremony only needs `threshold - 1` others to co-sign. Without
///    this distinction the screen waited on all 3 peers in a 2-of-3
///    signing flow, which would never advance.
/// 3. **Optional chain row** — only signing has a meaningful chain
///    label (joiners pick it up from
///    `SessionType::Signing.blockchain`; creators fall back to
///    `wallet_state.curve_type`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Ceremony {
    #[default]
    Dkg,
    Signing {
        /// Blockchain or curve identifier rendered in the header
        /// ("ethereum", "secp256k1", ...). `None` only on the rare
        /// path where neither the session nor the wallet has any
        /// curve hint, which mostly never happens at runtime.
        chain: Option<String>,
    },
}

/// Participant status in the DKG process
#[derive(Debug, Clone)]
pub struct ParticipantInfo {
    pub device_id: String,
    pub status: ParticipantStatus,
    pub round_progress: DKGRound,
    pub is_connected: bool,
    pub webrtc_connected: bool,  // WebRTC connection state
    pub data_channel_open: bool, // Data channel state
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParticipantStatus {
    Waiting,
    WebRTCConnecting,
    DataChannelOpen,
    MeshReady,
    Round1Complete,
    Round2Complete,
    Completed,
    Failed(String),
}

/// Professional DKG progress component
#[derive(Debug, Clone)]
pub struct DKGProgressComponent {
    props: Props,
    session_id: String,
    total_participants: u16,
    threshold: u16,
    participants: Vec<ParticipantInfo>,
    current_round: DKGRound,
    progress_percentage: f64,
    error_message: Option<String>,
    focused: bool,
    selected_action: usize, // 0 = Cancel, 1 = Copy Session ID
    websocket_connected: bool, // Track WebSocket connection status
    mesh_ready_count: usize,  // Track how many participants are mesh-ready
    all_data_channels_open: bool, // Track if all data channels are open
    /// What ceremony this progress bar is tracking — drives the title,
    /// the chain row in the header, and the "expected other
    /// participants" denominator (total-1 for DKG, threshold-1 for
    /// signing). Reuses the same layout + participant mesh rendering
    /// without a separate component file.
    ceremony: Ceremony,
}

impl Default for DKGProgressComponent {
    fn default() -> Self {
        Self::new("DKG-00000000".to_string(), 3, 2)
    }
}

impl DKGProgressComponent {
    pub fn new(session_id: String, total_participants: u16, threshold: u16) -> Self {
        Self {
            props: Props::default(),
            session_id,
            total_participants,
            threshold,
            participants: vec![],
            current_round: DKGRound::Initialization,
            progress_percentage: 0.0,
            error_message: None,
            focused: false,
            selected_action: 0,
            websocket_connected: false, // Default to disconnected
            mesh_ready_count: 0,
            all_data_channels_open: false,
            ceremony: Ceremony::default(),
        }
    }

    /// Override the default `Ceremony::Dkg` for this mount — used by
    /// the signing flow's mount site so the title reads
    /// `🖊️ Signing Progress - Online Mode`, the chain row appears in
    /// the header, and the participant-expectation math switches from
    /// `total - 1` to `threshold - 1` (a 2-of-3 signing only needs 1
    /// other co-signer; without this swap the screen would forever
    /// wait on the 3rd peer that signing intentionally does not need).
    pub fn set_ceremony(&mut self, ceremony: Ceremony) {
        self.ceremony = ceremony;
    }

    /// How many OTHER participants this ceremony needs. DKG requires
    /// every party (`total - 1` others); a t-of-n signing only needs
    /// `threshold - 1` others. Single source of truth — used for the
    /// P2P status line, the placeholder roster, the mesh-ready
    /// status line, and the all-channels-open check inside
    /// `update_webrtc_status`.
    fn expected_other_participants(&self) -> usize {
        let denom = match self.ceremony {
            Ceremony::Dkg => self.total_participants,
            Ceremony::Signing { .. } => self.threshold,
        };
        denom.saturating_sub(1) as usize
    }

    /// Title text for the outer block — branches on ceremony.
    fn ceremony_title(&self) -> &'static str {
        match self.ceremony {
            Ceremony::Dkg => "🔐 DKG",
            Ceremony::Signing { .. } => "🖊️  Signing",
        }
    }
    
    /// Set WebSocket connection status
    pub fn set_websocket_connected(&mut self, connected: bool) {
        self.websocket_connected = connected;
    }
    
    /// Set selected action (0 = Cancel DKG, 1 = Copy Session ID)
    pub fn set_selected_action(&mut self, action: usize) {
        self.selected_action = action;
    }
    
    /// Update the session information
    pub fn set_session_info(&mut self, session_id: String, total: u16, threshold: u16) {
        self.session_id = session_id;
        self.total_participants = total;
        self.threshold = threshold;
    }
    
    /// Add or update a participant
    pub fn update_participant(&mut self, device_id: String, status: ParticipantStatus) {
        if let Some(participant) = self.participants.iter_mut().find(|p| p.device_id == device_id) {
            participant.status = status;
        } else {
            self.participants.push(ParticipantInfo {
                device_id,
                status,
                round_progress: DKGRound::Initialization,
                is_connected: true,
                webrtc_connected: false,
                data_channel_open: false,
            });
        }
        self.update_progress();
    }
    
    /// Update the current DKG round
    pub fn set_round(&mut self, round: DKGRound) {
        self.current_round = round;
        self.update_progress();
    }

    /// Update WebRTC connection status for a participant
    pub fn update_webrtc_status(&mut self, device_id: String, webrtc_connected: bool, data_channel_open: bool) {
        if let Some(participant) = self.participants.iter_mut().find(|p| p.device_id == device_id) {
            participant.webrtc_connected = webrtc_connected;
            participant.data_channel_open = data_channel_open;

            // Update status based on connection state
            if data_channel_open {
                participant.status = ParticipantStatus::DataChannelOpen;
            } else if webrtc_connected {
                participant.status = ParticipantStatus::WebRTCConnecting;
            }
        } else {
            // Add new participant if not exists
            self.participants.push(ParticipantInfo {
                device_id,
                status: if data_channel_open {
                    ParticipantStatus::DataChannelOpen
                } else if webrtc_connected {
                    ParticipantStatus::WebRTCConnecting
                } else {
                    ParticipantStatus::Waiting
                },
                round_progress: DKGRound::Initialization,
                is_connected: webrtc_connected || data_channel_open,
                webrtc_connected,
                data_channel_open,
            });
        }

        // Check if all data channels are open (comparing against the
        // ceremony-specific other-participant count: total-1 for DKG,
        // threshold-1 for signing).
        let expected_other_participants = self.expected_other_participants();
        self.all_data_channels_open = self.participants.len() >= expected_other_participants &&
            self.participants.iter().all(|p| p.data_channel_open);

        // Update mesh_ready_count based on actual data channels open
        // Mesh is ready when we have all expected participants with open data channels
        if self.all_data_channels_open && self.participants.len() >= expected_other_participants {
            self.mesh_ready_count = expected_other_participants;
        } else {
            self.mesh_ready_count = 0;
        }
    }

    /// Update mesh status
    pub fn update_mesh_status(&mut self, ready_count: usize, all_connected: bool) {
        self.mesh_ready_count = ready_count;
        if all_connected {
            // Update all participants to MeshReady if they have data channels open
            for participant in &mut self.participants {
                if participant.data_channel_open {
                    participant.status = ParticipantStatus::MeshReady;
                }
            }
        }
    }
    
    /// Calculate overall progress
    fn update_progress(&mut self) {
        let connected = self.participants.len() as f64;
        let total = self.total_participants as f64;
        
        match self.current_round {
            DKGRound::Initialization => {
                // Initial setup
                self.progress_percentage = 5.0;
            }
            DKGRound::WaitingForParticipants => {
                // Progress based on participants joining
                self.progress_percentage = 5.0 + (connected / total) * 20.0;
            }
            DKGRound::Round1 => {
                // 25% base + progress through round 1
                let round1_complete = self.participants.iter()
                    .filter(|p| matches!(p.status, ParticipantStatus::Round1Complete | ParticipantStatus::Round2Complete | ParticipantStatus::Completed))
                    .count() as f64;
                self.progress_percentage = 25.0 + (round1_complete / total) * 35.0;
            }
            DKGRound::Round2 => {
                // 60% base + progress through round 2
                let round2_complete = self.participants.iter()
                    .filter(|p| matches!(p.status, ParticipantStatus::Round2Complete | ParticipantStatus::Completed))
                    .count() as f64;
                self.progress_percentage = 60.0 + (round2_complete / total) * 35.0;
            }
            DKGRound::Finalization => {
                self.progress_percentage = 95.0;
            }
            DKGRound::Complete => {
                self.progress_percentage = 100.0;
            }
        }
    }

    fn get_round_color(&self) -> Color {
        match self.current_round {
            DKGRound::Initialization => Color::Yellow,
            DKGRound::WaitingForParticipants => Color::Yellow,
            DKGRound::Round1 => Color::Cyan,
            DKGRound::Round2 => Color::Blue,
            DKGRound::Finalization => Color::Green,
            DKGRound::Complete => Color::LightGreen,
        }
    }
    
    fn get_status_symbol(status: &ParticipantStatus) -> &'static str {
        match status {
            ParticipantStatus::Waiting => "⏳",
            ParticipantStatus::WebRTCConnecting => "🔄",
            ParticipantStatus::DataChannelOpen => "📡",
            ParticipantStatus::MeshReady => "🔗",
            ParticipantStatus::Round1Complete => "✓",
            ParticipantStatus::Round2Complete => "✓✓",
            ParticipantStatus::Completed => "✅",
            ParticipantStatus::Failed(_) => "❌",
        }
    }
    
    fn get_status_color(status: &ParticipantStatus) -> Color {
        match status {
            ParticipantStatus::Waiting => Color::Gray,
            ParticipantStatus::WebRTCConnecting => Color::Yellow,
            ParticipantStatus::DataChannelOpen => Color::Cyan,
            ParticipantStatus::MeshReady => Color::Blue,
            ParticipantStatus::Round1Complete => Color::Cyan,
            ParticipantStatus::Round2Complete => Color::Blue,
            ParticipantStatus::Completed => Color::Green,
            ParticipantStatus::Failed(_) => Color::Red,
        }
    }
}

impl Component for DKGProgressComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Check if area is too small
        if area.width < 20 || area.height < 15 {
            // Render a simple message if space is insufficient
            let msg = Paragraph::new("Window too small")
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center);
            frame.render_widget(msg, area);
            return;
        }
        
        // Main container — title reflects the ceremony (DKG or signing).
        let title_text = format!(" {} Progress - Online Mode ", self.ceremony_title());
        let block = Block::default()
            .title(title_text)
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if self.focused { Color::Cyan } else { Color::Gray }));
        frame.render_widget(block.clone(), area);
        
        // Create inner area for content (accounting for borders)
        let inner_area = block.inner(area);
        
        // Use more flexible constraints
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),     // Header with session info (flexible)
                Constraint::Length(3),  // Progress bar
                Constraint::Min(5),     // Participants list (flexible)
                Constraint::Min(3),     // Actions/Status (flexible)
            ])
            .margin(1)
            .split(inner_area);
        
        // Safely render each section if chunk exists
        if chunks.len() >= 4 {
            // Header - Session Information
            self.render_header(frame, chunks[0]);
            
            // Progress Bar
            self.render_progress_bar(frame, chunks[1]);
            
            // Participants List
            self.render_participants(frame, chunks[2]);
            
            // Actions/Status
            self.render_actions(frame, chunks[3]);
        } else {
            // Fallback: render simple status if layout failed
            let msg = Paragraph::new(format!("DKG Session: {}\nParticipants: {}/{}", 
                self.session_id, self.participants.len(), self.total_participants))
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center);
            frame.render_widget(msg, inner_area);
        }
    }
    
    fn query<'a>(&'a self, attr: tuirealm::props::Attribute) -> Option<tuirealm::props::QueryResult<'a>> {
        self.props.get_for_query(attr)
    }
    
    fn attr(&mut self, attr: tuirealm::props::Attribute, value: tuirealm::props::AttrValue) {
        self.props.set(attr, value);
    }
    
    fn state(&self) -> tuirealm::state::State {
        State::Single(StateValue::String(self.session_id.clone()))
    }
    
    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(tuirealm::command::Direction::Left) => {
                if self.selected_action > 0 {
                    self.selected_action -= 1;
                    CmdResult::Changed(self.state())
                } else {
                    CmdResult::NoChange
                }
            }
            Cmd::Move(tuirealm::command::Direction::Right) => {
                if self.selected_action < 1 {
                    self.selected_action += 1;
                    CmdResult::Changed(self.state())
                } else {
                    CmdResult::NoChange
                }
            }
            Cmd::Submit => {
                if self.selected_action == 0 {
                    // Cancel DKG
                    CmdResult::Submit(State::Single(StateValue::String("cancel".to_string())))
                } else {
                    // Copy Session ID
                    CmdResult::Submit(State::Single(StateValue::String("copy".to_string())))
                }
            }
            _ => CmdResult::NoChange,
        }
    }
}

impl DKGProgressComponent {
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(2),
            ])
            .split(area);
        
        // Session ID with WebSocket status
        let ws_status = if self.websocket_connected {
            Span::styled("🟢 WebSocket Connected", Style::default().fg(Color::Green))
        } else {
            Span::styled("🔴 WebSocket Disconnected", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        };
        
        let session_text = vec![
            Line::from(vec![
                Span::styled("Session ID: ", Style::default().fg(Color::Gray)),
                Span::styled(&self.session_id, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("  |  "),
                ws_status,
            ]),
        ];
        let session_para = Paragraph::new(session_text)
            .alignment(Alignment::Center);
        frame.render_widget(session_para, chunks[0]);
        
        // Configuration row. For signing we suffix the resolved chain
        // identifier so the user can sanity-check what they're about
        // to put a signature on; DKG has no chain at this point in
        // the flow, so the row is just the threshold.
        let mut config_spans = vec![
            Span::styled("Configuration: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}-of-{} Threshold", self.threshold, self.total_participants),
                Style::default().fg(Color::Cyan),
            ),
        ];
        if let Ceremony::Signing { chain: Some(ref chain) } = self.ceremony {
            config_spans.push(Span::raw("  |  "));
            config_spans.push(Span::styled(
                "Chain: ",
                Style::default().fg(Color::Gray),
            ));
            config_spans.push(Span::styled(
                chain.clone(),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ));
        }
        let config_para = Paragraph::new(vec![Line::from(config_spans)])
            .alignment(Alignment::Center);
        frame.render_widget(config_para, chunks[1]);
        
        // Current Round
        let round_text = vec![
            Line::from(vec![
                Span::styled("Current Round: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{:?}", self.current_round),
                    Style::default().fg(self.get_round_color()).add_modifier(Modifier::BOLD)
                ),
            ]),
        ];
        let round_para = Paragraph::new(round_text)
            .alignment(Alignment::Center);
        frame.render_widget(round_para, chunks[2]);
        
        // Participants Count with WebRTC details
        let data_channels_open = self.participants.iter().filter(|p| p.data_channel_open).count();
        let webrtc_connected = self.participants.iter().filter(|p| p.webrtc_connected).count();

        // For DKG we need every party (total - 1 others); for signing
        // only `threshold - 1` others. Without this swap a 2-of-3 sign
        // ceremony would forever read "0/2" when only 1 peer is needed.
        let other_participants = self.expected_other_participants();

        let participants_text = vec![
            Line::from(vec![
                Span::styled("P2P Status: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("WebRTC: {}/{} | Channels: {}/{} | Mesh: {}/1",
                            webrtc_connected, other_participants,
                            data_channels_open, other_participants,
                            if self.mesh_ready_count >= other_participants { 1 } else { 0 }),
                    Style::default().fg(if self.all_data_channels_open {
                        Color::Green
                    } else if data_channels_open > 0 {
                        Color::Yellow
                    } else {
                        Color::Red
                    })
                ),
            ]),
        ];
        let participants_para = Paragraph::new(participants_text)
            .alignment(Alignment::Center);
        frame.render_widget(participants_para, chunks[3]);
    }
    
    fn render_progress_bar(&self, frame: &mut Frame, area: Rect) {
        let progress_label = format!(
            "Progress: {:.0}% - {}",
            self.progress_percentage,
            match self.current_round {
                DKGRound::Initialization => "Initializing protocol...",
                DKGRound::WaitingForParticipants => "Waiting for participants...",
                DKGRound::Round1 => "Generating commitments...",
                DKGRound::Round2 => "Exchanging shares...",
                DKGRound::Finalization => "Finalizing DKG...",
                DKGRound::Complete => "DKG complete!",
            }
        );
        
        // Ensure percentage is valid (0-100) before passing to Gauge
        let safe_percentage = if self.progress_percentage.is_nan() || self.progress_percentage.is_infinite() {
            0
        } else {
            self.progress_percentage.clamp(0.0, 100.0) as u16
        };
        
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(Style::default().fg(self.get_round_color()).bg(Color::Black))
            .percent(safe_percentage)
            .label(progress_label);
        
        frame.render_widget(gauge, area);
    }
    
    fn render_participants(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Participants ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));
        
        let items: Vec<ListItem> = self.participants
            .iter()
            .map(|p| {
                let status_symbol = Self::get_status_symbol(&p.status);
                let status_color = Self::get_status_color(&p.status);
                // Show detailed connection status
                let connection_symbol = if p.data_channel_open {
                    "🟢"  // Green circle for data channel open
                } else if p.webrtc_connected {
                    "🟡"  // Yellow circle for WebRTC connected
                } else {
                    "🔴"  // Red circle for disconnected
                };
                
                let content = Line::from(vec![
                    Span::raw(format!("  {} ", connection_symbol)),
                    Span::styled(&p.device_id, Style::default().fg(Color::White)),
                    Span::raw(" - "),
                    Span::styled(status_symbol, Style::default().fg(status_color)),
                    Span::raw(" "),
                    Span::styled(
                        match &p.status {
                            ParticipantStatus::Waiting => "Waiting".to_string(),
                            ParticipantStatus::WebRTCConnecting => "WebRTC Connecting".to_string(),
                            ParticipantStatus::DataChannelOpen => "Channel Open".to_string(),
                            ParticipantStatus::MeshReady => "Mesh Ready".to_string(),
                            ParticipantStatus::Round1Complete => "Round 1 Done".to_string(),
                            ParticipantStatus::Round2Complete => "Round 2 Done".to_string(),
                            ParticipantStatus::Completed => "Completed".to_string(),
                            ParticipantStatus::Failed(e) => format!("Failed: {}", e),
                        },
                        Style::default().fg(status_color)
                    ),
                ]);
                
                ListItem::new(content)
            })
            .collect();
        
        // Add placeholder slots for missing participants (excluding self).
        // Uses the ceremony-aware count so a 2-of-3 sign run only shows
        // ONE waiting slot, not two — signing genuinely doesn't need
        // the third device, and the placeholder text was the most
        // visible "waiting for participant 2..." misdirection.
        let mut all_items = items;
        let expected_other_participants = self.expected_other_participants();
        for i in self.participants.len()..expected_other_participants {
            all_items.push(ListItem::new(Line::from(vec![
                Span::raw("  ⏳ "),
                Span::styled(
                    format!("Waiting for participant {}...", i + 1),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
                ),
            ])));
        }
        
        let list = List::new(all_items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        frame.render_widget(list, area);
    }
    
    fn render_actions(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
            ])
            .split(area);
        
        // Error message or status
        if let Some(ref error) = self.error_message {
            let error_text = vec![
                Line::from(vec![
                    Span::styled("⚠️ Error: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::styled(error, Style::default().fg(Color::Red)),
                ]),
            ];
            let error_para = Paragraph::new(error_text)
                .alignment(Alignment::Center);
            frame.render_widget(error_para, chunks[0]);
        } else {
            // Check WebSocket connection first
            let status_text = if !self.websocket_connected {
                "❌ WebSocket disconnected - Cannot proceed without signal server".to_string()
            } else {
                match self.current_round {
                    DKGRound::Initialization => {
                        if self.all_data_channels_open {
                            "🟢 All data channels established! Ready for DKG".to_string()
                        } else if self.participants.iter().any(|p| p.data_channel_open) {
                            "🟡 Establishing data channels...".to_string()
                        } else {
                            "📡 Establishing WebRTC connections...".to_string()
                        }
                    },
                    DKGRound::WaitingForParticipants => {
                        let expected_other_participants = self.expected_other_participants();
                        if self.mesh_ready_count == expected_other_participants {
                            match self.ceremony {
                                Ceremony::Dkg => "🟢 Mesh fully connected! Starting DKG...".to_string(),
                                Ceremony::Signing { .. } => "🟢 Mesh ready! Starting signing ceremony...".to_string(),
                            }
                        } else {
                            format!("⏳ Mesh formation: {}/{} ready", self.mesh_ready_count, expected_other_participants)
                        }
                    },
                    DKGRound::Round1 => "🔄 Round 1: Generating and broadcasting commitments...".to_string(),
                    DKGRound::Round2 => "🔄 Round 2: Generating and distributing shares...".to_string(),
                    DKGRound::Finalization => "🔄 Finalizing key generation...".to_string(),
                    DKGRound::Complete => "🎉 DKG complete! Wallet created — press Esc to return.".to_string(),
                }
            };

            let status_color = if !self.websocket_connected {
                Color::Red
            } else {
                self.get_round_color()
            };

            let status_para = Paragraph::new(status_text.as_str())
                .style(Style::default().fg(status_color))
                .alignment(Alignment::Center);
            frame.render_widget(status_para, chunks[0]);
        }
        
        // Action buttons
        let cancel_style = if self.selected_action == 0 {
            Style::default().fg(Color::Black).bg(Color::Red)
        } else {
            Style::default().fg(Color::Red)
        };
        
        let copy_style = if self.selected_action == 1 {
            Style::default().fg(Color::Black).bg(Color::Green)
        } else {
            Style::default().fg(Color::Green)
        };
        
        let actions_text = vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(" Cancel DKG ", cancel_style),
                Span::raw("    "),
                Span::styled(" Copy Session ID ", copy_style),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("←→", Style::default().fg(Color::DarkGray)),
                Span::raw(" Switch • "),
                Span::styled("Enter", Style::default().fg(Color::DarkGray)),
                Span::raw(" Select • "),
                Span::styled("Esc", Style::default().fg(Color::DarkGray)),
                Span::raw(" Back"),
            ]),
        ];
        
        let actions_para = Paragraph::new(actions_text)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::DarkGray))
            );
        frame.render_widget(actions_para, chunks[1]);
    }
}

impl AppComponent<Message, UserEvent> for DKGProgressComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Message> {
        tracing::debug!("🎮 DKGProgress received event: {:?}", event);
        
        match event {
            // Intentionally do NOT consume Left/Right here — return `None` so
            // the key bubbles up to the global keymap, which dispatches
            // `Message::ScrollLeft` / `Message::ScrollRight`. Those handlers
            // update `model.ui_state.selected_indices[DKGProgress]`, which the
            // next remount reads via `set_selected_action`. If we consume here
            // and mutate only our local `self.selected_action`, the very next
            // `ForceRemount` resets it to whatever's in the model (0), which
            // is exactly the "right arrow doesn't work" bug.
            Event::Keyboard(KeyEvent { code: Key::Left, .. })
            | Event::Keyboard(KeyEvent { code: Key::Right, .. }) => None,
            Event::Keyboard(KeyEvent {
                code: Key::Enter,
                ..
            }) => {
                if self.selected_action == 0 {
                    // Cancel DKG
                    Some(Message::CancelDKG)
                } else {
                    // Copy Session ID to clipboard (or show notification)
                    Some(Message::ShowNotification {
                        kind: crate::elm::model::NotificationKind::Info,
                        text: format!("Session ID copied: {}", self.session_id),
                    })
                }
            }
            Event::Keyboard(KeyEvent {
                code: Key::Esc,
                ..
            }) => {
                Some(Message::NavigateBack)
            }
            Event::User(UserEvent::FocusGained) => {
                self.focused = true;
                None
            }
            Event::User(UserEvent::FocusLost) => {
                self.focused = false;
                None
            }
            _ => None,
        }
    }
}

impl MpcWalletComponent for DKGProgressComponent {
    fn id(&self) -> Id {
        Id::DKGProgress
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn on_focus(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dkg_expects_total_minus_one_others() {
        // 2-of-3 DKG: still needs all 3 parties (= 2 others) to
        // generate the key share.
        let component = DKGProgressComponent::new("session".into(), 3, 2);
        assert_eq!(component.expected_other_participants(), 2);
        assert_eq!(component.ceremony_title(), "🔐 DKG");
    }

    #[test]
    fn signing_expects_threshold_minus_one_others() {
        // The reported bug: a 2-of-3 sign run shouldn't wait on a
        // third peer. Threshold = 2 → 1 other co-signer required.
        let mut component = DKGProgressComponent::new("session".into(), 3, 2);
        component.set_ceremony(Ceremony::Signing { chain: None });
        assert_eq!(
            component.expected_other_participants(),
            1,
            "2-of-3 signing must wait on threshold-1 = 1 peer, not total-1 = 2",
        );
        assert_eq!(component.ceremony_title(), "🖊️  Signing");
    }

    #[test]
    fn signing_carries_chain_label_through_ceremony() {
        let mut component = DKGProgressComponent::new("session".into(), 3, 2);
        component.set_ceremony(Ceremony::Signing {
            chain: Some("ethereum".into()),
        });
        match &component.ceremony {
            Ceremony::Signing { chain } => assert_eq!(chain.as_deref(), Some("ethereum")),
            other => panic!("expected Signing ceremony, got {:?}", other),
        }
    }

    #[test]
    fn placeholder_count_uses_threshold_for_signing() {
        // Indirect check: when no peers have been added, the
        // expected-others count drives both the placeholder roster
        // length AND the P2P denominator. We assert the count itself
        // here since render_participants reads from a Frame we don't
        // have in unit tests.
        let mut component = DKGProgressComponent::new("session".into(), 5, 3);
        assert_eq!(
            component.expected_other_participants(),
            4,
            "DKG with total=5 wants 4 others",
        );
        component.set_ceremony(Ceremony::Signing { chain: None });
        assert_eq!(
            component.expected_other_participants(),
            2,
            "3-of-5 signing wants threshold-1 = 2 others",
        );
    }

    #[test]
    fn larger_threshold_signing_correctly_scales() {
        // 4-of-7: signing needs 3 others. Confirms the formula isn't
        // accidentally hard-coded to 1 for the 2-of-3 case.
        let mut component = DKGProgressComponent::new("session".into(), 7, 4);
        component.set_ceremony(Ceremony::Signing { chain: None });
        assert_eq!(component.expected_other_participants(), 3);
    }

    #[test]
    fn one_of_one_signing_needs_zero_other_participants() {
        // Edge case: solo signing, no peer mesh required.
        let mut component = DKGProgressComponent::new("session".into(), 1, 1);
        component.set_ceremony(Ceremony::Signing { chain: None });
        assert_eq!(component.expected_other_participants(), 0);
    }
}