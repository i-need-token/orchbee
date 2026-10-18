use super::{diagnostic_token, escape, quoted, table_label, unquoted};

#[test]
fn unquoted_text_preserves_copyable_paths_quotes_and_unicode() {
    let text = "C:\\Users\\luna\\中文 \"🐝\"";
    assert_eq!(unquoted(text), text);
    assert_eq!(unquoted(r"\\server\share\folder"), r"\\server\share\folder");
}

#[test]
fn unquoted_text_escapes_controls_and_directional_formatting() {
    assert_eq!(
        unquoted("a\nb\r\tc\0\u{1b}[31m\u{85}\u{202e}\u{2028}\u{2029}"),
        "a\\nb\\r\\tc\\u{0000}\\u{001b}[31m\\u{0085}\\u{202e}\\u{2028}\\u{2029}"
    );
}

#[test]
fn escapes_quotes_backslash_and_controls() {
    assert_eq!(escape("plain"), "plain");
    assert_eq!(escape("a\"b\\c"), "a\\\"b\\\\c");
    assert_eq!(escape("a\nb\tc\0d"), "a\\nb\\tc\\u{0000}d");
    assert_eq!(escape("x\u{1b}[0m"), "x\\u{001b}[0m");
    assert_eq!(escape("\u{202e}evil"), "\\u{202e}evil");
    assert_eq!(escape("\u{85}"), "\\u{0085}");
    // Ordinary Unicode survives untouched.
    assert_eq!(escape("中文终端 🐝"), "中文终端 🐝");
}

#[test]
fn quotes_values() {
    assert_eq!(quoted("a b"), "\"a b\"");
    assert_eq!(quoted(""), "\"\"");
}

#[test]
fn truncates_table_labels_on_display_width() {
    let label = "x".repeat(100);
    let cell = table_label(&label);
    assert!(cell.ends_with("…\""));
    assert!(super::display_width(&cell) <= 80);

    let short = table_label("short");
    assert_eq!(short, "\"short\"");

    // Wide characters count by display width, not bytes.
    let wide = "中".repeat(50);
    let cell = table_label(&wide);
    assert!(cell.ends_with("…\""));

    // An escaped control never splits: the budget cut lands before the
    // whole six-column fragment, never inside it.
    let controls = "\u{1b}".repeat(40);
    let cell = table_label(&controls);
    assert!(cell.ends_with("…\""));
    let body = &cell[1..cell.len() - "…\"".len()];
    assert!(body.len().is_multiple_of("\\u{001b}".len()), "{body}");
    assert!(cell.matches("\\u{001b}").count() > 0);
}

#[test]
fn table_labels_budget_and_preserve_complete_graphemes() {
    for (prefix_length, grapheme, retained) in [
        (76, "❤\u{fe0f}", ""),
        (75, "👩\u{200d}💻", "👩\u{200d}💻"),
        (76, "🇨🇳", ""),
    ] {
        let prefix = "x".repeat(prefix_length);
        let cell = table_label(&format!("{prefix}{grapheme}tail"));

        assert!(super::display_width(&cell) <= 80, "{cell}");
        assert_eq!(cell, format!("\"{prefix}{retained}…\""));
    }
}

#[test]
fn diagnostic_tokens_quote_escape_and_bound() {
    // Ordinary tokens render quoted and readable, combining marks intact.
    assert_eq!(diagnostic_token("bogus"), "\"bogus\"");
    assert_eq!(diagnostic_token("e\u{301}项目"), "\"e\u{301}项目\"");
    // Hostile control and bidi input is escaped, never raw.
    let escaped = diagnostic_token("a\u{1b}[0m\u{202e}z\n");
    assert_eq!(escaped, "\"a\\u{001b}[0m\\u{202e}z\\n\"");
    // A token past 256 scalars is cut before display, then to the cell.
    let long = diagnostic_token(&"x".repeat(300));
    assert!(long.ends_with("…\""));
    assert!(super::display_width(&long) <= 80);
    // A combining cluster survives the scalar cut and the column cut at a
    // scalar boundary — never mid-escape.
    let cluster = diagnostic_token(&"e\u{301}".repeat(150));
    assert!(cluster.ends_with("…\""));
    assert!(super::display_width(&cluster) <= 80);
}

#[test]
fn diagnostic_tokens_mark_scalar_truncation() {
    // Exactly 256 scalars is complete input: no marker is added.
    let exact = "\u{200b}".repeat(256);
    assert_eq!(diagnostic_token(&exact), format!("\"{exact}\""));
    // Scalar 257 makes the cut explicit even when the dropped tail is
    // invisible — zero-width and combining input must still mark.
    assert_eq!(
        diagnostic_token(&"\u{200b}".repeat(257)),
        format!("\"{}\u{2026}\"", "\u{200b}".repeat(256))
    );
    assert!(diagnostic_token(&"\u{301}".repeat(300)).ends_with("…\""));
    // Plain long ASCII stays column-bounded.
    let ascii = diagnostic_token(&"x".repeat(300));
    assert!(ascii.ends_with("…\""));
    assert!(super::display_width(&ascii) <= 80);
    // Escaped scalars are never split: each kept `\u{001b}` or `\"`
    // fragment is whole, and the marker stays inside the closing quote.
    let controls = diagnostic_token(&"\u{1b}".repeat(200));
    assert!(controls.ends_with("…\""));
    let body = &controls[1..controls.len() - "…\"".len()];
    assert!(body.len().is_multiple_of("\\u{001b}".len()), "{body}");
    let quotes = diagnostic_token(&"\"".repeat(200));
    assert!(quotes.ends_with("…\""));
    let body = &quotes[1..quotes.len() - "…\"".len()];
    assert!(body.len().is_multiple_of("\\\"".len()), "{body}");
    // A bidi escape wider than the remaining column budget yields whole.
    let bidi = diagnostic_token(&format!("{}\u{202e}", "y".repeat(74)));
    assert_eq!(bidi, format!("\"{}\u{2026}\"", "y".repeat(74)));
}
