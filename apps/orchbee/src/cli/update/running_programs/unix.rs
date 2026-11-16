//! The process scan on macOS and Linux, which uninstall waits on. Linux reads each process's program
//! file from `/proc/<pid>/exe`, which names the real file. macOS asks `/bin/ps` for each process's
//! start path, its first argument: a program started by absolute path, as every Session process and
//! the npm carrier's Client are, is found through any link in that path, but one a shell started by
//! name is not. Nothing here needs unsafe code. No process can be told to run from a folder's files
//! beyond that, so the write probe, which store cleanup needs, stays unsupported.
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

/// Parent links a walk follows at most, so a cycle of reused identifiers ends.
const WALK_LIMIT: usize = 64;

/// One process whose program file is inside a scanned folder, or a parent in a chain of parents.
pub(in crate::cli::update) struct StoreProcess {
    pub(in crate::cli::update) pid: u32,
    pub(in crate::cli::update) image: PathBuf,
}

/// One listed process: its parent and, when it can be read, its program path.
struct Listed {
    pid: u32,
    parent: u32,
    image: Option<PathBuf>,
}

/// The processes whose program file is inside `root`, other than those in `except` and their
/// descendants, and whether the system listed every process. A process this user cannot query is
/// not reported; a listing that fails part way keeps the processes found before.
pub(in crate::cli::update) fn processes_inside(
    root: &Path,
    except: &[u32],
) -> (Vec<StoreProcess>, bool) {
    let (listed, complete) = listing();
    let parents: HashMap<u32, u32> = listed.iter().map(|item| (item.pid, item.parent)).collect();
    let excepted = |pid: u32| {
        let mut current = pid;
        for _ in 0..WALK_LIMIT {
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
    // The root as spelled and as resolved, since a program path names the real file.
    let roots: Vec<PathBuf> = [Some(root.to_owned()), fs::canonicalize(root).ok()]
        .into_iter()
        .flatten()
        .collect();
    let processes = listed
        .into_iter()
        .filter(|item| item.pid != 0 && !excepted(item.pid))
        .filter_map(|item| {
            let image = item.image?;
            // A start path may pass through a link, such as the command link in `~/.local/bin`.
            let image = if image.is_absolute() && cfg!(target_os = "macos") {
                fs::canonicalize(&image).unwrap_or(image)
            } else {
                image
            };
            roots
                .iter()
                .any(|root| inside(&image, root))
                .then_some(StoreProcess {
                    pid: item.pid,
                    image,
                })
        })
        .collect();
    (processes, complete)
}

/// This process's chain of parents, nearest first, from one listing, in which a process whose
/// parent ended already names its new parent. The walk ends at the first parent whose program path
/// cannot be read.
pub(in crate::cli::update) fn ancestors() -> Vec<StoreProcess> {
    let (listed, _) = listing();
    let by_pid: HashMap<u32, &Listed> = listed.iter().map(|item| (item.pid, item)).collect();
    let mut chain: Vec<StoreProcess> = Vec::new();
    let mut current = std::process::id();
    for _ in 0..WALK_LIMIT {
        let Some(pid) = by_pid.get(&current).map(|item| item.parent) else {
            break;
        };
        if pid == 0 || pid == current || chain.iter().any(|link| link.pid == pid) {
            break;
        }
        let Some(image) = by_pid.get(&pid).and_then(|item| item.image.clone()) else {
            break;
        };
        chain.push(StoreProcess { pid, image });
        current = pid;
    }
    chain
}

/// Always `Unsupported`, so store cleanup removes nothing as unused here.
pub(in crate::cli::update) fn no_process_runs_from(_folder: &Path) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}

/// Whether a complete listing shows no process whose start path lies inside `folder`; a listing
/// that fails part way tells nothing. Only a folder whose every program Orchbee itself starts by
/// absolute path can rely on this, since a program a shell started by name is not found: a macOS
/// window version, whose bundle no window runs from because the launcher copies it into the
/// Applications folder.
#[cfg(target_os = "macos")]
pub(in crate::cli::update) fn no_process_started_from(folder: &Path) -> io::Result<bool> {
    match processes_inside(folder, &[]) {
        (processes, true) => Ok(processes.is_empty()),
        (_, false) => Err(io::ErrorKind::Other.into()),
    }
}

/// Whether `path` is strictly inside `root`, by whole components.
pub(in crate::cli::update) fn inside(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

/// Whether two paths name the same file or folder, by whole components.
pub(in crate::cli::update) fn same(left: &Path, right: &Path) -> bool {
    left == right
}

/// Every process with its parent and program file, from `/proc`.
#[cfg(not(target_os = "macos"))]
fn listing() -> (Vec<Listed>, bool) {
    let Ok(entries) = fs::read_dir("/proc") else {
        return (Vec::new(), false);
    };
    let mut complete = true;
    let mut listed = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            complete = false;
            continue;
        };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        // A process that ended since the listing has no status left.
        let Some(parent) = fs::read_to_string(entry.path().join("stat"))
            .ok()
            .as_deref()
            .and_then(parent_in_stat)
        else {
            continue;
        };
        let image =
            fs::read_link(entry.path().join("exe"))
                .ok()
                .map(|image| match image.to_str() {
                    // The program file was removed while the process runs from it.
                    Some(text) if text.ends_with(" (deleted)") => {
                        PathBuf::from(&text[..text.len() - " (deleted)".len()])
                    }
                    _ => image,
                });
        listed.push(Listed { pid, parent, image });
    }
    (listed, complete)
}

/// The parent process identifier in a `/proc/<pid>/stat` line: the second field after the command
/// name, which is parenthesized and may itself hold parentheses and spaces.
#[cfg(not(target_os = "macos"))]
fn parent_in_stat(stat: &str) -> Option<u32> {
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// Every process with its parent and start path, from `/bin/ps`. A listing that fails or does not
/// fit is incomplete and keeps the lines read.
#[cfg(target_os = "macos")]
fn listing() -> (Vec<Listed>, bool) {
    use std::time::{Duration, Instant};
    /// Room for thousands of processes with long program paths.
    const LISTING_LIMIT: usize = 4 * 1024 * 1024;
    let mut command = std::process::Command::new("/bin/ps");
    command
        .args(["-axo", "pid=,ppid=,comm="])
        .env_clear()
        .env("LC_ALL", "C");
    // Retention runs this inside an install or update, whose thread blocks the interrupts it
    // captures, so `ps` starts with an empty signal mask.
    let result = orchbee_local_runtime::run_unmasked_unix_command(
        command,
        orchbee_local_runtime::SpawnEnvironment::Cleared,
        LISTING_LIMIT,
        Some(Instant::now() + Duration::from_secs(5)),
        || Ok(false),
    );
    let complete = result.failure.is_none() && !result.stdout_truncated;
    let listed = String::from_utf8_lossy(&result.stdout)
        .lines()
        .filter_map(|line| {
            let (pid, rest) = line.trim_start().split_once(' ')?;
            let (parent, comm) = rest.trim_start().split_once(' ')?;
            let comm = comm.trim();
            Some(Listed {
                pid: pid.parse().ok()?,
                parent: parent.parse().ok()?,
                image: (!comm.is_empty()).then(|| PathBuf::from(comm)),
            })
        })
        .collect();
    (listed, complete)
}
