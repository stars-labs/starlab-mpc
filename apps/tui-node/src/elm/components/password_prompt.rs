//! Password Prompt Component — renders the two-field password screen.
//!
//! The component is **view-only**: it holds no cleartext. Every keystroke
//! routes through `app.rs::handle_key_event` → `Message::Password*` →
//! `update.rs`, which mutates the draft state on `Model.wallet_state`.
//! Before each mount, `mount_components` calls
//! [`PasswordPromptComponent::set_from_model`] to copy the *lengths*, focus
//! flag, and current error into the component. The view then renders
//! bullets (`•`) of the right length plus an underscore caret on the
//! focused field.
//!
//! Why view-only: tuirealm's per-component `on()` is bypassed by the
//! app-level key handler in this codebase (see `handle_key_event` for
//! ThresholdConfig / DKGProgress doing the same pattern). Keeping the
//! cleartext out of the component mirrors that convention and — as a
//! bonus — means `Debug`-printing the component never leaks password
//! characters. The `Model.wallet_state` is redacted at the `Debug` impl
//! level too.

use crate::elm::components::{Id, MpcWalletComponent, UserEvent};
use crate::elm::message::Message;
use crate::elm::model::WalletState;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::Props;
use tuirealm::ratatui::Frame;
use tuirealm::state::State;

#[derive(Debug, Clone, Default)]
pub struct PasswordPromptComponent {
    props: Props,
    /// How many bullets to draw for the password field — copied from
    /// `wallet_state.password_draft.len()` at mount time.
    password_len: usize,
    /// Same, for the confirm field.
    confirm_len: usize,
    /// `true` iff the confirm field has focus.
    focus_confirm: bool,
    /// Validation error to render on the error row, or `None` to hide it.
    error: Option<String>,
    focused: bool,
}

impl PasswordPromptComponent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pull the current draft state off the Model. Call this right before
    /// `app.mount(...)` so the freshly-mounted component matches whatever
    /// the user has typed so far. Never copies the cleartext — only the
    /// lengths.
    pub fn set_from_model(&mut self, ws: &WalletState) {
        self.password_len = ws.password_draft.chars().count();
        self.confirm_len = ws.confirm_draft.chars().count();
        self.focus_confirm = ws.password_focus_confirm;
        self.error = ws.password_error.clone();
    }

    fn render_field_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        bullet_count: usize,
        is_focused: bool,
    ) {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

        let mut content = "•".repeat(bullet_count);
        if is_focused {
            content.push('_'); // caret so typing location is visible
        }

        let border_color = if is_focused {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let widget = Paragraph::new(content).block(
            Block::default()
                .title(label)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color)),
        );
        frame.render_widget(widget, area);
    }
}

impl Component for PasswordPromptComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

        let outer = Block::default()
            .title(" Set Wallet Password ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // explainer
                Constraint::Length(3), // password field
                Constraint::Length(3), // confirm field
                Constraint::Length(2), // error line (if any)
                Constraint::Min(1),    // hints (bottom)
            ])
            .split(inner);

        let explainer = Paragraph::new(
            "This password encrypts this device's key share in the local keystore.\n\
             Each participant picks their own — no coordination required.",
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
        frame.render_widget(explainer, rows[0]);

        self.render_field_row(
            frame,
            rows[1],
            " Password ",
            self.password_len,
            !self.focus_confirm,
        );
        self.render_field_row(
            frame,
            rows[2],
            " Confirm ",
            self.confirm_len,
            self.focus_confirm,
        );

        if let Some(ref msg) = self.error {
            let error_para = Paragraph::new(msg.as_str())
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            frame.render_widget(error_para, rows[3]);
        }

        let hints = Paragraph::new("Enter = Submit    Tab = Next field    Esc = Cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hints, rows[4]);
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

impl AppComponent<Message, UserEvent> for PasswordPromptComponent {
    /// The app-level `handle_key_event` owns keyboard routing for this
    /// screen (see `app.rs`), so we deliberately never consume events
    /// here. Returning `None` keeps tuirealm from accidentally acting on
    /// something we also routed through the Model.
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Message> {
        None
    }
}

impl MpcWalletComponent for PasswordPromptComponent {
    fn id(&self) -> Id {
        Id::PasswordPrompt
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
    fn set_from_model_copies_lengths_but_not_cleartext() {
        let mut ws = WalletState::default();
        ws.password_draft = "secretpw".to_string();
        ws.confirm_draft = "secretpw1".to_string();
        ws.password_focus_confirm = true;
        ws.password_error = Some("bad".to_string());

        let mut c = PasswordPromptComponent::new();
        c.set_from_model(&ws);

        assert_eq!(c.password_len, 8);
        assert_eq!(c.confirm_len, 9);
        assert!(c.focus_confirm);
        assert_eq!(c.error.as_deref(), Some("bad"));

        // The component stores no cleartext anywhere — the only way it
        // could is via a `password`/`confirm` field that no longer exists.
        // Debug-print it and spot-check that the secret is not in the
        // output. This is a weak property test but catches the "I
        // accidentally re-added a String field" regression.
        let dbg = format!("{:?}", c);
        assert!(!dbg.contains("secretpw"), "component Debug leaked cleartext: {dbg}");
    }

    #[test]
    fn default_state_has_no_error_and_password_focus() {
        let c = PasswordPromptComponent::new();
        assert_eq!(c.password_len, 0);
        assert_eq!(c.confirm_len, 0);
        assert!(!c.focus_confirm, "fresh mount focuses the password field first");
        assert!(c.error.is_none());
    }
}
