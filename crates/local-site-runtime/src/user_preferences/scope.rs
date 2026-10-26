//! Only startup adapters select the configuration identity; wire requests cannot supply paths.
use orchbee_workbench_protocol::PreferencesFailure;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPreferencesScope {
    pub(super) directory: PathBuf,
}
impl UserPreferencesScope {
    /// Freeze an absolute configuration identity. Distinct explicit files never share preferences.
    pub fn for_configuration(path: &Path) -> Result<Self, PreferencesFailure> {
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(PreferencesFailure::Unavailable);
        }
        let parent = path.parent().ok_or(PreferencesFailure::Unavailable)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(PreferencesFailure::Unavailable)?;
        #[cfg(windows)]
        let name = name.to_lowercase();
        let identity = format!("{:x}", Sha256::digest(name.as_bytes()));
        Ok(Self {
            directory: parent.join(format!("preferences-{identity}")),
        })
    }

    /// The managed preferences directory is not an arbitrary user-provided wire operand.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    #[cfg(windows)]
    pub fn from_environment(
        env: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, PreferencesFailure> {
        let path = crate::windows_user_settings_path(env)
            .map_err(|_| PreferencesFailure::Unavailable)?
            .ok_or(PreferencesFailure::Unavailable)?;
        Self::for_configuration(&path)
    }

    #[cfg(unix)]
    pub fn from_environment(
        env: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, PreferencesFailure> {
        let path = if let Some(explicit) = env("ORCHBEE_CONFIG_FILE") {
            PathBuf::from(explicit)
        } else {
            default_configuration(&env)?
        };
        Self::for_configuration(&path)
    }
}

#[cfg(target_os = "macos")]
fn default_configuration(
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, PreferencesFailure> {
    Ok(
        PathBuf::from(env("HOME").ok_or(PreferencesFailure::Unavailable)?)
            .join("Library/Application Support/Orchbee/config.toml"),
    )
}
#[cfg(all(unix, not(target_os = "macos")))]
fn default_configuration(
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, PreferencesFailure> {
    let base = match env("XDG_CONFIG_HOME") {
        Some(base) => PathBuf::from(base),
        None => PathBuf::from(env("HOME").ok_or(PreferencesFailure::Unavailable)?).join(".config"),
    };
    Ok(base.join("Orchbee/config.toml"))
}
