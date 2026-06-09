//! Wallet Detail Component - Shows detailed wallet information

use crate::elm::components::{Id, UserEvent, MpcWalletComponent};
use crate::elm::message::Message;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::ratatui::Frame;
use tuirealm::props::Props;
use tuirealm::state::State;
use tuirealm::command::{Cmd, CmdResult};
use ratatui::layout::Rect;

#[derive(Debug, Clone, Default)]
pub struct WalletDetail {
    props: Props,
    wallet_id: Option<String>,
    focused: bool,
}

impl WalletDetail {
    pub fn with_wallet_id(wallet_id: String) -> Self {
        Self {
            props: Props::default(),
            wallet_id: Some(wallet_id),
            focused: false,
        }
    }
}

impl Component for WalletDetail {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::style::{Color, Style};
        
        let wallet_info = if let Some(ref id) = self.wallet_id {
            format!("Wallet Details\nID: {}", id)
        } else {
            "No wallet selected".to_string()
        };
        
        let widget = Paragraph::new(wallet_info)
            .block(Block::default()
                .title("Wallet Detail")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)));
        
        frame.render_widget(widget, area);
    }
    
    fn query<'a>(&'a self, attr: tuirealm::props::Attribute) -> Option<tuirealm::props::QueryResult<'a>> {
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