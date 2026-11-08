//! Atomic channel preference editing; network/install effects belong to the CLI caller.
use super::super::{
    LIMIT, SettingsError, UpdateChannel, parse, read_user_settings_at, user_settings_path,
};
use crate::{DataRoot, PrivateCache};
use std::{ffi::OsString, path::PathBuf};

/// Validates the complete existing configuration, preserves TOML syntax, and atomically saves
/// only update.channel. An explicitly selected absent file may be created by this writer.
pub fn set_update_channel(
    channel: UpdateChannel,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, SettingsError> {
    let failure = || SettingsError::new("file", "io-error");
    let path = user_settings_path(&env)?.ok_or_else(failure)?;
    read_user_settings_at(Some(&path), false, &env)?;
    let parent = path.parent().ok_or_else(failure)?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(failure)?;
    // The writer lock stays in the data root's internal folder, so the folder holding the settings
    // gains only a staging file while the write runs.
    let lock = DataRoot::resolve(&env)
        .map_err(|_| failure())?
        .config_lock_directory();
    let mut cache =
        PrivateCache::acquire_with_lock_in(parent, "config", &lock).map_err(|_| failure())?;
    let bytes = cache
        .read(name, LIMIT as usize)
        .map_err(|_| failure())?
        .unwrap_or_default();
    let text =
        std::str::from_utf8(&bytes).map_err(|_| SettingsError::new("file", "invalid-utf8"))?;
    parse(text, &path, &env)?;
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| SettingsError::new("file", "invalid-toml"))?;
    if document.get("update").is_none() {
        document["update"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    let mut value = toml_edit::Value::from(channel.as_str());
    if let Some(previous) = document["update"]
        .get("channel")
        .and_then(toml_edit::Item::as_value)
    {
        *value.decor_mut() = previous.decor().clone();
    }
    document["update"]["channel"] = toml_edit::Item::Value(value);
    let after = document.to_string();
    if after.len() as u64 > LIMIT {
        return Err(SettingsError::new("file", "too-large"));
    }
    let settings = parse(&after, &path, &env)?;
    if settings.update_channel != channel {
        return Err(failure());
    }
    // Recheck a noncooperating editor before the atomic publication.
    if cache
        .read(name, LIMIT as usize)
        .map_err(|_| failure())?
        .unwrap_or_default()
        != bytes
    {
        return Err(failure());
    }
    cache
        .replace(name, after.as_bytes())
        .map_err(|_| failure())?;
    Ok(path)
}
