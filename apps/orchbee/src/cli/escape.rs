//! Safe text presentation for user-controlled data: quoting, unambiguous
//! ASCII escapes for control and formatting characters, and display-width
//! truncation for table cells. Escape sequences are never split.

/// Characters always escaped as `\u{xxxx}` even when they are printable, so a
/// rendered label can never smuggle terminal control or bidirectional state.
fn needs_escape(character: char) -> bool {
    matches!(character,
        '\u{0000}'..='\u{001f}'
        | '\u{007f}'..='\u{009f}'
        | '\u{061c}'
        | '\u{200e}' | '\u{200f}'
        | '\u{2028}' | '\u{2029}'
        | '\u{202a}'..='\u{202e}'
        | '\u{2066}'..='\u{2069}'
        | '\u{feff}')
}

fn escape_character(character: char) -> String {
    match character {
        '\\' => "\\\\".to_owned(),
        '"' => "\\\"".to_owned(),
        '\n' => "\\n".to_owned(),
        '\r' => "\\r".to_owned(),
        '\t' => "\\t".to_owned(),
        character if needs_escape(character) => format!("\\u{{{:04x}}}", character as u32),
        character => character.to_string(),
    }
}

/// `value` as an escaped body without quotes.
pub(crate) fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        escaped.push_str(&escape_character(character));
    }
    escaped
}

/// `value` as a quoted, escaped text value.
pub(crate) fn quoted(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        quoted.push_str(&escape_character(character));
    }
    quoted.push('"');
    quoted
}

const TABLE_LABEL_WIDTH: usize = 80;

#[cfg(unix)]
pub(crate) fn display_width(text: &str) -> usize {
    ratatui::text::Line::from(text).width()
}

// Non-unix builds keep a scalar-count fallback: no snapshot source exists off
// Linux, so table rendering is unreachable there and no new width crate is
// introduced.
#[cfg(not(unix))]
pub(crate) fn display_width(text: &str) -> usize {
    text.chars().count()
}

/// A quoted label truncated so the whole cell fits `TABLE_LABEL_WIDTH` display
/// columns; a truncated value carries the ellipsis inside its closing quote.
pub(crate) fn table_label(value: &str) -> String {
    // A whole label that fits inside the quotes needs no ellipsis.
    let full = escape(value);
    if display_width(&full) <= TABLE_LABEL_WIDTH - 2 {
        return format!("\"{full}\"");
    }
    // Otherwise reserve two quotes plus the ellipsis and stop at a safe point.
    let budget = TABLE_LABEL_WIDTH - 3;
    let mut escaped = String::new();
    let mut used = 0_usize;
    for character in value.chars() {
        let fragment = escape_character(character);
        let width = display_width(&fragment);
        if used + width > budget {
            break;
        }
        escaped.push_str(&fragment);
        used += width;
    }
    format!("\"{escaped}…\"")
}

/// The longest user-controlled token a diagnostic may echo: quoted, escaped,
/// first cut to a 256-Unicode-scalar prefix (scalars, not bytes or columns)
/// marked with `…` when more scalars remain, then to the 80-column cell.
/// Non-Unicode input never reaches this layer — the argv envelope rejects it
/// before tokenization.
const DIAGNOSTIC_TOKEN_SCALARS: usize = 256;

pub(crate) fn diagnostic_token(value: &str) -> String {
    let mut chars = value.chars();
    let mut prefix: String = chars.by_ref().take(DIAGNOSTIC_TOKEN_SCALARS).collect();
    if chars.next().is_some() {
        prefix.push('…');
    }
    table_label(&prefix)
}

#[cfg(test)]
mod tests {
    use super::{diagnostic_token, escape, quoted, table_label};

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
}
