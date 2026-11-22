//! The Worker's care of the Session's Keeper and of its Host's dispatch on Windows
//! (`proposals/platform/host-handover.md`, the hang check and the Keeper's life from H2). Between
//! Server requests, a Keeper the Host reports lost, or reports unresponsive and that does not answer
//! the Worker either, is replaced; and a Host whose dispatch has stopped turning is ended once the
//! Keeper holds every terminal it runs, so recovery follows as for a crash. A hang check never ends
//! a Host that runs a terminal the Keeper does not hold, and no step here ends the Worker.

use std::{
    io,
    time::{Duration, Instant},
};

use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::{HoldingState, KeeperProcess, KeeperStatus};
use winsafe::{HPROCESS, HPROCESSLIST, co};

use super::shutdown::ManagedHost;
use crate::{diagnostic_events, host_connection::HostBrokerConnection};

/// How often the Worker cares for the Keeper and reads its Host's `dispatch-progress`.
const PERIOD: Duration = Duration::from_secs(2);
/// How long one `dispatch-progress` read may take.
const PROBE_BOUND: Duration = Duration::from_secs(1);
/// How long a count that has not grown, or a Host that has not answered, counts as hung.
const HANG_BOUND: Duration = Duration::from_secs(10);
/// How long ending a hung Host waits for it to exit.
const END_BOUND: Duration = Duration::from_secs(2);

/// The care's schedule and what the Host's answers showed so far.
pub(super) struct KeeperCare {
    due: Instant,
    /// The last `dispatch-progress` the Host answered, and when it last changed.
    progress: Option<u64>,
    turned: Instant,
    /// The terminals the Host's last answer counted.
    terminals: Option<u32>,
    /// Whether the current stall was recorded; a Host left running is recorded once per stall.
    recorded: bool,
}

impl KeeperCare {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            due: now + PERIOD,
            progress: None,
            turned: now,
            terminals: None,
            recorded: false,
        }
    }

    /// When the next care is due.
    pub(super) const fn due(&self) -> Instant {
        self.due
    }

    /// Cares for the Keeper and probes the Host when the care is due; a Worker without a Keeper
    /// only keeps the schedule.
    pub(super) fn run(
        &mut self,
        host: &HostBrokerConnection,
        server: &LocalStream,
        managed: &mut ManagedHost,
    ) {
        let now = Instant::now();
        if now < self.due {
            return;
        }
        self.due = now + PERIOD;
        let events = host.take_keeper_events();
        if events.unheld {
            diagnostic_events::keeper_failed(
                "hold",
                &io::Error::other("the Host runs a terminal its Keeper does not hold"),
            );
        }
        let Some(keeper) = managed.keeper() else {
            return;
        };
        // A Keeper the Host reports unresponsive is replaced only when it does not answer the
        // Worker either within its reply bound.
        if events.lost || (events.unresponsive && keeper.status().is_err()) {
            replace(host, server, managed);
            return;
        }
        self.probe(host, managed);
    }

    /// Reads the Host's `dispatch-progress` and ends a Host hung for [`HANG_BOUND`] once the Keeper
    /// holds every terminal it runs.
    fn probe(&mut self, host: &HostBrokerConnection, managed: &mut ManagedHost) {
        if let Some((progress, terminals)) = host.query_progress(Instant::now() + PROBE_BOUND) {
            self.terminals = Some(terminals);
            if self.progress != Some(progress) {
                self.progress = Some(progress);
                self.turned = Instant::now();
                self.recorded = false;
            }
        }
        let stalled = self.turned.elapsed();
        if stalled < HANG_BOUND {
            return;
        }
        let held = self
            .terminals
            .is_some_and(|terminals| holds_every_terminal(managed, terminals));
        if held && managed.end_host(Instant::now() + END_BOUND).is_ok() {
            diagnostic_events::host_hung(true, stalled);
            // The Worker learns of the Host's end from its connection, as after a crash.
            self.turned = Instant::now();
            self.recorded = true;
            return;
        }
        if !self.recorded {
            diagnostic_events::host_hung(false, stalled);
            self.recorded = true;
        }
    }
}

/// Replaces the Keeper of `managed`; a failed replacement leaves the terminals running unheld and
/// this Worker without a Keeper.
fn replace(host: &HostBrokerConnection, server: &LocalStream, managed: &mut ManagedHost) {
    let Some(keeper) = managed.take_keeper() else {
        return;
    };
    let replaced = managed
        .host_identity()
        .map_err(Into::into)
        .and_then(|process| keeper.replace(host, server, process));
    match replaced {
        Ok(keeper) => {
            diagnostic_events::keeper_replaced();
            managed.keep(Some(keeper));
        }
        Err(error) => diagnostic_events::keeper_failed("replace", error.as_ref()),
    }
}

/// Whether the Keeper of `managed` holds every terminal its Host runs, which the Host's last answer
/// counted as `terminals`: at least that many holdings are held for the Host, and every live child
/// process of the Host is the program or OpenConsole of one. A recovered terminal's processes are
/// children of the Host that ended, so the count stands for those.
fn holds_every_terminal(managed: &mut ManagedHost, terminals: u32) -> bool {
    let Ok(host) = managed.host_identity() else {
        return false;
    };
    let Some(Ok(status)) = managed.keeper().map(|keeper| keeper.status()) else {
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
