//! The macOS and Linux process reads of the holder check: each process's parent and start path, from
//! one `/bin/ps` listing on macOS and from `/proc` on Linux. A Unix parent link always names a live
//! parent, since an orphan passes to `launchd` or `init`, so the walk needs no start times.
//! Elsewhere no listing is read, and the check holds.

use super::Scan;

/// Lists the processes named like the hub's program; `None` when the hub's program or the process
/// list cannot be read.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) fn scan(process_id: u32) -> Option<Scan> {
    use super::ANCESTORS;
    use std::{collections::HashMap, path::Path};

    let listed = listing()?;
    let hub_image = listed
        .iter()
        .find(|entry| entry.pid == process_id)
        .and_then(Listed::image)?;
    let name = Path::new(&hub_image).file_name()?.to_owned();
    let parents: HashMap<u32, u32> = listed
        .iter()
        .map(|entry| (entry.pid, entry.parent))
        .collect();
    let own = std::process::id();
    let mut ancestors = Vec::new();
    let mut child = own;
    while ancestors.len() < ANCESTORS {
        let Some(&parent) = parents.get(&child) else {
            break;
        };
        if parent <= 1 || parent == own || ancestors.contains(&parent) {
            break;
        }
        ancestors.push(parent);
        child = parent;
    }
    // A process started by a relative path is named by its file name and cannot be classified.
    let named = listed
        .iter()
        .filter(|entry| Path::new(&entry.comm).file_name() == Some(name.as_os_str()))
        .map(|entry| (entry.pid, entry.image()))
        .collect();
    Some(Scan {
        hub: process_id,
        hub_image,
        caller: own,
        caller_image: std::env::current_exe()
            .ok()
            .and_then(|path| path.canonicalize().ok())
            .and_then(|path| path.into_os_string().into_string().ok()),
        ancestors,
        named,
    })
}

/// No process listing is read elsewhere, so whatever may hold the hub counts as holding it.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) const fn scan(_process_id: u32) -> Option<Scan> {
    None
}

/// One process of the listing.
#[cfg(any(target_os = "macos", target_os = "linux"))]
struct Listed {
    pid: u32,
    parent: u32,
    /// The path the process was started by, as `ps` reports it.
    comm: String,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl Listed {
    /// The canonical program path; `None` for a relative start path or one that no longer
    /// resolves.
    fn image(&self) -> Option<String> {
        let path = std::path::Path::new(&self.comm);
        path.is_absolute()
            .then(|| path.canonicalize().ok())
            .flatten()
            .and_then(|path| path.into_os_string().into_string().ok())
    }
}

/// Every process with its parent and start path, from `/bin/ps`; `None` when the listing fails or
/// does not fit.
#[cfg(target_os = "macos")]
fn listing() -> Option<Vec<Listed>> {
    use std::time::{Duration, Instant};
    /// Room for thousands of processes with long program paths.
    const LISTING_LIMIT: usize = 4 * 1024 * 1024;
    let mut command = std::process::Command::new("/bin/ps");
    command
        .args(["-axo", "pid=,ppid=,comm="])
        .env_clear()
        .env("LC_ALL", "C");
    let result = orchbee_local_runtime::run_unix_command(
        command,
        LISTING_LIMIT,
        Some(Instant::now() + Duration::from_secs(5)),
        || Ok(false),
    );
    if result.failure.is_some() || result.stdout_truncated {
        return None;
    }
    Some(
        String::from_utf8_lossy(&result.stdout)
            .lines()
            .filter_map(|line| {
                let (pid, rest) = line.trim_start().split_once(' ')?;
                let (parent, comm) = rest.trim_start().split_once(' ')?;
                Some(Listed {
                    pid: pid.parse().ok()?,
                    parent: parent.parse().ok()?,
                    comm: comm.trim().to_owned(),
                })
            })
            .collect(),
    )
}

/// Every process with its parent, from `/proc`; `None` when `/proc` cannot be read. A process of
/// this user carries its program path, or its short name when that cannot be read or the file was
/// replaced, which classifies nothing; another user's carries none, since it cannot attach to this
/// user's hub. A process that ends during the walk is left out.
#[cfg(target_os = "linux")]
fn listing() -> Option<Vec<Listed>> {
    use std::os::unix::fs::MetadataExt as _;
    let own = std::fs::metadata("/proc/self").ok()?.uid();
    let mut listed = Vec::new();
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse().ok())
        else {
            continue;
        };
        let folder = entry.path();
        // `<pid> (<name>) <state> <parent> …`, where the name may hold spaces and parentheses.
        let Ok(stat) = std::fs::read_to_string(folder.join("stat")) else {
            continue;
        };
        let Some((name, rest)) = stat
            .split_once(" (")
            .and_then(|(_, rest)| rest.rsplit_once(") "))
        else {
            continue;
        };
        let Some(parent) = rest.split(' ').nth(1).and_then(|field| field.parse().ok()) else {
            continue;
        };
        let comm = if entry.metadata().is_ok_and(|metadata| metadata.uid() == own) {
            std::fs::read_link(folder.join("exe"))
                .ok()
                .and_then(|path| path.into_os_string().into_string().ok())
                .filter(|path| !path.ends_with(" (deleted)"))
                .unwrap_or_else(|| name.to_owned())
        } else {
            String::new()
        };
        listed.push(Listed { pid, parent, comm });
    }
    Some(listed)
}
