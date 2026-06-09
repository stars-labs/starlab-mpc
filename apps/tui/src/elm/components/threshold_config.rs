//! Threshold Configuration Component
//!
//! Professional component for configuring MPC threshold parameters with visual explanations

use crate::elm::components::{Id, UserEvent, MpcWalletComponent};
use crate::elm::message::Message;

use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::event::Event;
use ratatui::layout::{Rect, Constraint, Direction as LayoutDirection, Layout, Alignment};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, List, ListItem, Wrap, Gauge};
use tuirealm::component::{AppComponent, Component};
use tuirealm::ratatui::Frame;
use tuirealm::props::Props;
use tuirealm::state::{State, StateValue};

/// Professional threshold configuration component
#[derive(Debug, Clone)]
pub struct ThresholdConfigComponent {
    props: Props,
    participants: usize,
    threshold: usize,
    focused: bool,
    selected_field: usize, // 0 = participants, 1 = threshold
}

#[derive(Debug, Clone)]
struct ThresholdPreset {
    name: &'static str,
    participants: usize,
    threshold: usize,
    security_model: &'static str,
    use_case: &'static str,
    pros: Vec<&'static str>,
    cons: Vec<&'static str>,
}

impl Default for ThresholdConfigComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ThresholdConfigComponent {
    pub fn new() -> Self {
        Self {
            props: Props::default(),
            participants: 3,
            threshold: 2,
            focused: false,
            selected_field: 0,
        }
    }
    
    pub fn with_values(participants: u16, threshold: u16, selected_field: usize) -> Self {
        tracing::info!("🆕 Creating ThresholdConfigComponent with selected_field={}", selected_field);
        Self {
            props: Props::default(),
            participants: participants as usize,
            threshold: threshold as usize,
            focused: false,
            selected_field,
        }
    }
    
    fn get_presets(&self) -> Vec<ThresholdPreset> {
        vec![
            ThresholdPreset {
                name: "Standard Security (2-of-3)",
                participants: 3,
                threshold: 2,
                security_model: "Balanced",
                use_case: "Small teams, personal wallets",
                pros: vec![
                    "✓ One backup participant",
                    "✓ Simple coordination",
                    "✓ Good availability",
                ],
                cons: vec![
                    "✗ Limited redundancy",
                    "✗ Single point of failure risk",
                ],
            },
            ThresholdPreset {
                name: "Enhanced Security (3-of-5)",
                participants: 5,
                threshold: 3,
                security_model: "High Security",
                use_case: "Business operations, team treasuries",
                pros: vec![
                    "✓ Two backup participants",
                    "✓ Better fault tolerance",
                    "✓ Distributed control",
                ],
                cons: vec![
                    "✗ More complex setup",
                    "✗ Requires 5 devices/parties",
                ],
            },
            ThresholdPreset {
                name: "Enterprise Grade (5-of-7)",
                participants: 7,
                threshold: 5,
                security_model: "Maximum Security",
                use_case: "Corporate treasuries, DAOs",
                pros: vec![
                    "✓ High redundancy",
                    "✓ Enterprise compliance",
                    "✓ Maximum security",
                ],
                cons: vec![
                    "✗ Complex coordination",
                    "✗ Slower operations",
                ],
            },
        ]
    }
    
    fn validate_threshold(&self) -> Result<(), String> {
        if self.participants < 2 {
            return Err("Minimum 2 participants required".to_string());
        }
        if self.participants > 10 {
            return Err("Maximum 10 participants supported".to_string());
        }
        if self.threshold < 2 {
            return Err("Threshold must be at least 2".to_string());
        }
        if self.threshold > self.participants {
            return Err("Threshold cannot exceed participants".to_string());
        }
        if self.threshold < (self.participants / 2) + 1 {
            return Err("Threshold should be majority (>50%) for security".to_string());
        }
        Ok(())
    }
    
    fn get_security_score(&self) -> f32 {
        let ratio = self.threshold as f32 / self.participants as f32;
        let redundancy = (self.participants - self.threshold) as f32 / self.participants as f32;
        
        // Security score based on threshold ratio and redundancy
        (ratio * 0.7 + redundancy * 0.3).min(1.0)
    }
}

impl Component for ThresholdConfigComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        tracing::info!("🖼️ ThresholdConfigComponent::view() called with selected_field={}", self.selected_field);
        let chunks = Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(5),   // Header
                Constraint::Length(12),  // Configuration
                Constraint::Length(10),  // Visual representation
                Constraint::Min(0),      // Presets
                Constraint::Length(4),   // Footer
            ])
            .margin(1)
            .split(area);
        
        // Header
        self.render_header(frame, chunks[0]);
        
        // Configuration controls
        self.render_configuration(frame, chunks[1]);
        
        // Visual representation
        self.render_visual(frame, chunks[2]);
        
        // Presets
        self.render_presets(frame, chunks[3]);
        
        // Footer
        self.render_footer(frame, chunks[4]);
    }
    
    fn query<'a>(&'a self, attr: tuirealm::props::Attribute) -> Option<tuirealm::props::QueryResult<'a>> {
        self.props.get_for_query(attr)
    }
    
    fn attr(&mut self, attr: tuirealm::props::Attribute, value: tuirealm::props::AttrValue) {
        self.props.set(attr, value);
    }
    
    fn state(&self) -> tuirealm::state::State {
        State::Single(StateValue::Usize(self.threshold))
    }
    
    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(Direction::Up) => {
                if self.selected_field == 0 && self.participants < 10 {
                    self.participants += 1;
                    self.threshold = self.threshold.min(self.participants);
                } else if self.selected_field == 1 && self.threshold < self.participants {
                    self.threshold += 1;
                }
                CmdResult::Changed(self.state())
            }
            Cmd::Move(Direction::Down) => {
                if self.selected_field == 0 && self.participants > 2 {
                    self.participants -= 1;
                    self.threshold = self.threshold.min(self.participants);
                } else if self.selected_field == 1 && self.threshold > 2 {
                    self.threshold -= 1;
                }
                CmdResult::Changed(self.state())
            }
            Cmd::Move(Direction::Left) => {
                self.selected_field = 0;
                CmdResult::Changed(self.state())
            }
            Cmd::Move(Direction::Right) => {
                self.selected_field = 1;
                CmdResult::Changed(self.state())
            }
            Cmd::Submit => CmdResult::Submit(self.state()),
            _ => CmdResult::NoChange,
        }
    }
}

impl ThresholdConfigComponent {
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header_text = ["⚙️ THRESHOLD CONFIGURATION (Step 3 of 3)",
            "",
            "Set the number of participants and signing threshold",
            "Threshold = minimum signers needed to authorize transactions"];
        
        let header = Paragraph::new(header_text.join("\n"))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" MPC Threshold Parameters ")
                    .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            );
        frame.render_widget(header, area);
    }
    
    fn render_configuration(&self, frame: &mut Frame, area: Rect) {
        tracing::debug!("🎨 Rendering ThresholdConfig with selected_field={}", self.selected_field);
        let chunks = Layout::default()
            .direction(LayoutDirection::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(area);
        
        // Participants control
        let participants_selected = self.selected_field == 0;
        let participants_text = format!(
            "👥 Total Participants\n\n{}\n\n{}\n\nRange: 2-10\n\n{}",
            self.participants,
            if participants_selected { "▲ Increase / ▼ Decrease" } else { "← Select to modify" },
            "Total number of key share holders"
        );
        
        let participants_widget = Paragraph::new(participants_text)
            .style(
                Style::default()
                    .fg(if participants_selected { Color::Yellow } else { Color::White })
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(if participants_selected { BorderType::Thick } else { BorderType::Rounded })
                    .border_style(
                        Style::default().fg(if participants_selected { Color::Yellow } else { Color::Gray })
                    )
            );
        frame.render_widget(participants_widget, chunks[0]);
        
        // Threshold control
        let threshold_selected = self.selected_field == 1;
        let threshold_text = format!(
            "🔑 Signing Threshold\n\n{}\n\n{}\n\nRange: 2-{}\n\n{}",
            self.threshold,
            if threshold_selected { "▲ Increase / ▼ Decrease" } else { "→ Select to modify" },
            self.participants,
            "Minimum signers for approval"
        );
        
        let threshold_widget = Paragraph::new(threshold_text)
            .style(
                Style::default()
                    .fg(if threshold_selected { Color::Yellow } else { Color::White })
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(if threshold_selected { BorderType::Thick } else { BorderType::Rounded })
                    .border_style(
                        Style::default().fg(if threshold_selected { Color::Yellow } else { Color::Gray })
                    )
            );
        frame.render_widget(threshold_widget, chunks[1]);
    }
    
    fn render_visual(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(3),
            ])
            .split(area);
        
        // Visual representation
        let visual = format!(
            "{}-of-{} Configuration",
            self.threshold, self.participants
        );
        let visual_widget = Paragraph::new(visual)
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
                    .title(" Current Configuration ")
            );
        frame.render_widget(visual_widget, chunks[0]);
        
        // Security analysis
        let validation_result = self.validate_threshold();
        let (analysis_text, analysis_color) = match validation_result {
            Ok(_) => {
                let redundancy = self.participants - self.threshold;
                (
                    format!(
                        "✅ Valid: {} signers required, {} can be offline",
                        self.threshold, redundancy
                    ),
                    Color::Green
                )
            }
            Err(msg) => (format!("⚠️ {}", msg), Color::Red),
        };
        
        let analysis = Paragraph::new(analysis_text)
            .style(Style::default().fg(analysis_color))
            .alignment(Alignment::Center);
        frame.render_widget(analysis, chunks[1]);
        
        // Security score gauge
        let security_score = self.get_security_score();
        let gauge = Gauge::default()
            .block(Block::default().title("Security Level"))
            .gauge_style(Style::default().fg(Color::Green))
            .percent((security_score * 100.0) as u16)
            .label(format!("{:.0}%", security_score * 100.0));
        frame.render_widget(gauge, chunks[2]);
        
        // Example scenario
        let scenario = format!(
            "Example: With {}-of-{}, you need {} people to sign, {} can be unavailable",
            self.threshold, self.participants, self.threshold, self.participants - self.threshold
        );
        let scenario_widget = Paragraph::new(scenario)
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(scenario_widget, chunks[3]);
    }
    
    fn render_presets(&self, frame: &mut Frame, area: Rect) {
        let presets = self.get_presets();
        let preset_items: Vec<ListItem> = presets
            .iter()
            .map(|p| {
                let is_current = p.participants == self.participants && p.threshold == self.threshold;
                let text = format!(
                    "{} {} - {} ({})\n   Pros: {}\n   Cons: {}",
                    if is_current { "▶" } else { " " },
                    p.name,
                    p.use_case,
                    p.security_model,
                    p.pros.join(", "),
                    p.cons.join(", ")
                );
                ListItem::new(text).style(
                    Style::default().fg(if is_current { Color::Yellow } else { Color::Gray })
                )
            })
            .collect();
        
        let list = List::new(preset_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Common Presets ")
            );
        
        frame.render_widget(list, area);
    }
    
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_text = [format!("Configuration: {}-of-{} threshold", self.threshold, self.participants),
            "".to_string(),
            "← → Switch Fields | ↑↓ Adjust Values | Enter: Confirm | Esc: Back".to_string(),
            "💡 Recommended: Use majority threshold (>50%) for security".to_string()];
        
        let footer = Paragraph::new(footer_text.join("\n"))
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::ITALIC)
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::DarkGray))
            );
        frame.render_widget(footer, area);
    }
}

impl AppComponent<Message, UserEvent> for ThresholdConfigComponent {
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

impl MpcWalletComponent for ThresholdConfigComponent {
    fn id(&self) -> Id {
        Id::CreateWallet
    }
    
    fn is_visible(&self) -> bool {
        true
    }
    
    fn on_focus(&mut self, focused: bool) {
        self.focused = focused;
    }
}