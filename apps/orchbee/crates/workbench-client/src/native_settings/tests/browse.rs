//! Navigation, filtering and child return never acquire a settings write boundary.
use super::*;

#[test]
fn global_search_uses_both_locales_and_never_indexes_values() {
    let mut editor = ready();
    editor.edit(Edit::Search);
    editor.paste("终端 颜色");
    let View::Draft(filtered) = editor.view().unwrap() else {
        panic!("draft")
    };
    let rows = filtered.content_rows();
    assert!(rows.contains(&Row::Foreground));
    assert!(rows.contains(&Row::Background));
    assert_eq!(filtered.values, &basis().values);
    assert!(!filtered.dirty);
    editor.edit(Edit::Line(
        crate::terminal_text::line_editor::LineKey::KillToStart,
    ));
    editor.paste("#141414");
    let View::Draft(empty) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert!(
        empty.content_rows().is_empty(),
        "literal values are not search metadata"
    );
    assert!(!empty.dirty);
}

#[test]
fn search_accept_reveals_a_result_and_escape_unwinds_only_the_current_layer() {
    let mut editor = ready();
    editor.edit(Edit::Adjust(true));
    let values = match editor.view().unwrap() {
        View::Draft(view) => view.values.clone(),
        _ => panic!("draft"),
    };
    editor.edit(Edit::Search);
    editor.paste("sound");
    editor.edit(Edit::Activate);
    let View::Draft(result) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(result.selected, Row::Sound);
    assert!(!result.searching);
    assert!(result.follow_selection);
    assert_eq!(result.values, &values);
    editor.edit(Edit::Cancel);
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if view.query.is_empty() && view.values == &values)
    );
    editor.edit(Edit::Cancel);
    assert!(!editor.visible());
}

#[test]
fn picker_preview_cancel_and_accept_keep_the_parent_draft_and_frozen_source() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Theme));
    draw(&mut editor);
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Next);
    let View::Draft(candidate) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert!(candidate.picker.is_some());
    assert!(
        !candidate.dirty,
        "a provisional choice is not a parent edit"
    );
    assert_ne!(
        editor.appearance_preview().unwrap().application_theme,
        candidate.values.application_theme
    );
    editor.edit(Edit::Cancel);
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if view.picker.is_none() && !view.dirty)
    );
    assert_eq!(
        editor.appearance_preview().unwrap().application_theme,
        basis().values.application_theme
    );
    draw(&mut editor);
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Next);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if view.picker.is_none() && view.dirty && view.basis_revision == 1)
    );
    let submission = submit(&mut editor);
    assert_eq!(
        edited_keys(&submission),
        vec![PreferenceKey::ApplicationTheme]
    );
}

#[test]
fn filtered_profile_return_restores_search_focus_and_scroll_without_discarding_edits() {
    let mut editor = ready();
    editor.edit(Edit::Search);
    editor.paste("launch");
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    assert!(matches!(editor.view(), Some(View::Draft(view)) if view.page == Page::Profiles));
    editor.edit(Edit::Select(Row::AddProfile));
    draw(&mut editor);
    editor.edit(Edit::Activate);
    editor.edit(Edit::Cancel);
    let View::Draft(parent) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(parent.page, Page::General);
    assert_eq!(parent.selected, Row::LaunchProfiles);
    assert_eq!(parent.query, "launch");
    assert!(parent.dirty);
}

#[test]
fn every_picker_accepts_one_typed_leaf_and_refuses_a_stale_pointer_choice() {
    for row in [
        Row::Language,
        Row::Theme,
        Row::LightTheme,
        Row::DarkTheme,
        Row::ColorSource,
        Row::TerminalTheme,
        Row::Reminders,
        Row::CompletionReminders,
    ] {
        let mut editor = ready();
        editor.edit(Edit::Select(row));
        draw(&mut editor);
        editor.edit(Edit::Activate);
        draw(&mut editor);
        let old_token = editor.view().unwrap().token();
        editor.edit(Edit::Next);
        let View::Draft(candidate) = editor.view().unwrap() else {
            panic!("picker")
        };
        let choice = candidate.picker.unwrap().candidate();
        assert_eq!(candidate.values, &basis().values);
        assert!(editor.control(old_token, Control::Choose(choice)).is_none());
        assert!(
            matches!(editor.view(), Some(View::Draft(view)) if !view.dirty && view.picker.is_some())
        );
        draw(&mut editor);
        let token = editor.view().unwrap().token();
        editor.control(token, Control::Choose(choice));
        let submission = submit(&mut editor);
        assert_eq!(
            edited_keys(&submission),
            vec![setting_for_row(row).unwrap().key]
        );
    }
}

#[test]
fn search_has_an_atomic_byte_bound_and_an_empty_result_cannot_submit_a_dirty_draft() {
    let mut editor = ready();
    editor.edit(Edit::Adjust(true));
    editor.edit(Edit::Search);
    assert_eq!(editor.remaining_text_bytes(), 256);
    editor.paste(&"界".repeat(86));
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if view.invalid && view.query.is_empty())
    );
    editor.edit(Edit::Line(
        crate::terminal_text::line_editor::LineKey::KillToStart,
    ));
    editor.paste("no-such-setting");
    draw(&mut editor);
    assert!(editor.edit(Edit::Activate).is_none());
    assert!(editor.edit(Edit::Save).is_none());
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if view.searching && view.content_rows().is_empty() && view.dirty)
    );
}

#[test]
fn picker_external_update_keeps_the_frozen_basis_and_cancel_reveals_newest_confirmation() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Theme));
    draw(&mut editor);
    editor.edit(Edit::Activate);
    let mut external = basis();
    external.revision = 2;
    external.values.application_theme = ApplicationAppearance::Dark;
    assert_eq!(editor.observe(&external), Observation::Conflict);
    let View::Draft(retained) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(retained.basis_revision, 1);
    assert!(retained.picker.is_some() && retained.conflict);
    editor.edit(Edit::Cancel);
    assert_eq!(
        editor.appearance_preview().unwrap().application_theme,
        ApplicationAppearance::Dark
    );
    editor.edit(Edit::Cancel);
    assert!(editor.appearance_preview().is_none());
}
