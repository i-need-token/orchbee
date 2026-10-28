//! Translate console records into the existing terminal byte decoder. The
//! decoder remains the only owner of bindings, paste, modal input, and mouse.
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
mod records;
pub(crate) use records::Reader;

fn encode(event: Event, output: &mut Vec<u8>) {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            let modifiers = 1
                + u8::from(key.modifiers.contains(KeyModifiers::SHIFT))
                + 2 * u8::from(key.modifiers.contains(KeyModifiers::ALT))
                + 4 * u8::from(key.modifiers.contains(KeyModifiers::CONTROL));
            let mut bytes = match key.code {
                KeyCode::Char(character) => {
                    let mut bytes = [0; 4];
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && character.is_ascii()
                        && (' '..='~').contains(&character)
                    {
                        vec![(character as u8).to_ascii_uppercase() & 0x1f]
                    } else {
                        character.encode_utf8(&mut bytes).as_bytes().to_vec()
                    }
                }
                KeyCode::Enter => vec![b'\r'],
                KeyCode::Backspace => vec![0x7f],
                KeyCode::Tab => vec![b'\t'],
                KeyCode::Esc => vec![0x1b],
                KeyCode::BackTab => b"\x1b[Z".to_vec(),
                KeyCode::Up
                | KeyCode::Down
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Home
                | KeyCode::End => {
                    let final_byte = match key.code {
                        KeyCode::Up => 'A',
                        KeyCode::Down => 'B',
                        KeyCode::Right => 'C',
                        KeyCode::Left => 'D',
                        KeyCode::Home => 'H',
                        _ => 'F',
                    };
                    if modifiers == 1 {
                        format!("\x1b[{final_byte}").into_bytes()
                    } else {
                        format!("\x1b[1;{modifiers}{final_byte}").into_bytes()
                    }
                }
                KeyCode::Insert
                | KeyCode::Delete
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::F(_) => {
                    let number = match key.code {
                        KeyCode::Insert => 2,
                        KeyCode::Delete => 3,
                        KeyCode::PageUp => 5,
                        KeyCode::PageDown => 6,
                        KeyCode::F(index @ 1..=12) => {
                            [11, 12, 13, 14, 15, 17, 18, 19, 20, 21, 23, 24][usize::from(index - 1)]
                        }
                        _ => return,
                    };
                    if modifiers == 1 {
                        format!("\x1b[{number}~").into_bytes()
                    } else {
                        format!("\x1b[{number};{modifiers}~").into_bytes()
                    }
                }
                _ => return,
            };
            if key.modifiers.contains(KeyModifiers::ALT)
                && matches!(
                    key.code,
                    KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace | KeyCode::Tab
                )
            {
                output.push(0x1b);
            }
            output.append(&mut bytes);
        }
        Event::FocusGained => output.extend_from_slice(b"\x1b[I"),
        Event::FocusLost => output.extend_from_slice(b"\x1b[O"),
        Event::Mouse(mouse) => {
            let button = |button| match button {
                MouseButton::Left => 0,
                MouseButton::Middle => 1,
                MouseButton::Right => 2,
            };
            let (mut code, released) = match mouse.kind {
                MouseEventKind::Down(value) => (button(value), false),
                MouseEventKind::Up(value) => (button(value), true),
                MouseEventKind::Drag(value) => (32 + button(value), false),
                MouseEventKind::ScrollUp => (64, false),
                MouseEventKind::ScrollDown => (65, false),
                MouseEventKind::ScrollLeft => (66, false),
                MouseEventKind::ScrollRight => (67, false),
                // Button-free hover: the motion bit (32) with no button
                // identity (3) — the `1003` report a capturing menu needs.
                MouseEventKind::Moved => (32 + 3, false),
            };
            code += 4 * u8::from(mouse.modifiers.contains(KeyModifiers::SHIFT))
                + 8 * u8::from(mouse.modifiers.contains(KeyModifiers::ALT))
                + 16 * u8::from(mouse.modifiers.contains(KeyModifiers::CONTROL));
            output.extend_from_slice(
                format!(
                    "\x1b[<{code};{};{}{}",
                    u32::from(mouse.column) + 1,
                    u32::from(mouse.row) + 1,
                    if released { 'm' } else { 'M' }
                )
                .as_bytes(),
            );
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "windows_console_input/tests.rs"]
mod tests;
