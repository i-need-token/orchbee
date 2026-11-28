//! A Quick Open file list, made as VS Code ef5ca45's `FileWalker` makes one with rg
//! (`fileSearch.ts` and `ripgrepFileSearch.ts`) for its search cache: `rg --files` in each folder
//! with the request's globs and the request's folders nested in it excluded; with exclude
//! exceptions, a second run that lists only what they re-include; a folder listed again without
//! following links when they form too many loops; and each path checked against the request's
//! includes. Globs match case-sensitively, as Quick Open's query asks. Folders run one after
//! another, where VS Code runs them at once.

use std::{
    collections::HashSet,
    sync::atomic::{AtomicBool, Ordering},
};

use orchbee_files_protocol::{ErrorCode, FilesError};

use super::{
    args::{self, FolderOptions},
    file_list::FileList,
    glob::{self, Excludes, Globs},
    js, paths,
    rg::{self, Ended, Feed, Launch, Stderr},
    slots::Slots,
};
use crate::scope::path::NativePath;

/// A list's folders and how they are listed, its globs as the user wrote them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Request {
    pub(super) folders: Vec<NativePath>,
    pub(super) includes: Vec<String>,
    /// Every exclude key, exceptions included.
    pub(super) excludes: Vec<String>,
    pub(super) use_ignore_files: bool,
    pub(super) use_parent_ignore_files: bool,
    pub(super) follow_symlinks: bool,
}

/// Lists `request`'s files until done, the list is full or `cancelled` is set.
///
/// # Errors
///
/// `cancelled`; the failure to read a folder; `invalid-request` for a glob rg rejects, with rg's
/// message as `detail`; `unavailable{search-binary}`; and `failed`.
pub(super) fn list(
    request: &Request,
    launch: &dyn Launch,
    slots: &Slots,
    cancelled: &AtomicBool,
) -> Result<FileList, FilesError> {
    let mut walk = Walk {
        request,
        launch,
        slots,
        cancelled,
        list: FileList::new(&request.folders),
        includes: (!request.includes.is_empty())
            .then(|| Globs::new(request.includes.iter().map(String::as_str), false)),
    };
    for index in 0..request.folders.len() {
        if walk.stopped() {
            break;
        }
        rg::searchable(request.folders[index].as_path())?;
        walk.folder(index)?;
    }
    if cancelled.load(Ordering::Acquire) {
        return Err(FilesError::new(ErrorCode::Cancelled));
    }
    let mut list = walk.list;
    list.shrink();
    Ok(list)
}

struct Walk<'a> {
    request: &'a Request,
    launch: &'a dyn Launch,
    slots: &'a Slots,
    cancelled: &'a AtomicBool,
    list: FileList,
    /// The request's includes, which every listed path passes (`matchFile`).
    includes: Option<Globs>,
}

/// One folder's globs, as `FileWalker`'s constructor prepares them.
struct Folder {
    index: usize,
    path: NativePath,
    /// The ordinary excludes, the folders of the request nested in this one among them.
    plain: Vec<String>,
    /// Every exclude, for which paths an exception covers.
    excludes: Excludes,
    /// The folders of the request nested in this one, relative to it.
    nested: Vec<Vec<u16>>,
}

impl Walk<'_> {
    /// Whether nothing more is listed: the list is full or the listing was cancelled.
    fn stopped(&self) -> bool {
        self.list.truncated() || self.cancelled.load(Ordering::Acquire)
    }

    /// `folderTraversal` of folder `index`.
    fn folder(&mut self, index: usize) -> Result<(), FilesError> {
        let path = self.request.folders[index].clone();
        let units = js::units(path.as_str());
        let start = if units.last() == Some(&js::SEPARATOR) {
            units.len()
        } else {
            units.len() + 1
        };
        let nested: Vec<Vec<u16>> = self
            .request
            .folders
            .iter()
            .map(|other| js::units(other.as_str()))
            .filter(|other| *other != units)
            .filter(|other| paths::is_equal_or_parent(other, &units, false, js::SEPARATOR))
            .map(|other| other[start.min(other.len())..].to_vec())
            .collect();
        let mut keys = self.request.excludes.clone();
        keys.extend(nested.iter().map(|folder| js::text(folder)));
        let folder = Folder {
            index,
            plain: keys
                .iter()
                .filter(|key| !glob::is_exception(key))
                .cloned()
                .collect(),
            excludes: Excludes::new(&keys, false),
            nested,
            path,
        };
        let exceptions = folder.excludes.exceptions().to_vec();
        if exceptions.is_empty() {
            return self.traverse(&folder, Traversal::default());
        }
        let mut seen = HashSet::new();
        self.traverse(
            &folder,
            Traversal {
                seen: Some(&mut seen),
                ..Traversal::default()
            },
        )?;
        if self.stopped() {
            return Ok(());
        }
        self.traverse(
            &folder,
            Traversal {
                seen: Some(&mut seen),
                exceptions: Some(&exceptions),
                skip: None,
            },
        )
    }

    /// `cmdTraversal`: one rg run in `folder`, following links as the request asks, and the run
    /// again without following them when they form too many loops.
    fn traverse(
        &mut self,
        folder: &Folder,
        traversal: Traversal<'_, '_>,
    ) -> Result<(), FilesError> {
        self.run(folder, traversal, self.request.follow_symlinks)
    }

    fn run(
        &mut self,
        folder: &Folder,
        mut traversal: Traversal<'_, '_>,
        follow: bool,
    ) -> Result<(), FilesError> {
        let request = self.request;
        let options = FolderOptions {
            includes: &request.includes,
            excludes: &folder.plain,
            use_ignore_files: request.use_ignore_files,
            use_parent_ignore_files: request.use_parent_ignore_files,
            follow_symlinks: follow,
            ignore_glob_case: false,
            below_top: args::below_top(folder.path.as_path()),
        };
        let arguments = args::files(&options, traversal.exceptions);
        let (launch, slots, cancelled) = (self.launch, self.slots, self.cancelled);
        let Some(_slot) = slots.acquire(|| cancelled.load(Ordering::Acquire)) else {
            return Ok(());
        };
        let mut listed = Listed {
            walk: self,
            folder,
            traversal: &mut traversal,
            follow,
            reported: Vec::new(),
            pending: Vec::new(),
            stderr: Stderr::default(),
            got_data: false,
            retry: false,
        };
        let ended = launch.run(&arguments, folder.path.as_path(), &mut listed);
        match ended {
            Ended::Unavailable => return Err(rg::unavailable()),
            Ended::Failed => return Err(FilesError::new(ErrorCode::Failed)),
            Ended::Exited | Ended::Stopped => {}
        }
        // As VS Code's run closes: a retry reads nothing more; otherwise rg's standard error fails
        // the listing only when rg listed nothing and VS Code shows that error.
        if !listed.retry && ended == Ended::Exited {
            if !listed.got_data
                && let Some(error) = rg::listing_error(listed.stderr.kept())
            {
                return Err(error);
            }
            listed.last();
        }
        let Listed {
            reported, retry, ..
        } = listed;
        if !retry || self.cancelled.load(Ordering::Acquire) {
            return Ok(());
        }
        // VS Code skips the paths it already reported, while what the folder's runs saw carries
        // over.
        let skip: HashSet<Vec<u16>> = reported.into_iter().collect();
        let again = Traversal {
            seen: traversal.seen,
            exceptions: traversal.exceptions,
            skip: Some(&skip),
        };
        self.run(folder, again, false)
    }
}

/// What a run shares with the runs of its folder (`ITraversalOptions`).
#[derive(Default)]
struct Traversal<'s, 'e> {
    /// The paths an exception covers that a run of the folder listed.
    seen: Option<&'s mut HashSet<Vec<u16>>>,
    /// The run lists what these exceptions re-include.
    exceptions: Option<&'e [String]>,
    /// The paths the run before a retry reported.
    skip: Option<&'s HashSet<Vec<u16>>>,
}

/// The consumer of one `rg --files` run.
struct Listed<'l, 'a, 's, 'e> {
    walk: &'l mut Walk<'a>,
    folder: &'l Folder,
    traversal: &'l mut Traversal<'s, 'e>,
    follow: bool,
    /// The paths listed while following links, which a retry skips.
    reported: Vec<Vec<u16>>,
    /// The output line not yet ended.
    pending: Vec<u8>,
    stderr: Stderr,
    got_data: bool,
    retry: bool,
}

impl Feed for Listed<'_, '_, '_, '_> {
    fn more(&mut self) -> bool {
        !self.retry && !self.walk.stopped()
    }

    fn stdout(&mut self, bytes: &[u8]) -> bool {
        if !bytes.is_empty() {
            self.got_data = true;
        }
        if !self.more() {
            return false;
        }
        let mut rest = bytes;
        while let Some(end) = rest.iter().position(|byte| *byte == b'\n') {
            self.pending.extend_from_slice(&rest[..end]);
            let line = std::mem::take(&mut self.pending);
            // A path that is not UTF-8 cannot be named on the wire.
            if let Ok(line) = std::str::from_utf8(&line) {
                self.path(&js::units(line));
            }
            self.pending = line;
            self.pending.clear();
            rest = &rest[end + 1..];
            if !self.more() {
                return false;
            }
        }
        self.pending.extend_from_slice(rest);
        true
    }

    fn stderr(&mut self, bytes: &[u8]) {
        let counting = self.follow && !self.retry && !self.walk.stopped();
        if self.stderr.push(bytes, counting) {
            self.retry = true;
        }
    }
}

impl Listed<'_, '_, '_, '_> {
    /// The line rg left unended when it exited, trimmed as VS Code trims its last line; rg ends
    /// every line, so there is none.
    fn last(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        if let Ok(last) = std::str::from_utf8(&pending)
            && self.more()
        {
            self.path(&js::units(js::trim(last)));
        }
    }

    /// One path rg listed, relative to the folder, through the run's filters to the list.
    fn path(&mut self, relative: &[u16]) {
        if relative.is_empty() {
            return;
        }
        if self
            .traversal
            .skip
            .is_some_and(|skip| skip.contains(relative))
        {
            return;
        }
        let excepted = self.traversal.exceptions.is_some();
        if excepted {
            let seen = self
                .traversal
                .seen
                .as_deref()
                .is_some_and(|seen| seen.contains(relative));
            if seen || !self.reincluded(relative) {
                return;
            }
        }
        if let Some(seen) = self.traversal.seen.as_deref_mut() {
            // Exception runs only list what an exception covers, so only that can be listed twice.
            if excepted || self.folder.excludes.is_excepted(relative, None) {
                seen.insert(relative.to_vec());
            }
        }
        let basename = &relative[paths::basename_start(relative)..];
        if self
            .walk
            .includes
            .as_ref()
            .is_some_and(|includes| !includes.matches(relative, Some(basename)))
        {
            return;
        }
        if self.walk.list.push(self.folder.index, relative) && self.follow {
            self.reported.push(relative.to_vec());
        }
    }

    /// `createExceptionRunFilter`: an exception covers the path, the includes admit it, and no
    /// folder of the request nested in this one holds it, since that folder is listed on its own.
    fn reincluded(&self, relative: &[u16]) -> bool {
        let basename = &relative[paths::basename_start(relative)..];
        self.folder.excludes.is_excepted(relative, None)
            && self
                .walk
                .includes
                .as_ref()
                .is_none_or(|includes| includes.matches(relative, Some(basename)))
            && !self
                .folder
                .nested
                .iter()
                .any(|nested| paths::is_equal_or_parent(relative, nested, false, js::SEPARATOR))
    }
}
