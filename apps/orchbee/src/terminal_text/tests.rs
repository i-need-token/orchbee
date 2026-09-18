use std::borrow::Cow;

use super::{
    display_width, is_forbidden_terminal_scalar, sanitize_terminal_text, truncate_display,
};

#[test]
fn classifies_only_the_forbidden_terminal_scalars() {
    for character in [
        '\0',
        '\u{1f}',
        '\u{7f}',
        '\u{80}',
        '\u{85}',
        '\u{9f}',
        '\u{2028}',
        '\u{2029}',
        '\u{202e}',
        '\u{200e}',
        '\u{feff}',
        '\u{e0041}',
    ] {
        assert!(
            is_forbidden_terminal_scalar(character),
            "expected U+{:04X} to be forbidden",
            u32::from(character)
        );
    }
    for character in [
        ' ',
        '~',
        '\u{a0}',
        '\u{301}',
        '\u{200b}',
        '\u{200c}',
        '\u{200d}',
        '界',
        '\u{1f642}',
    ] {
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
fn strips_forbidden_scalars_and_escapes_without_mutating_the_canonical_source() {
    let source =
        String::from("prefix\0\u{1b}\r\n\u{7f}\u{80}\u{85}\u{2028}\u{2029}e\u{301}界suffix");
    let canonical = source.clone();

    let sanitized = sanitize_terminal_text(&source);

    assert!(matches!(&sanitized, Cow::Owned(_)));
    assert_eq!(sanitized, "prefixe\u{301}界suffix");
    assert_eq!(source, canonical);
}

#[test]
fn strips_whole_ansi_sequences_instead_of_leaving_residue() {
    for (source, expected) in [
        ("a\u{1b}[31mb", "ab"),
        ("a\u{1b}[1;38;2;9;9;9mb", "ab"),
        ("a\u{1b}[?25lx", "ax"),
        ("a\u{1b}]8;;http://example\u{7}b", "ab"),
        ("a\u{1b}]8;;http://example\u{1b}\\b", "ab"),
        ("a\u{1b}Pq\u{1b}\\b", "ab"),
        ("a\u{1b}(0b", "ab"),
        ("a\u{9b}31mb", "ab"),
        ("a\u{9d}8;;x\u{7}b", "ab"),
        ("a\u{9f}payload\u{1b}\\b", "ab"),
        ("a\u{1b}[3", "a"),
        ("a\u{1b}]8;;x", "a"),
        ("a\u{1b}", "a"),
        ("a\u{202e}bc", "abc"),
        ("a\u{e0041}b", "ab"),
    ] {
        assert_eq!(
            sanitize_terminal_text(source),
            expected,
            "source {source:?}"
        );
    }
}

#[test]
fn measures_display_width_in_cells() {
    for (text, width) in [
        ("", 0),
        ("abc", 3),
        ("界面", 4),
        ("e\u{301}", 1),
        ("a\u{301}界", 3),
        ("🙂", 2),
        ("…", 1),
    ] {
        assert_eq!(display_width(text), width, "width of {text:?}");
    }
}

#[test]
fn truncates_on_grapheme_boundaries_with_a_budgeted_marker() {
    for (text, budget, marker, expected) in [
        ("hello", 10, "…", "hello"),
        ("hello world", 8, "…", "hello w…"),
        ("界面文字", 5, "…", "界面…"),
        ("界面", 2, "…", "…"),
        ("界面", 1, "…", "…"),
        ("e\u{301}xyz", 3, "…", "e\u{301}x…"),
        ("hello", 0, "…", ""),
        ("hello", 1, "..", ""),
        ("", 4, "…", ""),
    ] {
        assert_eq!(truncate_display(text, budget, marker), expected);
        assert!(display_width(expected) <= budget);
    }
}
