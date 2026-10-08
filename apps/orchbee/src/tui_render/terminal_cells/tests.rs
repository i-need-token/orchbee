use super::*;
use ratatui::{Terminal, backend::TestBackend};
use std::sync::Arc;
#[test]
fn renders_unicode_width_styles_and_cursor_without_emitting_control_text()
-> Result<(), Box<dyn std::error::Error>> {
    let mut frame = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))?;
    let mut terminal = Terminal::new(TestBackend::new(10, 4))?;
    let area = Rect::new(1, 1, 6, 2);
    let theme = Theme::terminal_native();
    terminal.draw(|surface| super::render(surface, area, &frame, true, &theme))?;
    assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "A");
    assert_eq!(terminal.backend().buffer()[(2, 1)].symbol(), "界");
    assert_eq!(terminal.backend().buffer()[(4, 1)].symbol(), "e\u{301}");
    for forbidden in ["\x1b]52;c;clipboard\x07", "\u{85}", "\u{2028}", "\u{2029}"] {
        frame.rows[0].cells[0].text = Arc::from(forbidden);
        terminal.draw(|surface| super::render(surface, area, &frame, false, &theme))?;
        assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "�");
    }
    assert!(
        !terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol().contains('\x1b'))
    );
    Ok(())
}
