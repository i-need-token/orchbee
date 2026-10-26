//! Shared Windows user settings: fatal source boundaries, field-local admission, and explicit repair.
mod agent_sidebar;
mod diagnostics;
mod source;
use diagnostics::{DiagnosticsCollector, unknown_field};
mod sidebar_tokens;
mod space_sidebar;
pub use sidebar_tokens::WindowsSidebarTokenStyle;
pub use space_sidebar::{
    MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES, WindowsSpaceSidebarSettings, WindowsSpaceSidebarToken,
    WindowsSpaceSidebarTokenKind,
};
mod keys;
pub use agent_sidebar::{
    MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES, WindowsAgentSidebarSettings, WindowsAgentSidebarToken,
    WindowsAgentSidebarTokenKind,
};
mod notifications;
pub use diagnostics::{
    MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS, WindowsSettingsDiagnostic,
    WindowsSettingsDiagnosticDisposition, WindowsSettingsDiagnostics,
};
pub use notifications::{
    MAXIMUM_AGENT_SOUND_OVERRIDES, MAXIMUM_NOTIFICATION_DELAY_SECONDS,
    MAXIMUM_NOTIFICATION_PROFILE_BYTES, WindowsAgentSoundSetting, WindowsNotificationDelivery,
    WindowsNotificationSettings,
};
#[cfg(windows)]
#[cfg(windows)]
mod reset_keys;
mod shell;
mod theme;
pub use theme::WindowsThemeName;
mod reference;
mod window_title;
use crate::windows_user_path::{WINDOWS_USER_PATH_LIMIT, normalize_windows_user_path};
pub use keys::{
    WindowsCommandSize, WindowsCustomCommand, WindowsCustomCommandAction,
    WindowsCustomCommandEntry, WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings,
    WindowsKeyTrigger, trigger_label,
};
use reference::windows_user_config_has_child;
pub use reference::{
    WINDOWS_USER_CONFIG_OPTIONS, WindowsDefaultActionBindings, WindowsUserConfigOption,
    default_windows_user_settings_toml, default_windows_user_settings_toml_with_actions,
};
#[cfg(windows)]
#[cfg(windows)]
pub use reset_keys::{WindowsKeysResetOutcome, WindowsKeysResetReport, reset_windows_keybindings};
pub use shell::{WindowsShellMode, WindowsShellSettings};
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
};
pub use window_title::{DEFAULT_WINDOW_TITLE, WindowsWindowTitleTemplate};

const LIMIT: u64 = 64 * 1024;
#[cfg(windows)]
#[cfg(windows)]
mod channel;
#[cfg(windows)]
#[cfg(windows)]
pub use channel::set_windows_update_channel;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsUserSettings {
    pub theme: WindowsThemeName,
    pub window_title: String,
    pub agent_sidebar: WindowsAgentSidebarSettings,
    pub space_sidebar: WindowsSpaceSidebarSettings,
    pub notifications: WindowsNotificationSettings,
    pub keys: WindowsKeySettings,
    pub shell: WindowsShellSettings,
    pub focus_new_panel: bool,
    pub path: Option<PathBuf>,
    pub exists: bool,
    pub explicit: bool,
    pub update_channel: WindowsUpdateChannel,
    pub update_channel_source: &'static str,
    pub manifest_check: bool,
    pub manifest_check_source: &'static str,
    pub cwd: Option<(WindowsDirectoryPolicy, Option<String>)>,
    pub worktree_root: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// One complete effective settings candidate and its bounded admission diagnostics.
pub struct WindowsUserSettingsLoad {
    /// Effective settings after field-local fallback.
    pub settings: WindowsUserSettings,
    /// Diagnostics describing every visible fallback, ignored field, or unsupported field.
    pub diagnostics: WindowsSettingsDiagnostics,
}

impl std::ops::Deref for WindowsUserSettingsLoad {
    type Target = WindowsUserSettings;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowsUpdateChannel {
    #[default]
    Stable,
    Preview,
}
impl WindowsUpdateChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Preview => "preview",
        }
    }
}

impl Default for WindowsUserSettings {
    fn default() -> Self {
        Self {
            theme: WindowsThemeName::default(),
            window_title: DEFAULT_WINDOW_TITLE.into(),
            agent_sidebar: WindowsAgentSidebarSettings::default(),
            space_sidebar: WindowsSpaceSidebarSettings::default(),
            notifications: WindowsNotificationSettings::default(),
            keys: WindowsKeySettings::default(),
            shell: WindowsShellSettings::default(),
            focus_new_panel: true,
            path: None,
            exists: false,
            explicit: false,
            update_channel: WindowsUpdateChannel::Stable,
            update_channel_source: "default",
            manifest_check: true,
            manifest_check_source: "default",
            cwd: None,
            worktree_root: None,
        }
    }
}

/// Resolve one complete effective configuration and its diagnostics.
pub fn read_windows_user_settings(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    read_file(env)
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
    source::require()?;
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
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    let location = location(&env)?;
    read_file_at(
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
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    read_file_at(path, explicit, env)
}

fn read_file_at(
    path: Option<&Path>,
    explicit: bool,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    source::require()?;
    let Some(path) = path else {
        if explicit {
            return Err(WindowsSettingsError::new("file", "invalid-path"));
        }
        return Ok(WindowsUserSettingsLoad::default());
    };
    let path = path.to_owned();
    let file = match source::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !explicit => {
            return Ok(WindowsUserSettingsLoad {
                settings: WindowsUserSettings {
                    path: Some(path),
                    ..WindowsUserSettings::default()
                },
                diagnostics: WindowsSettingsDiagnostics::default(),
            });
        }
        Err(error) => {
            return Err(WindowsSettingsError::new(
                "file",
                if error.kind() == std::io::ErrorKind::NotFound {
                    "missing"
                } else if std::fs::metadata(&path).is_ok_and(|metadata| !metadata.is_file()) {
                    "not-file"
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
    let mut load = parse(text, &path, &env)?;
    load.settings.explicit = explicit;
    Ok(load)
}

fn parse(
    text: &str,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    let table: toml::Table =
        toml::from_str(text).map_err(|_| WindowsSettingsError::new("file", "invalid-toml"))?;
    let mut settings = WindowsUserSettings {
        path: Some(path.to_owned()),
        exists: true,
        ..WindowsUserSettings::default()
    };
    let mut diagnostics = DiagnosticsCollector::default();
    for (section, value) in table {
        if section == "ui" {
            let Some(table) = value.as_table() else {
                diagnostics.push(
                    "ui",
                    "wrong-type",
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                );
                continue;
            };
            for key in table
                .keys()
                .filter(|key| !windows_user_config_has_child("ui", key))
            {
                diagnostics.push(
                    unknown_field("ui", key),
                    "unknown-key",
                    WindowsSettingsDiagnosticDisposition::Ignored,
                );
            }
            if table.contains_key("toast") || table.contains_key("sound") {
                settings.notifications = WindowsNotificationSettings::parse_lenient(
                    table.get("toast"),
                    table.get("sound"),
                    &mut diagnostics,
                );
            }
            if let Some(sidebar) = table.get("sidebar") {
                if let Some(sidebar) = sidebar.as_table() {
                    for key in sidebar
                        .keys()
                        .filter(|key| !windows_user_config_has_child("ui.sidebar", key))
                    {
                        diagnostics.push(
                            unknown_field("ui.sidebar", key),
                            "unknown-key",
                            WindowsSettingsDiagnosticDisposition::Ignored,
                        );
                    }
                    if let Some(spaces) = sidebar.get("spaces") {
                        match parse_space_sidebar(spaces, &mut diagnostics) {
                            Ok(profile) => settings.space_sidebar = profile,
                            Err(error) => diagnostics.push_error(
                                &error,
                                WindowsSettingsDiagnosticDisposition::Defaulted,
                            ),
                        }
                    }
                    if let Some(agents) = sidebar.get("agents") {
                        match parse_agent_sidebar(agents, &mut diagnostics) {
                            Ok(profile) => settings.agent_sidebar = profile,
                            Err(error) => diagnostics.push_error(
                                &error,
                                WindowsSettingsDiagnosticDisposition::Defaulted,
                            ),
                        }
                    }
                } else {
                    diagnostics.push(
                        "ui.sidebar",
                        "wrong-type",
                        WindowsSettingsDiagnosticDisposition::Defaulted,
                    );
                }
            }
            if let Some(value) = table.get("window_title") {
                if let Some(value) = value.as_str() {
                    match WindowsWindowTitleTemplate::parse(value) {
                        Ok(_) => settings.window_title = value.into(),
                        Err(reason) => diagnostics.push(
                            "ui.window_title",
                            reason,
                            WindowsSettingsDiagnosticDisposition::Defaulted,
                        ),
                    }
                } else {
                    diagnostics.push(
                        "ui.window_title",
                        "wrong-type",
                        WindowsSettingsDiagnosticDisposition::Defaulted,
                    );
                }
            }
            continue;
        }
        if section == "keys" {
            settings.keys = WindowsKeySettings::parse_lenient(&value, &mut diagnostics);
            continue;
        }
        let Some(table) = value.as_table() else {
            let field = match section.as_str() {
                "terminal" => "terminal",
                "update" => "update",
                "worktrees" => "worktrees",
                "session" => "session",
                _ => {
                    diagnostics.push(
                        unknown_field("", &section),
                        "unknown-key",
                        WindowsSettingsDiagnosticDisposition::Ignored,
                    );
                    continue;
                }
            };
            diagnostics.push(
                field,
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            );
            continue;
        };
        match section.as_str() {
            "terminal" => parse_terminal(table, &mut settings, &mut diagnostics, path, env),
            "update" => parse_update(table, &mut settings, &mut diagnostics),
            "worktrees" => parse_worktrees(table, &mut settings, &mut diagnostics, path, env),
            _ => diagnostics.push(
                unknown_field("", &section),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            ),
        }
    }
    Ok(WindowsUserSettingsLoad {
        settings,
        diagnostics: diagnostics.finish(),
    })
}

fn parse_agent_sidebar(
    value: &toml::Value,
    diagnostics: &mut DiagnosticsCollector,
) -> Result<WindowsAgentSidebarSettings, WindowsSettingsError> {
    let Some(table) = value.as_table() else {
        return WindowsAgentSidebarSettings::parse(value);
    };
    let mut admitted = table.clone();
    admitted.retain(|key, _| {
        let known = windows_user_config_has_child("ui.sidebar.agents", key);
        if !known {
            diagnostics.push(
                unknown_field("ui.sidebar.agents", key),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        known
    });
    WindowsAgentSidebarSettings::parse(&toml::Value::Table(admitted))
}

fn parse_space_sidebar(
    value: &toml::Value,
    diagnostics: &mut DiagnosticsCollector,
) -> Result<WindowsSpaceSidebarSettings, WindowsSettingsError> {
    let Some(table) = value.as_table() else {
        return WindowsSpaceSidebarSettings::parse(value);
    };
    let mut admitted = table.clone();
    admitted.retain(|key, _| {
        let known = windows_user_config_has_child("ui.sidebar.spaces", key);
        if !known {
            diagnostics.push(
                unknown_field("ui.sidebar.spaces", key),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        known
    });
    WindowsSpaceSidebarSettings::parse(&toml::Value::Table(admitted))
}

fn parse_terminal(
    table: &toml::Table,
    settings: &mut WindowsUserSettings,
    diagnostics: &mut DiagnosticsCollector,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) {
    for key in table
        .keys()
        .filter(|key| !windows_user_config_has_child("terminal", key))
    {
        diagnostics.push(
            unknown_field("terminal", key),
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
    }
    if let Some(value) = table.get("focus_new_panel") {
        match value.as_bool() {
            Some(value) => settings.focus_new_panel = value,
            None => diagnostics.push(
                "terminal.focus_new_panel",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
    let mut executable = String::new();
    if let Some(value) = table.get("default_shell") {
        match value.as_str() {
            Some(value) => match WindowsShellSettings::new(value, "auto") {
                Ok(shell) => executable = shell.executable().to_owned(),
                Err(error) => {
                    diagnostics.push_error(&error, WindowsSettingsDiagnosticDisposition::Defaulted)
                }
            },
            None => diagnostics.push(
                "terminal.default_shell",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
    let mut mode = "auto";
    if let Some(value) = table.get("shell_mode") {
        match value.as_str() {
            Some(value) => match WindowsShellSettings::new("", value) {
                Ok(_) => mode = value,
                Err(error) => {
                    diagnostics.push_error(&error, WindowsSettingsDiagnosticDisposition::Defaulted)
                }
            },
            None => diagnostics.push(
                "terminal.shell_mode",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
    settings.shell = WindowsShellSettings::new(&executable, mode)
        .expect("independently admitted shell leaves compose");
    let Some(value) = table.get("new_cwd") else {
        return;
    };
    let Some(value) = value.as_str() else {
        diagnostics.push(
            "terminal.new_cwd",
            "wrong-type",
            WindowsSettingsDiagnosticDisposition::Defaulted,
        );
        return;
    };
    let candidate = match value.trim() {
        "" | "follow" => Ok((WindowsDirectoryPolicy::Follow, None)),
        "home" => Ok((WindowsDirectoryPolicy::Home, None)),
        "current" => Ok((WindowsDirectoryPolicy::Current, None)),
        "workspace" => Ok((WindowsDirectoryPolicy::Workspace, None)),
        _ => normalize(value, path, env, "terminal.new_cwd")
            .map(|value| (WindowsDirectoryPolicy::Explicit, Some(value))),
    };
    match candidate {
        Ok(candidate) => settings.cwd = Some(candidate),
        Err(error) => {
            diagnostics.push_error(&error, WindowsSettingsDiagnosticDisposition::Defaulted)
        }
    }
}

fn parse_update(
    table: &toml::Table,
    settings: &mut WindowsUserSettings,
    diagnostics: &mut DiagnosticsCollector,
) {
    for key in table
        .keys()
        .filter(|key| !windows_user_config_has_child("update", key))
    {
        diagnostics.push(
            unknown_field("update", key),
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
    }
    if let Some(value) = table.get("channel") {
        match value.as_str() {
            Some("stable") => {
                settings.update_channel = WindowsUpdateChannel::Stable;
                settings.update_channel_source = "file";
            }
            Some("preview") => {
                settings.update_channel = WindowsUpdateChannel::Preview;
                settings.update_channel_source = "file";
            }
            Some(_) => diagnostics.push(
                "update.channel",
                "invalid-channel",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
            None => diagnostics.push(
                "update.channel",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
    if let Some(value) = table.get("manifest_check") {
        match value.as_bool() {
            Some(value) => {
                settings.manifest_check = value;
                settings.manifest_check_source = "file";
            }
            None => diagnostics.push(
                "update.manifest_check",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
}

fn parse_worktrees(
    table: &toml::Table,
    settings: &mut WindowsUserSettings,
    diagnostics: &mut DiagnosticsCollector,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) {
    for key in table
        .keys()
        .filter(|key| !windows_user_config_has_child("worktrees", key))
    {
        diagnostics.push(
            unknown_field("worktrees", key),
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
    }
    let Some(value) = table.get("directory") else {
        return;
    };
    let candidate = value
        .as_str()
        .ok_or(WindowsSettingsError::new(
            "worktrees.directory",
            "wrong-type",
        ))
        .and_then(|value| normalize(value, path, env, "worktrees.directory"));
    match candidate {
        Ok(value) => settings.worktree_root = Some(value),
        Err(error) => {
            diagnostics.push_error(&error, WindowsSettingsDiagnosticDisposition::Defaulted)
        }
    }
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
        let file = file.to_str().ok_or(std::io::ErrorKind::InvalidData)?;
        let end = file
            .rfind(['/', '\\'])
            .ok_or(std::io::ErrorKind::NotFound)?;
        let parent = if end > 0 && file.as_bytes().get(end - 1) == Some(&b':') {
            &file[..=end]
        } else {
            &file[..end]
        };
        Ok(PathBuf::from(parent))
    })
    .map_err(|_| WindowsSettingsError::new(field, "invalid-path"))
}

#[cfg(test)]
mod tests;
