//! Whether something may hold an incompatible hub, which this client cannot attach to and so cannot
//! ask. No record shows such a client the hub's `presence` attachments, so the check errs toward
//! holding: a live Session holds it, and so does a Session read that cannot finish; among the
//! processes, a holder is a live client that could be attached to that hub, and a process that
//! cannot be classified counts as one. `apps/orchbee/docs/machines.md#incompatible-hubs` owns the
//! rule.

use super::sessions::{self, SessionRead};
use std::{path::Path, time::Instant};

#[cfg(unix)]
#[path = "holders/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "holders/windows.rs"]
mod platform;
/// The probes are release-store paths of Windows.
#[cfg(all(test, windows))]
mod tests;

use platform::scan;

/// The most parent links the walk from this process follows, so a cycle of reused ids ends. Linux
/// has no process scan.
#[cfg(any(windows, target_os = "macos"))]
const ANCESTORS: usize = 64;

/// Whether a live Session or a client process may hold the hub `process_id`.
pub(super) fn held(process_id: u32, deadline: Instant) -> bool {
    let sessions = sessions::runtime_base(|name| std::env::var_os(name))
        .map_or(SessionRead::Unknown, |base| {
            sessions::read_sessions(&base, deadline)
        });
    decide(sessions, scan(process_id).as_ref())
}

/// The processes the check measures, against the hub and this process.
#[derive(Debug)]
pub(super) struct Scan {
    /// The hub's process id.
    pub(super) hub: u32,
    /// The hub's program path.
    pub(super) hub_image: String,
    /// This process's id.
    pub(super) caller: u32,
    /// This process's program path; `None` when it cannot be read.
    pub(super) caller_image: Option<String>,
    /// This process's parent, its parent, and so on, each started before its child.
    pub(super) ancestors: Vec<u32>,
    /// Every process whose program file name is the hub's, with its program path, `None` when it
    /// cannot be read.
    pub(super) named: Vec<(u32, Option<String>)>,
}

/// The decision over what the reads found: held unless the Session read found no live Session and
/// no process named like the hub's program may be a client attached to it. `None` stands for
/// processes that could not be listed.
pub(super) fn decide(sessions: SessionRead, scan: Option<&Scan>) -> bool {
    if sessions != SessionRead::NoneLive {
        return true;
    }
    let Some(scan) = scan else {
        return true;
    };
    scan.named
        .iter()
        .any(|(process_id, image)| may_hold(scan, *process_id, image.as_deref()))
}

/// Whether one process named like the hub's program may be a client attached to it. The hub and
/// this process are not; nor is this process's ancestor, which started it, nor a launcher, which
/// only waits for the client it started, nor a run of this process's own program, which is this
/// version and so cannot attach to a hub this version finds incompatible. Any other run, and one
/// whose program cannot be read, may be.
fn may_hold(scan: &Scan, process_id: u32, image: Option<&str>) -> bool {
    if process_id == scan.hub || process_id == scan.caller || scan.ancestors.contains(&process_id) {
        return false;
    }
    let Some(image) = image else {
        return true;
    };
    let caller = scan.caller_image.as_deref();
    let same = |other: &str| image.eq_ignore_ascii_case(other);
    let own_program = caller.is_some_and(same);
    let launcher = [Some(scan.hub_image.as_str()), caller]
        .into_iter()
        .flatten()
        .filter_map(launcher)
        .any(|launcher| same(&launcher));
    !(own_program || launcher)
}

/// The launcher of the release store a Client program `<store>\versions\<version>\<name>` belongs
/// to, `<store>\bin\<name>`, as the store layout in `apps/orchbee/docs/windows-cli-configuration.md`
/// places it; `None` for a program outside such a store.
fn launcher(image: &str) -> Option<String> {
    let image = Path::new(image);
    let name = image.file_name()?;
    let versions = image.parent()?.parent()?;
    if !versions
        .file_name()?
        .to_str()?
        .eq_ignore_ascii_case("versions")
    {
        return None;
    }
    versions
        .parent()?
        .join("bin")
        .join(name)
        .into_os_string()
        .into_string()
        .ok()
}
