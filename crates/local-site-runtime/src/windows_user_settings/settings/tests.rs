//! Decision cases for the three-field edit and the observed-source write boundary.
use super::*;
use std::{fs, path::PathBuf};

fn changes(theme: WindowsThemeName) -> WindowsNativeSettingsChanges {
    WindowsNativeSettingsChanges {
        theme: Some(theme),
        background_reminders: Some(false),
        sound_enabled: Some(true),
    }
}

#[test]
fn three_field_edits_preserve_uncontrolled_values_and_comments_in_each_table_form() {
    let sources = [
        "# 项目\n[theme] # header\nname = 'terminal-native' # choice\nextra = ['keep', '中文']\n[ui.toast]\ndelivery = 'system'\nagent_delivery = 'terminal' # channel\ndelay_seconds = 17\n[ui.sound]\nenabled = false # mute\n[ui.sound.agents]\ncodex = 'off'\n[terminal]\ndefault_shell = ''\n",
        "# inline\ntheme = {name = 'terminal-native', extra = '保留'}\nui = {toast = {agent_delivery = 'terminal', delay_seconds = 17}, sound = {enabled = false, agents = {codex = 'off'}}}\n",
        "# dotted\ntheme.name = 'terminal-native' # choice\ntheme.extra = '保留'\nui.toast.agent_delivery = 'terminal'\nui.toast.delay_seconds = 17\nui.sound.enabled = false\nui.sound.agents.codex = 'off'\n",
        "# missing owned sections\n[unknown]\nvalue = '中文' # untouched\n",
    ];
    for text in sources {
        for theme in [
            WindowsThemeName::TerminalNative,
            WindowsThemeName::OneLight,
            WindowsThemeName::OneDark,
        ] {
            let before = parse(text, Path::new("C:/example/config.toml"), &|_| None).unwrap();
            let after = document::edit(text, changes(theme), &before.settings).unwrap();
            let load = parse(&after, Path::new("C:/example/config.toml"), &|_| None).unwrap();
            assert_eq!(load.settings.theme, theme);
            assert_eq!(
                load.settings.notifications.agent_delivery,
                WindowsNotificationDelivery::Off
            );
            assert!(load.settings.notifications.sound_enabled);
            assert_eq!(
                load.settings.notifications.delay_seconds,
                before.settings.notifications.delay_seconds
            );
            assert_eq!(
                load.settings.notifications.delivery,
                before.settings.notifications.delivery
            );
            assert_eq!(
                load.settings.notifications.agent_sounds(),
                before.settings.notifications.agent_sounds()
            );
            for line in text.lines().filter(|line| {
                line.trim_start().starts_with('#')
                    || line.contains("# choice")
                    || line.contains("# channel")
                    || line.contains("# mute")
                    || line.contains("# untouched")
                    || line.contains("# header")
            }) {
                let comment = line.split_once('#').unwrap().1;
                assert!(
                    after.contains(&format!("#{comment}")),
                    "lost comment in {after}"
                );
            }
            assert_eq!(
                document::edit(&after, changes(theme), &load.settings).unwrap(),
                after
            );
        }
    }
}

#[test]
fn enabling_reminders_preserves_an_enabled_channel_and_only_changes_dirty_leaves() {
    for channel in [
        WindowsNotificationDelivery::Off,
        WindowsNotificationDelivery::Tui,
        WindowsNotificationDelivery::Terminal,
        WindowsNotificationDelivery::System,
    ] {
        let text = format!(
            "[ui.toast]\nagent_delivery = '{}'\ndelivery = 'system'\n[ui.sound]\nenabled = false\n",
            channel.as_str()
        );
        let load = parse(&text, Path::new("C:/example/config.toml"), &|_| None).unwrap();
        let after = document::edit(
            &text,
            WindowsNativeSettingsChanges {
                background_reminders: Some(true),
                ..Default::default()
            },
            &load.settings,
        )
        .unwrap();
        let next = parse(&after, Path::new("C:/example/config.toml"), &|_| None).unwrap();
        assert_eq!(
            next.settings.notifications.agent_delivery,
            if channel == WindowsNotificationDelivery::Off {
                WindowsNotificationDelivery::Tui
            } else {
                channel
            }
        );
        assert!(!next.settings.notifications.sound_enabled);
        assert_eq!(
            next.settings.notifications.delivery,
            WindowsNotificationDelivery::System
        );
    }
    let text = "ui = 17 # malformed unrelated section\n";
    let load = parse(text, Path::new("C:/example/config.toml"), &|_| None).unwrap();
    let after = document::edit(
        text,
        WindowsNativeSettingsChanges {
            theme: Some(WindowsThemeName::OneDark),
            ..Default::default()
        },
        &load.settings,
    )
    .unwrap();
    assert!(after.starts_with(text));
    assert!(document::edit(text, changes(WindowsThemeName::OneDark), &load.settings).is_err());
    assert!(document::edit("", Default::default(), &WindowsUserSettings::default()).is_err());
}

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Self(std::env::temp_dir().join(format!("orchbee-native-settings-{nonce:032x}")));
        fs::create_dir(&root.0).unwrap();
        root
    }
    fn path(&self) -> PathBuf {
        self.0.join("private").join("项目配置.toml")
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let actual = self.0.canonicalize().unwrap();
        assert_eq!(
            actual.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            actual
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-native-settings-")
        );
        fs::remove_dir_all(actual).unwrap();
    }
}

#[test]
fn observation_is_read_only_and_absence_differs_from_empty_content() {
    let root = Root::new();
    let path = root.path();
    let missing = read_windows_native_settings(&path, false, |_| None).unwrap();
    assert_eq!(missing.source.as_str(), "missing");
    assert!(!path.parent().unwrap().exists());
    assert!(read_windows_native_settings(&path, true, |_| None).is_err());
    drop(WindowsPrivateCache::acquire(path.parent().unwrap()).unwrap());
    fs::write(&path, "").unwrap();
    let empty = read_windows_native_settings(&path, false, |_| None).unwrap();
    assert_ne!(empty.source, missing.source);
    assert_eq!(empty.source.as_str().len(), 64);
    assert!(empty.load.settings.exists);
}

#[test]
fn explicit_commit_consumes_the_candidate_and_refuses_observed_external_changes() {
    let root = Root::new();
    let path = root.path();
    let observed = read_windows_native_settings(&path, false, |_| None).unwrap();
    let prepared = prepare_windows_native_settings(
        &path,
        false,
        &observed.source,
        changes(WindowsThemeName::OneLight),
        |_| None,
    )
    .unwrap();
    assert!(!path.exists());
    assert_eq!(
        prepared.candidate().settings.theme,
        WindowsThemeName::OneLight
    );
    assert!(prepared.commit().unwrap());
    let saved = fs::read(&path).unwrap();
    let observed = read_windows_native_settings(&path, false, |_| None).unwrap();
    let prepared = prepare_windows_native_settings(
        &path,
        false,
        &observed.source,
        changes(WindowsThemeName::OneDark),
        |_| None,
    )
    .unwrap();
    let external = [saved, b"\n# editor changed the file\n".to_vec()].concat();
    fs::write(&path, &external).unwrap();
    assert_eq!(
        prepared.commit(),
        Err(WindowsNativeSettingsFailure::SourceChanged)
    );
    assert_eq!(fs::read(&path).unwrap(), external);
    assert!(matches!(
        prepare_windows_native_settings(
            &path,
            false,
            &observed.source,
            changes(WindowsThemeName::OneDark),
            |_| None
        ),
        Err(WindowsNativeSettingsFailure::SourceChanged)
    ));
}

#[test]
fn unsafe_alias_and_invalid_source_are_preserved_without_repair() {
    let root = Root::new();
    let path = root.path();
    drop(WindowsPrivateCache::acquire(path.parent().unwrap()).unwrap());
    for bytes in [b"[broken".as_slice(), b"\xff", &[b'x'; 65537]] {
        fs::write(&path, bytes).unwrap();
        assert!(read_windows_native_settings(&path, false, |_| None).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::write(&path, "# retained\n").unwrap();
    let observed = read_windows_native_settings(&path, false, |_| None).unwrap();
    fs::hard_link(&path, path.with_extension("alias")).unwrap();
    assert!(
        prepare_windows_native_settings(
            &path,
            false,
            &observed.source,
            changes(WindowsThemeName::OneDark),
            |_| None
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"# retained\n");
}
