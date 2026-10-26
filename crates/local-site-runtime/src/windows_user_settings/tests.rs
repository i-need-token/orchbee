use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn sidebar_admission_shares_the_whole_file_owner_with_keys_and_notifications() {
    let read = |text: &str| parse(text, Path::new(r"C:\config.toml"), &|_| None);
    let settings = read("[keys]\nprefix='ctrl+a'\n[ui.toast]\ndelivery='off'\n[ui.sidebar.agents]\nrows=[['$summary']]\nrow_gap=1\n").unwrap();
    assert_eq!(settings.agent_sidebar.row_gap(), 1);
    assert!(
        read("[ui.sidebar.spaces]\nrows=[]")
            .unwrap()
            .space_sidebar
            .rows()
            .is_empty()
    );
    assert_eq!(
        read("[ui.sidebar.spaces]\nrows=[[\"agent\"]]")
            .unwrap()
            .diagnostics
            .items()[0]
            .field,
        "ui.sidebar.spaces"
    );
    assert_eq!(
        settings
            .agent_sidebar
            .rows_for_agent("codex", None)
            .unwrap()[0][0]
            .name(),
        "$summary"
    );
    for text in [
        "[ui.sidebar]\nother=1",
        "[ui.sidebar.agents]\nrows=[['typo']]",
        "[ui]\nsidebar=42",
    ] {
        assert!(!read(text).unwrap().diagnostics.is_empty());
    }
    let admitted = read("[ui.sidebar.agents]\nrows=[['agent']]\n[keys]\nhelp='ctrl+c'").unwrap();
    assert_eq!(admitted.keys.overrides[0].action, "help");
    assert!(admitted.keys.overrides[0].triggers.is_empty());
    assert_eq!(admitted.diagnostics.items()[0].field, "keys.help");
}

#[test]
fn update_channel_is_strict_and_independent_of_manifest_checks() {
    let read = |text: &str| parse(text, Path::new(r"C:\config.toml"), &|_| None);
    for text in ["", "[update]\nmanifest_check=false"] {
        let settings = read(text).unwrap();
        assert_eq!(settings.update_channel, WindowsUpdateChannel::Stable);
        assert_eq!(settings.update_channel_source, "default");
    }
    for channel in ["stable", "preview"] {
        let settings = read(&format!(
            "[update]\nchannel='{channel}'\nmanifest_check=false"
        ))
        .unwrap();
        assert_eq!(settings.update_channel.as_str(), channel);
        assert_eq!(settings.update_channel_source, "file");
        assert!(!settings.manifest_check);
    }
    for (value, reason) in [
        ("true", "wrong-type"),
        ("'nightly'", "invalid-channel"),
        ("'Preview'", "invalid-channel"),
        ("''", "invalid-channel"),
    ] {
        let admitted = read(&format!("[update]\nchannel={value}")).unwrap();
        assert_eq!(admitted.update_channel, WindowsUpdateChannel::Stable);
        assert!(admitted.diagnostics.items().iter().any(|diagnostic| {
            diagnostic.field == "update.channel" && diagnostic.reason == reason
        }));
    }
    let unknown = read("[update]\nchannel='preview'\nunknown=true").unwrap();
    assert_eq!(unknown.update_channel, WindowsUpdateChannel::Preview);
    assert_eq!(unknown.diagnostics.items()[0].field, "update.unknown");
    let invalid = read("[update]\nchannel='preview'\nmanifest_check='true'").unwrap();
    assert_eq!(invalid.update_channel, WindowsUpdateChannel::Preview);
    assert!(invalid.manifest_check);
}

#[test]
fn configured_shell_is_strict_literal_and_keeps_directory_settings() {
    for mode in ["auto", "login", "non_login"] {
        let settings = parse(
            &format!("[terminal]\ndefault_shell=' pwsh.exe '\nshell_mode='{mode}'\nnew_cwd='home'"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        )
        .unwrap();
        assert_eq!(settings.shell.executable(), "pwsh.exe");
        assert_eq!(settings.shell.mode().as_str(), mode);
        assert_eq!(settings.cwd, Some((WindowsDirectoryPolicy::Home, None)));
    }
    for (field, value) in [
        ("default_shell", "42"),
        ("shell_mode", "false"),
        ("shell_mode", "'LOGIN'"),
        ("default_shell", "'C:relative.exe'"),
        ("default_shell", "'./shell.exe'"),
    ] {
        let admitted = parse(
            &format!("[terminal]\n{field}={value}"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        )
        .unwrap();
        assert_eq!(admitted.shell, WindowsShellSettings::default());
        assert_eq!(
            admitted.diagnostics.items()[0].field,
            format!("terminal.{field}")
        );
    }
    assert!(WindowsShellSettings::new(&"x".repeat(4096), "auto").is_ok());
    assert!(WindowsShellSettings::new(&"x".repeat(4097), "auto").is_err());
    for invalid in [
        "a\0b",
        "a\nb",
        r"\\server\share\shell.exe",
        r"\\?\C:\shell.exe",
    ] {
        assert!(
            WindowsShellSettings::new(invalid, "auto").is_err(),
            "{invalid:?}"
        );
    }
}

#[test]
fn private_shell_snapshot_is_complete_and_does_not_consume_other_arguments() {
    let mut args = [
        "--default-shell",
        "C:/shell with spaces/pwsh.exe",
        "--shell-mode",
        "login",
        "remaining",
    ]
    .map(OsString::from)
    .into_iter();
    let shell = WindowsShellSettings::from_private_arguments(&mut args).unwrap();
    assert_eq!(shell.mode(), WindowsShellMode::Login);
    assert!(shell.executable().ends_with("pwsh.exe"));
    assert_eq!(args.next(), Some("remaining".into()));
    for words in [
        vec![],
        vec!["--default-shell", "pwsh"],
        vec!["--shell-mode", "login", "--default-shell", "pwsh"],
        vec!["--default-shell", "", "--shell-mode", "bad"],
    ] {
        assert!(
            WindowsShellSettings::from_private_arguments(
                &mut words.into_iter().map(OsString::from)
            )
            .is_err()
        );
    }
}

#[test]
fn companion_asset_location_reuses_precedence_without_opening_settings() {
    let path = windows_user_settings_path(|key| match key {
        "ORCHBEE_CONFIG_FILE" => Some(OsString::from(
            "C:\\missing-orchbee-configuration\\settings.toml",
        )),
        "APPDATA" => Some(OsString::from("relative-is-ignored")),
        _ => None,
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        path,
        PathBuf::from("C:\\missing-orchbee-configuration\\settings.toml")
    );
    assert!(windows_user_settings_path(|_| None).unwrap().is_none());
    assert!(
        windows_user_settings_path(
            |key| (key == "ORCHBEE_CONFIG_FILE").then(|| OsString::from("relative.toml"))
        )
        .is_err()
    );
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-user-settings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("config.toml")
    }
    fn read(
        &self,
        value: Option<OsString>,
    ) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
        read_windows_user_settings(|key| match key {
            "ORCHBEE_CONFIG_FILE" => Some(self.file().into_os_string()),
            "ORCHBEE_RESUME_AGENTS_ON_RESTORE" => value.clone(),
            _ => None,
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.file().exists() {
            std::fs::remove_file(self.file()).unwrap();
        }
        std::fs::remove_dir(&self.0).unwrap();
    }
}

#[test]
fn parsed_field_failures_warn_without_exposing_values_and_duplicate_toml_is_fatal() {
    for (text, field, reason) in [
        ("secret = 'DO-NOT-ECHO'", "secret", "unknown-key"),
        (
            "[terminal]\nunknown = 'DO-NOT-ECHO'",
            "terminal.unknown",
            "unknown-key",
        ),
        ("terminal = 2", "terminal", "wrong-type"),
        ("[terminal]\nnew_cwd = 5", "terminal.new_cwd", "wrong-type"),
        (
            "[worktrees]\ndirectory = ''",
            "worktrees.directory",
            "invalid-path",
        ),
        (
            "[worktrees]\ndirectory = 'C:relative'",
            "worktrees.directory",
            "invalid-path",
        ),
        ("session = 1", "session", "wrong-type"),
        (
            "[session]\nresume_agents_on_restore = 'false'",
            "session.resume_agents_on_restore",
            "wrong-type",
        ),
        (
            "[session]\nresume_agents_on_restore = 0",
            "session.resume_agents_on_restore",
            "wrong-type",
        ),
        (
            "[session]\nunknown = false",
            "session.unknown",
            "unknown-key",
        ),
    ] {
        let admitted = parse(text, Path::new(r"C:\config\config.toml"), &|_| None).unwrap();
        assert!(
            admitted
                .diagnostics
                .items()
                .iter()
                .any(|diagnostic| { diagnostic.field == field && diagnostic.reason == reason })
        );
        assert!(!format!("{:?}", admitted.diagnostics).contains("DO-NOT-ECHO"));
    }
    let error = parse(
        "[session]\nresume_agents_on_restore = true\nresume_agents_on_restore = false",
        Path::new(r"C:\config\config.toml"),
        &|_| None,
    )
    .unwrap_err();
    assert_eq!((error.field, error.reason), ("file", "invalid-toml"));
}

#[test]
fn defaults_file_and_environment_precedence_are_complete() {
    let absent = read_windows_user_settings(|_| None).unwrap();
    assert!(absent.resume_agents_on_restore);
    assert_eq!(absent.resume_agents_on_restore_source, "default");
    let fixture = Fixture::new();
    for (body, value, source) in [
        ("", true, "default"),
        ("[session]", true, "default"),
        ("[session]\nresume_agents_on_restore = false", false, "file"),
        ("[session]\nresume_agents_on_restore = true", true, "file"),
    ] {
        std::fs::write(fixture.file(), body).unwrap();
        let settings = fixture.read(None).unwrap();
        assert_eq!(
            (
                settings.resume_agents_on_restore,
                settings.resume_agents_on_restore_source
            ),
            (value, source)
        );
        for (override_value, expected) in [("true", true), ("false", false)] {
            let settings = fixture.read(Some(override_value.into())).unwrap();
            assert_eq!(
                (
                    settings.resume_agents_on_restore,
                    settings.resume_agents_on_restore_source
                ),
                (expected, "environment")
            );
        }
    }
    for value in ["", "1", "FALSE", " false "] {
        assert_eq!(
            fixture.read(Some(value.into())).unwrap_err(),
            WindowsSettingsError::new("ORCHBEE_RESUME_AGENTS_ON_RESTORE", "invalid-boolean")
        );
    }
    use std::os::windows::ffi::OsStringExt;
    assert_eq!(
        fixture
            .read(Some(OsString::from_wide(&[0xd800])))
            .unwrap_err()
            .reason,
        "invalid-boolean"
    );
    std::fs::write(
        fixture.file(),
        "[session]\nresume_agents_on_restore='secret'",
    )
    .unwrap();
    let admitted = fixture.read(Some("true".into())).unwrap();
    assert!(admitted.resume_agents_on_restore);
    assert_eq!(admitted.resume_agents_on_restore_source, "environment");
    assert_eq!(
        admitted.diagnostics.items()[0].field,
        "session.resume_agents_on_restore"
    );

    std::fs::write(fixture.file(), "[session]\nresume_agents_on_restore=true").unwrap();
    let reloaded = read_windows_user_settings_at(Some(&fixture.file()), true, |name| {
        (name == "ORCHBEE_RESUME_AGENTS_ON_RESTORE").then(|| "false".into())
    })
    .unwrap();
    assert!(!reloaded.resume_agents_on_restore);
    assert_eq!(reloaded.resume_agents_on_restore_source, "environment");
    assert_eq!(
        read_windows_user_settings_at(Some(&fixture.file()), true, |name| {
            (name == "ORCHBEE_RESUME_AGENTS_ON_RESTORE").then(|| "invalid".into())
        })
        .unwrap_err(),
        WindowsSettingsError::new("ORCHBEE_RESUME_AGENTS_ON_RESTORE", "invalid-boolean")
    );
}

#[test]
fn file_bounds_missing_locations_and_no_creation_are_preserved() {
    let fixture = Fixture::new();
    let settings = read_windows_user_settings(|key| {
        (key == "APPDATA").then(|| fixture.0.clone().into_os_string())
    })
    .unwrap();
    assert!(!settings.exists);
    assert_eq!(settings.path, Some(fixture.0.join("Orchbee/config.toml")));
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    assert_eq!(fixture.read(None).unwrap_err().reason, "missing");
    std::fs::write(fixture.file(), vec![b' '; LIMIT as usize]).unwrap();
    assert!(fixture.read(None).is_ok());
    std::fs::write(fixture.file(), vec![b' '; LIMIT as usize + 1]).unwrap();
    assert_eq!(fixture.read(None).unwrap_err().reason, "too-large");
    std::fs::write(fixture.file(), [0xff]).unwrap();
    assert_eq!(fixture.read(None).unwrap_err().reason, "invalid-utf8");
    std::fs::remove_file(fixture.file()).unwrap();
    std::fs::create_dir(fixture.file()).unwrap();
    assert_eq!(fixture.read(None).unwrap_err().reason, "not-file");
    std::fs::remove_dir(fixture.file()).unwrap();
    for value in ["", "relative", "C:relative", r"\\server\share\config.toml"] {
        assert_eq!(
            location(&|name| (name == "ORCHBEE_CONFIG_FILE").then(|| value.into()))
                .unwrap_err()
                .field,
            "ORCHBEE_CONFIG_FILE"
        );
    }
}

#[test]
fn paths_keep_file_parent_home_and_literal_operand_semantics() {
    let settings = parse(
        "[terminal]\nnew_cwd = '../项目'\n[worktrees]\ndirectory = '~/worktrees'",
        Path::new(r"C:\config\config.toml"),
        &|key| (key == "USERPROFILE").then(|| r"C:\home".into()),
    )
    .unwrap();
    assert_eq!(
        Path::new(settings.cwd.as_ref().unwrap().1.as_ref().unwrap()),
        Path::new(r"C:\项目")
    );
    assert_eq!(
        Path::new(settings.worktree_root.as_ref().unwrap()),
        Path::new(r"C:\home\worktrees")
    );
    for (value, policy) in [
        ("", WindowsDirectoryPolicy::Follow),
        ("follow", WindowsDirectoryPolicy::Follow),
        ("home", WindowsDirectoryPolicy::Home),
        ("current", WindowsDirectoryPolicy::Current),
        ("workspace", WindowsDirectoryPolicy::Workspace),
    ] {
        let settings = parse(
            &format!("[terminal]\nnew_cwd='{value}'"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        )
        .unwrap();
        assert_eq!(settings.cwd, Some((policy, None)));
    }
}

#[test]
fn manifest_check_is_strict_defaulted_and_reload_keeps_the_frozen_location() {
    let fixture = Fixture::new();
    std::fs::write(fixture.file(), "[update]\nmanifest_check=false\n").unwrap();
    let initial = fixture.read(None).unwrap();
    assert!(!initial.manifest_check);
    assert_eq!(initial.manifest_check_source, "file");
    assert!(initial.explicit);
    std::fs::write(fixture.file(), "[update]\nmanifest_check=true\n").unwrap();
    let next =
        read_windows_user_settings_at(initial.path.as_deref(), initial.explicit, |_| None).unwrap();
    assert!(next.manifest_check);
    assert_eq!(next.manifest_check_source, "file");
    for text in ["[update]\nmanifest_check='false'", "[update]\nunknown=true"] {
        std::fs::write(fixture.file(), text).unwrap();
        let admitted = fixture.read(None).unwrap();
        assert!(admitted.manifest_check);
        assert!(!admitted.diagnostics.is_empty());
    }
    std::fs::remove_file(fixture.file()).unwrap();
    assert_eq!(
        read_windows_user_settings_at(initial.path.as_deref(), true, |_| None)
            .unwrap_err()
            .reason,
        "missing"
    );
    let default = read_windows_user_settings_at(initial.path.as_deref(), false, |_| None).unwrap();
    assert!(default.manifest_check);
    assert_eq!(default.manifest_check_source, "default");
}

#[test]
fn window_title_settings_default_empty_invalid_syntax_and_strict_type() {
    assert_eq!(
        WindowsUserSettings::default().window_title,
        "{hostname}: {workspace}"
    );
    for title in ["", "{workspace}/{tab}"] {
        let settings = parse(
            &format!("[ui]\nwindow_title='{title}'\n[update]\nmanifest_check=false"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        )
        .unwrap();
        assert_eq!(settings.window_title, title);
        assert!(!settings.manifest_check);
    }
    let invalid = parse(
        "[ui]\nwindow_title='{unknown}'",
        Path::new(r"C:\config.toml"),
        &|_| None,
    )
    .unwrap();
    assert_eq!(invalid.window_title, DEFAULT_WINDOW_TITLE);
    assert_eq!(invalid.diagnostics.items()[0].reason, "unknown-token");
    for text in ["[ui]\nwindow_title=2", "[ui]\nunknown='x'"] {
        assert!(
            !parse(text, Path::new(r"C:\config.toml"), &|_| None)
                .unwrap()
                .diagnostics
                .is_empty()
        );
    }
}

#[test]
fn unknown_fields_and_partial_key_failures_are_bounded_warnings() {
    let admitted = parse(
        "onboarding=false\n[server]\naddress='DO-NOT-ECHO'\n[advanced]\n[experimental]\nenabled=true\n[remote]\nenabled=true\n[theme]\nname='one-light'\ncustom={}\n[terminal]\nkitty_graphics=true\n[update]\nversion_check=false\n[keys]\nhelp=['prefix+?', 'ctrl+c', 'f1']\nnew_tab='prefix+n'\nnavigate_pane_left='h'\nindexed=true\n[ui]\nsidebar_width=24\nunknown_presentation=true\n[ui.toast]\nenabled=true\nclipboard=true\n[ui.sound]\ndone_path='done.mp3'",
        Path::new(r"C:\config.toml"),
        &|_| None,
    )
    .unwrap();
    assert_eq!(admitted.settings.theme, WindowsThemeName::TerminalNative);
    assert!(
        admitted
            .diagnostics
            .items()
            .iter()
            .all(|diagnostic| { diagnostic.field != "theme.name" })
    );
    for field in [
        "onboarding",
        "server",
        "advanced",
        "experimental",
        "remote",
        "theme",
        "terminal.kitty_graphics",
        "update.version_check",
        "keys.indexed",
        "keys.navigate_pane_left",
        "ui.sidebar_width",
        "ui.sound.done_path",
        "ui.toast.enabled",
        "ui.toast.clipboard",
        "ui.unknown_presentation",
    ] {
        assert!(
            admitted.diagnostics.items().iter().any(|diagnostic| {
                diagnostic.field == field
                    && diagnostic.reason == "unknown-key"
                    && diagnostic.disposition == WindowsSettingsDiagnosticDisposition::Ignored
            }),
            "{field}"
        );
    }
    assert_eq!(
        admitted.notifications.delivery,
        WindowsNotificationDelivery::Off
    );
    let help = admitted
        .keys
        .overrides
        .iter()
        .find(|override_| override_.action == "help")
        .unwrap();
    assert_eq!(help.triggers.len(), 2);
    assert!(
        admitted
            .keys
            .overrides
            .iter()
            .any(|override_| { override_.action == "new_tab" && override_.triggers.len() == 1 })
    );
}

#[test]
fn invalid_explicit_key_action_stays_explicitly_disabled() {
    for body in ["help=[]", "help=''", "help=42", "help=['ctrl+c']"] {
        let admitted = parse(
            &format!("[keys]\n{body}\nnew_tab='prefix+n'"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        )
        .unwrap();
        let help = admitted
            .keys
            .overrides
            .iter()
            .find(|override_| override_.action == "help")
            .unwrap();
        assert!(help.triggers.is_empty());
        assert!(
            admitted.keys.overrides.iter().any(|override_| {
                override_.action == "new_tab" && override_.triggers.len() == 1
            })
        );
        assert!(
            admitted
                .diagnostics
                .items()
                .iter()
                .any(|diagnostic| diagnostic.field == "keys.help")
        );
    }
}

#[test]
fn explicit_key_conflicts_use_registry_order_instead_of_toml_order() {
    for body in [
        "help='prefix+q'\nnew_tab='prefix+q'",
        "new_tab='prefix+q'\nhelp='prefix+q'",
    ] {
        let admitted = parse(
            &format!("[keys]\n{body}"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        )
        .unwrap();
        let help = admitted
            .keys
            .overrides
            .iter()
            .find(|setting| setting.action == "help")
            .unwrap();
        let new_tab = admitted
            .keys
            .overrides
            .iter()
            .find(|setting| setting.action == "new_tab")
            .unwrap();
        assert_eq!(help.triggers.len(), 1);
        assert!(new_tab.triggers.is_empty());
        assert!(admitted.diagnostics.items().iter().any(|diagnostic| {
            diagnostic.field == "keys.new_tab"
                && diagnostic.reason == "duplicate-binding"
                && diagnostic.disposition == WindowsSettingsDiagnosticDisposition::Disabled
        }));
    }
}

#[test]
fn every_registered_action_can_be_explicitly_disabled() {
    let text = format!(
        "[keys]\n{}",
        keys::ACTIONS
            .iter()
            .map(|action| format!("{action}=[]"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let admitted = parse(&text, Path::new(r"C:\config.toml"), &|_| None).unwrap();
    assert_eq!(keys::ACTIONS.len(), 37);
    assert_eq!(admitted.keys.overrides.len(), keys::ACTIONS.len());
    assert!(
        admitted
            .keys
            .overrides
            .iter()
            .all(|setting| setting.triggers.is_empty())
    );
}

#[test]
fn invalid_explicit_notification_delivery_defaults_off() {
    let admitted = parse(
        "[ui.toast]\ndelivery='invalid'",
        Path::new(r"C:\config.toml"),
        &|_| None,
    )
    .unwrap();
    assert_eq!(
        admitted.notifications.delivery,
        WindowsNotificationDelivery::Off
    );
    assert!(admitted.diagnostics.items().iter().any(|diagnostic| {
        diagnostic.field == "ui.toast.delivery"
            && diagnostic.disposition == WindowsSettingsDiagnosticDisposition::Defaulted
    }));
}

#[test]
fn warning_record_is_sorted_bounded_and_reports_omissions() {
    let text: String = (0..140)
        .map(|index| format!("unknown_{index:03}=true\n"))
        .collect();
    let admitted = parse(&text, Path::new(r"C:\config.toml"), &|_| None).unwrap();
    assert_eq!(
        admitted.diagnostics.items().len(),
        MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS
    );
    assert_eq!(admitted.diagnostics.omitted_count(), 12);
    assert!(admitted.diagnostics.items().windows(2).all(|pair| (
        pair[0].field.as_str(),
        pair[0].reason
    ) <= (
        pair[1].field.as_str(),
        pair[1].reason
    )));
}

#[test]
fn empty_unknown_key_has_a_nonempty_canonical_diagnostic_field() {
    let admitted = parse("\"\" = true", Path::new(r"C:\config.toml"), &|_| None).unwrap();
    assert_eq!(admitted.diagnostics.items().len(), 1);
    assert_eq!(admitted.diagnostics.items()[0].field, "\\u{empty}");
    assert_eq!(admitted.diagnostics.items()[0].reason, "unknown-key");
}

#[test]
fn printed_default_configuration_roundtrips_without_freezing_action_defaults() {
    let text = default_windows_user_settings_toml();
    assert!(text.ends_with('\n'));
    assert!(!text.ends_with("\n\n"));
    let admitted = parse(&text, Path::new(r"C:\config.toml"), &|_| None).unwrap();
    let defaults = WindowsUserSettings::default();
    assert!(admitted.diagnostics.is_empty());
    assert_eq!(admitted.keys, defaults.keys);
    assert_eq!(admitted.shell, defaults.shell);
    assert_eq!(admitted.notifications, defaults.notifications);
    assert_eq!(admitted.agent_sidebar, defaults.agent_sidebar);
    assert_eq!(admitted.space_sidebar, defaults.space_sidebar);
    assert_eq!(admitted.window_title, defaults.window_title);
    assert_eq!(admitted.update_channel, defaults.update_channel);
    assert_eq!(admitted.update_channel_source, "file");
    assert_eq!(admitted.manifest_check, defaults.manifest_check);
    assert_eq!(admitted.manifest_check_source, "file");
    assert_eq!(
        admitted.resume_agents_on_restore,
        defaults.resume_agents_on_restore
    );
    assert_eq!(admitted.resume_agents_on_restore_source, "file");
    assert_eq!(admitted.cwd, None);
    assert_eq!(admitted.worktree_root, None);
    assert_eq!(admitted.path, Some(PathBuf::from(r"C:\config.toml")));
    assert!(admitted.exists);
    assert!(!admitted.explicit);
}

#[test]
fn default_configuration_reference_covers_the_setting_and_action_registries() {
    let actions = WindowsKeySettings::registered_actions()
        .iter()
        .map(|&action| WindowsDefaultActionBindings {
            action,
            bindings: Vec::new(),
        })
        .collect::<Vec<_>>();
    let text = default_windows_user_settings_toml_with_actions(&actions);
    for option in WINDOWS_USER_CONFIG_OPTIONS {
        let path_line = format!("# {}", option.path);
        assert!(
            text.lines().any(|line| line == path_line.as_str()),
            "{} is missing from the generated reference",
            option.path
        );
        assert!(text.contains(&format!("#   Default: {}", option.default)));
        let parts = option.path.split('.').collect::<Vec<_>>();
        for index in 0..parts.len() - 1 {
            let table = parts[..index + 1].join(".");
            assert!(windows_user_config_has_child(&table, parts[index + 1]));
        }
    }
    for action in WindowsKeySettings::registered_actions() {
        let action_line = format!("# {action} = []");
        assert!(text.lines().any(|line| line == action_line.as_str()));
    }
    let admitted = parse(&text, Path::new(r"C:\config.toml"), &|_| None).unwrap();
    assert!(admitted.diagnostics.is_empty());
    assert_eq!(admitted.keys, WindowsUserSettings::default().keys);
}

#[test]
fn default_configuration_quotes_and_roundtrips_binding_strings() {
    let text = default_windows_user_settings_toml_with_actions(&[WindowsDefaultActionBindings {
        action: "help",
        bindings: vec!["prefix+\"".to_owned(), "prefix+\\".to_owned()],
    }]);
    let action_line = text
        .lines()
        .find(|line| line.starts_with("# help = "))
        .unwrap();
    let config = text.replace(action_line, action_line.strip_prefix("# ").unwrap());
    let admitted = parse(&config, Path::new(r"C:\config.toml"), &|_| None).unwrap();
    assert!(admitted.diagnostics.is_empty());
    let settings = &admitted.keys;
    let help = settings
        .overrides
        .iter()
        .find(|override_| override_.action == "help")
        .unwrap();
    assert_eq!(
        help.triggers
            .iter()
            .map(|trigger| trigger.chord.key.as_str())
            .collect::<Vec<_>>(),
        vec!["\"", "\\"]
    );
    assert!(help.triggers.iter().all(|trigger| trigger.prefix));
    let roundtripped = WindowsKeySettings::from_profile(&settings.profile()).unwrap();
    assert_eq!(&roundtripped, settings);
}

#[test]
fn split_focus_defaults_true_and_admits_only_booleans() {
    for (value, expected, valid) in [
        ("true", true, true),
        ("false", false, true),
        ("'false'", true, false),
    ] {
        let load = parse(
            &format!("[terminal]\nfocus_new_panel={value}"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        )
        .unwrap();
        assert_eq!(load.focus_new_panel, expected);
        assert_eq!(load.diagnostics.is_empty(), valid);
    }
    assert!(WindowsUserSettings::default().focus_new_panel);
}
