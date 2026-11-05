//! The text a configured command is known by when it has no description: its command line or plugin
//! target as the TUI's key help paints it, bounded for the wire.
//!
//! The TUI paints that text through its chrome criterion (`terminal_text::sanitize_terminal_text` in
//! `orchbee-workbench-client`), so this projection keeps the same rule: control characters, line
//! and paragraph separators, and format scalars that reorder or hide text are dropped, and an escape
//! sequence goes whole, so `ESC [31m` leaves no `[31m` behind. Nothing else is removed.
use orchbee_workbench_protocol::MAXIMUM_CONFIGURED_COMMAND_DISPLAY_BYTES;

const ELLIPSIS: char = '…';

/// The display text, or `None` when nothing visible remains. Text over the bound is cut at a
/// character boundary and ends with an ellipsis.
pub(super) fn configured_command_display(command: &str) -> Option<String> {
    let mut text = chrome_text(command);
    if text.trim().is_empty() {
        return None;
    }
    if text.len() > MAXIMUM_CONFIGURED_COMMAND_DISPLAY_BYTES {
        let budget = MAXIMUM_CONFIGURED_COMMAND_DISPLAY_BYTES - ELLIPSIS.len_utf8();
        let cut = (0..=budget)
            .rev()
            .find(|&index| text.is_char_boundary(index))
            .unwrap_or(0);
        text.truncate(cut);
        text.push(ELLIPSIS);
    }
    Some(text)
}

fn is_forbidden(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{2028}'
                | '\u{2029}'
                | '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{206f}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{e0001}'
                | '\u{e0020}'..='\u{e007f}'
        )
}

fn chrome_text(value: &str) -> String {
    let mut text = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        match character {
            '\u{1b}' => skip(&mut characters, Sequence::Dispatch),
            '\u{9b}' => skip(&mut characters, Sequence::Csi),
            '\u{9d}' => skip(&mut characters, Sequence::Osc),
            '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => skip(&mut characters, Sequence::String),
            _ if !is_forbidden(character) => text.push(character),
            _ => {}
        }
    }
    text
}

enum Sequence {
    Dispatch,
    Intermediates,
    Csi,
    Osc,
    String,
}

fn restart(character: char) -> Option<Sequence> {
    match character {
        '\u{1b}' => Some(Sequence::Dispatch),
        '\u{9b}' => Some(Sequence::Csi),
        '\u{9d}' => Some(Sequence::Osc),
        '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => Some(Sequence::String),
        _ => None,
    }
}

/// Consumes one sequence as the TUI does: a fresh initiator inside dispatch, intermediates or CSI
/// starts over, OSC ends at BEL or ST, a string ends at ST, and a truncated sequence runs to the end.
fn skip(characters: &mut impl Iterator<Item = char>, mut state: Sequence) {
    let mut escaped = false;
    for character in characters.by_ref() {
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
            Sequence::Intermediates | Sequence::Csi => match restart(character) {
                Some(next) => state = next,
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
