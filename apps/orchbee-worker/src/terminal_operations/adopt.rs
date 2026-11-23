//! A replacement Worker's first phase. Until the Server's AdoptHost pages are complete it serves
//! only those pages and a stop, declining every other request; it then adopts the Hosts of the
//! Worker it replaces, or none, and continues in the loop of that state: the admitted loop with
//! the kernel imported from every Host's inventory, or the lazy pre-Host loop. On Windows that is
//! up to one Host per slot, each adopted, or recovered into a recovery Host when it ended, with
//! the Session's Keeper adopted once for all of them. A failed adoption is answered, then ends the
//! Worker. It starts no Host but a recovery Host and never ends itself before an adoption fails.

use std::{
    cell::RefCell,
    error::Error,
    io,
    num::NonZeroU64,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AdoptHostFailure, AdoptHostKind, AdoptHostOutcome, AdoptHostPageKey, AdoptHostResult,
    CapabilityNodeMessage, DeclinedRequestKind, WorkerRequestDeclined, WorkerShutdownOutcome,
    WorkerShutdownResult,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_runtime::{Cohort, HostSlot};
use orchbee_local_worker_protocol::WorkerId;
use orchbee_terminal_protocol::CorrelationId;

use super::{background_jobs::BackgroundJobs, server_transport, shutdown};
use crate::{
    agent_drain::AgentDrain,
    default_interactive_launch::SystemShell,
    diagnostic_events::{AdoptionRecord, DeclineCause, ExitCause, Peer, Reason, ServingSite, Site},
    host_adoption as host,
    host_connection::HostBrokerConnection,
    terminal_authorization::AuthorizedHostOperationKernel,
};

mod pages;
#[cfg(any(windows, target_os = "macos"))]
mod pairs;

/// How long after the last page arrives its answer is written at the latest, inside the Server's
/// 20 s window.
const ADOPTION_BUDGET: Duration = Duration::from_secs(18);

/// Where the first phase left this Worker.
enum Phase {
    /// A stop completed before any adoption.
    Stopped,
    /// The Worker it replaces had no Host.
    NoHost,
    /// The Hosts were adopted and their bindings imported.
    Host(Box<Adopted>),
}

struct Adopted {
    found: Vec<Found>,
    #[cfg(any(windows, target_os = "macos"))]
    keeper: Option<crate::session_keeper::SessionKeeper>,
    kernel: AuthorizedHostOperationKernel,
}

pub(crate) fn run(
    server: UnixStream,
    worker_id: WorkerId,
    generation: NonZeroU64,
    cohort: &Cohort,
    runtime: &Path,
    system_shell: &SystemShell,
) -> Result<(), Box<dyn Error>> {
    server.set_nonblocking(true)?;
    // The Agent Drain queue belongs to the admitted Server connection and exists before the Host
    // connection that appends to it.
    let drain = Rc::new(RefCell::new(AgentDrain::new()));
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    let agent_integrations = crate::agent_integration_jobs::AgentIntegrationJobs::new();
    let agent_probes = crate::agent_probe_jobs::AgentProbeJobs::new();
    let plugin_jobs = crate::plugin_jobs::PluginJobs::new();
    let mutation_slot =
        crate::worktree_mutation_job::WorktreeMutationSlot::new(crate::worktree_mutation_git::run);
    let jobs = || BackgroundJobs {
        discovery: &worktree_slot,
        integrations: &agent_integrations,
        probes: &agent_probes,
        mutations: &mutation_slot,
        plugins: &plugin_jobs,
    };
    let site = ServingSite::new(Site::ServerReceive);
    let phase = serve_until_adopted(
        &server,
        worker_id,
        generation,
        cohort,
        runtime,
        &drain,
        jobs(),
        &site,
    )
    .map_err(|error| super::exit_cause::attach_without_host(error, site.get()));
    let outcome = match phase {
        // No job ran before adoption, and the lazy loop owns its own.
        Ok(Phase::NoHost) => {
            return super::lazy::run(server, worker_id, generation, cohort, runtime, system_shell);
        }
        Ok(Phase::Stopped) => Ok(()),
        Ok(Phase::Host(adopted)) => {
            let adopted = *adopted;
            let mut hosts = super::host_set::HostSet::managed(super::host_set::Starter {
                cohort,
                runtime,
                drain: Rc::clone(&drain),
            });
            for found in adopted.found {
                // The background programs each Host still runs for the Worker this one replaces.
                if let Some(programs) =
                    crate::host_programs::HostPrograms::between_requests(&found.host)
                {
                    plugin_jobs.adopt(&programs);
                }
                #[cfg(any(windows, target_os = "macos"))]
                let offered = found.offered;
                #[cfg(not(any(windows, target_os = "macos")))]
                let offered = false;
                hosts.add(found.slot, found.host, found.managed, offered);
            }
            #[cfg(any(windows, target_os = "macos"))]
            hosts.keep(adopted.keeper);
            super::run_admitted_with_handoff(
                server,
                hosts,
                worker_id,
                generation,
                adopted.kernel,
                super::AdmittedAux {
                    drain: &drain,
                    system_shell,
                    worktree_slot: &worktree_slot,
                    agent_integrations: &agent_integrations,
                    agent_probes: &agent_probes,
                    mutation_slot: &mutation_slot,
                    plugin_jobs: &plugin_jobs,
                    session_runtime: Some(runtime),
                },
                #[cfg(windows)]
                super::handoff::Handoff::new(Some(runtime)),
            )
        }
        Err(error) => Err(error),
    };
    jobs().settle_exit(outcome)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the wait before an adoption takes the admitted Worker's owners one by one, as its caller holds them"
)]
fn serve_until_adopted(
    server: &UnixStream,
    worker_id: WorkerId,
    generation: NonZeroU64,
    cohort: &Cohort,
    runtime: &Path,
    drain: &Rc<RefCell<AgentDrain>>,
    jobs: BackgroundJobs<'_>,
    site: &ServingSite,
) -> Result<Phase, Box<dyn Error>> {
    let mut pages = pages::Pages::default();
    loop {
        site.set(Site::ServerReceive);
        let (request, arrived) = server_transport::receive_timed(server, None)?;
        let request = match request {
            CapabilityNodeMessage::AdoptHostRequest(request) => request,
            CapabilityNodeMessage::WorkerShutdownRequest(request) => {
                site.set(Site::Shutdown);
                if let Some(reason) = shutdown::refusal(request, worker_id, generation, true) {
                    let _no_host = server_transport::send(
                        server,
                        None,
                        &CapabilityNodeMessage::WorkerShutdownResult(WorkerShutdownResult {
                            request,
                            outcome: WorkerShutdownOutcome::Refused(reason),
                        }),
                    )?;
                    continue;
                }
                shutdown::run(server, request, worker_id, generation, jobs, None)?;
                return Ok(Phase::Stopped);
            }
            // Nothing is bound before the adoption, so nothing else can be served.
            request => {
                super::decline(server, None, &request, DeclineCause::Unavailable)?;
                continue;
            }
        };
        site.set(Site::Adopt);
        let key = request.key();
        match pages.accept(request, worker_id, generation) {
            Err(pages::Refused) => super::send_decline(
                server,
                None,
                WorkerRequestDeclined::operation(DeclinedRequestKind::AdoptHost, key.adoption_id),
                DeclineCause::Unavailable,
            )?,
            Ok(pages::Accepted::Page) => {
                let _no_host = answer(server, None, key, AdoptHostOutcome::PageAccepted)?;
            }
            Ok(pages::Accepted::Complete { target, bindings }) => {
                let adoption = Adoption {
                    server,
                    key,
                    arrived,
                    worker_id,
                    generation,
                };
                if target.slots().is_empty() {
                    return adoption.no_host(Vec::new());
                }
                let correlation = CorrelationId::from_bytes(key.adoption_id.as_bytes())?;
                #[cfg(any(windows, target_os = "macos"))]
                let mut keeper = KeeperAdoption {
                    runtime,
                    named: target.keeper(),
                    keeper: None,
                    tried: false,
                };
                let mut found = Vec::with_capacity(target.slots().len());
                for slot in target.slots() {
                    let channel = host::Channel {
                        server,
                        correlation,
                        drain: Rc::clone(drain),
                        deadline: arrived + ADOPTION_BUDGET,
                    };
                    match adopt_slot(
                        slot,
                        cohort,
                        runtime,
                        channel,
                        #[cfg(any(windows, target_os = "macos"))]
                        &mut keeper,
                        &bindings,
                    ) {
                        Ok(Some(adopted)) => found.push(adopted),
                        Ok(None) => {}
                        Err(failed) => return Err(adoption.fail(failed)),
                    }
                }
                // Every Host the target named ended on its own, and with it every terminal.
                if found.is_empty() {
                    let missing = bindings.iter().map(|binding| binding.terminal_id).collect();
                    return adoption.no_host(missing);
                }
                return adoption.import(
                    found,
                    #[cfg(any(windows, target_os = "macos"))]
                    keeper.keeper,
                    &bindings,
                );
            }
        }
    }
}

/// Adopts the Host in one slot of the target, or on Windows and macOS recovers the terminals of one
/// that ended into a recovery Host this Worker starts in that slot. `None` on Windows names a Host that
/// refused this Worker as `stopping` and then ended: it drained or stopped on its own, and its slot
/// is freed.
fn adopt_slot(
    slot: &orchbee_capability_node_protocol::AdoptHostSlot,
    cohort: &Cohort,
    runtime: &Path,
    channel: host::Channel<'_>,
    #[cfg(any(windows, target_os = "macos"))] keeper: &mut KeeperAdoption<'_>,
    bindings: &[orchbee_capability_node_protocol::AdoptBinding],
) -> Result<Option<Found>, host::Failed> {
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = bindings;
    let refused = |error: &str| host::Failed {
        failure: AdoptHostFailure::HostRefused,
        restarts: 0,
        error: error.into(),
    };
    let Some(host_slot) = HostSlot::new(slot.slot) else {
        return Err(refused("no such Host slot"));
    };
    // A Linux Session runs its one Host in the first slot.
    #[cfg(all(unix, not(target_os = "macos")))]
    if host_slot != HostSlot::FIRST {
        let _ = (cohort, channel);
        return Err(refused("a POSIX Session runs one Host"));
    }
    #[cfg(any(windows, target_os = "macos"))]
    let keeper_channel = (channel.server, channel.correlation, channel.deadline);
    let found = match slot.kind {
        AdoptHostKind::Host { process } => {
            match host::adopt(runtime, host_slot, process, slot.epoch, channel) {
                #[cfg(any(windows, target_os = "macos"))]
                Err(failed)
                    if failed.error.is::<crate::host_connection::HostStopping>()
                        && ended_on_its_own(runtime, host_slot, process, keeper_channel.2) =>
                {
                    return Ok(None);
                }
                adopted => adopted.map(|found| Found::adopted(host_slot, found)),
            }
        }
        #[cfg(any(windows, target_os = "macos"))]
        AdoptHostKind::Recovery => {
            let held = if keeper.named.is_some() {
                keeper.adopted()
            } else {
                None
            };
            match held {
                Some(held) => super::recovery::recover(
                    cohort, runtime, host_slot, held, slot.epoch, bindings, channel,
                )
                .map(|recovered| Found::recovered(host_slot, recovered)),
                None => Err(host::Failed {
                    failure: AdoptHostFailure::RecoveryFailed,
                    restarts: 0,
                    error: if keeper.named.is_some() {
                        "the Session's Keeper is gone".into()
                    } else {
                        "a recovery names no Keeper".into()
                    },
                }),
            }
        }
        // No Linux Session starts a Keeper.
        #[cfg(not(any(windows, target_os = "macos")))]
        AdoptHostKind::Recovery => {
            let _ = (cohort, channel);
            Err(host::Failed {
                failure: AdoptHostFailure::RecoveryFailed,
                restarts: 0,
                error: "no Keeper runs on this platform".into(),
            })
        }
    }?;
    #[cfg(any(windows, target_os = "macos"))]
    if matches!(slot.kind, AdoptHostKind::Host { .. }) {
        return with_keeper(found, keeper, keeper_channel, bindings).map(Some);
    }
    Ok(Some(found))
}

/// Frees `slot` of a Host that refused this Worker as `stopping` once its process ended within
/// [`ENDED_BOUND`] and `deadline`: the Worker before drained it, or it stopped, and no Worker can
/// reach its terminals any more.
#[cfg(any(windows, target_os = "macos"))]
fn ended_on_its_own(
    runtime: &Path,
    slot: HostSlot,
    process: Option<orchbee_capability_node_protocol::AdoptHostProcess>,
    deadline: Instant,
) -> bool {
    let Some(process) = process else {
        return false;
    };
    if !host::ended_before(process, deadline.min(Instant::now() + ENDED_BOUND)) {
        return false;
    }
    let released = host::release_slot(
        runtime,
        slot,
        (process.process_id.get(), process.creation_time.get()),
    );
    crate::diagnostic_events::host_drained(slot.index(), released);
    true
}

/// How long an adoption waits for a Host that refused it as `stopping` to end.
#[cfg(any(windows, target_os = "macos"))]
const ENDED_BOUND: Duration = Duration::from_secs(2);

/// What an adoption imports from one slot: the Host, how this Worker owns it, and the Host's
/// complete inventory.
struct Found {
    target: &'static str,
    slot: HostSlot,
    host: HostBrokerConnection,
    managed: shutdown::ManagedHost,
    /// Whether the Session's Keeper was offered to the Host during the adoption.
    #[cfg(any(windows, target_os = "macos"))]
    offered: bool,
    inventory: Vec<crate::terminal_authorization::InventoryTerminal>,
    restarts: u32,
}

impl Found {
    fn adopted(slot: HostSlot, found: host::Found) -> Self {
        Self {
            target: "host",
            slot,
            host: found.host,
            managed: shutdown::ManagedHost::adopted(found.owner),
            #[cfg(any(windows, target_os = "macos"))]
            offered: false,
            inventory: found.inventory,
            restarts: found.restarts,
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn recovered(slot: HostSlot, recovered: super::recovery::Recovered) -> Self {
        Self {
            target: "recovery",
            slot,
            host: recovered.host,
            managed: shutdown::ManagedHost::new(recovered.child),
            offered: true,
            inventory: recovered.inventory,
            restarts: recovered.restarts,
        }
    }
}

/// The Session's Keeper an adoption answers for, adopted once, when the first Host that holds in
/// one needs it: the one the target names, or any the Session record names when it names none.
#[cfg(any(windows, target_os = "macos"))]
struct KeeperAdoption<'a> {
    runtime: &'a Path,
    named: Option<orchbee_capability_node_protocol::AdoptHostProcess>,
    keeper: Option<crate::session_keeper::SessionKeeper>,
    tried: bool,
}

#[cfg(any(windows, target_os = "macos"))]
impl KeeperAdoption<'_> {
    /// The adopted Keeper; a failed adoption is recorded once and leaves none.
    fn adopted(&mut self) -> Option<&mut crate::session_keeper::SessionKeeper> {
        if !std::mem::replace(&mut self.tried, true) {
            let expected = self
                .named
                .map(|keeper| (keeper.process_id.get(), keeper.creation_time.get()));
            match crate::session_keeper::SessionKeeper::adopt(self.runtime, expected) {
                Ok(keeper) => self.keeper = keeper,
                Err(error) => crate::diagnostic_events::keeper_failed("adopt", error.as_ref()),
            }
        }
        self.keeper.as_mut()
    }
}

/// An adopted Host with the Session's Keeper, where the record names a live one: the Host holds in
/// it, and the orphans of the Host's epoch a binding names are recovered into it before
/// its inventory is read again. A Keeper that cannot be adopted leaves the terminals unheld and the
/// adoption going on; a recovery that fails fails it.
#[cfg(any(windows, target_os = "macos"))]
fn with_keeper(
    mut adopted: Found,
    keeper: &mut KeeperAdoption<'_>,
    channel: (&UnixStream, CorrelationId, Instant),
    bindings: &[orchbee_capability_node_protocol::AdoptBinding],
) -> Result<Found, host::Failed> {
    let (server, correlation, deadline) = channel;
    if !adopted.host.keeper() {
        return Ok(adopted);
    }
    let Some(keeper) = keeper.adopted() else {
        return Ok(adopted);
    };
    adopted.offered = true;
    let attached = adopted
        .managed
        .host_identity()
        .map_err(Box::<dyn Error>::from)
        .and_then(|process| {
            keeper.attach(
                &adopted.host,
                crate::session_keeper::Serving::Request(server),
                process,
            )?;
            Ok(process)
        });
    let process = match attached {
        Ok(process) => process,
        Err(error) => {
            crate::diagnostic_events::keeper_failed("attach", error.as_ref());
            return Ok(adopted);
        }
    };
    {
        let restarts = adopted.restarts;
        let recovered =
            super::recovery::recover_orphans(keeper, &adopted.host, server, process, bindings)
                .map_err(|error| host::Failed {
                    failure: AdoptHostFailure::RecoveryFailed,
                    restarts,
                    error,
                })?;
        if recovered > 0 {
            let (inventory, more) =
                host::read_inventory(&adopted.host, server, correlation, deadline)?;
            adopted.inventory = inventory;
            adopted.restarts += more;
        }
    }
    Ok(adopted)
}

/// The completed pages of one adoption, answered once.
struct Adoption<'a> {
    server: &'a UnixStream,
    key: AdoptHostPageKey,
    arrived: Instant,
    worker_id: WorkerId,
    generation: NonZeroU64,
}

impl Adoption<'_> {
    /// Answers an adoption that leaves no Host, with `missing` the bindings whose Host ended.
    fn no_host(
        self,
        missing: Vec<orchbee_capability_node_protocol::TerminalId>,
    ) -> Result<Phase, Box<dyn Error>> {
        let _no_host = answer(
            self.server,
            None,
            self.key,
            AdoptHostOutcome::Adopted {
                imported: 0,
                quarantined: 0,
                missing,
                moved: Vec::new(),
            },
        )?;
        crate::diagnostic_events::adoption(self.record("no-host", "adopted", 0));
        Ok(Phase::NoHost)
    }

    /// Imports the bindings against every adopted Host's inventory at once, so each binding finds
    /// its terminal on the Host that serves its epoch, and answers the adoption. On Windows the two
    /// copies a half-done move left of a terminal are paired first, and a binding that names the
    /// copy a move left behind is answered as moved.
    fn import(
        self,
        mut found: Vec<Found>,
        #[cfg(any(windows, target_os = "macos"))] keeper: Option<
            crate::session_keeper::SessionKeeper,
        >,
        bindings: &[orchbee_capability_node_protocol::AdoptBinding],
    ) -> Result<Phase, Box<dyn Error>> {
        let target = if found.iter().any(|found| found.target == "recovery") {
            "recovery"
        } else {
            "host"
        };
        let restarts = found
            .iter()
            .fold(0_u32, |sum, found| sum.saturating_add(found.restarts));
        #[cfg(any(windows, target_os = "macos"))]
        let paired = match pairs::pair(
            &mut found,
            bindings,
            self.server,
            self.arrived + ADOPTION_BUDGET,
        ) {
            Ok(paired) => paired,
            Err(failed) => return Err(self.fail(failed)),
        };
        #[cfg(any(windows, target_os = "macos"))]
        let bindings = paired.bindings.as_slice();
        let mut inventory = Vec::new();
        for found in &mut found {
            inventory.append(&mut found.inventory);
        }
        let mut kernel = AuthorizedHostOperationKernel::new(self.worker_id, self.generation)?;
        let imported = match kernel.import(bindings, &inventory) {
            Ok(imported) => imported,
            Err(error) => {
                return Err(self.fail(host::Failed {
                    failure: AdoptHostFailure::InventoryFailed,
                    restarts,
                    error: Box::new(error),
                }));
            }
        };
        for found in &found {
            let epoch = *found.host.host_epoch().as_bytes();
            found.host.quarantine(
                imported
                    .quarantined
                    .iter()
                    .filter(|identity| identity.host_epoch().as_bytes() == epoch)
                    .map(|identity| identity.terminal_id()),
            );
        }
        let quarantined = u32::try_from(imported.quarantined.len())?;
        let missing = u32::try_from(imported.missing.len())?;
        // A rebound binding whose owner the import did not take is missing, not moved.
        #[cfg(any(windows, target_os = "macos"))]
        let moved = paired
            .moved
            .into_iter()
            .filter(|moved| !imported.missing.contains(&moved.terminal_id))
            .collect();
        #[cfg(not(any(windows, target_os = "macos")))]
        let moved = Vec::new();
        let host = &found
            .first()
            .ok_or_else(|| io::Error::other("an adoption without a Host"))?
            .host;
        let host_closed = answer(
            self.server,
            Some(host),
            self.key,
            AdoptHostOutcome::Adopted {
                imported: imported.imported,
                quarantined,
                missing: imported.missing,
                moved,
            },
        );
        match host_closed {
            Ok(false) => {}
            Ok(true) => {
                return Err(super::exit_cause::attach(super::peer_closed().into(), host));
            }
            Err(error) => return Err(super::exit_cause::attach(error, host)),
        }
        crate::diagnostic_events::adoption(AdoptionRecord {
            imported: imported.imported,
            quarantined,
            missing,
            ..self.record(target, "adopted", restarts)
        });
        // An orphan of an adopted Host's epoch that no binding names is discarded once the import
        // succeeded: no person can reach it.
        #[cfg(any(windows, target_os = "macos"))]
        let mut keeper = keeper;
        #[cfg(any(windows, target_os = "macos"))]
        if let Some(keeper) = keeper.as_mut() {
            for found in &found {
                super::recovery::discard_unbound(
                    keeper,
                    *found.host.host_epoch().as_bytes(),
                    bindings,
                );
            }
        }
        Ok(Phase::Host(Box::new(Adopted {
            found,
            #[cfg(any(windows, target_os = "macos"))]
            keeper,
            kernel,
        })))
    }

    /// Answers the failure, then returns the error that ends this Worker at site `adopt`. A
    /// recovery Host or Keeper the attempt started or adopted stays running for the next attempt.
    fn fail(self, failed: host::Failed) -> Box<dyn Error> {
        let _delivery = answer(
            self.server,
            None,
            self.key,
            AdoptHostOutcome::Failed(failed.failure),
        );
        crate::diagnostic_events::adoption(self.record(
            "host",
            failure_token(failed.failure),
            failed.restarts,
        ));
        let reason = match failed.failure {
            AdoptHostFailure::HostGone | AdoptHostFailure::HostUnreachable => Reason::HostClosed,
            AdoptHostFailure::HostUnverified
            | AdoptHostFailure::HostMismatch
            | AdoptHostFailure::HostRefused
            | AdoptHostFailure::InventoryFailed
            | AdoptHostFailure::RecoveryFailed => Reason::HostProtocol,
        };
        Box::new(ExitCause {
            site: Site::Adopt,
            reason,
            peer: Peer::Host,
            waited: None,
            source: failed.error,
        })
    }

    fn record(
        &self,
        target: &'static str,
        outcome: &'static str,
        inventory_restarts: u32,
    ) -> AdoptionRecord {
        AdoptionRecord {
            target,
            outcome,
            imported: 0,
            quarantined: 0,
            missing: 0,
            pages: u32::from(self.key.page.index) + 1,
            inventory_restarts,
            elapsed: self.arrived.elapsed(),
        }
    }
}

fn answer(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    key: AdoptHostPageKey,
    outcome: AdoptHostOutcome,
) -> Result<bool, Box<dyn Error>> {
    let result = AdoptHostResult::new(key, outcome)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    Ok(server_transport::send(
        server,
        host,
        &CapabilityNodeMessage::AdoptHostResult(result),
    )?)
}

const fn failure_token(failure: AdoptHostFailure) -> &'static str {
    match failure {
        AdoptHostFailure::HostGone => "host-gone",
        AdoptHostFailure::HostUnreachable => "host-unreachable",
        AdoptHostFailure::HostUnverified => "host-unverified",
        AdoptHostFailure::HostMismatch => "host-mismatch",
        AdoptHostFailure::HostRefused => "host-refused",
        AdoptHostFailure::InventoryFailed => "inventory-failed",
        AdoptHostFailure::RecoveryFailed => "recovery-failed",
    }
}
