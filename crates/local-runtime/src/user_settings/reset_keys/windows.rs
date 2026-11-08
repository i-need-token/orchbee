//! Explicit offline repair; read-only settings admission never invokes this writer.
use super::super::{LIMIT, SettingsError, user_settings_path};
use crate::{DataRoot, private_filesystem as permissions};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{KeysResetOutcome, KeysResetReport};

fn failed(field: &'static str, reason: &'static str) -> KeysResetOutcome {
    KeysResetOutcome::Failed {
        error: SettingsError::new(field, reason),
        backup_path: None,
        recovery_required: false,
    }
}

/// Does not activate a Session or apply the resulting file to any running Client. The backup goes
/// to `internal/backups/<YYYY-MM-DD>-reset-keys/` in the data root, whichever file it copies.
pub fn reset_keybindings(env: impl Fn(&str) -> Option<OsString>) -> KeysResetReport {
    let backups = DataRoot::resolve(&env).ok().map(|root| root.backups());
    match user_settings_path(env) {
        Err(error) => KeysResetReport {
            path: None,
            outcome: failed(error.field, error.reason),
        },
        Ok(path) => {
            let outcome = path.as_deref().map_or(KeysResetOutcome::Missing, |path| {
                reset_at(path, backups.as_deref())
            });
            KeysResetReport { path, outcome }
        }
    }
}

fn open(path: &Path, write: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(write)
        .share_mode(0)
        .custom_flags(0x0020_0000) // Open the reparse point itself, never its target.
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.file_attributes() & 0x400 != 0
        || winapi_util::file::information(&file)?.number_of_links() != 1
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(file)
}

fn read(file: &mut File) -> Result<String, SettingsError> {
    if file
        .metadata()
        .map_err(|_| SettingsError::new("file", "io-error"))?
        .len()
        > LIMIT
    {
        return Err(SettingsError::new("file", "too-large"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SettingsError::new("file", "io-error"))?;
    if bytes.len() as u64 > LIMIT {
        return Err(SettingsError::new("file", "too-large"));
    }
    String::from_utf8(bytes).map_err(|_| SettingsError::new("file", "invalid-utf8"))
}

fn reset_at(path: &Path, backups: Option<&Path>) -> KeysResetOutcome {
    let mut original = match open(path, false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return KeysResetOutcome::Missing;
        }
        Err(_) => return failed("file", "unavailable"),
    };
    let before = match read(&mut original) {
        Ok(text) => text,
        Err(error) => return failed(error.field, error.reason),
    };
    let after = match without_keys(&before) {
        Ok(Some(text)) => text,
        Ok(None) => return KeysResetOutcome::NoKeys,
        Err(error) => return failed(error.field, error.reason),
    };
    let Some(backups) = backups else {
        return failed("backup", "create-failed");
    };
    apply_prepared(path, backups, original, &before, &after)
}

fn apply_prepared(
    path: &Path,
    backups: &Path,
    original: File,
    before: &str,
    after: &str,
) -> KeysResetOutcome {
    let Some(parent) = path.parent() else {
        return failed("file", "invalid-path");
    };
    let Ok(_pins) = permissions::open_tree(parent) else {
        return failed("file", "unsafe-parent");
    };
    drop(original);
    let mut original = match open(path, true) {
        Ok(file) => file,
        Err(_) => return failed("file", "unavailable"),
    };
    match read(&mut original) {
        Ok(current) if current == before => {}
        Ok(_) => return failed("file", "changed"),
        Err(error) => return failed(error.field, error.reason),
    }
    commit(
        path,
        backups,
        &mut original,
        before.as_bytes(),
        after.as_bytes(),
        replace,
    )
}

/// Byte-preserving edits are accepted only if the whole TOML equals the original minus keys.
fn without_keys(content: &str) -> Result<Option<String>, SettingsError> {
    let mut expected: toml::Table =
        toml::from_str(content).map_err(|_| SettingsError::new("file", "invalid-toml"))?;
    if expected.remove("keys").is_none() {
        return Ok(None);
    }
    let mut result = String::new();
    let mut in_table = false;
    let mut skipping = false;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[')
            && let Ok(header) = toml::from_str::<toml::Table>(trimmed)
            && !header.is_empty()
        {
            in_table = true;
            skipping = header.len() == 1 && header.contains_key("keys");
        } else if !in_table
            && toml::from_str::<toml::Table>(trimmed)
                .is_ok_and(|table| table.len() == 1 && table.contains_key("keys"))
        {
            continue;
        }
        if !skipping {
            result.push_str(line);
        }
    }
    let actual: toml::Table =
        toml::from_str(&result).map_err(|_| SettingsError::new("keys", "unsafe-edit"))?;
    if actual != expected {
        return Err(SettingsError::new("keys", "unsafe-edit"));
    }
    Ok(Some(result))
}

fn replace(file: &mut File, bytes: &[u8]) -> io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    file.write_all(bytes)?;
    file.set_len(bytes.len() as u64)?;
    file.sync_all()
}

fn commit(
    path: &Path,
    backups: &Path,
    original: &mut File,
    before: &[u8],
    after: &[u8],
    mut write: impl FnMut(&mut File, &[u8]) -> io::Result<()>,
) -> KeysResetOutcome {
    let (backup_path, mut backup) = match create_backup(path, backups) {
        Ok(value) => value,
        Err(_) => return failed("backup", "create-failed"),
    };
    // No source bytes reach an unverified inherited ACL.
    let stored = permissions::require_private(&backup)
        .and_then(|()| backup.write_all(before))
        .and_then(|()| backup.sync_all());
    drop(backup);
    if stored.is_err() {
        return cleanup_failure("backup", "write-failed", backup_path);
    }
    if write(original, after).is_ok() {
        return KeysResetOutcome::Reset { backup_path };
    }
    if write(original, before).is_ok() {
        cleanup_failure("file", "write-failed-restored", backup_path)
    } else {
        KeysResetOutcome::Failed {
            error: SettingsError::new("file", "restore-required"),
            backup_path: Some(backup_path),
            recovery_required: true,
        }
    }
}

fn cleanup_failure(field: &'static str, reason: &'static str, path: PathBuf) -> KeysResetOutcome {
    let residue = fs::remove_file(&path).err().map(|_| path.clone());
    if residue.is_none()
        && let Some(folder) = path.parent()
    {
        // The dated folder held only this copy; a folder that cannot go stays empty.
        let _ = fs::remove_dir(folder);
    }
    KeysResetOutcome::Failed {
        error: SettingsError::new(field, reason),
        backup_path: residue,
        recovery_required: false,
    }
}

/// Creates the next free `<YYYY-MM-DD>-reset-keys` folder, then `-2`, `-3` and so on, as a private
/// folder below `backups`, and an exclusive copy target named like the settings file inside it.
fn create_backup(path: &Path, backups: &Path) -> io::Result<(PathBuf, File)> {
    let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_millis();
    let date = crate::local_clock(u64::try_from(now).map_err(io::Error::other)?)
        .ok_or(io::ErrorKind::Unsupported)?;
    let stem = format!(
        "{:04}-{:02}-{:02}-reset-keys",
        date.year, date.month, date.day
    );
    let _pins = crate::data_root::create_state_directory(backups)?;
    for index in 1..=BACKUP_FOLDER_LIMIT {
        let folder = backups.join(if index == 1 {
            stem.clone()
        } else {
            format!("{stem}-{index}")
        });
        match fs::create_dir(&folder) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
        let mut directory = permissions::open_directory(&folder, true)?;
        permissions::protect_new(&mut directory)?;
        let candidate = folder.join(name);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&candidate)?;
        return Ok((candidate, file));
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

/// Resets on one day beyond this many leave the settings unchanged instead of searching on.
const BACKUP_FOLDER_LIMIT: u32 = 1000;

#[cfg(test)]
mod tests;
