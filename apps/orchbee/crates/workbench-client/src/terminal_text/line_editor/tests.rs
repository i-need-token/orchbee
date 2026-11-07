use super::{LengthLimit, LineEditor, LineKey};
use crate::terminal_events::keys::{KeyCode, KeyEvent, Modifiers};
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
}

#[test]
fn prefill_preserves_exact_text_with_cursor_at_end() {
    // A protocol-legal label may carry scalars the chrome projection cannot
    // show; prefill keeps the bytes so cancel restores the real value.
    let text = "a\u{7}\u{1b}[31mb";
    let editor = labels(text);
    assert_eq!(editor.text(), text);
    assert_eq!(editor.cursor(), text.len());
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
    let before = (editor.text().to_owned(), editor.cursor());
    assert!(editor.insert("y").is_err());
    assert_eq!((editor.text().to_owned(), editor.cursor()), before);
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
    let state = editor.cursor();
    assert!(editor.insert("b").is_err());
    assert_eq!(editor.cursor(), state);
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
    editor.clear();
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor(), 0);
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
            e.clear();
        },
    ];
    for operation in operations {
        operation(&mut editor);
        assert_invariants(&editor);
    }
    assert_eq!(editor.text(), "");
}

fn apply(editor: &mut LineEditor, key: LineKey) -> bool {
    let changed = editor.apply(key).unwrap();
    assert_invariants(editor);
    changed
}

#[test]
fn consecutive_kills_to_either_end_yank_back_in_reading_order() {
    let mut editor = labels("你好 world");
    while editor.cursor() > "你好".len() {
        editor.left();
    }
    assert!(apply(&mut editor, LineKey::KillToEnd));
    assert_eq!(editor.text(), "你好");
    assert!(apply(&mut editor, LineKey::KillToStart));
    assert_eq!((editor.text(), editor.cursor()), ("", 0));
    assert!(apply(&mut editor, LineKey::Yank));
    assert_eq!(editor.text(), "你好 world");
    assert_eq!(editor.cursor(), editor.text().len());
    assert!(!apply(&mut editor, LineKey::KillToEnd));
}

#[test]
fn any_other_edit_ends_the_kill_chain() {
    let mut editor = labels("one two three");
    assert!(apply(&mut editor, LineKey::KillWordBack));
    assert!(apply(&mut editor, LineKey::KillWordBack));
    assert_eq!(editor.text(), "one ");
    editor.insert("x").unwrap();
    assert!(editor.backspace());
    assert!(apply(&mut editor, LineKey::KillWordBack));
    assert_eq!(editor.text(), "");
    assert!(apply(&mut editor, LineKey::Yank));
    assert_eq!(editor.text(), "one ");
}

#[test]
fn word_keys_follow_unicode_words_over_combining_cjk_and_emoji() {
    let mut editor = labels("a\u{301}b 中文 😀 end");
    editor.home();
    assert!(apply(&mut editor, LineKey::WordRight));
    assert_eq!(editor.cursor(), "a\u{301}b".len());
    assert!(apply(&mut editor, LineKey::WordRight));
    assert_eq!(editor.cursor(), "a\u{301}b 中".len());
    assert!(apply(&mut editor, LineKey::KillWordForward));
    assert!(apply(&mut editor, LineKey::KillWordForward));
    assert_eq!(editor.text(), "a\u{301}b 中");
    assert!(!apply(&mut editor, LineKey::KillWordForward));
    assert!(apply(&mut editor, LineKey::WordLeft));
    assert_eq!(editor.cursor(), "a\u{301}b ".len());
    assert!(apply(&mut editor, LineKey::WordLeft));
    assert_eq!(editor.cursor(), 0);
    assert!(!apply(&mut editor, LineKey::WordLeft));
    assert!(apply(&mut editor, LineKey::Yank));
    assert_eq!(editor.text(), "文 😀 enda\u{301}b 中");
}

#[test]
fn yank_over_budget_is_rejected_atomically() {
    let mut editor = LineEditor::new("ab".to_owned(), LengthLimit::Scalars(3));
    assert!(editor.apply(LineKey::KillToStart).unwrap());
    editor.insert("xyz").unwrap();
    let before = (editor.text().to_owned(), editor.cursor());
    assert!(editor.apply(LineKey::Yank).is_err());
    assert_eq!((editor.text().to_owned(), editor.cursor()), before);
    assert!(editor.apply(LineKey::KillToStart).unwrap());
    assert!(editor.apply(LineKey::Yank).unwrap());
    assert_eq!(editor.text(), "xyz");
}

#[test]
fn host_keys_map_only_with_their_exact_modifiers() {
    let press = |character, mods| LineKey::for_key(KeyEvent::press(KeyCode::Char(character), mods));
    let alt = Modifiers::NONE.with_alt();
    assert_eq!(press('k', Modifiers::CTRL), Some(LineKey::KillToEnd));
    assert_eq!(press('y', Modifiers::CTRL), Some(LineKey::Yank));
    assert_eq!(press('b', alt), Some(LineKey::WordLeft));
    assert_eq!(press('f', alt), Some(LineKey::WordRight));
    assert_eq!(press('d', alt), Some(LineKey::KillWordForward));
    assert_eq!(press('K', Modifiers::CTRL), None);
    assert_eq!(press('b', Modifiers::CTRL.with_alt()), None);
    assert_eq!(press('b', Modifiers::NONE), None);
}
