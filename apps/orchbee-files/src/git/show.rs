//! `git.show`: a file's content at `HEAD`, in the index or at a commit, as VS Code's Git extension
//! reads it with `git show --textconv`, served through the read transfer the file engine opened.
//! Each show first asks which blob the reference names and keeps content by repository, path and
//! that blob, so a reference that moved never answers with its old content. Content stays for 3
//! minutes after its last read ends, swept every 5 minutes, and at most 64 MiB stays in all.
//! Content past the Client's limit is `too-large`, and a version without the path is `not-found`.

use super::{
    command::{self, Access},
    discovery::Found,
    errors::{Classifier, GitError},
    output::GitOutput,
    ports::{Content, ShowRead},
    runner::{self, Flow, Limits},
};
use orchbee_files_protocol::ErrorCode;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

/// The most content kept in all, in bytes.
const KEPT_BYTES: usize = 64 * 1024 * 1024;
/// How long content stays after its last read ends.
const KEPT_FOR: Duration = Duration::from_secs(3 * 60);
/// How often content that outlived its stay is dropped.
pub(super) const SWEEP_EVERY: Duration = Duration::from_secs(5 * 60);
/// The deadline of each of a show's two commands.
const DEADLINE: Duration = Duration::from_secs(30);

/// The version of a file a show reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Reference {
    Head,
    Index,
    /// A full object name.
    Commit(String),
}

impl Reference {
    /// Git's name for the object at `relative`, a path below the repository spelled with `/`.
    fn object(&self, relative: &str) -> String {
        match self {
            Self::Head => format!("HEAD:{relative}"),
            // The stage is always named, so a path that starts with a digit and a colon is never
            // read as one.
            Self::Index => format!(":0:{relative}"),
            Self::Commit(commit) => format!("{commit}:{relative}"),
        }
    }
}

/// Serves `path`'s content in `reference` through `read`, from `shows` or from Git.
pub(super) fn serve(
    found: &Found,
    reference: &Reference,
    path: &str,
    shows: &Mutex<Shows>,
    read: Box<dyn ShowRead>,
    output: &Mutex<GitOutput>,
) {
    let limits = Limits::new("show", DEADLINE, read.cancellation());
    match content(found, reference, path, shows, read.limit(), &limits, output) {
        Ok(content) => read.supply(content),
        Err(error) => fail(read, error),
    }
}

/// Answers `read` with `error`.
pub(super) fn fail(read: Box<dyn ShowRead>, error: GitError) {
    read.fail(error.into_wire());
}

/// `path`'s content in `reference`, at most `limit` bytes of it.
///
/// # Errors
///
/// `outside-scope` for a path outside the repository, `not-found` when the version has no such
/// path, `too-large` past `limit`, or how Git failed.
pub(super) fn content(
    found: &Found,
    reference: &Reference,
    path: &str,
    shows: &Mutex<Shows>,
    limit: u64,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Content, GitError> {
    let relative = found.relative(path)?;
    let mut command = found.command(Access::Read, None);
    command
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(reference.object(&relative));
    let named = runner::run(command, Vec::new(), limits, output)?;
    // `--quiet` turns a name that resolves to nothing into exit status 1 without a message.
    if named.exited_with(1) {
        return Err(GitError::new(ErrorCode::NotFound));
    }
    let named = named.checked(Classifier::General)?;
    let blob = String::from_utf8_lossy(&named.stdout).trim().to_owned();
    if !command::is_object_name(&blob) {
        return Err(GitError::new(ErrorCode::Failed));
    }
    let key = Key {
        repository: found.toplevel.clone(),
        path: relative,
        blob,
    };
    if let Some(kept) = lock(shows).get(&key) {
        if (*kept).as_ref().len() as u64 > limit {
            return Err(GitError::new(ErrorCode::TooLarge));
        }
        return Ok(kept);
    }
    // The blob is read by its name with the path beside it, as `git show --textconv` reads
    // `<ref>:<path>`, so a reference that moves meanwhile cannot change what is kept for it.
    let mut command = found.command(Access::Read, None);
    command
        .args(["cat-file", "--textconv"])
        .arg(format!("--path={}", key.path))
        .arg(&key.blob);
    let mut bytes = Vec::new();
    let mut over = false;
    let ran = runner::stream(command, Vec::new(), limits, output, |piece| {
        if (bytes.len() + piece.len()) as u64 > limit {
            over = true;
            return Flow::Enough;
        }
        bytes.extend_from_slice(piece);
        Flow::More
    })?;
    if over {
        return Err(GitError::new(ErrorCode::TooLarge));
    }
    ran.checked(Classifier::General)?;
    Ok(lock(shows).insert(key, bytes))
}

/// Content is kept by repository, path below it and the blob a reference named.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Key {
    repository: String,
    path: String,
    blob: String,
}

/// One version's content and when its last read ended.
struct Kept {
    bytes: Vec<u8>,
    ended: Mutex<Option<Instant>>,
}

/// What a read holds of kept content; its drop is the read's end, from which the stay counts.
struct Lease(Arc<Kept>);

impl AsRef<[u8]> for Lease {
    fn as_ref(&self) -> &[u8] {
        &self.0.bytes
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        *lock(&self.0.ended) = Some(Instant::now());
    }
}

/// The content `git.show` keeps. Content a read holds is never dropped; the bound makes room by
/// dropping the content whose last read ended longest ago, and content that cannot fit is served
/// without being kept.
pub(super) struct Shows {
    kept: HashMap<Key, Arc<Kept>>,
    bytes: usize,
    capacity: usize,
}

impl Shows {
    pub(super) fn new() -> Self {
        Self::with_capacity(KEPT_BYTES)
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            kept: HashMap::new(),
            bytes: 0,
            capacity,
        }
    }

    fn get(&self, key: &Key) -> Option<Content> {
        self.kept.get(key).map(lease)
    }

    /// Keeps `bytes` under `key` when they fit, and returns them for a read; content another
    /// show kept under `key` meanwhile is returned instead.
    fn insert(&mut self, key: Key, bytes: Vec<u8>) -> Content {
        if let Some(kept) = self.kept.get(&key) {
            return lease(kept);
        }
        let size = bytes.len();
        let kept = Arc::new(Kept {
            bytes,
            ended: Mutex::new(None),
        });
        if self.make_room(size) {
            self.bytes += size;
            self.kept.insert(key, Arc::clone(&kept));
        }
        lease(&kept)
    }

    /// Drops content no read holds, the longest ended first, until `size` more bytes fit; false,
    /// with nothing dropped, when they cannot.
    fn make_room(&mut self, size: usize) -> bool {
        let mut idle = self
            .kept
            .iter()
            .filter(|(_, kept)| Arc::strong_count(kept) == 1)
            .map(|(key, kept)| (*lock(&kept.ended), kept.bytes.len(), key.clone()))
            .collect::<Vec<_>>();
        let freeable = idle.iter().map(|(_, bytes, _)| bytes).sum::<usize>();
        if (self.bytes - freeable).saturating_add(size) > self.capacity {
            return false;
        }
        idle.sort_by_key(|(ended, _, _)| *ended);
        let mut idle = idle.into_iter();
        while self.bytes + size > self.capacity {
            let Some((_, bytes, key)) = idle.next() else {
                break;
            };
            self.kept.remove(&key);
            self.bytes -= bytes;
        }
        true
    }

    /// Drops the content no read holds whose last read ended at least 3 minutes before `now`.
    pub(super) fn sweep(&mut self, now: Instant) {
        self.kept.retain(|_, kept| {
            Arc::strong_count(kept) > 1
                || lock(&kept.ended)
                    .is_some_and(|ended| now.saturating_duration_since(ended) < KEPT_FOR)
        });
        self.bytes = self.kept.values().map(|kept| kept.bytes.len()).sum();
    }
}

fn lease(kept: &Arc<Kept>) -> Content {
    Arc::new(Lease(Arc::clone(kept)))
}

/// A panic ends the process, so a poisoned lock is never observed.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
