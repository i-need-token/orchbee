//! The Terminal Hosts one Worker holds (`proposals/platform/versioning-and-upgrade.md`, Host
//! lifetime): its own Host, from its own cohort, which takes every new terminal and program, and on
//! Windows and macOS the older Hosts a replacement adopted from another installation, which keep
//! serving their terminals. It routes a request to the Host that serves what the request names, starts the
//! Worker's own Host when a launch needs one, and, while the Worker holds its own Host, drains an
//! older Host that runs neither a terminal nor a program, then frees its slot once it ends. It
//! serves no request itself.

use std::{cell::RefCell, error::Error, path::Path, rc::Rc, time::Instant};

use orchbee_capability_node_protocol::{HostEpoch, OperationId};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_runtime::{Cohort, HostSlot};

use super::{
    host_start::{HOST_START_ATTEMPTS, HostStart, start_host},
    shutdown::{ManagedHost, StopHosts},
};
use crate::{
    agent_drain::AgentDrain, diagnostic_events::Site, host_connection::HostBrokerConnection,
    host_programs::HeldPrograms,
};

/// How often an older Host is checked for emptiness between requests.
#[cfg(any(windows, target_os = "macos"))]
const DRAIN_CHECK_PERIOD: std::time::Duration = std::time::Duration::from_secs(2);
/// How long one emptiness check or `drain` request may take.
#[cfg(any(windows, target_os = "macos"))]
const DRAIN_BOUND: std::time::Duration = std::time::Duration::from_millis(500);
/// How long a drained Host that closed its connection may take to exit.
#[cfg(any(windows, target_os = "macos"))]
const END_BOUND: std::time::Duration = std::time::Duration::from_secs(2);

/// One Host the Worker holds.
struct HeldHost {
    slot: HostSlot,
    connection: HostBrokerConnection,
    /// Its owner in a managed Session; an unmanaged Worker owns no Host.
    managed: Option<ManagedHost>,
    /// It runs this Worker's own cohort Terminal Host, so new terminals and programs go there.
    own: bool,
    #[cfg(any(windows, target_os = "macos"))]
    watch: super::keeper_care::HostWatch,
    #[cfg(any(windows, target_os = "macos"))]
    drain: Drain,
}

/// Where draining an older Host stands.
#[cfg(any(windows, target_os = "macos"))]
#[derive(Clone, Copy)]
enum Drain {
    /// It serves, and is next checked for emptiness at this instant.
    Serving(Instant),
    /// It was told to drain, so it ends once it holds no terminal, and that end is normal.
    Drained,
}

impl HeldHost {
    #[cfg(any(windows, target_os = "macos"))]
    const fn drained(&self) -> bool {
        matches!(self.drain, Drain::Drained)
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    const fn drained(&self) -> bool {
        false
    }

    fn serves(&self, epoch: HostEpoch) -> bool {
        !self.drained() && epoch.as_bytes() == *self.connection.host_epoch().as_bytes()
    }
}

/// How a managed Worker starts its own Host: its cohort, the Session runtime folder, and the Agent
/// Drain queue every Host connection appends to.
pub(super) struct Starter<'a> {
    pub(super) cohort: &'a Cohort,
    pub(super) runtime: &'a Path,
    pub(super) drain: Rc<RefCell<AgentDrain>>,
}

/// Every Host the Worker holds, never none: the own Host is never drained, and an older Host only
/// while an own one is held.
pub(super) struct HostSet<'a> {
    hosts: Vec<HeldHost>,
    starter: Option<Starter<'a>>,
    /// The cohort's Terminal Host program, which an own Host runs and a Keeper starts from.
    #[cfg(any(windows, target_os = "macos"))]
    host_program: Option<std::path::PathBuf>,
    /// Own Host starts that failed in a row, and whether another may begin.
    failed_starts: u32,
    may_start: bool,
    /// The Session's Keeper this Worker answers for, which a stop ends once every Host is gone.
    #[cfg(any(windows, target_os = "macos"))]
    keeper: Option<crate::session_keeper::SessionKeeper>,
    /// A Keeper an update replaced or rolled back, or an adoption found beside the current one,
    /// until every Host detached it and it ended.
    #[cfg(windows)]
    retiring: Option<crate::session_keeper::RetiringKeeper>,
    /// The care of the Session's Keeper between requests.
    #[cfg(any(windows, target_os = "macos"))]
    care: super::keeper_care::KeeperCare,
}

impl<'a> HostSet<'a> {
    /// The one Host an unmanaged Worker was given, which it neither starts nor stops.
    pub(super) fn unmanaged(connection: HostBrokerConnection) -> Self {
        let mut hosts = Self::new(None);
        hosts.push(HostSlot::FIRST, connection, None, true, false);
        hosts
    }

    /// A managed Worker's Hosts, to which [`Self::add`] adds each it started or adopted; it starts
    /// its own Host through `starter` when it holds none.
    pub(super) fn managed(starter: Starter<'a>) -> Self {
        Self::new(Some(starter))
    }

    fn new(starter: Option<Starter<'a>>) -> Self {
        Self {
            #[cfg(any(windows, target_os = "macos"))]
            host_program: starter.as_ref().map(|starter| {
                starter
                    .cohort
                    .executable(orchbee_local_runtime::CohortArtifact::TerminalHost)
            }),
            starter,
            hosts: Vec::new(),
            failed_starts: 0,
            may_start: true,
            #[cfg(any(windows, target_os = "macos"))]
            keeper: None,
            #[cfg(windows)]
            retiring: None,
            // The first care is due at once, so a first Keeper starts right after the answer that
            // came before it.
            #[cfg(any(windows, target_os = "macos"))]
            care: super::keeper_care::KeeperCare::new(Instant::now()),
        }
    }

    /// Adds the Host in `slot` this Worker started or adopted, whose owner is `managed`. On Windows
    /// and macOS an adopted Host is the Worker's own only when its image is the cohort's Terminal
    /// Host, and on Windows `offered` says whether the Session's Keeper this Worker answers for was
    /// attached to it.
    pub(super) fn add(
        &mut self,
        slot: HostSlot,
        connection: HostBrokerConnection,
        managed: ManagedHost,
        offered: bool,
    ) {
        #[cfg(any(windows, target_os = "macos"))]
        let own = self
            .host_program
            .as_deref()
            .is_none_or(|program| managed.runs(program));
        #[cfg(not(any(windows, target_os = "macos")))]
        let own = true;
        self.push(slot, connection, Some(managed), own, offered);
    }

    #[cfg_attr(
        not(any(windows, target_os = "macos")),
        expect(unused_mut, unused_variables)
    )]
    fn push(
        &mut self,
        slot: HostSlot,
        mut connection: HostBrokerConnection,
        managed: Option<ManagedHost>,
        own: bool,
        offered: bool,
    ) {
        #[cfg(any(windows, target_os = "macos"))]
        if let Some(first) = self.hosts.first() {
            connection.share_titles(&first.connection);
        }
        self.hosts.push(HeldHost {
            slot,
            connection,
            managed,
            own,
            #[cfg(any(windows, target_os = "macos"))]
            watch: super::keeper_care::HostWatch::new(Instant::now(), offered),
            #[cfg(any(windows, target_os = "macos"))]
            drain: Drain::Serving(Instant::now()),
        });
    }

    /// Keeps `keeper`, the Session's Keeper this Worker started or adopted, and on Windows retires a
    /// live Keeper its adoption found in the other slot.
    #[cfg(any(windows, target_os = "macos"))]
    #[cfg_attr(target_os = "macos", expect(unused_mut))]
    pub(super) fn keep(&mut self, mut keeper: Option<crate::session_keeper::SessionKeeper>) {
        #[cfg(windows)]
        if let Some(other) = keeper
            .as_mut()
            .and_then(crate::session_keeper::SessionKeeper::take_other)
        {
            self.retiring = Some(other);
        }
        if keeper.is_some() {
            self.keeper = keeper;
        }
    }

    /// Whether moves wait for the Session's Keeper: its replacement or update is due or under way,
    /// or a Keeper still retires.
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) fn keeper_changing(&self) -> bool {
        #[cfg(windows)]
        let retiring = self.retiring.is_some();
        // A macOS Keeper retires only once updates arrive with H5's M7.
        #[cfg(target_os = "macos")]
        let retiring = false;
        self.care.keeper_changing(self.keeper.as_ref(), retiring)
    }

    /// Whether this Worker owns its Hosts in a managed Session.
    pub(super) const fn is_managed(&self) -> bool {
        self.starter.is_some()
    }

    /// The Worker's own Host, when it holds one.
    pub(super) fn own(&self) -> Option<usize> {
        self.hosts.iter().position(|held| held.own)
    }

    /// The Host a request that names no terminal goes to: the own Host, or the first held.
    pub(super) fn primary_index(&self) -> usize {
        self.own().unwrap_or(0)
    }

    pub(super) fn primary(&self) -> &HostBrokerConnection {
        &self.hosts[self.primary_index()].connection
    }

    pub(super) fn connection(&self, index: usize) -> &HostBrokerConnection {
        &self.hosts[index].connection
    }

    /// The Host that serves terminals of `epoch`, or the primary one, which answers the epoch as
    /// stale.
    pub(super) fn by_epoch(&self, epoch: HostEpoch) -> usize {
        self.hosts
            .iter()
            .position(|held| held.serves(epoch))
            .unwrap_or_else(|| self.primary_index())
    }

    /// The Host that holds attachment `id`, or the primary one, which answers it as unavailable.
    pub(super) fn by_attachment(&self, id: OperationId) -> usize {
        self.hosts
            .iter()
            .position(|held| !held.drained() && held.connection.viewers.borrow().contains(id))
            .unwrap_or_else(|| self.primary_index())
    }

    /// Every Host that still serves, for a request that reads them all.
    pub(super) fn serving(&self) -> Vec<&HostBrokerConnection> {
        self.hosts
            .iter()
            .filter(|held| !held.drained())
            .map(|held| &held.connection)
            .collect()
    }

    /// The programs of every Host that still serves, for a request read from `server`.
    pub(super) fn programs<'s>(&'s self, server: &'s UnixStream) -> Option<HeldPrograms<'s>> {
        HeldPrograms::serving(
            self.hosts
                .iter()
                .filter(|held| !held.drained())
                .map(|held| (&held.connection, held.own)),
            server,
        )
    }

    /// Names `site` as where a failure of any Host would be recorded, for a request that reaches
    /// every Host.
    pub(super) fn set_site(&self, site: Site) {
        for held in &self.hosts {
            held.connection.pacing.set_site(site);
        }
    }

    /// The display mode a Host last observed from a Client's outer terminal, the own Host's first.
    pub(super) fn observed_colors(&self) -> Option<orchbee_local_runtime::UserTerminalColors> {
        std::iter::once(&self.hosts[self.primary_index()])
            .chain(self.hosts.iter())
            .find_map(|held| held.connection.host_colors.borrow().latest())
    }

    /// Waits for the Server's next request while taking every Host's events, until `idle` passes.
    /// A drained Host that ends meanwhile is dropped, and its slot freed.
    pub(super) fn receive(
        &mut self,
        server: &UnixStream,
        idle: Option<Instant>,
    ) -> Result<
        Option<(
            orchbee_capability_node_protocol::CapabilityNodeMessage,
            Instant,
        )>,
        orchbee_capability_node_protocol::ReadMessageError,
    > {
        let watched: Vec<_> = self
            .hosts
            .iter()
            .map(|held| super::server_transport::Watched {
                connection: &held.connection,
                may_end: held.drained(),
            })
            .collect();
        let among = super::server_transport::receive_among(server, &watched, idle)?;
        for index in among.ended.into_iter().rev() {
            let held = self.hosts.remove(index);
            #[cfg(any(windows, target_os = "macos"))]
            self.end_drained(held);
            #[cfg(not(any(windows, target_os = "macos")))]
            drop(held);
        }
        Ok(among.received)
    }

    /// Ends every Host's current request bound, and names the receive as each one's site.
    pub(super) fn end_request(&self) {
        for held in &self.hosts {
            held.connection.pacing.end_request();
            held.connection.pacing.set_site(Site::ServerReceive);
        }
    }

    /// Starts every Host's bound for the request whose bytes first became readable at `readable`.
    pub(super) fn begin_request_at(&self, readable: Instant) {
        for held in &self.hosts {
            held.connection.pacing.begin_request_at(readable);
        }
    }

    /// The Host whose failure ended the serving loop: the first that noted one, or the primary.
    pub(super) fn failed(&self) -> usize {
        self.hosts
            .iter()
            .position(|held| held.connection.pacing.failure().is_some())
            .unwrap_or_else(|| self.primary_index())
    }

    /// The Worker's own Host, started in the lowest free slot when it holds none, with the bound of
    /// the request read at `readable`. A failed start is recorded and fails only the request that
    /// needed it; after one no later start could pass, or the last of its attempts, none begins.
    pub(super) fn own_or_start(&mut self, readable: Instant) -> Result<usize, Box<dyn Error>> {
        if let Some(index) = self.own() {
            return Ok(index);
        }
        let Some(starter) = self.starter.as_ref() else {
            return Err("an unmanaged Worker starts no Host".into());
        };
        if !self.may_start {
            return Err("this Worker starts no further Host".into());
        }
        let slot = HostSlot::ALL
            .into_iter()
            .find(|slot| self.hosts.iter().all(|held| held.slot != *slot))
            .ok_or("every Host slot holds a Host")?;
        match start_host(
            starter.cohort,
            starter.runtime,
            Rc::clone(&starter.drain),
            HostStart::Fresh(slot),
        ) {
            Ok((connection, child)) => {
                crate::diagnostic_events::host_started();
                self.failed_starts = 0;
                connection.stream().set_nonblocking(true)?;
                connection.pacing.begin_request_at(readable);
                // The Keeper's care has this Host attach the Session's Keeper between requests. A
                // macOS Keeper parents only a terminal it starts, and one the Host forks itself
                // loses its exit status once a move lets that Host end, so a macOS Host attaches
                // it before it starts the terminal this request needs.
                self.add(slot, connection, ManagedHost::new(child), false);
                #[cfg(target_os = "macos")]
                self.attach_keeper();
                Ok(self.hosts.len() - 1)
            }
            Err(failure) => {
                failure.record();
                self.failed_starts += 1;
                self.may_start = failure.retryable() && self.failed_starts < HOST_START_ATTEMPTS;
                Err(failure.into_error())
            }
        }
    }

    /// What a managed stop ends: every Host that still serves, with the epoch its outcome names,
    /// the primary Host's. An unmanaged Worker stops none.
    pub(super) fn stop_hosts(&mut self) -> Option<StopHosts<'_>> {
        if !self.is_managed() {
            return None;
        }
        let epoch = HostEpoch::from_bytes(*self.primary().host_epoch().as_bytes()).ok()?;
        Some(StopHosts {
            hosts: self
                .hosts
                .iter_mut()
                .filter(|held| !held.drained())
                .filter_map(|held| Some((&held.connection, held.managed.as_mut()?)))
                .collect(),
            epoch,
            #[cfg(any(windows, target_os = "macos"))]
            keeper: &mut self.keeper,
            #[cfg(windows)]
            retiring: &mut self.retiring,
        })
    }
}

#[cfg(any(windows, target_os = "macos"))]
impl HostSet<'_> {
    /// The care between requests: the Session's Keeper and, on Windows, each Host's dispatch, then
    /// draining older Hosts. Returns when it began if a Server request became readable while it ran.
    pub(super) fn care(&mut self, server: &UnixStream) -> Option<Instant> {
        let began = Instant::now();
        let cared_began = self.with_care(|care, cared, keepers| care.run(cared, keepers, server));
        let checked = self.drain_empty();
        cared_began
            .or_else(|| (checked && super::server_transport::readable_now(server)).then_some(began))
    }

    /// Has each Host attach the Session's Keeper at once, starting it when none runs, ahead of the
    /// care: for a macOS Host this Worker just started for a request.
    #[cfg(target_os = "macos")]
    pub(super) fn attach_keeper(&mut self) {
        self.with_care(|care, cared, keepers| care.attach_now(cared, keepers));
    }

    /// Runs `turn` with the Keeper's care, each Host that still serves, and the Session's Keepers.
    fn with_care<R>(
        &mut self,
        turn: impl FnOnce(
            &mut super::keeper_care::KeeperCare,
            &mut [super::keeper_care::Cared<'_>],
            super::keeper_care::Keepers<'_>,
        ) -> R,
    ) -> R {
        use super::keeper_care::{Cared, Keepers};
        let program = self
            .starter
            .as_ref()
            .zip(self.host_program.as_deref())
            .map(|(starter, program)| (starter.runtime, program));
        let mut cared: Vec<Cared<'_>> = self
            .hosts
            .iter_mut()
            .filter(|held| !held.drained())
            .filter_map(|held| {
                Some(Cared {
                    connection: &held.connection,
                    managed: held.managed.as_mut()?,
                    watch: &mut held.watch,
                })
            })
            .collect();
        let keepers = Keepers {
            keeper: &mut self.keeper,
            #[cfg(windows)]
            retiring: &mut self.retiring,
            program,
        };
        turn(&mut self.care, &mut cared, keepers)
    }

    /// When the next care is due, so the wait for a request wakes for it.
    pub(super) const fn care_due(&self) -> Instant {
        self.care.due()
    }
}

#[cfg(any(windows, target_os = "macos"))]
impl HostSet<'_> {
    /// The older Hosts that still serve, which a move empties into the own Host.
    pub(super) fn older(&self) -> Vec<usize> {
        self.hosts
            .iter()
            .enumerate()
            .filter(|(_, held)| !held.own && !held.drained())
            .map(|(index, _)| index)
            .collect()
    }

    /// Whether the Host at `index` holds its running terminals in the Session's current Keeper,
    /// which a move needs of both Hosts.
    pub(super) fn keeper_attached(&self, index: usize) -> bool {
        self.keeper.is_some() && self.hosts[index].watch.attached()
    }

    /// The process of the Host at `index`, which a quiesce names as the Host its mark is for.
    pub(super) fn host_process(&self, index: usize) -> Option<(u32, u64)> {
        self.hosts[index].managed.as_ref()?.host_identity().ok()
    }

    /// The Session record slot of the Host at `index`, for diagnostics.
    pub(super) fn slot_index(&self, index: usize) -> u8 {
        self.hosts[index].slot.index()
    }
}

#[cfg(any(windows, target_os = "macos"))]
impl HostSet<'_> {
    /// Checks the Host left one of its exchanges unanswered once more as the Worker exits, as the
    /// Keeper's care does.
    pub(super) fn after_unanswered(&mut self, index: usize) {
        let held = &mut self.hosts[index];
        let Some(managed) = held.managed.as_mut() else {
            return;
        };
        let mut cared = super::keeper_care::Cared {
            connection: &held.connection,
            managed,
            watch: &mut held.watch,
        };
        super::keeper_care::after_unanswered(&mut cared, &mut self.keeper);
    }

    /// Drains each older Host that holds no terminal and runs no program, while the Worker holds its
    /// own Host: such a Host takes nothing new, so one check between requests at most every
    /// [`DRAIN_CHECK_PERIOD`] finds it empty for good. Returns whether any Host was checked.
    fn drain_empty(&mut self) -> bool {
        if self.own().is_none() {
            return false;
        }
        let now = Instant::now();
        let mut checked = false;
        for held in self.hosts.iter_mut().filter(|held| !held.own) {
            let Drain::Serving(due) = held.drain else {
                continue;
            };
            if now < due || held.connection.runs_programs() {
                continue;
            }
            checked = true;
            held.drain = Drain::Serving(now + DRAIN_CHECK_PERIOD);
            let empty = held
                .connection
                .query_process(Instant::now() + DRAIN_BOUND)
                .is_some_and(|entry| entry.terminals == 0);
            if empty && held.connection.drain_host(Instant::now() + DRAIN_BOUND) {
                held.drain = Drain::Drained;
                crate::diagnostic_events::host_draining(held.slot.index());
            }
        }
        checked
    }

    /// Frees the slot of `held`, a drained Host whose connection ended, in the Session record once
    /// its process exited.
    fn end_drained(&self, held: HeldHost) {
        let Some(mut managed) = held.managed else {
            return;
        };
        let deadline = Instant::now() + END_BOUND;
        let exited = loop {
            match managed.exited() {
                Ok(true) => break true,
                Ok(false) if Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Ok(false) | Err(_) => break false,
            }
        };
        let released = exited
            && self.starter.as_ref().is_some_and(|starter| {
                managed.host_identity().is_ok_and(|process| {
                    crate::host_adoption::release_slot(starter.runtime, held.slot, process)
                })
            });
        crate::diagnostic_events::host_drained(held.slot.index(), released);
    }
}
