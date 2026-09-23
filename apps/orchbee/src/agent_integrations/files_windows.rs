//! Managed filesystem effects on Windows. Ancestors are opened without
//! delete sharing; publication and backup moves never replace an arrival.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf, Prefix},
};

const REPARSE: u32 = 0x400;
const OPEN_REPARSE: u32 = 0x0020_0000;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_LIMIT: u64 = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FsStep {
    Done,
    Conflict,
    Failed,
}

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsafe managed integration path",
    )
}

pub(super) fn path_admitted(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if text.len() > 4096
        || text.chars().any(char::is_control)
        || !path.is_absolute()
        || text
            .split(['\\', '/'])
            .any(|part| matches!(part, "." | ".."))
        || !matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    {
        return false;
    }
    path.components().all(|part| match part {
        Component::Normal(name) => name.to_str().is_some_and(|name| {
            let stem = name
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            !name.contains([':', '*', '?', '"', '<', '>', '|'])
                && !name.ends_with(['.', ' '])
                && !matches!(
                    stem.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                )
                && !((stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(
                        &stem[3..],
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    ))
        }),
        _ => true,
    })
}

fn directory(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .custom_flags(OPEN_REPARSE | BACKUP_SEMANTICS)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & REPARSE != 0 {
        return Err(denied());
    }
    Ok(file)
}

pub(super) fn pin_tree(path: &Path, create: bool) -> io::Result<Vec<File>> {
    if !path_admitted(path) {
        return Err(denied());
    }
    let mut current = PathBuf::new();
    let mut handles = Vec::new();
    for part in path.components() {
        current.push(part.as_os_str());
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        match directory(&current) {
            Ok(file) => handles.push(file),
            Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
                handles.push(directory(&current)?);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(handles)
}

fn pin_parent(path: &Path) -> io::Result<Vec<File>> {
    if !path_admitted(path) {
        return Err(denied());
    }
    pin_tree(path.parent().ok_or_else(denied)?, false)
}

pub(super) fn regular_file(path: &Path) -> io::Result<File> {
    let _parents = pin_parent(path)?;
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(OPEN_REPARSE)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & REPARSE != 0 {
        return Err(denied());
    }
    Ok(file)
}

pub(super) fn read_bounded(path: &Path, maximum: u64) -> io::Result<Vec<u8>> {
    let file = regular_file(path)?;
    if file.metadata()?.len() > maximum {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(denied());
    }
    Ok(bytes)
}

fn fresh_name(parent: &Path, tag: &str) -> io::Result<PathBuf> {
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
    let nonce = u128::from_le_bytes(random);
    Ok(parent.join(format!(".orchbee-agent-{tag}-{nonce:032x}")))
}

pub(crate) fn stage_directory(parent: &Path, tag: &str) -> io::Result<PathBuf> {
    let _parents = pin_tree(parent, true)?;
    for _ in 0..8 {
        let path = fresh_name(parent, tag)?;
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

pub(crate) fn write_file_exclusive(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let _parents = pin_parent(path)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .custom_flags(OPEN_REPARSE)
        .open(path)?
        .write_all(bytes)
}

pub(crate) fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    // Payload validation excludes the separately bounded capsule manifest.
    let mut budget = (129usize, 2 * 1024 * 1024usize + 1024);
    copy_entry(source, target, 0, &mut budget)
}

fn copy_entry(
    source: &Path,
    target: &Path,
    depth: usize,
    budget: &mut (usize, usize),
) -> io::Result<()> {
    if depth > 8 || budget.0 == 0 {
        return Err(denied());
    }
    budget.0 -= 1;
    let _source_parent = pin_parent(source)?;
    let _target_parent = pin_parent(target)?;
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_attributes() & REPARSE != 0 {
        return Err(denied());
    }
    if metadata.is_dir() {
        let _source = directory(source)?;
        fs::create_dir(target)?;
        let _target = directory(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(
                &entry.path(),
                &target.join(entry.file_name()),
                depth + 1,
                budget,
            )?;
        }
        Ok(())
    } else {
        let bytes = read_bounded(source, FILE_LIMIT)?;
        budget.1 = budget.1.checked_sub(bytes.len()).ok_or_else(denied)?;
        write_file_exclusive(target, &bytes)
    }
}

fn move_no_replace(source: &Path, target: &Path) -> FsStep {
    let (Ok(_source), Ok(_target)) = (pin_parent(source), pin_parent(target)) else {
        return FsStep::Failed;
    };
    let (Some(source), Some(target)) = (source.to_str(), target.to_str()) else {
        return FsStep::Failed;
    };
    match winsafe::MoveFile(source, target) {
        Ok(()) => FsStep::Done,
        Err(error)
            if error == winsafe::co::ERROR::ALREADY_EXISTS
                || error == winsafe::co::ERROR::FILE_EXISTS =>
        {
            FsStep::Conflict
        }
        Err(_) => FsStep::Failed,
    }
}

pub(crate) fn publish_no_replace(staged: &Path, target: &Path) -> FsStep {
    move_no_replace(staged, target)
}

pub(crate) fn move_aside(target: &Path) -> Result<Option<PathBuf>, FsStep> {
    let _parents = pin_parent(target).map_err(|_| FsStep::Failed)?;
    match fs::symlink_metadata(target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(FsStep::Failed),
        Ok(_) => {}
    }
    let parent = target.parent().ok_or(FsStep::Failed)?;
    for _ in 0..8 {
        let backup = fresh_name(parent, "backup").map_err(|_| FsStep::Failed)?;
        match move_no_replace(target, &backup) {
            FsStep::Done => return Ok(Some(backup)),
            FsStep::Conflict => {}
            failed => return Err(failed),
        }
    }
    Err(FsStep::Conflict)
}

pub(crate) fn restore_backup(backup: &Path, target: &Path) -> FsStep {
    move_no_replace(backup, target)
}

pub(crate) fn entry_digest(path: &Path) -> Option<String> {
    use sha2::Digest;
    let _parents = pin_parent(path).ok()?;
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_attributes() & REPARSE != 0 {
        return None;
    }
    if metadata.is_file() {
        return Some(format!(
            "{:x}",
            sha2::Sha256::digest(read_bounded(path, FILE_LIMIT).ok()?)
        ));
    }
    let _root = directory(path).ok()?;
    let mut files = super::inspect::fingerprint_directory(path)?;
    files.sort();
    let mut hasher = sha2::Sha256::new();
    for (relative, digest) in files {
        hasher.update(relative.as_bytes());
        hasher.update(b"\0");
        hasher.update(digest.as_bytes());
        hasher.update(b"\0");
    }
    Some(format!("{:x}", hasher.finalize()))
}

pub(crate) fn remove_owned(target: &Path, is_directory: bool) -> FsStep {
    let mut remaining = 256;
    match remove_entry(target, is_directory, 0, &mut remaining) {
        Ok(()) => FsStep::Done,
        Err(error) if error.kind() == io::ErrorKind::NotFound => FsStep::Done,
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => FsStep::Conflict,
        Err(_) => FsStep::Failed,
    }
}

fn remove_entry(
    path: &Path,
    is_directory: bool,
    depth: usize,
    remaining: &mut usize,
) -> io::Result<()> {
    if depth > 10 || *remaining == 0 {
        return Err(denied());
    }
    *remaining -= 1;
    let _parents = pin_parent(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_attributes() & REPARSE != 0 || metadata.is_dir() != is_directory {
        return Err(denied());
    }
    if !is_directory {
        return fs::remove_file(path);
    }
    let root = directory(path)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        remove_entry(
            &entry.path(),
            entry.file_type()?.is_dir(),
            depth + 1,
            remaining,
        )?;
    }
    drop(root);
    // Parent handles still pin the path. remove_dir does not traverse a
    // replacement junction at this last component.
    fs::remove_dir(path)
}
