//! Release-channel writes; downloads and installation remain application-owned.
use super::{SettingsEditError, UpdateChannel, set_setting};
use std::{ffi::OsString, path::PathBuf};

/// Save only `update.channel` through the settings editor, keeping the rest of the file as it is.
/// A missing file is created holding just that key.
///
/// # Errors
/// Returns the editor's typed refusal; nothing is written then.
pub fn set_update_channel(
    channel: UpdateChannel,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, SettingsEditError> {
    set_setting(
        &["update", "channel"],
        &toml::Value::String(channel.as_str().into()),
        env,
    )
}
