//! CLI projection of the shared Windows settings owner.
use super::{Failure, FailureKind};
use crate::invocation::CreationDirectory;
#[cfg(test)]
use orchbee_local_site_runtime::WindowsUserSettings;
use orchbee_local_site_runtime::{
    WindowsDirectoryPolicy, WindowsSettingsDiagnostics, WindowsSettingsError,
    WindowsUserSettingsLoad, read_windows_user_settings,
};
use orchbee_workbench_protocol::{LaunchOptions, WorkbenchDirectoryPath};
use std::path::PathBuf;

#[derive(Debug)]
pub(super) struct Settings {
    theme: orchbee_local_site_runtime::WindowsThemeName,
    window_title: String,
    agent_sidebar: orchbee_local_site_runtime::WindowsAgentSidebarSettings,
    space_sidebar: orchbee_local_site_runtime::WindowsSpaceSidebarSettings,
    notifications: orchbee_local_site_runtime::WindowsNotificationSettings,
    shell: orchbee_local_site_runtime::WindowsShellSettings,
    keys: orchbee_local_site_runtime::WindowsKeySettings,
    pub(super) path: Option<PathBuf>,
    pub(super) exists: bool,
    pub(super) cwd: Option<(CreationDirectory, Option<WorkbenchDirectoryPath>)>,
    pub(super) worktree_root: Option<WorkbenchDirectoryPath>,
    resume_agents_on_restore: bool,
    manifest_check: bool,
    manifest_check_source: &'static str,
    resume_agents_on_restore_source: &'static str,
    pub(super) diagnostics: WindowsSettingsDiagnostics,
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

impl From<WindowsSettingsError> for Error {
    fn from(error: WindowsSettingsError) -> Self {
        Self::new(error.field, error.reason)
    }
}

pub(super) fn load() -> Result<Settings, Error> {
    project(read_windows_user_settings(|name| std::env::var_os(name))?)
}

fn project(load: WindowsUserSettingsLoad) -> Result<Settings, Error> {
    let WindowsUserSettingsLoad {
        settings: raw,
        diagnostics,
    } = load;
    let path = |value: String, field| {
        WorkbenchDirectoryPath::parse(&value).map_err(|_| Error::new(field, "invalid-path"))
    };
    Ok(Settings {
        theme: raw.theme,
        window_title: raw.window_title,
        agent_sidebar: raw.agent_sidebar,
        space_sidebar: raw.space_sidebar,
        notifications: raw.notifications,
        shell: raw.shell,
        keys: raw.keys,
        path: raw.path,
        exists: raw.exists,
        cwd: raw
            .cwd
            .map(|(policy, value)| {
                Ok::<_, Error>((
                    match policy {
                        WindowsDirectoryPolicy::Follow => CreationDirectory::Follow,
                        WindowsDirectoryPolicy::Home => CreationDirectory::Home,
                        WindowsDirectoryPolicy::Current => CreationDirectory::Current,
                        WindowsDirectoryPolicy::Workspace => CreationDirectory::Workspace,
                        WindowsDirectoryPolicy::Explicit => CreationDirectory::Explicit,
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
        resume_agents_on_restore: raw.resume_agents_on_restore,
        manifest_check: raw.manifest_check,
        manifest_check_source: raw.manifest_check_source,
        resume_agents_on_restore_source: raw.resume_agents_on_restore_source,
        diagnostics,
    })
}

impl Settings {
    pub(super) fn apply_directory(
        &self,
        directory: &mut CreationDirectory,
        options: &mut LaunchOptions,
    ) {
        if *directory == CreationDirectory::Default
            && let Some((policy, path)) = &self.cwd
        {
            *directory = *policy;
            options.cwd = path.clone();
        }
    }
    pub(super) fn check_details(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut details = serde_json::Map::from_iter([
            ("themeName".into(), self.theme.as_str().into()),
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
                    orchbee_local_site_runtime::WindowsWindowTitleTemplate::parse(
                        &self.window_title
                    )
                    .err()
                ),
            ),
            ("keyOverrides".into(), self.keys.overrides.len().into()),
            ("keyPrefix".into(), {
                let bindings = crate::client_view::Bindings::configured(&self.keys);
                crate::client_view::format_configuration_binding(&bindings.prefix).into()
            }),
            ("defaultShell".into(), self.shell.executable().into()),
            ("shellMode".into(), self.shell.mode().as_str().into()),
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
                "resumeAgentsOnRestore".into(),
                self.resume_agents_on_restore.into(),
            ),
            (
                "resumeAgentsOnRestoreSource".into(),
                self.resume_agents_on_restore_source.into(),
            ),
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
