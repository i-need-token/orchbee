//! Presents CLI paths, bounded table labels, and diagnostic tokens using shared text escaping.
//! Platform spelling and display-width limits stay here; escape sequences are never split.

use unicode_segmentation::UnicodeSegmentation;

pub(crate) use crate::terminal_text::{escape, quoted, unquoted};

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
