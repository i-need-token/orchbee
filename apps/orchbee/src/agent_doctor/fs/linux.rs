//! Linux no-follow descriptors, native ownership, environment and executable observations.
use super::{
    Budget, EntryKind, EnvValue, FileRead, Lstat, MAX_PATH_BYTES, Ownership, PathScan,
    observation::Native, path_from_bytes,
};
use nix::{
    errno::Errno,
    fcntl::OFlag,
    sys::stat::{FileStat, Mode, SFlag, fstat},
    unistd::{AccessFlags, access},
};
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{self, Read},
    os::{fd::OwnedFd, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
};

pub(super) static NATIVE: Native = Native {
    lstat,
    read_regular,
    read_dir: |path| fs::read_dir(path),
    reject_entry: |_| false,
    env_value,
    home_directory,
    inherited_path: || std::env::var_os("PATH"),
    current_executable: std::env::current_exe,
    scan_path_for,
    endpoint_user: || Some(nix::unistd::geteuid().as_raw()),
    codex_target: |provider, home| Some(home.join(provider.home_target)),
    guarded_target: |provider, home| Some(home.join(provider.home_target)),
    bootstrap_target: |provider, home| Some(home.join(provider.home_target)),
    configuration_targets: None,
};
pub(crate) fn lstat(path: &Path) -> Lstat {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Lstat::Absent,
        Err(_) => return Lstat::Unreadable,
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Lstat::Symlink;
    }
    let kind = if file_type.is_file() {
        EntryKind::Regular
    } else if file_type.is_dir() {
        EntryKind::Directory
    } else if file_type.is_socket() {
        EntryKind::Socket
    } else {
        EntryKind::Other
    };
    Lstat::Entry {
        kind,
        ownership: Some(Ownership {
            uid: metadata.uid(),
            mode: metadata.mode() & 0o7777,
        }),
        size: metadata.size(),
    }
}

pub(crate) fn read_regular(path: &Path, cap: u64, budget: &mut Budget) -> FileRead {
    let descriptor = match nix::fcntl::open(
        path,
        OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(Errno::ENOENT) => return FileRead::Absent,
        Err(Errno::ELOOP) => return FileRead::Symlink,
        Err(_) => return FileRead::Unreadable,
    };
    read_regular_descriptor(descriptor, cap, budget)
}

fn read_regular_descriptor(descriptor: OwnedFd, cap: u64, budget: &mut Budget) -> FileRead {
    let stat: FileStat = match fstat(&descriptor) {
        Ok(stat) => stat,
        Err(_) => return FileRead::Unreadable,
    };
    if SFlag::from_bits_truncate(stat.st_mode) != SFlag::S_IFREG {
        return FileRead::UnsafeKind;
    }
    if stat.st_size < 0 || stat.st_size as u64 > cap {
        return FileRead::OverLimit;
    }
    if stat.st_size as u64 > budget.remaining_bytes() {
        return FileRead::BudgetExhausted;
    }
    let limit = cap.min(budget.remaining_bytes()).saturating_add(1);
    let mut bytes = Vec::new();
    let file = File::from(descriptor);
    if file.take(limit).read_to_end(&mut bytes).is_err() {
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
        Some(value) => EnvValue::Value(value.as_bytes().to_vec()),
        None => EnvValue::Unset,
    }
}

pub(crate) fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub(crate) fn scan_path_for(name: &str, path_env: &OsStr) -> PathScan {
    if !valid_executable_name(name) {
        return PathScan {
            found: None,
            truncated: false,
        };
    }
    let mut truncated = false;
    let mut directories_seen = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    for directory in path_env.as_bytes().split(|byte| *byte == b':') {
        directories_seen += 1;
        if directories_seen > 256 {
            truncated = true;
            break;
        }
        if directory.is_empty()
            || directory.len() > MAX_PATH_BYTES
            || !directory.starts_with(b"/")
            || directory.iter().any(|byte| *byte <= 0x1f || *byte == 0x7f)
        {
            continue;
        }
        let mut candidate = directory.to_vec();
        candidate.push(b'/');
        candidate.extend_from_slice(name.as_bytes());
        if !seen.insert(candidate.clone()) {
            continue;
        }
        let candidate_path = path_from_bytes(&candidate);
        let executable = access(&candidate_path, AccessFlags::X_OK).is_ok()
            && fs::metadata(&candidate_path)
                .map(|metadata| metadata.is_file())
                .unwrap_or(false);
        if executable {
            return PathScan {
                found: Some(candidate),
                truncated,
            };
        }
    }
    PathScan {
        found: None,
        truncated,
    }
}

fn valid_executable_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}
