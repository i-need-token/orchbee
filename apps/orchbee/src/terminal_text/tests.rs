use std::borrow::Cow;

use super::{
    display_width, is_forbidden_chrome_scalar, is_forbidden_terminal_scalar,
    sanitize_terminal_symbol, sanitize_terminal_text, truncate_display,
};

#[test]
fn classifies_the_shared_never_paint_scalars() {
    for character in [
        '\0', '\u{1f}', '\u{7f}', '\u{80}', '\u{85}', '\u{9f}', '\u{2028}', '\u{2029}',
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
        '\u{202e}',
        '\u{feff}',
        '\u{e0041}',
    ] {
        assert!(
            !is_forbidden_terminal_scalar(character),
            "expected U+{:04X} to remain printable for program content",
            u32::from(character)
        );
    }
}

#[test]
fn chrome_criterion_adds_reorder_and_hide_formats() {
    for character in [
        '\u{061c}',
        '\u{200e}',
        '\u{200f}',
        '\u{202a}',
        '\u{202e}',
        '\u{2066}',
        '\u{206f}',
        '\u{feff}',
        '\u{fff9}',
        '\u{fffb}',
        '\u{e0001}',
        '\u{e0020}',
        '\u{e0041}',
    ] {
        assert!(
            is_forbidden_chrome_scalar(character),
            "expected U+{:04X} to be forbidden in chrome text",
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
            !is_forbidden_chrome_scalar(character),
            "expected U+{:04X} to remain printable in chrome text",
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
fn csi_aborts_on_new_escape_or_c1_initiator() {
    for (source, expected) in [
        ("a\u{1b}[3\u{1b}[31mb", "ab"),
        ("a\u{1b}[\u{1b}[31mb", "ab"),
        ("a\u{1b}[3\u{1b}]x\u{9c}b", "ab"),
        ("a\u{1b}[3\u{1b}Px\u{9c}b", "ab"),
        ("a\u{1b}[3\u{9d}x\u{9c}b", "ab"),
        ("a\u{9b}3\u{1b}[31mb", "ab"),
        ("a\u{1b}[3\u{9b}31mb", "ab"),
        ("a\u{1b}(\u{9d}x\u{9c}b", "ab"),
        ("a\u{1b}[3\u{1b}", "a"),
        ("a\u{1b}[3\u{1b}[31mb后文", "ab后文"),
    ] {
        assert_eq!(
            sanitize_terminal_text(source),
            expected,
            "source {source:?}"
        );
        assert_eq!(
            sanitize_terminal_symbol(source),
            expected,
            "symbol source {source:?}"
        );
    }
}

#[test]
fn repeated_csi_interruptions_stay_linear() {
    let mut source = String::from("a");
    source.push_str(&"\u{1b}[3".repeat(65_536));
    source.push_str("\u{1b}[31mb");
    assert_eq!(sanitize_terminal_text(&source), "ab");
    assert_eq!(sanitize_terminal_symbol(&source), "ab");
}

#[test]
fn escape_runs_stay_linear_without_recursion() {
    for escapes in [2usize, 3, 1024, 65_536] {
        let source = format!("a{}[31mb", "\u{1b}".repeat(escapes));
        assert_eq!(sanitize_terminal_text(&source), "ab");
        assert_eq!(sanitize_terminal_symbol(&source), "ab");
    }
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
        ("a\u{9d}8;;x\u{9c}b", "ab"),
        ("a\u{9d}8;;x\u{1b}\\b", "ab"),
        ("a\u{1b}]8;;x\u{9c}b", "ab"),
        ("a\u{90}payload\u{9c}b", "ab"),
        ("a\u{1b}Pq\u{9c}b", "ab"),
        ("a\u{9d}x\u{1b}Yb\u{9c}c", "ac"),
        ("prefix\u{9d}t\u{9c}中文字suffix", "prefix中文字suffix"),
        ("a\u{1b}\u{1b}[31mb", "ab"),
        ("a\u{1b}\u{1b}\u{1b}[31mb", "ab"),
        ("a\u{1b}(\u{1b}[31mb", "ab"),
        ("a\u{1b}\u{1b}b", "a"),
        ("a\u{1b}\u{1b}", "a"),
        ("a\u{1b}\u{1b}]x\u{9c}b", "ab"),
        ("a\u{9d}x\u{1b}\u{1b}[31mb", "a"),
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
fn chrome_strips_tag_scalars_and_degrades_tagged_flag_sequences() {
    // Tag letters are stripped as hide-able controls in chrome labels; a
    // subdivision flag such as England (base flag + tag letters + cancel tag)
    // degrades to its base flag. Intentional, recorded in theme-api-03.md —
    // not every emoji form is preserved in chrome text.
    assert_eq!(
        sanitize_terminal_text("🏴\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f}"),
        "🏴"
    );
    let source = "👩\u{200d}💻🇨🇳e\u{301}界";
    assert!(matches!(sanitize_terminal_text(source), Cow::Borrowed(_)));
}

#[test]
fn pane_symbol_keeps_program_formats_and_strips_controls() {
    // Pane symbols are the program's own content: bidirectional marks and
    // emoji tag sequences pass through untouched.
    let flag = "🏴\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f}";
    assert!(matches!(sanitize_terminal_symbol(flag), Cow::Borrowed(_)));
    let rtl = "א\u{200f}ב";
    assert!(matches!(sanitize_terminal_symbol(rtl), Cow::Borrowed(_)));
    for (source, expected) in [
        ("a\u{1b}[31mb", "ab"),
        ("a\u{9d}8;;x\u{9c}b", "ab"),
        ("a\0b", "ab"),
    ] {
        assert_eq!(sanitize_terminal_symbol(source), expected);
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
