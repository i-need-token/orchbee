use super::{unwrapped_text_fits, wrapped_text_fits};
use crate::terminal_text::sanitize_terminal_text;
use ratatui::{layout::Rect, text::Text};

#[test]
fn complete_wrapped_unicode_uses_final_inner_dimensions() {
    let label = "界".repeat(512);
    let text = Text::from(sanitize_terminal_text(&label).into_owned());
    assert!(wrapped_text_fits(&text, Rect::new(0, 0, 64, 16)));
    assert!(!wrapped_text_fits(&text, Rect::new(0, 0, 64, 15)));
    assert!(!wrapped_text_fits(&text, Rect::new(0, 0, 1, 1024)));
    assert!(!wrapped_text_fits(&text, Rect::default()));
}

#[test]
fn combining_text_is_measured_after_terminal_sanitization() {
    let unsafe_label = "e\u{301}\x1b[31m\n界";
    let safe = sanitize_terminal_text(unsafe_label);
    assert!(!safe.contains('\x1b'));
    assert!(!safe.contains('\n'));
    let text = Text::from(safe.as_ref());
    let width = u16::try_from(text.width()).unwrap();
    assert!(wrapped_text_fits(&text, Rect::new(0, 0, width, 1)));
    assert!(unwrapped_text_fits(&safe, Rect::new(0, 0, width, 1)));
    assert!(!unwrapped_text_fits(&safe, Rect::new(0, 0, width - 1, 1)));
}
