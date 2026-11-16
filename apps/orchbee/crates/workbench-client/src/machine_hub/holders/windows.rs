//! The Windows process reads of the holder check: a Toolhelp snapshot, each process's program path
//! and its start time.

use super::{ANCESTORS, Scan};
use std::{collections::HashMap, path::Path};
use winsafe::{self as w, co};

/// Lists the processes named like the hub's program; `None` when the hub's program or the process
/// list cannot be read.
pub(super) fn scan(process_id: u32) -> Option<Scan> {
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
