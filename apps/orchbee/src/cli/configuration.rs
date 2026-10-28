//! Offline `config check` projection of `config.toml`; creation reads the Session snapshot instead.
use super::{Failure, FailureKind};
use crate::invocation::CreationDirectory;
#[cfg(test)]
use orchbee_local_runtime::UserSettings;
use orchbee_local_runtime::{
    DirectoryPolicy, SettingsDiagnostics, SettingsError, UserSettingsLoad, read_user_settings,
};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;
use std::path::PathBuf;

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

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Error {
    pub(super) field: &'static str,
    pub(super) reason: &'static str,
}
impl Error {
    pub(super) const fn new(field: &'static str, reason: &'static str) -> Self {
        Self { field, reason }
    }
    pub(super) fn failure(self) -> Failure {
        Failure::new(FailureKind::CheckFailed)
            .failure_schema("orchbee.cli.failure.configuration")
            .details(serde_json::Map::from_iter([
                ("field".into(), self.field.into()),
                ("reason".into(), self.reason.into()),
            ]))
    }
}

impl From<SettingsError> for Error {
    fn from(error: SettingsError) -> Self {
        Self::new(error.field, error.reason)
    }
}

pub(super) fn load() -> Result<Settings, Error> {
    project(read_user_settings(|name| std::env::var_os(name))?)
}

fn project(load: UserSettingsLoad) -> Result<Settings, Error> {
    let UserSettingsLoad {
        settings: raw,
        diagnostics,
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
