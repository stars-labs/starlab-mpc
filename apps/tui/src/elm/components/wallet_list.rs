//! Wallet List Component
//!
//! Displays the list of available wallets with their metadata.

use crate::elm::components::{Id, MpcWalletComponent, UserEvent};
use crate::elm::message::Message;
use crate::keystore::WalletGroup;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::props::{Color, Style, TextModifiers};
// tuirealm 4.0 split Alignment into horizontal/vertical. For widget
// layout we want ratatui's plain Alignment.
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::widgets::{
    Block, BorderType as TuiBorderType, Borders as TuiBorders, List, ListItem, ListState, Paragraph,
};
use tuirealm::component::{AppComponent, Component};
use tuirealm::props::Props;
use tuirealm::ratatui::Frame;
use tuirealm::state::{State, StateValue};

/// Wallet list component
#[derive(Debug, Clone)]
pub struct WalletList {
    props: Props,
    wallets: Vec<WalletGroup>,
    selected: usize,
    focused: bool,
    scroll_offset: usize,
}

impl Default for WalletList {
    fn default() -> Self {
        Self::new()
    }
}

impl WalletList {
    pub fn new() -> Self {
        let mut props = Props::default();
        props.set(
            tuirealm::props::Attribute::Title,
            tuirealm::props::AttrValue::String("Manage Wallets".to_string()),
        );
        // Set borders - tuirealm doesn't have Borders::ALL, so we use default

        Self {
            props,
            wallets: Vec::new(),
            selected: 0,
            focused: false,
            scroll_offset: 0,
        }
    }

    pub fn set_wallets(&mut self, wallets: Vec<WalletGroup>) {
        self.wallets = wallets;
        if self.selected >= self.wallets.len() && !self.wallets.is_empty() {
            self.selected = self.wallets.len() - 1;
        }
    }

    /// Sync the selected row from model state at mount time. The
    /// source of truth lives in `Model.ui_state.selected_indices`;
    /// ScrollUp/ScrollDown in the app loop mutate it, and this
    /// setter is how the mounted component learns about it. Without
    /// this wiring arrow keys would fire, mutate the model, and the
    /// component would render stale `self.selected = 0`.
    pub fn set_selected(&mut self, index: usize) {
        let max = self.wallets.len().saturating_sub(1);
        self.selected = index.min(max);
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        let visible_height = 10;
        if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected + 1 - visible_height;
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            // Adjust scroll if needed
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
        }
    }

    fn move_down(&mut self) {
        if self.selected < self.wallets.len().saturating_sub(1) {
            self.selected += 1;
            // Adjust scroll if needed
            let visible_height = 10; // Approximate visible items
            if self.selected >= self.scroll_offset + visible_height {
                self.scroll_offset = self.selected - visible_height + 1;
            }
        }
    }

    fn select_current(&self) -> Option<Message> {
        self.wallets
            .get(self.selected)
            .map(|wallet| Message::SelectWallet {
                wallet_id: wallet.id().to_string(),
            })
    }
}

/// Manage Wallets keys (routed by `app.rs::handle_key_event`).
const KEY_HINTS: &str = "Enter = Sign    e = Export    i = Import    Esc = Back";

/// Details rows for a wallet: its share info, then EVERY account-0 address
/// across its curve keys — Ethereum from the ECDSA key, Bitcoin from the
/// Taproot key, Solana from ed25519. BIP-44 all the way: never the raw
/// group-key address — the root key is a derivation parent only.
fn details_lines(wallet: &WalletGroup) -> Vec<String> {
    let w = wallet.primary();
    let mut lines = vec![format!(
        "Created: {} | Device: {} | Index: {}/{}",
        w.created_at, w.device_id, w.participant_index, w.total_participants
    )];
    let accounts = wallet.accounts(0);
    if accounts.is_empty() {
        lines.push("Account 0: (underivable)".to_string());
    }
    lines.extend(
        accounts
            .into_iter()
            .map(|(chain, _path, address)| format!("Account 0 ({chain}): {address}")),
    );
    lines
}

/// One list row: name, threshold, chains, key prefix, creation date.
fn row_text(wallet: &WalletGroup) -> String {
    let display = wallet.display_name();
    let name = if display.chars().count() > 24 {
        format!("{}…", display.chars().take(23).collect::<String>())
    } else {
        display.to_string()
    };
    let w = wallet.primary();
    let key_prefix = w.group_public_key.chars().take(10).collect::<String>();
    let created = w.created_at.split('T').next().unwrap_or(&w.created_at);
    format!(
        "{}  {}/{}  {}  key:{}  {}",
        name,
        wallet.threshold(),
        wallet.total_participants(),
        wallet.chains_label(),
        key_prefix,
        created,
    )
}

impl Component for WalletList {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let details_text = self
            .wallets
            .get(self.selected)
            .map(details_lines)
            .unwrap_or_default()
            .into_iter()
            .chain(std::iter::once(KEY_HINTS.to_string()))
            .collect::<Vec<_>>()
            .join("\n");
        let details_height = details_text.lines().count() as u16 + 2; // + borders

        // Create layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),                 // List area
                Constraint::Length(details_height), // Details + key hints
            ])
            .split(area);

        // Render wallet list
        if self.wallets.is_empty() {
            // Show empty state
            let empty_msg = Paragraph::new("No wallets found. Create a new wallet to get started.")
                .block(
                    Block::default()
                        .title("Wallets")
                        .borders(TuiBorders::ALL)
                        .border_type(TuiBorderType::Rounded)
                        .border_style(if self.focused {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::Gray)
                        }),
                )
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);

            frame.render_widget(empty_msg, chunks[0]);
        } else {
            // Create list items
            let items: Vec<ListItem> = self
                .wallets
                .iter()
                .enumerate()
                .skip(self.scroll_offset)
                .map(|(i, wallet)| {
                    let is_selected = i == self.selected;

                    let style = if is_selected {
                        if self.focused {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(TextModifiers::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(TextModifiers::BOLD)
                        }
                    } else {
                        Style::default().fg(Color::Gray)
                    };

                    let prefix = if is_selected { "► " } else { "  " };
                    // Name (label, else id), t/n, the wallet's chains, a
                    // group-key prefix and the creation date: two wallets
                    // from the same minute still differ by key.
                    let text = format!("{}{}", prefix, row_text(wallet));

                    ListItem::new(text).style(style)
                })
                .collect();

            // Create the list widget
            let mut list_state = ListState::default();
            list_state.select(Some(self.selected - self.scroll_offset));

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(format!("Wallets ({} total)", self.wallets.len()))
                        .borders(TuiBorders::ALL)
                        .border_type(TuiBorderType::Rounded)
                        .border_style(if self.focused {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::Gray)
                        }),
                )
                .highlight_style(Style::default().bg(Color::DarkGray));

            frame.render_stateful_widget(list, chunks[0], &mut list_state);
        }

        // Selected wallet details + the screen's keys.
        let details_widget = Paragraph::new(details_text)
            .block(
                Block::default()
                    .title("Details")
                    .borders(TuiBorders::ALL)
                    .border_type(TuiBorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::Gray))
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(details_widget, chunks[1]);
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
            Cmd::Move(tuirealm::command::Direction::Up) => {
                self.move_up();
                CmdResult::Changed(self.state())
            }
            Cmd::Move(tuirealm::command::Direction::Down) => {
                self.move_down();
                CmdResult::Changed(self.state())
            }
            Cmd::Submit => CmdResult::Submit(self.state()),
            _ => CmdResult::NoChange,
        }
    }
}

impl AppComponent<Message, UserEvent> for WalletList {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Up,
                modifiers: KeyModifiers::NONE,
            }) => {
                self.move_up();
                None
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }) => {
                self.move_down();
                None
            }
            Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }) => self.select_current(),
            Event::Keyboard(KeyEvent {
                code: Key::Esc,
                modifiers: KeyModifiers::NONE,
            }) => {
                // Properly navigate back, not exit!
                Some(Message::NavigateBack)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('d'),
                modifiers: KeyModifiers::NONE,
            }) => {
                // Delete wallet
                self.wallets
                    .get(self.selected)
                    .map(|wallet| Message::DeleteWallet {
                        wallet_id: wallet.id().to_string(),
                    })
            }
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

impl MpcWalletComponent for WalletList {
    fn id(&self) -> Id {
        Id::WalletList
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
    use crate::keystore::WalletMetadata;

    fn entry(curve: &str) -> WalletMetadata {
        WalletMetadata::new(
            "w1".to_string(),
            "dev".to_string(),
            curve.to_string(),
            2,
            3,
            1,
            "021de2d69979f0a03ea413e7ed6a32ad02111b90d1f03793649157d3e4ee952143".to_string(),
        )
    }

    /// The ECDSA and Taproot files of one wallet id are ONE row whose
    /// details list the Ethereum (ECDSA) and Bitcoin (Taproot) accounts.
    #[test]
    fn one_row_per_wallet_with_ethereum_from_ecdsa_and_bitcoin_from_taproot() {
        let groups = crate::keystore::group_wallets(&[
            entry("secp256k1"),
            entry(starlab_core::ecdsa::ECDSA_CURVE),
        ]);
        assert_eq!(groups.len(), 1);
        let lines = details_lines(&groups[0]);
        assert!(
            lines[1].starts_with("Account 0 (Ethereum): 0x"),
            "{lines:?}"
        );
        assert!(
            lines[2].starts_with("Account 0 (Bitcoin): bc1p"),
            "{lines:?}"
        );
        assert!(row_text(&groups[0]).contains("2/3  ETH BTC"));
    }

    #[test]
    fn a_taproot_only_wallet_has_no_ethereum_account() {
        let groups = crate::keystore::group_wallets(&[entry("secp256k1")]);
        let lines = details_lines(&groups[0]);
        assert!(!lines.iter().any(|l| l.contains("Ethereum")), "{lines:?}");
    }
}
