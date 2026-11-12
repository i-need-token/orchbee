//! Whether something may hold an incompatible hub, which this client cannot attach to and so cannot
//! ask. No record shows such a client the hub's `presence` attachments, so the check errs toward
//! holding: a live Session holds it, and so may any other run of the hub's program.

use super::sessions;
use std::{path::Path, time::Instant};
use winsafe::{self as w, co};

/// Whether a live Session or another run of the hub's program may hold the hub `process_id`.
pub(super) fn held(process_id: u32, deadline: Instant) -> bool {
    let live = sessions::runtime_base(|name| std::env::var_os(name))
        .is_none_or(|base| sessions::any_live(&base, deadline));
    live || other_runs(process_id)
}

/// Whether a process other than the hub and this one runs a program file named as the hub's; true
/// when the hub's program or the process list cannot be read.
fn other_runs(process_id: u32) -> bool {
    let Some(name) = program_name(process_id) else {
        return true;
    };
    let Ok(mut snapshot) = w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None)
    else {
        return true;
    };
    let own = std::process::id();
    let mut entries = snapshot.iter_processes();
    entries.any(|entry| {
        entry.map_or(true, |entry| {
            entry.th32ProcessID != process_id
                && entry.th32ProcessID != own
                && entry.szExeFile().eq_ignore_ascii_case(&name)
        })
    })
}

fn program_name(process_id: u32) -> Option<String> {
    let process =
        w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
    let image = process
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .ok()?;
    Path::new(&image)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
}
