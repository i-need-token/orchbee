//! The hang check on Windows: the Worker reads each Host's `dispatch-progress` between requests and
//! ends a Host whose dispatch has stopped turning once the Keeper holds every terminal it runs, so
//! recovery follows as for a crash. It never ends a Host that runs a terminal the Keeper does not
//! hold.

use std::{
    io,
    time::{Duration, Instant},
};

use orchbee_local_runtime::{HoldingState, KeeperProcess, KeeperStatus};
use winsafe::{HPROCESS, HPROCESSLIST, co};

use super::{Cared, END_BOUND, ManagedHost};
use crate::{diagnostic_events, session_keeper::SessionKeeper};

/// How long one `dispatch-progress` read may take.
const PROBE_BOUND: Duration = Duration::from_secs(1);
/// How long a count that has not grown, or a Host that has not answered, counts as hung.
const HANG_BOUND: Duration = Duration::from_secs(10);
/// How far apart the two `dispatch-progress` reads lie that check a Host which left an exchange
/// unanswered: a dispatch loop that still turns does so every 20 milliseconds while idle.
const RECHECK_GAP: Duration = Duration::from_secs(1);

/// Reads the Host's `dispatch-progress` and ends a Host hung for [`HANG_BOUND`] once the Keeper
/// holds every terminal it runs.
pub(super) fn probe(host: &mut Cared<'_>, keeper: &mut Option<SessionKeeper>) {
    if let Some((progress, terminals)) =
        host.connection.query_progress(Instant::now() + PROBE_BOUND)
    {
        host.watch.terminals = Some(terminals);
        if host.watch.progress != Some(progress) {
            host.watch.progress = Some(progress);
            host.watch.turned = Instant::now();
            host.watch.recorded = false;
        }
    }
    let stalled = host.watch.turned.elapsed();
    if stalled < HANG_BOUND {
        return;
    }
    end_hung(host, keeper, stalled);
}

/// Checks a Host once more as the Worker exits because the Host left one of its exchanges
/// unanswered for the whole exchange bound, which counts as hung unless two `dispatch-progress`
/// reads [`RECHECK_GAP`] apart show its dispatch still turning; a hung Host is then ended as the
/// care ends one, and recovery follows the Worker's exit.
pub(in crate::terminal_operations) fn after_unanswered(
    host: &mut Cared<'_>,
    keeper: &mut Option<SessionKeeper>,
) {
    if keeper.is_none() {
        return;
    }
    let first = host.connection.query_progress(Instant::now() + PROBE_BOUND);
    std::thread::sleep(RECHECK_GAP);
    let second = host.connection.query_progress(Instant::now() + PROBE_BOUND);
    if let (Some((before, _)), Some((after, _))) = (first, second)
        && before != after
    {
        return;
    }
    if let Some((_, terminals)) = second.or(first) {
        host.watch.terminals = Some(terminals);
    }
    let stalled = host.watch.turned.elapsed();
    end_hung(host, keeper, stalled);
}

/// Ends a hung Host once the Keeper holds every terminal it runs; a Host left running is recorded
/// once per stall.
fn end_hung(host: &mut Cared<'_>, keeper: &mut Option<SessionKeeper>, stalled: Duration) {
    let held = host
        .watch
        .terminals
        .is_some_and(|terminals| holds_every_terminal(host.managed, keeper, terminals));
    if held && host.managed.end_host(Instant::now() + END_BOUND).is_ok() {
        diagnostic_events::host_hung(true, stalled);
        // The Worker learns of the Host's end from its connection, as after a crash.
        host.watch.turned = Instant::now();
        host.watch.recorded = true;
        return;
    }
    if !host.watch.recorded {
        diagnostic_events::host_hung(false, stalled);
        host.watch.recorded = true;
    }
}

/// Whether `keeper` holds every terminal the Host of `managed` runs, which the Host's last answer
/// counted as `terminals`: at least that many holdings are held for the Host, and every live child
/// process of the Host is the program or OpenConsole of one. A recovered terminal's processes are
/// children of the Host that ended, so the count stands for those.
fn holds_every_terminal(
    managed: &ManagedHost,
    keeper: &mut Option<SessionKeeper>,
    terminals: u32,
) -> bool {
    let Ok(host) = managed.host_identity() else {
        return false;
    };
    let Some(Ok(status)) = keeper.as_mut().map(SessionKeeper::status) else {
        return false;
    };
    let owner = KeeperProcess {
        pid: host.0,
        created: host.1,
    };
    let held = held_processes(&status, owner);
    let count = status
        .holdings
        .iter()
        .filter(|holding| holding.owner == owner && holding.state == HoldingState::Held)
        .count();
    u32::try_from(count).is_ok_and(|count| count >= terminals)
        && matches!(every_child_held(host, &held), Ok(true))
}

/// The program and OpenConsole processes of each holding held for `owner`.
fn held_processes(status: &KeeperStatus, owner: KeeperProcess) -> Vec<u32> {
    status
        .holdings
        .iter()
        .filter(|holding| holding.owner == owner && holding.state == HoldingState::Held)
        .flat_map(|holding| [holding.program.pid, holding.console.pid])
        .collect()
}

/// Whether every live child process of the Host `host` is one of `held`. A held process keeps its
/// ID while the Keeper holds it, so its ID names it; a process whose parent ID is the Host's but
/// that started before the Host is the child of an earlier process with that ID.
fn every_child_held(host: (u32, u64), held: &[u32]) -> io::Result<bool> {
    let mut snapshot = HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None)
        .map_err(io::Error::other)?;
    for process in snapshot.iter_processes() {
        let process = process.map_err(io::Error::other)?;
        let pid = process.th32ProcessID;
        if process.th32ParentProcessID != host.0 || pid == host.0 || held.contains(&pid) {
            continue;
        }
        if started_before(pid, host.1) != Some(true) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether process `pid` started before `created`; `None` when it cannot be opened or queried.
fn started_before(pid: u32, created: u64) -> Option<bool> {
    let process = HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let (creation, _, _, _) = process.GetProcessTimes().ok()?;
    Some(u64::from(creation) < created)
}
