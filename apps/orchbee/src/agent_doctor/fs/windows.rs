//! Windows observations only: no process execution or pipe connection.
use std::{
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{self, Read},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf, Prefix},
};

use super::native::{ConfigurationTarget, Native};
use super::{Budget, EntryKind, EnvValue, FileRead, Lstat, MAX_PATH_BYTES, PathScan};

pub(super) static NATIVE: Native = Native {
    lstat,
    read_regular,
    read_dir: |path| fs::read_dir(path),
    reject_entry: |entry| matches!(lstat(&entry.path()), Lstat::Symlink | Lstat::Unreadable),
    env_value,
    home_directory,
    inherited_path: || std::env::var_os("PATH"),
    current_executable: std::env::current_exe,
    scan_path_for,
    endpoint_user: || None,
    codex_target: |_, home| codex_configuration_path(home),
    guarded_target: |provider, home| {
        orchbee_agent_integration_writer::extension_directory(provider.id, home)
    },
    bootstrap_target: |provider, home| {
        if provider.id == "opencode" {
            orchbee_agent_integration_writer::opencode_entries(home).map(|paths| paths.current)
        } else if provider.id == "kilo" {
            orchbee_agent_integration_writer::kilo_entry(home)
        } else {
            Some(home.join(provider.home_target))
        }
    },
    configuration_targets: Some(|provider, home, kinds| {
        orchbee_agent_integration_writer::hook_configuration_candidates_for(
            provider.id,
            home,
            kinds,
        )
        .map(|targets| {
            targets
                .into_iter()
                .map(|target| ConfigurationTarget {
                    path: target.path,
                    kind: target.kind,
                    root_policy: orchbee_agent_integration_writer::CONFIGURATION_ROOT_POLICY,
                })
                .collect()
        })
    }),
};

const REPARSE_POINT: u32 = 0x400;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;

pub(crate) fn path_admitted(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if text.is_empty()
        || text.len() > MAX_PATH_BYTES
        || text.chars().any(char::is_control)
        || !path.is_absolute()
    {
        return false;
    }
    if !matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_) | Prefix::UNC(..) | Prefix::VerbatimUNC(..)))
    {
        return false;
    }
    if matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::UNC(_, share) | Prefix::VerbatimUNC(_, share) if share.eq_ignore_ascii_case(OsStr::new("pipe"))))
    {
        return false;
    }
    path.components().all(|component| match component {
        Component::Normal(name) => {
            let Some(name) = name.to_str() else {
                return false;
            };
            let stem = name
                .split('.')
                .next()
                .unwrap_or_default()
                .trim_end_matches(' ')
                .to_ascii_uppercase();
            !name.chars().any(|ch| ch.is_control() || ch == ':')
                && !matches!(
                    stem.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                )
                && !(stem.starts_with("COM") || stem.starts_with("LPT"))
                    .then(|| &stem[3..])
                    .is_some_and(|suffix| {
                        matches!(
                            suffix,
                            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                        )
                    })
        }
        _ => true,
    })
}

pub(crate) fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    // Invalid UTF-8 remains in the document as hex. The empty path is never
    // admitted for IO, so it cannot accidentally select another file.
    std::str::from_utf8(bytes)
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub(crate) fn lstat(path: &Path) -> Lstat {
    if !path_admitted(path) {
        return Lstat::Unreadable;
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Lstat::Absent,
        Err(_) => return Lstat::Unreadable,
    };
    if metadata.file_attributes() & REPARSE_POINT != 0 {
        return Lstat::Symlink;
    }
    Lstat::Entry {
        kind: if metadata.is_file() {
            EntryKind::Regular
        } else if metadata.is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::Other
        },
        ownership: None,
        size: metadata.len(),
    }
}

pub(crate) fn read_regular(path: &Path, cap: u64, budget: &mut Budget) -> FileRead {
    if !path_admitted(path) {
        return FileRead::UnsafeKind;
    }
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return FileRead::Absent,
        Err(_) => return FileRead::Unreadable,
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return FileRead::Unreadable,
    };
    if metadata.file_attributes() & REPARSE_POINT != 0 {
        return FileRead::Symlink;
    }
    if !metadata.is_file() {
        return FileRead::UnsafeKind;
    }
    if metadata.len() > cap {
        return FileRead::OverLimit;
    }
    if metadata.len() > budget.remaining_bytes() {
        return FileRead::BudgetExhausted;
    }
    let mut bytes = Vec::new();
    if file
        .take(cap.min(budget.remaining_bytes()).saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
    {
        return FileRead::Unreadable;
    }
    if bytes.len() as u64 > cap {
        return FileRead::OverLimit;
    }
    if bytes.len() as u64 > budget.remaining_bytes() {
        return FileRead::BudgetExhausted;
    }
    budget.consume(bytes.len() as u64);
    FileRead::Bytes(bytes)
}

pub(crate) fn env_value(name: &str) -> EnvValue {
    match std::env::var_os(name) {
        Some(value) => EnvValue::Value(value.as_encoded_bytes().to_vec()),
        None => EnvValue::Unset,
    }
}

pub(crate) fn home_directory() -> Option<PathBuf> {
    std::env::home_dir()
}

pub(crate) fn codex_configuration_path(home: &Path) -> Option<PathBuf> {
    match std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        Some(value) => {
            let root = PathBuf::from(value);
            matches!(
                lstat(&root),
                Lstat::Entry {
                    kind: EntryKind::Directory,
                    ..
                }
            )
            .then(|| root.join("config.toml"))
        }
        None => {
            let root = home.join(".codex");
            path_admitted(&root).then(|| root.join("config.toml"))
        }
    }
}

pub(crate) fn scan_path_for(name: &str, path_env: &OsStr) -> PathScan {
    let mut result = PathScan {
        found: None,
        truncated: false,
    };
    if name.is_empty()
        || name.len() > 32
        || !name.as_bytes()[0].is_ascii_lowercase()
        || !name
            .bytes()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'-')
    {
        return result;
    }
    // Fixed Windows command entry types, not arbitrary PATHEXT executability.
    // Presence is an inspection fact, never permission to execute a script.
    for (index, directory) in std::env::split_paths(path_env).enumerate() {
        if index >= 256 {
            result.truncated = true;
            break;
        }
        if !path_admitted(&directory) {
            continue;
        }
        for extension in ["exe", "com", "cmd", "bat", "ps1"] {
            let path = directory.join(format!("{name}.{extension}"));
            if path_admitted(&path) && fs::metadata(&path).is_ok_and(|metadata| metadata.is_file())
            {
                result.found = Some(path.as_os_str().as_encoded_bytes().to_vec());
                return result;
            }
        }
    }
    result
}
