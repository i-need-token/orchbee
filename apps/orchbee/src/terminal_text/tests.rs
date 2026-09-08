use std::borrow::Cow;

use super::{is_forbidden_terminal_scalar, sanitize_terminal_text};

#[test]
fn classifies_only_the_forbidden_terminal_scalars() {
    for character in [
        '\0', '\u{1f}', '\u{7f}', '\u{80}', '\u{85}', '\u{9f}', '\u{2028}', '\u{2029}',
    ] {
        assert!(
            is_forbidden_terminal_scalar(character),
            "expected U+{:04X} to be forbidden",
            u32::from(character)
        );
    }
    for character in [' ', '~', '\u{a0}', '\u{301}', '\u{200d}', '界'] {
        assert!(
            !is_forbidden_terminal_scalar(character),
            "expected U+{:04X} to remain printable",
            u32::from(character)
        );
    }
}

#[test]
fn borrows_safe_text_without_changing_combining_or_double_width_scalars() {
    let source = "safe e\u{301} 界 \u{200d}";

    let sanitized = sanitize_terminal_text(source);

    assert!(matches!(sanitized, Cow::Borrowed(value) if value == source));
}

#[test]
fn replaces_each_forbidden_scalar_without_mutating_the_canonical_source() {
    let source =
        String::from("prefix\0\u{1b}\r\n\u{7f}\u{80}\u{85}\u{9f}\u{2028}\u{2029}e\u{301}界suffix");
    let canonical = source.clone();

    let sanitized = sanitize_terminal_text(&source);

    assert!(matches!(&sanitized, Cow::Owned(_)));
    assert_eq!(sanitized, "prefix����������e\u{301}界suffix");
    assert_eq!(source, canonical);
}
