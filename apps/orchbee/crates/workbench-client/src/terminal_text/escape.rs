//! Renders controls and directional formatting as visible ASCII escapes, with optional
//! string-literal quoting. Path spelling, display-width limits, and diagnostics belong to callers.

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
pub fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        escaped.push_str(&escape_character(character));
    }
    escaped
}

/// Text without string-literal quoting: printable characters remain copyable,
/// while terminal controls and bidirectional formatting are escaped.
pub fn unquoted(value: &str) -> String {
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

/// `value` as a quoted, escaped text value.
pub fn quoted(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        quoted.push_str(&escape_character(character));
    }
    quoted.push('"');
    quoted
}
