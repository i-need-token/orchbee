//! Shared Windows user settings: bounded strict reads and explicitly invoked repair.
mod keys;
mod notifications;
pub use notifications::{WindowsNotificationDelivery, WindowsNotificationSettings};
mod reset_keys;
mod shell;
mod window_title;
use crate::windows_user_path::{WINDOWS_USER_PATH_LIMIT, normalize_windows_user_path};
pub use keys::{WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings, WindowsKeyTrigger};
pub use reset_keys::{WindowsKeysResetOutcome, WindowsKeysResetReport, reset_windows_keybindings};
pub use shell::{WindowsShellMode, WindowsShellSettings};
use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
pub use window_title::{DEFAULT_WINDOW_TITLE, WindowsWindowTitleTemplate};

const LIMIT: u64 = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsUserSettings {
    pub window_title: String,
    pub notifications: WindowsNotificationSettings,
    pub keys: WindowsKeySettings,
    pub shell: WindowsShellSettings,
    pub path: Option<PathBuf>,
    pub exists: bool,
    pub explicit: bool,
    pub manifest_check: bool,
    pub manifest_check_source: &'static str,
    pub cwd: Option<(WindowsDirectoryPolicy, Option<String>)>,
    pub worktree_root: Option<String>,
    pub resume_agents_on_restore: bool,
    pub resume_agents_on_restore_source: &'static str,
}

impl Default for WindowsUserSettings {
    fn default() -> Self {
        Self {
            window_title: DEFAULT_WINDOW_TITLE.into(),
            notifications: WindowsNotificationSettings::default(),
            keys: WindowsKeySettings::default(),
            shell: WindowsShellSettings::default(),
            path: None,
            exists: false,
            explicit: false,
            manifest_check: true,
            manifest_check_source: "default",
            cwd: None,
            worktree_root: None,
            resume_agents_on_restore: true,
            resume_agents_on_restore_source: "default",
        }
    }
}

/// Resolve once; an environment override never bypasses full-file validation.
pub fn read_windows_user_settings(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettings, WindowsSettingsError> {
    let mut settings = read_file(&env)?;
    if let Some(value) = env("ORCHBEE_RESUME_AGENTS_ON_RESTORE") {
        settings.resume_agents_on_restore = match value.to_str() {
            Some("true") => true,
            Some("false") => false,
            _ => {
                return Err(WindowsSettingsError::new(
                    "ORCHBEE_RESUME_AGENTS_ON_RESTORE",
                    "invalid-boolean",
                ));
            }
        };
        settings.resume_agents_on_restore_source = "environment";
    }
    Ok(settings)
}

#[derive(Debug, Eq, PartialEq)]
pub struct WindowsSettingsError {
    pub field: &'static str,
    pub reason: &'static str,
}
impl WindowsSettingsError {
    pub const fn new(field: &'static str, reason: &'static str) -> Self {
        Self { field, reason }
    }
}

impl std::fmt::Display for WindowsSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid Windows configuration: {} ({})",
            self.field, self.reason
        )
    }
}
impl std::error::Error for WindowsSettingsError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsDirectoryPolicy {
    Follow,
    Home,
    Current,
    Workspace,
    Explicit,
}

fn location(
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<Option<(PathBuf, bool)>, WindowsSettingsError> {
    let (value, field, explicit) = if let Some(value) = env("ORCHBEE_CONFIG_FILE") {
        (value, "ORCHBEE_CONFIG_FILE", true)
    } else if let Some(value) = env("APPDATA") {
        (value, "APPDATA", false)
    } else {
        return Ok(None);
    };
    let raw = value
        .to_str()
        .ok_or(WindowsSettingsError::new(field, "invalid-path"))?;
    let path = Path::new(raw);
    if !path.is_absolute() || raw.is_empty() {
        return Err(WindowsSettingsError::new(field, "invalid-path"));
    }
    let checked =
        normalize_windows_user_path(raw, env, || Err(std::io::ErrorKind::NotFound.into()))
            .map_err(|_| WindowsSettingsError::new(field, "invalid-path"))?;
    let path = PathBuf::from(checked.as_str());
    let path = if explicit {
        path
    } else {
        path.join("Orchbee/config.toml")
    };
    if path
        .to_str()
        .is_none_or(|value| value.len() > WINDOWS_USER_PATH_LIMIT)
    {
        return Err(WindowsSettingsError::new(field, "invalid-path"));
    }
    Ok(Some((path, explicit)))
}

/// The shared settings location without opening the file or applying runtime settings.
/// Companion user assets use this parent directory; absence remains explicit.
pub fn windows_user_settings_path(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<Option<PathBuf>, WindowsSettingsError> {
    location(&env).map(|location| location.map(|(path, _)| path))
}

fn read_file(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettings, WindowsSettingsError> {
    let location = location(&env)?;
    read_windows_user_settings_at(
        location.as_ref().map(|(path, _)| path.as_path()),
        location.as_ref().is_some_and(|(_, explicit)| *explicit),
        env,
    )
}
/// Explicit reload keeps the admitted startup path, including default-file absence semantics.
pub fn read_windows_user_settings_at(
    path: Option<&Path>,
    explicit: bool,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettings, WindowsSettingsError> {
    let Some(path) = path else {
        if explicit {
            return Err(WindowsSettingsError::new("file", "invalid-path"));
        }
        return Ok(WindowsUserSettings::default());
    };
    let path = path.to_owned();
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !explicit => {
            return Ok(WindowsUserSettings {
                path: Some(path),
                ..WindowsUserSettings::default()
            });
        }
        Err(error) => {
            return Err(WindowsSettingsError::new(
                "file",
                if error.kind() == std::io::ErrorKind::NotFound {
                    "missing"
                } else {
                    "io-error"
                },
            ));
        }
    };
    let metadata = file
        .metadata()
        .map_err(|_| WindowsSettingsError::new("file", "io-error"))?;
    if !metadata.is_file() {
        return Err(WindowsSettingsError::new("file", "not-file"));
    }
    if metadata.len() > LIMIT {
        return Err(WindowsSettingsError::new("file", "too-large"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WindowsSettingsError::new("file", "io-error"))?;
    if bytes.len() as u64 > LIMIT {
        return Err(WindowsSettingsError::new("file", "too-large"));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| WindowsSettingsError::new("file", "invalid-utf8"))?;
    let mut settings = parse(text, &path, &env)?;
    settings.explicit = explicit;
    Ok(settings)
}

fn parse(
    text: &str,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettings, WindowsSettingsError> {
    let table: toml::Table =
        toml::from_str(text).map_err(|_| WindowsSettingsError::new("file", "invalid-toml"))?;
    let mut settings = WindowsUserSettings {
        path: Some(path.to_owned()),
        exists: true,
        ..WindowsUserSettings::default()
    };
    for (section, value) in table {
        if section == "ui" {
            let table = value
                .as_table()
                .ok_or(WindowsSettingsError::new("ui.window_title", "wrong-type"))?;
            if table
                .keys()
                .any(|key| !matches!(key.as_str(), "window_title" | "toast"))
            {
                return Err(WindowsSettingsError::new("ui.window_title", "unknown-key"));
            }
            if let Some(value) = table.get("toast") {
                settings.notifications = WindowsNotificationSettings::parse(value)?;
            }
            if let Some(value) = table.get("window_title") {
                let value = value
                    .as_str()
                    .ok_or(WindowsSettingsError::new("ui.window_title", "wrong-type"))?;
                if value.len() > 4096 {
                    return Err(WindowsSettingsError::new(
                        "ui.window_title",
                        "value-too-long",
                    ));
                }
                settings.window_title = value.into();
            }
            continue;
        }
        if section == "keys" {
            settings.keys = WindowsKeySettings::parse(&value)?;
            continue;
        }
        let (key, field) = match section.as_str() {
            "terminal" => ("new_cwd", "terminal.new_cwd"),
            "update" => ("manifest_check", "update.manifest_check"),
            "worktrees" => ("directory", "worktrees.directory"),
            "session" => (
                "resume_agents_on_restore",
                "session.resume_agents_on_restore",
            ),
            _ => return Err(WindowsSettingsError::new("file", "unknown-key")),
        };
        let table = value
            .as_table()
            .ok_or(WindowsSettingsError::new(field, "wrong-type"))?;
        if table.keys().any(|entry| {
            entry != key
                && !(section == "terminal"
                    && matches!(entry.as_str(), "default_shell" | "shell_mode"))
        }) {
            return Err(WindowsSettingsError::new(field, "unknown-key"));
        }
        if section == "terminal" {
            let shell_value = |key, field, default| {
                table.get(key).map_or(Ok(default), |value| {
                    value
                        .as_str()
                        .ok_or(WindowsSettingsError::new(field, "wrong-type"))
                })
            };
            settings.shell = WindowsShellSettings::new(
                shell_value("default_shell", "terminal.default_shell", "")?,
                shell_value("shell_mode", "terminal.shell_mode", "auto")?,
            )?;
        }
        let Some(value) = table.get(key) else {
            continue;
        };
        if section == "update" {
            settings.manifest_check = value
                .as_bool()
                .ok_or(WindowsSettingsError::new(field, "wrong-type"))?;
            settings.manifest_check_source = "file";
            continue;
        }
        if section == "session" {
            settings.resume_agents_on_restore = value
                .as_bool()
                .ok_or(WindowsSettingsError::new(field, "wrong-type"))?;
            settings.resume_agents_on_restore_source = "file";
            continue;
        }
        let value = value
            .as_str()
            .ok_or(WindowsSettingsError::new(field, "wrong-type"))?;
        if section == "terminal" {
            settings.cwd = Some(match value.trim() {
                "" | "follow" => (WindowsDirectoryPolicy::Follow, None),
                "home" => (WindowsDirectoryPolicy::Home, None),
                "current" => (WindowsDirectoryPolicy::Current, None),
                "workspace" => (WindowsDirectoryPolicy::Workspace, None),
                _ => (
                    WindowsDirectoryPolicy::Explicit,
                    Some(normalize(value, path, env, field)?),
                ),
            });
        } else {
            settings.worktree_root = Some(normalize(value, path, env, field)?);
        }
    }
    Ok(settings)
}

fn normalize(
    value: &str,
    file: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
    field: &'static str,
) -> Result<String, WindowsSettingsError> {
    if value.trim().is_empty() {
        return Err(WindowsSettingsError::new(field, "invalid-path"));
    }
    normalize_windows_user_path(value, env, || {
        file.parent()
            .map(Path::to_owned)
            .ok_or_else(|| std::io::ErrorKind::NotFound.into())
    })
    .map_err(|_| WindowsSettingsError::new(field, "invalid-path"))
}

#[cfg(test)]
mod tests;
