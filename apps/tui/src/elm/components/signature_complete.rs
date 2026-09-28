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

        // Message + hash section. When `signed_hash` is Some we're
        // showing a user-facing message that got EIP-191 wrapped
        // before signing — render both the message (as ASCII if
        // valid UTF-8, else hex) AND the hash that actually went
        // into FROST. When None, just show the raw bytes.
        let (message_label, message_preview) = match &info.signed_hash {
            Some(hash) => {
                let preview = match std::str::from_utf8(&info.message) {
                    Ok(s) => format!("\"{}\"", s),
                    Err(_) => hex::encode(&info.message),
                };
                (
                    format!(
                        "EIP-191 Message ({} bytes) — signed hash: 0x{}",
                        info.message.len(),
                        hex::encode(hash)
                    ),
                    preview,
                )
            }
            None => (
                format!("Message ({} bytes):", info.message.len()),
                hex::encode(&info.message),
            ),
        };
        frame.render_widget(
            Paragraph::new(message_label).style(Style::default().fg(Color::Yellow)),
            rows[0],
        );
        frame.render_widget(
            Paragraph::new(message_preview)
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: true }),
            rows[1],
        );

        // Signature hex.
        let sig_label = signature_label(&info.wallet_id, info.signature.len());
        frame.render_widget(
            Paragraph::new(sig_label).style(Style::default().fg(Color::Yellow)),
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
            Paragraph::new(verify_text).style(
                Style::default()
                    .fg(verify_color)
                    .add_modifier(Modifier::BOLD),
            ),
            rows[6],
        );

        // Hints.
        let hints =
            Paragraph::new("Enter = Done    Esc = Done    C = Copy signature    Ctrl-C = Quit")
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

/// What kind of signature the account produced. secp256k1 FROST is the
/// BIP-340 suite: a Schnorr signature, which spends a Taproot output but is
/// NOT ecrecover-able on Ethereum.
fn signature_label(wallet_id: &str, len: usize) -> String {
    match starlab_core::accounts::parse_child_wallet_id(wallet_id) {
        Some((_, "bitcoin", _)) => {
            format!("BIP-340 Schnorr signature ({len} bytes) — Taproot key-path spend:")
        }
        Some((_, "ethereum", _)) => format!(
            "BIP-340 Schnorr signature ({len} bytes) over the EIP-191 hash (not ecrecover-compatible):"
        ),
        Some(_) => format!("Ed25519 signature ({len} bytes):"),
        None => format!("FROST signature ({len} bytes):"),
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
            signed_hash: None,
            signature: vec![0xABu8; 64],
            verified: true,
        }
    }

    #[test]
    fn set_from_model_copies_snapshot() {
        let ws = WalletState {
            last_completed_signature: Some(info_fixture()),
            ..Default::default()
        };
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

    #[test]
    fn signature_label_names_the_scheme_per_chain() {
        assert!(
            signature_label("r-bitcoin-0", 64).starts_with("BIP-340 Schnorr signature (64 bytes)")
        );
        assert!(signature_label("r-ethereum-0", 64).contains("not ecrecover-compatible"));
        assert!(signature_label("r-solana-0", 64).starts_with("Ed25519 signature"));
        assert!(!signature_label("r-ethereum-0", 64).contains("ecrecover-ready"));
    }

    /// An Ethereum account signs the EIP-191 hash of the typed message;
    /// the screen shows both, and names the signature as BIP-340 Schnorr.
    #[test]
    fn signed_hash_present_renders_ethereum_labels() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let ws = WalletState {
            last_completed_signature: Some(CompletedSignatureInfo {
                request_id: "inline".into(),
                wallet_id: "w-ethereum-0".into(),
                message: b"hello".to_vec(),
                signed_hash: Some(vec![0xDEu8; 32]),
                signature: vec![0xAAu8; 64],
                verified: true,
            }),
            ..Default::default()
        };

        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).expect("TestBackend");
        let mut c = SignatureCompleteComponent::new();
        c.set_from_model(&ws);
        terminal.draw(|f| c.view(f, f.area())).expect("draw");
        let buf = terminal.backend().buffer();
        let area = buf.area();
        let mut rendered = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    rendered.push_str(cell.symbol());
                }
            }
            rendered.push('\n');
        }

        assert!(
            rendered.contains("EIP-191 Message"),
            "signed_hash Some must produce the EIP-191 label; got: {rendered}"
        );
        assert!(
            rendered.contains("BIP-340 Schnorr signature (64 bytes)"),
            "must name the signature scheme; got: {rendered}"
        );
        assert!(
            rendered.contains("\"hello\""),
            "user's typed message must render as ASCII when UTF-8 valid"
        );
    }
}
