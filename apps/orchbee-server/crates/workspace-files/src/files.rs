//! Directory listings and working-tree reads, confined beneath the Workspace's canonical root.
use crate::git::{self, Run};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_ENTRIES, MAXIMUM_WORKSPACE_FILE_NAME_BYTES,
    MAXIMUM_WORKSPACE_FILE_TEXT_BYTES, WorkspaceDirectoryEntry, WorkspaceEntryKind,
    WorkspaceFileContent, WorkspaceFilesFailure as Failure, WorkspaceFilesOutcome as Outcome,
};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Bytes of a file's start that are looked at for a NUL before it counts as text.
const BINARY_PROBE_BYTES: usize = 8 * 1024;
/// Encoded names one listing carries before it reports truncation, well inside the result cap.
const LISTING_NAME_BUDGET: usize = 4 * 1024 * 1024;

/// The Workspace root, canonicalized once per request.
pub(crate) struct Root {
    canonical: PathBuf,
}

impl Root {
    pub(crate) fn open(root: &str) -> Result<Self, Failure> {
        let path = Path::new(root);
        if !path.is_absolute() {
            return Err(Failure::Denied);
        }
        let canonical = std::fs::canonicalize(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Failure::NotFound,
            std::io::ErrorKind::PermissionDenied => Failure::Denied,
            _ => Failure::Unavailable,
        })?;
        if !canonical.is_dir() {
            return Err(Failure::NotFound);
        }
        Ok(Self { canonical })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.canonical
    }

    /// The lexical location of a validated relative path; it may not exist.
    pub(crate) fn join(&self, relative: &str) -> PathBuf {
        let mut path = self.canonical.clone();
        if !relative.is_empty() {
            path.extend(relative.split('/'));
        }
        path
    }

    /// The canonical location of an existing relative path, refused when a symbolic link
    /// leads outside the root.
    pub(crate) fn resolve(&self, relative: &str) -> Result<PathBuf, Failure> {
        let resolved =
            std::fs::canonicalize(self.join(relative)).map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Failure::NotFound,
                std::io::ErrorKind::PermissionDenied => Failure::Denied,
                _ => Failure::Unavailable,
            })?;
        if resolved.starts_with(&self.canonical) {
            Ok(resolved)
        } else {
            Err(Failure::Denied)
        }
    }
}

pub(crate) fn list_directory(root: &Root, relative: &str, deadline: Instant) -> Outcome {
    let directory = match root.resolve(relative) {
        Ok(directory) => directory,
        Err(reason) => return Outcome::Failed { reason },
    };
    let Ok(read) = std::fs::read_dir(&directory) else {
        return Outcome::Failed {
            reason: if directory.is_dir() {
                Failure::Denied
            } else {
                Failure::NotFound
            },
        };
    };
    let mut entries = Vec::new();
    let mut truncated = false;
    let mut budget = LISTING_NAME_BUDGET;
    for entry in read.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            // A name that is not UTF-8 cannot be named on the wire.
            continue;
        };
        if name == ".git" || !valid_name(&name) {
            continue;
        }
        if entries.len() == MAXIMUM_WORKSPACE_FILE_ENTRIES || name.len() + 96 > budget {
            truncated = true;
            break;
        }
        budget -= name.len() + 96;
        let symbolic_link = entry.file_type().is_ok_and(|kind| kind.is_symlink());
        // A link reports its target's kind; one that dangles is `other`.
        let entry_kind = match std::fs::metadata(entry.path()) {
            Ok(metadata) if metadata.is_dir() => WorkspaceEntryKind::Directory,
            Ok(metadata) if metadata.is_file() => WorkspaceEntryKind::File,
            _ => WorkspaceEntryKind::Other,
        };
        entries.push(WorkspaceDirectoryEntry {
            name,
            entry_kind,
            symbolic_link,
            ignored: false,
        });
    }
    let ignored = ignored_names(&directory, &entries, deadline);
    for entry in &mut entries {
        entry.ignored = ignored.contains(&entry.name);
    }
    entries.sort_by(|left, right| {
        let directory =
            |entry: &WorkspaceDirectoryEntry| entry.entry_kind != WorkspaceEntryKind::Directory;
        directory(left)
            .cmp(&directory(right))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.name.cmp(&right.name))
    });
    Outcome::Directory { entries, truncated }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAXIMUM_WORKSPACE_FILE_NAME_BYTES
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
}

/// The listed names Git's ignore rules exclude; none outside a repository or when Git fails.
fn ignored_names(
    directory: &Path,
    entries: &[WorkspaceDirectoryEntry],
    deadline: Instant,
) -> BTreeSet<String> {
    if entries.is_empty() {
        return BTreeSet::new();
    }
    let mut input = Vec::new();
    for entry in entries {
        input.extend_from_slice(entry.name.as_bytes());
        // A trailing separator lets directory-only patterns such as `build/` match.
        if entry.entry_kind == WorkspaceEntryKind::Directory {
            input.push(b'/');
        }
        input.push(0);
    }
    match git::run(
        directory,
        &["check-ignore", "-z", "--stdin"],
        Some(input),
        LISTING_NAME_BUDGET * 2,
        deadline,
    ) {
        // Exit 0 names ignored paths; 1 means none is ignored; anything else knows nothing.
        Run::Exited {
            status: Some(0),
            stdout,
            truncated: false,
        } => stdout
            .split(|byte| *byte == 0)
            .filter_map(|name| std::str::from_utf8(name).ok())
            .filter(|name| !name.is_empty())
            .map(|name| name.trim_end_matches('/').to_owned())
            .collect(),
        _ => BTreeSet::new(),
    }
}

pub(crate) fn read_worktree(root: &Root, relative: &str) -> Outcome {
    let path = match root.resolve(relative) {
        Ok(path) => path,
        // A file deleted from the tree reads as absent, so its diff shows it removed.
        Err(Failure::NotFound) => return absent(),
        Err(reason) => return Outcome::Failed { reason },
    };
    let Ok(metadata) = std::fs::metadata(&path) else {
        return absent();
    };
    if !metadata.is_file() {
        return Outcome::Failed {
            reason: Failure::NotFound,
        };
    }
    let byte_length = metadata.len();
    if usize::try_from(byte_length)
        .map_or(true, |length| length > MAXIMUM_WORKSPACE_FILE_TEXT_BYTES)
    {
        return Outcome::File {
            byte_length,
            content: WorkspaceFileContent::TooLarge {},
        };
    }
    let mut bytes = Vec::new();
    let read = std::fs::File::open(&path).and_then(|file| {
        file.take(MAXIMUM_WORKSPACE_FILE_TEXT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
    });
    match read {
        Ok(_) => file_outcome(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Outcome::Failed {
            reason: Failure::Denied,
        },
        Err(_) => Outcome::Failed {
            reason: Failure::Unavailable,
        },
    }
}

pub(crate) fn absent() -> Outcome {
    Outcome::File {
        byte_length: 0,
        content: WorkspaceFileContent::Absent {},
    }
}

/// What a file's bytes come to on the wire.
pub(crate) fn file_outcome(bytes: Vec<u8>) -> Outcome {
    let byte_length = bytes.len() as u64;
    if bytes.len() > MAXIMUM_WORKSPACE_FILE_TEXT_BYTES {
        return Outcome::File {
            byte_length,
            content: WorkspaceFileContent::TooLarge {},
        };
    }
    if bytes[..bytes.len().min(BINARY_PROBE_BYTES)].contains(&0) {
        return Outcome::File {
            byte_length,
            content: WorkspaceFileContent::Binary {},
        };
    }
    match String::from_utf8(bytes) {
        Ok(text) => Outcome::File {
            byte_length,
            content: WorkspaceFileContent::Text { text },
        },
        Err(_) => Outcome::File {
            byte_length,
            content: WorkspaceFileContent::Binary {},
        },
    }
}
