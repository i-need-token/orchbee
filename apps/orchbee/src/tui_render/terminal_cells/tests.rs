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
    terminal.draw(|surface| super::render(surface, area, &frame, true))?;
    assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "A");
    assert_eq!(terminal.backend().buffer()[(2, 1)].symbol(), "界");
    assert_eq!(terminal.backend().buffer()[(4, 1)].symbol(), "e\u{301}");
    frame.rows[0].cells[0].text = Arc::from("\x1b]52;c;clipboard\x07");
    terminal.draw(|surface| super::render(surface, area, &frame, false))?;
    assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "�");
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
