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
//!
//! ```text
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
//! ```
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
    /// A wallet with an ECDSA and a Taproot key can sign as its Ethereum
    /// or Bitcoin account (Tab switches).
    chain_choice: bool,
    bitcoin: bool,
    /// The account-0 address of the chosen chain, e.g.
    /// "Ethereum account 0: 0x…" (empty when the wallet isn't listed).
    account_line: String,
    /// The wallet's display name (label, else id) for the title.
    wallet_name: String,
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
        self.error = ws.sign_error.clone();
        self.chain_choice = ws.sign_chains_for(&self.wallet_id).len() > 1;
        let chain = ws.chosen_sign_chain(&self.wallet_id);
        self.bitcoin = chain == Some("bitcoin");
        self.account_line = account_line(ws, &self.wallet_id, chain);
        self.wallet_name = ws
            .wallet_group(&self.wallet_id)
            .map(|g| g.display_name().to_string())
            .unwrap_or_else(|| self.wallet_id.clone());

        // Pull the signing key's group pubkey from the loaded wallet list
        // so the user has a visual cross-check that they're signing with
        // the right wallet (paranoid but cheap). Ethereum signs with the
        // wallet's ECDSA key, Bitcoin with its Taproot key.
        let curve = match chain {
            Some("ethereum") => starlab_core::ecdsa::ECDSA_CURVE,
            Some("bitcoin") => "secp256k1",
            _ => "ed25519",
        };
        let group = ws.wallet_group(&self.wallet_id);
        self.group_pubkey_short = group
            .as_ref()
            .and_then(|g| g.entry(curve).or_else(|| g.entries().first()))
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

    /// "Chain: Ethereum (ECDSA)    Tab: switch to Bitcoin" for a wallet with
    /// a chain choice.
    fn chain_line(&self) -> Option<String> {
        if !self.chain_choice {
            return None;
        }
        let (current, other) = if self.bitcoin {
            ("Bitcoin (Taproot)", "Ethereum")
        } else {
            ("Ethereum (ECDSA)", "Bitcoin")
        };
        Some(format!("Chain: {current}    Tab: switch to {other}"))
    }
}

/// "Ethereum account 0: 0x…" for the chosen chain (public-only derivation
/// from the wallet's key on that chain); empty if not derivable.
fn account_line(ws: &WalletState, wallet_id: &str, chain: Option<&str>) -> String {
    let (Some(group), Some(chain)) = (ws.wallet_group(wallet_id), chain) else {
        return String::new();
    };
    group
        .accounts(0)
        .into_iter()
        .find(|(name, _, _)| name.eq_ignore_ascii_case(chain))
        .map(|(name, _, address)| format!("{name} account 0: {address}"))
        .unwrap_or_default()
}

impl Component for SignTransactionComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

        let outer_title = format!(" 🖊️  Sign with {} ", self.wallet_name);
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
                Constraint::Length(3), // group key, account, chain lines
                Constraint::Length(1), // spacer
                Constraint::Length(5), // message input
                Constraint::Length(2), // error (if any)
                Constraint::Min(1),    // hints
            ])
            .split(inner);

        // Group-key cross-check.
        let mut header = format!("Group key: {}", self.group_pubkey_short);
        if !self.account_line.is_empty() {
            header.push('\n');
            header.push_str(&self.account_line);
        }
        if let Some(chain) = self.chain_line() {
            header.push('\n');
            header.push_str(&chain);
        }
        let group_line = Paragraph::new(header).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(group_line, rows[0]);

        // Message input with caret.
        let content = format!("{}_", self.message_preview);
        let msg_widget = Paragraph::new(content).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(if self.bitcoin {
                    " Sighash to sign (32-byte hex) "
                } else {
                    " Message to sign "
                })
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
        let hints = Paragraph::new(if self.chain_choice {
            "Enter = Sign    Tab = Chain    Esc = Cancel"
        } else {
            "Enter = Sign    Esc = Cancel"
        })
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
    fn secp256k1_wallet_offers_the_bitcoin_account() {
        let ws = WalletState {
            curve_type: "secp256k1",
            sign_on_bitcoin: true,
            sign_error: Some("bad sighash".to_string()),
            ..Default::default()
        };
        let mut c = SignTransactionComponent::new("w");
        c.set_from_model(&ws);
        assert_eq!(
            c.chain_line().as_deref(),
            Some("Chain: Bitcoin (Taproot)    Tab: switch to Ethereum")
        );
        assert_eq!(c.error.as_deref(), Some("bad sighash"));
    }

    fn entry(curve: &str) -> crate::keystore::WalletMetadata {
        crate::keystore::WalletMetadata::new(
            "w".to_string(),
            "dev".to_string(),
            curve.to_string(),
            2,
            3,
            1,
            "021de2d69979f0a03ea413e7ed6a32ad02111b90d1f03793649157d3e4ee952143".to_string(),
        )
    }

    /// Ethereum signs with the wallet's ECDSA key: the screen names the
    /// scheme and shows that key's account address.
    #[test]
    fn ethereum_signs_with_the_ecdsa_key_and_shows_its_address() {
        let ws = WalletState {
            curve_type: "secp256k1",
            wallets: vec![entry("secp256k1"), entry(starlab_core::ecdsa::ECDSA_CURVE)],
            ..Default::default()
        };
        let mut c = SignTransactionComponent::new("w");
        c.set_from_model(&ws);
        assert_eq!(
            c.chain_line().as_deref(),
            Some("Chain: Ethereum (ECDSA)    Tab: switch to Bitcoin")
        );
        assert!(
            c.account_line.starts_with("Ethereum account 0: 0x"),
            "{}",
            c.account_line
        );
    }

    /// A wallet without an ECDSA key (e.g. only its Taproot file imported)
    /// can only sign as Bitcoin: no Tab choice.
    #[test]
    fn taproot_only_wallet_signs_bitcoin_without_a_choice() {
        let ws = WalletState {
            curve_type: "secp256k1",
            wallets: vec![entry("secp256k1")],
            ..Default::default()
        };
        let mut c = SignTransactionComponent::new("w");
        c.set_from_model(&ws);
        assert!(c.chain_line().is_none());
        assert!(c.bitcoin);
        assert!(c.account_line.starts_with("Bitcoin account 0: bc1p"));
    }

    #[test]
    fn ed25519_wallet_has_no_chain_choice() {
        let ws = WalletState {
            curve_type: "ed25519",
            sign_on_bitcoin: true,
            ..Default::default()
        };
        let mut c = SignTransactionComponent::new("w");
        c.set_from_model(&ws);
        assert!(c.chain_line().is_none());
    }

    #[test]
    fn set_from_model_falls_back_when_wallet_not_in_cache() {
        let ws = WalletState::default();
        let mut c = SignTransactionComponent::new("wallet-unknown");
        c.set_from_model(&ws);
        assert_eq!(c.group_pubkey_short, "(not in cached list)");
    }
}
