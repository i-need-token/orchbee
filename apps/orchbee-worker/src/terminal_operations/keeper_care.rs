//! The Worker's care of the Session's Keeper, and on Windows of its Hosts' dispatch
//! (`proposals/platform/host-handover.md`, the hang check and the Keeper's life from H2). It runs
//! between Server requests: it starts the Session's Keeper when none runs and a Host holds
//! terminals there, has each Host the Worker holds attach that Keeper, and replaces a Keeper a Host
//! reports lost, or reports unresponsive and that does not answer the Worker either. On Windows it
//! also updates an adopted Keeper that runs another installation's program and retires the Keeper
//! an update leaves (`update`), and ends a Host whose dispatch has stopped turning once the Keeper
//! holds every terminal it runs, so recovery follows as for a crash (`hang`), which macOS does too;
//! macOS gains the update with H5's M7. A hang check never ends a Host that runs a terminal the
//! Keeper does not hold, and no step here ends the Worker.
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

use super::shutdown::ManagedHost;
#[cfg(windows)]
use crate::session_keeper::RetiringKeeper;
use crate::{
    diagnostic_events,
    host_connection::HostBrokerConnection,
    session_keeper::{Serving, SessionKeeper},
};

mod hang;
#[cfg(windows)]
mod update;

pub(super) use hang::after_unanswered;

/// How often the Worker cares for the Keeper and, on Windows, reads each Host's `dispatch-progress`.
const PERIOD: Duration = Duration::from_secs(2);
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
    /// Whether the Keeper runs another installation's program and is updated.
    #[cfg(windows)]
    update: update::KeeperUpdate,
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

/// The Session's Keeper this Worker answers for, the Keeper it retires, and the Session runtime
/// folder and Terminal Host program a new one starts from.
pub(super) struct Keepers<'a> {
    pub(super) keeper: &'a mut Option<SessionKeeper>,
    #[cfg(windows)]
    pub(super) retiring: &'a mut Option<RetiringKeeper>,
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

#[cfg(any(windows, target_os = "macos"))]
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
            #[cfg(windows)]
            update: update::KeeperUpdate::new(now),
        }
    }

    /// Whether moves wait for the Session's Keeper `keeper`: it waits for its replacement, an
    /// update of it is due or not yet judged, or another Keeper still `retiring`.
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) fn keeper_changing(&self, keeper: Option<&SessionKeeper>, retiring: bool) -> bool {
        #[cfg(windows)]
        let updating = self.update.pending(keeper);
        // A macOS Keeper is never updated in place before H5's M7.
        #[cfg(target_os = "macos")]
        let updating = {
            let _ = keeper;
            false
        };
        self.replacing || retiring || updating
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
        keepers: Keepers<'_>,
        server: &LocalStream,
    ) -> Option<Instant> {
        let began = Instant::now();
        if began < self.due {
            return None;
        }
        self.care(hosts, keepers);
        self.due = Instant::now() + PERIOD;
        super::server_transport::readable_now(server).then_some(began)
    }

    fn care(&mut self, hosts: &mut [Cared<'_>], mut keepers: Keepers<'_>) {
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
        if let Some(current) = keepers.keeper.as_mut()
            && (lost || (unresponsive && current.status().is_err()))
        {
            self.replacing = true;
        }
        if self.replacing || keepers.keeper.is_none() {
            self.start(hosts, &mut keepers);
        } else {
            offer(hosts, keepers.keeper);
            #[cfg(windows)]
            self.update.run(hosts, &mut keepers);
            for host in hosts.iter_mut() {
                hang::probe(host, keepers.keeper);
            }
        }
        #[cfg(windows)]
        update::retire(hosts, &mut keepers);
    }

    /// Starts the Session's Keeper for a Host that holds terminals there, ending first the Keeper it
    /// replaces, and has every Host attach it. A failed start leaves the terminals running unheld.
    fn start(&mut self, hosts: &mut [Cared<'_>], keepers: &mut Keepers<'_>) {
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
        let Some((runtime, executable)) = keepers.program else {
            return;
        };
        self.next_start = now + START_SPACING;
        let replaced = std::mem::take(&mut self.replacing);
        // A replacement takes the lost Keeper's slot, and a first Keeper the record's current one.
        let slot = match keepers.keeper.as_ref() {
            Some(lost) => Ok(lost.slot()),
            None => SessionKeeper::current_slot(runtime),
        };
        if let Some(mut lost) = keepers.keeper.take() {
            lost.end(now + END_BOUND);
        }
        for host in hosts.iter_mut() {
            host.watch.offered = false;
            host.watch.attached = false;
        }
        let host = &mut hosts[first];
        let started = slot.and_then(|slot| {
            let process = host.managed.host_identity()?;
            SessionKeeper::start_attached(runtime, slot, executable, host.connection, process)
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
                *keepers.keeper = Some(started);
                offer(hosts, keepers.keeper);
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
