//! Directory listings and working-tree reads, confined beneath the Workspace's canonical root.
use crate::git::{self, Run};
use base64::Engine as _;
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_BYTES_READ, MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES,
    MAXIMUM_WORKSPACE_FILE_ENTRIES, MAXIMUM_WORKSPACE_FILE_NAME_BYTES,
    MAXIMUM_WORKSPACE_FILE_OPEN_BYTES, WORKSPACE_FILE_CHUNK_BYTES, WorkspaceDirectoryEntry,
    WorkspaceEntryKind, WorkspaceFileContent, WorkspaceFileEncoding as Encoding,
    WorkspaceFileVersion, WorkspaceFilesFailure as Failure, WorkspaceFilesOutcome as Outcome,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{Read as _, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

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

/// Where a read starts and how it returns content.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Read {
    pub(crate) offset: u64,
    pub(crate) encoding: Encoding,
}

impl Read {
    /// The largest file this read opens at all.
    pub(crate) fn limit(self) -> u64 {
        match self.encoding {
            Encoding::Text => MAXIMUM_WORKSPACE_FILE_OPEN_BYTES,
            Encoding::Bytes => MAXIMUM_WORKSPACE_FILE_BYTES_READ,
        }
    }
}

/// One chunk of a working-tree file, with the version it was read at. The first chunk of a file
/// small enough to edit reads it whole to hash it, then answers its start.
pub(crate) fn read_worktree(root: &Root, relative: &str, read: Read) -> Outcome {
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
    let modified = modified(&metadata);
    let version = |sha256| {
        Some(WorkspaceFileVersion {
            modified: modified.clone(),
            sha256,
        })
    };
    if byte_length > read.limit() {
        return Outcome::File {
            byte_length,
            version: version(None),
            content: WorkspaceFileContent::TooLarge {},
        };
    }
    if read.offset > byte_length {
        return Outcome::Failed {
            reason: Failure::Malformed,
        };
    }
    let whole = read.offset == 0 && byte_length <= MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES;
    let bytes = std::fs::File::open(&path).and_then(|mut file| {
        let mut bytes = Vec::new();
        if whole {
            file.take(MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES + 1)
                .read_to_end(&mut bytes)?;
        } else {
            file.seek(SeekFrom::Start(read.offset))?;
            file.take(WORKSPACE_FILE_CHUNK_BYTES as u64)
                .read_to_end(&mut bytes)?;
        }
        Ok(bytes)
    });
    match bytes {
        // The file may have changed since its size was read; answer what was read, and hash it
        // only while it is still small enough to edit.
        Ok(bytes) if whole && bytes.len() as u64 <= MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES => {
            let byte_length = bytes.len() as u64;
            let sha256 = hex(&Sha256::digest(&bytes));
            chunk(byte_length, version(Some(sha256)), read, 0, &bytes)
        }
        Ok(bytes) if whole => chunk(
            byte_length.max(bytes.len() as u64),
            version(None),
            read,
            0,
            &bytes,
        ),
        Ok(bytes) => chunk(byte_length, version(None), read, read.offset, &bytes),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Outcome::Failed {
            reason: Failure::Denied,
        },
        Err(_) => Outcome::Failed {
            reason: Failure::Unavailable,
        },
    }
}

/// A file's modification time in nanoseconds since the Unix epoch, in decimal; `0` when unknown.
pub(crate) fn modified(metadata: &std::fs::Metadata) -> String {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or_else(|| "0".to_owned(), |since| since.as_nanos().to_string())
}

pub(crate) fn absent() -> Outcome {
    Outcome::File {
        byte_length: 0,
        version: None,
        content: WorkspaceFileContent::Absent {},
    }
}

/// The chunk at `read.offset` of a file of `byte_length`, from `bytes`, which hold the file from
/// `start`, at or before the offset.
pub(crate) fn chunk(
    byte_length: u64,
    version: Option<WorkspaceFileVersion>,
    read: Read,
    start: u64,
    bytes: &[u8],
) -> Outcome {
    let file = |content| Outcome::File {
        byte_length,
        version: version.clone(),
        content,
    };
    let Some(window) = read
        .offset
        .checked_sub(start)
        .and_then(|skip| usize::try_from(skip).ok())
        .and_then(|skip| bytes.get(skip..))
    else {
        return Outcome::Failed {
            reason: Failure::Malformed,
        };
    };
    let mut take = window.len().min(WORKSPACE_FILE_CHUNK_BYTES);
    if read.encoding == Encoding::Text {
        if read.offset == 0 && window[..window.len().min(BINARY_PROBE_BYTES)].contains(&0) {
            return file(WorkspaceFileContent::Binary {});
        }
        let ends = read.offset + take as u64 >= byte_length;
        match std::str::from_utf8(&window[..take]) {
            Ok(_) => {}
            // A chunk that would stop inside a character stops before it instead, so every
            // chunk is whole text and the next starts on a character.
            Err(error) if error.error_len().is_none() && !ends && error.valid_up_to() > 0 => {
                take = error.valid_up_to();
            }
            // Bytes that are not UTF-8 anywhere in the file make the whole file binary.
            Err(_) => return file(WorkspaceFileContent::Binary {}),
        }
    }
    let end = read.offset + take as u64;
    let next_offset = (end < byte_length).then_some(end);
    let taken = &window[..take];
    file(match read.encoding {
        Encoding::Text => WorkspaceFileContent::Text {
            offset: read.offset,
            text: String::from_utf8_lossy(taken).into_owned(),
            next_offset,
        },
        Encoding::Bytes => WorkspaceFileContent::Bytes {
            offset: read.offset,
            base64: base64::engine::general_purpose::STANDARD.encode(taken),
            next_offset,
        },
    })
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}
