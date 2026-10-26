//! Safe text presentation for user-controlled data: quoting, unambiguous
//! ASCII escapes for control and formatting characters, and display-width
//! truncation for table cells. Escape sequences are never split.

use unicode_segmentation::UnicodeSegmentation;

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

/// Text without string-literal quoting: printable characters remain copyable,
/// while terminal controls and bidirectional formatting are escaped.
pub(crate) fn unquoted(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len());
    for character in value.chars() {
        if needs_escape(character) {
            rendered.push_str(&escape_character(character));
        } else {
            rendered.push(character);
        }
    }
    rendered
}

/// A path as the platform spells it: on Windows, a drive-absolute or UNC path
/// reported with forward slashes uses backslashes. Controls stay escaped and
/// the result remains copyable.
pub(crate) fn native_path(value: &str) -> String {
    // `\\?\C:\…` is an API spelling; people read and paste `C:\…`.
    let value = match value.strip_prefix("\\\\?\\") {
        Some(rest) if matches!(rest.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic()) => {
            rest
        }
        _ => value,
    };
    platform::native_path(value)
}

#[cfg(windows)]
#[path = "escape/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "escape/unix.rs"]
mod platform;

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

#[cfg(any(unix, windows))]
pub(crate) fn display_width(text: &str) -> usize {
    ratatui::text::Line::from(text).width()
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn display_width(text: &str) -> usize {
    text.chars().count()
}

/// A quoted label truncated so the whole cell fits `TABLE_LABEL_WIDTH` display
/// columns; a truncated value carries the ellipsis inside its closing quote.
pub(crate) fn table_label(value: &str) -> String {
    table_label_width(value, TABLE_LABEL_WIDTH)
}

/// A quoted, escaped label clipped to the supplied terminal-cell width.
/// The closing quote and ellipsis stay visible when the label is shortened.
pub(crate) fn table_label_width(value: &str, width: usize) -> String {
    if width < 2 {
        return if width == 0 {
            String::new()
        } else {
            "…".to_owned()
        };
    }
    // A whole label that fits inside the quotes needs no ellipsis.
    let full = escape(value);
    if display_width(&full) <= width.saturating_sub(2) {
        return format!("\"{full}\"");
    }
    if width == 2 {
        return "…".to_owned();
    }
    // Otherwise reserve two quotes plus the ellipsis and stop at a safe point.
    let budget = width - 3;
    let mut escaped = String::new();
    let mut used = 0_usize;
    for grapheme in value.graphemes(true) {
        let fragment = escape(grapheme);
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
mod tests;
