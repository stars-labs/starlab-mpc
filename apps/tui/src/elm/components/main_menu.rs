//! Main menu screen — root navigation into wallet creation, join, sign, settings.

use crate::elm::components::{Id, MpcWalletComponent, UserEvent};
use crate::elm::message::Message;

use ratatui::layout::{Alignment, Constraint, Direction as LayoutDirection, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::Props;
use tuirealm::ratatui::Frame;
use tuirealm::state::{State, StateValue};

/// Professional main menu with enhanced styling
#[derive(Debug, Clone)]
pub struct MainMenu {
    props: Props,
    items: Vec<MenuItem>,
    selected: usize,
    focused: bool,
    wallet_count: usize,
    /// The safe-prime generator's state ("ECDSA setup" of the next wallet).
    ecdsa_setup: crate::protocal::ecdsa::PrimeStatus,
}

#[derive(Debug, Clone)]
struct MenuItem {
    icon: &'static str,
    label: String,
    description: String,
    enabled: bool,
    badge: Option<String>,
    priority: Priority,
}

#[derive(Debug, Clone)]
enum Priority {
    High,   // Primary actions - bright colors
    Medium, // Secondary actions - normal colors
    Low,    // Tertiary actions - muted colors
}

impl Default for MainMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MainMenu {
    pub fn new() -> Self {
        Self::with_wallet_count(0)
    }

    pub fn with_wallet_count(wallet_count: usize) -> Self {
        let mut items = vec![
            MenuItem {
                icon: "🆕",
                label: "Create New Wallet".to_string(),
                description: "Initialize new MPC wallet with DKG ceremony".to_string(),
                enabled: true,
                badge: Some("Primary".to_string()),
                priority: Priority::High,
            },
            MenuItem {
                icon: "🔗",
                label: "Join Session".to_string(),
                description: "Participate in existing DKG or signing session".to_string(),
                enabled: true,
                badge: Some("Connect".to_string()),
                priority: Priority::High,
            },
        ];

        // Add wallet-dependent options
        if wallet_count > 0 {
            items.extend(vec![
                MenuItem {
                    icon: "💼",
                    label: "Manage Wallets".to_string(),
                    description: format!(
                        "View and manage {} wallet{}",
                        wallet_count,
                        if wallet_count == 1 { "" } else { "s" }
                    ),
                    enabled: true,
                    badge: Some(format!("{}", wallet_count)),
                    priority: Priority::Medium,
                },
                MenuItem {
                    icon: "✍️",
                    label: "Sign Transaction".to_string(),
                    description: "Create threshold signature for transaction".to_string(),
                    enabled: true,
                    badge: Some("Ready".to_string()),
                    priority: Priority::High,
                },
            ]);
        }

        // Always available options
        items.extend(vec![
            MenuItem {
                icon: "⚙️",
                label: "Settings".to_string(),
                description: "Configure network, security, and display options".to_string(),
                enabled: true,
                badge: None,
                priority: Priority::Low,
            },
            MenuItem {
                icon: "🚪",
                label: "Exit".to_string(),
                description: "Close application securely".to_string(),
                enabled: true,
                badge: None,
                priority: Priority::Low,
            },
        ]);

        let props = Props::default();

        Self {
            props,
            items,
            selected: 0,
            focused: false,
            wallet_count,
            ecdsa_setup: crate::protocal::ecdsa::PrimeStatus::default(),
        }
    }

    /// Mirror `wallet_state.ecdsa_setup` at mount time.
    pub fn set_ecdsa_setup(&mut self, status: crate::protocal::ecdsa::PrimeStatus) {
        self.ecdsa_setup = status;
    }

    /// The header line about the Ethereum key setup, e.g. "⏳ ECDSA setup:
    /// preparing safe primes (42s so far, usually 1-5 min) — needed for the
    /// next wallet's Ethereum key".
    fn ecdsa_setup_line(&self) -> (String, Color) {
        use crate::protocal::ecdsa::PrimeStatus;
        match self.ecdsa_setup {
            PrimeStatus::Ready => (format!("✅ {}", self.ecdsa_setup.describe()), Color::Green),
            PrimeStatus::InsecureTestFixed => {
                (format!("⚠️ {}", self.ecdsa_setup.describe()), Color::Red)
            }
            PrimeStatus::Generating { .. } | PrimeStatus::Idle => (
                format!(
                    "⏳ {} — needed for the next wallet's Ethereum key",
                    self.ecdsa_setup.describe()
                ),
                Color::Yellow,
            ),
        }
    }

    /// Set the selected index
    pub fn set_selected(&mut self, index: usize) {
        self.selected = index.min(self.items.len().saturating_sub(1));
    }

    fn get_priority_color(&self, priority: &Priority) -> Color {
        match priority {
            Priority::High => Color::Cyan,
            Priority::Medium => Color::Yellow,
            Priority::Low => Color::Gray,
        }
    }

    fn get_status_summary(&self) -> String {
        if self.wallet_count == 0 {
            "🔒 No wallets configured - create one, or press i to import a backup".to_string()
        } else if self.wallet_count == 1 {
            "✅ 1 wallet configured and ready for operations".to_string()
        } else {
            format!(
                "✅ {} wallets configured - Multi-wallet environment",
                self.wallet_count
            )
        }
    }
}

impl Component for MainMenu {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Create sophisticated layout with header, main content, and status footer
        let chunks = Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(6), // Header with title and branding
                Constraint::Min(0),    // Main menu content
                Constraint::Length(5), // Status footer with system info
            ])
            .margin(1)
            .split(area);

        // Enhanced header section
        self.render_header(frame, chunks[0]);

        // Professional menu content
        self.render_menu(frame, chunks[1]);

        // Status footer with system information
        self.render_footer(frame, chunks[2]);
    }

    fn query<'a>(
        &'a self,
        attr: tuirealm::props::Attribute,
    ) -> Option<tuirealm::props::QueryResult<'a>> {
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
            Cmd::Move(Direction::Up) => {
                if self.selected > 0 {
                    self.selected -= 1;
                } else {
                    self.selected = self.items.len() - 1;
                }
                CmdResult::Changed(self.state())
            }
            Cmd::Move(Direction::Down) => {
                if self.selected < self.items.len() - 1 {
                    self.selected += 1;
                } else {
                    self.selected = 0;
                }
                CmdResult::Changed(self.state())
            }
            Cmd::Submit => {
                if self.items[self.selected].enabled {
                    CmdResult::Submit(self.state())
                } else {
                    CmdResult::NoChange
                }
            }
            _ => CmdResult::NoChange,
        }
    }
}

impl MainMenu {
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header_chunks = Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area);

        // Main title with professional branding
        let title = Paragraph::new("🏦 MPC Wallet Terminal Interface")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" MPC Wallet — Threshold Signer ")
                    .title_style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
            );
        frame.render_widget(title, header_chunks[0]);

        // Subtitle with version info
        let subtitle = Paragraph::new(concat!(
            "Threshold Signature System v",
            env!("CARGO_PKG_VERSION"),
        ))
        .style(
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )
        .alignment(Alignment::Center);
        frame.render_widget(subtitle, header_chunks[1]);

        // Security notice
        let security = Paragraph::new("🔐 FROST (Bitcoin, Solana) + threshold ECDSA (Ethereum)")
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Center);
        frame.render_widget(security, header_chunks[2]);

        // Ethereum key setup: the background safe-prime generation a
        // wallet DKG needs (a DKG started before it's ready waits for it).
        let (setup, color) = self.ecdsa_setup_line();
        let setup = Paragraph::new(setup)
            .style(Style::default().fg(color))
            .alignment(Alignment::Center);
        frame.render_widget(setup, header_chunks[3]);
    }

    fn render_menu(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == self.selected;
                let priority_color = self.get_priority_color(&item.priority);

                // Create selection indicator
                let indicator = if is_selected { "▶ " } else { "  " };

                // Create badge display
                let badge_display = if let Some(ref badge) = item.badge {
                    format!(" [{}]", badge)
                } else {
                    String::new()
                };

                // Main content with enhanced formatting
                let content = if is_selected {
                    // Expanded view for selected item
                    format!(
                        "{}{} {}{}  {}\n    └─ {}",
                        indicator,
                        item.icon,
                        item.label,
                        badge_display,
                        if item.enabled { "✅" } else { "🚫" },
                        item.description
                    )
                } else {
                    // Compact view for non-selected items
                    format!(
                        "{}{} {}{}  {}",
                        indicator,
                        item.icon,
                        item.label,
                        badge_display,
                        if item.enabled { "" } else { "(Disabled)" }
                    )
                };

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else if item.enabled {
                    Style::default().fg(priority_color)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(" Main Menu ")
                    .title_style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(if self.focused {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Gray)
                    }),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected));

        frame.render_stateful_widget(list, area, &mut list_state);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_chunks = Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(2),
            ])
            .split(area);

        // System status
        let status = self.get_status_summary();
        let status_widget = Paragraph::new(status)
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Center);
        frame.render_widget(status_widget, footer_chunks[0]);

        // Controls
        let controls = if self.focused {
            "🎮 Navigation: ↑↓ Select Options • Enter: Execute • i: Import Wallet • Ctrl-C: Quit"
        } else {
            "💡 Press any key to begin • Professional MPC Wallet Management System"
        };

        let controls_widget = Paragraph::new(controls)
            .style(
                Style::default()
                    .fg(if self.focused {
                        Color::Green
                    } else {
                        Color::Gray
                    })
                    .add_modifier(Modifier::ITALIC),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Controls ")
                    .title_style(Style::default().fg(Color::Gray)),
            );
        frame.render_widget(controls_widget, footer_chunks[1]);

        // Footer info
        let footer_info = "© 2025 MPC Wallet • FROST + threshold ECDSA";
        let footer_widget = Paragraph::new(footer_info)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(footer_widget, footer_chunks[2]);
    }
}

impl AppComponent<Message, UserEvent> for MainMenu {
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
            _ => {
                // All key handling is done at the app level - KISS!
                None
            }
        }
    }
}

impl MpcWalletComponent for MainMenu {
    fn id(&self) -> Id {
        Id::MainMenu
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
    use crate::protocal::ecdsa::PrimeStatus;

    #[test]
    fn ecdsa_setup_line_says_preparing_with_elapsed_time() {
        let mut menu = MainMenu::with_wallet_count(0);
        menu.set_ecdsa_setup(PrimeStatus::Generating { elapsed_secs: 42 });
        let (line, color) = menu.ecdsa_setup_line();
        assert_eq!(
            line,
            "⏳ ECDSA setup: preparing safe primes (42s so far, usually 1-5 min) — needed for \
             the next wallet's Ethereum key"
        );
        assert_eq!(color, Color::Yellow);
    }

    #[test]
    fn ecdsa_setup_line_says_ready() {
        let mut menu = MainMenu::with_wallet_count(1);
        menu.set_ecdsa_setup(PrimeStatus::Ready);
        assert_eq!(menu.ecdsa_setup_line().0, "✅ ECDSA setup: ready");
    }
}
