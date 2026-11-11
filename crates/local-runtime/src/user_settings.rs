//! Shared user settings in `config.toml`: fatal source boundaries, field-local admission, and
//! single-key edits.
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
mod desktop;
mod launch_profiles;
mod preferences;
mod reference;
mod registry;
mod reset_keys;
mod window_title;
use crate::{
    DataRoot,
    user_path::{USER_PATH_LIMIT, normalize_user_path},
};
pub use desktop::{DesktopTerminalSettings, MacosOptionAsAlt};
pub use keys::{
    CommandSize, CustomCommand, CustomCommandAction, CustomCommandEntry, KeyChord, KeyOverride,
    KeySettings, KeyTrigger, trigger_label,
};
use orchbee_workbench_protocol::UserPreferencesValues;
pub use reference::{
    DefaultActionBindings, default_user_settings_toml, default_user_settings_toml_with_actions,
};
pub(crate) use registry::recognized_path;
pub use registry::{
    SETTING_KEYS, SETTING_PLATFORMS, SettingCategory, SettingClients, SettingDefault,
    SettingEffect, SettingGroup, SettingKey, SettingValue, find_setting,
};
use registry::{Scalar, user_config_has_child};
pub use reset_keys::{KeysResetOutcome, KeysResetReport, reset_keybindings};
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
};
pub use window_title::{DEFAULT_WINDOW_TITLE, WindowTitleTemplate};

/// Largest `config.toml` the reader admits: room for every platform's launch profiles.
pub const MAXIMUM_USER_SETTINGS_BYTES: u64 = 2 * 1024 * 1024;
const LIMIT: u64 = MAXIMUM_USER_SETTINGS_BYTES;
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
    /// The common preferences the file carries. `preferences.json` remains their runtime source
    /// until the clients read them from here.
    pub preferences: UserPreferencesValues,
    /// Settings only the desktop app applies.
    pub desktop: DesktopTerminalSettings,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// One complete effective settings candidate and its bounded admission diagnostics.
pub struct UserSettingsLoad {
    /// Effective settings after field-local fallback.
    pub settings: UserSettings,
    /// Diagnostics describing every visible fallback, ignored field, or unsupported field.
    pub diagnostics: SettingsDiagnostics,
    /// Present when the file is not valid UTF-8 or TOML; `settings` then hold the defaults.
    pub syntax: Option<SettingsSyntaxError>,
}

/// Where `config.toml` stops being valid UTF-8 or TOML.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsSyntaxError {
    /// `invalid-utf8` or `invalid-toml`.
    pub reason: &'static str,
    /// One-based line of the first error.
    pub line: usize,
    /// One-based column, in characters, of the first error.
    pub column: usize,
}

impl SettingsSyntaxError {
    fn at(text: &str, offset: usize, reason: &'static str) -> Self {
        let offset = (0..=offset.min(text.len()))
            .rev()
            .find(|offset| text.is_char_boundary(*offset))
            .unwrap_or(0);
        let before = &text[..offset];
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        Self {
            reason,
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

impl UserSettingsLoad {
    /// Refuse a file that does not parse, for callers that must not act on defaults in its place:
    /// `config check`, an explicit reload, and update operations.
    pub fn require_valid_syntax(self) -> Result<Self, SettingsError> {
        match self.syntax {
            Some(syntax) => Err(SettingsError::new("file", syntax.reason)),
            None => Ok(self),
        }
    }

    /// Keep `last_valid` in effect when the file no longer parses; the syntax error and its
    /// diagnostic stay visible.
    pub fn or_last_valid(mut self, last_valid: &UserSettings) -> Self {
        if self.syntax.is_some() {
            self.settings = UserSettings {
                path: self.settings.path,
                exists: self.settings.exists,
                explicit: self.settings.explicit,
                ..last_valid.clone()
            };
        }
        self
    }
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
            preferences: UserPreferencesValues::default(),
            desktop: DesktopTerminalSettings::default(),
        }
    }
}

/// Resolve one complete effective configuration and its diagnostics. A file that is not valid
/// UTF-8 or TOML yields the defaults with [`UserSettingsLoad::syntax`] set; only an unreadable or
/// oversized file, or an unusable location, is an error.
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
        write!(f, "invalid configuration: {} ({})", self.field, self.reason)
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

/// `ORCHBEE_CONFIG_FILE` when set, otherwise `config.toml` in the data root. An explicit file
/// moves only the settings; every other record stays in the data root. A home folder that is not
/// set leaves no location, which activation reports with the variable that should supply it.
fn location(
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<Option<(PathBuf, bool)>, SettingsError> {
    source::require()?;
    let (path, field, explicit) = if let Some(value) = env("ORCHBEE_CONFIG_FILE") {
        let field = "ORCHBEE_CONFIG_FILE";
        let raw = value
            .to_str()
            .ok_or(SettingsError::new(field, "invalid-path"))?;
        if !Path::new(raw).is_absolute() || raw.is_empty() {
            return Err(SettingsError::new(field, "invalid-path"));
        }
        let checked = normalize_user_path(raw, env, || Err(std::io::ErrorKind::NotFound.into()))
            .map_err(|_| SettingsError::new(field, "invalid-path"))?;
        (PathBuf::from(checked.as_str()), field, true)
    } else {
        match DataRoot::resolve(env) {
            Ok(root) => (root.config_file(), DataRoot::variable(env), false),
            Err(error) if error.reason == "missing" => return Ok(None),
            Err(error) => return Err(SettingsError::new(error.variable, error.reason)),
        }
    };
    if path
        .to_str()
        .is_none_or(|value| value.len() > USER_PATH_LIMIT)
    {
        return Err(SettingsError::new(field, "invalid-path"));
    }
    Ok(Some((path, explicit)))
}

/// The shared settings location without opening the file or applying runtime settings; absence
/// remains explicit. Records other than settings derive from the data root, never from this path.
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
                ..UserSettingsLoad::default()
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
    let mut load = parse_bytes(&bytes, &path, &env);
    load.settings.explicit = explicit;
    Ok(load)
}

fn parse_bytes(
    bytes: &[u8],
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) -> UserSettingsLoad {
    match std::str::from_utf8(bytes) {
        Ok(text) => parse(text, path, env),
        Err(error) => {
            let valid = std::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap_or_default();
            unparsed(
                path,
                SettingsSyntaxError::at(valid, valid.len(), "invalid-utf8"),
            )
        }
    }
}

/// The defaults in place of a file that does not parse, with one diagnostic naming the file.
fn unparsed(path: &Path, syntax: SettingsSyntaxError) -> UserSettingsLoad {
    let mut diagnostics = DiagnosticsCollector::default();
    diagnostics.push(
        "file",
        syntax.reason,
        SettingsDiagnosticDisposition::Defaulted,
    );
    UserSettingsLoad {
        settings: UserSettings {
            path: Some(path.to_owned()),
            exists: true,
            ..UserSettings::default()
        },
        diagnostics: diagnostics.finish(),
        syntax: Some(syntax),
    }
}

fn parse(text: &str, path: &Path, env: &impl Fn(&str) -> Option<OsString>) -> UserSettingsLoad {
    let table: toml::Table = match toml::from_str(text) {
        Ok(table) => table,
        Err(error) => {
            let offset = error.span().map_or(0, |span| span.start);
            return unparsed(path, SettingsSyntaxError::at(text, offset, "invalid-toml"));
        }
    };
    let mut settings = UserSettings {
        path: Some(path.to_owned()),
        exists: true,
        ..UserSettings::default()
    };
    let mut diagnostics = DiagnosticsCollector::default();
    report_unknown(&table, "", &mut diagnostics);
    let mut fields = Fields {
        root: &table,
        diagnostics: &mut diagnostics,
    };
    parse_appearance(&mut fields, &mut settings);
    parse_terminal(&mut fields, &mut settings, path, env);
    if let Some(value) = fields.value("worktrees.directory") {
        let candidate = value
            .as_str()
            .ok_or(SettingsError::new("worktrees.directory", "wrong-type"))
            .and_then(|value| normalize(value, path, env, "worktrees.directory"));
        match candidate {
            Ok(value) => settings.worktree_root = Some(value),
            Err(error) => fields.push_error(&error),
        }
    }
    if let Some(value) = fields.value("update.channel") {
        match value.as_str() {
            Some("stable") => {
                settings.update_channel = UpdateChannel::Stable;
                settings.update_channel_source = "file";
            }
            Some("preview") => {
                settings.update_channel = UpdateChannel::Preview;
                settings.update_channel_source = "file";
            }
            Some(_) => fields.push("update.channel", "invalid-channel"),
            None => fields.push("update.channel", "wrong-type"),
        }
    }
    if let Some(value) = fields.boolean("agent_detection.auto_update") {
        settings.manifest_check = value;
        settings.manifest_check_source = "file";
    }
    settings.preferences = preferences::parse(&mut fields);
    settings.desktop = DesktopTerminalSettings::parse(&mut fields);
    if let Some(notifications) = table.get("notifications").and_then(toml::Value::as_table) {
        settings.notifications =
            NotificationSettings::parse_lenient(notifications, &mut diagnostics);
    }
    if let Some(keys) = table.get("keys") {
        settings.keys = KeySettings::parse_lenient(keys, &mut diagnostics);
    }
    UserSettingsLoad {
        settings,
        diagnostics: diagnostics.finish(),
        syntax: None,
    }
}

/// Report every key the registry does not know, and every registered table holding a value that
/// is not a table. A registered key's own value is left to its admission.
fn report_unknown(table: &toml::Table, parent: &str, diagnostics: &mut DiagnosticsCollector) {
    for (key, value) in table {
        if !user_config_has_child(parent, key) {
            diagnostics.push(
                unknown_field(parent, key),
                "unknown-key",
                SettingsDiagnosticDisposition::Ignored,
            );
            continue;
        }
        let segments: Vec<&str> = parent
            .split('.')
            .filter(|segment| !segment.is_empty())
            .chain([key.as_str()])
            .collect();
        if registry::find_setting(&segments).is_some() {
            continue;
        }
        let path = segments.join(".");
        match value.as_table() {
            Some(child) => report_unknown(child, &path, diagnostics),
            None => diagnostics.push(path, "wrong-type", SettingsDiagnosticDisposition::Defaulted),
        }
    }
}

/// Registry-typed reads from one parsed file; every refused value leaves its default and a
/// diagnostic.
struct Fields<'a> {
    root: &'a toml::Table,
    diagnostics: &'a mut DiagnosticsCollector,
}

impl<'a> Fields<'a> {
    /// The value at a dotted path whose segments are plain names; absent when any table on the
    /// way is missing or is not a table.
    fn value(&self, path: &str) -> Option<&'a toml::Value> {
        let mut segments = path.split('.');
        let mut value = self.root.get(segments.next()?)?;
        for segment in segments {
            value = value.as_table()?.get(segment)?;
        }
        Some(value)
    }

    fn push(&mut self, field: impl Into<String>, reason: &'static str) {
        self.diagnostics
            .push(field, reason, SettingsDiagnosticDisposition::Defaulted);
    }

    fn push_error(&mut self, error: &SettingsError) {
        self.diagnostics
            .push_error(error, SettingsDiagnosticDisposition::Defaulted);
    }

    /// The admitted value of one registered key, or `None` when it is absent or refused.
    fn scalar(&mut self, path: &str) -> Option<Scalar> {
        let segments: Vec<&str> = path.split('.').collect();
        let setting = registry::find_setting(&segments)?;
        let value = self.value(path)?;
        match setting.admit(value) {
            Ok(scalar) => Some(scalar),
            Err(reason) => {
                self.push(path.to_owned(), reason);
                None
            }
        }
    }

    fn boolean(&mut self, path: &str) -> Option<bool> {
        match self.scalar(path)? {
            Scalar::Boolean(value) => Some(value),
            Scalar::Integer(_) | Scalar::Text(_) => None,
        }
    }

    fn integer(&mut self, path: &str) -> Option<i64> {
        match self.scalar(path)? {
            Scalar::Integer(value) => Some(value),
            Scalar::Boolean(_) | Scalar::Text(_) => None,
        }
    }

    fn text(&mut self, path: &str) -> Option<String> {
        match self.scalar(path)? {
            Scalar::Text(value) => Some(value),
            Scalar::Boolean(_) | Scalar::Integer(_) => None,
        }
    }
}

fn parse_appearance(fields: &mut Fields<'_>, settings: &mut UserSettings) {
    if let Some(value) = fields.text("appearance.window_title") {
        match WindowTitleTemplate::parse(&value) {
            Ok(_) => settings.window_title = value,
            Err(reason) => fields.push("appearance.window_title", reason),
        }
    }
    if let Some(value) = fields.value("appearance.sidebar.agents") {
        match AgentSidebarSettings::parse(&registered_only(value, "appearance.sidebar.agents")) {
            Ok(profile) => settings.agent_sidebar = profile,
            Err(error) => fields.push_error(&error),
        }
    }
    if let Some(value) = fields.value("appearance.sidebar.workspaces") {
        match SpaceSidebarSettings::parse(&registered_only(value, "appearance.sidebar.workspaces"))
        {
            Ok(profile) => settings.space_sidebar = profile,
            Err(error) => fields.push_error(&error),
        }
    }
}

/// A table without the keys the registry does not know; those are reported as unknown already.
fn registered_only(value: &toml::Value, table_path: &str) -> toml::Value {
    let Some(table) = value.as_table() else {
        return value.clone();
    };
    let mut admitted = table.clone();
    admitted.retain(|key, _| user_config_has_child(table_path, key));
    toml::Value::Table(admitted)
}

fn parse_terminal(
    fields: &mut Fields<'_>,
    settings: &mut UserSettings,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) {
    if let Some(value) = fields.boolean("terminal.focus_new_panel") {
        settings.focus_new_panel = value;
    }
    let Some(value) = fields.value("terminal.new_cwd") else {
        return;
    };
    let Some(value) = value.as_str() else {
        fields.push("terminal.new_cwd", "wrong-type");
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
        Err(error) => fields.push_error(&error),
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
