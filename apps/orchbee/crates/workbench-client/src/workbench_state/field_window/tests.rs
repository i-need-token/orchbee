use super::super::line_editor::{LengthLimit, LineEditor};
use super::*;

fn editor(text: &str) -> LineEditor {
    LineEditor::new(text.to_owned(), LengthLimit::Scalars(512))
}

#[test]
fn zero_width_field_is_not_renderable() {
    assert!(field_window(&editor("ab"), 0).is_none());
}

#[test]
fn short_draft_shows_whole_text_with_cursor_at_end() {
    let editor = editor("ab中");
    let window = field_window(&editor, 10).unwrap();
    assert_eq!(window.display(), "ab中");
    // a(1) + b(1) + 中(2) = 4 cells.
    assert_eq!(window.cursor_column(), 4);
}

#[test]
fn overflowing_draft_windows_to_the_cursor_and_packs_whole_clusters() {
    let mut editor = editor("ab中cd");
    editor.home();
    let window = field_window(&editor, 4).unwrap();
    assert_eq!(window.display(), "ab中"); // a(1)+b(1)+中(2) fills the field
    assert_eq!(window.cursor_column(), 0);

    editor.end();
    let window = field_window(&editor, 4).unwrap();
    // 中cd would leave the cursor on cell 4 — outside the 4-cell field — so
    // the window advances one more boundary and keeps the cursor inside.
    assert_eq!(window.display(), "cd");
    assert_eq!(window.cursor_column(), 2);
}

#[test]
fn window_stops_before_exceeding_the_field_and_never_splits_a_cluster() {
    let mut editor = editor("a中b");
    editor.home();
    let window = field_window(&editor, 3).unwrap();
    // 中 alone fills the remaining two cells; adding b would overflow.
    assert_eq!(window.display(), "a中");
    editor.end();
    let window = field_window(&editor, 2).unwrap();
    assert_eq!(window.display(), "b");
    assert_eq!(window.cursor_column(), 1);
}

#[test]
fn cluster_wider_than_the_field_shows_an_empty_window_not_a_fake_cursor() {
    let editor = editor("中中");
    let window = field_window(&editor, 1).unwrap();
    assert_eq!(window.display(), "");
    assert_eq!(window.cursor_column(), 0);
}

#[test]
fn cursor_column_counts_combining_marks_as_zero_width() {
    let editor = editor("a\u{301}");
    let window = field_window(&editor, 8).unwrap();
    assert_eq!(window.display(), "a\u{301}");
    assert_eq!(window.cursor_column(), 1);
}

#[test]
fn escape_sequence_strips_whole_and_the_raw_cursor_maps_into_display() {
    // The deterministic vector: a raw CSI inside the draft, a narrow field,
    // and a cursor that moves across and inside the stripped sequence.
    // Raw "x\u{1b}[31myz" projects to "xyz" — the sequence leaves no residue.
    let mut editor = editor("x\u{1b}[31myz");
    assert_eq!(editor.text(), "x\u{1b}[31myz", "raw text stays exact");

    // Cursor before y — inside the stripped sequence's tail: it lands where
    // the sequence's removal landed, one cell in.
    editor.home();
    for _ in 0.."x\u{1b}[31m".len() {
        editor.right();
    }
    assert_eq!(editor.cursor(), "x\u{1b}[31m".len());
    let window = field_window(&editor, 2).unwrap();
    assert_eq!(window.display(), "xy");
    assert_eq!(window.cursor_column(), 1);

    // Cursor inside the sequence maps to the same removal position — never a
    // fabricated cell mid-sequence.
    editor.home();
    editor.right();
    editor.right();
    editor.right();
    assert_eq!(editor.cursor(), 3, "raw boundary inside the CSI bytes");
    let window = field_window(&editor, 2).unwrap();
    assert_eq!(window.display(), "xy");
    assert_eq!(window.cursor_column(), 1);

    // Cursor at the end overflows the 2-cell field: the window scrolls so the
    // cursor stays inside, on the last cell.
    editor.end();
    let window = field_window(&editor, 2).unwrap();
    assert_eq!(window.display(), "z");
    assert_eq!(window.cursor_column(), 1);
}

#[test]
fn cursor_mid_draft_maps_to_its_projected_position() {
    let mut editor = editor("wxyz");
    editor.home();
    editor.right();
    editor.right();
    let window = field_window(&editor, 4).unwrap();
    assert_eq!(window.display(), "wxyz");
    assert_eq!(window.cursor_column(), 2);
}

#[test]
fn raw_boundary_inside_a_stripped_sequence_snaps_to_a_display_boundary() {
    // "a" + a dropped BEL + a combining mark: the mark merges with a in
    // display, so the raw boundary before it lands mid-cluster and snaps to
    // the cluster start.
    let mut editor = editor("a\u{7}\u{301}");
    editor.home();
    editor.right();
    editor.right();
    assert_eq!(editor.cursor(), 2, "the raw boundary before the mark");
    let window = field_window(&editor, 4).unwrap();
    assert_eq!(window.display(), "a\u{301}");
    assert_eq!(window.cursor_column(), 0);
}

#[test]
fn long_draft_stays_windowed_and_editable() {
    let text = "x".repeat(512);
    let mut editor = editor(&text);
    let window = field_window(&editor, 10).unwrap();
    // Nine trailing cells keep the cursor on the field's last cell.
    assert_eq!(window.display().len(), 9);
    assert_eq!(window.cursor_column(), 9);
    editor.home();
    let window = field_window(&editor, 10).unwrap();
    assert_eq!(window.display(), "x".repeat(10));
    assert_eq!(window.cursor_column(), 0);
}
