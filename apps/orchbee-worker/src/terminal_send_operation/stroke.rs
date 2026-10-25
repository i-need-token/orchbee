//! A validated programmatic key becomes one Host-owned semantic press.
use orchbee_capability_node_protocol::{SendKey, SendKeyStroke, SendStrokeKey};
use orchbee_terminal_protocol::{
    AttachmentId, KeyAction, KeyInput, ProtocolError, TerminalKeyCode, TerminalModifiers,
};
use std::num::NonZeroU64;

pub(super) fn input(
    stroke: SendKeyStroke,
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
) -> Result<KeyInput, ProtocolError> {
    let flags = stroke.modifiers();
    let modifiers = TerminalModifiers {
        control: flags & 1 != 0,
        alt: flags & 2 != 0,
        shift: flags & 4 != 0,
        meta: flags & 8 != 0,
        ..TerminalModifiers::default()
    };
    let (code, text, unshifted_codepoint) = match stroke.key() {
        SendStrokeKey::Named(key) => (named_code(key).to_owned(), String::new(), 0),
        SendStrokeKey::Insert => ("Insert".to_owned(), String::new(), 0),
        SendStrokeKey::Function(number) => (format!("F{number}"), String::new(), 0),
        SendStrokeKey::Character(value) => {
            let text = if modifiers.shift {
                shifted(value)
            } else {
                value
            };
            (character_code(value), text.to_string(), u32::from(value))
        }
    };
    Ok(KeyInput {
        action: KeyAction::Press,
        attachment_id,
        capability_revision,
        composing: false,
        consumed_modifiers: TerminalModifiers {
            shift: modifiers.shift && matches!(stroke.key(), SendStrokeKey::Character(_)),
            ..TerminalModifiers::default()
        },
        key: TerminalKeyCode::parse(&code)?,
        modifiers,
        option_as_alt: false,
        text,
        unshifted_codepoint,
    })
}

fn named_code(key: SendKey) -> &'static str {
    match key {
        SendKey::Enter => "Enter",
        SendKey::Escape => "Escape",
        SendKey::Tab => "Tab",
        SendKey::Backspace => "Backspace",
        SendKey::Delete => "Delete",
        SendKey::Up => "ArrowUp",
        SendKey::Down => "ArrowDown",
        SendKey::Left => "ArrowLeft",
        SendKey::Right => "ArrowRight",
        SendKey::Home => "Home",
        SendKey::End => "End",
        SendKey::PageUp => "PageUp",
        SendKey::PageDown => "PageDown",
    }
}

fn character_code(value: char) -> String {
    if value.is_ascii_alphabetic() {
        return format!("Key{}", value.to_ascii_uppercase());
    }
    if value.is_ascii_digit() {
        return format!("Digit{value}");
    }
    match value {
        ' ' => "Space",
        '`' | '~' => "Backquote",
        '-' | '_' => "Minus",
        '=' | '+' => "Equal",
        '[' | '{' => "BracketLeft",
        ']' | '}' => "BracketRight",
        '\\' | '|' => "Backslash",
        ';' | ':' => "Semicolon",
        '\'' | '"' => "Quote",
        ',' | '<' => "Comma",
        '.' | '>' => "Period",
        '/' | '?' => "Slash",
        _ => "Unidentified",
    }
    .to_owned()
}

fn shifted(value: char) -> char {
    match value {
        'a'..='z' => value.to_ascii_uppercase(),
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '`' => '~',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        _ => value,
    }
}

#[cfg(test)]
mod tests;
