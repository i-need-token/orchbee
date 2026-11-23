//! The Worker's care of the Session's Keeper and of its Hosts' dispatch on Windows
//! (`proposals/platform/host-handover.md`, the hang check and the Keeper's life from H2). It runs
//! between Server requests: it starts the Session's Keeper when none runs and a Host recovers
//! terminals, has each Host the Worker holds attach that Keeper, replaces a Keeper a Host reports
//! lost, or reports unresponsive and that does not answer the Worker either, and ends a Host whose
//! dispatch has stopped turning once the Keeper holds every terminal it runs, so recovery follows as
//! for a crash. A hang check never ends a Host that runs a terminal the Keeper does not hold, and no
//! step here ends the Worker.
//!
//! A Server request that arrives while the care runs waits for it, so the care answers when it
//! began, and that request's bound runs from then. A request that meets a hung Host first keeps the
//! care from running until its exchange passes its bound and ends the Worker, so the Worker checks
//! that Host once more before it exits.

use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};

use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::{HoldingState, KeeperProcess, KeeperStatus};
use winsafe::{HPROCESS, HPROCESSLIST, co};

use super::shutdown::ManagedHost;
use crate::{
    diagnostic_events,
    host_connection::HostBrokerConnection,
    session_keeper::{Serving, SessionKeeper},
};

/// How often the Worker cares for the Keeper and reads each Host's `dispatch-progress`.
const PERIOD: Duration = Duration::from_secs(2);
/// How long one `dispatch-progress` read may take.
const PROBE_BOUND: Duration = Duration::from_secs(1);
/// How long a count that has not grown, or a Host that has not answered, counts as hung.
const HANG_BOUND: Duration = Duration::from_secs(10);
/// How far apart the two `dispatch-progress` reads lie that check a Host which left an exchange
/// unanswered: a dispatch loop that still turns does so every 20 milliseconds while idle.
const RECHECK_GAP: Duration = Duration::from_secs(1);
/// How long ending a hung Host or a replaced Keeper waits for it to exit.
const END_BOUND: Duration = Duration::from_secs(2);
/// How long after one Keeper start the next may begin, so a Keeper that keeps ending is not
/// started in a loop.
const START_SPACING: Duration = Duration::from_secs(30);
/// Failed Keeper starts after which this Worker starts none: its terminals then run unheld.
const START_ATTEMPTS: u32 = 3;

/// The care's schedule and the Keeper starts it made.
pub(super) struct KeeperCare {
    due: Instant,
    /// When the next Keeper start may begin, and how many failed.
    next_start: Instant,
    failed_starts: u32,
    /// Whether the Keeper ended or stopped answering and waits for its replacement.
    replacing: bool,
}

/// What the care saw of one Host.
pub(super) struct HostWatch {
    /// Whether the Session's current Keeper was offered to this Host.
    offered: bool,
    /// Whether this Host attached the Session's current Keeper.
    attached: bool,
    /// The last `dispatch-progress` the Host answered, and when it last changed.
    progress: Option<u64>,
    turned: Instant,
    /// The terminals the Host's last answer counted.
    terminals: Option<u32>,
    /// Whether the current stall was recorded; a Host left running is recorded once per stall.
    recorded: bool,
}

/// One Host the care looks after: its connection, its owner, and what the care saw of it.
pub(super) struct Cared<'a> {
    pub(super) connection: &'a HostBrokerConnection,
    pub(super) managed: &'a mut ManagedHost,
    pub(super) watch: &'a mut HostWatch,
}

/// The Session's Keeper this Worker answers for, and the Session runtime folder and Terminal Host
/// program a new one starts from.
pub(super) struct KeeperSlot<'a> {
    pub(super) keeper: &'a mut Option<SessionKeeper>,
    pub(super) program: Option<(&'a Path, &'a Path)>,
}

impl HostWatch {
    /// A Host the Session's current Keeper was already `offered` to, or not yet.
    pub(super) const fn new(now: Instant, offered: bool) -> Self {
        Self {
            offered,
            attached: offered,
            progress: None,
            turned: now,
            terminals: None,
            recorded: false,
        }
    }
}

impl HostWatch {
    /// Whether this Host holds its running terminals in the Session's current Keeper.
    pub(super) const fn attached(&self) -> bool {
        self.attached
    }
}

impl KeeperCare {
    /// A care whose first turn is due at once, so a first Keeper starts right after the answer
    /// that came before it.
    pub(super) fn new(now: Instant) -> Self {
        Self {
            due: now,
            next_start: now,
            failed_starts: 0,
            replacing: false,
        }
    }

    /// Whether the Keeper ended or stopped answering and waits for its replacement.
    pub(super) const fn replacing(&self) -> bool {
        self.replacing
    }

    /// When the next care is due.
    pub(super) const fn due(&self) -> Instant {
        self.due
    }

    /// Cares for the Keeper and probes each Host when the care is due. Returns when the care began
    /// if a Server request became readable while it ran.
    pub(super) fn run(
        &mut self,
        hosts: &mut [Cared<'_>],
        keeper: KeeperSlot<'_>,
        server: &LocalStream,
    ) -> Option<Instant> {
        let began = Instant::now();
        if began < self.due {
            return None;
        }
        self.care(hosts, keeper);
        self.due = Instant::now() + PERIOD;
        super::server_transport::readable_now(server).then_some(began)
    }

    fn care(&mut self, hosts: &mut [Cared<'_>], keeper: KeeperSlot<'_>) {
        let (mut lost, mut unresponsive) = (false, false);
        for host in hosts.iter() {
            let events = host.connection.take_keeper_events();
            if events.unheld {
                diagnostic_events::keeper_failed(
                    "hold",
                    &io::Error::other("the Host runs a terminal its Keeper does not hold"),
                );
            }
            lost |= events.lost;
            unresponsive |= events.unresponsive;
        }
        // A Keeper a Host reports unresponsive is replaced only when it does not answer the Worker
        // either within its reply bound.
        if let Some(current) = keeper.keeper.as_mut()
            && (lost || (unresponsive && current.status().is_err()))
        {
            self.replacing = true;
        }
        if self.replacing || keeper.keeper.is_none() {
            self.start(hosts, keeper);
            return;
        }
        offer(hosts, keeper.keeper);
        for host in hosts.iter_mut() {
            probe(host, keeper.keeper);
        }
    }

    /// Starts the Session's Keeper for a Host that recovers terminals, ending first the Keeper it
    /// replaces, and has every Host attach it. A failed start leaves the terminals running unheld.
    fn start(&mut self, hosts: &mut [Cared<'_>], keeper: KeeperSlot<'_>) {
        let now = Instant::now();
        // Only a Host that can take a kept program back gets a Keeper.
        let Some(first) = hosts
            .iter()
            .position(|host| host.connection.keeper() && host.connection.recovery())
        else {
            return;
        };
        if self.failed_starts >= START_ATTEMPTS || now < self.next_start {
            return;
        }
        let Some((runtime, executable)) = keeper.program else {
            return;
        };
        self.next_start = now + START_SPACING;
        let replaced = std::mem::take(&mut self.replacing);
        if let Some(mut lost) = keeper.keeper.take() {
            lost.end(now + END_BOUND);
        }
        for host in hosts.iter_mut() {
            host.watch.offered = false;
            host.watch.attached = false;
        }
        let host = &mut hosts[first];
        let started = host
            .managed
            .host_identity()
            .map_err(Into::into)
            .and_then(|process| {
                SessionKeeper::start_attached(runtime, executable, host.connection, process)
            });
        match started {
            Ok(started) => {
                if replaced {
                    diagnostic_events::keeper_replaced();
                } else {
                    diagnostic_events::keeper_started();
                }
                host.watch.offered = true;
                host.watch.attached = true;
                *keeper.keeper = Some(started);
                offer(hosts, keeper.keeper);
            }
            Err(error) => {
                self.failed_starts += 1;
                let step = if replaced { "replace" } else { "start" };
                diagnostic_events::keeper_failed(step, error.as_ref());
            }
        }
    }
}

/// Has each Host the current Keeper was not yet offered to attach it, once: a Host this Worker
/// started after the Keeper, or one whose attachment failed and stays unheld.
fn offer(hosts: &mut [Cared<'_>], keeper: &mut Option<SessionKeeper>) {
    let Some(keeper) = keeper.as_mut() else {
        return;
    };
    for host in hosts
        .iter_mut()
        .filter(|host| !host.watch.offered && host.connection.keeper())
    {
        host.watch.offered = true;
        let attached = host
            .managed
            .host_identity()
            .map_err(Into::into)
            .and_then(|process| keeper.attach(host.connection, Serving::BetweenRequests, process));
        match attached {
            Ok(()) => host.watch.attached = true,
            Err(error) => diagnostic_events::keeper_failed("attach", error.as_ref()),
        }
    }
}

/// Reads the Host's `dispatch-progress` and ends a Host hung for [`HANG_BOUND`] once the Keeper
/// holds every terminal it runs.
fn probe(host: &mut Cared<'_>, keeper: &mut Option<SessionKeeper>) {
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
pub(super) fn after_unanswered(host: &mut Cared<'_>, keeper: &mut Option<SessionKeeper>) {
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
