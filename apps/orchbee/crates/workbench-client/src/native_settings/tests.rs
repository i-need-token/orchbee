//! Independent episode traces and concrete conflict/reset/text boundaries.
use super::*;
use crate::client_view::{KeyCode, KeyEvent, Modifiers};
use orchbee_workbench_protocol::{AgentReminderMode, ApplicationAppearance, PreferencesLocale};
mod appearance;
mod browse;
mod themes;

fn basis() -> Basis {
    Basis {
        source: "missing".into(),
        values: UserPreferencesValues::default(),
        claimed: Default::default(),
    }
}
fn basis_with_application_theme(application_theme: ApplicationAppearance) -> Basis {
    Basis {
        source: "missing".into(),
        values: UserPreferencesValues {
            application_theme,
            ..UserPreferencesValues::default()
        },
        claimed: Default::default(),
    }
}
fn ready_with_basis(initial: Basis) -> Editor {
    let mut editor = Editor::default();
    let token = editor.open().unwrap();
    assert!(editor.loaded(token, initial));
    draw(&mut editor);
    editor
}
fn ready() -> Editor {
    ready_with_basis(basis())
}
fn ready_with_application_theme(application_theme: ApplicationAppearance) -> Editor {
    ready_with_basis(basis_with_application_theme(application_theme))
}
fn draw(editor: &mut Editor) {
    editor.draw(editor.view().unwrap().token());
}
fn submit(editor: &mut Editor) -> Submission {
    editor.edit(Edit::Select(Row::Save));
    draw(editor);
    editor.edit(Edit::Activate).unwrap()
}

#[test]
fn shared_preference_metadata_is_complete_stable_and_feature_owned() {
    let projected = PreferenceKey::ALL.into_iter().collect::<Vec<_>>();
    assert_eq!(
        SETTINGS
            .iter()
            .map(|metadata| metadata.key)
            .collect::<Vec<_>>(),
        projected,
        "the native projection must cover the closed protocol inventory in canonical order"
    );
    let rows = SETTINGS
        .iter()
        .map(|metadata| metadata.row)
        .collect::<Vec<_>>();
    assert_eq!(
        rows.as_slice(),
        &Page::General.rows()[..SETTINGS.len()],
        "ordinary setting rows must not acquire a second ordering"
    );
    for metadata in &SETTINGS {
        assert_eq!(setting(metadata.key), metadata);
        assert_eq!(setting_for_row(metadata.row), Some(metadata));
    }
    assert_eq!(
        setting(PreferenceKey::CustomForeground).availability,
        SettingAvailability::TerminalDisplayThemeCustom
    );
    assert_eq!(
        setting(PreferenceKey::CopyOnSelect).category,
        SettingCategory::Terminal
    );
    assert_eq!(
        setting(PreferenceKey::LaunchProfiles).effect,
        SettingEffect::NewDefaultLaunch
    );
    assert_eq!(
        setting(PreferenceKey::CompletionReminderMinimumSeconds).control,
        SettingControl::BoundedInteger {
            minimum: 1,
            maximum: 3_600,
            small_step: 1,
            large_step: 10,
            unit: SettingUnit::Seconds,
        }
    );
    assert_eq!(
        setting(PreferenceKey::ScrollbackBytes).control,
        SettingControl::BoundedInteger {
            minimum: 0,
            maximum: 1_000_000_000,
            small_step: 1_000_000,
            large_step: 10_000_000,
            unit: SettingUnit::Bytes,
        }
    );
}

#[test]
fn bounded_adjustments_use_owner_steps_without_rounding_and_disable_at_endpoints() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::CompletionThreshold));
    draw(&mut editor);
    editor.edit(Edit::AdjustLarge(false));
    let View::Draft(threshold) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(threshold.values.completion_reminder_minimum_seconds, 1);
    assert_eq!(
        threshold.adjustment(Row::CompletionThreshold),
        Some(Adjustment {
            backward: false,
            forward: true,
        })
    );
    let token = threshold.token;
    draw(&mut editor);
    editor.edit(Edit::Adjust(false));
    assert!(matches!(
        editor.view(),
        Some(View::Draft(draft))
            if draft.token == token
                && draft.values.completion_reminder_minimum_seconds == 1
    ));
    editor.edit(Edit::AdjustLarge(true));
    assert!(matches!(
        editor.view(),
        Some(View::Draft(draft)) if draft.values.completion_reminder_minimum_seconds == 11
    ));

    editor.edit(Edit::Select(Row::Scrollback));
    draw(&mut editor);
    editor.edit(Edit::Activate);
    editor.edit(Edit::Clear);
    editor.paste("10.000001");
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Adjust(false));
    assert!(matches!(
        editor.view(),
        Some(View::Draft(draft)) if draft.values.scrollback_bytes == 9_000_001
    ));
}

#[test]
fn primary_key_accepts_legacy_cr_but_rejects_explicit_enter_modifiers_and_release() {
    for text_entry in [false, true] {
        for (code, mods) in [
            (KeyCode::Enter, Modifiers::NONE),
            (KeyCode::Char('m'), Modifiers::CTRL),
        ] {
            for kind in [
                crate::client_view::KeyKind::Press,
                crate::client_view::KeyKind::Repeat,
            ] {
                let mut key = KeyEvent::press(code, mods);
                key.kind = kind;
                assert!(matches!(key_edit(key, text_entry), Some(Edit::Activate)));
            }
            let mut key = KeyEvent::press(code, mods);
            key.kind = crate::client_view::KeyKind::Release;
            assert!(key_edit(key, text_entry).is_none());
        }
        for mods in [
            Modifiers::CTRL,
            Modifiers::NONE.with_shift(),
            Modifiers::NONE.with_alt(),
            Modifiers::NONE.with_super_(),
        ] {
            assert!(key_edit(KeyEvent::press(KeyCode::Enter, mods), text_entry).is_none());
        }
        assert!(
            key_edit(
                KeyEvent::press(KeyCode::Char('m'), Modifiers::CTRL.with_shift()),
                text_entry,
            )
            .is_none()
        );
        assert!(!matches!(
            key_edit(
                KeyEvent::press(KeyCode::Char('y'), Modifiers::NONE),
                text_entry
            ),
            Some(Edit::Activate)
        ));
    }
}

#[test]
fn confirmation_key_map_keeps_list_space_and_text_keys_out_of_the_decision() {
    let mut editor = ready();
    let list = editor.view().unwrap();
    assert!(matches!(
        key_edit_for_view(KeyEvent::press(KeyCode::Space, Modifiers::NONE), list),
        Some(Edit::Activate)
    ));
    editor.edit(Edit::Select(Row::ResetAll));
    draw(&mut editor);
    editor.edit(Edit::Activate);
    draw(&mut editor);
    for key in [
        KeyEvent::press(KeyCode::Space, Modifiers::NONE),
        KeyEvent::press(KeyCode::Char(' '), Modifiers::NONE),
        KeyEvent::press(KeyCode::Char('y'), Modifiers::NONE),
        KeyEvent::press(KeyCode::Enter, Modifiers::CTRL),
        KeyEvent::press(KeyCode::Enter, Modifiers::NONE.with_shift()),
    ] {
        assert!(key_edit_for_view(key, editor.view().unwrap()).is_none());
        assert!(
            matches!(editor.view(), Some(View::Draft(draft)) if draft.confirmation == Some(Confirmation::ResetAll) && !draft.dirty)
        );
    }
    assert!(matches!(
        key_edit_for_view(
            KeyEvent::press(KeyCode::Esc, Modifiers::NONE),
            editor.view().unwrap()
        ),
        Some(Edit::Cancel)
    ));
    let edit = key_edit_for_view(
        KeyEvent::press(KeyCode::Char('m'), Modifiers::CTRL),
        editor.view().unwrap(),
    )
    .expect("legacy Enter");
    editor.edit(edit);
    assert!(
        matches!(editor.view(), Some(View::Draft(draft)) if draft.confirmation.is_none() && draft.dirty)
    );
}

#[test]
fn shifted_horizontal_keys_are_large_steps_only_outside_text_editors() {
    let shifted_left = KeyEvent::press(KeyCode::Left, Modifiers::NONE.with_shift());
    let shifted_right = KeyEvent::press(KeyCode::Right, Modifiers::NONE.with_shift());
    assert!(matches!(
        key_edit(shifted_left, false),
        Some(Edit::AdjustLarge(false))
    ));
    assert!(matches!(
        key_edit(shifted_right, false),
        Some(Edit::AdjustLarge(true))
    ));
    assert!(key_edit(shifted_left, true).is_none());
    assert!(key_edit(shifted_right, true).is_none());
}

#[test]
fn typed_boolean_and_changed_projections_follow_the_local_draft() {
    let mut editor = ready();
    let View::Draft(before) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(before.checked(Row::Sound), Some(false));
    assert!(!before.changed(Row::Sound));
    editor.edit(Edit::Select(Row::Sound));
    draw(&mut editor);
    editor.edit(Edit::Activate);
    let View::Draft(after) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(after.checked(Row::Sound), Some(true));
    assert!(after.changed(Row::Sound));
    assert!(!after.changed(Row::Save));
}

#[test]
fn wheel_preserves_the_focus_anchor_and_painted_geometry_owns_row_activation() {
    let mut editor = ready_with_application_theme(ApplicationAppearance::Light);
    editor.edit(Edit::Select(Row::Theme));
    draw(&mut editor);
    let View::Draft(before) = editor.view().unwrap() else {
        panic!("draft");
    };
    let token = before.token;
    let values = before.values.clone();
    let next = editor.scroll(token, false, 1).unwrap();
    assert_ne!(next, token);
    let View::Draft(visible) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(visible.selected, Row::Theme);
    assert_eq!(visible.values, &values);
    assert_eq!(visible.viewport, 1);
    assert!(!visible.follow_selection);
    assert_eq!(visible.first_visible(4), 1);
    editor.presented(visible.presentation(4));
    editor.edit(Edit::Adjust(true));
    assert!(matches!(
        editor.view(),
        Some(View::Draft(draft))
            if draft.values.application_theme == ApplicationAppearance::Dark
                && draft.selected == Row::Theme
                && !draft.follow_selection
    ));

    let View::Draft(visible) = editor.view().unwrap() else {
        panic!("draft");
    };
    let draw = visible.presentation(4);
    let token = visible.token;
    editor.presented(draw);
    let next = editor.scroll(token, false, 2).unwrap();
    let View::Draft(hidden) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(hidden.viewport, 3);
    editor.presented(hidden.presentation(4));
    editor.edit(Edit::Adjust(true));
    assert!(matches!(
        editor.view(),
        Some(View::Draft(draft))
            if draft.values.application_theme == ApplicationAppearance::Dark
                && draft.selected == Row::Theme
                && !draft.follow_selection
                && draft.token == next
    ));

    editor.edit(Edit::Next);
    let View::Draft(following) = editor.view().unwrap() else {
        panic!("draft");
    };
    let first = following.first_visible(4);
    let selected = following
        .entries()
        .iter()
        .position(|entry| *entry == Entry::Setting(following.selected))
        .unwrap();
    assert!(following.follow_selection);
    assert!((first..first + 4).contains(&selected));
}

#[test]
fn settings_mouse_controls_follow_text_back_confirmation_and_hidden_save_lifetimes() {
    let mut editor = ready();
    draw(&mut editor);
    editor.click(editor.view().unwrap().token(), Row::LaunchProfiles);
    draw(&mut editor);
    editor.click(editor.view().unwrap().token(), Row::ProfileName);
    editor.edit(Edit::Clear);
    editor.paste("Mouse profile");
    draw(&mut editor);
    let stale = editor.view().unwrap().token();
    editor.control(stale, Control::Accept);
    assert!(
        matches!(editor.view(), Some(View::Draft(draft)) if draft.text.is_none() && draft.values.launch_profiles.profiles[0].name.as_deref() == Some("Mouse profile"))
    );
    editor.control(stale, Control::Close);
    assert!(
        editor.visible(),
        "old control cannot close the new draft revision"
    );
    draw(&mut editor);
    editor.control(editor.view().unwrap().token(), Control::Back);
    assert!(
        matches!(editor.view(), Some(View::Draft(draft)) if draft.page == Page::General && draft.dirty)
    );
    draw(&mut editor);
    editor.click(editor.view().unwrap().token(), Row::ResetAll);
    draw(&mut editor);
    editor.control(editor.view().unwrap().token(), Control::Back);
    assert!(
        matches!(editor.view(), Some(View::Draft(draft)) if draft.confirmation.is_none() && draft.dirty)
    );
    draw(&mut editor);
    let submission = editor
        .click(editor.view().unwrap().token(), Row::Save)
        .unwrap();
    editor.control(submission.token, Control::Close);
    assert!(!editor.visible());
    assert!(
        editor.open().is_none(),
        "closing the surface retains the in-flight write"
    );
    assert!(editor.complete(submission.token));
    assert!(editor.open().is_some());
}
#[test]
fn only_a_dirty_successfully_drawn_draft_can_submit_once() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::Save));
    assert!(editor.edit(Edit::Activate).is_none());
    draw(&mut editor);
    let submission = editor.edit(Edit::Activate).unwrap();
    let SettingsOperation::Save { changes, .. } = &submission.operation else {
        panic!("save");
    };
    assert_eq!(
        changes.keys().collect::<Vec<_>>(),
        [PreferenceKey::SoundEnabled]
    );
    assert!(changes.apply(&basis().values).unwrap().sound_enabled);
    assert!(editor.edit(Edit::Activate).is_none());
    editor.edit(Edit::Cancel);
    assert!(!editor.visible());
    assert!(editor.open().is_none());
    assert!(editor.complete(submission.token));
    assert!(editor.open().is_some());
}
#[test]
fn stale_frames_cancelled_loads_and_hidden_frames_cannot_authorize_a_save() {
    let mut editor = Editor::default();
    let old = editor.open().unwrap();
    editor.edit(Edit::Cancel);
    assert!(!editor.loaded(old, basis()));
    let current = editor.open().unwrap();
    assert!(!editor.loaded(old, basis()));
    assert!(editor.loaded(current, basis()));
    draw(&mut editor);
    editor.edit(Edit::Adjust(true));
    editor.draw(old);
    assert!(editor.click(old, Row::Save).is_none());
    editor.edit(Edit::Select(Row::Save));
    draw(&mut editor);
    editor.not_presented();
    assert!(editor.edit(Edit::Activate).is_none());
    assert!(editor.visible(), "resize preserves the draft");
    draw(&mut editor);
    assert!(editor.edit(Edit::Activate).is_some());
}

#[test]
fn input_context_tracks_whole_surface_proof_across_state_replacements() {
    let mut editor = Editor::default();
    assert_eq!(editor.input_context(), None);

    let loading = editor.open().unwrap();
    assert_eq!(
        editor.input_context(),
        Some(SettingsInputContext {
            token: loading,
            phase: PresentationPhase::Pending,
            readiness: SettingsInputReadiness::Recovery,
            remaining_text_bytes: 0,
        })
    );
    editor.presented(SettingsDraw::surface(loading));
    assert_eq!(
        editor.input_context().unwrap().readiness,
        SettingsInputReadiness::Interactive
    );

    assert!(editor.loaded(loading, basis()));
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    let draft_token = draft.token;
    assert_ne!(draft_token, loading);
    assert_eq!(
        editor.input_context(),
        Some(SettingsInputContext {
            token: draft_token,
            phase: PresentationPhase::Admitted,
            readiness: SettingsInputReadiness::Recovery,
            remaining_text_bytes: 0,
        }),
        "loading proof cannot authorize its replacement draft"
    );
    editor.presented(SettingsDraw::surface(loading));
    assert_eq!(
        editor.input_context().unwrap().readiness,
        SettingsInputReadiness::Recovery,
        "a stale loading receipt cannot restore readiness"
    );
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    let draw = draft.presentation(4);
    editor.presented(draw);

    editor.edit(Edit::Select(Row::CompletionThreshold));
    editor.edit(Edit::Activate);
    let text = editor.input_context().unwrap();
    assert_eq!(text.readiness, SettingsInputReadiness::Interactive);
    assert_eq!(
        text.remaining_text_bytes,
        Row::CompletionThreshold.text_limit().unwrap()
            - basis()
                .values
                .completion_reminder_minimum_seconds
                .to_string()
                .len()
    );
    assert_ne!(text.token, draft_token);
    editor.not_presented();
    assert_eq!(
        editor.input_context().unwrap(),
        SettingsInputContext {
            token: text.token,
            phase: PresentationPhase::Admitted,
            readiness: SettingsInputReadiness::Recovery,
            remaining_text_bytes: 0,
        },
        "a covered surface retains capture intent without text admission"
    );
}

#[test]
fn saving_and_suspended_settings_require_a_new_surface_receipt() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    let submission = submit(&mut editor);
    assert_eq!(
        editor.input_context().unwrap().readiness,
        SettingsInputReadiness::Recovery,
        "the saving surface replaces its painted draft"
    );
    editor.presented(SettingsDraw::surface(submission.token));
    assert_eq!(
        editor.input_context().unwrap().readiness,
        SettingsInputReadiness::Interactive
    );

    assert!(editor.failed(submission.token, None));
    assert_eq!(
        editor.input_context().unwrap().readiness,
        SettingsInputReadiness::Recovery,
        "the restored draft replaces the painted saving surface"
    );
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("restored draft");
    };
    let draw = draft.presentation(4);
    editor.presented(draw);
    assert_eq!(
        editor.geometry_blocked(),
        Some(GeometrySettlement::Suspended)
    );
    let suspended = editor.input_context().unwrap();
    assert_eq!(suspended.phase, PresentationPhase::Suspended);
    assert_eq!(suspended.readiness, SettingsInputReadiness::Recovery);
    editor.presented(draw);
    assert_eq!(
        editor.input_context().unwrap().readiness,
        SettingsInputReadiness::Interactive
    );

    editor.edit(Edit::Cancel);
    assert_eq!(editor.input_context(), None);
}

#[test]
fn first_geometry_block_refuses_the_surface_but_settles_accepted_work() {
    let mut editor = Editor::default();
    let loading = editor.open().unwrap();
    assert_eq!(editor.presentation(), PresentationPhase::Pending);
    assert_eq!(editor.geometry_blocked(), Some(GeometrySettlement::Refused));
    assert!(!editor.visible());
    assert!(
        editor.open().is_none(),
        "the accepted read still owns this editor incarnation"
    );
    assert!(
        !editor.loaded(loading, basis()),
        "a refused surface settles the read without reopening itself"
    );
    assert!(editor.open().is_some());

    let mut loaded = Editor::default();
    let token = loaded.open().unwrap();
    assert!(loaded.loaded(token, basis()));
    assert_eq!(loaded.geometry_blocked(), Some(GeometrySettlement::Refused));
    assert!(
        !loaded.visible(),
        "an unpainted draft is not silently captured"
    );
}

#[test]
fn admitted_draft_suspends_and_resumes_with_its_local_navigation() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::LaunchProfiles));
    draw(&mut editor);
    editor.edit(Edit::Activate);
    draw(&mut editor);
    assert_eq!(editor.presentation(), PresentationPhase::Admitted);
    let View::Draft(before) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(before.page, Page::Profiles);
    assert!(before.dirty);

    assert_eq!(
        editor.geometry_blocked(),
        Some(GeometrySettlement::Suspended)
    );
    assert_eq!(editor.presentation(), PresentationPhase::Suspended);
    let View::Draft(suspended) = editor.view().unwrap() else {
        panic!("retained draft")
    };
    assert_eq!(suspended.page, Page::Profiles);
    assert!(suspended.values.sound_enabled);
    assert!(suspended.dirty);
    let suspended_token = suspended.token;
    let suspended_row = suspended.selected;
    let resumed_draw = suspended.presentation(4);
    assert!(editor.edit(Edit::Next).is_none());
    assert_eq!(
        editor.view().unwrap().token(),
        suspended_token,
        "suspended input cannot mutate the hidden draft"
    );

    editor.presented(resumed_draw);
    assert_eq!(editor.presentation(), PresentationPhase::Admitted);
    editor.edit(Edit::Next);
    assert!(matches!(
        editor.view(),
        Some(View::Draft(resumed))
            if resumed.page == Page::Profiles
                && resumed.values.sound_enabled
                && resumed.dirty
                && resumed.selected != suspended_row
    ));
}
#[test]
fn definite_failures_preserve_the_original_cas_basis_and_allow_an_explicit_retry() {
    let mut editor = ready();
    editor.edit(Edit::Adjust(true));
    let submission = submit(&mut editor);
    assert!(editor.failed(submission.token, None));
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(draft.values.locale, PreferencesLocale::EnUs);
    assert!(draft.dirty);
    assert!(
        editor.edit(Edit::Activate).is_none(),
        "failure disarms the frame"
    );
    draw(&mut editor);
    assert!(
        editor.click(submission.token, Row::Save).is_none(),
        "a prior frame cannot authorize the restored draft"
    );
    let retry = submit(&mut editor);
    let SettingsOperation::Save { source, .. } = retry.operation else {
        panic!("save");
    };
    assert_eq!(
        source.as_str(),
        "missing",
        "no silent rebase over the other client"
    );
}

#[test]
fn a_confirmed_observation_during_save_is_retained_if_the_draft_returns() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    let submission = submit(&mut editor);
    let mut latest = basis();
    latest.source = "b".repeat(64);
    latest.values.locale = PreferencesLocale::ZhCn;
    assert_eq!(editor.observe(&latest), Observation::Unchanged);
    assert!(editor.failed(submission.token, Some(Reconciliation::SourceChanged)));
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(draft.basis_source, "missing");
    assert_eq!(draft.effective_source, latest.source.as_str());
    assert_eq!(draft.effective_values.locale, PreferencesLocale::ZhCn);
    assert!(draft.conflict);
    assert_eq!(draft.reconciliation, Some(Reconciliation::SourceChanged));
    assert!(!draft.enabled(Row::Save));
}

#[test]
fn unknown_save_requires_a_matched_explicit_read_before_another_save() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    let submission = submit(&mut editor);
    assert!(editor.failed(submission.token, Some(Reconciliation::OutcomeUnknown)));
    let View::Draft(retained) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert!(retained.dirty);
    assert_eq!(
        retained.reconciliation,
        Some(Reconciliation::OutcomeUnknown)
    );
    assert!(!retained.enabled(Row::Save));

    draw(&mut editor);
    assert!(editor.edit(Edit::Activate).is_none());
    assert_eq!(
        editor.observe(&basis()),
        Observation::Unchanged,
        "a cached passive snapshot is not a new read receipt"
    );

    let mut passive = basis();
    passive.source = "c".repeat(64);
    passive.values.locale = PreferencesLocale::ZhCn;
    assert_eq!(editor.observe(&passive), Observation::Conflict);
    let View::Draft(conflicted) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(conflicted.basis_source, "missing");
    assert_eq!(conflicted.effective_source, passive.source.as_str());
    assert_eq!(
        conflicted.reconciliation,
        Some(Reconciliation::OutcomeUnknown),
        "passive runtime publication cannot clear unknown commit state"
    );
    assert!(!conflicted.enabled(Row::Save));
    assert!(!editor.loaded(submission.token, passive.clone()));

    editor.edit(Edit::Select(Row::Reload));
    editor.edit(Edit::Activate);
    draw(&mut editor);
    let reload = editor.edit(Edit::Activate).expect("explicit fresh read");
    assert!(matches!(reload.operation, SettingsOperation::Open {}));
    assert!(!editor.loaded(submission.token, passive.clone()));
    assert!(editor.loaded(reload.token, passive));
    assert!(matches!(
        editor.view(),
        Some(View::Draft(draft))
            if !draft.dirty
                && !draft.conflict
                && draft.reconciliation.is_none()
                && draft.basis_source == draft.effective_source
    ));
}
#[test]
fn external_observation_refreshes_clean_views_and_disarms_old_pointer_frames() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Theme));
    draw(&mut editor);
    let old = editor.view().unwrap().token();
    let mut latest = basis();
    latest.source = "a".repeat(64);
    latest.values.locale = PreferencesLocale::ZhCn;
    assert_eq!(editor.observe(&latest), Observation::Refreshed);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(draft.values.locale, PreferencesLocale::ZhCn);
    assert_eq!(draft.basis_values.locale, PreferencesLocale::ZhCn);
    assert_eq!(draft.effective_values.locale, PreferencesLocale::ZhCn);
    assert!(!draft.conflict);
    assert_eq!(draft.selected, Row::Theme);
    assert!(!draft.dirty);
    assert_ne!(draft.token, old);
    assert!(editor.click(old, Row::Sound).is_none());
    editor.edit(Edit::Adjust(true));
    let SettingsOperation::Save { source, changes } = submit(&mut editor).operation else {
        panic!("save")
    };
    assert_eq!(source.as_str(), latest.source);
    assert_eq!(
        changes.keys().collect::<Vec<_>>(),
        [PreferenceKey::ApplicationTheme]
    );
}
#[test]
fn external_observation_never_rebases_edits_text_or_confirmations() {
    for row in [Row::Sound, Row::CompletionThreshold, Row::ResetAll] {
        let mut editor = ready();
        editor.edit(Edit::Select(row));
        editor.edit(Edit::Activate);
        let old = editor.view().unwrap().token();
        let mut latest = basis();
        latest.source = "b".repeat(64);
        latest.values.locale = PreferencesLocale::ZhCn;
        assert_eq!(editor.observe(&latest), Observation::Conflict);
        let View::Draft(draft) = editor.view().unwrap() else {
            panic!("draft")
        };
        assert_eq!(draft.values.locale, PreferencesLocale::System);
        assert_eq!(draft.basis_values.locale, PreferencesLocale::System);
        assert_eq!(draft.effective_values.locale, PreferencesLocale::ZhCn);
        assert!(draft.conflict);
        assert_ne!(draft.token, old, "a new effective value revokes old hits");
        let State::Draft(draft) = &editor.state else {
            panic!("draft")
        };
        assert_eq!(draft.basis.source, "missing");
        if row == Row::CompletionThreshold {
            assert!(draft.text.is_some());
        }
        if row == Row::ResetAll {
            assert_eq!(draft.confirmation, Some(Confirmation::ResetAll));
        }
    }
}

#[test]
fn profiles_child_freezes_the_basis_and_restores_parent_navigation() {
    let mut editor = ready();
    draw(&mut editor);
    let token = editor.view().unwrap().token();
    editor.scroll(token, false, 4).unwrap();
    editor.edit(Edit::Select(Row::LaunchProfiles));
    let View::Draft(parent) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(parent.viewport, 4);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    assert!(matches!(
        editor.view(),
        Some(View::Draft(draft)) if draft.page == Page::Profiles && draft.selected == Row::Profile
    ));

    let mut latest = basis();
    latest.source = "d".repeat(64);
    latest.values.locale = PreferencesLocale::ZhCn;
    assert_eq!(editor.observe(&latest), Observation::Conflict);
    let View::Draft(child) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(child.values.locale, PreferencesLocale::System);
    assert_eq!(child.basis_values.locale, PreferencesLocale::System);
    assert_eq!(child.effective_values.locale, PreferencesLocale::ZhCn);
    assert!(child.conflict);

    editor.edit(Edit::Cancel);
    let View::Draft(restored) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(restored.page, Page::General);
    assert_eq!(restored.selected, Row::LaunchProfiles);
    assert_eq!(restored.viewport, 4);
    assert!(restored.follow_selection);
    assert!(restored.conflict);
}
#[test]
fn profile_reset_claims_only_catalog_and_preserves_other_edits_and_valid_selection() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::LaunchProfiles));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::AddProfile));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::ResetProfiles));
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(draft.profile, 0);
    assert_eq!(draft.values.launch_profiles, Default::default());
    assert!(draft.values.sound_enabled);
    assert!(draft.dirty, "reset claims an already-default catalog");
    editor.edit(Edit::Cancel);
    let SettingsOperation::Save { changes, .. } = submit(&mut editor).operation else {
        panic!("save")
    };
    assert_eq!(changes.keys().count(), 2);
    assert_eq!(
        serde_json::to_value(&changes).unwrap()["launchProfiles"],
        serde_json::Value::Null
    );
    assert!(changes.apply(&basis().values).unwrap().sound_enabled);
}
#[test]
fn reset_all_rewinds_selection_after_visiting_an_added_profile() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::LaunchProfiles));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::AddProfile));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Cancel);
    editor.edit(Edit::Select(Row::ResetAll));
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(draft.profile, 0);
    assert!(draft.enabled(Row::AddProfile));
}
#[test]
fn explicit_reset_claims_even_default_leaves_and_later_edits_override_only_their_value() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::ResetAll));
    editor.edit(Edit::Activate);
    assert!(
        matches!(editor.view(), Some(View::Draft(draft)) if draft.confirmation == Some(Confirmation::ResetAll))
    );
    draw(&mut editor);
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    let SettingsOperation::Save { changes, .. } = submit(&mut editor).operation else {
        panic!("save");
    };
    assert_eq!(changes.keys().count(), PreferenceKey::ALL.len());
    assert!(changes.apply(&basis().values).unwrap().sound_enabled);
}
#[test]
fn text_is_grapheme_atomic_and_invalid_values_keep_the_editor_open() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::CompletionThreshold));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Clear);
    editor.paste("0");
    editor.edit(Edit::Activate);
    assert!(
        matches!(editor.view(), Some(View::Draft(draft)) if draft.invalid && draft.text.is_some())
    );
    editor.edit(Edit::Clear);
    editor.paste("3600");
    editor.edit(Edit::Activate);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(draft.values.completion_reminder_minimum_seconds, 3600);
    editor.edit(Edit::Select(Row::LaunchProfiles));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::ProfileName));
    editor.edit(Edit::Activate);
    editor.paste("A👩‍💻");
    editor.edit(Edit::Backspace);
    editor.edit(Edit::Activate);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(
        draft.values.launch_profiles.profiles[0].name.as_deref(),
        Some("A")
    );
}
#[test]
fn profile_arguments_are_literal_and_deleting_the_default_selects_an_existing_profile() {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::LaunchProfiles));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::AddProfile));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::CommandType));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::Executable));
    editor.edit(Edit::Activate);
    editor.paste(if cfg!(windows) {
        "C:\\tools\\shell.exe"
    } else {
        "/bin/sh"
    });
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::AddArgument));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Activate);
    editor.paste("a b;$literal");
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::SetDefault));
    editor.edit(Edit::Activate);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(draft.values.launch_profiles.default_profile_id, "profile-1");
    let orchbee_workbench_protocol::UserLaunchCommand::CustomExecutable { arguments, .. } =
        &draft.values.launch_profiles.profiles[1].command
    else {
        panic!("custom");
    };
    assert_eq!(arguments, &["a b;$literal"]);
    editor.edit(Edit::Select(Row::DeleteProfile));
    editor.edit(Edit::Activate);
    draw(&mut editor);
    editor.edit(Edit::Activate);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    assert_eq!(
        draft.values.launch_profiles.default_profile_id,
        "system-default"
    );
}

fn invalid_nondefault_profile() -> Editor {
    let mut editor = ready();
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::LaunchProfiles));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::AddProfile));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::ProfileName));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Clear);
    editor.paste("Broken profile");
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::CommandType));
    editor.edit(Edit::Activate);
    // Return while a different valid profile is selected, as in the reported failure.
    editor.edit(Edit::Select(Row::Profile));
    editor.edit(Edit::Adjust(false));
    editor.edit(Edit::Cancel);
    editor.edit(Edit::Select(Row::Save));
    draw(&mut editor);
    editor
}

#[test]
fn save_locates_the_nondefault_executable_and_correction_preserves_the_complete_draft() {
    let mut editor = invalid_nondefault_profile();
    let View::Draft(before) = editor.view().unwrap() else {
        panic!("draft")
    };
    let values = before.values.clone();
    let stale = before.token;
    assert!(
        editor.edit(Edit::Activate).is_none(),
        "invalid Save cannot dispatch"
    );
    let View::Draft(error) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(error.values, &values);
    assert_eq!(error.basis_source, "missing");
    assert_eq!(error.page, Page::Profiles);
    assert_eq!(error.profile, 1);
    assert_eq!(error.selected, Row::Executable);
    assert_eq!(error.invalid_field, Some((1, UserLaunchField::Executable)));
    assert!(error.invalid && error.follow_selection && error.text.is_none());
    assert_ne!(error.token, stale);
    editor.control(stale, Control::Close);
    assert!(editor.visible());
    draw(&mut editor);
    editor.edit(Edit::Activate);
    editor.paste(if cfg!(windows) {
        "C:\\tools\\shell.exe"
    } else {
        "/bin/sh"
    });
    editor.edit(Edit::Activate);
    let View::Draft(corrected) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert!(!corrected.invalid);
    assert_eq!(corrected.invalid_field, None);
    assert_eq!(corrected.profile, 1);
    assert_eq!(
        corrected.values.launch_profiles.default_profile_id,
        "system-default"
    );
    assert!(corrected.values.sound_enabled);
    editor.edit(Edit::Cancel);
    let View::Draft(parent) = editor.view().unwrap() else {
        panic!("draft")
    };
    assert_eq!(
        parent.selected,
        Row::Save,
        "validation return restores its parent"
    );
    draw(&mut editor);
    let submission = editor
        .edit(Edit::Activate)
        .expect("corrected draft saves directly");
    let SettingsOperation::Save { changes, .. } = submission.operation else {
        panic!("save")
    };
    let saved = changes.apply(&basis().values).unwrap();
    assert!(saved.sound_enabled);
    assert_eq!(saved.launch_profiles.profiles.len(), 2);
    assert_eq!(
        saved.launch_profiles.profiles[1].name.as_deref(),
        Some("Broken profile")
    );
    assert!(editor.complete(submission.token));
    let reopened = editor.open().unwrap();
    assert!(editor.loaded(
        reopened,
        Basis {
            values: saved,
            ..basis()
        }
    ));
    assert!(
        matches!(editor.view(), Some(View::Draft(draft)) if !draft.invalid && draft.invalid_field.is_none() && !draft.dirty)
    );
}

#[test]
fn removing_resetting_or_reloading_an_invalid_profile_retires_its_diagnostic() {
    for action in [
        Row::DeleteProfile,
        Row::ResetProfiles,
        Row::ResetAll,
        Row::Reload,
    ] {
        let mut editor = invalid_nondefault_profile();
        assert!(editor.edit(Edit::Activate).is_none());
        if matches!(action, Row::ResetAll | Row::Reload) {
            editor.edit(Edit::Cancel);
        }
        editor.edit(Edit::Select(action));
        editor.edit(Edit::Activate);
        draw(&mut editor);
        let operation = editor.edit(Edit::Activate);
        if action == Row::Reload {
            let reload = operation.expect("explicit reload");
            assert!(editor.loaded(reload.token, basis()));
        } else {
            assert!(operation.is_none(), "delete/reset remains local");
        }
        let View::Draft(draft) = editor.view().unwrap() else {
            panic!("draft")
        };
        assert!(!draft.invalid);
        assert_eq!(draft.invalid_field, None);
        assert_eq!(draft.profile, 0);
        assert!(draft.values.validate().is_ok());
        assert_eq!(
            draft.values.sound_enabled,
            matches!(action, Row::DeleteProfile | Row::ResetProfiles)
        );
        if action != Row::Reload {
            if draft.page == Page::Profiles {
                editor.edit(Edit::Cancel);
            }
            assert!(matches!(
                submit(&mut editor).operation,
                SettingsOperation::Save { .. }
            ));
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Trace {
    Open,
    Load,
    Draw,
    Theme,
    Reminders,
    Sound,
    SaveRow,
    Activate,
    NextChoice,
    PreviousChoice,
    Cancel,
    Complete,
    StaleFrame,
}

#[derive(Clone, Copy, Debug)]
enum ProvisionalChoice {
    Theme(usize),
    Reminders(bool),
}

/// An independent episode oracle tracks only semantic values and authorization,
/// not the editor's token allocation or internal state representation.
fn replay(trace: &[Trace]) -> Result<(), String> {
    let mut editor = Editor::default();
    let mut phase = 0;
    let mut visible = false;
    let mut pending = None;
    let mut stale = None;
    let mut values = (0, false, false);
    let mut row = Row::Theme;
    let mut presented = false;
    let mut drawn = false;
    let mut saves = Vec::new();
    let mut provisional = None;
    let saved_values = UserPreferencesValues::default();
    let names = [
        ApplicationAppearance::System,
        ApplicationAppearance::Light,
        ApplicationAppearance::Dark,
    ];
    for op in trace {
        match op {
            Trace::Open => {
                let token = editor.open();
                if token.is_some() != (phase == 0) {
                    return Err("open admission".into());
                }
                if let Some(token) = token {
                    phase = 1;
                    visible = true;
                    pending = Some(token);
                    stale = stale.or(Some(token));
                }
            }
            Trace::Load => {
                if let Some(token) = pending {
                    let accepted = editor.loaded(token, basis());
                    let expected = phase == 1 && visible;
                    if accepted != expected {
                        return Err("load admission".into());
                    }
                    if accepted {
                        editor.edit(Edit::Select(Row::Theme));
                    }
                    if phase == 1 {
                        phase = if visible { 2 } else { 0 };
                        values = (0, false, false);
                        row = Row::Theme;
                        presented = false;
                        drawn = false;
                        provisional = None;
                    }
                }
            }
            Trace::Draw => {
                if let Some(view) = editor.view() {
                    editor.draw(view.token());
                    if phase == 2 {
                        presented = true;
                        drawn = true;
                    }
                }
            }
            Trace::Theme | Trace::Reminders | Trace::Sound | Trace::SaveRow => {
                let selected = match op {
                    Trace::Theme => Row::Theme,
                    Trace::Reminders => Row::Reminders,
                    Trace::Sound => Row::Sound,
                    _ => Row::Save,
                };
                editor.edit(Edit::Select(selected));
                if phase == 2 {
                    row = selected;
                    drawn = false;
                    provisional = None;
                }
            }
            Trace::Activate => {
                let dirty = values != (0, false, false);
                let expected =
                    phase == 2 && provisional.is_none() && row == Row::Save && drawn && dirty;
                let submission = editor.edit(Edit::Activate);
                if submission.is_some() != expected {
                    return Err("save requires drawn dirty intent".into());
                }
                if let Some(submission) = submission {
                    if saves.contains(&submission.token) {
                        return Err("repeated save identity".into());
                    }
                    saves.push(submission.token);
                    pending = Some(submission.token);
                    phase = 3;
                    let SettingsOperation::Save { changes, .. } = submission.operation else {
                        return Err("wrong operation".into());
                    };
                    let expected = UserPreferencesValues {
                        application_theme: names[values.0],
                        agent_notification_mode: if values.1 {
                            AgentReminderMode::Hidden
                        } else {
                            AgentReminderMode::Never
                        },
                        sound_enabled: values.2,
                        ..UserPreferencesValues::default()
                    };
                    if changes != UserPreferencesPatch::between(&saved_values, &expected) {
                        return Err("dirty leaves".into());
                    }
                } else if phase == 2 && presented && row != Row::Save {
                    if let Some(candidate) = provisional {
                        // Enter opens a picker first. Only a subsequently drawn candidate can
                        // cross its local acceptance boundary; the parent draft owns Save.
                        if drawn {
                            match candidate {
                                ProvisionalChoice::Theme(index) => values.0 = index,
                                ProvisionalChoice::Reminders(hidden) => values.1 = hidden,
                            }
                            provisional = None;
                            drawn = false;
                        }
                    } else {
                        match row {
                            Row::Theme => provisional = Some(ProvisionalChoice::Theme(values.0)),
                            Row::Reminders => {
                                provisional = Some(ProvisionalChoice::Reminders(values.1))
                            }
                            Row::Sound => values.2 = !values.2,
                            _ => {}
                        }
                        drawn = false;
                    }
                }
            }
            Trace::NextChoice | Trace::PreviousChoice => {
                // These trace operations navigate an open choice episode; root row selection
                // remains represented by the named Theme/Reminders/Sound/SaveRow operations.
                if let Some(candidate) = &mut provisional {
                    let forward = matches!(op, Trace::NextChoice);
                    editor.edit(if forward { Edit::Next } else { Edit::Previous });
                    match candidate {
                        ProvisionalChoice::Theme(index) => {
                            *index = (*index + if forward { 1 } else { 2 }) % 3;
                        }
                        ProvisionalChoice::Reminders(hidden) => *hidden = !*hidden,
                    }
                    drawn = false;
                }
            }
            Trace::Cancel => {
                editor.edit(Edit::Cancel);
                if provisional.take().is_some() {
                    // Local return restores the accepted parent appearance and retains its task.
                    drawn = false;
                } else {
                    visible = false;
                    presented = false;
                    if !matches!(phase, 1 | 3) {
                        phase = 0;
                    }
                }
            }
            Trace::Complete => {
                if let Some(token) = pending {
                    if editor.complete(token) != matches!(phase, 1 | 3) {
                        return Err("completion admission".into());
                    }
                    if matches!(phase, 1 | 3) {
                        phase = 0;
                        visible = false;
                        presented = false;
                        provisional = None;
                    }
                }
            }
            Trace::StaleFrame => {
                if let Some(token) = stale {
                    editor.draw(token);
                    if editor.click(token, Row::Save).is_some() {
                        return Err("stale frame saved".into());
                    }
                }
            }
        }
        if editor.visible() != (phase == 2 || visible && matches!(phase, 1 | 3)) {
            return Err("surface ownership".into());
        }
        if let Some(View::Draft(draft)) = editor.view() {
            if draft.values.application_theme != names[values.0]
                || (draft.values.agent_notification_mode == AgentReminderMode::Hidden) != values.1
                || draft.values.sound_enabled != values.2
                || draft.selected != row
                || draft.dirty != (values != (0, false, false))
            {
                return Err("draft projection".into());
            }
            let expected_picker = provisional.map(|candidate| match candidate {
                ProvisionalChoice::Theme(index) => {
                    (Row::Theme, index, Choice::ApplicationTheme(names[values.0]))
                }
                ProvisionalChoice::Reminders(hidden) => (
                    Row::Reminders,
                    usize::from(hidden),
                    Choice::AgentReminders(if values.1 {
                        AgentReminderMode::Hidden
                    } else {
                        AgentReminderMode::Never
                    }),
                ),
            });
            if draft
                .picker
                .map(|picker| (picker.row(), picker.cursor(), picker.original()))
                != expected_picker
            {
                return Err("provisional choice is distinct from the accepted parent".into());
            }
            let preview_theme = match provisional {
                Some(ProvisionalChoice::Theme(index)) => names[index],
                _ => names[values.0],
            };
            if editor.appearance_preview().unwrap().application_theme != preview_theme {
                return Err("appearance preview projection".into());
            }
            if draft.basis_values != &saved_values || draft.effective_values != &saved_values {
                return Err("local navigation changed saved or effective values".into());
            }
        }
    }
    Ok(())
}
fn shrink(mut trace: Vec<Trace>) -> Vec<Trace> {
    let mut index = 0;
    while index < trace.len() {
        let mut candidate = trace.clone();
        candidate.remove(index);
        if replay(&candidate).is_err() {
            trace = candidate;
        } else {
            index += 1;
        }
    }
    trace
}
#[test]
fn generated_settings_episodes_preserve_draft_authority_and_shrink_replayable_failures() {
    let choices = [
        Trace::Open,
        Trace::Load,
        Trace::Draw,
        Trace::Theme,
        Trace::Reminders,
        Trace::Sound,
        Trace::SaveRow,
        Trace::Activate,
        Trace::Cancel,
        Trace::Complete,
        Trace::StaleFrame,
    ];
    let mut picker_choices = choices.to_vec();
    picker_choices.extend([Trace::NextChoice, Trace::PreviousChoice]);
    // Preserve the original seeded traces and add a second cohort with provisional navigation.
    for choices in [choices.as_slice(), picker_choices.as_slice()] {
        let mut seed = 0x827e_402a_616d_9147_u64;
        for _ in 0..256 {
            let mut trace = Vec::new();
            for _ in 0..128 {
                seed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                trace.push(choices[(seed >> 32) as usize % choices.len()]);
            }
            if let Err(error) = replay(&trace) {
                panic!("{error}; replay={:?}", shrink(trace));
            }
        }
    }
}

#[test]
fn episode_oracle_keeps_provisional_accept_cancel_and_undrawn_acceptance_replayable() {
    for tail in [
        vec![Trace::Activate],
        vec![Trace::Activate, Trace::NextChoice, Trace::Activate],
        vec![
            Trace::Activate,
            Trace::NextChoice,
            Trace::Draw,
            Trace::Activate,
            Trace::SaveRow,
            Trace::Draw,
            Trace::Activate,
            Trace::Complete,
        ],
        vec![
            Trace::Activate,
            Trace::NextChoice,
            Trace::Cancel,
            Trace::Sound,
            Trace::Activate,
            Trace::SaveRow,
            Trace::Draw,
            Trace::Activate,
            Trace::Complete,
        ],
        vec![
            Trace::Reminders,
            Trace::Activate,
            Trace::NextChoice,
            Trace::PreviousChoice,
            Trace::NextChoice,
            Trace::Draw,
            Trace::Activate,
            Trace::SaveRow,
            Trace::Draw,
            Trace::Activate,
            Trace::Complete,
        ],
    ] {
        let mut trace = vec![Trace::Open, Trace::Load, Trace::Draw];
        trace.extend(tail);
        assert_eq!(replay(&trace), Ok(()), "{trace:?}");
    }
}

#[test]
fn login_shell_admission_uses_the_injected_capability_without_changing_other_draft_values() {
    for supported in [false, true] {
        let mut editor = Editor::new(supported);
        let token = editor.open().unwrap();
        assert!(editor.loaded(token, basis()));
        draw(&mut editor);
        editor.edit(Edit::Select(Row::LaunchProfiles));
        editor.edit(Edit::Activate);
        editor.edit(Edit::Select(Row::Login));
        draw(&mut editor);
        let View::Draft(before) = editor.view().unwrap() else {
            panic!("draft")
        };
        assert_eq!(before.enabled(Row::Login), supported);
        let mut expected = before.values.clone();
        if supported {
            expected.launch_profiles.profiles[0].command =
                orchbee_workbench_protocol::UserLaunchCommand::SystemShell { login: true };
        }
        editor.edit(Edit::Adjust(true));
        let View::Draft(after) = editor.view().unwrap() else {
            panic!("draft")
        };
        assert_eq!(after.values, &expected);
    }
}
