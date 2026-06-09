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
use crate::elm::model::{PasswordPromptPurpose, WalletState};
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
    /// Current wallet-name draft (plaintext — it's not a secret), shown
    /// only in the SetNew flow. Copied from `wallet_state.wallet_name_draft`.
    wallet_name: String,
    /// `true` iff the wallet-name field has focus (SetNew only).
    wallet_name_focus: bool,
    /// Validation error to render on the error row, or `None` to hide it.
    error: Option<String>,
    /// Whether this mount is collecting a NEW password (DKG creator/
    /// joiner) or UNLOCKING an existing wallet (cold-start signing).
    /// Drives title, explainer copy, and whether the confirm row is
    /// rendered. Mirrors `WalletState.password_prompt_purpose`; copied
    /// in via `set_from_model`.
    purpose: PasswordPromptPurpose,
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
        self.wallet_name = ws.wallet_name_draft.clone();
        self.wallet_name_focus = ws.wallet_name_focus;
        self.error = ws.password_error.clone();
        self.purpose = ws.password_prompt_purpose.clone();
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

    /// Like `render_field_row` but shows the literal text (the wallet name
    /// isn't a secret, so no bullet masking). A placeholder is rendered
    /// dimmed when the field is empty.
    fn render_text_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        text: &str,
        is_focused: bool,
    ) {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

        let (content, fg) = if text.is_empty() && !is_focused {
            ("e.g. Treasury".to_string(), Color::DarkGray)
        } else {
            let mut c = text.to_string();
            if is_focused {
                c.push('_');
            }
            (c, Color::Reset)
        };

        let border_color = if is_focused {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let widget = Paragraph::new(content)
            .style(Style::default().fg(fg))
            .block(
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

        // Title + body copy + layout all branch on purpose. The two
        // shapes are similar enough to share render_field_row but
        // different enough that we don't try to thread one
        // hyper-parameterized layout through both.
        let is_unlock = matches!(self.purpose, PasswordPromptPurpose::Unlock);
        let title = if is_unlock {
            " Unlock Wallet "
        } else {
            " Set Wallet Password "
        };
        let explainer_text = if is_unlock {
            "Enter the password you set when this wallet's key share was created.\n\
             It decrypts the local share so this device can join the signing ceremony."
        } else {
            "This password encrypts this device's key share in the local keystore.\n\
             Each participant picks their own — no coordination required."
        };
        let hints_text = if is_unlock {
            "Enter = Unlock    Esc = Cancel"
        } else {
            "Enter = Submit    Tab = Next field    Esc = Cancel"
        };

        let outer = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        // SetNew: explainer / name / password / confirm / error / hints.
        // Unlock:  explainer /  --  / password /  ---   / error / hints.
        // We keep six rows in both layouts so the screen geometry is
        // stable; in Unlock the name/confirm rows shrink to length 0
        // instead of rendering spurious empty boxes.
        let name_row_height = if is_unlock { 0 } else { 3 };
        let confirm_row_height = if is_unlock { 0 } else { 3 };
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),                    // explainer
                Constraint::Length(name_row_height),      // wallet name (or 0)
                Constraint::Length(3),                    // password field
                Constraint::Length(confirm_row_height),   // confirm field (or 0)
                Constraint::Length(2),                    // error line (if any)
                Constraint::Min(1),                       // hints (bottom)
            ])
            .split(inner);

        let explainer = Paragraph::new(explainer_text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });
        frame.render_widget(explainer, rows[0]);

        // Wallet name (SetNew only) — optional local display label.
        if !is_unlock {
            self.render_text_row(
                frame,
                rows[1],
                " Wallet name (optional) ",
                &self.wallet_name,
                self.wallet_name_focus,
            );
        }

        // In Unlock mode the password field is the only field, so it is
        // always focused. In SetNew it's focused only when neither the
        // name nor the confirm field has focus.
        let pw_focused = if is_unlock {
            true
        } else {
            !self.wallet_name_focus && !self.focus_confirm
        };
        self.render_field_row(
            frame,
            rows[2],
            " Password ",
            self.password_len,
            pw_focused,
        );
        if !is_unlock {
            self.render_field_row(
                frame,
                rows[3],
                " Confirm ",
                self.confirm_len,
                !self.wallet_name_focus && self.focus_confirm,
            );
        }

        if let Some(ref msg) = self.error {
            let error_para = Paragraph::new(msg.as_str())
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            frame.render_widget(error_para, rows[4]);
        }

        let hints = Paragraph::new(hints_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hints, rows[5]);
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
        let ws = WalletState {
            password_draft: "secretpw".to_string(),
            confirm_draft: "secretpw1".to_string(),
            password_focus_confirm: true,
            password_error: Some("bad".to_string()),
            ..Default::default()
        };

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
        assert_eq!(
            c.purpose,
            PasswordPromptPurpose::SetNew,
            "default purpose is SetNew so existing DKG flows behave unchanged"
        );
    }

    #[test]
    fn set_from_model_copies_unlock_purpose() {
        let ws = WalletState {
            password_draft: "x".to_string(),
            password_prompt_purpose: PasswordPromptPurpose::Unlock,
            ..Default::default()
        };
        let mut c = PasswordPromptComponent::new();
        c.set_from_model(&ws);
        assert_eq!(c.purpose, PasswordPromptPurpose::Unlock);
        assert_eq!(c.password_len, 1);
    }
}
