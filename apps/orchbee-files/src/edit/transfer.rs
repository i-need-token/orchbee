//! `fs.move` and `fs.copy`: each source goes into the target directory under its own name, one
//! after another, and each is answered on its own. Where the name is taken, the request's conflict
//! rule decides as VS Code does: `fail` answers `exists`; `increment` takes VS Code's next free
//! copy name, claimed exclusively; `replace` removes the existing item permanently first; and
//! `merge` combines a folder with an existing folder, each child by the same rule, and replaces
//! anything else. Links move, copy and are replaced as links, never what they name.
//!
//! - A move renames, as [`entries::relocate`] does; to another volume it copies, then removes the
//!   source, as VS Code moves.
//! - A copy creates each item exclusively and copies a file with the operating system's copy, which
//!   keeps its permissions. A copied link names what its source named, or the same place in the
//!   copy where that lies inside the copied item, as VS Code copies a link. Windows without the
//!   privilege to create a link copies what the link names instead, as VS Code does then.
//! - A copy that fails or is cancelled removes what it created; a merge keeps what it combined.
//!   A cancelled transfer answers `cancelled` for the item it stopped in and each after it.
//! - An item is never replaced or merged onto when that would remove its source, however the two
//!   paths spell their folders.

use std::{
    collections::HashSet,
    ffi::OsString,
    fs::{self, File, FileType, Permissions},
    io,
    path::{Path, PathBuf},
};

use orchbee_files_protocol::{Conflict, ErrorCode, FilePath, FilesError, Operation, TransferItem};

use super::{entries, names};
use crate::{blocking::Helpers, error, scope::path::NativePath};

/// The most names an `increment` tries before it answers `exists`.
const MAXIMUM_ATTEMPTS: usize = 10_000;

/// What a transfer does with its sources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Transfer {
    /// `fs.move`.
    Move,
    /// `fs.copy`.
    Copy,
}

impl Transfer {
    pub(super) const fn operation(self) -> Operation {
        match self {
            Self::Move => Operation::FsMove,
            Self::Copy => Operation::FsCopy,
        }
    }
}

/// A source's path and its target in the target directory, as scope admits them.
pub(super) type Admitted = Result<(NativePath, NativePath), FilesError>;

/// How one source ended: its answer, and where it went when it went.
pub(super) struct Transferred {
    pub(super) item: TransferItem,
    /// The source and its final path.
    pub(super) done: Option<(NativePath, NativePath)>,
}

/// Transfers each of `sources` as `transfer` says, in order, and answers each. `admit` gives a
/// source's path and its target in the target directory, as scope admits them, when the source's
/// turn comes.
pub(super) fn run(
    helpers: &Helpers,
    transfer: Transfer,
    sources: &[FilePath],
    admit: &dyn Fn(&FilePath) -> Admitted,
    conflict: Conflict,
    cancelled: &dyn Fn() -> bool,
) -> Vec<Transferred> {
    let transferring = Transferring {
        helpers,
        transfer,
        conflict,
        cancelled,
    };
    sources
        .iter()
        .map(|requested| {
            let outcome = if cancelled() {
                Err(failed(ErrorCode::Cancelled))
            } else {
                admit(requested)
                    .map_err(Failed::from)
                    .and_then(|(source, target)| {
                        let path = transferring.item(&source, &target)?;
                        let wired =
                            wire(path.as_path()).ok_or_else(|| failed(ErrorCode::Failed))?;
                        Ok((wired, source, path))
                    })
            };
            let (path, error, done) = match outcome {
                Ok((wired, source, path)) => (Some(wired), None, Some((source, path))),
                Err(Failed { error, copied }) => {
                    (copied.as_deref().and_then(wire), Some(*error), None)
                }
            };
            Transferred {
                item: TransferItem {
                    source: requested.clone(),
                    path,
                    error,
                },
                done,
            }
        })
        .collect()
}

/// Why an item failed, and where a move to another volume copied it when it could not remove
/// the source. The error is boxed, so a step's result stays small.
struct Failed {
    error: Box<FilesError>,
    copied: Option<PathBuf>,
}

impl From<FilesError> for Failed {
    fn from(error: FilesError) -> Self {
        Self {
            error: Box::new(error),
            copied: None,
        }
    }
}

/// Why an item could not take a name.
enum Placing {
    /// Something is there.
    Taken,
    Failed(Failed),
}

impl From<FilesError> for Placing {
    fn from(error: FilesError) -> Self {
        Self::Failed(error.into())
    }
}

/// The item a copy started from and where it goes: a copied link that named a place inside the
/// first names the same place inside the second.
#[derive(Clone)]
struct Roots {
    source: NativePath,
    target: NativePath,
}

/// What one step of a copy created.
enum Copied {
    /// The item, whole.
    Done,
    /// A file, empty, whose content follows.
    File,
    /// A folder, whose children follow, and which takes the source's permissions after them. A
    /// folder copied from what a link names has that real location, which a copy enters once.
    Folder {
        names: Vec<OsString>,
        permissions: Permissions,
        real: Option<PathBuf>,
    },
}

/// One step of filling a copied folder.
enum Filling {
    Item(PathBuf, PathBuf),
    Permissions(PathBuf, Permissions),
}

/// One step of a merge.
enum Merging {
    /// A folder onto an existing folder: each child follows.
    Folder(PathBuf, PathBuf),
    /// A child: placed where its name is free, merged onto a folder, and otherwise replacing.
    Child(PathBuf, PathBuf),
    /// A moved folder whose children all went, which goes too.
    Emptied(PathBuf),
}

struct Transferring<'a> {
    helpers: &'a Helpers,
    transfer: Transfer,
    conflict: Conflict,
    cancelled: &'a dyn Fn() -> bool,
}

impl Transferring<'_> {
    const fn operation(&self) -> Operation {
        self.transfer.operation()
    }

    fn step<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> io::Result<T> + Send + 'static,
    ) -> Result<T, FilesError> {
        entries::run(self.helpers, self.operation(), work)
    }

    fn item(&self, source: &NativePath, target: &NativePath) -> Result<NativePath, Failed> {
        let path = source.as_path().to_path_buf();
        let (kind, folder_name) = self.step(move || {
            let kind = fs::symlink_metadata(&path)?.file_type();
            // VS Code names a copy of a link to a folder as a folder's, with no extension.
            let folder_name = fs::metadata(&path).is_ok_and(|facts| facts.is_dir());
            Ok((kind, folder_name))
        })?;
        if self.transfer == Transfer::Copy
            && !(kind.is_file() || kind.is_dir() || kind.is_symlink())
        {
            return Err(failed(ErrorCode::Failed));
        }
        if target.key() == source.key() {
            return match (self.transfer, self.conflict) {
                (Transfer::Copy, Conflict::Increment) => {
                    self.increment(source, target, &kind, folder_name)
                }
                (Transfer::Copy, Conflict::Fail) => Err(failed(ErrorCode::Exists)),
                // A move to where the item is, or a copy onto itself, leaves it as it is, as in
                // VS Code.
                _ => Ok(source.clone()),
            };
        }
        if target.is_within(source) {
            return Err(failed(ErrorCode::InvalidRequest));
        }
        if let Some(folder) = target.as_path().parent().map(Path::to_path_buf) {
            self.step(move || entries::create_folders(&folder))?;
        }
        match self.place(
            source.as_path(),
            target.as_path(),
            &kind,
            &roots(source, target),
        ) {
            Ok(()) => Ok(target.clone()),
            Err(Placing::Failed(failure)) => Err(failure),
            Err(Placing::Taken) => self.resolve(source, target, &kind, folder_name),
        }
    }

    /// Settles an item whose target is taken, as the conflict rule says.
    fn resolve(
        &self,
        source: &NativePath,
        target: &NativePath,
        kind: &FileType,
        folder_name: bool,
    ) -> Result<NativePath, Failed> {
        match self.conflict {
            Conflict::Fail => return Err(failed(ErrorCode::Exists)),
            Conflict::Increment => return self.increment(source, target, kind, folder_name),
            Conflict::Replace | Conflict::Merge => {}
        }
        let (existing, moved) = (
            target.as_path().to_path_buf(),
            source.as_path().to_path_buf(),
        );
        let (holds, folder) = self.step(move || {
            let folder = fs::symlink_metadata(&existing)?.file_type().is_dir();
            Ok((holds(&existing, &moved)?, folder))
        })?;
        // Replacing or merging onto what holds the source would take the source with it.
        if holds || source.is_within(target) {
            return Err(failed(ErrorCode::InvalidRequest));
        }
        if self.conflict == Conflict::Merge && kind.is_dir() && folder {
            return self.merge(source, target).map(|()| target.clone());
        }
        entries::long(|| entries::remove(target.as_path()))?;
        match self.place(
            source.as_path(),
            target.as_path(),
            kind,
            &roots(source, target),
        ) {
            Ok(()) => Ok(target.clone()),
            Err(Placing::Failed(failure)) => Err(failure),
            Err(Placing::Taken) => Err(failed(ErrorCode::Exists)),
        }
    }

    /// Places the item at VS Code's next free copy name after `target`'s.
    fn increment(
        &self,
        source: &NativePath,
        target: &NativePath,
        kind: &FileType,
        folder_name: bool,
    ) -> Result<NativePath, Failed> {
        let directory = target
            .parent()
            .ok_or_else(|| failed(ErrorCode::InvalidName))?;
        let mut name = target.name().to_owned();
        for _ in 0..MAXIMUM_ATTEMPTS {
            name = names::increment(&name, folder_name);
            let candidate = names::child(&directory, &name)?;
            let roots = roots(source, &candidate);
            match self.place(source.as_path(), candidate.as_path(), kind, &roots) {
                Ok(()) => return Ok(candidate),
                Err(Placing::Taken) => {}
                Err(Placing::Failed(failure)) => return Err(failure),
            }
        }
        Err(failed(ErrorCode::Exists))
    }

    /// Moves or copies the item at `from`, of type `kind`, to `to`, where nothing may be.
    fn place(&self, from: &Path, to: &Path, kind: &FileType, roots: &Roots) -> Result<(), Placing> {
        if self.transfer == Transfer::Copy {
            return self.copy(from, to, roots);
        }
        let (source, target, moved) = (from.to_path_buf(), to.to_path_buf(), *kind);
        let renamed = self.helpers.run(self.operation(), move || {
            entries::relocate(&source, &target, &moved)
        })?;
        match renamed {
            Ok(()) => Ok(()),
            Err(failure) if failure.kind() == io::ErrorKind::AlreadyExists => Err(Placing::Taken),
            Err(failure) if failure.kind() == io::ErrorKind::CrossesDevices => {
                self.copy(from, to, roots)?;
                // Once copied, the source goes, as VS Code moves across volumes.
                entries::long(|| entries::remove(from)).map_err(|error| {
                    Placing::Failed(Failed {
                        error: Box::new(error),
                        copied: Some(to.to_path_buf()),
                    })
                })
            }
            Err(failure) => Err(error::from_io(&failure).into()),
        }
    }

    /// Copies the item at `from` to `to`, where nothing may be, and removes what it created when
    /// it fails or is cancelled.
    fn copy(&self, from: &Path, to: &Path, roots: &Roots) -> Result<(), Placing> {
        let (source, target, linked) = (from.to_path_buf(), to.to_path_buf(), roots.clone());
        let first = self.helpers.run(self.operation(), move || {
            copy_one(&source, &target, &linked)
        })?;
        let first = match first {
            Ok(first) => first,
            Err(failure) if failure.kind() == io::ErrorKind::AlreadyExists => {
                return Err(Placing::Taken);
            }
            Err(failure) => return Err(error::from_io(&failure).into()),
        };
        self.fill(from, to, roots, first).map_err(|failure| {
            let _ = entries::long(|| entries::remove(to));
            Placing::Failed(failure)
        })
    }

    /// Copies the children of what the first step of a copy created, folder by folder.
    fn fill(&self, from: &Path, to: &Path, roots: &Roots, first: Copied) -> Result<(), Failed> {
        let mut filling = Vec::new();
        let mut entered = HashSet::new();
        let mut real_target = None;
        let mut follow = |filling: &mut Vec<Filling>, from: &Path, to: &Path, copied: Copied| {
            let (names, permissions, real) = match copied {
                Copied::Done => return Ok::<(), FilesError>(()),
                Copied::File => return entries::long(|| fs::copy(from, to).map(drop)),
                Copied::Folder {
                    names,
                    permissions,
                    real,
                } => (names, permissions, real),
            };
            if let Some(real) = real {
                // What a link names is copied once, and never from inside the copy itself.
                if real_target.is_none() {
                    let target = roots.target.as_path().to_path_buf();
                    real_target = Some(self.step(move || fs::canonicalize(&target))?);
                }
                let inside = real_target
                    .as_ref()
                    .is_some_and(|inside| real.starts_with(inside));
                if inside || !entered.insert(real) {
                    return Ok(());
                }
            }
            filling.push(Filling::Permissions(to.to_path_buf(), permissions));
            for name in names.into_iter().rev() {
                filling.push(Filling::Item(from.join(&name), to.join(&name)));
            }
            Ok(())
        };
        follow(&mut filling, from, to, first)?;
        while let Some(step) = filling.pop() {
            if (self.cancelled)() {
                return Err(failed(ErrorCode::Cancelled));
            }
            match step {
                Filling::Item(from, to) => {
                    let (source, target, linked) = (from.clone(), to.clone(), roots.clone());
                    let copied = self.step(move || copy_one(&source, &target, &linked))?;
                    follow(&mut filling, &from, &to, copied)?;
                }
                Filling::Permissions(to, permissions) => {
                    self.step(move || finish_folder(&to, permissions))?;
                }
            }
        }
        Ok(())
    }

    /// Merges the folder at `source` onto the existing folder at `target`.
    fn merge(&self, source: &NativePath, target: &NativePath) -> Result<(), Failed> {
        let roots = roots(source, target);
        let mut merging = vec![Merging::Folder(
            source.as_path().to_path_buf(),
            target.as_path().to_path_buf(),
        )];
        while let Some(step) = merging.pop() {
            if (self.cancelled)() {
                return Err(failed(ErrorCode::Cancelled));
            }
            let (from, to) = match step {
                Merging::Folder(from, to) => {
                    let folder = from.clone();
                    let names = self.step(move || list(&folder))?;
                    if self.transfer == Transfer::Move {
                        merging.push(Merging::Emptied(from.clone()));
                    }
                    for name in names.into_iter().rev() {
                        merging.push(Merging::Child(from.join(&name), to.join(&name)));
                    }
                    continue;
                }
                Merging::Emptied(folder) => {
                    self.step(move || fs::remove_dir(&folder))?;
                    continue;
                }
                Merging::Child(from, to) => (from, to),
            };
            let (source, existing) = (from.clone(), to.clone());
            let (kind, taken) = self.step(move || {
                let kind = fs::symlink_metadata(&source)?.file_type();
                let taken = match fs::symlink_metadata(&existing) {
                    Ok(facts) => Some(facts.file_type()),
                    Err(failure) if failure.kind() == io::ErrorKind::NotFound => None,
                    Err(failure) => return Err(failure),
                };
                Ok((kind, taken))
            })?;
            match taken {
                Some(existing) if kind.is_dir() && existing.is_dir() => {
                    merging.push(Merging::Folder(from, to));
                    continue;
                }
                Some(_) => entries::long(|| entries::remove(&to))?,
                None => {}
            }
            match self.place(&from, &to, &kind, &roots) {
                Ok(()) => {}
                Err(Placing::Taken) => return Err(failed(ErrorCode::Exists)),
                // Where a child's move to another volume left its copy is not the item's path.
                Err(Placing::Failed(Failed { error, .. })) => {
                    return Err(Failed {
                        error,
                        copied: None,
                    });
                }
            }
        }
        Ok(())
    }
}

fn roots(source: &NativePath, target: &NativePath) -> Roots {
    Roots {
        source: source.clone(),
        target: target.clone(),
    }
}

/// Whether removing the item at `target` would remove the one at `source`: it is that item, or a
/// folder holding it, by their real locations, however each path spells its folders. Removing a
/// link removes nothing it names.
fn holds(target: &Path, source: &Path) -> io::Result<bool> {
    if fs::symlink_metadata(target)?.file_type().is_symlink() {
        return Ok(false);
    }
    let real_target = fs::canonicalize(target)?;
    let real_source = if fs::symlink_metadata(source)?.file_type().is_symlink() {
        // A link lies where its folder really is.
        match source.parent() {
            Some(folder) => fs::canonicalize(folder)?,
            None => return Ok(false),
        }
    } else {
        fs::canonicalize(source)?
    };
    Ok(real_source.starts_with(real_target))
}

/// Copies the item at `from` to `to`, where nothing may be: a link as a link, a file empty, with
/// its content to follow, and a folder empty, with its children to follow.
fn copy_one(from: &Path, to: &Path, roots: &Roots) -> io::Result<Copied> {
    let facts = fs::symlink_metadata(from)?;
    let kind = facts.file_type();
    if kind.is_symlink() {
        return copy_link(from, to, &kind, roots);
    }
    if kind.is_dir() {
        fs::create_dir(to)?;
        return Ok(Copied::Folder {
            names: list(from)?,
            permissions: facts.permissions(),
            real: None,
        });
    }
    if kind.is_file() {
        File::create_new(to)?;
        return Ok(Copied::File);
    }
    // A pipe, socket or device inside a copied folder has no content to copy and stays behind.
    Ok(Copied::Done)
}

/// Copies the link at `from`, of type `kind`, to `to`.
fn copy_link(from: &Path, to: &Path, kind: &FileType, roots: &Roots) -> io::Result<Copied> {
    let named = rebase(fs::read_link(from)?, roots);
    match link(&named, to, kind) {
        #[cfg(windows)]
        Err(failure) if failure.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) => {
            // Without the privilege to create a link, the copy holds what the link names.
            let facts = fs::metadata(from).map_err(|_| failure)?;
            if facts.is_dir() {
                fs::create_dir(to)?;
                Ok(Copied::Folder {
                    names: list(from)?,
                    permissions: facts.permissions(),
                    real: Some(fs::canonicalize(from)?),
                })
            } else {
                File::create_new(to)?;
                Ok(Copied::File)
            }
        }
        linked => linked.map(|()| Copied::Done),
    }
}

/// Windows refuses a link to an account without the privilege to create one.
#[cfg(windows)]
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

#[cfg(unix)]
fn link(named: &Path, to: &Path, _kind: &FileType) -> io::Result<()> {
    std::os::unix::fs::symlink(named, to)
}

#[cfg(windows)]
fn link(named: &Path, to: &Path, kind: &FileType) -> io::Result<()> {
    use std::os::windows::fs::{FileTypeExt, symlink_dir, symlink_file};
    if kind.is_symlink_dir() {
        symlink_dir(named, to)
    } else {
        symlink_file(named, to)
    }
}

/// What a copied link names: what its source named, or the same place below the copy's target
/// where that lies below the copy's source, as VS Code rebases an absolute link.
fn rebase(named: PathBuf, roots: &Roots) -> PathBuf {
    let below = named.to_str().and_then(|text| {
        // Windows reads an absolute link in the verbatim form.
        #[cfg(windows)]
        let text = text.strip_prefix(r"\\?\").unwrap_or(text);
        let path = NativePath::parse(text).ok()?;
        let below: Vec<String> = path.below(&roots.source)?.map(str::to_owned).collect();
        Some(below)
    });
    match below {
        Some(below) => below
            .iter()
            .fold(roots.target.as_path().to_path_buf(), |path, segment| {
                path.join(segment)
            }),
        None => named,
    }
}

/// The names in the folder at `folder`, byte for byte.
fn list(folder: &Path) -> io::Result<Vec<OsString>> {
    fs::read_dir(folder)?
        .map(|item| item.map(|item| item.file_name()))
        .collect()
}

/// Gives a copied folder its source's permissions once its children are in it: on macOS and
/// Linux the permission bits, as VS Code copies them; Windows keeps a folder's own.
fn finish_folder(folder: &Path, permissions: Permissions) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(folder, Permissions::from_mode(permissions.mode() & 0o777))
    }
    #[cfg(not(unix))]
    {
        let _ = (folder, permissions);
        Ok(())
    }
}

fn failed(code: ErrorCode) -> Failed {
    FilesError::new(code).into()
}

/// `path` as a reply names it.
fn wire(path: &Path) -> Option<FilePath> {
    path.to_str().and_then(|path| FilePath::new(path).ok())
}

#[cfg(test)]
mod tests;
