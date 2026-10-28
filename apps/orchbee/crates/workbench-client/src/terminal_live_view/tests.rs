use super::*;

fn frame() -> TerminalTextFrame {
    let mut frame = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    frame.geometry.columns = 104;
    frame.geometry.rows = 36;
    frame.cursor.column = 90;
    frame.cursor.row = 35;
    frame.cursor.visible = true;
    frame
}

#[test]
fn smaller_observer_follows_the_cursor_without_changing_physical_geometry() {
    let source = frame();
    let view = LiveView::new(&source, 79, 22);
    assert_eq!(
        (view.column, view.row, view.columns, view.rows),
        (12, 16, 79, 20)
    );
    assert!(view.clipped);
    assert_eq!(
        view.content(Rect::new(2, 3, 79, 22)),
        Rect::new(2, 4, 79, 20)
    );
    assert_eq!((source.geometry.columns, source.geometry.rows), (104, 36));
}

#[test]
fn clear_resize_hidden_cursor_and_narrow_boundaries_keep_a_valid_live_window() {
    let mut source = frame();
    for (columns, rows) in [
        (80, 24),
        (120, 40),
        (60, 12),
        (40, 10),
        (19, 4),
        (1, 1),
        (0, 0),
    ] {
        for visible in [false, true] {
            source.cursor.visible = visible;
            for (column, row) in [(0, 0), (78, 20), (79, 21), (103, 35)] {
                source.cursor.column = column;
                source.cursor.row = row;
                let view = LiveView::new(&source, columns, rows);
                assert!(view.column + view.columns <= 104);
                assert!(view.row + view.rows <= 36);
                if visible && columns > 0 && rows > 0 {
                    assert!((view.column..view.column + view.columns).contains(&column));
                    assert!((view.row..view.row + view.rows).contains(&row));
                } else if !visible {
                    assert_eq!(view.row + view.rows, 36);
                }
            }
        }
    }
    source.cursor.visible = true;
    source.cursor.column = 2;
    source.cursor.row = 1;
    let after_clear = LiveView::new(&source, 40, 10);
    assert_eq!((after_clear.column, after_clear.row), (0, 0));
    let expanded = LiveView::new(&source, 120, 40);
    assert!(!expanded.clipped);
    assert_eq!((expanded.column, expanded.row), (0, 0));
}
