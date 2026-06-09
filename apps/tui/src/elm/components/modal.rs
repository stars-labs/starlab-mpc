//! Modal dialog — centered overlay for Confirm / Error / Success /
//! Progress / Input.
//!
//! Historical note: the `view` method used to be a bare stub
//! (`// Modal rendering will be implemented`), meaning every
//! `Modal::Error` / `Modal::Confirm` pushed into `Model.ui_state.modal`
//! rendered nothing. Wrong-password unlock failures, corrupt-signing-
//! session warnings, delete-wallet confirmations — all invisible.
//! Full-flow test caught it (/docs: Bug C).
//!
//! This implementation handles every variant of the `Modal` enum the
//! Model can produce. Input-variant is shown with its prompt + default
//! value but the actual input routing is a separate concern (keyboard
//! handling lives at the app level); here we just render what's there.

use crate::elm::components::{Id, MpcWalletComponent, UserEvent};
use crate::elm::message::Message;
use crate::elm::model::Modal;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::Props;
use tuirealm::ratatui::Frame;
use tuirealm::state::State;

#[derive(Debug, Clone, Default)]
pub struct ModalComponent {
    props: Props,
    modal: Option<Modal>,
    focused: bool,
}

impl ModalComponent {
    pub fn set_modal(&mut self, modal: Option<Modal>) {
        self.modal = modal;
    }

    /// Copy the current modal off `Model.ui_state.modal`. Same
    /// `set_from_model` pattern as the rest of the Phase C UI
    /// components — the mount site runs this before each mount so the
    /// component matches Model state.
    pub fn set_from_model(&mut self, model: &crate::elm::Model) {
        self.modal = model.ui_state.modal.clone();
    }
}

/// Border color + title prefix that match the modal's semantic
/// category. Makes the "is this an error or a confirm?" distinction
/// visible at a glance.
fn kind_style(modal: &Modal) -> (Color, &'static str) {
    match modal {
        Modal::Error { .. } => (Color::Red, "❌ "),
        Modal::Success { .. } => (Color::LightGreen, "✅ "),
        Modal::Confirm { .. } => (Color::Yellow, "⚠️  "),
        Modal::Progress { .. } => (Color::Cyan, "⏳ "),
        Modal::Input { .. } => (Color::Cyan, "📝 "),
    }
}

impl Component for ModalComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let Some(modal) = self.modal.as_ref() else {
            return;
        };

        // `area` comes in already centered by `ElmApp::render` via
        // `centered_rect(60, 20, main_area)`. Clear it first so whatever's
        // underneath doesn't bleed through the edges when the caller
        // hasn't fully blanked the background.
        frame.render_widget(Clear, area);

        let (border_color, title_prefix) = kind_style(modal);

        match modal {
            Modal::Error { title, message } | Modal::Success { title, message } => {
                render_simple_dialog(
                    frame,
                    area,
                    border_color,
                    &format!("{}{}", title_prefix, title),
                    message,
                    "Press Enter or Esc to dismiss",
                );
            }
            Modal::Confirm { title, message, .. } => {
                render_simple_dialog(
                    frame,
                    area,
                    border_color,
                    &format!("{}{}", title_prefix, title),
                    message,
                    "Enter = Confirm    Esc = Cancel",
                );
            }
            Modal::Progress {
                title,
                message,
                progress,
            } => {
                render_progress_dialog(
                    frame,
                    area,
                    border_color,
                    &format!("{}{}", title_prefix, title),
                    message,
                    *progress,
                );
            }
            Modal::Input {
                title,
                prompt,
                default_value,
                ..
            } => {
                // Input-modal keystroke routing isn't wired up this
                // phase — show a passive preview so users see the
                // prompt. If an Input modal ever actually fires in
                // prod, we'll know from the behavior that we need to
                // finish the routing.
                render_simple_dialog(
                    frame,
                    area,
                    border_color,
                    &format!("{}{}", title_prefix, title),
                    &format!("{}\n\nDefault: {}", prompt, default_value),
                    "Press Esc to dismiss (Input routing not wired this phase)",
                );
            }
        }
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

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

fn render_simple_dialog(
    frame: &mut Frame,
    area: Rect,
    border_color: Color,
    title: &str,
    message: &str,
    hints: &str,
) {
    let outer = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let body = Paragraph::new(message)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White));
    frame.render_widget(body, rows[0]);

    let hint_line = Paragraph::new(hints)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint_line, rows[1]);
}

fn render_progress_dialog(
    frame: &mut Frame,
    area: Rect,
    border_color: Color,
    title: &str,
    message: &str,
    progress: f32,
) {
    let outer = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1), // bar
            Constraint::Length(1), // pct text
            Constraint::Length(1), // hint
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        rows[0],
    );

    // A plain block-character progress bar + separate percentage line.
    // Using `Gauge` would look nicer but its label is a single rendered
    // attribute that ratatui's TestBackend doesn't surface, making
    // rendering tests unreliable.
    let pct = progress.clamp(0.0, 1.0);
    let pct_int = (pct * 100.0) as u8;
    let bar_width = rows[1].width.saturating_sub(2) as usize; // leave room for brackets
    let filled = ((pct * bar_width as f32) as usize).min(bar_width);
    let bar_text = format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(bar_width.saturating_sub(filled))
    );
    frame.render_widget(
        Paragraph::new(bar_text).alignment(Alignment::Center).style(
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(format!("{}%", pct_int))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White)),
        rows[2],
    );

    let hint_line = Paragraph::new("Esc to cancel")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint_line, rows[3]);
}

impl AppComponent<Message, UserEvent> for ModalComponent {
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Message> {
        None
    }
}

impl MpcWalletComponent for ModalComponent {
    fn id(&self) -> Id {
        Id::Modal
    }

    fn is_visible(&self) -> bool {
        self.modal.is_some()
    }

    fn on_focus(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render(comp: &mut ModalComponent) -> String {
        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).expect("TestBackend");
        terminal
            .draw(|f| comp.view(f, f.area()))
            .expect("draw");
        let buf = terminal.backend().buffer();
        let a = buf.area();
        let mut out = String::new();
        for y in 0..a.height {
            for x in 0..a.width {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn no_modal_renders_nothing() {
        let mut c = ModalComponent::default();
        let rendered = render(&mut c);
        assert!(!rendered.contains("Error"));
        assert!(!rendered.contains("╭"));
    }

    #[test]
    fn error_modal_renders_title_and_message() {
        let mut c = ModalComponent::default();
        c.set_modal(Some(Modal::Error {
            title: "Unlock Failed".to_string(),
            message: "Wrong password — try again.".to_string(),
        }));
        let rendered = render(&mut c);
        assert!(
            rendered.contains("Unlock Failed"),
            "error modal must render title; got {:?}",
            &rendered[..rendered.len().min(400)]
        );
        assert!(
            rendered.contains("Wrong password"),
            "error modal must render message; got {:?}",
            &rendered[..rendered.len().min(400)]
        );
        assert!(
            rendered.contains("Enter or Esc"),
            "error modal must render dismiss hint"
        );
    }

    #[test]
    fn confirm_modal_shows_cancel_hint() {
        let mut c = ModalComponent::default();
        c.set_modal(Some(Modal::Confirm {
            title: "Delete?".to_string(),
            message: "This cannot be undone.".to_string(),
            on_confirm: Box::new(Message::NavigateHome),
            on_cancel: Box::new(Message::CloseModal),
        }));
        let rendered = render(&mut c);
        assert!(rendered.contains("Delete?"));
        assert!(
            rendered.contains("Enter = Confirm"),
            "confirm modal must show Enter=Confirm hint"
        );
        assert!(rendered.contains("Esc = Cancel"));
    }

    #[test]
    fn progress_modal_renders_percentage() {
        let mut c = ModalComponent::default();
        c.set_modal(Some(Modal::Progress {
            title: "Encrypting".to_string(),
            message: "Please wait…".to_string(),
            progress: 0.42,
        }));
        let rendered = render(&mut c);
        assert!(rendered.contains("Encrypting"));
        assert!(
            rendered.contains("42%"),
            "progress modal must show the percentage; got {:?}",
            &rendered[..rendered.len().min(500)]
        );
    }
}
