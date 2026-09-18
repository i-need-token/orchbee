use std::borrow::Cow;

use ratatui::{
    style::Style,
    text::{Line, Span},
};

/// The shared never-paint criterion for pane cells and terminal output:
/// control characters and line/paragraph separators. Deliberately minimal —
/// program content may legitimately carry bidirectional marks, tag letters,
/// and other format scalars, so chrome-only exclusions live in
/// [`is_forbidden_chrome_scalar`].
pub(crate) fn is_forbidden_terminal_scalar(character: char) -> bool {
    character.is_control() || matches!(character, '\u{2028}' | '\u{2029}')
}

/// Format scalars that reorder or hide text: bidirectional marks, embeddings,
/// overrides, and isolates; the byte-order mark; interlinear annotation
/// anchors; and the tag block. Zero-width joiners stay legal — removing them
/// merges or re-shapes legitimate grapheme clusters.
fn is_reorder_or_hide_format(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}' | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

/// The chrome criterion: the pane criterion plus format scalars that reorder
/// or hide text. Applies to external labels and titles only — never to a
/// program's own pane cells.
fn is_forbidden_chrome_scalar(character: char) -> bool {
    is_forbidden_terminal_scalar(character) || is_reorder_or_hide_format(character)
}

/// Strips control characters and whole ANSI escape sequences from a pane cell
/// symbol before it reaches the terminal writer, so program output cannot
/// inject terminal commands or leave printable residue like `31m` behind.
/// Format scalars pass through: bidirectional marks and emoji tag sequences
/// are the program's own content.
pub(crate) fn sanitize_terminal_symbol(value: &str) -> Cow<'_, str> {
    project_with(value, is_forbidden_terminal_scalar, false).0
}

/// [`sanitize_terminal_symbol`] plus the format scalars of
/// [`is_forbidden_chrome_scalar`], for untrusted chrome labels and titles.
pub(crate) fn sanitize_terminal_text(value: &str) -> Cow<'_, str> {
    project_with(value, is_forbidden_chrome_scalar, false).0
}

/// The single-pass safe projection of a raw draft text: the chrome-safe
/// display text plus the raw→visible byte map a field needs to place a raw
/// cursor inside it. `raw_to_visible[i]` is the display byte index the raw
/// prefix `value[..i]` projects to under the whole-text parse — bytes inside
/// a stripped sequence map to the position where its removal landed. `None`
/// is the identity projection of text that needed no change.
pub(crate) struct TerminalTextProjection<'a> {
    display: Cow<'a, str>,
    raw_to_visible: Option<Vec<usize>>,
}

impl<'a> TerminalTextProjection<'a> {
    /// The chrome-safe text the whole raw value projects to.
    pub(crate) fn display(&self) -> &Cow<'a, str> {
        &self.display
    }

    /// The display byte index a raw byte boundary projects to.
    pub(crate) fn visible_index(&self, raw_index: usize) -> usize {
        match &self.raw_to_visible {
            Some(map) => map[raw_index.min(map.len() - 1)],
            None => raw_index.min(self.display.len()),
        }
    }
}

/// Projects `value` through [`sanitize_terminal_text`]'s single parse and
/// keeps the raw→visible byte map, so a raw cursor lands inside the display
/// text exactly where the parse put it — never re-parsing a raw middle
/// fragment as if from ground state.
pub(crate) fn project_terminal_text(value: &str) -> TerminalTextProjection<'_> {
    let (display, raw_to_visible) = project_with(value, is_forbidden_chrome_scalar, true);
    TerminalTextProjection {
        display,
        raw_to_visible,
    }
}

/// Shared sanitizer loop, optionally recording the raw→visible byte map of
/// the same parse. Escape sequences are recognized and consumed whole — CSI
/// through its final byte, OSC through BEL or either ST form (`ESC \` or the
/// C1 `U+009C`), DCS/SOS/PM/APC through ST, and nF sequences through their
/// final byte. The C1 initiators `0x90/0x98/0x9B/0x9D/0x9E/0x9F` take the
/// same paths. A sequence truncated at end-of-input consumes to the end.
fn project_with(
    value: &str,
    forbidden: impl Fn(char) -> bool + Copy,
    want_map: bool,
) -> (Cow<'_, str>, Option<Vec<usize>>) {
    if !value.chars().any(forbidden) {
        return (Cow::Borrowed(value), None);
    }
    let mut sanitized = String::with_capacity(value.len());
    let mut map = want_map.then(|| vec![0; value.len() + 1]);
    let mut characters = value.char_indices().peekable();
    while let Some((start, character)) = characters.next() {
        match character {
            '\u{1b}' => skip_sequence(&mut characters, Sequence::Dispatch),
            '\u{9b}' => skip_sequence(&mut characters, Sequence::Csi),
            '\u{9d}' => skip_sequence(&mut characters, Sequence::Osc),
            '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => {
                skip_sequence(&mut characters, Sequence::String)
            }
            _ if !forbidden(character) => {
                if let Some(map) = &mut map {
                    map[start] = sanitized.len();
                }
                sanitized.push(character);
                if let Some(map) = &mut map {
                    map[start + 1..start + character.len_utf8()].fill(sanitized.len());
                }
                continue;
            }
            _ => {}
        }
        // The scalar — alone or with the sequence it started — emitted
        // nothing: every raw boundary it covered maps to the current end.
        if let Some(map) = &mut map {
            let end = characters.peek().map_or(value.len(), |&(index, _)| index);
            map[start..end].fill(sanitized.len());
        }
    }
    if let Some(map) = &mut map {
        map[value.len()] = sanitized.len();
    }
    (Cow::Owned(sanitized), map)
}

/// Parse state for one in-flight escape sequence.
enum Sequence {
    /// Just after `ESC` — awaiting the dispatch byte.
    Dispatch,
    /// Inside nF intermediates (`0x20..=0x2f`); the next byte ends it.
    Intermediates,
    /// Inside CSI; ends at a final byte (`0x40..=0x7e`).
    Csi,
    /// Inside OSC; ends at BEL or ST.
    Osc,
    /// Inside DCS/SOS/PM/APC; ends at ST.
    String,
}

/// Maps a byte that aborts a control-function sequence to the state the new
/// sequence enters: `ESC` re-enters dispatch, a C1 initiator starts its own
/// family. Anything else is sequence content.
fn abort_state(character: char) -> Option<Sequence> {
    match character {
        '\u{1b}' => Some(Sequence::Dispatch),
        '\u{9b}' => Some(Sequence::Csi),
        '\u{9d}' => Some(Sequence::Osc),
        '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => Some(Sequence::String),
        _ => None,
    }
}

/// Consumes an escape sequence starting in `state`. A fresh `ESC` or C1
/// initiator inside dispatch, intermediates, or CSI aborts the sequence in
/// progress and re-dispatches; inside an OSC/string payload they remain
/// payload bytes, which are consumed only through the payload's terminator.
/// A truncated sequence consumes to the end. The loop stays iterative so
/// arbitrary interruption runs cannot overflow the stack.
fn skip_sequence(characters: &mut impl Iterator<Item = (usize, char)>, mut state: Sequence) {
    let mut escaped = false;
    for (_, character) in characters.by_ref() {
        match state {
            Sequence::Dispatch => match character {
                '\u{1b}' => {}
                '[' | '\u{9b}' => state = Sequence::Csi,
                ']' | '\u{9d}' => state = Sequence::Osc,
                'P' | 'X' | '^' | '_' | '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => {
                    state = Sequence::String
                }
                '\u{20}'..='\u{2f}' => state = Sequence::Intermediates,
                _ => return,
            },
            Sequence::Intermediates | Sequence::Csi => match abort_state(character) {
                Some(next) => {
                    state = next;
                    escaped = false;
                }
                None => match state {
                    Sequence::Intermediates if ('\u{20}'..='\u{2f}').contains(&character) => {}
                    Sequence::Intermediates => return,
                    _ if ('\u{40}'..='\u{7e}').contains(&character) => return,
                    _ => {}
                },
            },
            Sequence::Osc | Sequence::String => {
                if (escaped && character == '\\')
                    || character == '\u{9c}'
                    || (matches!(state, Sequence::Osc) && character == '\u{7}')
                {
                    return;
                }
                escaped = character == '\u{1b}';
            }
        }
    }
}

/// Display width in terminal cells: East Asian wide characters count as two,
/// combining marks as zero. This is the one canonical measure — the same
/// function for measuring, truncating, and laying out chrome text.
pub(crate) fn display_width(text: &str) -> usize {
    Line::raw(text).width()
}

/// Truncates `text` to fit `budget` cells and appends `marker`, which is
/// budgeted before the text. Returns `text` unchanged when it already fits and
/// an empty string when even the marker exceeds the budget. Splits only on
/// grapheme boundaries, so a wide character or combining sequence is never cut.
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
