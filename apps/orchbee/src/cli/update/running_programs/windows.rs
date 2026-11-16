//! The process scan through a Toolhelp snapshot and the full image name of each process, and the
//! write probe, which opens every program and library below a folder for writing as Setup's probe
//! does (`apps/desktop/installer/windows/orchbee.iss`). Windows refuses that open while a process has
//! the file loaded, and opening changes nothing. A folder rename would tell nothing, since Windows
//! moves a folder while a program runs from it.
use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use winsafe::{self as w, co};

/// `FILE_ATTRIBUTE_REPARSE_POINT`: a link, junction or other reparse point.
const REPARSE_POINT: u32 = 0x400;
/// `FILE_FLAG_OPEN_REPARSE_POINT`, so an open never follows a link.
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
/// `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`, so the probe itself holds nothing back.
const SHARE_ALL: u32 = 7;
/// Folder levels the probe descends; a release path has at most twelve parts.
const PROBE_DEPTH: usize = 16;

/// One process whose program file is inside a scanned folder.
pub(in crate::cli::update) struct StoreProcess {
    pub(in crate::cli::update) pid: u32,
    pub(in crate::cli::update) image: PathBuf,
}

/// The processes whose program file is inside `root`, other than those in `except` and their
/// descendants, and whether the system listed every process. A process this user cannot query is
/// not reported; a listing that fails part way keeps the processes found before.
pub(in crate::cli::update) fn processes_inside(
    root: &Path,
    except: &[u32],
) -> (Vec<StoreProcess>, bool) {
    let Ok(mut snapshot) = w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None)
    else {
        return (Vec::new(), false);
    };
    let mut complete = true;
    let parents: HashMap<u32, u32> = snapshot
        .iter_processes()
        .filter_map(|entry| {
            complete &= entry.is_ok();
            entry.ok()
        })
        .map(|entry| (entry.th32ProcessID, entry.th32ParentProcessID))
        .collect();
    // A bounded walk up the parent links, so a cycle of reused identifiers ends.
    let excepted = |pid: u32| {
        let mut current = pid;
        for _ in 0..64 {
            if except.contains(&current) {
                return true;
            }
            match parents.get(&current) {
                Some(&parent) if parent != 0 && parent != current => current = parent,
                _ => return false,
            }
        }
        false
    };
    let pids: Vec<u32> = parents
        .keys()
        .copied()
        .filter(|pid| *pid != 0 && !excepted(*pid))
        .collect();
    let processes = pids
        .into_iter()
        .filter_map(|pid| {
            let image = image(pid)?;
            inside(&image, root).then_some(StoreProcess { pid, image })
        })
        .collect();
    (processes, complete)
}

/// Whether no process runs from the files below `folder`: the scan finds no process whose program
/// file lies inside it, this one included, and every `.exe`, `.dll` and `.node` below it opens for
/// writing. Links and junctions below it are neither followed nor probed.
///
/// # Errors
/// When the system cannot list every process, or a folder below cannot be read or lies deeper than
/// a release can, so nothing can be told.
pub(in crate::cli::update) fn no_process_runs_from(folder: &Path) -> io::Result<bool> {
    no_process_runs_from_except(folder, &[])
}

/// [`no_process_runs_from`] for the files below `folder` other than the files and folders `skip`
/// names, which both checks leave out: a flat window layout's own files beside the window store's.
///
/// # Errors
/// As [`no_process_runs_from`].
pub(in crate::cli::update) fn no_process_runs_from_except(
    folder: &Path,
    skip: &[PathBuf],
) -> io::Result<bool> {
    let skipped = |path: &Path| {
        skip.iter()
            .any(|skipped| same(path, skipped) || inside(path, skipped))
    };
    let (processes, complete) = processes_inside(folder, &[]);
    if !complete {
        return Err(io::Error::other("the process list is incomplete"));
    }
    if processes.iter().any(|process| !skipped(&process.image)) {
        return Ok(false);
    }
    loaded_below(folder, 0, &skipped).map(|loaded| !loaded)
}

/// Whether a program or library below `folder`, outside what `skipped` holds, cannot be opened for
/// writing.
fn loaded_below(folder: &Path, depth: usize, skipped: &dyn Fn(&Path) -> bool) -> io::Result<bool> {
    if depth > PROBE_DEPTH {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        // The listing's own attributes, so a link is never followed to read them.
        let metadata = entry.metadata()?;
        let path = entry.path();
        if metadata.file_attributes() & REPARSE_POINT != 0 || skipped(&path) {
            continue;
        }
        if metadata.is_dir() {
            if loaded_below(&path, depth + 1, skipped)? {
                return Ok(true);
            }
        } else if program(&path) && !opens_for_writing(&path) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A program or library a process can load: `.exe`, `.dll` or a Node.js addon `.node`.
fn program(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            ["exe", "dll", "node"]
                .iter()
                .any(|program| extension.eq_ignore_ascii_case(program))
        })
}

/// Whether `path` opens for reading and writing; any refusal other than absence counts as loaded,
/// so a file the probe cannot judge keeps its folder.
fn opens_for_writing(path: &Path) -> bool {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(SHARE_ALL)
        .custom_flags(OPEN_REPARSE_POINT)
        .open(path)
    {
        Ok(_) => true,
        Err(error) => error.kind() == io::ErrorKind::NotFound,
    }
}

fn image(pid: u32) -> Option<PathBuf> {
    let process =
        w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    process
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .ok()
        .map(PathBuf::from)
}

/// Whether `path` is strictly inside `root`, comparing names without letter case.
pub(in crate::cli::update) fn inside(path: &Path, root: &Path) -> bool {
    let (path, root) = (fold(path), fold(root));
    path.strip_prefix(root.as_str())
        .is_some_and(|rest| rest.len() > 1 && rest.starts_with('\\'))
}

/// Whether two paths name the same file or folder, comparing names without letter case.
pub(in crate::cli::update) fn same(left: &Path, right: &Path) -> bool {
    fold(left) == fold(right)
}

fn fold(path: &Path) -> String {
    path.to_string_lossy()
        .to_lowercase()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_owned()
}
