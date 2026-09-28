//! Export / Import wallet screens (Manage Wallets `e` / `i`).
//!
//! View-only, like `SignTransactionComponent`: keystrokes are routed by
//! `app.rs::handle_key_event` into `Message::Transfer*`, which edit the
//! drafts on `WalletState`; this component renders them. Both screens ask
//! for a file path and the wallet password: Export checks the password
//! unlocks the share before writing the (still encrypted) file, Import uses
//! it to decrypt and validate the file.

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
pub struct WalletTransferComponent {
    props: Props,
    /// `Some(wallet_id)` on the Export screen, `None` on Import.
    export_wallet_id: Option<String>,
    path: String,
    /// Length of the password draft — only bullets are rendered.
    password_len: usize,
    focus_password: bool,
    error: Option<String>,
}

impl WalletTransferComponent {
    pub fn export(wallet_id: impl Into<String>) -> Self {
        Self {
            export_wallet_id: Some(wallet_id.into()),
            ..Self::default()
        }
    }

    pub fn import() -> Self {
        Self::default()
    }

    pub fn set_from_model(&mut self, ws: &WalletState) {
        self.path = ws.transfer_path_draft.clone();
        self.password_len = ws.transfer_password_draft.chars().count();
        self.focus_password = ws.transfer_focus_password;
        self.error = ws.transfer_error.clone();
    }

    fn title(&self) -> String {
        match &self.export_wallet_id {
            Some(id) => format!(" Export wallet {id} "),
            None => " Import wallet ".to_string(),
        }
    }

    fn hints(&self) -> &'static str {
        if self.export_wallet_id.is_some() {
            "The file stays encrypted with the wallet password.    Enter = Export    Tab = Switch field    Esc = Cancel"
        } else {
            "Enter = Import    Tab = Switch field    Esc = Cancel"
        }
    }
}

impl Component for WalletTransferComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

        let outer = Block::default()
            .title(self.title())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // path
                Constraint::Length(3), // password
                Constraint::Length(2), // error
                Constraint::Min(1),    // hints
            ])
            .split(inner);

        let field = |title: &'static str, text: String, focused: bool| {
            Paragraph::new(text).block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(if focused {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    })),
            )
        };
        let caret = |focused: bool| if focused { "_" } else { "" };

        frame.render_widget(
            field(
                " File path ",
                format!("{}{}", self.path, caret(!self.focus_password)),
                !self.focus_password,
            ),
            rows[0],
        );
        frame.render_widget(
            field(
                " Wallet password ",
                format!(
                    "{}{}",
                    "•".repeat(self.password_len),
                    caret(self.focus_password)
                ),
                self.focus_password,
            ),
            rows[1],
        );
        if let Some(ref msg) = self.error {
            frame.render_widget(
                Paragraph::new(msg.as_str())
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                rows[2],
            );
        }
        frame.render_widget(
            Paragraph::new(self.hints())
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
            rows[3],
        );
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

impl AppComponent<Message, UserEvent> for WalletTransferComponent {
    /// All keystrokes flow through `app.rs::handle_key_event`.
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Message> {
        None
    }
}

impl MpcWalletComponent for WalletTransferComponent {
    fn id(&self) -> Id {
        Id::WalletTransfer
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn on_focus(&mut self, _focused: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_renders_the_password_as_bullets_only() {
        let ws = WalletState {
            transfer_path_draft: "/tmp/w.json".to_string(),
            transfer_password_draft: "hunter2".to_string(),
            transfer_focus_password: true,
            ..Default::default()
        };
        let mut c = WalletTransferComponent::import();
        c.set_from_model(&ws);
        assert_eq!(c.path, "/tmp/w.json");
        assert_eq!(c.password_len, 7);
        assert!(!format!("{c:?}").contains("hunter2"));
    }
}
