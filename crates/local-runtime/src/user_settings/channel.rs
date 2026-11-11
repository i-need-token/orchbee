//! Release-channel writes; downloads and installation remain application-owned.
use super::{SettingsError, UpdateChannel, set_setting};
use std::{ffi::OsString, path::PathBuf};

/// Save only `update.channel` through the settings editor, keeping the rest of the file as it is.
/// A missing file is created holding just that key.
pub fn set_update_channel(
    channel: UpdateChannel,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, SettingsError> {
    set_setting(
        &["update", "channel"],
        &toml::Value::String(channel.as_str().into()),
        env,
    )
    .map_err(SettingsError::from)
}
