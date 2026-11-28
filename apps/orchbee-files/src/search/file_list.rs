//! One Quick Open file list: every listed file's path relative to its folder, as the UTF-16 code
//! units VS Code's scorer reads, in the order the listing reported them. A list holds at most
//! 500,000 paths or 128 MiB; a listing that reaches either bound stops and marks the list
//! truncated.

use super::{js, paths};
use crate::scope::path::NativePath;

/// The most paths one list holds.
const MAXIMUM_PATHS: usize = 500_000;
/// The most memory one list's paths and their entries take.
const MAXIMUM_BYTES: usize = 128 * 1024 * 1024;

/// A listed folder: its path, and the name Quick Open puts before its paths when it filters a
/// search of several folders (`folderName`).
pub(super) struct Folder {
    pub(super) path: NativePath,
    pub(super) name: Vec<u16>,
}

pub(super) struct FileList {
    folders: Vec<Folder>,
    /// Whether paths are filtered with their folder's name before them: the list has more than
    /// one folder.
    named: bool,
    units: Vec<u16>,
    entries: Vec<Entry>,
    truncated: bool,
}

#[derive(Clone, Copy)]
struct Entry {
    start: u32,
    len: u32,
    /// Where the file's name starts within its path.
    name: u32,
    folder: u16,
}

/// One listed file.
pub(super) struct File<'a> {
    pub(super) folder: &'a Folder,
    /// The path relative to the folder, as rg printed it.
    pub(super) path: &'a [u16],
    /// The file's name: the end of the path.
    pub(super) label: &'a [u16],
    /// The path's folder part, `.` at the top.
    pub(super) description: &'a [u16],
}

impl FileList {
    /// An empty list of `folders`, at most 64.
    pub(super) fn new(folders: &[NativePath]) -> Self {
        let folders: Vec<Folder> = folders
            .iter()
            .map(|path| Folder {
                name: js::units(path.name()),
                path: path.clone(),
            })
            .collect();
        Self {
            named: folders.len() > 1,
            folders,
            units: Vec::new(),
            entries: Vec::new(),
            truncated: false,
        }
    }

    /// Adds the file at `relative` in folder `folder`; false once the list is full, which marks
    /// it truncated.
    pub(super) fn push(&mut self, folder: usize, relative: &[u16]) -> bool {
        if self.truncated {
            return false;
        }
        let bytes = self.bytes() + relative.len() * 2 + size_of::<Entry>();
        let (Ok(start), Ok(len), Ok(folder)) = (
            u32::try_from(self.units.len()),
            u32::try_from(relative.len()),
            u16::try_from(folder),
        ) else {
            self.truncated = true;
            return false;
        };
        if self.entries.len() >= MAXIMUM_PATHS || bytes > MAXIMUM_BYTES {
            self.truncated = true;
            return false;
        }
        let name = u32::try_from(paths::basename_start(relative)).unwrap_or(0);
        self.units.extend_from_slice(relative);
        self.entries.push(Entry {
            start,
            len,
            name,
            folder,
        });
        true
    }

    /// Whether the listing stopped at the list's bound.
    pub(super) const fn truncated(&self) -> bool {
        self.truncated
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    /// What the list takes, as its bound counts it.
    pub(super) fn bytes(&self) -> usize {
        self.units.len() * 2 + self.entries.len() * size_of::<Entry>()
    }

    /// Releases what the list reserved beyond its paths, once it is complete.
    pub(super) fn shrink(&mut self) {
        self.units.shrink_to_fit();
        self.entries.shrink_to_fit();
    }

    /// Whether filtering puts each path's folder name before it.
    pub(super) const fn named(&self) -> bool {
        self.named
    }

    /// File `index`, below [`Self::len`].
    pub(super) fn file(&self, index: usize) -> File<'_> {
        let entry = self.entries[index];
        let start = entry.start as usize;
        let path = &self.units[start..start + entry.len as usize];
        let name = entry.name as usize;
        File {
            folder: &self.folders[usize::from(entry.folder)],
            path,
            label: &path[name..],
            description: paths::dirname(path),
        }
    }
}
