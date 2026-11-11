use super::*;
use crate::NotificationDelivery;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn sidebar_admission_shares_the_whole_file_owner_with_keys_and_notifications() {
    let read = |text: &str| parse(text, Path::new(r"C:\config.toml"), &|_| None);
    let settings = read(
        "[keys]\nprefix='ctrl+a'\n[notifications]\ncli_delivery='off'\n[appearance.sidebar.agents]\nrows=[['$summary']]\nrow_gap=1\n",
    );
    assert_eq!(settings.agent_sidebar.row_gap(), 1);
    assert!(
        read("[appearance.sidebar.workspaces]\nrows=[]")
            .space_sidebar
            .rows()
            .is_empty()
    );
    assert_eq!(
        read("[appearance.sidebar.workspaces]\nrows=[[\"agent\"]]")
            .diagnostics
            .items()[0]
            .field,
        "appearance.sidebar.workspaces"
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
        "[appearance.sidebar]\nother=1",
        "[appearance.sidebar.agents]\nrows=[['typo']]",
        "[appearance]\nsidebar=42",
    ] {
        assert!(!read(text).diagnostics.is_empty());
    }
    let admitted = read("[appearance.sidebar.agents]\nrows=[['agent']]\n[keys]\nhelp='ctrl+c'");
    assert_eq!(admitted.keys.overrides[0].action, "help");
    assert!(admitted.keys.overrides[0].triggers.is_empty());
    assert_eq!(admitted.diagnostics.items()[0].field, "keys.help");
}

#[test]
fn update_channel_is_strict_and_independent_of_manifest_checks() {
    let read = |text: &str| parse(text, Path::new(r"C:\config.toml"), &|_| None);
    for text in ["", "[agent_detection]\nauto_update=false"] {
        let settings = read(text);
        assert_eq!(settings.update_channel, UpdateChannel::Stable);
        assert_eq!(settings.update_channel_source, "default");
    }
    for channel in ["stable", "preview"] {
        let settings = read(&format!(
            "[update]\nchannel='{channel}'\n[agent_detection]\nauto_update=false"
        ));
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
        let admitted = read(&format!("[update]\nchannel={value}"));
        assert_eq!(admitted.update_channel, UpdateChannel::Stable);
        assert!(admitted.diagnostics.items().iter().any(|diagnostic| {
            diagnostic.field == "update.channel" && diagnostic.reason == reason
        }));
    }
    let unknown = read("[update]\nchannel='preview'\nunknown=true");
    assert_eq!(unknown.update_channel, UpdateChannel::Preview);
    assert_eq!(unknown.diagnostics.items()[0].field, "update.unknown");
    let invalid = read("[update]\nchannel='preview'\n[agent_detection]\nauto_update='true'");
    assert_eq!(invalid.update_channel, UpdateChannel::Preview);
    assert!(invalid.manifest_check);
}

#[test]
fn retired_shell_keys_are_ignored_and_keep_directory_settings() {
    let settings = parse(
        "[terminal]\ndefault_shell='pwsh.exe'\nshell_mode='login'\nnew_cwd='home'",
        Path::new(r"C:\config.toml"),
        &|_| None,
    );
    assert_eq!(settings.cwd, Some((DirectoryPolicy::Home, None)));
    for field in ["terminal.default_shell", "terminal.shell_mode"] {
        assert!(
            settings
                .diagnostics
                .items()
                .iter()
                .any(|item| item.field == field
                    && item.reason == "unknown-key"
                    && item.disposition == SettingsDiagnosticDisposition::Ignored)
        );
    }
}

#[cfg(windows)]
#[test]
fn companion_asset_location_reuses_precedence_without_opening_settings() {
    let path = user_settings_path(|key| match key {
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
    assert!(
        user_settings_path(
            |key| (key == "ORCHBEE_CONFIG_FILE").then(|| OsString::from("relative.toml"))
        )
        .is_err()
    );
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = crate::test_temporary_directory().join(format!(
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
    fn read(&self) -> Result<UserSettingsLoad, SettingsError> {
        read_user_settings(|key| match key {
            "ORCHBEE_CONFIG_FILE" => Some(self.file().into_os_string()),
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
fn parsed_field_failures_warn_without_exposing_values() {
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
            "[terminal]\nnew_cwd = '\\\\?\\C:'",
            "terminal.new_cwd",
            "invalid-path",
        ),
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
        ("[update]\nunknown = false", "update.unknown", "unknown-key"),
    ] {
        let admitted = parse(text, Path::new(r"C:\config\config.toml"), &|_| None);
        assert!(
            admitted
                .diagnostics
                .items()
                .iter()
                .any(|diagnostic| { diagnostic.field == field && diagnostic.reason == reason })
        );
        assert!(!format!("{:?}", admitted.diagnostics).contains("DO-NOT-ECHO"));
    }
}

#[test]
fn file_bounds_missing_locations_and_no_creation_are_preserved() {
    let fixture = Fixture::new();
    let environment = |key: &str| {
        (key == crate::DataRoot::home_variable()).then(|| fixture.0.clone().into_os_string())
    };
    let settings = read_user_settings(environment).unwrap();
    assert!(!settings.exists);
    assert_eq!(
        settings.path,
        Some(crate::DataRoot::resolve(environment).unwrap().config_file())
    );
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    assert_eq!(fixture.read().unwrap_err().reason, "missing");
    std::fs::write(fixture.file(), vec![b' '; LIMIT as usize]).unwrap();
    assert!(fixture.read().is_ok());
    std::fs::write(fixture.file(), vec![b' '; LIMIT as usize + 1]).unwrap();
    assert_eq!(fixture.read().unwrap_err().reason, "too-large");
    std::fs::remove_file(fixture.file()).unwrap();
    std::fs::create_dir(fixture.file()).unwrap();
    assert_eq!(fixture.read().unwrap_err().reason, "not-file");
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
    );
    assert_eq!(
        Path::new(settings.cwd.as_ref().unwrap().1.as_ref().unwrap()),
        Path::new(r"C:\项目")
    );
    assert_eq!(
        Path::new(settings.worktree_root.as_ref().unwrap()),
        Path::new(r"C:\home\worktrees")
    );
    for (value, policy) in [
        ("", DirectoryPolicy::Follow),
        ("follow", DirectoryPolicy::Follow),
        ("home", DirectoryPolicy::Home),
        ("current", DirectoryPolicy::Current),
        ("workspace", DirectoryPolicy::Workspace),
    ] {
        let settings = parse(
            &format!("[terminal]\nnew_cwd='{value}'"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        );
        assert_eq!(settings.cwd, Some((policy, None)));
    }
}

#[test]
fn manifest_check_is_strict_defaulted_and_reload_keeps_the_frozen_location() {
    let fixture = Fixture::new();
    std::fs::write(fixture.file(), "[agent_detection]\nauto_update=false\n").unwrap();
    let initial = fixture.read().unwrap();
    assert!(!initial.manifest_check);
    assert_eq!(initial.manifest_check_source, "file");
    assert!(initial.explicit);
    std::fs::write(fixture.file(), "[agent_detection]\nauto_update=true\n").unwrap();
    let next = read_user_settings_at(initial.path.as_deref(), initial.explicit, |_| None).unwrap();
    assert!(next.manifest_check);
    assert_eq!(next.manifest_check_source, "file");
    for text in [
        "[agent_detection]\nauto_update='false'",
        "[agent_detection]\nunknown=true",
    ] {
        std::fs::write(fixture.file(), text).unwrap();
        let admitted = fixture.read().unwrap();
        assert!(admitted.manifest_check);
        assert!(!admitted.diagnostics.is_empty());
    }
    std::fs::remove_file(fixture.file()).unwrap();
    assert_eq!(
        read_user_settings_at(initial.path.as_deref(), true, |_| None)
            .unwrap_err()
            .reason,
        "missing"
    );
    let default = read_user_settings_at(initial.path.as_deref(), false, |_| None).unwrap();
    assert!(default.manifest_check);
    assert_eq!(default.manifest_check_source, "default");
}

#[test]
fn window_title_settings_default_empty_invalid_syntax_and_strict_type() {
    assert_eq!(
        UserSettings::default().window_title,
        "{hostname}: {workspace}"
    );
    for title in ["", "{workspace}/{tab}"] {
        let settings = parse(
            &format!("[appearance]\nwindow_title='{title}'\n[agent_detection]\nauto_update=false"),
            Path::new(r"C:\config.toml"),
            &|_| None,
        );
        assert_eq!(settings.window_title, title);
        assert!(!settings.manifest_check);
    }
    let invalid = parse(
        "[appearance]\nwindow_title='{unknown}'",
        Path::new(r"C:\config.toml"),
        &|_| None,
    );
    assert_eq!(invalid.window_title, DEFAULT_WINDOW_TITLE);
    assert_eq!(invalid.diagnostics.items()[0].reason, "unknown-token");
    for text in ["[appearance]\nwindow_title=2", "[appearance]\nunknown='x'"] {
        assert!(
            !parse(text, Path::new(r"C:\config.toml"), &|_| None)
                .diagnostics
                .is_empty()
        );
    }
}

#[test]
fn unknown_fields_and_partial_key_failures_are_bounded_warnings() {
    let admitted = parse(
        "onboarding=false\n[server]\naddress='DO-NOT-ECHO'\n[advanced]\n[experimental]\nenabled=true\n[remote]\nenabled=true\n[theme]\nname='one-light'\ncustom={}\n[terminal]\nkitty_graphics=true\n[update]\nversion_check=false\n[keys]\nhelp=['prefix+?', 'ctrl+c', 'f1']\nnew_tab='prefix+n'\nnavigate_pane_left='h'\nindexed=true\n[appearance]\nsidebar_width=24\nunknown_presentation=true\n[notifications]\nenabled=true\nclipboard=true\ndone_path='done.mp3'\n[ui]\nwindow_title='{workspace}'",
        Path::new(r"C:\config.toml"),
        &|_| None,
    );
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
        "appearance.sidebar_width",
        "notifications.done_path",
        "notifications.enabled",
        "notifications.clipboard",
        "appearance.unknown_presentation",
        "ui",
    ] {
        assert!(
            admitted.diagnostics.items().iter().any(|diagnostic| {
                diagnostic.field == field
                    && diagnostic.reason == "unknown-key"
                    && diagnostic.disposition == SettingsDiagnosticDisposition::Ignored
            }),
            "{field}"
        );
    }
    assert_eq!(admitted.notifications.delivery, NotificationDelivery::Off);
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
        );
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
        );
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
                && diagnostic.disposition == SettingsDiagnosticDisposition::Disabled
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
    let admitted = parse(&text, Path::new(r"C:\config.toml"), &|_| None);
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
        "[notifications]\ncli_delivery='invalid'",
        Path::new(r"C:\config.toml"),
        &|_| None,
    );
    assert_eq!(admitted.notifications.delivery, NotificationDelivery::Off);
    assert!(admitted.diagnostics.items().iter().any(|diagnostic| {
        diagnostic.field == "notifications.cli_delivery"
            && diagnostic.disposition == SettingsDiagnosticDisposition::Defaulted
    }));
}

#[test]
fn warning_record_is_sorted_bounded_and_reports_omissions() {
    let text: String = (0..140)
        .map(|index| format!("unknown_{index:03}=true\n"))
        .collect();
    let admitted = parse(&text, Path::new(r"C:\config.toml"), &|_| None);
    assert_eq!(
        admitted.diagnostics.items().len(),
        MAXIMUM_SETTINGS_DIAGNOSTICS
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
    let admitted = parse("\"\" = true", Path::new(r"C:\config.toml"), &|_| None);
    assert_eq!(admitted.diagnostics.items().len(), 1);
    assert_eq!(admitted.diagnostics.items()[0].field, "\\u{empty}");
    assert_eq!(admitted.diagnostics.items()[0].reason, "unknown-key");
}

#[test]
fn printed_default_configuration_roundtrips_without_freezing_action_defaults() {
    let text = default_user_settings_toml();
    assert!(text.ends_with('\n'));
    assert!(!text.ends_with("\n\n"));
    let admitted = parse(&text, Path::new(r"C:\config.toml"), &|_| None);
    let defaults = UserSettings::default();
    assert!(admitted.diagnostics.is_empty());
    assert_eq!(admitted.keys, defaults.keys);
    assert_eq!(admitted.notifications, defaults.notifications);
    assert_eq!(admitted.agent_sidebar, defaults.agent_sidebar);
    assert_eq!(admitted.space_sidebar, defaults.space_sidebar);
    assert_eq!(admitted.window_title, defaults.window_title);
    assert_eq!(admitted.update_channel, defaults.update_channel);
    assert_eq!(admitted.manifest_check, defaults.manifest_check);
    assert_eq!(admitted.cwd, None);
    assert_eq!(admitted.worktree_root, None);
    assert_eq!(admitted.path, Some(PathBuf::from(r"C:\config.toml")));
    assert!(admitted.exists);
    assert!(!admitted.explicit);
}

#[test]
fn default_configuration_reference_covers_the_setting_and_action_registries() {
    let actions = KeySettings::registered_actions()
        .iter()
        .map(|&action| DefaultActionBindings {
            action,
            bindings: Vec::new(),
        })
        .collect::<Vec<_>>();
    let text = default_user_settings_toml_with_actions(&actions);
    assert!(
        text.lines()
            .all(|line| line.is_empty() || line.starts_with('#')),
        "the reference holds only comments"
    );
    assert!(toml::from_str::<toml::Table>(&text).unwrap().is_empty());
    for setting in SETTING_KEYS {
        let described = text.contains(setting.description.split_whitespace().next().unwrap());
        assert_eq!(described, setting.reference, "{}", setting.path);
        let leaf = setting.path.rsplit('.').next().unwrap();
        if setting.reference && !leaf.starts_with(['<', '*']) {
            assert!(
                text.lines()
                    .any(|line| line.starts_with(&format!("# {leaf} = "))
                        || line.contains(&format!(
                            "{}]",
                            setting.path.replace("<platform>", "windows")
                        ))),
                "{} has no line in the reference",
                setting.path
            );
        }
    }
    for action in KeySettings::registered_actions() {
        let action_line = format!("# {action} = []");
        assert!(text.lines().any(|line| line == action_line.as_str()));
    }
    let uncommented: String = text
        .lines()
        .filter_map(|line| line.strip_prefix("# "))
        .filter(|line| {
            line.starts_with('[')
                || line.split_once(" = ").is_some_and(|(key, _)| {
                    !key.is_empty()
                        && key
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
                })
        })
        .map(|line| format!("{line}\n"))
        .collect();
    let admitted = parse(&uncommented, Path::new(r"C:\config.toml"), &|_| None);
    assert_eq!(admitted.syntax, None, "{uncommented}");
}

#[test]
fn default_configuration_quotes_and_roundtrips_binding_strings() {
    let text = default_user_settings_toml_with_actions(&[DefaultActionBindings {
        action: "help",
        bindings: vec!["prefix+\"".to_owned(), "prefix+\\".to_owned()],
    }]);
    let action_line = text
        .lines()
        .find(|line| line.starts_with("# help = "))
        .unwrap();
    let config = text
        .replace(action_line, action_line.strip_prefix("# ").unwrap())
        .replace("\n# [keys]\n", "\n[keys]\n");
    let admitted = parse(&config, Path::new(r"C:\config.toml"), &|_| None);
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
    let roundtripped = KeySettings::from_profile(&settings.profile()).unwrap();
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
        );
        assert_eq!(load.focus_new_panel, expected);
        assert_eq!(load.diagnostics.is_empty(), valid);
    }
    assert!(UserSettings::default().focus_new_panel);
}

// Startup no longer refuses a broken file: the defaults apply and the error names where it is.
#[test]
fn syntax_errors_leave_defaults_and_name_the_line_and_column() {
    let path = Path::new(r"C:\config.toml");
    let broken = parse(
        "[terminal]\nfocus_new_panel = false\n[terminal]\nnew_cwd = 'home'\n",
        path,
        &|_| None,
    );
    let syntax = broken.syntax.unwrap();
    assert_eq!((syntax.reason, syntax.line), ("invalid-toml", 3));
    assert!((1..=2).contains(&syntax.column), "{syntax:?}");
    assert!(broken.focus_new_panel);
    assert_eq!(broken.cwd, None);
    assert!(broken.exists);
    assert!(broken.diagnostics.items().iter().any(|diagnostic| {
        diagnostic.field == "file"
            && diagnostic.reason == "invalid-toml"
            && diagnostic.disposition == SettingsDiagnosticDisposition::Defaulted
    }));
    let last_valid = UserSettings {
        focus_new_panel: false,
        ..UserSettings::default()
    };
    let kept = broken.clone().or_last_valid(&last_valid);
    assert!(!kept.focus_new_panel);
    assert_eq!(kept.path.as_deref(), Some(path));
    assert_eq!(
        broken.require_valid_syntax().unwrap_err(),
        SettingsError::new("file", "invalid-toml")
    );
    let mut bytes = "[terminal]\n  new_cwd = 'é".as_bytes().to_vec();
    bytes.push(0xff);
    let syntax = parse_bytes(&bytes, path, &|_| None).syntax.unwrap();
    assert_eq!(
        (syntax.reason, syntax.line, syntax.column),
        ("invalid-utf8", 2, 15)
    );
    let fine = parse("[terminal]\nfocus_new_panel = false\n", path, &|_| None);
    assert_eq!(
        fine.clone().or_last_valid(&UserSettings::default()),
        fine.clone()
    );
    assert!(fine.require_valid_syntax().is_ok());
}

// The common preferences and desktop keys parse into the model under their new names, with the
// file's spellings, while every old `ui.*` name is just an unknown key.
#[test]
fn preference_and_desktop_keys_parse_with_registry_types() {
    use orchbee_workbench_protocol::{AgentReminderMode, CompletionReminderMode};
    let load = parse(
        "[general]\nlanguage = 'zh-CN'\n[appearance]\nmode = 'dark'\ndark_theme = 'dracula'\ncustom_foreground = '#ABCDEF'\n[terminal]\ncopy_on_select = true\nscrollback_bytes = 2000000000\nfont_size = 20\nfont_weight = 450\nmacos_option_as_alt = 'left'\n[notifications]\nagent_reminder = 'when-hidden'\ncompletion_reminder = 'hidden'\ncompletion_minimum_seconds = 30\n[agents]\nresume_on_restore = false\n[ui.toast]\ndelivery = 'system'\n",
        Path::new(r"C:\config.toml"),
        &|_| None,
    );
    let preferences = &load.preferences;
    assert_eq!(
        serde_json::to_value(preferences.locale).unwrap(),
        serde_json::json!("zh-CN")
    );
    assert_eq!(preferences.dark_theme, "dracula");
    assert!(preferences.copy_on_select);
    assert!(!preferences.resume_agents_on_restore);
    assert_eq!(
        preferences.agent_notification_mode,
        AgentReminderMode::Hidden
    );
    assert_eq!(
        preferences.completion_reminder_mode,
        CompletionReminderMode::Never
    );
    assert_eq!(preferences.completion_reminder_minimum_seconds, 30);
    assert_eq!(preferences.custom_foreground, "#141414");
    assert_eq!(preferences.scrollback_bytes, 10_000_000);
    assert_eq!(load.desktop.font_size, 20);
    assert_eq!(load.desktop.font_weight, 400);
    assert_eq!(load.desktop.macos_option_as_alt, MacosOptionAsAlt::Left);
    assert_eq!(load.notifications.delivery, NotificationDelivery::Off);
    for (field, reason) in [
        ("appearance.custom_foreground", "invalid-value"),
        ("terminal.scrollback_bytes", "invalid-value"),
        ("terminal.font_weight", "invalid-value"),
        ("notifications.completion_reminder", "invalid-mode"),
        ("ui", "unknown-key"),
    ] {
        assert!(
            load.diagnostics
                .items()
                .iter()
                .any(|diagnostic| diagnostic.field == field && diagnostic.reason == reason),
            "{field}: {reason}"
        );
    }
}

// Launch profiles: each platform reads only its own table, the built-in profile stays, and a
// mistake in another platform's table still shows.
#[test]
fn launch_profiles_read_the_current_platform_and_check_every_table() {
    let platform = registry::current_platform();
    let other = SETTING_PLATFORMS
        .into_iter()
        .find(|candidate| *candidate != platform)
        .unwrap();
    let executable = if platform == "windows" {
        r"C:\Program Files\PowerShell\7\pwsh.exe"
    } else {
        "/usr/local/bin/pwsh"
    };
    let login = other != "windows";
    let load = parse(
        &format!(
            "[terminal.default_profile]\n{platform} = 'pwsh'\n{other} = 'missing'\n[[terminal.profiles.{platform}]]\nid = 'pwsh'\nkind = 'custom-executable'\nexecutable = '{executable}'\narguments = ['-NoLogo']\nicon = 'x'\n[[terminal.profiles.{other}]]\nid = 'zsh'\nkind = 'system-shell'\nlogin = {login}\n"
        ),
        Path::new(r"C:\config.toml"),
        &|_| None,
    );
    let catalog = &load.preferences.launch_profiles;
    assert_eq!(catalog.default_profile_id, "pwsh");
    assert_eq!(
        catalog
            .profiles
            .iter()
            .map(|profile| profile.id.as_str())
            .collect::<Vec<_>>(),
        ["system-default", "pwsh"]
    );
    assert_eq!(catalog.profiles[1].name.as_deref(), Some("pwsh"));
    let fields: Vec<_> = load
        .diagnostics
        .items()
        .iter()
        .map(|diagnostic| (diagnostic.field.as_str(), diagnostic.reason))
        .collect();
    assert!(fields.contains(&(
        format!("terminal.default_profile.{other}").as_str(),
        "unknown-profile"
    )));
    assert!(fields.contains(&(
        format!("terminal.profiles.{platform}[].icon").as_str(),
        "unknown-key"
    )));
    let broken = parse(
        &format!(
            "[[terminal.profiles.{platform}]]\nid = 'x'\nkind = 'system-shell'\nexecutable = 'sh'\n"
        ),
        Path::new(r"C:\config.toml"),
        &|_| None,
    );
    assert_eq!(
        broken.preferences.launch_profiles,
        orchbee_workbench_protocol::UserLaunchProfiles::default()
    );
    assert!(broken.diagnostics.items().iter().any(|diagnostic| {
        diagnostic.field == format!("terminal.profiles.{platform}")
            && diagnostic.reason == "invalid-value"
    }));
    // Each table is judged by its own platform's launch rules on every host: a custom executable
    // must be absolute there, and Windows has no login shell.
    for (platform, executable, admitted) in [
        ("windows", r"C:\Tools\shell.exe", true),
        ("windows", r"\\server\share\shell.exe", true),
        ("windows", "/usr/bin/zsh", false),
        ("windows", "shell.exe", false),
        ("macos", "/bin/zsh", true),
        ("macos", r"C:\Tools\shell.exe", false),
        ("linux", "/usr/bin/bash", true),
        ("linux", "bash", false),
    ] {
        let load = parse(
            &format!(
                "[[terminal.profiles.{platform}]]\nid = 'x'\nkind = 'custom-executable'\nexecutable = '{executable}'\n"
            ),
            Path::new(r"C:\config.toml"),
            &|_| None,
        );
        assert_eq!(
            load.diagnostics.is_empty(),
            admitted,
            "{platform} {executable}"
        );
    }
    for (platform, admitted) in [("windows", false), ("macos", true), ("linux", true)] {
        let load = parse(
            &format!(
                "[[terminal.profiles.{platform}]]\nid = 'x'\nkind = 'system-shell'\nlogin = true\n"
            ),
            Path::new(r"C:\config.toml"),
            &|_| None,
        );
        assert_eq!(load.diagnostics.is_empty(), admitted, "{platform} login");
    }
}
