use super::*;
use crossterm::event::{KeyEvent, MouseEvent};

#[test]
fn unicode_prefix_arrows_and_release_preserve_existing_decoder_contract() {
    let mut bytes = Vec::new();
    for key in [
        KeyEvent::new(KeyCode::Char('中'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
        KeyEvent::new_with_kind(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
    ] {
        encode(Event::Key(key), &mut bytes);
    }
    assert_eq!(bytes, "中\x02\x1b[1;5C\x1bx".as_bytes());
}

#[test]
fn mouse_coordinates_modifiers_and_release_use_sgr_not_utf8_coordinates() {
    let mut bytes = Vec::new();
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::ScrollDown,
    ] {
        encode(
            Event::Mouse(MouseEvent {
                kind,
                column: 300,
                row: 12,
                modifiers: KeyModifiers::SHIFT,
            }),
            &mut bytes,
        );
    }
    assert_eq!(bytes, b"\x1b[<4;301;13M\x1b[<4;301;13m\x1b[<69;301;13M");
}

#[test]
fn host_focus_records_preserve_order_and_are_not_encoded_as_keys() {
    let mut bytes = Vec::new();
    encode(
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
        &mut bytes,
    );
    encode(Event::FocusLost, &mut bytes);
    encode(Event::FocusGained, &mut bytes);
    encode(
        Event::Key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)),
        &mut bytes,
    );
    assert_eq!(bytes, b"a\x1b[O\x1b[Ib");
}
