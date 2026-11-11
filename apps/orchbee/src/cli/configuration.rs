//! Offline `config check` projection of `config.toml`; creation reads the Session snapshot instead.
use super::{Failure, FailureKind};
use crate::invocation::CreationDirectory;
#[cfg(test)]
use orchbee_local_runtime::UserSettings;
use orchbee_local_runtime::{
    DirectoryPolicy, SettingsDiagnostics, SettingsEditError, SettingsError, SettingsSyntaxError,
    UserSettingsLoad, read_user_settings,
};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug)]
pub(super) struct Settings {
    window_title: String,
    agent_sidebar: orchbee_local_runtime::AgentSidebarSettings,
    space_sidebar: orchbee_local_runtime::SpaceSidebarSettings,
    notifications: orchbee_local_runtime::NotificationSettings,
    keys: orchbee_local_runtime::KeySettings,
    pub(super) path: Option<PathBuf>,
    pub(super) exists: bool,
    cwd: Option<(CreationDirectory, Option<WorkbenchDirectoryPath>)>,
    worktree_root: Option<WorkbenchDirectoryPath>,
    manifest_check: bool,
    manifest_check_source: &'static str,
    pub(super) diagnostics: SettingsDiagnostics,
}

/// Why the settings could not be used: `file`, an environment variable or a settings key path, one
/// closed reason token and, for a file that does not parse, where it stops parsing.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Error {
    pub(super) field: String,
    pub(super) reason: &'static str,
    /// The one-based line and column of a syntax error; present exactly for `invalid-toml` and
    /// `invalid-utf8`.
    pub(super) position: Option<(usize, usize)>,
    /// The file a syntax error is in. `config check` names it; failure documents leave it out.
    pub(super) path: Option<PathBuf>,
}
impl Error {
    pub(super) fn new(field: impl Into<String>, reason: &'static str) -> Self {
        Self {
            field: field.into(),
            reason,
            position: None,
            path: None,
        }
    }

    pub(super) fn syntax(syntax: SettingsSyntaxError, path: Option<PathBuf>) -> Self {
        Self {
            position: Some((syntax.line, syntax.column)),
            path,
            ..Self::new("file", syntax.reason)
        }
    }

    /// The closed `{field, reason, line?, column?}` detail object.
    pub(super) fn details(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut details = serde_json::Map::from_iter([
            ("field".into(), self.field.clone().into()),
            ("reason".into(), self.reason.into()),
        ]);
        if let Some((line, column)) = self.position {
            details.insert("line".into(), line.into());
            details.insert("column".into(), column.into());
        }
        details
    }

    /// The failing `user-config` check's details: the closed detail object and, for a syntax
    /// error, the file it is in.
    pub(super) fn check_details(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut details = self.details();
        if let Some(path) = self.path.as_deref().and_then(std::path::Path::to_str) {
            details.insert("path".into(), path.into());
        }
        details
    }

    pub(super) fn failure(self) -> Failure {
        Failure::new(FailureKind::CheckFailed)
            .failure_schema("orchbee.cli.failure.configuration")
            .details(self.details())
    }
}

impl From<SettingsError> for Error {
    fn from(error: SettingsError) -> Self {
        Self::new(error.field, error.reason)
    }
}

/// Each edit refusal keeps its own reason: an edit-specific one names `file`, a refused value names
/// the reader's field, and a syntax error keeps its line and column.
impl From<SettingsEditError> for Error {
    fn from(error: SettingsEditError) -> Self {
        match error {
            SettingsEditError::UnknownKey { key } => Self::new(key, "unknown-key"),
            SettingsEditError::InvalidValue { field, reason, .. } => Self::new(field, reason),
            SettingsEditError::Syntax(syntax) => Self::syntax(syntax, None),
            SettingsEditError::Source(error) => error.into(),
            SettingsEditError::ReadOnly
            | SettingsEditError::InUse
            | SettingsEditError::Locked
            | SettingsEditError::Changed
            | SettingsEditError::TooLarge
            | SettingsEditError::Io(_) => Self::new("file", error.reason()),
        }
    }
}

/// The settings a command acts on; a file that does not parse is refused with its position.
pub(super) fn read_valid(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<UserSettingsLoad, Error> {
    let load = read_user_settings(env)?;
    match load.syntax {
        Some(syntax) => Err(Error::syntax(syntax, load.settings.path)),
        None => Ok(load),
    }
}

pub(super) fn load() -> Result<Settings, Error> {
    project(read_valid(|name| std::env::var_os(name))?)
}

fn project(load: UserSettingsLoad) -> Result<Settings, Error> {
    let UserSettingsLoad {
        settings: raw,
        diagnostics,
        ..
    } = load;
    let path = |value: String, field| {
        WorkbenchDirectoryPath::parse(&value).map_err(|_| Error::new(field, "invalid-path"))
    };
    Ok(Settings {
        window_title: raw.window_title,
        agent_sidebar: raw.agent_sidebar,
        space_sidebar: raw.space_sidebar,
        notifications: raw.notifications,
        keys: raw.keys,
        path: raw.path,
        exists: raw.exists,
        cwd: raw
            .cwd
            .map(|(policy, value)| {
                Ok::<_, Error>((
                    match policy {
                        DirectoryPolicy::Follow => CreationDirectory::Follow,
                        DirectoryPolicy::Home => CreationDirectory::Home,
                        DirectoryPolicy::Current => CreationDirectory::Current,
                        DirectoryPolicy::Workspace => CreationDirectory::Workspace,
                        DirectoryPolicy::Explicit => CreationDirectory::Explicit,
                    },
                    value
                        .map(|value| path(value, "terminal.new_cwd"))
                        .transpose()?,
                ))
            })
            .transpose()?,
        worktree_root: raw
            .worktree_root
            .map(|value| path(value, "worktrees.directory"))
            .transpose()?,
        manifest_check: raw.manifest_check,
        manifest_check_source: raw.manifest_check_source,
        diagnostics,
    })
}

impl Settings {
    pub(super) fn check_details(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut details = serde_json::Map::from_iter([
            (
                "spacesSidebarProfile".into(),
                self.space_sidebar.profile().into(),
            ),
            (
                "agentSidebarProfile".into(),
                self.agent_sidebar.profile().into(),
            ),
            (
                "notificationDelivery".into(),
                self.notifications.delivery.as_str().into(),
            ),
            (
                "notificationAgentDelivery".into(),
                self.notifications.agent_delivery.as_str().into(),
            ),
            (
                "notificationDelaySeconds".into(),
                self.notifications.delay_seconds.into(),
            ),
            (
                "notificationSoundEnabled".into(),
                self.notifications.sound_enabled.into(),
            ),
            (
                "notificationAgentSounds".into(),
                serde_json::Value::Object(serde_json::Map::from_iter(
                    self.notifications
                        .agent_sounds()
                        .iter()
                        .map(|(agent, setting)| (agent.clone(), setting.as_str().into())),
                )),
            ),
            (
                "notificationPosition".into(),
                self.notifications.position().into(),
            ),
            ("windowTitle".into(), self.window_title.clone().into()),
            (
                "windowTitleDiagnostic".into(),
                serde_json::json!(
                    orchbee_local_runtime::WindowTitleTemplate::parse(&self.window_title).err()
                ),
            ),
            ("keyOverrides".into(), self.keys.overrides.len().into()),
            ("keyPrefix".into(), {
                let bindings = crate::client_view::Bindings::configured(&self.keys);
                crate::client_view::format_configuration_binding(&bindings.prefix).into()
            }),
            ("manifestCheck".into(), self.manifest_check.into()),
            (
                "manifestCheckSource".into(),
                self.manifest_check_source.into(),
            ),
            (
                "path".into(),
                serde_json::json!(self.path.as_ref().and_then(|path| path.to_str())),
            ),
            ("exists".into(), self.exists.into()),
            (
                "newCwd".into(),
                serde_json::json!(
                    self.cwd
                        .as_ref()
                        .map(|(policy, path)| match policy {
                            CreationDirectory::Follow | CreationDirectory::Default => "follow",
                            CreationDirectory::Home => "home",
                            CreationDirectory::Current => "current",
                            CreationDirectory::Workspace => "workspace",
                            CreationDirectory::Explicit =>
                                path.as_ref().expect("normalized setting path").as_str(),
                        })
                        .unwrap_or("follow")
                ),
            ),
            (
                "worktreeDirectory".into(),
                serde_json::json!(self.worktree_root),
            ),
        ]);
        details.insert(
            "diagnostics".into(),
            serde_json::Value::Array(
                self.diagnostics
                    .items()
                    .iter()
                    .map(|diagnostic| {
                        serde_json::json!({
                            "field": diagnostic.field,
                            "reason": diagnostic.reason,
                            "disposition": diagnostic.disposition.as_str(),
                        })
                    })
                    .collect(),
            ),
        );
        details.insert(
            "diagnosticsOmitted".into(),
            self.diagnostics.omitted_count().into(),
        );
        details
    }
}

#[cfg(test)]
mod tests;
