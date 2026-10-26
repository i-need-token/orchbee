//! Key repair results are portable; only the selected platform writer may perform file effects.
use super::SettingsError;
use std::path::PathBuf;
#[derive(Debug)]
pub struct KeysResetReport {
    pub path: Option<PathBuf>,
    pub outcome: KeysResetOutcome,
}

#[derive(Debug)]
pub enum KeysResetOutcome {
    Missing,
    NoKeys,
    Reset {
        backup_path: PathBuf,
    },
    Failed {
        error: SettingsError,
        backup_path: Option<PathBuf>,
        recovery_required: bool,
    },
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::reset_keybindings;
#[cfg(not(windows))]
pub fn reset_keybindings(_env: impl Fn(&str) -> Option<std::ffi::OsString>) -> KeysResetReport {
    KeysResetReport {
        path: None,
        outcome: KeysResetOutcome::Failed {
            error: SettingsError::new("platform", "unsupported-platform"),
            backup_path: None,
            recovery_required: false,
        },
    }
}
