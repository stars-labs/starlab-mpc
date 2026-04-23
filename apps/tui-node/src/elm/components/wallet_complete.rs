//! WalletComplete Component — terminal screen shown after a successful DKG.
//!
//! Pulls its data from a `CompletedWalletInfo` snapshot that
//! `Message::DKGFinalized` stashes onto `Model.wallet_state`. The
//! component itself is view-only; it does not own any state. Keyboard
//! handling (Enter / Esc → `NavigateHome`) lives in
//! `app.rs::handle_key_event` consistent with the rest of the
//! screen-oriented components in this codebase (see
//! `password_prompt.rs` for the same pattern).
//!
//! Layout (top-to-bottom):
//!     ┌──  🎉 Wallet Ready — <wallet_id>  ──────────────┐
//!     │ Group Verifying Key (<curve>):                 │
//!     │     <66-char hex, monospace>                   │
//!     │                                                │
//!     │ Addresses (N chains):                          │
//!     │    ethereum      0xabc…def                    │
//!     │    bitcoin       bc1qxyz…                     │
//!     │                                                │
//!     │ Enter = Done    Esc = Done    Ctrl-C = Quit    │
//!     └────────────────────────────────────────────────┘

use crate::elm::components::{Id, MpcWalletComponent, UserEvent};
use crate::elm::message::Message;
use crate::elm::model::{CompletedWalletInfo, WalletState};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::Props;
use tuirealm::ratatui::Frame;
use tuirealm::state::State;

#[derive(Debug, Clone, Default)]
pub struct WalletCompleteComponent {
    props: Props,
    info: Option<CompletedWalletInfo>,
    focused: bool,
}

impl WalletCompleteComponent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Called from `app.rs::mount_components` right before the component
    /// is mounted. Pulls whatever `Message::DKGFinalized` stashed onto
    /// `wallet_state.last_finalized_wallet` so the view can render it.
    pub fn set_from_model(&mut self, ws: &WalletState) {
        self.info = ws.last_finalized_wallet.clone();
    }
}

impl Component for WalletCompleteComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

        let info = match self.info.as_ref() {
            Some(i) => i,
            None => {
                // Shouldn't happen in practice — the mount branch only
                // runs when `last_finalized_wallet` is populated — but
                // render a clear diagnostic if it does rather than a
                // blank frame the user can't make sense of.
                let p = Paragraph::new(
                    "WalletComplete: no finalized-wallet snapshot on Model. \
                     This is a bug — please file.",
                )
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red));
                frame.render_widget(p, area);
                return;
            }
        };

        let outer_title = format!(" 🎉 Wallet Ready — {} ", info.wallet_id);
        let outer = Block::default()
            .title(outer_title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightGreen));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        // Main vertical stack. Fixed-height sections so the hints row
        // always sits on the bottom even when the address list is
        // short.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1), // "Group Verifying Key" label
                Constraint::Length(3), // hex-key monospace block (wraps once for 66 chars)
                Constraint::Length(1), // spacer
                Constraint::Length(1), // "Addresses" header
                Constraint::Min(3),    // address list (grows)
                Constraint::Length(1), // hints (bottom)
            ])
            .split(inner);

        // ---- Group key section ----
        let group_label = format!("Group Verifying Key ({}):", info.curve_type);
        frame.render_widget(
            Paragraph::new(group_label).style(Style::default().fg(Color::Yellow)),
            rows[0],
        );
        frame.render_widget(
            Paragraph::new(info.group_pubkey_hex.as_str())
                .style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .wrap(Wrap { trim: true }),
            rows[1],
        );

        // ---- Addresses section ----
        let addr_header = if info.addresses.is_empty() {
            "Addresses: (none derived for this curve)".to_string()
        } else {
            format!(
                "Addresses ({} chain{}):",
                info.addresses.len(),
                if info.addresses.len() == 1 { "" } else { "s" }
            )
        };
        frame.render_widget(
            Paragraph::new(addr_header).style(Style::default().fg(Color::Yellow)),
            rows[3],
        );

        if !info.addresses.is_empty() {
            // Fixed 16-char chain column keeps rows visually aligned
            // without pulling in a full Table widget for what's a few
            // rows. If chain names ever exceed 16 chars we'll switch.
            let rows_text: String = info
                .addresses
                .iter()
                .map(|(chain, addr)| format!("   {:<16} {}", chain, addr))
                .collect::<Vec<_>>()
                .join("\n");
            frame.render_widget(
                Paragraph::new(rows_text)
                    .style(Style::default().fg(Color::White))
                    .wrap(Wrap { trim: false }),
                rows[4],
            );
        }

        // ---- Hints row ----
        let hints =
            Paragraph::new("Enter = Done    Esc = Done    C = Copy group key    Ctrl-C = Quit")
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

impl AppComponent<Message, UserEvent> for WalletCompleteComponent {
    /// Keyboard routing is owned by `app.rs::handle_key_event`. See
    /// `password_prompt.rs` for the same pattern + rationale.
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Message> {
        None
    }
}

impl MpcWalletComponent for WalletCompleteComponent {
    fn id(&self) -> Id {
        Id::WalletComplete
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

    fn info_fixture() -> CompletedWalletInfo {
        CompletedWalletInfo {
            wallet_id: "wallet-dkg_abcd".to_string(),
            group_pubkey_hex:
                "021de2d69979f0a03ea413e7ed6a32ad02111b90d1f03793649157d3e4ee952143".to_string(),
            curve_type: "secp256k1".to_string(),
            addresses: vec![
                ("ethereum".to_string(), "0x1234abcd".to_string()),
                ("bitcoin".to_string(), "bc1qrest".to_string()),
            ],
        }
    }

    #[test]
    fn set_from_model_copies_snapshot() {
        let ws = WalletState {
            last_finalized_wallet: Some(info_fixture()),
            ..Default::default()
        };
        let mut c = WalletCompleteComponent::new();
        c.set_from_model(&ws);
        assert_eq!(c.info.as_ref().map(|i| i.wallet_id.as_str()), Some("wallet-dkg_abcd"));
    }

    #[test]
    fn set_from_model_with_no_snapshot_leaves_component_empty() {
        let ws = WalletState::default();
        let mut c = WalletCompleteComponent::new();
        c.set_from_model(&ws);
        assert!(c.info.is_none());
    }
}
