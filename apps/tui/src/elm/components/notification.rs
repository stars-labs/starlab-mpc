//! Bottom status line: the newest live toast from
//! `Model.ui_state.notifications`.
//!
//! The frame is split once, up front ([`split_status_line`]): screens (and
//! the centered modal) get everything above the last row, the last row is
//! this line. Nothing is ever drawn over a screen. Toasts expire on their
//! own (`NotificationKind::ttl`), so the line clears itself.

use crate::elm::model::{Notification, NotificationKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Split `area` into (screen area, one-row status line at the bottom).
pub fn split_status_line(area: Rect) -> (Rect, Rect) {
    let [screen, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);
    (screen, status)
}

fn kind_style(kind: &NotificationKind) -> Style {
    match kind {
        NotificationKind::Info => Style::default().fg(Color::Cyan),
        NotificationKind::Success => Style::default().fg(Color::LightGreen),
        NotificationKind::Warning => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        NotificationKind::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    }
}

/// Single-width symbols only — emoji with variation selectors render at
/// inconsistent widths across terminals and shift the rest of the line.
fn kind_icon(kind: &NotificationKind) -> &'static str {
    match kind {
        NotificationKind::Info => "ℹ",
        NotificationKind::Success => "✔",
        NotificationKind::Warning => "⚠",
        NotificationKind::Error => "✖",
    }
}

/// Draw `notification` (if any) on the one-row `area`. Multi-line text is
/// flattened; anything wider than the row is clipped at the edge.
pub fn render_status_line(frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
    let Some(n) = notification else {
        return;
    };
    let style = kind_style(&n.kind);
    let text = n.text.split_whitespace().collect::<Vec<_>>().join(" ");
    let line = Line::from(vec![
        Span::styled(format!(" {} ", kind_icon(&n.kind)), style),
        Span::styled(text, style),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elm::model::UIState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::{Block, Borders};

    fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
        let buf = terminal.backend().buffer();
        (0..buf.area().width)
            .map(|x| buf.cell((x, y)).map_or(" ", |c| c.symbol()))
            .collect()
    }

    /// Draw a stand-in screen (bordered block titled "HEADER") the way
    /// `ElmApp::render` does, plus the status line.
    fn draw(ui: &UIState, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("TestBackend");
        terminal
            .draw(|f| {
                let (screen, status) = split_status_line(f.area());
                f.render_widget(
                    Block::default().borders(Borders::ALL).title("HEADER"),
                    screen,
                );
                render_status_line(f, status, ui.current_notification());
            })
            .expect("draw");
        terminal
    }

    #[test]
    fn notification_is_on_last_row_and_screen_header_is_untouched() {
        let mut ui = UIState::default();
        ui.notify(NotificationKind::Warning, "Disconnected from network");
        let terminal = draw(&ui, 60, 8);

        assert!(
            row(&terminal, 0).contains("HEADER"),
            "row 0: {:?}",
            row(&terminal, 0)
        );
        assert!(!row(&terminal, 0).contains("Disconnected"));
        let last = row(&terminal, 7);
        assert!(
            last.contains("⚠ Disconnected from network"),
            "last row: {last:?}"
        );
        // The screen's bottom border sits directly above the status line.
        assert!(
            row(&terminal, 6).starts_with('└'),
            "row 6: {:?}",
            row(&terminal, 6)
        );
    }

    #[test]
    fn only_the_newest_notification_is_shown() {
        let mut ui = UIState::default();
        ui.notify(NotificationKind::Info, "older toast");
        ui.notify(NotificationKind::Success, "newest toast");
        let terminal = draw(&ui, 60, 5);
        let all: String = (0..5).map(|y| row(&terminal, y)).collect();
        assert!(all.contains("newest toast"));
        assert!(!all.contains("older toast"));
    }

    #[test]
    fn no_notification_leaves_status_line_blank() {
        let terminal = draw(&UIState::default(), 40, 4);
        assert!(row(&terminal, 3).trim().is_empty());
    }

    #[test]
    fn long_text_is_clipped_to_the_row() {
        let mut ui = UIState::default();
        ui.notify(
            NotificationKind::Error,
            format!("line one\nline two {}", "x".repeat(200)),
        );
        let terminal = draw(&ui, 30, 4);
        let last = row(&terminal, 3);
        assert!(last.contains("✖ line one line two"), "last row: {last:?}");
        // Nothing spilled into the screen area.
        assert!(row(&terminal, 0).contains("HEADER"));
    }
}
