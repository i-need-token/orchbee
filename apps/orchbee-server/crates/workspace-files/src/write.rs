//! Saving a working-tree file: only while it is still the version the client read, and, where the
//! platform allows, so that no reader ever sees half of it.
use crate::files::{Root, hex, modified};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES, WorkspaceFileVersion, WorkspaceFilesFailure as Failure,
    WorkspaceFilesOutcome as Outcome,
};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{ErrorKind, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Attempts at a fresh temporary name before the save writes in place instead.
const TEMPORARY_ATTEMPTS: u32 = 4;

/// Writes `text` to the working-tree file at `relative`. `expected` is the SHA-256 the client
/// read the file at, or none for a file that must not exist yet; any other file answers
/// `conflict` with what it is now, and nothing is written.
pub(crate) fn write_worktree(
    root: &Root,
    relative: &str,
    text: &str,
    expected: Option<&str>,
) -> Outcome {
    let (parent, name) = relative.rsplit_once('/').unwrap_or(("", relative));
    // The directory must already exist inside the root; a save never creates directories.
    let directory = match root.resolve(parent) {
        Ok(directory) if directory.is_dir() => directory,
        Ok(_) => return failed(Failure::NotFound),
        Err(reason) => return failed(reason),
    };
    let entry = directory.join(name);
    match std::fs::symlink_metadata(&entry) {
        Err(error) if error.kind() == ErrorKind::NotFound => match expected {
            // The file the client read is gone.
            Some(_) => Outcome::Conflict { version: None },
            None => create(&entry, text),
        },
        Err(error) => failed(io_failure(&error)),
        Ok(_) => match root.resolve(relative) {
            Ok(target) => replace(&target, text, expected),
            // A symbolic link whose target is missing or outside the root is not written
            // through, since that would create a file the right bar cannot show.
            Err(Failure::NotFound) => failed(Failure::Denied),
            Err(reason) => failed(reason),
        },
    }
}

/// A file that must not exist yet: created exclusively, so a file another program made since the
/// client looked is never replaced.
fn create(path: &Path, text: &str) -> Outcome {
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Outcome::Conflict {
                version: current(path).ok().map(|(_, version)| version),
            };
        }
        Err(error) => return failed(io_failure(&error)),
    };
    if let Err(error) = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _removed = std::fs::remove_file(path);
        return failed(io_failure(&error));
    }
    drop(file);
    sync_directory(path);
    written(path, text)
}

/// An existing regular file, replaced only while its content still hashes to `expected`.
fn replace(target: &Path, text: &str, expected: Option<&str>) -> Outcome {
    let (metadata, version) = match current(target) {
        Ok(found) => found,
        Err(reason) => return failed(reason),
    };
    if !metadata.is_file() {
        return failed(Failure::NotFound);
    }
    if expected.is_none() || version.sha256.as_deref() != expected {
        return Outcome::Conflict {
            version: Some(version),
        };
    }
    // A read-only file stays unwritten, as it would in any editor that asks first.
    if metadata.permissions().readonly() {
        return failed(Failure::Denied);
    }
    let saved = match Temporary::write(target, &metadata, text) {
        // The file's links, owner or group would not survive a rename, or the directory takes
        // no new file: write in place.
        Ok(None) => in_place(target, &metadata, text),
        Ok(Some(temporary)) => temporary.replace(target, &metadata, text),
        Err(error) => Err(io_failure(&error)),
    };
    match saved {
        Ok(None) => written(target, text),
        Ok(Some(newer)) => Outcome::Conflict {
            version: Some(newer),
        },
        Err(reason) => failed(reason),
    }
}

/// A complete copy of the new content beside the target, ready to rename over it.
struct Temporary {
    path: PathBuf,
}

impl Temporary {
    /// The written copy, or none where a rename would lose something the target has.
    fn write(target: &Path, metadata: &Metadata, text: &str) -> std::io::Result<Option<Self>> {
        let Some((directory, name)) = target.parent().zip(target.file_name()) else {
            return Ok(None);
        };
        let mut attempt = 0;
        let (path, mut file) = loop {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos());
            let path = directory.join(format!(
                ".{}.{}-{stamp:x}.orchbee-save",
                name.to_string_lossy(),
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => break (path, file),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    attempt += 1;
                    if attempt == TEMPORARY_ATTEMPTS {
                        return Ok(None);
                    }
                }
                Err(error) if error.kind() == ErrorKind::PermissionDenied => return Ok(None),
                Err(error) => return Err(error),
            }
        };
        let temporary = Self { path };
        if !rename_keeps(&file, metadata)? {
            return Ok(None);
        }
        file.set_permissions(metadata.permissions())?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        Ok(Some(temporary))
    }

    /// Renames the copy over the target, unless the target changed since it was hashed, which
    /// answers its newer version. A rename the platform refuses, such as over a file another
    /// program holds open, writes in place instead.
    fn replace(
        self,
        target: &Path,
        hashed: &Metadata,
        text: &str,
    ) -> Result<Option<WorkspaceFileVersion>, Failure> {
        if let Some(newer) = changed_since(target, hashed)? {
            return Ok(Some(newer));
        }
        match std::fs::rename(&self.path, target) {
            Ok(()) => {
                sync_directory(target);
                Ok(None)
            }
            Err(_) => {
                drop(self);
                in_place(target, hashed, text)
            }
        }
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        // Gone already once renamed.
        let _removed = std::fs::remove_file(&self.path);
    }
}

/// Truncates and rewrites the target itself, keeping its links, owner and attributes; a reader
/// may see it part-written.
fn in_place(
    target: &Path,
    hashed: &Metadata,
    text: &str,
) -> Result<Option<WorkspaceFileVersion>, Failure> {
    if let Some(newer) = changed_since(target, hashed)? {
        return Ok(Some(newer));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(target)
        .map_err(|error| io_failure(&error))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| io_failure(&error))?;
    Ok(None)
}

/// The target's newer version when its size or modification time moved since it was hashed.
fn changed_since(
    target: &Path,
    hashed: &Metadata,
) -> Result<Option<WorkspaceFileVersion>, Failure> {
    let now = std::fs::metadata(target).map_err(|error| io_failure(&error))?;
    if now.len() == hashed.len() && now.modified().ok() == hashed.modified().ok() {
        return Ok(None);
    }
    current(target).map(|(_, version)| Some(version))
}

/// The file's metadata and version: its modification time, and its hash up to the editable size.
fn current(path: &Path) -> Result<(Metadata, WorkspaceFileVersion), Failure> {
    let mut file = File::open(path).map_err(|error| io_failure(&error))?;
    let metadata = file.metadata().map_err(|error| io_failure(&error))?;
    let mut sha256 = None;
    if metadata.is_file() && metadata.len() <= MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES {
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| io_failure(&error))?;
        if bytes.len() as u64 <= MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES {
            sha256 = Some(hex(&Sha256::digest(&bytes)));
        }
    }
    let version = WorkspaceFileVersion {
        modified: modified(&metadata),
        sha256,
    };
    Ok((metadata, version))
}

/// The version a save leaves: the hash of what was written, and the file's new time.
fn written(path: &Path, text: &str) -> Outcome {
    match std::fs::metadata(path) {
        Ok(metadata) => Outcome::Written {
            version: WorkspaceFileVersion {
                modified: modified(&metadata),
                sha256: Some(hex(&Sha256::digest(text.as_bytes()))),
            },
        },
        Err(error) => failed(io_failure(&error)),
    }
}

/// Whether renaming a new file `created` over the target keeps what the target has: a single
/// link, and the same owner and group.
#[cfg(unix)]
fn rename_keeps(created: &File, target: &Metadata) -> std::io::Result<bool> {
    use std::os::unix::fs::MetadataExt as _;
    let created = created.metadata()?;
    Ok(target.nlink() == 1 && created.uid() == target.uid() && created.gid() == target.gid())
}

/// Windows keeps no owner on the file that a rename would change, and its link count is not
/// readable here, so a rename keeps what matters.
#[cfg(not(unix))]
fn rename_keeps(_created: &File, _target: &Metadata) -> std::io::Result<bool> {
    Ok(true)
}

/// Makes a created or renamed entry durable by syncing its directory.
#[cfg(unix)]
fn sync_directory(path: &Path) {
    if let Some(directory) = path.parent() {
        let _synced = File::open(directory).and_then(|directory| directory.sync_all());
    }
}

/// Windows records a rename in the file system's own journal and has no directory sync.
#[cfg(not(unix))]
fn sync_directory(_path: &Path) {}

fn io_failure(error: &std::io::Error) -> Failure {
    match error.kind() {
        ErrorKind::NotFound => Failure::NotFound,
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => Failure::Denied,
        _ => Failure::Unavailable,
    }
}

fn failed(reason: Failure) -> Outcome {
    Outcome::Failed { reason }
}
