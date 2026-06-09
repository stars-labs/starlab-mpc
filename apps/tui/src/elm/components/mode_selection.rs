//! Mode Selection Component - Online vs Offline
//!
//! Professional component explaining the differences between online and offline modes

use crate::elm::components::{Id, UserEvent, MpcWalletComponent};
use crate::elm::message::Message;

use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::event::Event;
use ratatui::layout::{Rect, Constraint, Direction as LayoutDirection, Layout, Alignment};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, Wrap};
use tuirealm::component::{AppComponent, Component};
use tuirealm::ratatui::Frame;
use tuirealm::props::Props;
use tuirealm::state::{State, StateValue};

/// Professional mode selection component
#[derive(Debug, Clone)]
pub struct ModeSelectionComponent {
    props: Props,
    selected: usize,
    focused: bool,
    websocket_connected: bool,
    websocket_url: String,
}

#[derive(Debug, Clone)]
struct OperationMode {
    name: &'static str,
    icon: &'static str,
    security_level: &'static str,
    speed: &'static str,
    requirements: Vec<&'static str>,
    use_cases: Vec<&'static str>,
    pros: Vec<&'static str>,
    cons: Vec<&'static str>,
}

impl Default for ModeSelectionComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeSelectionComponent {
    pub fn new() -> Self {
        Self {
            props: Props::default(),
            selected: 0,
            focused: false,
            websocket_connected: false,
            websocket_url: String::new(),
        }
    }

    pub fn with_selected(selected: usize) -> Self {
        Self {
            props: Props::default(),
            selected,
            focused: false,
            websocket_connected: false,
            websocket_url: String::new(),
        }
    }

    pub fn set_websocket_connected(&mut self, connected: bool) {
        self.websocket_connected = connected;
    }

    pub fn set_websocket_url(&mut self, url: String) {
        self.websocket_url = url;
    }

    /// Whether the currently-selected mode can be confirmed.
    /// Online mode requires an active signaling WebSocket.
    pub fn can_submit(&self) -> bool {
        self.selected != 0 || self.websocket_connected
    }
    
    fn get_modes(&self) -> Vec<OperationMode> {
        vec![
            OperationMode {
                name: "Online Mode (Hot Wallet)",
                icon: "🌐",
                security_level: "High Security",
                speed: "Real-time Operations",
                requirements: vec![
                    "• Active internet connection",
                    "• WebSocket server access (wss://panda.qzz.io)",
                    "• WebRTC capability for P2P mesh",
                    "• TLS 1.3 encryption support",
                ],
                use_cases: vec![
                    "• Daily trading operations",
                    "• Quick transaction signing",
                    "• Real-time DKG ceremonies",
                    "• Instant participant coordination",
                ],
                pros: vec![
                    "✅ Instant key generation (< 30 seconds)",
                    "✅ Real-time participant discovery",
                    "✅ Automatic session synchronization",
                    "✅ Live status updates",
                    "✅ Convenient for regular operations",
                ],
                cons: vec![
                    "⚠️ Requires network connectivity",
                    "⚠️ Vulnerable to network-level attacks",
                    "⚠️ Trust in signaling infrastructure needed",
                    "⚠️ Not suitable for highest-value assets",
                ],
            },
            OperationMode {
                name: "Offline Mode (Cold Wallet)",
                icon: "🔒",
                security_level: "Maximum Security",
                speed: "Manual Coordination",
                requirements: vec![
                    "• Air-gapped machines (network disabled)",
                    "• Removable storage media (SD cards/USB)",
                    "• Physical access to all participants",
                    "• Secure channels for data exchange",
                ],
                use_cases: vec![
                    "• Treasury management",
                    "• Cold storage operations",
                    "• High-value asset protection",
                    "• Regulatory compliance requirements",
                ],
                pros: vec![
                    "✅ Complete air-gap protection",
                    "✅ No network attack surface",
                    "✅ Verifiable at each step",
                    "✅ Meets strict compliance standards",
                    "✅ Maximum security for critical assets",
                ],
                cons: vec![
                    "⚠️ Slower coordination (hours/days)",
                    "⚠️ Requires physical media exchange",
                    "⚠️ Manual verification needed",
                    "⚠️ Less convenient for frequent use",
                ],
            },
        ]
    }
}

impl Component for ModeSelectionComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(5),   // Header
                Constraint::Length(3),   // WebSocket status banner
                Constraint::Min(0),      // Content
                Constraint::Length(4),   // Footer
            ])
            .margin(1)
            .split(area);

        // Header
        self.render_header(frame, chunks[0]);

        // WebSocket connection status banner
        self.render_ws_status(frame, chunks[1]);

        // Main content - split horizontally for side-by-side comparison
        let content_chunks = Layout::default()
            .direction(LayoutDirection::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(chunks[2]);

        // Render both modes side by side
        let modes = self.get_modes();
        for (i, mode) in modes.iter().enumerate() {
            self.render_mode(frame, content_chunks[i], mode, i == self.selected);
        }

        // Footer with controls
        self.render_footer(frame, chunks[3]);
    }
    
    fn query<'a>(&'a self, attr: tuirealm::props::Attribute) -> Option<tuirealm::props::QueryResult<'a>> {
        self.props.get_for_query(attr)
    }
    
    fn attr(&mut self, attr: tuirealm::props::Attribute, value: tuirealm::props::AttrValue) {
        self.props.set(attr, value);
    }
    
    fn state(&self) -> tuirealm::state::State {
        State::Single(StateValue::Usize(self.selected))
    }
    
    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(Direction::Left) | Cmd::Move(Direction::Up) => {
                self.selected = 0;
                CmdResult::Changed(self.state())
            }
            Cmd::Move(Direction::Right) | Cmd::Move(Direction::Down) => {
                self.selected = 1;
                CmdResult::Changed(self.state())
            }
            Cmd::Submit => CmdResult::Submit(self.state()),
            _ => CmdResult::NoChange,
        }
    }
}

impl ModeSelectionComponent {
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header_text = ["🔐 OPERATION MODE SELECTION (Step 1 of 3)",
            "",
            "Choose between Online (Hot) and Offline (Cold) wallet modes",
            "This decision affects security, convenience, and operational workflow"];
        
        let header = Paragraph::new(header_text.join("\n"))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Choose Your Security Model ")
                    .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            );
        frame.render_widget(header, area);
    }
    
    fn render_ws_status(&self, frame: &mut Frame, area: Rect) {
        let url_display = if self.websocket_url.is_empty() {
            "<no signaling server configured>".to_string()
        } else {
            self.websocket_url.clone()
        };

        let (text, color) = if self.websocket_connected {
            (
                format!("🟢 Connected to signaling server  ({})", url_display),
                Color::Green,
            )
        } else {
            (
                format!(
                    "🔴 Disconnected from signaling server  ({}) — Online mode unavailable until reconnected",
                    url_display
                ),
                Color::Red,
            )
        };

        let widget = Paragraph::new(text)
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(color))
                    .title(" 🔌 WebSocket Status ")
                    .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            );
        frame.render_widget(widget, area);
    }

    fn render_mode(&self, frame: &mut Frame, area: Rect, mode: &OperationMode, selected: bool) {
        let chunks = Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(3),   // Title
                Constraint::Length(2),   // Security & Speed
                Constraint::Length(6),   // Requirements
                Constraint::Length(6),   // Use Cases
                Constraint::Min(0),      // Pros & Cons
            ])
            .margin(1)
            .split(area);
        
        // Title (tag Online mode as unavailable when the signaling WebSocket is down)
        let is_online_mode = mode.name.starts_with("Online");
        let online_unavailable = is_online_mode && !self.websocket_connected;
        let title_text = if online_unavailable {
            format!("{} {}  [UNAVAILABLE — WebSocket down]", mode.icon, mode.name)
        } else {
            format!("{} {}", mode.icon, mode.name)
        };
        let title_color = if online_unavailable {
            Color::Red
        } else if selected {
            Color::Yellow
        } else {
            Color::White
        };
        let title = Paragraph::new(title_text)
            .style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);
        
        // Security & Speed badges
        let badges = Paragraph::new(format!("🛡️ {} | ⚡ {}", mode.security_level, mode.speed))
            .style(Style::default().fg(if selected { Color::Green } else { Color::Gray }))
            .alignment(Alignment::Center);
        frame.render_widget(badges, chunks[1]);
        
        // Requirements
        let req_text = format!("📋 Requirements:\n{}", mode.requirements.join("\n"));
        let requirements = Paragraph::new(req_text)
            .style(Style::default().fg(if selected { Color::Cyan } else { Color::DarkGray }))
            .wrap(Wrap { trim: true });
        frame.render_widget(requirements, chunks[2]);
        
        // Use Cases
        let use_case_text = format!("💼 Use Cases:\n{}", mode.use_cases.join("\n"));
        let use_cases = Paragraph::new(use_case_text)
            .style(Style::default().fg(if selected { Color::Magenta } else { Color::DarkGray }))
            .wrap(Wrap { trim: true });
        frame.render_widget(use_cases, chunks[3]);
        
        // Pros & Cons
        let pros_cons = format!(
            "Advantages:\n{}\n\nConsiderations:\n{}",
            mode.pros.join("\n"),
            mode.cons.join("\n")
        );
        let pros_cons_widget = Paragraph::new(pros_cons)
            .style(Style::default().fg(if selected { Color::White } else { Color::DarkGray }))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(if selected { BorderType::Thick } else { BorderType::Rounded })
                    .border_style(
                        Style::default().fg(if selected { Color::Yellow } else { Color::Gray })
                    )
            );
        frame.render_widget(pros_cons_widget, chunks[4]);
    }
    
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let selected_mode = if self.selected == 0 { "Online" } else { "Offline" };
        let blocked = !self.can_submit();

        let status_line = if blocked {
            "Selected: Online Mode — ⚠️ connect the signaling WebSocket to continue".to_string()
        } else {
            format!("Selected: {} Mode", selected_mode)
        };
        let controls_line = if blocked {
            "← → Switch Between Modes | Enter: disabled (WebSocket down) | Esc: Cancel"
        } else {
            "← → Switch Between Modes | Enter: Next Step | Esc: Cancel"
        };

        let footer_text = [status_line,
            "".to_string(),
            controls_line.to_string(),
            "💡 Tip: You can switch modes later, but it requires re-initialization".to_string()];

        let footer = Paragraph::new(footer_text.join("\n"))
            .style(
                Style::default()
                    .fg(if blocked { Color::Red } else { Color::Green })
                    .add_modifier(Modifier::ITALIC),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
        frame.render_widget(footer, area);
    }
}

impl AppComponent<Message, UserEvent> for ModeSelectionComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Message> {
        match event {
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

impl MpcWalletComponent for ModeSelectionComponent {
    fn id(&self) -> Id {
        Id::ModeSelection
    }
    
    fn is_visible(&self) -> bool {
        true
    }
    
    fn on_focus(&mut self, focused: bool) {
        self.focused = focused;
    }
}