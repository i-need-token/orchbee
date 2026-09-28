//! CLI projection of the shared Windows settings owner.
use super::{Failure, FailureKind};
use crate::invocation::CreationDirectory;
use orchbee_local_site_runtime::{
    WindowsDirectoryPolicy, WindowsSettingsError, WindowsUserSettings, read_windows_user_settings,
};
use orchbee_workbench_protocol::{LaunchOptions, WorkbenchDirectoryPath};
use std::path::PathBuf;

#[derive(Debug)]
pub(super) struct Settings {
    pub(super) path: Option<PathBuf>,
    pub(super) exists: bool,
    pub(super) cwd: Option<(CreationDirectory, Option<WorkbenchDirectoryPath>)>,
    pub(super) worktree_root: Option<WorkbenchDirectoryPath>,
    resume_agents_on_restore: bool,
    resume_agents_on_restore_source: &'static str,
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

fn project(raw: WindowsUserSettings) -> Result<Settings, Error> {
    let path = |value: String, field| {
        WorkbenchDirectoryPath::parse(&value).map_err(|_| Error::new(field, "invalid-path"))
    };
    Ok(Settings {
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
        resume_agents_on_restore_source: raw.resume_agents_on_restore_source,
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
        serde_json::Map::from_iter([
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
        ])
    }
}

#[cfg(test)]
mod tests;
