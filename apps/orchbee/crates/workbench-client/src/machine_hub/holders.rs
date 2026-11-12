//! Whether something may hold an incompatible hub, which this client cannot attach to and so cannot
//! ask. No record shows such a client the hub's `presence` attachments, so the check errs toward
//! holding: a live Session holds it, and so does a Session read that cannot finish; among the
//! processes, a holder is a live client that could be attached to that hub, and a process that
//! cannot be classified counts as one. `apps/orchbee/docs/machines.md#incompatible-hubs` owns the
//! rule.

use super::sessions::{self, SessionRead};
use std::{collections::HashMap, path::Path, time::Instant};
use winsafe::{self as w, co};

#[cfg(test)]
mod tests;

/// The most parent links the walk from this process follows, so a cycle of reused ids ends.
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

/// Lists the processes named like the hub's program; `None` when the hub's program or the process
/// list cannot be read.
fn scan(process_id: u32) -> Option<Scan> {
    let hub_image = image(process_id)?;
    let name = Path::new(&hub_image)
        .file_name()
        .and_then(|name| name.to_str())?
        .to_owned();
    let mut snapshot =
        w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).ok()?;
    let mut parents = HashMap::new();
    let mut named = Vec::new();
    for entry in snapshot.iter_processes() {
        let entry = entry.ok()?;
        parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
        if entry.szExeFile().eq_ignore_ascii_case(&name) {
            named.push(entry.th32ProcessID);
        }
    }
    let own = std::process::id();
    let current = w::HPROCESS::GetCurrentProcess();
    Some(Scan {
        hub: process_id,
        hub_image,
        caller: own,
        caller_image: current
            .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
            .ok(),
        ancestors: ancestors(own, started(&current), &parents),
        named: named
            .into_iter()
            .map(|named| (named, image(named)))
            .collect(),
    })
}

/// This process's ancestors by the parent links of `parents`. A parent that started after its
/// child, or whose start cannot be read, is another process that took a dead parent's id, and the
/// walk ends before it.
fn ancestors(own: u32, own_start: Option<u64>, parents: &HashMap<u32, u32>) -> Vec<u32> {
    let mut chain = Vec::new();
    let (mut child, mut child_start) = (own, own_start);
    while chain.len() < ANCESTORS {
        let (Some(&parent), Some(start)) = (parents.get(&child), child_start) else {
            break;
        };
        if parent == own || chain.contains(&parent) {
            break;
        }
        let parent_start =
            w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, parent)
                .ok()
                .and_then(|process| started(&process));
        if parent_start.is_none_or(|parent_start| parent_start > start) {
            break;
        }
        chain.push(parent);
        (child, child_start) = (parent, parent_start);
    }
    chain
}

/// The program path of `process_id`.
fn image(process_id: u32) -> Option<String> {
    w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, process_id)
        .ok()?
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .ok()
}

/// When `process` started, in the system's 100-nanosecond ticks.
fn started(process: &w::HPROCESS) -> Option<u64> {
    let (created, ..) = process.GetProcessTimes().ok()?;
    Some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}
