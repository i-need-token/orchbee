// The editor's rules are the evidence settings pages and scripts rely on: only the named keys
// change and all of them or none do, the rest of the person's text survives, a broken file is never
// written, a concurrent edit is not lost, and links and held files are treated truthfully.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = crate::test_temporary_directory().join(format!(
            "orchbee-settings-editor-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn root(&self) -> PathBuf {
        self.0.join("root")
    }

    fn file(&self) -> PathBuf {
        self.root().join("config.toml")
    }

    fn env(&self) -> impl Fn(&str) -> Option<OsString> + '_ {
        |name| (name == "ORCHBEE_HOME").then(|| self.root().into_os_string())
    }

    fn write(&self, text: &str) {
        fs::create_dir_all(self.root()).unwrap();
        fs::write(self.file(), text).unwrap();
    }

    fn text(&self) -> String {
        fs::read_to_string(self.file()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::metadata(self.file()) {
            let mut permissions = metadata.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = fs::set_permissions(self.file(), permissions);
        }
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn string(value: &str) -> toml::Value {
    toml::Value::String(value.into())
}

fn key(path: &str) -> Vec<String> {
    path.split('.').map(str::to_owned).collect()
}

fn set(path: &str, value: toml::Value) -> SettingChange {
    SettingChange::Set {
        key: key(path),
        value,
    }
}

fn changes(changes: Vec<SettingChange>) -> SettingChanges {
    SettingChanges::new(changes).unwrap()
}

/// This platform's launch-profile keys and a profile array whose program path is absolute here.
fn profiles(id: &str) -> (String, String, toml::Value) {
    let platform = super::super::registry::current_platform();
    let executable = if cfg!(windows) {
        r"C:\\Program Files\\PowerShell\\7\\pwsh.exe"
    } else {
        "/usr/bin/pwsh"
    };
    let value = toml::from_str::<toml::Table>(&format!(
        "p = [{{ id = '{id}', kind = 'custom-executable', executable = \"{executable}\", arguments = ['-NoLogo'] }}]"
    ))
    .unwrap()["p"]
        .clone();
    (
        format!("terminal.profiles.{platform}"),
        format!("terminal.default_profile.{platform}"),
        value,
    )
}

#[test]
fn set_changes_only_its_key_and_keeps_comments_order_and_unknown_keys() {
    let fixture = Fixture::new();
    let original = "# my settings\n[terminal]\n# keep this\nfocus_new_panel = true # inline note\nnew_cwd = 'home'\n\n[mystery]\nanswer = 42\n";
    fixture.write(original);
    set_setting(
        &["terminal", "focus_new_panel"],
        &toml::Value::Boolean(false),
        fixture.env(),
    )
    .unwrap();
    assert_eq!(
        fixture.text(),
        original.replace("focus_new_panel = true", "focus_new_panel = false")
    );
    set_setting(&["appearance", "mode"], &string("dark"), fixture.env()).unwrap();
    let text = fixture.text();
    assert!(text.starts_with(&original.replace("= true", "= false")));
    assert!(text.ends_with("[appearance]\nmode = \"dark\"\n"), "{text}");
    let load = crate::read_user_settings(fixture.env()).unwrap();
    assert!(!load.focus_new_panel);
    assert!(
        load.diagnostics
            .items()
            .iter()
            .any(|diagnostic| diagnostic.field == "mystery")
    );
    assert!(!fixture.root().join(PENDING_NAME).exists());

    // Several changes, a whole launch-profile array among them, land in one write and leave the
    // keys they do not name as they were.
    let (table, default, value) = profiles("pwsh");
    let edit = changes(vec![
        set("terminal.copy_on_select", toml::Value::Boolean(true)),
        SettingChange::Restore {
            key: key("appearance.mode"),
        },
        set(&table, value),
        set(&default, string("pwsh")),
    ]);
    assert_eq!(edit_settings(&edit, fixture.env()), Ok(true));
    let text = fixture.text();
    assert!(
        text.starts_with(
            "# my settings\n[terminal]\n# keep this\nfocus_new_panel = false # inline note\nnew_cwd = 'home'\n"
        ),
        "{text}"
    );
    assert!(text.contains("\n[mystery]\nanswer = 42\n"), "{text}");
    assert!(!text.contains("[appearance]"), "{text}");
    let load = crate::read_user_settings(fixture.env()).unwrap();
    assert!(load.values.copy_on_select);
    assert_eq!(load.values.launch_profiles.default_profile_id, "pwsh");
    assert_eq!(
        load.diagnostics
            .items()
            .iter()
            .map(|diagnostic| diagnostic.field.as_str())
            .collect::<Vec<_>>(),
        ["mystery"]
    );
    assert_eq!(edit_settings(&edit, fixture.env()), Ok(false));
    assert_eq!(fixture.text(), text);
    assert_eq!(
        SettingChanges::new(vec![
            set("terminal.padding", toml::Value::Integer(1)),
            set("terminal.padding", toml::Value::Integer(2)),
        ]),
        None
    );
    assert_eq!(SettingChanges::new(Vec::new()), None);
}

#[test]
fn restore_removes_the_key_and_only_tables_left_empty_without_comments() {
    let fixture = Fixture::new();
    fixture.write(
        "[appearance]\nmode = 'dark'\n\n# notification choices\n[notifications]\nsound = true\n\n[terminal]\ncopy_on_select = true\nfocus_new_panel = false\n",
    );
    restore_setting_default(&["appearance", "mode"], fixture.env()).unwrap();
    restore_setting_default(&["notifications", "sound"], fixture.env()).unwrap();
    restore_setting_default(&["terminal", "copy_on_select"], fixture.env()).unwrap();
    let text = fixture.text();
    let table: toml::Table = text.parse().unwrap();
    assert!(!table.contains_key("appearance"), "{text}");
    assert!(table["notifications"].as_table().unwrap().is_empty());
    assert!(text.contains("# notification choices\n[notifications]"));
    assert_eq!(
        table["terminal"]
            .as_table()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["focus_new_panel"]
    );
    let before = fixture.text();
    restore_setting_default(&["general", "language"], fixture.env()).unwrap();
    assert_eq!(fixture.text(), before);
}

#[test]
fn a_missing_file_is_created_with_only_the_key() {
    let fixture = Fixture::new();
    restore_setting_default(&["terminal", "padding"], fixture.env()).unwrap();
    assert!(!fixture.file().exists());
    set_setting(
        &["notifications", "sound_by_agent", "codex.cli"],
        &string("off"),
        fixture.env(),
    )
    .unwrap();
    assert_eq!(
        fixture.text(),
        "[notifications.sound_by_agent]\n\"codex.cli\" = \"off\"\n"
    );
}

#[test]
fn refusals_leave_the_file_untouched() {
    let fixture = Fixture::new();
    let broken = "[terminal]\nfocus_new_panel = true\nfocus_new_panel = false\n";
    fixture.write(broken);
    let refused = set_setting(&["appearance", "mode"], &string("dark"), fixture.env());
    assert!(matches!(
        refused,
        Err(SettingsEditError::Syntax(SettingsSyntaxError {
            reason: "invalid-toml",
            line: 3,
            ..
        }))
    ));
    assert_eq!(fixture.text(), broken);
    fixture.write("[appearance]\nmode = 'light'\n");
    for (key, value, reason) in [
        (&["appearance", "mode"][..], string("dim"), "invalid-mode"),
        (
            &["terminal", "font_size"][..],
            toml::Value::Integer(4),
            "invalid-value",
        ),
        (
            &["keys", "help"][..],
            string("not a chord"),
            "invalid-binding",
        ),
    ] {
        let refused = set_setting(key, &value, fixture.env()).unwrap_err();
        assert_eq!(refused.reason(), reason, "{key:?}");
    }
    assert_eq!(
        set_setting(&["ui", "window_title"], &string("x"), fixture.env()),
        Err(SettingsEditError::UnknownKey {
            key: "ui.window_title".into()
        })
    );
    assert_eq!(
        set_setting(&["keys", "command"], &string("x"), fixture.env()).map(drop),
        Err(SettingsEditError::InvalidValue {
            key: "keys.command".into(),
            field: "keys.command".into(),
            reason: "wrong-type",
        })
    );
    // One refused change refuses the whole edit, and names the change it is charged to.
    let valid = || set("appearance.mode", string("dark"));
    for (refused, error) in [
        (
            set("terminal.font_size", toml::Value::Integer(4)),
            SettingsEditError::InvalidValue {
                key: "terminal.font_size".into(),
                field: "terminal.font_size".into(),
                reason: "invalid-value",
            },
        ),
        (
            set("ui.window_title", string("x")),
            SettingsEditError::UnknownKey {
                key: "ui.window_title".into(),
            },
        ),
    ] {
        assert_eq!(
            edit_settings(&changes(vec![valid(), refused]), fixture.env()),
            Err(error)
        );
    }
    let (table, default, value) = profiles("pwsh");
    let entries = toml::Value::Array(vec![value.as_array().unwrap()[0].clone(); 65]);
    assert_eq!(
        edit_settings(&changes(vec![set(&table, entries)]), fixture.env())
            .map_err(|error| error.reason()),
        Err("value-too-long")
    );
    // A profile this platform cannot start is refused at edit as the reader refuses it at read.
    let mut relative = value.clone();
    relative.as_array_mut().unwrap()[0]
        .as_table_mut()
        .unwrap()
        .insert("executable".into(), string("pwsh"));
    assert_eq!(
        edit_settings(&changes(vec![set(&table, relative)]), fixture.env()),
        Err(SettingsEditError::InvalidValue {
            key: table.clone(),
            field: table.clone(),
            reason: "invalid-value",
        })
    );
    assert_eq!(fixture.text(), "[appearance]\nmode = 'light'\n");
    // A change may not leave another key the reader then refuses, such as a default profile that
    // names a removed profile.
    edit_settings(
        &changes(vec![set(&table, value), set(&default, string("pwsh"))]),
        fixture.env(),
    )
    .unwrap();
    let with_profiles = fixture.text();
    assert_eq!(
        restore_setting_default(&table.split('.').collect::<Vec<_>>(), fixture.env()),
        Err(SettingsEditError::InvalidValue {
            key: table.clone(),
            field: default.clone(),
            reason: "unknown-profile",
        })
    );
    assert_eq!(fixture.text(), with_profiles);
    fixture.write("[appearance]\nmode = 'light'\n");
    let mut permissions = fs::metadata(fixture.file()).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(fixture.file(), permissions).unwrap();
    assert_eq!(
        set_setting(&["appearance", "mode"], &string("dark"), fixture.env()),
        Err(SettingsEditError::ReadOnly)
    );
    assert_eq!(fixture.text(), "[appearance]\nmode = 'light'\n");
}

#[test]
fn an_external_edit_during_the_write_is_kept_and_the_edit_redone() {
    let fixture = Fixture::new();
    fixture.write("[terminal]\nfocus_new_panel = true\n");
    let file = fixture.file();
    let mut reads = 0;
    let mut read = |target: &Path| {
        reads += 1;
        if reads == 2 {
            fs::write(
                target,
                "[terminal]\nfocus_new_panel = true\ncopy_on_select = true\n",
            )
            .unwrap();
        }
        read_target(target)
    };
    assert_eq!(
        edit_locked(
            &[set("appearance.mode", string("dark"))],
            &file,
            &file,
            &fixture.env(),
            &mut read,
        ),
        Ok(true)
    );
    assert_eq!(
        fixture.text(),
        "[terminal]\nfocus_new_panel = true\ncopy_on_select = true\n\n[appearance]\nmode = \"dark\"\n"
    );
    let mut writes = 0;
    let mut always_changing = |target: &Path| {
        writes += 1;
        fs::write(target, format!("# edit {writes}\n")).unwrap();
        read_target(target)
    };
    assert_eq!(
        edit_locked(
            &[set("appearance.mode", string("light"))],
            &file,
            &file,
            &fixture.env(),
            &mut always_changing,
        ),
        Err(SettingsEditError::Changed)
    );
}

#[test]
fn a_symbolic_link_keeps_pointing_at_the_replaced_target() {
    let fixture = Fixture::new();
    let target = fixture.0.join("dotfiles").join("orchbee.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, "# synced\n").unwrap();
    let link = fixture.0.join("link.toml");
    if let Err(error) = symlink_file(&target, &link) {
        // Creating symbolic links needs Developer Mode or elevation on Windows.
        eprintln!("symbolic link not created ({error}); the link rule is unverified here");
        return;
    }
    let env = |name: &str| match name {
        "ORCHBEE_HOME" => Some(fixture.root().into_os_string()),
        "ORCHBEE_CONFIG_FILE" => Some(link.clone().into_os_string()),
        _ => None,
    };
    set_setting(
        &["terminal", "copy_on_select"],
        &toml::Value::Boolean(true),
        env,
    )
    .unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "# synced\n\n[terminal]\ncopy_on_select = true\n"
    );
}

#[cfg(windows)]
fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

// Only Windows refuses to replace a file another program holds open.
#[cfg(windows)]
#[test]
fn a_file_held_open_is_retried_then_reported() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    fixture.write("[terminal]\nfocus_new_panel = true\n");
    let hold = || {
        OpenOptions::new()
            .read(true)
            .share_mode(1) // FILE_SHARE_READ: renaming over the file is refused while open.
            .open(fixture.file())
            .unwrap()
    };
    let held = hold();
    let released = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        drop(held);
    });
    set_setting(&["appearance", "mode"], &string("dark"), fixture.env()).unwrap();
    released.join().unwrap();
    assert!(fixture.text().contains("mode = \"dark\""));
    let _held = hold();
    assert_eq!(
        set_setting(&["appearance", "mode"], &string("light"), fixture.env()),
        Err(SettingsEditError::InUse)
    );
}
