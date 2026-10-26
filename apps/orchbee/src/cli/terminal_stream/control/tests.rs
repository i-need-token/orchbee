use super::*;
use crate::cli::terminal_stream::tests::SNAPSHOT;
use orchbee_terminal_protocol::decode_terminal_text_frame;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[cfg(windows)]
mod windows;

#[test]
fn scroll_uses_crossterm_modifiers_and_routes_shell_pages_mouse_and_alternate_screen() -> TestResult
{
    let frame = |bits: u32, alternate: bool| {
        let mut bytes = SNAPSHOT.to_vec();
        bytes[100..104].copy_from_slice(&bits.to_le_bytes());
        bytes[98] = if alternate { 2 } else { 1 };
        decode_terminal_text_frame(&bytes).unwrap()
    };
    let scroll = input::Scroll {
        direction: input::Direction::Up,
        lines: 80,
        source: input::ScrollSource::Wheel,
        column: None,
        row: None,
        modifiers: 7,
    };
    assert_eq!(
        scroll_effect(&frame(0, false), scroll, false),
        ScrollEffect::History(80)
    );
    assert_eq!(
        scroll_effect(&frame(0x220, false), scroll, false),
        ScrollEffect::Input(b"\x1b[<92;1;1M".to_vec())
    );
    assert_eq!(
        scroll_effect(
            &frame(0x220, true),
            input::Scroll {
                column: Some(1),
                row: Some(1),
                modifiers: 1,
                ..scroll
            },
            false
        ),
        ScrollEffect::Input(b"\x1b[<68;2;2M".to_vec())
    );
    assert_eq!(
        scroll_effect(&frame((1 << 18) | 1, true), scroll, false),
        ScrollEffect::Input(b"\x1bOA".to_vec())
    );
    // DEC 1007 is independent of cursor mode. Disabled never emits arrows;
    // tracking and an already active history view keep their own routing.
    for cursor in [0, 1] {
        assert_eq!(
            scroll_effect(&frame(cursor, true), scroll, false),
            ScrollEffect::History(80)
        );
        assert_eq!(
            scroll_effect(&frame((1 << 18) | cursor, false), scroll, false),
            ScrollEffect::History(80)
        );
        assert_eq!(
            scroll_effect(&frame((1 << 18) | cursor, true), scroll, true),
            ScrollEffect::History(80)
        );
    }
    assert_eq!(
        scroll_effect(
            &frame(1 << 18, true),
            input::Scroll {
                direction: input::Direction::Down,
                ..scroll
            },
            false
        ),
        ScrollEffect::Input(b"\x1b[B".to_vec())
    );
    assert_eq!(
        scroll_effect(&frame((1 << 18) | 0x220, true), scroll, false),
        ScrollEffect::Input(b"\x1b[<92;1;1M".to_vec())
    );
    let page = input::Scroll {
        source: input::ScrollSource::PageKey,
        ..scroll
    };
    assert_eq!(
        scroll_effect(&frame(1, false), page, false),
        ScrollEffect::Input(b"\x1b[5~".to_vec())
    );
    assert_eq!(
        scroll_effect(&frame(5, false), page, false),
        ScrollEffect::History(80)
    );
    assert_eq!(
        scroll_effect(&frame(5, true), page, false),
        ScrollEffect::Input(b"\x1b[5~".to_vec())
    );
    assert_eq!(
        scroll_effect(&frame(0x820, false), scroll, false),
        ScrollEffect::Input(b"\x1b[<92;1;1M".to_vec())
    );
    Ok(())
}
