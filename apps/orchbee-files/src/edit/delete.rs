//! `fs.delete` and `fs.restore`. A delete moves each item to the machine's trash, or removes it
//! permanently, as its mode says: a link itself, never what it names, and a folder with everything
//! in it. An item the trash does not take stays where it is and answers `trash-unavailable` or
//! `trash-failed`, for the Client to confirm a permanent delete, as VS Code confirms one; a delete
//! never falls back to removing it permanently.
//!
//! A capturing delete first keeps what VS Code's undo keeps of each item it deletes: the content of
//! a file of at most 5,000,000 bytes, and a folder, which a restore recreates empty. A link is kept
//! as what it names, as VS Code reads it. Stashes live in this process, at most 64 MiB of paths and
//! content in all, the oldest evicted first, and are lost on exit; a delete whose capture would
//! pass that keeps nothing. A restore takes its stash and recreates folders first, then files,
//! never over an item that took a name meanwhile. A stash that was evicted, taken or never kept
//! answers `undo-expired`.

use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{self, Write as _},
    path::Path,
    sync::{Mutex, MutexGuard, PoisonError},
};

use orchbee_files_protocol::{
    DeleteMode, ErrorCode, FilePath, FilesError, Operation, PathItem, StashId, WorkspaceId,
};

use super::entries;
use crate::{
    blocking::Helpers,
    git::ports::{Trash, TrashError},
    scope::path::NativePath,
};

/// The largest file a capture keeps, VS Code's `MAX_UNDO_FILE_SIZE`.
const MAXIMUM_FILE_BYTES: u64 = 5_000_000;

/// The paths and content all stashes of the process hold at most.
const BUDGET_BYTES: usize = 64 * 1024 * 1024;

/// One deleted item a stash keeps, under the path its delete named.
pub(super) struct Kept {
    path: FilePath,
    /// A file's content; a folder has none.
    content: Option<Vec<u8>>,
}

impl Kept {
    /// What it holds of the budget: its path and content.
    fn bytes(&self) -> usize {
        self.path.as_str().len() + self.content.as_ref().map_or(0, Vec::len)
    }
}

/// The undo stashes of the process.
pub(super) struct Stashes {
    held: Mutex<Held>,
}

struct Held {
    stashes: VecDeque<Stash>,
    bytes: usize,
    budget: usize,
}

struct Stash {
    id: StashId,
    workspace: WorkspaceId,
    items: Vec<Kept>,
    bytes: usize,
}

impl Stashes {
    pub(super) fn new() -> Self {
        Self::with_budget(BUDGET_BYTES)
    }

    fn with_budget(budget: usize) -> Self {
        Self {
            held: Mutex::new(Held {
                stashes: VecDeque::new(),
                bytes: 0,
                budget,
            }),
        }
    }

    /// Keeps `items` for `workspace`, evicting the oldest stashes until they fit, and names the
    /// stash; `None` when there is nothing to keep, more than the budget, or no identity to name
    /// it.
    pub(super) fn keep(&self, workspace: WorkspaceId, items: Vec<Kept>) -> Option<StashId> {
        let bytes = items.iter().map(Kept::bytes).sum();
        let mut held = self.lock();
        if items.is_empty() || bytes > held.budget {
            return None;
        }
        let id = identity()?;
        while held.bytes + bytes > held.budget {
            let Some(evicted) = held.stashes.pop_front() else {
                break;
            };
            held.bytes -= evicted.bytes;
        }
        held.bytes += bytes;
        held.stashes.push_back(Stash {
            id,
            workspace,
            items,
            bytes,
        });
        Some(id)
    }

    /// Takes the stash `id` a delete in `workspace` kept.
    pub(super) fn take(&self, workspace: WorkspaceId, id: StashId) -> Option<Vec<Kept>> {
        let mut held = self.lock();
        let at = held
            .stashes
            .iter()
            .position(|stash| stash.id == id && stash.workspace == workspace)?;
        let stash = held.stashes.remove(at)?;
        held.bytes -= stash.bytes;
        Some(stash.items)
    }

    fn lock(&self) -> MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A fresh stash identity.
fn identity() -> Option<StashId> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).ok()?;
    StashId::from_bytes(bytes)
}

/// Admits a path for the change a delete or restore makes, as scope admits it then.
pub(super) type Admit<'a> = dyn Fn(&FilePath) -> Result<NativePath, FilesError> + 'a;

/// One delete, besides its paths.
pub(super) struct Deleting<'a> {
    pub(super) helpers: &'a Helpers,
    pub(super) trash: &'a dyn Trash,
    pub(super) mode: DeleteMode,
    pub(super) capture: bool,
    pub(super) cancelled: &'a dyn Fn() -> bool,
}

impl Deleting<'_> {
    /// Deletes each of `paths` in order, each once `admit` admits it when its turn comes, and
    /// answers each, with what a capture kept of the items it deleted.
    pub(super) fn run(&self, paths: &[FilePath], admit: &Admit<'_>) -> (Vec<PathItem>, Vec<Kept>) {
        let (mut kept, mut bytes, mut capturing) = (Vec::new(), 0_usize, self.capture);
        let items = paths
            .iter()
            .map(|path| {
                let deleted = self.item(path, admit, capturing).map(|captured| {
                    let Some(captured) = captured else {
                        return;
                    };
                    bytes = bytes.saturating_add(captured.bytes());
                    kept.push(captured);
                    // A capture past the budget could never be kept, so it holds nothing more.
                    if bytes > BUDGET_BYTES {
                        capturing = false;
                        kept = Vec::new();
                    }
                });
                PathItem {
                    path: path.clone(),
                    error: deleted.err(),
                }
            })
            .collect();
        (items, kept)
    }

    fn item(
        &self,
        path: &FilePath,
        admit: &Admit<'_>,
        capturing: bool,
    ) -> Result<Option<Kept>, FilesError> {
        if (self.cancelled)() {
            return Err(FilesError::new(ErrorCode::Cancelled));
        }
        let target = admit(path)?;
        let captured = if capturing {
            self.capture(path, &target)
        } else {
            None
        };
        self.delete(target.as_path())?;
        Ok(captured)
    }

    /// What VS Code's undo keeps of the item at `target`: its content when it reads as a file of
    /// at most 5,000,000 bytes, and itself when it reads as a folder. An item it cannot read is
    /// not kept.
    fn capture(&self, path: &FilePath, target: &NativePath) -> Option<Kept> {
        let read = target.as_path().to_path_buf();
        let content = entries::run(self.helpers, Operation::FsDelete, move || {
            let facts = fs::metadata(&read)?;
            if facts.is_dir() {
                Ok(Some(None))
            } else if facts.is_file() && facts.len() <= MAXIMUM_FILE_BYTES {
                fs::read(&read).map(|content| Some(Some(content)))
            } else {
                Ok(None)
            }
        });
        let content = content.ok().flatten()?;
        Some(Kept {
            path: path.clone(),
            content,
        })
    }

    /// Moves the item at `target` to the trash or removes it, once a helper found something
    /// there, so a volume that does not answer is a `timeout` before either starts. Either runs on
    /// this thread, since its time grows with the item.
    fn delete(&self, target: &Path) -> Result<(), FilesError> {
        let found = target.to_path_buf();
        entries::run(self.helpers, Operation::FsDelete, move || {
            fs::symlink_metadata(found).map(drop)
        })?;
        match self.mode {
            DeleteMode::Permanent => entries::long(|| entries::remove(target)),
            DeleteMode::Trash => self.trash.trash(target).map_err(|refused| {
                FilesError::new(match refused {
                    TrashError::Unavailable => ErrorCode::TrashUnavailable,
                    TrashError::Failed => ErrorCode::TrashFailed,
                })
            }),
        }
    }
}

/// Recreates each item `kept` holds, folders first, then files, each once `admit` admits it, and
/// answers each in the order the stash held them.
pub(super) fn restore(helpers: &Helpers, kept: Vec<Kept>, admit: &Admit<'_>) -> Vec<PathItem> {
    let mut answers: Vec<Option<PathItem>> = kept.iter().map(|_| None).collect();
    let (folders, files): (Vec<_>, Vec<_>) = kept
        .into_iter()
        .enumerate()
        .partition(|(_, kept)| kept.content.is_none());
    for (at, Kept { path, content }) in folders.into_iter().chain(files) {
        let restored = admit(&path).and_then(|target| {
            let target = target.as_path().to_path_buf();
            entries::run(helpers, Operation::FsRestore, move || {
                recreate(&target, content)
            })
        });
        answers[at] = Some(PathItem {
            path,
            error: restored.err(),
        });
    }
    answers.into_iter().flatten().collect()
}

/// Recreates a folder with the folders above it, which may exist already, or a file
/// exclusively, with `content`.
fn recreate(path: &Path, content: Option<Vec<u8>>) -> io::Result<()> {
    let Some(content) = content else {
        return entries::create_folders(path);
    };
    if let Some(folder) = path.parent() {
        entries::create_folders(folder)?;
    }
    let written = File::create_new(path)?.write_all(&content);
    if written.is_err() {
        // A file the restore could not fill is not left half restored.
        let _ = fs::remove_file(path);
    }
    written
}

#[cfg(test)]
mod tests;
