//! Shared Windows user settings: fatal source boundaries, field-local admission, and explicit repair.
mod agent_sidebar;
mod diagnostics;
mod source;
use diagnostics::{DiagnosticsCollector, unknown_field};
mod sidebar_tokens;
mod space_sidebar;
pub use sidebar_tokens::SidebarTokenStyle;
pub use space_sidebar::{
    MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES, SpaceSidebarSettings, SpaceSidebarToken,
    SpaceSidebarTokenKind,
};
mod keys;
pub use agent_sidebar::{
    AgentSidebarSettings, AgentSidebarToken, AgentSidebarTokenKind,
    MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES,
};
mod notifications;
pub use diagnostics::{
    MAXIMUM_SETTINGS_DIAGNOSTICS, SettingsDiagnostic, SettingsDiagnosticDisposition,
    SettingsDiagnostics,
};
pub use notifications::{
    MAXIMUM_AGENT_SOUND_OVERRIDES, MAXIMUM_NOTIFICATION_DELAY_SECONDS,
    MAXIMUM_NOTIFICATION_PROFILE_BYTES, NotificationSettings,
};
mod reference;
mod reset_keys;
mod window_title;
use crate::user_path::{USER_PATH_LIMIT, normalize_user_path};
pub use keys::{
    CommandSize, CustomCommand, CustomCommandAction, CustomCommandEntry, KeyChord, KeyOverride,
    KeySettings, KeyTrigger, trigger_label,
};
use reference::user_config_has_child;
pub use reference::{
    DefaultActionBindings, USER_CONFIG_OPTIONS, UserConfigOption, default_user_settings_toml,
    default_user_settings_toml_with_actions,
};
pub use reset_keys::{KeysResetOutcome, KeysResetReport, reset_keybindings};
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
};
pub use window_title::{DEFAULT_WINDOW_TITLE, WindowTitleTemplate};

const LIMIT: u64 = 64 * 1024;
mod channel;
pub use channel::set_update_channel;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserSettings {
    pub window_title: String,
    pub agent_sidebar: AgentSidebarSettings,
    pub space_sidebar: SpaceSidebarSettings,
    pub notifications: NotificationSettings,
    pub keys: KeySettings,
    pub focus_new_panel: bool,
    pub path: Option<PathBuf>,
    pub exists: bool,
    pub explicit: bool,
    pub update_channel: UpdateChannel,
    pub update_channel_source: &'static str,
    pub manifest_check: bool,
    pub manifest_check_source: &'static str,
    pub cwd: Option<(DirectoryPolicy, Option<String>)>,
    pub worktree_root: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// One complete effective settings candidate and its bounded admission diagnostics.
pub struct UserSettingsLoad {
    /// Effective settings after field-local fallback.
    pub settings: UserSettings,
    /// Diagnostics describing every visible fallback, ignored field, or unsupported field.
    pub diagnostics: SettingsDiagnostics,
}

impl std::ops::Deref for UserSettingsLoad {
    type Target = UserSettings;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UpdateChannel {
    #[default]
    Stable,
    Preview,
}
impl UpdateChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Preview => "preview",
        }
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            window_title: DEFAULT_WINDOW_TITLE.into(),
            agent_sidebar: AgentSidebarSettings::default(),
            space_sidebar: SpaceSidebarSettings::default(),
            notifications: NotificationSettings::default(),
            keys: KeySettings::default(),
            focus_new_panel: true,
            path: None,
            exists: false,
            explicit: false,
            update_channel: UpdateChannel::Stable,
            update_channel_source: "default",
            manifest_check: true,
            manifest_check_source: "default",
            cwd: None,
            worktree_root: None,
        }
    }
}

/// Resolve one complete effective configuration and its diagnostics.
pub fn read_user_settings(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<UserSettingsLoad, SettingsError> {
    read_file(env)
}

#[derive(Debug, Eq, PartialEq)]
pub struct SettingsError {
    pub field: &'static str,
    pub reason: &'static str,
}
impl SettingsError {
    pub const fn new(field: &'static str, reason: &'static str) -> Self {
        Self { field, reason }
    }
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid Windows configuration: {} ({})",
            self.field, self.reason
        )
    }
}
impl std::error::Error for SettingsError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectoryPolicy {
    Follow,
    Home,
    Current,
    Workspace,
    Explicit,
}

fn location(
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<Option<(PathBuf, bool)>, SettingsError> {
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
        .ok_or(SettingsError::new(field, "invalid-path"))?;
    let path = Path::new(raw);
    if !path.is_absolute() || raw.is_empty() {
        return Err(SettingsError::new(field, "invalid-path"));
    }
    let checked = normalize_user_path(raw, env, || Err(std::io::ErrorKind::NotFound.into()))
        .map_err(|_| SettingsError::new(field, "invalid-path"))?;
    let path = PathBuf::from(checked.as_str());
    let path = if explicit {
        path
    } else {
        path.join("Orchbee/config.toml")
    };
    if path
        .to_str()
        .is_none_or(|value| value.len() > USER_PATH_LIMIT)
    {
        return Err(SettingsError::new(field, "invalid-path"));
    }
    Ok(Some((path, explicit)))
}

/// The shared settings location without opening the file or applying runtime settings.
/// Companion user assets use this parent directory; absence remains explicit.
pub fn user_settings_path(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<Option<PathBuf>, SettingsError> {
    location(&env).map(|location| location.map(|(path, _)| path))
}

fn read_file(env: impl Fn(&str) -> Option<OsString>) -> Result<UserSettingsLoad, SettingsError> {
    let location = location(&env)?;
    read_file_at(
        location.as_ref().map(|(path, _)| path.as_path()),
        location.as_ref().is_some_and(|(_, explicit)| *explicit),
        env,
    )
}
/// Explicit reload keeps the admitted startup path, including default-file absence semantics.
pub fn read_user_settings_at(
    path: Option<&Path>,
    explicit: bool,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<UserSettingsLoad, SettingsError> {
    read_file_at(path, explicit, env)
}

fn read_file_at(
    path: Option<&Path>,
    explicit: bool,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<UserSettingsLoad, SettingsError> {
    source::require()?;
    let Some(path) = path else {
        if explicit {
            return Err(SettingsError::new("file", "invalid-path"));
        }
        return Ok(UserSettingsLoad::default());
    };
    let path = path.to_owned();
    let file = match source::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !explicit => {
            return Ok(UserSettingsLoad {
                settings: UserSettings {
                    path: Some(path),
                    ..UserSettings::default()
                },
                diagnostics: SettingsDiagnostics::default(),
            });
        }
        Err(error) => {
            return Err(SettingsError::new(
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
        .map_err(|_| SettingsError::new("file", "io-error"))?;
    if !metadata.is_file() {
        return Err(SettingsError::new("file", "not-file"));
    }
    if metadata.len() > LIMIT {
        return Err(SettingsError::new("file", "too-large"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SettingsError::new("file", "io-error"))?;
    if bytes.len() as u64 > LIMIT {
        return Err(SettingsError::new("file", "too-large"));
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| SettingsError::new("file", "invalid-utf8"))?;
    let mut load = parse(text, &path, &env)?;
    load.settings.explicit = explicit;
    Ok(load)
}

fn parse(
    text: &str,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<UserSettingsLoad, SettingsError> {
    let table: toml::Table =
        toml::from_str(text).map_err(|_| SettingsError::new("file", "invalid-toml"))?;
    let mut settings = UserSettings {
        path: Some(path.to_owned()),
        exists: true,
        ..UserSettings::default()
    };
    let mut diagnostics = DiagnosticsCollector::default();
    for (section, value) in table {
        if section == "ui" {
            let Some(table) = value.as_table() else {
                diagnostics.push("ui", "wrong-type", SettingsDiagnosticDisposition::Defaulted);
                continue;
            };
            for key in table.keys().filter(|key| !user_config_has_child("ui", key)) {
                diagnostics.push(
                    unknown_field("ui", key),
                    "unknown-key",
                    SettingsDiagnosticDisposition::Ignored,
                );
            }
            if table.contains_key("toast") || table.contains_key("sound") {
                settings.notifications = NotificationSettings::parse_lenient(
                    table.get("toast"),
                    table.get("sound"),
                    &mut diagnostics,
                );
            }
            if let Some(sidebar) = table.get("sidebar") {
                if let Some(sidebar) = sidebar.as_table() {
                    for key in sidebar
                        .keys()
                        .filter(|key| !user_config_has_child("ui.sidebar", key))
                    {
                        diagnostics.push(
                            unknown_field("ui.sidebar", key),
                            "unknown-key",
                            SettingsDiagnosticDisposition::Ignored,
                        );
                    }
                    if let Some(spaces) = sidebar.get("spaces") {
                        match parse_space_sidebar(spaces, &mut diagnostics) {
                            Ok(profile) => settings.space_sidebar = profile,
                            Err(error) => diagnostics
                                .push_error(&error, SettingsDiagnosticDisposition::Defaulted),
                        }
                    }
                    if let Some(agents) = sidebar.get("agents") {
                        match parse_agent_sidebar(agents, &mut diagnostics) {
                            Ok(profile) => settings.agent_sidebar = profile,
                            Err(error) => diagnostics
                                .push_error(&error, SettingsDiagnosticDisposition::Defaulted),
                        }
                    }
                } else {
                    diagnostics.push(
                        "ui.sidebar",
                        "wrong-type",
                        SettingsDiagnosticDisposition::Defaulted,
                    );
                }
            }
            if let Some(value) = table.get("window_title") {
                if let Some(value) = value.as_str() {
                    match WindowTitleTemplate::parse(value) {
                        Ok(_) => settings.window_title = value.into(),
                        Err(reason) => diagnostics.push(
                            "ui.window_title",
                            reason,
                            SettingsDiagnosticDisposition::Defaulted,
                        ),
                    }
                } else {
                    diagnostics.push(
                        "ui.window_title",
                        "wrong-type",
                        SettingsDiagnosticDisposition::Defaulted,
                    );
                }
            }
            continue;
        }
        if section == "keys" {
            settings.keys = KeySettings::parse_lenient(&value, &mut diagnostics);
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
                        SettingsDiagnosticDisposition::Ignored,
                    );
                    continue;
                }
            };
            diagnostics.push(
                field,
                "wrong-type",
                SettingsDiagnosticDisposition::Defaulted,
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
                SettingsDiagnosticDisposition::Ignored,
            ),
        }
    }
    Ok(UserSettingsLoad {
        settings,
        diagnostics: diagnostics.finish(),
    })
}

fn parse_agent_sidebar(
    value: &toml::Value,
    diagnostics: &mut DiagnosticsCollector,
) -> Result<AgentSidebarSettings, SettingsError> {
    let Some(table) = value.as_table() else {
        return AgentSidebarSettings::parse(value);
    };
    let mut admitted = table.clone();
    admitted.retain(|key, _| {
        let known = user_config_has_child("ui.sidebar.agents", key);
        if !known {
            diagnostics.push(
                unknown_field("ui.sidebar.agents", key),
                "unknown-key",
                SettingsDiagnosticDisposition::Ignored,
            );
        }
        known
    });
    AgentSidebarSettings::parse(&toml::Value::Table(admitted))
}

fn parse_space_sidebar(
    value: &toml::Value,
    diagnostics: &mut DiagnosticsCollector,
) -> Result<SpaceSidebarSettings, SettingsError> {
    let Some(table) = value.as_table() else {
        return SpaceSidebarSettings::parse(value);
    };
    let mut admitted = table.clone();
    admitted.retain(|key, _| {
        let known = user_config_has_child("ui.sidebar.spaces", key);
        if !known {
            diagnostics.push(
                unknown_field("ui.sidebar.spaces", key),
                "unknown-key",
                SettingsDiagnosticDisposition::Ignored,
            );
        }
        known
    });
    SpaceSidebarSettings::parse(&toml::Value::Table(admitted))
}

fn parse_terminal(
    table: &toml::Table,
    settings: &mut UserSettings,
    diagnostics: &mut DiagnosticsCollector,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) {
    for key in table
        .keys()
        .filter(|key| !user_config_has_child("terminal", key))
    {
        diagnostics.push(
            unknown_field("terminal", key),
            "unknown-key",
            SettingsDiagnosticDisposition::Ignored,
        );
    }
    if let Some(value) = table.get("focus_new_panel") {
        match value.as_bool() {
            Some(value) => settings.focus_new_panel = value,
            None => diagnostics.push(
                "terminal.focus_new_panel",
                "wrong-type",
                SettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
    let Some(value) = table.get("new_cwd") else {
        return;
    };
    let Some(value) = value.as_str() else {
        diagnostics.push(
            "terminal.new_cwd",
            "wrong-type",
            SettingsDiagnosticDisposition::Defaulted,
        );
        return;
    };
    let candidate = match value.trim() {
        "" | "follow" => Ok((DirectoryPolicy::Follow, None)),
        "home" => Ok((DirectoryPolicy::Home, None)),
        "current" => Ok((DirectoryPolicy::Current, None)),
        "workspace" => Ok((DirectoryPolicy::Workspace, None)),
        _ => normalize(value, path, env, "terminal.new_cwd")
            .map(|value| (DirectoryPolicy::Explicit, Some(value))),
    };
    match candidate {
        Ok(candidate) => settings.cwd = Some(candidate),
        Err(error) => diagnostics.push_error(&error, SettingsDiagnosticDisposition::Defaulted),
    }
}

fn parse_update(
    table: &toml::Table,
    settings: &mut UserSettings,
    diagnostics: &mut DiagnosticsCollector,
) {
    for key in table
        .keys()
        .filter(|key| !user_config_has_child("update", key))
    {
        diagnostics.push(
            unknown_field("update", key),
            "unknown-key",
            SettingsDiagnosticDisposition::Ignored,
        );
    }
    if let Some(value) = table.get("channel") {
        match value.as_str() {
            Some("stable") => {
                settings.update_channel = UpdateChannel::Stable;
                settings.update_channel_source = "file";
            }
            Some("preview") => {
                settings.update_channel = UpdateChannel::Preview;
                settings.update_channel_source = "file";
            }
            Some(_) => diagnostics.push(
                "update.channel",
                "invalid-channel",
                SettingsDiagnosticDisposition::Defaulted,
            ),
            None => diagnostics.push(
                "update.channel",
                "wrong-type",
                SettingsDiagnosticDisposition::Defaulted,
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
                SettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
}

fn parse_worktrees(
    table: &toml::Table,
    settings: &mut UserSettings,
    diagnostics: &mut DiagnosticsCollector,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) {
    for key in table
        .keys()
        .filter(|key| !user_config_has_child("worktrees", key))
    {
        diagnostics.push(
            unknown_field("worktrees", key),
            "unknown-key",
            SettingsDiagnosticDisposition::Ignored,
        );
    }
    let Some(value) = table.get("directory") else {
        return;
    };
    let candidate = value
        .as_str()
        .ok_or(SettingsError::new("worktrees.directory", "wrong-type"))
        .and_then(|value| normalize(value, path, env, "worktrees.directory"));
    match candidate {
        Ok(value) => settings.worktree_root = Some(value),
        Err(error) => diagnostics.push_error(&error, SettingsDiagnosticDisposition::Defaulted),
    }
}

fn normalize(
    value: &str,
    file: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
    field: &'static str,
) -> Result<String, SettingsError> {
    if value.trim().is_empty() {
        return Err(SettingsError::new(field, "invalid-path"));
    }
    normalize_user_path(value, env, || {
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
    .ok()
    // The Session publishes only paths every creation consumer admits.
    .filter(|value| orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(value).is_ok())
    .ok_or(SettingsError::new(field, "invalid-path"))
}

#[cfg(test)]
mod tests;
