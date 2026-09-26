//! Windows CLI settings: one bounded read, strict typed projection, no writes or cache.
use super::{Failure, FailureKind};
use crate::invocation::{CreationDirectory, user_path_windows};
use orchbee_workbench_protocol::{LaunchOptions, WorkbenchDirectoryPath};
use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

const LIMIT: u64 = 64 * 1024;

#[derive(Debug, Default)]
pub(super) struct Settings {
    pub(super) path: Option<PathBuf>,
    pub(super) exists: bool,
    pub(super) cwd: Option<(CreationDirectory, Option<WorkbenchDirectoryPath>)>,
    pub(super) worktree_root: Option<WorkbenchDirectoryPath>,
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

pub(super) fn load() -> Result<Settings, Error> {
    read(|name| std::env::var_os(name))
}

fn location(env: &impl Fn(&str) -> Option<OsString>) -> Result<Option<(PathBuf, bool)>, Error> {
    let (value, field, explicit) = if let Some(value) = env("ORCHBEE_CONFIG_FILE") {
        (value, "ORCHBEE_CONFIG_FILE", true)
    } else if let Some(value) = env("APPDATA") {
        (value, "APPDATA", false)
    } else {
        return Ok(None);
    };
    let raw = value.to_str().ok_or(Error::new(field, "invalid-path"))?;
    let path = Path::new(raw);
    if !path.is_absolute() || raw.is_empty() {
        return Err(Error::new(field, "invalid-path"));
    }
    let checked =
        user_path_windows::normalize(raw, env, || Err(std::io::ErrorKind::NotFound.into()))
            .map_err(|_| Error::new(field, "invalid-path"))?;
    let path = PathBuf::from(checked.as_str());
    let path = if explicit {
        path
    } else {
        path.join("Orchbee/config.toml")
    };
    WorkbenchDirectoryPath::parse(path.to_str().ok_or(Error::new(field, "invalid-path"))?)
        .map_err(|_| Error::new(field, "invalid-path"))?;
    Ok(Some((path, explicit)))
}

fn read(env: impl Fn(&str) -> Option<OsString>) -> Result<Settings, Error> {
    let Some((path, explicit)) = location(&env)? else {
        return Ok(Settings::default());
    };
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !explicit => {
            return Ok(Settings {
                path: Some(path),
                ..Settings::default()
            });
        }
        Err(error) => {
            return Err(Error::new(
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
        .map_err(|_| Error::new("file", "io-error"))?;
    if !metadata.is_file() {
        return Err(Error::new("file", "not-file"));
    }
    if metadata.len() > LIMIT {
        return Err(Error::new("file", "too-large"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new("file", "io-error"))?;
    if bytes.len() as u64 > LIMIT {
        return Err(Error::new("file", "too-large"));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::new("file", "invalid-utf8"))?;
    parse(text, &path, &env)
}

fn parse(
    text: &str,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<Settings, Error> {
    let table: toml::Table =
        toml::from_str(text).map_err(|_| Error::new("file", "invalid-toml"))?;
    let mut settings = Settings {
        path: Some(path.to_owned()),
        exists: true,
        ..Settings::default()
    };
    for (section, value) in table {
        let (key, field) = match section.as_str() {
            "terminal" => ("new_cwd", "terminal.new_cwd"),
            "worktrees" => ("directory", "worktrees.directory"),
            _ => return Err(Error::new("file", "unknown-key")),
        };
        let table = value.as_table().ok_or(Error::new(field, "wrong-type"))?;
        if table.keys().any(|entry| entry != key) {
            return Err(Error::new(field, "unknown-key"));
        }
        let Some(value) = table.get(key) else {
            continue;
        };
        let value = value.as_str().ok_or(Error::new(field, "wrong-type"))?;
        if section == "terminal" {
            settings.cwd = Some(match value.trim() {
                "" | "follow" => (CreationDirectory::Follow, None),
                "home" => (CreationDirectory::Home, None),
                "current" => (CreationDirectory::Current, None),
                "workspace" => (CreationDirectory::Workspace, None),
                _ => (
                    CreationDirectory::Explicit,
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
) -> Result<WorkbenchDirectoryPath, Error> {
    if value.trim().is_empty() {
        return Err(Error::new(field, "invalid-path"));
    }
    user_path_windows::normalize(value, env, || {
        file.parent()
            .map(Path::to_owned)
            .ok_or_else(|| std::io::ErrorKind::NotFound.into())
    })
    .map_err(|_| Error::new(field, "invalid-path"))
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
