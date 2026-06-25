//! Wallet Detail Component — wallet metadata + the BIP-44 ACCOUNTS table.
//!
//! "BIP-44 all the way": the root group key is a derivation parent only —
//! it is never rendered as a wallet address. This screen shows the account
//! table instead (account index × chain × address), derived public-only via
//! `starlab_core::accounts` — the shared single source of truth, so every
//! row matches the CLI's `wallet accounts` byte-for-byte.
//!
//! Keyboard routing is owned by `app.rs::handle_key_event` (same pattern as
//! `wallet_complete.rs`): '+' / '-' fire `Message::AccountsShowMore/Less`,
//! which mutate `Model.ui_state.accounts_shown`; the remount pushes the new
//! count back in through `set_accounts_shown`.

use crate::elm::components::{Id, MpcWalletComponent, UserEvent};
use crate::elm::message::Message;
use crate::keystore::WalletMetadata;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::Props;
use tuirealm::ratatui::Frame;
use tuirealm::state::State;

/// How many accounts (0..n) the table shows before the user presses '+'.
pub const DEFAULT_ACCOUNTS_SHOWN: u32 = 5;

#[derive(Debug, Clone)]
pub struct WalletDetail {
    props: Props,
    wallet_id: Option<String>,
    /// Every curve entry the keystore holds for this wallet id — the
    /// unified DKG persists the same id under ed25519/ AND secp256k1/,
    /// and the accounts table must cover both curves' chains.
    entries: Vec<WalletMetadata>,
    /// How many accounts (0..n) to derive rows for.
    accounts_shown: u32,
    focused: bool,
}

impl Default for WalletDetail {
    fn default() -> Self {
        Self {
            props: Props::default(),
            wallet_id: None,
            entries: Vec::new(),
            accounts_shown: DEFAULT_ACCOUNTS_SHOWN,
            focused: false,
        }
    }
}

impl WalletDetail {
    /// Called from `app.rs::mount_components` — hands over the wallet id
    /// plus all of its curve entries from `Model.wallet_state.wallets`.
    pub fn set_wallet(&mut self, wallet_id: String, entries: Vec<WalletMetadata>) {
        self.wallet_id = Some(wallet_id);
        self.entries = entries;
    }

    /// Sync the account count from `Model.ui_state.accounts_shown` at
    /// mount time ('+' / '-' mutate the model, the remount lands here).
    pub fn set_accounts_shown(&mut self, n: u32) {
        self.accounts_shown = n.max(1);
    }

    /// Derive the table rows: `(account, chain, path, address)` for
    /// accounts `0..accounts_shown` across every curve entry. Public-only
    /// derivation — no key share, no password. Undecodable/underivable
    /// entries are skipped rather than failing the whole screen.
    fn account_rows(&self) -> Vec<[String; 4]> {
        let mut rows = Vec::new();
        for account in 0..self.accounts_shown {
            for w in &self.entries {
                let Ok(group) = hex::decode(&w.group_public_key) else {
                    continue;
                };
                let Ok(addrs) =
                    starlab_core::accounts::account_addresses(&w.curve_type, &group, account)
                else {
                    continue;
                };
                for (chain, path, address) in addrs {
                    rows.push([account.to_string(), chain, path, address]);
                }
            }
        }
        rows
    }
}

impl Component for WalletDetail {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table};

        let title = match self.wallet_id.as_deref() {
            Some(id) => format!(" Wallet — {} ", id),
            None => " Wallet Detail ".to_string(),
        };
        let outer = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if self.focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::Gray)
            });
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        if self.entries.is_empty() {
            let p = Paragraph::new("Wallet not found in keystore.")
                .style(Style::default().fg(Color::Red));
            frame.render_widget(p, inner);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // wallet metadata summary
                Constraint::Min(3),    // accounts table (grows)
                Constraint::Length(1), // hints
            ])
            .split(inner);

        // ---- Metadata summary (no root address — BIP-44 all the way) ----
        let w0 = &self.entries[0];
        let mut curves: Vec<&str> = self.entries.iter().map(|w| w.curve_type.as_str()).collect();
        curves.sort_unstable();
        let summary = format!(
            "Name: {}\nThreshold: {}/{}  Curves: {}  Created: {}",
            w0.display_name(),
            w0.threshold,
            w0.total_participants,
            curves.join("+"),
            w0.created_at.split('T').next().unwrap_or(&w0.created_at),
        );
        frame.render_widget(
            Paragraph::new(summary).style(Style::default().fg(Color::Gray)),
            chunks[0],
        );

        // ---- Accounts table ----
        let header = Row::new(vec![
            Cell::from("Acct"),
            Cell::from("Chain"),
            Cell::from("Path"),
            Cell::from("Address"),
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        let rows: Vec<Row> = self
            .account_rows()
            .into_iter()
            .map(|[account, chain, path, address]| {
                Row::new(vec![
                    Cell::from(account),
                    Cell::from(chain),
                    Cell::from(path),
                    Cell::from(address),
                ])
                .style(Style::default().fg(Color::White))
            })
            .collect();
        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Length(10),
                Constraint::Length(20),
                Constraint::Min(20),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!("Accounts (0..{})", self.accounts_shown - 1))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(table, chunks[1]);

        // ---- Hints row ----
        let hints = Paragraph::new("+ = more accounts    - = fewer    Esc = Back")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hints, chunks[2]);
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

impl AppComponent<Message, UserEvent> for WalletDetail {
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Message> {
        None
    }
}

impl MpcWalletComponent for WalletDetail {
    fn id(&self) -> Id {
        Id::WalletDetail
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

    fn meta(curve: &str, group_hex: &str) -> WalletMetadata {
        WalletMetadata::new(
            "w1".to_string(),
            "d1".to_string(),
            curve.to_string(),
            2,
            3,
            1,
            group_hex.to_string(),
        )
    }

    /// A valid compressed secp256k1 point (the generator).
    const SECP_G: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    #[test]
    fn account_rows_cover_each_account_and_chain() {
        let mut c = WalletDetail::default();
        c.set_wallet("w1".to_string(), vec![meta("secp256k1", SECP_G)]);
        c.set_accounts_shown(2);
        let rows = c.account_rows();
        // 2 accounts × 2 secp chains (Ethereum, Bitcoin)
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0][0], "0");
        assert_eq!(rows[0][1], "Ethereum");
        assert!(rows[0][3].starts_with("0x"));
        assert_eq!(rows[3][0], "1");
        assert_eq!(rows[3][1], "Bitcoin");
        assert!(rows[3][3].starts_with("bc1"));
    }

    #[test]
    fn account_rows_skip_undecodable_group_keys() {
        let mut c = WalletDetail::default();
        c.set_wallet("w1".to_string(), vec![meta("secp256k1", "not-hex")]);
        assert!(c.account_rows().is_empty());
    }

    #[test]
    fn accounts_shown_floors_at_one() {
        let mut c = WalletDetail::default();
        c.set_accounts_shown(0);
        assert_eq!(c.accounts_shown, 1);
    }
}
