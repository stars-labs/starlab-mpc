//! Log-safe `{:?}` rendering for Elm messages, commands and terminal events.
//!
//! The Elm loop logs every `Message` / `Command` / key event it processes.
//! Several carry a cleartext wallet password (`SubmitPassword { value }`,
//! `password` fields on the headless / unlock / finalize variants), and every
//! typed or pasted character may be a password keystroke (`PasswordTypeChar`,
//! `KeyCode::Char`, `Event::Paste`). This masks those before they reach the
//! log file, without changing the public message types the desktop app builds.

use std::fmt::Debug;

const MASK: &str = "<redacted>";

/// `{:?}` of `value` with every password-bearing field masked.
pub fn redacted(value: &impl Debug) -> String {
    let s = format!("{value:?}");
    let s = mask_after(&s, "password: \"", '"');
    let s = mask_after(&s, "SubmitPassword { value: \"", '"');
    let s = mask_after(&s, "Paste(\"", '"');
    // Covers `PasswordTypeChar('x')` and crossterm / tuirealm `Char('x')`.
    mask_after(&s, "Char('", '\'')
}

/// Replace the quoted body that follows each `marker` (up to the next
/// unescaped `quote`) with [`MASK`].
fn mask_after(s: &str, marker: &str, quote: char) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(marker) {
        out.push_str(&rest[..i + marker.len()]);
        out.push_str(MASK);
        let body = &rest[i + marker.len()..];
        let mut escaped = false;
        let mut end = body.len();
        for (j, c) in body.char_indices() {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == quote {
                end = j;
                break;
            }
        }
        rest = &body[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elm::command::Command;
    use crate::elm::message::Message;

    #[test]
    fn masks_submit_password_value() {
        let s = redacted(&Message::SubmitPassword {
            value: "hunter2\"x".to_string(),
        });
        assert!(!s.contains("hunter2"), "{s}");
        assert!(s.contains(MASK), "{s}");
    }

    #[test]
    fn masks_password_fields_even_when_nested() {
        let s = redacted(&Command::SendMessage(Message::HeadlessSign {
            wallet_id: "w1".to_string(),
            message: "hello".to_string(),
            encoding: "utf8".to_string(),
            password: "s3cret".to_string(),
        }));
        assert!(!s.contains("s3cret"), "{s}");
        assert!(s.contains("w1") && s.contains("hello"), "{s}");
    }

    #[test]
    fn masks_typed_password_characters() {
        for c in ['p', '\'', ')'] {
            let s = redacted(&Message::PasswordTypeChar(c));
            assert_eq!(s, format!("PasswordTypeChar('{MASK}')"), "{c:?}");
        }
    }

    #[test]
    fn masks_key_events_and_pastes() {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        let key = Event::Key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        assert!(!redacted(&key).contains("'z'"), "{}", redacted(&key));
        let paste = Event::Paste("hunter2".to_string());
        assert!(
            !redacted(&paste).contains("hunter2"),
            "{}",
            redacted(&paste)
        );
        assert_eq!(redacted(&KeyCode::Enter), "Enter");
    }

    #[test]
    fn leaves_other_messages_alone() {
        assert_eq!(redacted(&Message::PasswordBackspace), "PasswordBackspace");
    }
}
