//! Notification Bar — renders toasts pushed to `Model.ui_state.notifications`.
//!
//! The app-level render reserves a 3-row strip at the top whenever the
//! notifications list is non-empty (see `ElmApp::render` + its
//! `notifications.is_empty()` check). This component fills that strip
//! with the newest notifications, one per row, colored by
//! `NotificationKind`. Up to 3 entries are shown — older ones scroll
//! off the top; they stay in `Model` until an explicit `DismissNotification`
//! removes them.
//!
//! Historical note: the `view` method used to be a bare stub
//! (`// Notification rendering will be implemented`) which meant every
//! toast in the app — DKG complete, wallet finalized, copy success,
//! error warnings, etc. — pushed into the Model but rendered
//! **nothing**. Full-flow test caught it (/docs: Bug A).

use crate::elm::components::{Id, MpcWalletComponent, UserEvent};
use crate::elm::message::Message;
use crate::elm::model::{Notification, NotificationKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::Props;
use tuirealm::ratatui::Frame;
use tuirealm::state::State;

/// How many notifications to render at once. The reserved strip is
/// 3 rows, unbordered — we use 2 rows for the newest toasts and
/// reserve the last row for a `(+N older)` overflow hint so the
/// user knows some were dropped.
const MAX_VISIBLE: usize = 2;

#[derive(Debug, Clone, Default)]
pub struct NotificationBar {
    props: Props,
    notifications: Vec<Notification>,
    focused: bool,
}

impl NotificationBar {
    pub fn set_notifications(&mut self, notifications: Vec<Notification>) {
        self.notifications = notifications;
    }

    /// Pull notifications off `Model.ui_state` at mount time. Mirror of
    /// the `set_from_model` pattern used by WalletComplete /
    /// SignatureComplete / PasswordPrompt — keeps the component's
    /// rendered state in sync with the Model without leaking the full
    /// Model into the component.
    pub fn set_from_model(&mut self, model: &crate::elm::Model) {
        self.notifications = model.ui_state.notifications.clone();
    }
}

fn kind_style(kind: &NotificationKind) -> Style {
    // Fg colors chosen to contrast against the default terminal
    // background at typical brightness levels. Warning/Error get BOLD
    // too so the user's eye snaps to them even in a busy log.
    match kind {
        NotificationKind::Info => Style::default().fg(Color::Cyan),
        NotificationKind::Success => Style::default().fg(Color::LightGreen),
        NotificationKind::Warning => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        NotificationKind::Error => Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD),
    }
}

fn kind_icon(kind: &NotificationKind) -> &'static str {
    match kind {
        NotificationKind::Info => "ℹ️ ",
        NotificationKind::Success => "✅",
        NotificationKind::Warning => "⚠️ ",
        NotificationKind::Error => "❌",
    }
}

impl Component for NotificationBar {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        if self.notifications.is_empty() {
            return;
        }

        // Take the last (newest) N notifications. `Vec::push` appends,
        // so the last slice element is the most recent.
        let mut tail: Vec<&Notification> = self
            .notifications
            .iter()
            .rev()
            .take(MAX_VISIBLE)
            .collect();
        // Undo the reverse so oldest-of-the-visible renders on top and
        // newest is right above the main content — feels natural since
        // the eye lands on the bottom of the bar first.
        tail.reverse();

        let more_hint = if self.notifications.len() > MAX_VISIBLE {
            format!(
                " (+{} older)",
                self.notifications.len() - MAX_VISIBLE
            )
        } else {
            String::new()
        };

        let lines: Vec<Line> = tail
            .into_iter()
            .map(|n| {
                Line::from(vec![
                    Span::styled(
                        format!("{} ", kind_icon(&n.kind)),
                        kind_style(&n.kind),
                    ),
                    Span::styled(n.text.clone(), kind_style(&n.kind)),
                ])
            })
            .collect();

        // If there are more notifications than we're showing, append a
        // tail line so the user knows some were skipped. Fits inline
        // because we're not spending rows on a border.
        let mut all_lines = lines;
        if !more_hint.is_empty() {
            all_lines.push(Line::from(Span::styled(
                more_hint.trim().to_string(),
                Style::default().fg(Color::DarkGray),
            )));
        }

        // No outer border — the reserved strip is only 3 rows, so
        // every row counts. The kind_style coloring distinguishes the
        // strip from the main content below.
        let widget = Paragraph::new(all_lines);
        frame.render_widget(widget, area);
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

impl AppComponent<Message, UserEvent> for NotificationBar {
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Message> {
        None
    }
}

impl MpcWalletComponent for NotificationBar {
    fn id(&self) -> Id {
        Id::NotificationBar
    }

    fn is_visible(&self) -> bool {
        !self.notifications.is_empty()
    }

    fn on_focus(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use uuid::Uuid;

    fn notif(kind: NotificationKind, text: &str) -> Notification {
        Notification {
            id: Uuid::new_v4().to_string(),
            text: text.to_string(),
            kind,
            timestamp: Utc::now(),
            dismissible: true,
        }
    }

    fn render(bar: &mut NotificationBar) -> String {
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).expect("TestBackend");
        terminal
            .draw(|f| bar.view(f, f.area()))
            .expect("draw");
        let buf = terminal.backend().buffer();
        let area = buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn empty_notifications_renders_nothing() {
        let mut bar = NotificationBar::default();
        let rendered = render(&mut bar);
        // Empty — no meaningful text. Accept whitespace, reject any
        // leftover markers from a stale render.
        assert!(rendered.trim().is_empty(), "got {:?}", rendered);
    }

    #[test]
    fn one_notification_renders_text_and_icon() {
        let mut bar = NotificationBar::default();
        bar.set_notifications(vec![notif(
            NotificationKind::Success,
            "Wallet created",
        )]);
        let rendered = render(&mut bar);
        assert!(
            rendered.contains("Wallet created"),
            "notification text must render; got {:?}",
            &rendered[..rendered.len().min(200)]
        );
    }

    #[test]
    fn when_more_than_visible_show_plus_hint() {
        let mut bar = NotificationBar::default();
        bar.set_notifications(vec![
            notif(NotificationKind::Info, "first"),
            notif(NotificationKind::Info, "second"),
            notif(NotificationKind::Info, "third"),
            notif(NotificationKind::Info, "fourth"),
        ]);
        let rendered = render(&mut bar);
        assert!(
            rendered.contains("+2 older"),
            "more-than-MAX_VISIBLE must show hint; got {:?}",
            &rendered[..rendered.len().min(400)]
        );
        // Newest two ("third", "fourth") are visible, oldest two are not.
        assert!(rendered.contains("fourth"));
        assert!(rendered.contains("third"));
        assert!(!rendered.contains("first"));
        assert!(!rendered.contains("second"));
    }
}
