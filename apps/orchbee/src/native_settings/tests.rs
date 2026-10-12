//! Independent episode traces and concrete conflict/reset/text boundaries.
use super::*;
use orchbee_workbench_protocol::{AgentReminderMode, ApplicationAppearance, PreferencesLocale};

fn basis() -> Basis {
    Basis {
        source: "missing".into(),
        values: UserPreferencesValues::default(),
        claimed: Default::default(),
        desktop_imported: false,
        desktop_main_imported: false,
    }
}
fn ready() -> Editor {
    let mut editor = Editor::default();
    let token = editor.open().unwrap();
    assert!(editor.loaded(token, basis()));
    editor
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
fn settings_mouse_controls_follow_text_back_confirmation_and_hidden_save_lifetimes() {
    let mut editor = ready();
    draw(&mut editor);
    editor.control(
        editor.view().unwrap().token(),
        Control::Row(Row::LaunchProfiles),
    );
    draw(&mut editor);
    editor.control(
        editor.view().unwrap().token(),
        Control::Row(Row::ProfileName),
    );
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
    editor.control(editor.view().unwrap().token(), Control::Row(Row::ResetAll));
    draw(&mut editor);
    editor.control(editor.view().unwrap().token(), Control::Back);
    assert!(
        matches!(editor.view(), Some(View::Draft(draft)) if draft.confirmation.is_none() && draft.dirty)
    );
    draw(&mut editor);
    let submission = editor
        .control(editor.view().unwrap().token(), Control::Row(Row::Save))
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
    let NativeSettingsOperation::Save { changes, .. } = &submission.operation else {
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
fn failures_preserve_the_original_cas_basis_and_the_exact_draft_without_replay() {
    let mut editor = ready();
    editor.edit(Edit::Adjust(true));
    let submission = submit(&mut editor);
    assert!(editor.failed(submission.token));
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
    let NativeSettingsOperation::Save { source, .. } = retry.operation else {
        panic!("save");
    };
    assert_eq!(
        source.as_str(),
        "missing",
        "no silent rebase over the other client"
    );
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
    assert_eq!(draft.selected, Row::Theme);
    assert!(!draft.dirty);
    assert_ne!(draft.token, old);
    assert!(editor.click(old, Row::Sound).is_none());
    editor.edit(Edit::Adjust(true));
    let NativeSettingsOperation::Save { source, changes } = submit(&mut editor).operation else {
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
        assert_eq!(draft.token, old);
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
    let NativeSettingsOperation::Save { changes, .. } = submit(&mut editor).operation else {
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
    let NativeSettingsOperation::Save { changes, .. } = submit(&mut editor).operation else {
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
    Cancel,
    Complete,
    StaleFrame,
}

/// An independent episode oracle tracks only semantic values and authorization,
/// not the editor's token allocation or internal state representation.
fn replay(trace: &[Trace]) -> Result<(), String> {
    let mut editor = Editor::default();
    let mut phase = 0;
    let mut visible = false;
    let mut pending = None;
    let mut stale = None;
    let mut values = (1, false, false);
    let mut row = Row::Theme;
    let mut drawn = false;
    let mut saves = Vec::new();
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
                        values = (1, false, false);
                        row = Row::Theme;
                        drawn = false;
                    }
                }
            }
            Trace::Draw => {
                if let Some(view) = editor.view() {
                    editor.draw(view.token());
                    if phase == 2 {
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
                }
            }
            Trace::Activate => {
                let dirty = values != (1, false, false);
                let expected = phase == 2 && row == Row::Save && drawn && dirty;
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
                    let NativeSettingsOperation::Save { changes, .. } = submission.operation else {
                        return Err("wrong operation".into());
                    };
                    let names = [
                        ApplicationAppearance::System,
                        ApplicationAppearance::Light,
                        ApplicationAppearance::Dark,
                    ];
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
                    if changes != UserPreferencesPatch::between(&basis().values, &expected) {
                        return Err("dirty leaves".into());
                    }
                } else if phase == 2 && row != Row::Save {
                    match row {
                        Row::Theme => values.0 = (values.0 + 1) % 3,
                        Row::Reminders => values.1 = !values.1,
                        Row::Sound => values.2 = !values.2,
                        _ => {}
                    }
                    drawn = false;
                }
            }
            Trace::Cancel => {
                editor.edit(Edit::Cancel);
                visible = false;
                if !matches!(phase, 1 | 3) {
                    phase = 0;
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
            let names = [
                ApplicationAppearance::System,
                ApplicationAppearance::Light,
                ApplicationAppearance::Dark,
            ];
            if draft.values.application_theme != names[values.0]
                || (draft.values.agent_notification_mode == AgentReminderMode::Hidden) != values.1
                || draft.values.sound_enabled != values.2
                || draft.selected != row
                || draft.dirty != (values != (1, false, false))
            {
                return Err("draft projection".into());
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
