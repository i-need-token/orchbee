//! What text takes once escaped into a JSON string, as the records' encoder escapes it, which the
//! engine's pages count so each stays inside one record.

/// The bytes `character` takes inside a JSON string.
pub(super) const fn escaped_width(character: char) -> usize {
    match character {
        '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
        '\0'..='\u{1f}' => 6,
        other => other.len_utf8(),
    }
}

/// The bytes `text` takes inside a JSON string, without its quotes.
pub(super) fn escaped_len(text: &str) -> usize {
    text.chars().map(escaped_width).sum()
}
