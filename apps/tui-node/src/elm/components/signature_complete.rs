//! SignatureComplete — terminal screen after a successful FROST signature.
//!
//! Parallel to `wallet_complete.rs`: view-only, reads the
//! `CompletedSignatureInfo` snapshot off `Model.wallet_state`,
//! renders the signature + verified status. Keyboard (Enter/Esc →
//! NavigateBack → MainMenu) is owned by `app.rs::handle_key_event`.

use crate::elm::components::{Id, MpcWalletComponent, UserEvent};
use crate::elm::message::Message;
use crate::elm::model::{CompletedSignatureInfo, WalletState};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::Props;
use tuirealm::ratatui::Frame;
use tuirealm::state::State;

#[derive(Debug, Clone, Default)]
pub struct SignatureCompleteComponent {
    props: Props,
    info: Option<CompletedSignatureInfo>,
    focused: bool,
}

impl SignatureCompleteComponent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Copy the snapshot off `Model.wallet_state.last_completed_signature`
    /// at mount time. Mirrors WalletCompleteComponent::set_from_model.
    pub fn set_from_model(&mut self, ws: &WalletState) {
        self.info = ws.last_completed_signature.clone();
    }
}

impl Component for SignatureCompleteComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

        let info = match self.info.as_ref() {
            Some(i) => i,
            None => {
                let p = Paragraph::new(
                    "SignatureComplete: no signature snapshot on Model. \
                     This is a bug — please file.",
                )
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red));
                frame.render_widget(p, area);
                return;
            }
        };

        let border_color = if info.verified {
            Color::LightGreen
        } else {
            // If we somehow reached this screen without verification
            // passing, paint the border red so the discrepancy is
            // visible at a glance.
            Color::Red
        };
        let title = format!(
            " {} Signature Complete — {} ",
            if info.verified { "✅" } else { "⚠️" },
            info.wallet_id
        );
        let outer = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1), // message label
                Constraint::Length(3), // message hex
                Constraint::Length(1), // spacer
                Constraint::Length(1), // sig label
                Constraint::Length(4), // sig hex (can wrap across 2 lines)
                Constraint::Length(1), // spacer
                Constraint::Length(1), // verified
                Constraint::Min(1),    // hints
            ])
            .split(inner);

        // Message hex (what was signed).
        frame.render_widget(
            Paragraph::new(format!("Message ({} bytes):", info.message.len()))
                .style(Style::default().fg(Color::Yellow)),
            rows[0],
        );
        let msg_hex = hex::encode(&info.message);
        frame.render_widget(
            Paragraph::new(msg_hex)
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: true }),
            rows[1],
        );

        // Signature hex.
        frame.render_widget(
            Paragraph::new(format!("FROST signature ({} bytes):", info.signature.len()))
                .style(Style::default().fg(Color::Yellow)),
            rows[3],
        );
        let sig_hex = hex::encode(&info.signature);
        frame.render_widget(
            Paragraph::new(sig_hex)
                .style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .wrap(Wrap { trim: true }),
            rows[4],
        );

        // Verified badge.
        let (verify_text, verify_color) = if info.verified {
            (
                "Verified: YES (signature verifies under group key)".to_string(),
                Color::LightGreen,
            )
        } else {
            (
                "Verified: NO — DO NOT use this signature".to_string(),
                Color::Red,
            )
        };
        frame.render_widget(
            Paragraph::new(verify_text)
                .style(Style::default().fg(verify_color).add_modifier(Modifier::BOLD)),
            rows[6],
        );

        // Hints.
        let hints = Paragraph::new("Enter = Done    Esc = Done    Ctrl-C = Quit")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hints, rows[7]);
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

impl AppComponent<Message, UserEvent> for SignatureCompleteComponent {
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Message> {
        None
    }
}

impl MpcWalletComponent for SignatureCompleteComponent {
    fn id(&self) -> Id {
        Id::SignatureComplete
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

    fn info_fixture() -> CompletedSignatureInfo {
        CompletedSignatureInfo {
            request_id: "inline".to_string(),
            wallet_id: "wallet-dkg_abcd".to_string(),
            message: b"hello world".to_vec(),
            signature: vec![0xABu8; 64],
            verified: true,
        }
    }

    #[test]
    fn set_from_model_copies_snapshot() {
        let mut ws = WalletState::default();
        ws.last_completed_signature = Some(info_fixture());
        let mut c = SignatureCompleteComponent::new();
        c.set_from_model(&ws);
        assert_eq!(
            c.info.as_ref().map(|i| i.wallet_id.as_str()),
            Some("wallet-dkg_abcd")
        );
    }

    #[test]
    fn set_from_model_with_no_snapshot_leaves_component_empty() {
        let ws = WalletState::default();
        let mut c = SignatureCompleteComponent::new();
        c.set_from_model(&ws);
        assert!(c.info.is_none());
    }
}
