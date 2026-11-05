//! Theme slot and color source pickers: preview, commit, cancel, reset and availability.
use super::*;
use orchbee_workbench_protocol::TuiColorSource;

fn basis_with(edit: impl FnOnce(&mut UserPreferencesValues)) -> Basis {
    let mut basis = basis();
    edit(&mut basis.values);
    basis
}

fn open_picker(editor: &mut Editor, row: Row) {
    editor.edit(Edit::Select(row));
    draw(editor);
    editor.edit(Edit::Activate);
    draw(editor);
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if view.picker.is_some_and(|picker| picker.row() == row)),
        "{row:?} opens its picker"
    );
}

#[test]
fn a_theme_candidate_previews_at_its_own_polarity_and_escape_restores_the_committed_look() {
    let initial = basis_with(|values| {
        values.application_theme = ApplicationAppearance::Dark;
        values.tui_color_source = TuiColorSource::Terminal;
    });
    let mut editor = ready_with_basis(initial.clone());
    open_picker(&mut editor, Row::LightTheme);
    editor.edit(Edit::Next);
    let preview = editor.appearance_preview().unwrap();
    assert_eq!(preview.light_theme, "dayfox");
    assert_eq!(preview.application_theme, ApplicationAppearance::Light);
    assert_eq!(preview.tui_color_source, TuiColorSource::Theme);
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if !view.dirty && view.values == &initial.values),
        "a candidate is not a draft edit"
    );
    editor.edit(Edit::Cancel);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&initial.values))
    );
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if view.picker.is_none() && !view.dirty)
    );
}

#[test]
fn committing_a_theme_sets_its_slot_and_shows_it_through_appearance_and_source() {
    let initial = basis_with(|values| {
        values.application_theme = ApplicationAppearance::Light;
        values.tui_color_source = TuiColorSource::Terminal;
    });
    let mut editor = ready_with_basis(initial);
    open_picker(&mut editor, Row::DarkTheme);
    editor.edit(Edit::Next);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    let Some(View::Draft(draft)) = editor.view() else {
        panic!("draft")
    };
    assert_eq!(draft.values.dark_theme, "gruvbox-dark");
    assert_eq!(draft.values.application_theme, ApplicationAppearance::Dark);
    assert_eq!(draft.values.tui_color_source, TuiColorSource::Theme);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(draft.values))
    );
    let SettingsOperation::Save { changes, .. } = submit(&mut editor).operation else {
        panic!("save")
    };
    assert_eq!(
        changes.keys().collect::<Vec<_>>(),
        [
            PreferenceKey::ApplicationTheme,
            PreferenceKey::DarkTheme,
            PreferenceKey::TuiColorSource
        ]
    );
}

#[test]
fn an_unshipped_slot_opens_on_its_fallback_and_recommitting_it_keeps_the_stored_id() {
    let initial = basis_with(|values| values.light_theme = "from-a-newer-build".into());
    let mut editor = ready_with_basis(initial.clone());
    open_picker(&mut editor, Row::LightTheme);
    let Some(View::Draft(view)) = editor.view() else {
        panic!("draft")
    };
    assert!(matches!(
        view.picker.unwrap().original(),
        Choice::ColorTheme(choice) if choice.theme().id() == "orchbee-light"
    ));
    editor.edit(Edit::Activate);
    assert!(
        matches!(editor.view(), Some(View::Draft(view)) if !view.dirty && view.values == &initial.values)
    );
}

#[test]
fn reset_all_restores_both_slots_and_the_color_source() {
    let initial = basis_with(|values| {
        values.light_theme = "dayfox".into();
        values.dark_theme = "dracula".into();
        values.tui_color_source = TuiColorSource::Terminal;
    });
    let mut editor = ready_with_basis(initial);
    editor.edit(Edit::Select(Row::ResetAll));
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    let defaults = UserPreferencesValues::default();
    assert!(matches!(editor.view(), Some(View::Draft(view)) if view.values == &defaults));
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&defaults))
    );
}

#[test]
fn reduced_color_makes_theme_rows_unavailable_with_a_reason_and_closes_their_picker() {
    let mut editor = ready();
    open_picker(&mut editor, Row::DarkTheme);
    editor.edit(Edit::Next);
    assert!(editor.configure_reduced_color(true));
    assert!(!editor.configure_reduced_color(true));
    draw(&mut editor);
    let Some(View::Draft(view)) = editor.view() else {
        panic!("draft")
    };
    assert!(view.picker.is_none() && !view.dirty);
    for row in [Row::LightTheme, Row::DarkTheme, Row::ColorSource] {
        assert!(!view.enabled(row), "{row:?}");
    }
    assert!(view.enabled(Row::Theme));
    assert!(
        crate::localization::Locale::EnUs
            .settings_detail(view)
            .contains("24-bit")
    );
    assert!(
        crate::localization::Locale::ZhCn
            .settings_detail(view)
            .contains("24 位")
    );
    editor.edit(Edit::Activate);
    assert!(matches!(editor.view(), Some(View::Draft(view)) if view.picker.is_none()));
    assert!(editor.configure_reduced_color(false));
    draw(&mut editor);
    assert!(matches!(editor.view(), Some(View::Draft(view)) if view.enabled(Row::DarkTheme)));
}
