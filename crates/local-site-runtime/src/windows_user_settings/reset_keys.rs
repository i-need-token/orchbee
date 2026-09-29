//! Explicit offline repair; read-only settings admission never invokes this writer.
use super::{LIMIT, WindowsSettingsError, windows_user_settings_path};
use crate::windows_permissions as permissions;
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct WindowsKeysResetReport {
    pub path: Option<PathBuf>,
    pub outcome: WindowsKeysResetOutcome,
}

#[derive(Debug)]
pub enum WindowsKeysResetOutcome {
    Missing,
    NoKeys,
    Reset {
        backup_path: PathBuf,
    },
    Failed {
        error: WindowsSettingsError,
        backup_path: Option<PathBuf>,
        recovery_required: bool,
    },
}

fn failed(field: &'static str, reason: &'static str) -> WindowsKeysResetOutcome {
    WindowsKeysResetOutcome::Failed {
        error: WindowsSettingsError::new(field, reason),
        backup_path: None,
        recovery_required: false,
    }
}

/// Does not activate a Site or apply the resulting file to any running Client.
pub fn reset_windows_keybindings(env: impl Fn(&str) -> Option<OsString>) -> WindowsKeysResetReport {
    match windows_user_settings_path(env) {
        Err(error) => WindowsKeysResetReport {
            path: None,
            outcome: failed(error.field, error.reason),
        },
        Ok(path) => {
            let outcome = path
                .as_deref()
                .map_or(WindowsKeysResetOutcome::Missing, reset_at);
            WindowsKeysResetReport { path, outcome }
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

fn read(file: &mut File) -> Result<String, WindowsSettingsError> {
    if file
        .metadata()
        .map_err(|_| WindowsSettingsError::new("file", "io-error"))?
        .len()
        > LIMIT
    {
        return Err(WindowsSettingsError::new("file", "too-large"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WindowsSettingsError::new("file", "io-error"))?;
    if bytes.len() as u64 > LIMIT {
        return Err(WindowsSettingsError::new("file", "too-large"));
    }
    String::from_utf8(bytes).map_err(|_| WindowsSettingsError::new("file", "invalid-utf8"))
}

fn reset_at(path: &Path) -> WindowsKeysResetOutcome {
    let mut original = match open(path, false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return WindowsKeysResetOutcome::Missing;
        }
        Err(_) => return failed("file", "unavailable"),
    };
    let before = match read(&mut original) {
        Ok(text) => text,
        Err(error) => return failed(error.field, error.reason),
    };
    let after = match without_keys(&before) {
        Ok(Some(text)) => text,
        Ok(None) => return WindowsKeysResetOutcome::NoKeys,
        Err(error) => return failed(error.field, error.reason),
    };
    apply_prepared(path, original, &before, &after)
}

fn apply_prepared(
    path: &Path,
    original: File,
    before: &str,
    after: &str,
) -> WindowsKeysResetOutcome {
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
        &mut original,
        before.as_bytes(),
        after.as_bytes(),
        replace,
    )
}

/// Byte-preserving edits are accepted only if the whole TOML equals the original minus keys.
fn without_keys(content: &str) -> Result<Option<String>, WindowsSettingsError> {
    let mut expected: toml::Table =
        toml::from_str(content).map_err(|_| WindowsSettingsError::new("file", "invalid-toml"))?;
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
        toml::from_str(&result).map_err(|_| WindowsSettingsError::new("keys", "unsafe-edit"))?;
    if actual != expected {
        return Err(WindowsSettingsError::new("keys", "unsafe-edit"));
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
    original: &mut File,
    before: &[u8],
    after: &[u8],
    mut write: impl FnMut(&mut File, &[u8]) -> io::Result<()>,
) -> WindowsKeysResetOutcome {
    let (backup_path, mut backup) = match create_backup(path) {
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
        return WindowsKeysResetOutcome::Reset { backup_path };
    }
    if write(original, before).is_ok() {
        cleanup_failure("file", "write-failed-restored", backup_path)
    } else {
        WindowsKeysResetOutcome::Failed {
            error: WindowsSettingsError::new("file", "restore-required"),
            backup_path: Some(backup_path),
            recovery_required: true,
        }
    }
}

fn cleanup_failure(
    field: &'static str,
    reason: &'static str,
    path: PathBuf,
) -> WindowsKeysResetOutcome {
    let residue = fs::remove_file(&path).err().map(|_| path);
    WindowsKeysResetOutcome::Failed {
        error: WindowsSettingsError::new(field, reason),
        backup_path: residue,
        recovery_required: false,
    }
}

fn create_backup(path: &Path) -> io::Result<(PathBuf, File)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    for _ in 0..16 {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let candidate = path.with_file_name(format!(
            "{name}.bak-keys-{}-{timestamp}-{index}",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

#[cfg(test)]
mod tests;
