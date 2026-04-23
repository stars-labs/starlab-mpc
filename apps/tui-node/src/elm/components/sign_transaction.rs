//! SignTransaction — pick a message and kick off a threshold signing ceremony.
//!
//! Pattern matches `PasswordPromptComponent`: the component is view-only
//! (no cleartext stored inside), and the input state lives on
//! `Model.wallet_state.sign_message_draft`. Keyboard routing is owned by
//! `app.rs::handle_key_event` — every printable character flows through
//! `Message::SignTypeChar`, backspace through `Message::SignBackspace`,
//! Enter through `Message::SignSubmit`.
//!
//! Layout:
//!     ┌── 🖊️  Sign with <wallet_id> ────────────┐
//!     │ Group key: <short>...                 │
//!     │                                        │
//!     │ ┌ Message to sign ─────────────────┐   │
//!     │ │ <user text>_                     │   │
//!     │ └──────────────────────────────────┘   │
//!     │                                        │
//!     │ <error, if any>                        │
//!     │                                        │
//!     │ Enter = Sign    Esc = Cancel           │
//!     └────────────────────────────────────────┘
//!
//! Phase C scope: message-only field. The KeyPackage is assumed to
//! already be loaded on AppState — for a fresh-DKG session that's true;
//! reloading a cold wallet from disk is Stage C.4's concern (the
//! password flow threads through PasswordPrompt).

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
pub struct SignTransactionComponent {
    props: Props,
    wallet_id: String,
    group_pubkey_short: String,
    message_preview: String,
    error: Option<String>,
    focused: bool,
}

impl SignTransactionComponent {
    pub fn new(wallet_id: impl Into<String>) -> Self {
        Self {
            wallet_id: wallet_id.into(),
            ..Self::default()
        }
    }

    /// Pulls the live draft off the Model for rendering. Called from
    /// `app.rs::mount_components` right before mount, same pattern as
    /// `PasswordPromptComponent::set_from_model`.
    pub fn set_from_model(&mut self, ws: &WalletState) {
        self.message_preview = ws.sign_message_draft.clone();

        // Pull the wallet's group pubkey from the loaded wallet list so
        // the user has a visual cross-check that they're signing with
        // the right wallet (paranoid but cheap).
        self.group_pubkey_short = ws
            .wallets
            .iter()
            .find(|w| w.session_id == self.wallet_id)
            .map(|w| {
                let k = &w.group_public_key;
                if k.len() > 24 {
                    format!("{}…{}", &k[..12], &k[k.len() - 8..])
                } else {
                    k.clone()
                }
            })
            .unwrap_or_else(|| "(not in cached list)".to_string());
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }
}

impl Component for SignTransactionComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

        let outer_title = format!(" 🖊️  Sign with {} ", self.wallet_id);
        let outer = Block::default()
            .title(outer_title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // group key line
                Constraint::Length(1), // spacer
                Constraint::Length(5), // message input
                Constraint::Length(2), // error (if any)
                Constraint::Min(1),    // hints
            ])
            .split(inner);

        // Group-key cross-check.
        let group_line = Paragraph::new(format!("Group key: {}", self.group_pubkey_short))
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(group_line, rows[0]);

        // Message input with caret.
        let content = format!("{}_", self.message_preview);
        let msg_widget = Paragraph::new(content)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title(" Message to sign ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
        frame.render_widget(msg_widget, rows[2]);

        // Inline error.
        if let Some(ref msg) = self.error {
            let err_para = Paragraph::new(msg.as_str())
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            frame.render_widget(err_para, rows[3]);
        }

        // Hints.
        let hints = Paragraph::new("Enter = Sign    Esc = Cancel")
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

impl AppComponent<Message, UserEvent> for SignTransactionComponent {
    /// All keystrokes flow through `app.rs::handle_key_event`. See
    /// `PasswordPromptComponent::on` for the same no-op pattern + why.
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Message> {
        None
    }
}

impl MpcWalletComponent for SignTransactionComponent {
    fn id(&self) -> Id {
        Id::SignTransaction
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
    fn set_from_model_copies_draft_and_finds_group_key() {
        use crate::keystore::WalletMetadata;
        let ws = WalletState {
            sign_message_draft: "hello world".to_string(),
            wallets: vec![WalletMetadata::new(
                "wallet-dkg_abcd".to_string(),
                "mpc-1".to_string(),
                "secp256k1".to_string(),
                2,
                3,
                1,
                "021de2d69979f0a03ea413e7ed6a32ad02111b90d1f03793649157d3e4ee952143".to_string(),
            )],
            ..Default::default()
        };

        let mut c = SignTransactionComponent::new("wallet-dkg_abcd");
        c.set_from_model(&ws);

        assert_eq!(c.message_preview, "hello world");
        assert!(
            c.group_pubkey_short.contains("021de2d6"),
            "short pubkey must include the leading chars; got {:?}",
            c.group_pubkey_short
        );
    }

    #[test]
    fn set_from_model_falls_back_when_wallet_not_in_cache() {
        let ws = WalletState::default();
        let mut c = SignTransactionComponent::new("wallet-unknown");
        c.set_from_model(&ws);
        assert_eq!(c.group_pubkey_short, "(not in cached list)");
    }
}
