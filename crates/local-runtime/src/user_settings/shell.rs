//! Validated literal Windows shell policy for startup and published launch defaults.
use super::SettingsError;
use std::{ffi::OsString, path::Path};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShellMode {
    #[default]
    Auto,
    Login,
    NonLogin,
}

impl ShellMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Login => "login",
            Self::NonLogin => "non_login",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShellSettings {
    executable: String,
    mode: ShellMode,
}

impl ShellSettings {
    pub fn new(executable: &str, mode: &str) -> Result<Self, SettingsError> {
        let executable = executable.trim();
        let invalid = || SettingsError::new("terminal.default_shell", "invalid-executable");
        if executable.len() > 4096
            || executable.chars().any(char::is_control)
            || executable.starts_with(['\\', '/'])
        {
            return Err(invalid());
        }
        let executable = if Path::new(executable).is_absolute() {
            crate::normalize_user_path(
                executable,
                |_| None,
                || Err(std::io::ErrorKind::InvalidInput.into()),
            )
            .map_err(|_| invalid())?
            .as_str()
            .to_owned()
        } else {
            if executable.contains(['/', '\\', ':', '"', '<', '>', '|', '?', '*'])
                || matches!(executable, "." | "..")
            {
                return Err(invalid());
            }
            executable.to_owned()
        };
        let mode = match mode {
            "auto" => ShellMode::Auto,
            "login" => ShellMode::Login,
            "non_login" => ShellMode::NonLogin,
            _ => {
                return Err(SettingsError::new("terminal.shell_mode", "invalid-mode"));
            }
        };
        Ok(Self { executable, mode })
    }

    pub fn executable(&self) -> &str {
        &self.executable
    }

    pub const fn mode(&self) -> ShellMode {
        self.mode
    }

    /// A matched private startup snapshot; the caller rejects any remaining arguments.
    pub fn from_private_arguments(
        arguments: &mut impl Iterator<Item = OsString>,
    ) -> Result<Self, SettingsError> {
        let mut value = |flag| {
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new(flag)) {
                return Err(SettingsError::new("shell", "invalid-snapshot"));
            }
            arguments
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or(SettingsError::new("shell", "invalid-snapshot"))
        };
        let executable = value("--default-shell")?;
        let mode = value("--shell-mode")?;
        Self::new(&executable, &mode)
    }
}
