//! Filesystem adapter — the only IO boundary of `agent doctor`. Every
//! filesystem fact the document reports flows through these bounded,
//! descriptor-gated observations; the pure classification lives in
//! `probes.rs` and consumes byte vectors and typed outcomes, never a path.

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};
mod observation;
pub(crate) use observation::{ConfigurationScan, Observation, Unsupported};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(unix)]
mod unix_path;
#[cfg(unix)]
pub(crate) use unix_path::{path_admitted, path_from_bytes};
/// The native path representation and endpoint output policy; every branch is type-checked.
pub(crate) const WINDOWS_PATHS: bool = cfg!(windows);
use super::{
    Budget,
    probes::{CapsuleEntry, MarkerFileEntry},
};

/// The current executable's adjacent distribution sidecar; never searches PATH.
pub(crate) fn current_adapter_directory(observation: Observation) -> Option<PathBuf> {
    Some(
        observation
            .current_executable()
            .ok()?
            .parent()?
            .join("agent-observation-adapters"),
    )
}

/// Native path admission: byte-bounded before rendering — never silently
/// truncated, never lossy-decoded. The Desktop PATH helper's ≤4096-*character*
/// directory cap is a separate source fact; admission here is in bytes.
pub(crate) const MAX_PATH_BYTES: usize = 4096;

/// The entry kind an `lstat`-shaped observation reports — the consumer's own
/// `symlink_metadata` classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryKind {
    Regular,
    Directory,
    Socket,
    Other,
}

/// One `lstat` of the final path component — never followed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Lstat {
    /// No entry exists at the path.
    Absent,
    /// The final component is a symlink — uninspected, not broken.
    Symlink,
    /// An entry exists; `size` is the declared regular-file length.
    Entry {
        kind: EntryKind,
        ownership: Option<Ownership>,
        size: u64,
    },
    /// Classification itself failed.
    Unreadable,
}

/// POSIX ownership is an observed optional fact; Windows does not manufacture a UID or mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Ownership {
    pub(crate) uid: u32,
    pub(crate) mode: u32,
}

/// A descriptor-admitted bounded read of a regular file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FileRead {
    /// Regular file whose declared size fit the cap.
    Bytes(Vec<u8>),
    /// No entry exists at the path.
    Absent,
    /// Entry exists but is not a regular file.
    UnsafeKind,
    /// The final component is a symlink — never followed.
    Symlink,
    /// The file exceeds (or grew past) the read cap.
    OverLimit,
    /// The file fit its own cap but not the remaining aggregate budget.
    BudgetExhausted,
    /// Open, stat, or read failed.
    Unreadable,
}

/// One directory listing entry — the entry's own dirent type, no follow.
#[derive(Clone, Debug)]
pub(crate) struct DirEntry {
    pub(crate) name: OsString,
    pub(crate) kind: ListKind,
}

/// The listing-visible kind of a directory member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ListKind {
    Regular,
    Directory,
    Symlink,
    Other,
}

/// One bounded directory listing.
#[derive(Clone, Debug)]
pub(crate) enum DirEntries {
    /// At most `maximum` entries, in readdir order.
    Entries(Vec<DirEntry>),
    /// No entry exists at the directory path.
    Absent,
    /// The final component is a symlink — never followed.
    Symlink,
    /// The entry exists but is not a directory.
    NotDirectory,
    /// The listing produced more than `maximum` entries.
    OverLimit,
    /// Classification or readdir failed.
    Unreadable,
}

/// The bounded inherited-PATH scan's first executable candidate and truncation status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PathScan {
    /// The first candidate satisfying `{access X_OK, stat isFile}` — raw path
    /// bytes, `None` when every admitted candidate missed.
    pub(crate) found: Option<Vec<u8>>,
    /// The PATH string held more directories than the per-source 256 cap — the
    /// scan covered the capped prefix only.
    pub(crate) truncated: bool,
}

/// The owning process's environment value for one variable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EnvValue {
    Unset,
    Value(Vec<u8>),
}

/// `lstat` of the final component — the probe's no-follow observation.
pub(crate) fn lstat(path: &Path, observation: Observation) -> Lstat {
    observation.lstat(path)
}

/// The native no-follow regular-file gate and bounded body read. A file that
/// fits its own cap but not the aggregate budget is `BudgetExhausted`, never
/// `OverLimit`. The run-level time budget remains cooperative.
pub(crate) fn read_regular(path: &Path, cap: u64, budget: &mut Budget) -> FileRead {
    budget.observation.read_regular(path, cap, budget)
}

/// A bounded listing of one directory. The final component is `lstat`-gated
/// before `read_dir` so a symlinked target reports `Symlink`, never followed;
/// mid-listing races degrade to `Unreadable` rather than a guessed answer.
pub(crate) fn list_dir(path: &Path, maximum: usize, observation: Observation) -> DirEntries {
    match lstat(path, observation) {
        Lstat::Absent => return DirEntries::Absent,
        Lstat::Symlink => return DirEntries::Symlink,
        Lstat::Entry { kind, .. } if kind != EntryKind::Directory => {
            return DirEntries::NotDirectory;
        }
        Lstat::Entry { .. } => {}
        Lstat::Unreadable => return DirEntries::Unreadable,
    }
    let mut entries = Vec::new();
    let listing = match observation.read_dir(path) {
        Ok(listing) => listing,
        Err(_) => return DirEntries::Unreadable,
    };
    for entry in listing {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => return DirEntries::Unreadable,
        };
        if entries.len() >= maximum {
            return DirEntries::OverLimit;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => return DirEntries::Unreadable,
        };
        if observation.reject_entry(&entry) {
            entries.push(DirEntry {
                name: entry.file_name(),
                kind: ListKind::Symlink,
            });
            continue;
        }
        let kind = if file_type.is_symlink() {
            ListKind::Symlink
        } else if file_type.is_file() {
            ListKind::Regular
        } else if file_type.is_dir() {
            ListKind::Directory
        } else {
            ListKind::Other
        };
        entries.push(DirEntry {
            name: entry.file_name(),
            kind,
        });
    }
    DirEntries::Entries(entries)
}

/// One environment variable of the owning process — raw bytes, never the
/// managed child's environment.
pub(crate) fn env_value(name: &str, observation: Observation) -> EnvValue {
    observation.env_value(name)
}

/// The owning process's `PATH` value — the only search-path source the
/// command inspects; a shell-derived PATH would require process execution.
pub(crate) fn inherited_path(observation: Observation) -> Option<OsString> {
    observation.inherited_path()
}

/// The owning process's `HOME` value.
pub(crate) fn home_directory(observation: Observation) -> Option<PathBuf> {
    observation.home_directory()
}

/// The inherited-PATH executable scan — the faithful port of
/// `resolveAgentExecutableCandidates`: name-shape gate, per-source ≤256
/// directories, per-directory ≤4096 bytes, control-character and absoluteness
/// admission, first-match `{access X_OK, stat isFile}`.
pub(crate) fn scan_path_for(name: &str, path_env: &OsStr, observation: Observation) -> PathScan {
    observation.scan_path_for(name, path_env)
}

// ---------------------------------------------------------------------------
// Bounded tree walks — the traversal halves of the capsule payload digest
// and the guarded-directory fingerprint, each mirroring its TypeScript loop.
// ---------------------------------------------------------------------------

/// One bounded capsule payload walk.
#[derive(Clone, Debug)]
pub(crate) enum CapsuleWalk {
    /// The collected payload entries (unsorted — the digest owns ordering).
    Entries(Vec<CapsuleEntry>),
    /// No entry exists at the root path.
    Absent,
    /// The root's final component is a symlink.
    Symlink,
    /// The root exists but is not a directory.
    NotDirectory,
    /// A symlink or non-file/non-directory entry refused traversal.
    UnsafeKind,
    /// A count/depth/byte cap was exceeded.
    OverLimit,
    /// Classification or IO failed.
    Unreadable,
    /// The aggregate read budget exhausted mid-walk.
    BudgetExhausted,
}

const CAPSULE_MAX_ENTRIES: usize = 128;
const CAPSULE_MAX_DEPTH: usize = 8;
const CAPSULE_MAX_FILE_BYTES: u64 = 512 * 1024;
const CAPSULE_MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024;

/// `computeAgentObservationCapsulePayloadDigest`'s traversal: every dirent
/// counts toward ≤128 across all directories, depth ≤8, `capsule.json` is
/// skipped only after the directory/file checks exactly like the source,
/// per-file ≤512KiB, payload total ≤2MiB, every symlink refuses traversal.
pub(crate) fn capsule_walk(root: &Path, budget: &mut Budget) -> CapsuleWalk {
    match lstat(root, budget.observation) {
        Lstat::Absent => return CapsuleWalk::Absent,
        Lstat::Symlink => return CapsuleWalk::Symlink,
        Lstat::Entry { kind, .. } if kind != EntryKind::Directory => {
            return CapsuleWalk::NotDirectory;
        }
        Lstat::Entry { .. } => {}
        Lstat::Unreadable => return CapsuleWalk::Unreadable,
    }
    let mut pending: Vec<(usize, PathBuf, String)> = vec![(0, root.to_path_buf(), String::new())];
    let mut entries = Vec::new();
    let mut entry_count = 0usize;
    let mut total_bytes = 0u64;
    while let Some((depth, directory, relative_dir)) = pending.pop() {
        if depth > CAPSULE_MAX_DEPTH {
            return CapsuleWalk::OverLimit;
        }
        let listing = match budget.observation.read_dir(&directory) {
            Ok(listing) => listing,
            Err(_) => return CapsuleWalk::Unreadable,
        };
        for entry in listing {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => return CapsuleWalk::Unreadable,
            };
            entry_count += 1;
            if entry_count > CAPSULE_MAX_ENTRIES {
                return CapsuleWalk::OverLimit;
            }
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => return CapsuleWalk::Unreadable,
            };
            if file_type.is_symlink() {
                return CapsuleWalk::UnsafeKind;
            }
            if budget.observation.reject_entry(&entry) {
                return CapsuleWalk::UnsafeKind;
            }
            // E-5: a non-UTF-8 payload path cannot be serialized like the JS
            // digest — the root is uninspectable, never "inconsistent".
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return CapsuleWalk::Unreadable;
            };
            let relative = if relative_dir.is_empty() {
                name.clone()
            } else {
                format!("{relative_dir}/{name}")
            };
            if file_type.is_dir() {
                entries.push(CapsuleEntry {
                    directory: true,
                    path: relative.clone(),
                    sha256: None,
                    size: None,
                });
                pending.push((depth + 1, entry.path(), relative));
                continue;
            }
            if !file_type.is_file() {
                return CapsuleWalk::UnsafeKind;
            }
            if relative == super::probes::CAPSULE_MANIFEST_NAME {
                continue;
            }
            let bytes = match read_regular(&entry.path(), CAPSULE_MAX_FILE_BYTES, budget) {
                FileRead::Bytes(bytes) => bytes,
                FileRead::Absent => return CapsuleWalk::Unreadable,
                FileRead::UnsafeKind | FileRead::Symlink => {
                    return CapsuleWalk::UnsafeKind;
                }
                FileRead::OverLimit => return CapsuleWalk::OverLimit,
                FileRead::BudgetExhausted => return CapsuleWalk::BudgetExhausted,
                FileRead::Unreadable => return CapsuleWalk::Unreadable,
            };
            total_bytes += bytes.len() as u64;
            if total_bytes > CAPSULE_MAX_TOTAL_BYTES {
                return CapsuleWalk::OverLimit;
            }
            entries.push(CapsuleEntry {
                directory: false,
                path: relative,
                sha256: Some(sha256_hex(&bytes)),
                size: Some(bytes.len() as u64),
            });
        }
    }
    CapsuleWalk::Entries(entries)
}

/// One bounded guarded-directory fingerprint walk.
#[derive(Clone, Debug)]
pub(crate) enum FingerprintWalk {
    /// The computed `{path, sha256}` file list (unsorted).
    Entries(Vec<MarkerFileEntry>),
    /// No entry exists at the root path.
    Absent,
    /// The root's final component is a symlink.
    Symlink,
    /// The root exists but is not a directory.
    NotDirectory,
    /// A symlink or non-file/non-directory entry refused traversal.
    UnsafeKind,
    /// A count/depth/byte cap was exceeded.
    OverLimit,
    /// Classification or IO failed.
    Unreadable,
    /// The aggregate read budget exhausted mid-walk.
    BudgetExhausted,
}

const FINGERPRINT_MAX_DEPTH: usize = 8;
const FINGERPRINT_MAX_FILES: usize = 64;
const FINGERPRINT_MAX_FILE_BYTES: u64 = 128 * 1024;
const FINGERPRINT_MAX_TOTAL_BYTES: u64 = 512 * 1024;

/// `fingerprintDirectory`'s traversal: depth ≤8, ≤64 files, per-file
/// ≤128KiB, total ≤512KiB, the marker name skipped at top level before the
/// directory check exactly like the source, every symlink refuses traversal.
pub(crate) fn fingerprint_walk(root: &Path, budget: &mut Budget) -> FingerprintWalk {
    match lstat(root, budget.observation) {
        Lstat::Absent => return FingerprintWalk::Absent,
        Lstat::Symlink => return FingerprintWalk::Symlink,
        Lstat::Entry { kind, .. } if kind != EntryKind::Directory => {
            return FingerprintWalk::NotDirectory;
        }
        Lstat::Entry { .. } => {}
        Lstat::Unreadable => return FingerprintWalk::Unreadable,
    }
    let mut pending: Vec<(usize, PathBuf, String)> = vec![(0, root.to_path_buf(), String::new())];
    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    while let Some((depth, directory, relative_dir)) = pending.pop() {
        if depth > FINGERPRINT_MAX_DEPTH {
            return FingerprintWalk::OverLimit;
        }
        let listing = match budget.observation.read_dir(&directory) {
            Ok(listing) => listing,
            Err(_) => return FingerprintWalk::Unreadable,
        };
        for entry in listing {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => return FingerprintWalk::Unreadable,
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => return FingerprintWalk::Unreadable,
            };
            if file_type.is_symlink() {
                return FingerprintWalk::UnsafeKind;
            }
            if budget.observation.reject_entry(&entry) {
                return FingerprintWalk::UnsafeKind;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return FingerprintWalk::Unreadable;
            };
            let relative = if relative_dir.is_empty() {
                name.clone()
            } else {
                format!("{relative_dir}/{name}")
            };
            if relative == super::probes::GUARDED_MARKER_NAME {
                continue;
            }
            if file_type.is_dir() {
                pending.push((depth + 1, entry.path(), relative));
                continue;
            }
            if !file_type.is_file() {
                return FingerprintWalk::UnsafeKind;
            }
            if files.len() >= FINGERPRINT_MAX_FILES {
                return FingerprintWalk::OverLimit;
            }
            let bytes = match read_regular(&entry.path(), FINGERPRINT_MAX_FILE_BYTES, budget) {
                FileRead::Bytes(bytes) => bytes,
                FileRead::Absent => return FingerprintWalk::Unreadable,
                FileRead::UnsafeKind | FileRead::Symlink => {
                    return FingerprintWalk::UnsafeKind;
                }
                FileRead::OverLimit => return FingerprintWalk::OverLimit,
                FileRead::BudgetExhausted => {
                    return FingerprintWalk::BudgetExhausted;
                }
                FileRead::Unreadable => return FingerprintWalk::Unreadable,
            };
            total_bytes += bytes.len() as u64;
            if total_bytes > FINGERPRINT_MAX_TOTAL_BYTES {
                return FingerprintWalk::OverLimit;
            }
            files.push(MarkerFileEntry {
                path: relative,
                sha256: sha256_hex(&bytes),
            });
        }
    }
    FingerprintWalk::Entries(files)
}

/// Lowercase hex SHA-256 of `bytes` — the digest of content this adapter
/// read, identical alphabet to the manifest/digest fields.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

// ---------------------------------------------------------------------------
// Observation endpoint — the consumer-mirroring `lstat` admission, private
// to this module (Root43 E-8: no shared public inspector for one consumer).
// ---------------------------------------------------------------------------

/// The endpoint socket's state under the consumers' own admission
/// predicate: socket kind plus effective-uid ownership. `mode` is an
/// informational detail only — a wrong-mode socket that still delivers
/// must not self-contradict the diagnosis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SocketFact {
    /// A socket owned by the effective user.
    Present { mode: u32 },
    /// No entry exists at the path.
    Absent,
    /// An entry exists but is not a socket.
    NotASocket { kind: EntryKind },
    /// A socket owned by another user.
    WrongOwner { owner: u32, mode: u32 },
    /// The final component is a symlink — never followed.
    UnsafeSymlink,
    /// Classification failed.
    Unreadable,
}

/// The observation endpoint filename inside the native runtime directory.
pub(crate) const OBSERVATION_SOCKET_NAME: &str = "agent-observation-v1.sock";

/// The endpoint path the hook payloads resolve:
/// `ORCHBEE_AGENT_OBSERVATION_SOCKET` when it carries a non-empty value,
/// else `<ORCHBEE_RUNTIME_BASE || /tmp>/orchbee-native-<euid>/agent-observation-v1.sock`.
/// Pure path derivation — no inspection.
pub(crate) fn observation_socket_path(euid: u32, observation: Observation) -> PathBuf {
    if let EnvValue::Value(value) = env_value(super::probes::OBSERVATION_SOCKET_ENV, observation)
        && !value.is_empty()
    {
        return path_from_bytes(&value);
    }
    let base = match env_value("ORCHBEE_RUNTIME_BASE", observation) {
        EnvValue::Value(value) if !value.is_empty() => path_from_bytes(&value),
        _ => PathBuf::from("/tmp"),
    };
    base.join(format!("orchbee-native-{euid}"))
        .join(OBSERVATION_SOCKET_NAME)
}

/// The endpoint probe: `lstat` of the final component — never a connect —
/// admitting exactly what the consumers admit (kind + owner).
pub(crate) fn observation_socket(path: &Path, euid: u32, observation: Observation) -> SocketFact {
    match lstat(path, observation) {
        Lstat::Absent => SocketFact::Absent,
        Lstat::Symlink => SocketFact::UnsafeSymlink,
        Lstat::Unreadable => SocketFact::Unreadable,
        Lstat::Entry {
            kind, ownership, ..
        } => {
            if kind != EntryKind::Socket {
                return SocketFact::NotASocket { kind };
            }
            match ownership {
                Some(Ownership { uid, mode }) if uid == euid => SocketFact::Present { mode },
                Some(Ownership { uid, mode }) => SocketFact::WrongOwner { owner: uid, mode },
                None => SocketFact::Unreadable,
            }
        }
    }
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{path_admitted, path_from_bytes};
