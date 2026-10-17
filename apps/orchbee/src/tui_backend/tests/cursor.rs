//! Cursor publication, erasure, position and failure traces at the actual backend boundary.

use super::*;
use ratatui::{
    buffer::{CellDiffOption, CellWidth},
    layout::Position,
    style::Style,
};
use std::num::NonZeroU16;

fn paint_draft(frame: &mut ratatui::Frame<'_>, cursor: Option<(u16, u16)>) {
    frame.buffer_mut().set_string(
        0,
        0,
        "abc",
        Style::new()
            .fg(Color::Rgb(20, 20, 20))
            .bg(Color::Rgb(252, 252, 252)),
    );
    if let Some(cursor) = cursor {
        frame.set_cursor_position(cursor);
    }
}

#[test]
fn cursor_only_movement_erases_the_old_block_and_preserves_the_input_anchor() -> io::Result<()> {
    let mut terminal = Terminal::new(new_test_backend(Vec::new(), TEST_DIMENSIONS))?;
    terminal.draw(|frame| paint_draft(frame, Some((0, 0))))?;
    let offset = terminal.backend().writer.len();
    // Both buffers contain the same draft: Ratatui emits no content diff.
    terminal.draw(|frame| paint_draft(frame, Some((1, 0))))?;
    let output = std::str::from_utf8(&terminal.backend().writer[offset..]).unwrap();
    let restored = output.find("\x1b[48;2;252;252;252ma").unwrap();
    let painted = output.find("\x1b[48;2;0;0;0mb").unwrap();
    assert!(restored < painted, "{output:?}");
    assert!(output.ends_with("\x1b[1;2H\x1b[?2026l"));
    assert!(!output.contains("\x1b[?25h"));
    assert!(!output.contains("\x1b]12;"));
    assert_eq!(terminal.get_cursor_position()?, Position::new(1, 0));
    Ok(())
}

#[test]
fn visibility_and_owner_position_changes_erase_the_previous_cursor_before_recovery()
-> io::Result<()> {
    let mut terminal = Terminal::new(new_test_backend(Vec::new(), TEST_DIMENSIONS))?;
    terminal.draw(|frame| paint_draft(frame, Some((1, 0))))?;
    for (cursor, expected_symbol) in [
        (None, None),
        (Some((3, 1)), Some(" ")),
        (None, None),
        (Some((2, 0)), Some("c")),
    ] {
        terminal.draw(|frame| paint_draft(frame, cursor))?;
        match expected_symbol {
            None => assert!(terminal.backend().painted_cursor.is_none()),
            Some(symbol) => {
                let (position, base) = terminal.backend().painted_cursor.as_ref().unwrap();
                let (x, y) = cursor.unwrap();
                assert_eq!(*position, Position::new(x, y));
                assert_eq!(base.symbol(), symbol);
                assert_eq!(terminal.get_cursor_position()?, Position::new(x, y));
            }
        }
    }
    // A full recovery redraw invalidates both Ratatui's diff and the backend's base cells.
    terminal.clear()?;
    terminal.draw(|frame| paint_draft(frame, Some((0, 0))))?;
    assert_eq!(
        terminal
            .backend()
            .painted_cursor
            .as_ref()
            .unwrap()
            .1
            .symbol(),
        "a"
    );
    let resized = WindowDimensions {
        columns: 6,
        rows: 3,
        ..TEST_DIMENSIONS
    };
    terminal.backend_mut().set_window_dimensions(resized);
    terminal.resize(ratatui::layout::Rect::new(0, 0, 6, 3))?;
    terminal.draw(|frame| paint_draft(frame, Some((2, 0))))?;
    assert_eq!(terminal.get_cursor_position()?, Position::new(2, 0));
    assert_eq!(terminal.backend().base_cells.area.width, 6);
    assert_eq!(
        terminal.backend().painted_cursor.as_ref().unwrap().0,
        Position::new(2, 0)
    );
    Ok(())
}

#[test]
fn a_wide_tail_preserves_the_glyph_footprint_and_the_exact_native_cursor_column() -> io::Result<()>
{
    let mut terminal = Terminal::new(new_test_backend(Vec::new(), TEST_DIMENSIONS))?;
    for symbol in ["界", "\u{1f1e8}", "e\u{301}"] {
        terminal.draw(|frame| {
            frame.buffer_mut()[(0, 0)]
                .set_symbol(symbol)
                .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::new(2).unwrap()));
            frame.buffer_mut()[(0, 0)].bg = Color::Rgb(252, 252, 252);
            frame.set_cursor_position((1, 0));
        })?;
        let (position, base) = terminal.backend().painted_cursor.as_ref().unwrap();
        assert_eq!(*position, Position::ORIGIN);
        assert_eq!(base.symbol(), symbol);
        assert_eq!(base.cell_width(), 2);
        assert_eq!(terminal.get_cursor_position()?, Position::new(1, 0));
        assert!(terminal.backend().writer.ends_with(b"\x1b[1;2H\x1b[?2026l"));
    }
    Ok(())
}

#[test]
fn cursor_write_failure_is_reported_and_the_cursor_request_is_not_replayed() -> io::Result<()> {
    let mut backend = new_test_backend(ScriptedWriter::new(), TEST_DIMENSIONS);
    let mut base = Cell::new("x");
    base.bg = Color::Rgb(252, 252, 252);
    backend.draw(std::iter::once((0, 0, &base)))?;
    backend.show_cursor()?;
    backend.set_cursor_position((0, 0))?;
    backend.writer.fail_writes_from = Some(backend.writer.write_calls + 4);
    assert_eq!(
        backend.flush().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert!(backend.pending_cursor.is_none());
    backend.writer.fail_writes_from = None;
    let offset = backend.writer.buffer.len();
    backend.flush()?;
    assert_eq!(&backend.writer.buffer[offset..], SYNCHRONIZED_UPDATE_END);
    Ok(())
}

#[test]
fn concealed_program_text_stays_concealed_under_a_visible_cursor() -> io::Result<()> {
    let mut backend = new_test_backend(Vec::new(), TEST_DIMENSIONS);
    let mut base = Cell::new("s");
    base.modifier = Modifier::HIDDEN;
    base.bg = Color::Rgb(252, 252, 252);
    backend.draw(std::iter::once((0, 0, &base)))?;
    backend.show_cursor()?;
    let offset = backend.writer.len();
    backend.flush()?;
    let cursor = std::str::from_utf8(&backend.writer[offset..]).unwrap();
    assert!(cursor.contains("\x1b[48;2;0;0;0m "));
    assert!(!cursor.contains('s'));
    assert!(!cursor.contains("\x1b[8m"));
    Ok(())
}
