use super::{LengthLimit, LineEditor};
use unicode_segmentation::UnicodeSegmentation;

const LABEL: usize = 512;
const PATH: usize = 4096;

fn labels(text: &str) -> LineEditor {
    LineEditor::new(text.to_owned(), LengthLimit::Scalars(LABEL))
}

fn boundaries(text: &str) -> Vec<usize> {
    let mut result: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(offset, _)| offset)
        .collect();
    result.push(text.len());
    result
}

fn assert_invariants(editor: &LineEditor) {
    assert!(
        boundaries(editor.text()).contains(&editor.cursor()),
        "cursor {} not a grapheme boundary of {:?}",
        editor.cursor(),
        editor.text()
    );
    assert!(editor.scroll() <= editor.cursor());
    assert!(
        boundaries(editor.text()).contains(&editor.scroll()),
        "scroll {} not a grapheme boundary of {:?}",
        editor.scroll(),
        editor.text()
    );
}

#[test]
fn prefill_preserves_exact_text_with_cursor_at_end() {
    // A protocol-legal label may carry scalars the chrome projection cannot
    // show; prefill keeps the bytes so cancel restores the real value.
    let text = "a\u{7}\u{1b}[31mb";
    let editor = labels(text);
    assert_eq!(editor.text(), text);
    assert_eq!(editor.cursor(), text.len());
    assert_eq!(editor.scroll(), 0);
    assert_invariants(&editor);
}

#[test]
fn empty_editor_boundary_operations_are_noops() {
    let mut editor = labels("");
    assert!(!editor.left());
    assert!(!editor.right());
    assert!(!editor.home());
    assert!(!editor.end());
    assert!(!editor.backspace());
    assert!(!editor.delete());
    assert!(!editor.word_back());
    assert_invariants(&editor);
}

#[test]
fn insert_appends_and_moves_cursor() {
    let mut editor = labels("");
    editor.insert("ab").unwrap();
    editor.insert("中").unwrap();
    assert_eq!(editor.text(), "ab中");
    assert_eq!(editor.cursor(), editor.text().len());
    assert_invariants(&editor);
}

#[test]
fn family_emoji_is_five_scalars_one_cluster() {
    // 👨‍👩‍👧 = MAN + ZWJ + WOMAN + ZWJ + GIRL: five scalars, one grapheme.
    let family = "👨‍👩‍👧";
    assert_eq!(family.chars().count(), 5);
    assert_eq!(boundaries(family), vec![0, family.len()]);

    let mut editor = LineEditor::new(String::new(), LengthLimit::Scalars(5));
    editor.insert(family).unwrap();
    assert_eq!(editor.text(), family);
    assert!(!editor.right());
    assert!(editor.left());
    assert_eq!(editor.cursor(), 0);
    assert!(editor.right());
    assert_eq!(editor.cursor(), family.len());
    assert!(editor.backspace());
    assert_eq!(editor.text(), "");
    assert_invariants(&editor);
}

#[test]
fn scalar_limit_boundary_is_exact() {
    let five = "é中á😀"; // 5 scalars (a + combining mark is two)
    assert_eq!(five.chars().count(), 5);
    let mut editor = LineEditor::new(five.to_owned(), LengthLimit::Scalars(5));
    let before = (editor.text().to_owned(), editor.cursor(), editor.scroll());
    assert!(editor.insert("y").is_err());
    assert_eq!(
        (editor.text().to_owned(), editor.cursor(), editor.scroll()),
        before
    );
    editor.backspace();
    editor.insert("y").unwrap();
    assert_eq!(editor.text().chars().count(), 5);
    assert_invariants(&editor);
}

#[test]
fn byte_limit_boundary_is_exact() {
    // 中 is three UTF-8 bytes.
    let mut editor = LineEditor::new("中".to_owned(), LengthLimit::Bytes(3));
    assert!(editor.insert("x").is_err());
    assert_eq!(editor.text(), "中");

    let mut editor = LineEditor::new(String::new(), LengthLimit::Bytes(PATH));
    editor.insert("中").unwrap();
    let nearly_full = "a".repeat(PATH - editor.text().len());
    editor.insert(&nearly_full).unwrap();
    assert_eq!(editor.text().len(), PATH);
    let state = (editor.cursor(), editor.scroll());
    assert!(editor.insert("b").is_err());
    assert_eq!((editor.cursor(), editor.scroll()), state);
    assert_invariants(&editor);
}

#[test]
fn zwj_insert_merges_neighbors_cursor_snaps_to_cluster_end() {
    // 👩|👧 + ZWJ → 👩‍👧: the intent lands inside the merged cluster and the
    // cursor snaps forward to its end.
    let mut editor = labels("👩👧");
    assert!(editor.left());
    assert_eq!(editor.cursor(), "👩".len());
    editor.insert("\u{200d}").unwrap();
    assert_eq!(editor.text(), "👩\u{200d}👧");
    assert_eq!(boundaries(editor.text()), vec![0, editor.text().len()]);
    assert_eq!(editor.cursor(), editor.text().len());
    assert_invariants(&editor);
}

#[test]
fn regional_indicator_merge_on_delete() {
    // 🇦x🇧: deleting x leaves the two regional indicators to merge into one
    // flag cluster; the span-start intent snaps forward to the cluster end.
    let mut editor = labels("🇦x🇧");
    while editor.cursor() > "🇦".len() {
        editor.left();
    }
    assert_eq!(editor.cursor(), "🇦".len());
    assert!(editor.delete());
    assert_eq!(editor.text(), "🇦🇧");
    assert_eq!(boundaries(editor.text()), vec![0, editor.text().len()]);
    assert_eq!(editor.cursor(), editor.text().len());
    assert_invariants(&editor);
}

#[test]
fn regional_indicator_merge_on_backspace() {
    let mut editor = labels("🇦x🇧");
    assert!(editor.left()); // 🇦x|🇧
    assert_eq!(editor.cursor(), "🇦x".len());
    assert!(editor.backspace());
    assert_eq!(editor.text(), "🇦🇧");
    assert_eq!(editor.cursor(), editor.text().len());
    assert_invariants(&editor);
}

#[test]
fn scroll_inside_merged_cluster_reclamps_to_boundary() {
    // Scroll at 🇧's start; backspacing x merges 🇦🇧 around that position.
    let mut editor = labels("🇦x🇧");
    editor.pin_scroll(1);
    assert!(editor.left()); // cursor 🇦x|🇧, scroll clamped to 🇧's start
    assert_eq!(editor.scroll(), "🇦x".len());
    assert!(editor.backspace());
    assert_eq!(editor.text(), "🇦🇧");
    assert_eq!(editor.scroll(), 0);
    assert_invariants(&editor);
}

#[test]
fn backspace_removes_combining_cluster_whole() {
    let mut editor = labels("ez\u{301}");
    assert!(editor.backspace());
    assert_eq!(editor.text(), "e");
    assert_eq!(editor.cursor(), 1);
    assert_invariants(&editor);
}

#[test]
fn combining_insert_stays_with_base() {
    // a| + ◌́ → á|: the mark joins the base; the end-of-insert intent is
    // already a boundary at the cluster end.
    let mut editor = labels("a");
    editor.insert("\u{301}").unwrap();
    assert_eq!(editor.text(), "a\u{301}");
    assert_eq!(boundaries(editor.text()), vec![0, 3]);
    assert_eq!(editor.cursor(), 3);
    assert!(editor.left());
    assert_eq!(editor.cursor(), 0);
    assert_invariants(&editor);
}

#[test]
fn combining_insert_does_not_nfc_normalize() {
    // a + U+0301 stays two scalars of raw UTF-8; never rewritten to U+00E1.
    let mut editor = labels("a");
    editor.insert("\u{301}").unwrap();
    assert_eq!(editor.text(), "a\u{301}");
    assert_ne!(editor.text(), "\u{e1}");
    assert_eq!(editor.text().chars().count(), 2);
}

#[test]
fn word_back_deletes_to_last_word_boundary() {
    for (text, expected) in [
        ("foo   ", ""),
        ("foo bar", "foo "),
        // UAX #29 keeps a.b together ('.' is MidNumLet between letters), so
        // the whole token is one word — unlike an ad hoc ASCII scan.
        ("foo.bar", ""),
        ("a !!", ""),
        ("中 文", "中 "),
        ("a 😀b", "a 😀"),
        ("", ""),
    ] {
        let mut editor = labels(text);
        let _ = editor.word_back();
        assert_eq!(editor.text(), expected, "Ctrl+W on {text:?}");
        assert_eq!(editor.cursor(), expected.len());
        assert_invariants(&editor);
    }
}

#[test]
fn word_back_mid_draft_keeps_suffix() {
    let mut editor = labels("one twothree");
    while editor.cursor() > "one two".len() {
        editor.left();
    }
    assert!(editor.word_back());
    assert_eq!(editor.text(), "one three");
    assert_eq!(editor.cursor(), "one ".len());
    assert_invariants(&editor);
}

#[test]
fn clear_resets_all_state() {
    let mut editor = labels("draft text");
    editor.pin_scroll(4);
    editor.clear();
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor(), 0);
    assert_eq!(editor.scroll(), 0);
    assert_invariants(&editor);
}

#[test]
fn home_and_end_traverse_boundaries() {
    let mut editor = labels("a中\u{301}😀");
    assert!(editor.home());
    assert_eq!(editor.cursor(), 0);
    assert!(!editor.home());
    assert!(editor.end());
    assert_eq!(editor.cursor(), editor.text().len());
    assert!(!editor.end());
    assert_invariants(&editor);
}

#[test]
fn left_right_step_whole_clusters() {
    let mut editor = labels("a👨‍👩‍👧b");
    assert!(editor.left());
    assert!(editor.left());
    assert_eq!(editor.cursor(), "a".len());
    assert!(editor.left());
    assert_eq!(editor.cursor(), 0);
    assert!(editor.right());
    assert!(editor.right());
    assert_eq!(editor.cursor(), "a👨‍👩‍👧".len());
    assert_invariants(&editor);
}

#[test]
fn insert_accepts_what_typed_input_delivered() {
    // The core is not a third parser: admission policy stays with the
    // typed-input boundary, so a delivered scalar inserts verbatim.
    let mut editor = labels("");
    editor.insert("\u{7}").unwrap();
    assert_eq!(editor.text(), "\u{7}");
    assert_invariants(&editor);
}

#[test]
fn cursor_column_measures_window_prefix() {
    // Cursor column = width(text[scroll..cursor]), not the whole prefix.
    let mut editor = labels("abcdef");
    editor.pin_scroll(4);
    assert_eq!(editor.scroll(), 3);
    assert_eq!(editor.cursor_column(), 3);
    assert_invariants(&editor);
}

#[test]
fn scroll_pins_whole_clusters() {
    // 中a in a 3-cell field: the wide cluster scrolls off whole, never splits.
    let mut editor = labels("中a");
    editor.pin_scroll(3);
    assert_eq!(editor.scroll(), "中".len());
    let viewport = editor.viewport(3).unwrap();
    assert_eq!(viewport.text, "a");
    assert_eq!(viewport.cursor_column, 1);
    assert_invariants(&editor);
}

#[test]
fn zero_width_field_is_not_renderable() {
    let mut editor = labels("ab");
    assert!(editor.viewport(0).is_none());
    editor.pin_scroll(0); // no fake cursor position; state untouched
    assert_eq!(editor.scroll(), 0);
    assert_invariants(&editor);
}

#[test]
fn over_wide_cluster_is_never_split() {
    // The last cluster does not fit even alone in a 2-cell field; the window
    // is empty and the cursor sits at column 0 — nothing is split.
    let mut editor = labels("中中");
    editor.pin_scroll(2);
    assert_eq!(editor.scroll(), editor.text().len());
    let viewport = editor.viewport(2).unwrap();
    assert_eq!(viewport.text, "");
    assert_eq!(viewport.cursor_column, 0);
    assert_invariants(&editor);
}

#[test]
fn viewport_stops_before_exceeding_field() {
    let mut editor = labels("ab中cd");
    editor.home();
    let viewport = editor.viewport(4).unwrap();
    assert_eq!(viewport.text, "ab中"); // a(1)+b(1)+中(2) fills the field
    assert_eq!(viewport.cursor_column, 0);
    assert_invariants(&editor);
}

#[test]
fn cursor_at_last_visible_cell() {
    let mut editor = labels("wxyz");
    editor.pin_scroll(4);
    let viewport = editor.viewport(4).unwrap();
    assert_eq!(viewport.text, "xyz");
    assert_eq!(viewport.cursor_column, 3); // last cell of the field
    assert_invariants(&editor);
}

#[test]
fn scroll_reclamps_after_clear() {
    let mut editor = labels("abcdef");
    editor.pin_scroll(3);
    assert!(editor.scroll() > 0);
    editor.clear();
    assert_eq!(editor.scroll(), 0);
    assert_invariants(&editor);
}

#[test]
fn combining_mark_cursor_column_counts_zero_width() {
    // á = one cell; the cursor column after it is 1, not 2.
    let mut editor = labels("a\u{301}");
    editor.pin_scroll(8);
    assert_eq!(editor.cursor_column(), 1);
    assert_invariants(&editor);
}

#[test]
fn family_emoji_width_comes_from_same_measure() {
    // No hardcoded cell claim: the cursor column equals the project's own
    // measured width of the cluster.
    let family = "👨‍👩‍👧";
    let mut editor = labels(family);
    editor.pin_scroll(64);
    let measured = ratatui::text::Line::raw(family).width();
    assert_eq!(editor.cursor_column(), measured);
    assert_invariants(&editor);
}

#[test]
fn repeated_mixed_operations_keep_invariants() {
    let mut editor = labels("");
    let operations: &[fn(&mut LineEditor)] = &[
        |e| {
            e.insert("hi中").unwrap();
        },
        |e| {
            e.left();
        },
        |e| {
            e.insert("👩‍👧").unwrap();
        },
        |e| {
            e.backspace();
        },
        |e| {
            e.word_back();
        },
        |e| {
            e.insert("\u{200d}x").unwrap();
        },
        |e| {
            e.home();
        },
        |e| {
            e.delete();
        },
        |e| {
            e.end();
        },
        |e| {
            e.pin_scroll(3);
        },
        |e| {
            e.clear();
        },
    ];
    for operation in operations {
        operation(&mut editor);
        assert_invariants(&editor);
    }
    assert_eq!(editor.text(), "");
}

#[test]
fn long_ascii_draft_scrolls_and_stays_editable() {
    let text = "x".repeat(LABEL);
    let mut editor = labels(&text);
    editor.pin_scroll(10);
    assert_eq!(editor.cursor_column(), 9); // last cell of the field
    let viewport = editor.viewport(10).unwrap();
    assert_eq!(viewport.text.len(), 9); // only nine cells trail the scroll mark
    editor.home();
    editor.pin_scroll(10);
    assert_eq!(editor.scroll(), 0);
    assert_invariants(&editor);
}
