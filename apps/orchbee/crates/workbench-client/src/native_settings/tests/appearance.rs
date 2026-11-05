//! Preview authority and the complete inventory's atomic Save boundary.
use super::*;
use orchbee_workbench_protocol::TerminalDisplayAppearance;

#[test]
fn every_admitted_key_edits_only_its_draft_and_submits_one_exact_patch() {
    for key in SETTINGS.iter().map(|metadata| metadata.key) {
        let mut basis = basis();
        basis.values.terminal_display_theme = TerminalDisplayAppearance::Custom;
        let mut editor = Editor::default();
        let token = editor.open().unwrap();
        editor.loaded(token, basis.clone());
        draw(&mut editor);
        let row = setting(key).row;
        editor.edit(Edit::Select(row));
        match key {
            PreferenceKey::CustomForeground | PreferenceKey::CustomBackground => {
                editor.edit(Edit::Activate);
                editor.edit(Edit::Clear);
                editor.paste("#123abc");
                editor.edit(Edit::Activate);
            }
            PreferenceKey::LaunchProfiles => {
                editor.edit(Edit::Activate);
                editor.edit(Edit::Select(Row::AddProfile));
                editor.edit(Edit::Activate);
                editor.edit(Edit::Cancel);
            }
            _ => {
                editor.edit(Edit::Adjust(true));
            }
        }
        let Some(View::Draft(draft)) = editor.view() else {
            panic!("{key:?}: editable draft");
        };
        assert!(draft.dirty, "{key:?}");
        assert_eq!(draft.basis_values, &basis.values, "{key:?}");
        assert_eq!(draft.effective_values, &basis.values, "{key:?}");
        let previewed = matches!(
            key,
            PreferenceKey::Locale
                | PreferenceKey::ApplicationTheme
                | PreferenceKey::LightTheme
                | PreferenceKey::DarkTheme
                | PreferenceKey::TuiColorSource
                | PreferenceKey::TerminalDisplayTheme
                | PreferenceKey::CustomForeground
                | PreferenceKey::CustomBackground
        );
        assert!(
            crate::localization::Locale::EnUs
                .settings_detail(draft)
                .contains(if previewed {
                    "Local preview"
                } else {
                    "After Save"
                }),
            "{key:?}"
        );
        assert!(
            crate::localization::Locale::ZhCn
                .settings_detail(draft)
                .contains(if previewed {
                    "本地预览"
                } else {
                    "保存后"
                }),
            "{key:?}"
        );
        let edited = draft.values.clone();
        edited.validate().unwrap();
        let submission = submit(&mut editor);
        let SettingsOperation::Save { source, changes } = submission.operation else {
            panic!("{key:?}: Save");
        };
        assert_eq!(source.as_str(), basis.source, "{key:?}");
        assert_eq!(changes.keys().collect::<Vec<_>>(), [key], "{key:?}");
        assert_eq!(changes.apply(&basis.values).unwrap(), edited, "{key:?}");
        assert!(editor.edit(Edit::Activate).is_none(), "{key:?}: no replay");
        assert_eq!(
            editor.appearance_preview(),
            Some(AppearancePreview::from_values(&edited))
        );
        editor.edit(Edit::Cancel);
        assert!(editor.appearance_preview().is_none());
        assert!(editor.complete(submission.token));
        assert!(editor.appearance_preview().is_none());
    }
}

#[test]
fn preview_survives_child_return_and_pending_save_but_expires_with_its_episode() {
    let mut editor = ready_with_application_theme(ApplicationAppearance::Light);
    editor.edit(Edit::Select(Row::Theme));
    editor.edit(Edit::Adjust(true));
    let mut dark = basis_with_application_theme(ApplicationAppearance::Light).values;
    dark.application_theme = ApplicationAppearance::Dark;
    assert_eq!(
        editor.appearance_preview().unwrap().application_theme,
        ApplicationAppearance::Dark
    );
    editor.edit(Edit::Select(Row::LaunchProfiles));
    editor.edit(Edit::Activate);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&dark))
    );
    editor.edit(Edit::Cancel);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&dark))
    );
    editor.geometry_blocked();
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&dark))
    );
    draw(&mut editor);
    let submission = submit(&mut editor);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&dark))
    );
    editor.edit(Edit::Cancel);
    assert!(editor.appearance_preview().is_none());
    assert!(editor.complete(submission.token));
    let opening = editor.open().unwrap();
    assert!(editor.appearance_preview().is_none());
    assert!(!editor.complete(submission.token));
    assert!(editor.loaded(opening, basis()));
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&basis().values))
    );
    editor.retire();
    assert!(editor.appearance_preview().is_none());
}

#[test]
fn conflicts_merge_only_local_appearance_leaves_and_unknown_withdraws_preview() {
    for reconciliation in [
        Some(Reconciliation::OutcomeUnknown),
        Some(Reconciliation::SourceChanged),
        None,
    ] {
        let mut editor = ready_with_application_theme(ApplicationAppearance::Light);
        editor.edit(Edit::Select(Row::Theme));
        editor.edit(Edit::Adjust(true));
        let mut preview = basis_with_application_theme(ApplicationAppearance::Light).values;
        preview.application_theme = ApplicationAppearance::Dark;
        let submission = submit(&mut editor);
        let SettingsOperation::Save { source, changes } = &submission.operation else {
            panic!("appearance conflict saves the local patch");
        };
        assert_eq!(source.as_str(), basis().source);
        assert_eq!(
            changes.keys().collect::<Vec<_>>(),
            [PreferenceKey::ApplicationTheme],
            "a conflict retains the exact locally edited leaf"
        );
        assert_eq!(changes.apply(&basis().values).unwrap(), preview);
        assert!(editor.failed(submission.token, reconciliation));
        if reconciliation == Some(Reconciliation::OutcomeUnknown) {
            assert!(editor.appearance_preview().is_none());
        } else {
            assert_eq!(
                editor.appearance_preview(),
                Some(AppearancePreview::from_values(&preview))
            );
        }
        let mut external = basis();
        external.source = "a".repeat(64);
        external.values.locale = PreferencesLocale::ZhCn;
        assert_eq!(editor.observe(&external), Observation::Conflict);
        draw(&mut editor);
        let mut merged = preview.clone();
        merged.locale = external.values.locale;
        if reconciliation == Some(Reconciliation::OutcomeUnknown) {
            assert!(editor.appearance_preview().is_none());
        } else {
            assert_eq!(
                editor.appearance_preview(),
                Some(AppearancePreview::from_values(&merged))
            );
        }
        assert!(matches!(editor.view(), Some(View::Draft(draft))
            if draft.values == &preview && draft.effective_values == &external.values));
        editor.edit(Edit::Select(Row::Reload));
        draw(&mut editor);
        assert!(
            editor.edit(Edit::Activate).is_none(),
            "reload asks for confirmation"
        );
        draw(&mut editor);
        let reload = editor.edit(Edit::Activate).expect("explicit fresh read");
        assert!(matches!(&reload.operation, SettingsOperation::Open {}));
        assert!(editor.appearance_preview().is_none());
        assert!(editor.loaded(reload.token, external.clone()));
        assert_eq!(
            editor.appearance_preview(),
            Some(AppearancePreview::from_values(&external.values))
        );
    }
}

#[test]
fn invalid_color_entry_and_reset_confirmation_keep_reversible_valid_presentation() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::TerminalTheme));
    editor.edit(Edit::Adjust(false));
    let mut custom = basis().values;
    custom.terminal_display_theme = TerminalDisplayAppearance::Custom;
    editor.edit(Edit::Select(Row::Foreground));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Clear);
    editor.paste("#bad");
    editor.edit(Edit::Activate);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&custom))
    );
    editor.edit(Edit::Cancel);
    editor.edit(Edit::Select(Row::ResetAll));
    editor.edit(Edit::Activate);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(&custom))
    );
    draw(&mut editor);
    editor.edit(Edit::Activate);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(
            &UserPreferencesValues::default()
        ))
    );
    assert!(matches!(editor.view(), Some(View::Draft(draft))
        if draft.dirty && draft.basis_values == &basis().values));
    editor.edit(Edit::Cancel);
    assert!(editor.appearance_preview().is_none());
}

#[test]
fn passive_unrelated_updates_keep_preview_and_reset_claims_all_appearance_leaves() {
    let mut editor = ready_with_application_theme(ApplicationAppearance::Light);
    editor.edit(Edit::Select(Row::Theme));
    editor.edit(Edit::Adjust(true));
    let mut external = basis();
    external.source = "b".repeat(64);
    external.values.copy_on_select = false;
    external.values.locale = PreferencesLocale::ZhCn;
    external.values.custom_foreground = "#abcdef".into();
    external.values.custom_background = "#123456".into();
    assert_eq!(editor.observe(&external), Observation::Conflict);
    let preview = editor.appearance_preview().unwrap();
    assert_eq!(preview.application_theme, ApplicationAppearance::Dark);
    assert_eq!(preview.locale, PreferencesLocale::ZhCn);
    assert_eq!(preview.custom_foreground, "#abcdef");
    assert_eq!(preview.custom_background, "#123456");
    editor.edit(Edit::Select(Row::ResetAll));
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    assert_eq!(
        editor.appearance_preview(),
        Some(AppearancePreview::from_values(
            &UserPreferencesValues::default()
        ))
    );
    assert!(matches!(editor.view(), Some(View::Draft(draft))
            if draft.conflict
                && draft.basis_values == &basis_with_application_theme(ApplicationAppearance::Light).values
                && !draft.enabled(Row::Save)));
}
