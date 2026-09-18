use std::borrow::Cow;

use ratatui::{
    style::Style,
    text::{Line, Span},
};

/// Returns `true` when the scalar must never be painted into the cell grid:
/// control characters, line/paragraph separators, and format controls that
/// reorder or hide text (bidirectional overrides, tag and interlinear
/// annotation characters, and the byte-order mark). Zero-width joiners and
/// combining marks are legal — removing them breaks legitimate graphemes.
pub(crate) fn is_forbidden_terminal_scalar(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{2028}' | '\u{2029}'
                | '\u{061c}'
                | '\u{200e}' | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{206f}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{e0001}'
                | '\u{e0020}'..='\u{e007f}'
        )
}

/// Strips control characters and whole ANSI escape sequences (CSI, OSC
/// terminated by BEL or ST, and the ST-terminated DCS/SOS/PM/APC family) so
/// untrusted titles and labels cannot inject terminal commands or leave
/// printable residue like `31m` behind. Borrows when the input is already
/// safe.
pub(crate) fn sanitize_terminal_text(value: &str) -> Cow<'_, str> {
    if !value.chars().any(is_forbidden_terminal_scalar) {
        return Cow::Borrowed(value);
    }
    let mut sanitized = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        match character {
            '\u{1b}' => skip_escape(&mut characters),
            '\u{9b}' => skip_csi(&mut characters),
            '\u{9d}' => skip_osc(&mut characters),
            '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => skip_string(&mut characters),
            _ if !is_forbidden_terminal_scalar(character) => sanitized.push(character),
            _ => {}
        }
    }
    Cow::Owned(sanitized)
}

/// Consumes the bytes after `ESC`: an intermediate-byte (nF) sequence ending in
/// one final byte, or one of the bracket/string families. A truncated sequence
/// consumes to the end.
fn skip_escape(characters: &mut impl Iterator<Item = char>) {
    match characters.next() {
        Some('[') => skip_csi(characters),
        Some(']') => skip_osc(characters),
        Some('P' | 'X' | '^' | '_') => skip_string(characters),
        Some(mut current @ '\u{20}'..='\u{2f}') => {
            while ('\u{20}'..='\u{2f}').contains(&current) {
                match characters.next() {
                    Some(next) => current = next,
                    None => return,
                }
            }
        }
        _ => {}
    }
}

/// Consumes CSI parameters and intermediates through the final byte
/// (`0x40..=0x7e`).
fn skip_csi(characters: &mut impl Iterator<Item = char>) {
    for character in characters.by_ref() {
        if ('\u{40}'..='\u{7e}').contains(&character) {
            return;
        }
    }
}

/// Consumes an OSC payload through BEL or ST (`ESC \`).
fn skip_osc(characters: &mut impl Iterator<Item = char>) {
    skip_until_terminator(characters, true);
}

/// Consumes a DCS/SOS/PM/APC payload through ST.
fn skip_string(characters: &mut impl Iterator<Item = char>) {
    skip_until_terminator(characters, false);
}

fn skip_until_terminator(characters: &mut impl Iterator<Item = char>, bel_terminates: bool) {
    let mut escaped = false;
    for character in characters.by_ref() {
        if escaped && character == '\\' {
            return;
        }
        if bel_terminates && character == '\u{7}' {
            return;
        }
        escaped = character == '\u{1b}';
    }
}

/// Display width in terminal cells: East Asian wide characters count as two,
/// combining marks as zero. This is the one canonical measure — the same
/// function for measuring, truncating, and laying out chrome text.
#[cfg_attr(not(test), allow(dead_code))] // render consumers land in a later slice
pub(crate) fn display_width(text: &str) -> usize {
    Line::raw(text).width()
}

/// Truncates `text` to fit `budget` cells and appends `marker`, which is
/// budgeted before the text. Returns `text` unchanged when it already fits and
/// an empty string when even the marker exceeds the budget. Splits only on
/// grapheme boundaries, so a wide character or combining sequence is never cut.
#[cfg_attr(not(test), allow(dead_code))] // render consumers land in a later slice
pub(crate) fn truncate_display(text: &str, budget: usize, marker: &str) -> String {
    if display_width(text) <= budget {
        return text.to_owned();
    }
    let marker_width = display_width(marker);
    if marker_width > budget {
        return String::new();
    }
    let text_budget = budget - marker_width;
    let mut truncated = String::new();
    let mut used = 0_usize;
    for grapheme in Span::raw(text).styled_graphemes(Style::default()) {
        let width = display_width(grapheme.symbol);
        if used + width > text_budget {
            break;
        }
        truncated.push_str(grapheme.symbol);
        used += width;
    }
    truncated.push_str(marker);
    truncated
}

#[cfg(test)]
mod tests;
