//! Serializable semantic traces pin draw authority, one submission and cancelled re-entry.
use super::*;
fn basis() -> Basis {
    Basis {
        revision: WorkbenchRevision::new(1),
        source: NativeSettingsSource::parse("missing").unwrap(),
        values: Values {
            theme: ConfigurationThemeName::TerminalNative,
            reminders: true,
            sound: false,
        },
    }
}
fn ready() -> Editor {
    let mut editor = Editor::default();
    let token = editor.open().unwrap();
    assert!(editor.loaded(token, basis()));
    editor
}
#[test]
fn only_a_dirty_successfully_drawn_draft_can_submit_once() {
    let mut editor = ready();
    editor.edit(Edit::Adjust(true));
    editor.edit(Edit::Select(Row::Save));
    assert!(editor.edit(Edit::Activate).is_none());
    editor.draw(editor.view().unwrap().token());
    let submission = editor.edit(Edit::Activate).unwrap();
    assert!(matches!(
        submission.operation,
        NativeSettingsOperation::Save {
            changes: NativeSettingsChanges {
                theme_name: Some(ConfigurationThemeName::OneLight),
                background_reminders: None,
                sound_enabled: None
            },
            ..
        }
    ));
    assert!(editor.edit(Edit::Activate).is_none());
    editor.edit(Edit::Cancel);
    assert!(!editor.visible());
    assert!(editor.open().is_none());
    assert!(editor.complete(submission.token));
    assert!(editor.open().is_some());
    assert!(!editor.complete(submission.token));
}
#[test]
fn stale_frames_and_late_loading_replies_cannot_open_or_save_a_replacement_episode() {
    let mut editor = ready();
    let old = editor.view().unwrap().token();
    editor.edit(Edit::Adjust(true));
    editor.edit(Edit::Select(Row::Save));
    editor.draw(old);
    assert!(editor.click(old, Row::Save).is_none());
    assert!(editor.edit(Edit::Activate).is_none());
    editor.edit(Edit::Cancel);
    let next = editor.open().unwrap();
    editor.edit(Edit::Cancel);
    assert!(!editor.loaded(next, basis()));
    let replacement = editor.open().unwrap();
    assert!(!editor.loaded(next, basis()));
    assert!(editor.loaded(replacement, basis()));
    let token = editor.view().unwrap().token();
    editor.draw(token);
    assert!(editor.click(token, Row::Save).is_none());
}
#[test]
fn reversal_is_a_no_op_and_save_can_include_each_owned_dirty_leaf() {
    let mut editor = ready();
    editor.edit(Edit::Adjust(true));
    editor.edit(Edit::Adjust(false));
    editor.edit(Edit::Select(Row::Save));
    editor.draw(editor.view().unwrap().token());
    assert!(editor.edit(Edit::Activate).is_none());
    editor.edit(Edit::Select(Row::Theme));
    editor.edit(Edit::Adjust(true));
    editor.edit(Edit::Select(Row::Reminders));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::Sound));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::Save));
    editor.draw(editor.view().unwrap().token());
    let NativeSettingsOperation::Save { changes, .. } =
        editor.edit(Edit::Activate).unwrap().operation
    else {
        panic!("save");
    };
    assert_eq!(changes.background_reminders, Some(false));
    assert_eq!(changes.sound_enabled, Some(true));
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
    let mut values = (0, true, false);
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
                    if phase == 1 {
                        phase = if visible { 2 } else { 0 };
                        values = (0, true, false);
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
                let dirty = values != (0, true, false);
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
                        ConfigurationThemeName::TerminalNative,
                        ConfigurationThemeName::OneLight,
                        ConfigurationThemeName::OneDark,
                    ];
                    if changes.theme_name != (values.0 != 0).then_some(names[values.0])
                        || changes.background_reminders != (!values.1).then_some(false)
                        || changes.sound_enabled != values.2.then_some(true)
                    {
                        return Err("dirty leaves".into());
                    }
                } else if phase == 2 && row != Row::Save {
                    match row {
                        Row::Theme => values.0 = (values.0 + 1) % 3,
                        Row::Reminders => values.1 = !values.1,
                        Row::Sound => values.2 = !values.2,
                        Row::Save => {}
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
        if let Some(View::Draft {
            values: actual,
            selected,
            dirty,
            ..
        }) = editor.view()
        {
            let names = [
                ConfigurationThemeName::TerminalNative,
                ConfigurationThemeName::OneLight,
                ConfigurationThemeName::OneDark,
            ];
            if actual
                != (Values {
                    theme: names[values.0],
                    reminders: values.1,
                    sound: values.2,
                })
                || selected != row
                || dirty != (values != (0, true, false))
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
