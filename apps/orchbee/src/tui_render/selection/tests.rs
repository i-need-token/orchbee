use super::*;
use orchbee_terminal_protocol::SelectionPoint;
use ratatui::{Terminal, backend::TestBackend, style::Color};

#[test]
fn reverse_drag_uses_absolute_rows_and_highlights_a_wide_glyph_as_one_unit() {
    let mut terminal = Terminal::new(TestBackend::new(8, 4)).unwrap();
    let selected = SelectionChangedState::Active {
        start: SelectionPoint { row: 41, column: 3 },
        end: SelectionPoint { row: 40, column: 2 },
        rectangle: false,
        preview: None,
    };
    terminal
        .draw(|frame| {
            frame.buffer_mut().set_string(1, 1, "中ab", Style::new());
            paint(
                frame,
                Rect::new(1, 1, 5, 2),
                40,
                Some(&selected),
                Style::new().bg(Color::Blue),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    // Absolute row 40 starts at pane column 2; the preceding wide glyph
    // does not enter the selection unless its trailing cell is selected.
    assert_ne!(buffer[(1, 1)].bg, Color::Blue);
    assert_eq!(buffer[(3, 1)].bg, Color::Blue);
    assert_eq!(buffer[(4, 2)].bg, Color::Blue);
    assert_ne!(buffer[(5, 2)].bg, Color::Blue);
    assert_ne!(buffer[(0, 1)].bg, Color::Blue);
    let trailing = SelectionChangedState::Active {
        start: SelectionPoint { row: 40, column: 1 },
        end: SelectionPoint { row: 40, column: 1 },
        rectangle: false,
        preview: None,
    };
    terminal
        .draw(|frame| {
            frame.buffer_mut().set_string(1, 1, "中ab", Style::new());
            paint(
                frame,
                Rect::new(1, 1, 5, 2),
                40,
                Some(&trailing),
                Style::new().bg(Color::Blue),
            );
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(1, 1)].bg, Color::Blue);
}
