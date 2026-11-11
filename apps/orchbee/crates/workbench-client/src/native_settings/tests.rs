//! Rules of the instant-apply page: one edit per change, one edit in flight, a refused edit puts
//! its row back, per-row restore, and the narrow tier's one column.
use super::*;
use crate::client_view::{KeyCode, KeyEvent, Modifiers};
use orchbee_local_runtime::UserSettingsField;
use orchbee_workbench_protocol::{
    ApplicationAppearance, SettingChange, SettingPath, SettingValue, SettingsPlatform,
    WorkbenchRevision,
};

/// The snapshot the Server publishes at `revision` for a file that gives `values` and sets `set`.
fn snapshot(revision: u64, values: &UserSettingsValues, set: &[&str]) -> SettingsSnapshot {
    let changes = orchbee_local_runtime::value_changes(&UserSettingsValues::default(), values, &[]);
    SettingsSnapshot {
        revision: WorkbenchRevision::new(revision),
        file_path: Some("config.toml".into()),
        file: SettingsFile::Ok {},
        platform: if cfg!(windows) {
            SettingsPlatform::Windows
        } else if cfg!(target_os = "macos") {
            SettingsPlatform::Macos
        } else {
            SettingsPlatform::Linux
        },
        values: changes
            .into_iter()
            .filter_map(|change| match change {
                SettingChange::Set { key, value } => Some((
                    key,
                    Some(match value {
                        SettingValue::Boolean(value) => SettingScalar::Boolean(value),
                        SettingValue::Integer(value) => SettingScalar::Integer(value),
                        SettingValue::Text(value) => SettingScalar::Text(value),
                        SettingValue::Entries(_) => return None,
                    }),
                )),
                SettingChange::Restore { .. } => None,
            })
            .collect(),
        set: set
            .iter()
            .map(|key| SettingPath::parse(key).unwrap())
            .collect(),
        launch_profiles: values.launch_profiles.clone(),
        diagnostics: Default::default(),
    }
}

fn platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn open(snapshot: &SettingsSnapshot) -> Editor {
    let mut editor = Editor::default();
    let token = editor.open().unwrap();
    assert!(editor.loaded(token, snapshot));
    draw(&mut editor);
    editor
}

fn draw(editor: &mut Editor) {
    editor.draw(editor.view().unwrap().token());
}

fn page(editor: &Editor) -> PageView<'_> {
    match editor.view() {
        Some(View::Page(page)) => page,
        _ => panic!("the page is open"),
    }
}

fn select(editor: &mut Editor, row: Row) {
    editor.edit(Edit::Select(row));
    draw(editor);
    assert_eq!(page(editor).selected(), row);
}

/// The keys one edit sets and the keys it restores, in the file's spelling.
fn keys(submission: &Submission) -> (Vec<String>, Vec<String>) {
    let SettingsOperation::Edit { changes } = &submission.operation else {
        panic!("a change sends one edit");
    };
    let mut set = Vec::new();
    let mut restored = Vec::new();
    for change in changes {
        match change {
            SettingChange::Set { key, .. } => set.push(key.to_string()),
            SettingChange::Restore { key } => restored.push(key.to_string()),
        }
    }
    (set, restored)
}

fn key(field: UserSettingsField) -> String {
    field.key().unwrap().to_owned()
}

#[test]
fn shared_preference_metadata_is_complete_and_every_setting_stays_reachable() {
    assert_eq!(
        SETTINGS
            .iter()
            .map(|metadata| metadata.key)
            .collect::<Vec<_>>(),
        UserSettingsField::ALL.into_iter().collect::<Vec<_>>(),
        "the native projection must cover the closed protocol inventory in canonical order"
    );
    for metadata in &SETTINGS {
        assert_eq!(setting(metadata.key), metadata);
        assert_eq!(setting_for_row(metadata.row), Some(metadata));
        let placed = Category::ALL
            .iter()
            .flat_map(|category| category.groups())
            .filter(|group| match metadata.row {
                Row::LightTheme => **group == Group::LightThemes,
                Row::DarkTheme => **group == Group::DarkThemes,
                row => group.rows().contains(&row),
            })
            .count();
        assert_eq!(placed, 1, "{:?} has exactly one place", metadata.row);
    }
    assert_eq!(
        setting(UserSettingsField::CustomForeground).availability,
        SettingAvailability::TerminalDisplayThemeCustom
    );
    assert_eq!(
        setting(UserSettingsField::LaunchProfiles).effect,
        SettingEffect::NewDefaultLaunch
    );
    assert_eq!(
        setting(UserSettingsField::CompletionReminderMinimumSeconds).control,
        SettingControl::BoundedInteger {
            minimum: 1,
            maximum: 3_600,
            small_step: 1,
            large_step: 10,
            unit: SettingUnit::Seconds,
        }
    );
    assert_eq!(
        setting(UserSettingsField::ScrollbackBytes).control,
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

/// How a test changes a row.
#[derive(Clone, Copy, Debug)]
enum Change {
    Key,
    Step,
}

#[test]
fn each_change_sends_one_edit_and_only_its_row_waits_while_navigation_stays_free() {
    let cases = [
        (
            Row::CopyOnSelect,
            Change::Key,
            UserSettingsField::CopyOnSelect,
        ),
        (Row::Sound, Change::Key, UserSettingsField::SoundEnabled),
        (
            Row::RestoreOutput,
            Change::Key,
            UserSettingsField::RestoreTerminalOutput,
        ),
        (
            Row::Scrollback,
            Change::Step,
            UserSettingsField::ScrollbackBytes,
        ),
    ];
    for (row, change, field) in cases {
        let mut editor = open(&snapshot(1, &UserSettingsValues::default(), &[]));
        select(&mut editor, row);
        let token = page(&editor).token();
        let submission = match change {
            Change::Key => editor.edit(Edit::Activate),
            Change::Step => editor.control(token, Control::Adjust { row, forward: true }),
        }
        .unwrap_or_else(|| panic!("{row:?} applies at once"));
        assert_eq!(keys(&submission), (vec![key(field)], Vec::new()), "{row:?}");
        assert_eq!(page(&editor).pending_row(), Some(row), "{row:?}");

        // A second change waits for the first; moving focus does not.
        editor.edit(Edit::Next);
        if page(&editor).selected() == row {
            editor.edit(Edit::Previous);
        }
        draw(&mut editor);
        assert_ne!(page(&editor).selected(), row, "{row:?}");
        assert!(editor.edit(Edit::Activate).is_none(), "{row:?}");
        assert!(editor.in_flight(), "{row:?}");
    }
}

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Applied,
    Refused,
}

#[test]
fn a_refused_edit_puts_its_row_back_to_the_file_and_an_applied_one_shows_the_confirmed_value() {
    for outcome in [Outcome::Applied, Outcome::Refused] {
        let mut editor = open(&snapshot(1, &UserSettingsValues::default(), &[]));
        select(&mut editor, Row::CopyOnSelect);
        let submission = editor.edit(Edit::Activate).unwrap();
        assert!(
            page(&editor).values().copy_on_select,
            "the row shows the request"
        );

        // Another writer's change arrives while the edit is in flight.
        let outside = UserSettingsValues {
            sound_enabled: false,
            ..UserSettingsValues::default()
        };
        let sound = key(UserSettingsField::SoundEnabled);
        editor.observe(&snapshot(2, &outside, &[sound.as_str()]));
        assert!(page(&editor).values().copy_on_select, "{outcome:?}");

        match outcome {
            Outcome::Refused => {
                assert!(editor.failed(submission.token));
                assert!(!page(&editor).values().copy_on_select, "back to the file");
            }
            Outcome::Applied => {
                assert!(editor.edited(submission.token));
                let applied = UserSettingsValues {
                    copy_on_select: true,
                    ..outside.clone()
                };
                editor.observe(&snapshot(
                    3,
                    &applied,
                    &[sound.as_str(), "terminal.copy_on_select"],
                ));
                assert!(page(&editor).values().copy_on_select);
                assert!(page(&editor).changed(Row::CopyOnSelect));
            }
        }
        assert!(!editor.in_flight(), "{outcome:?}");
        assert_eq!(page(&editor).pending_row(), None, "{outcome:?}");
        assert_eq!(
            page(&editor).values().sound_enabled,
            outside.sound_enabled,
            "the outside change shows once the edit settles: {outcome:?}"
        );
    }
}

#[test]
fn restore_sends_one_restore_for_a_changed_row_and_nothing_for_an_unchanged_one() {
    let profiles = format!("terminal.profiles.{}", platform());
    let default_profile = format!("terminal.default_profile.{}", platform());
    let cases: [(Row, Vec<String>); 3] = [
        (
            Row::CopyOnSelect,
            vec![key(UserSettingsField::CopyOnSelect)],
        ),
        (Row::PrefixKey, vec!["keys.prefix".to_owned()]),
        (
            Row::LaunchProfiles,
            vec![profiles.clone(), default_profile.clone()],
        ),
    ];
    for (row, expected) in cases {
        let set: Vec<&str> = expected.iter().map(String::as_str).collect();
        let mut editor = open(&snapshot(1, &UserSettingsValues::default(), &set[..1]));
        select(&mut editor, row);
        assert!(page(&editor).changed(row), "{row:?}");
        let submission = editor.edit(Edit::Delete).expect("a changed row restores");
        let (written, mut restored) = keys(&submission);
        restored.sort();
        let mut expected = expected.clone();
        expected.sort();
        assert!(written.is_empty(), "{row:?}");
        assert_eq!(restored, expected, "{row:?}");
        assert!(editor.failed(submission.token));

        let mut unchanged = open(&snapshot(1, &UserSettingsValues::default(), &[]));
        select(&mut unchanged, row);
        assert!(!page(&unchanged).changed(row), "{row:?}");
        assert!(unchanged.edit(Edit::Delete).is_none(), "{row:?}");
    }
}

#[test]
fn a_narrow_page_shows_one_column_and_escape_steps_back_one_column() {
    for narrow in [false, true] {
        let mut editor = open(&snapshot(1, &UserSettingsValues::default(), &[]));
        let token = page(&editor).token();
        editor.draw_with(token, narrow);
        assert_eq!(page(&editor).pane(), Pane::Categories);
        editor.control(token, Control::Category(Category::Terminal));
        editor.draw_with(token, narrow);
        assert_eq!(page(&editor).category(), Category::Terminal);
        assert_eq!(
            page(&editor).pane(),
            if narrow { Pane::Rows } else { Pane::Categories },
            "a narrow page opens the chosen category's rows"
        );
        editor.edit(Edit::Pane(true));
        editor.draw_with(token, narrow);
        assert_eq!(page(&editor).pane(), Pane::Rows);
        editor.edit(Edit::Cancel);
        if narrow {
            assert_eq!(page(&editor).pane(), Pane::Categories, "the way back");
            assert_eq!(page(&editor).category(), Category::Terminal);
        } else {
            assert!(editor.view().is_none(), "Esc leaves a two-column page");
        }
    }
}

#[test]
fn a_theme_line_sets_its_slot_and_the_appearance_that_shows_it_in_one_edit() {
    for (appearance, slot, shown) in [
        (
            ApplicationAppearance::Light,
            Row::DarkTheme,
            ApplicationAppearance::Dark,
        ),
        (
            ApplicationAppearance::Dark,
            Row::LightTheme,
            ApplicationAppearance::Light,
        ),
    ] {
        let values = UserSettingsValues {
            application_theme: appearance,
            ..UserSettingsValues::default()
        };
        let mut editor = open(&snapshot(1, &values, &[]));
        let theme = Choice::options(slot)
            .iter()
            .find_map(|choice| match choice {
                Choice::ColorTheme(theme) if !theme.current(&values) => Some(*theme),
                _ => None,
            })
            .expect("another shipped theme");
        select(&mut editor, Row::ThemeOption(theme));
        assert!(
            editor
                .appearance_preview()
                .is_some_and(|preview| preview.application_theme == shown),
            "a focused theme line previews at its own polarity"
        );
        let submission = editor.edit(Edit::Activate).unwrap();
        let (written, restored) = keys(&submission);
        assert!(restored.is_empty());
        assert!(written.contains(&key(setting_for_row(slot).unwrap().key)));
        assert!(written.contains(&key(UserSettingsField::ApplicationTheme)));
    }
}

#[test]
fn an_unusable_file_is_named_while_the_page_keeps_the_last_valid_values() {
    let values = UserSettingsValues {
        copy_on_select: true,
        ..UserSettingsValues::default()
    };
    for file in [
        SettingsFile::Unreadable {
            reason: "denied".into(),
        },
        SettingsFile::SyntaxError {
            reason: orchbee_workbench_protocol::SettingsSyntaxReason::InvalidToml,
            line: 3,
            column: 7,
        },
    ] {
        let mut editor = open(&snapshot(1, &values, &[]));
        let mut broken = snapshot(2, &values, &[]);
        broken.file = file.clone();
        assert!(editor.observe(&broken));
        assert!(page(&editor).file_fact().is_some(), "{file:?}");
        assert!(page(&editor).values().copy_on_select, "{file:?}");
        let mut repaired = snapshot(3, &values, &[]);
        repaired.file = SettingsFile::Ok {};
        assert!(editor.observe(&repaired));
        assert!(page(&editor).file_fact().is_none(), "{file:?}");
    }
}
