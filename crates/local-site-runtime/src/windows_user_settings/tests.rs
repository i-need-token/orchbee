use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

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
    fn read(&self, value: Option<OsString>) -> Result<WindowsUserSettings, WindowsSettingsError> {
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
fn strict_sections_types_duplicate_keys_and_private_diagnostics() {
    for (text, field, reason) in [
        ("secret = 'DO-NOT-ECHO'", "file", "unknown-key"),
        (
            "[terminal]\nunknown = 'DO-NOT-ECHO'",
            "terminal.new_cwd",
            "unknown-key",
        ),
        ("terminal = 2", "terminal.new_cwd", "wrong-type"),
        ("[terminal]\nnew_cwd = 5", "terminal.new_cwd", "wrong-type"),
        (
            "[terminal]\nnew_cwd = 'home'\nnew_cwd = 'follow'",
            "file",
            "invalid-toml",
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
        (
            "session = 1",
            "session.resume_agents_on_restore",
            "wrong-type",
        ),
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
            "session.resume_agents_on_restore",
            "unknown-key",
        ),
        (
            "[session]\nresume_agents_on_restore = true\nresume_agents_on_restore = false",
            "file",
            "invalid-toml",
        ),
    ] {
        let error = parse(text, Path::new(r"C:\config\config.toml"), &|_| None).unwrap_err();
        assert_eq!((error.field, error.reason), (field, reason));
        assert!(!format!("{error:?} {error}").contains("DO-NOT-ECHO"));
    }
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
    assert_eq!(
        fixture.read(Some("true".into())).unwrap_err().field,
        "session.resume_agents_on_restore"
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
        Path::new(r"C:\config\..\项目")
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
        let error = fixture.read(None).unwrap_err();
        assert_eq!(error.field, "update.manifest_check");
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
