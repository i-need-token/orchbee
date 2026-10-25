//! Normalize one semantic key without accessing the runtime or terminal modes.
use orchbee_client_server_protocol::{SendKey, SendKeyStroke, SendStrokeKey};

pub(super) fn parse(value: &str) -> Option<SendKeyStroke> {
    if value.trim() != value {
        return None;
    }
    let mut modifiers = 0;
    let mut base = None;
    for part in value.split('+') {
        if part.is_empty() {
            return None;
        }
        let flag = match part {
            "ctrl" => 1,
            "alt" => 2,
            "shift" => 4,
            "super" => 8,
            _ => 0,
        };
        if flag != 0 {
            if base.is_some() || modifiers & flag != 0 {
                return None;
            }
            modifiers |= flag;
        } else if base.replace(part).is_some() {
            return None;
        }
    }
    let base = base?;
    let mut key = match base {
        "enter" => SendStrokeKey::Named(SendKey::Enter),
        "escape" => SendStrokeKey::Named(SendKey::Escape),
        "tab" => SendStrokeKey::Named(SendKey::Tab),
        "backspace" => SendStrokeKey::Named(SendKey::Backspace),
        "delete" => SendStrokeKey::Named(SendKey::Delete),
        "up" => SendStrokeKey::Named(SendKey::Up),
        "down" => SendStrokeKey::Named(SendKey::Down),
        "left" => SendStrokeKey::Named(SendKey::Left),
        "right" => SendStrokeKey::Named(SendKey::Right),
        "home" => SendStrokeKey::Named(SendKey::Home),
        "end" => SendStrokeKey::Named(SendKey::End),
        "page-up" => SendStrokeKey::Named(SendKey::PageUp),
        "page-down" => SendStrokeKey::Named(SendKey::PageDown),
        "insert" => SendStrokeKey::Insert,
        "space" => SendStrokeKey::Character(' '),
        "plus" => SendStrokeKey::Character('+'),
        "minus" => SendStrokeKey::Character('-'),
        "comma" => SendStrokeKey::Character(','),
        "period" => SendStrokeKey::Character('.'),
        "slash" => SendStrokeKey::Character('/'),
        "backslash" => SendStrokeKey::Character('\\'),
        "quote" => SendStrokeKey::Character('\''),
        "double-quote" => SendStrokeKey::Character('"'),
        "semicolon" => SendStrokeKey::Character(';'),
        "colon" => SendStrokeKey::Character(':'),
        "percent" => SendStrokeKey::Character('%'),
        "ampersand" => SendStrokeKey::Character('&'),
        "backtick" => SendStrokeKey::Character('`'),
        _ if base.chars().count() == 1 => SendStrokeKey::Character(base.chars().next()?),
        _ => {
            let number: u8 = base.strip_prefix('f')?.parse().ok()?;
            if base != format!("f{number}") {
                return None;
            }
            SendStrokeKey::Function(number)
        }
    };
    if let SendStrokeKey::Character(value) = &mut key
        && value.is_ascii_uppercase()
    {
        *value = value.to_ascii_lowercase();
        modifiers |= 4;
    }
    SendKeyStroke::new(key, modifiers).ok()
}

#[cfg(test)]
mod tests;
