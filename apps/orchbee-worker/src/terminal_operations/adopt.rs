//! A replacement Worker's first phase. Until the Server's AdoptHost pages are complete it serves
//! only those pages and a stop, declining every other request; it then adopts the Host of the
//! Worker it replaces, or none, and continues in the loop of that state: the admitted loop with
//! the imported kernel, or the lazy pre-Host loop. A failed adoption is answered, then ends the
//! Worker. It never starts a Host and never ends itself before an adoption fails.

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
    AdoptHostFailure, AdoptHostOutcome, AdoptHostPageKey, AdoptHostResult, AdoptHostTarget,
    CapabilityNodeMessage, DeclinedRequestKind, WorkerRequestDeclined, WorkerShutdownOutcome,
    WorkerShutdownResult,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_runtime::Cohort;
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

/// How long after the last page arrives its answer is written at the latest, inside the Server's
/// 20 s window.
const ADOPTION_BUDGET: Duration = Duration::from_secs(18);

/// Where the first phase left this Worker.
enum Phase {
    /// A stop completed before any adoption.
    Stopped,
    /// The Worker it replaces had no Host.
    NoHost,
    /// The Host was adopted and its bindings imported.
    Host(Box<Adopted>),
}

struct Adopted {
    host: HostBrokerConnection,
    managed: shutdown::ManagedHost,
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
            let Adopted {
                mut host,
                managed: mut managed_host,
                kernel,
            } = *adopted;
            super::run_admitted_with_handoff(
                server,
                &mut host,
                worker_id,
                generation,
                kernel,
                super::AdmittedAux {
                    drain: &drain,
                    system_shell,
                    worktree_slot: &worktree_slot,
                    agent_integrations: &agent_integrations,
                    agent_probes: &agent_probes,
                    mutation_slot: &mutation_slot,
                    plugin_jobs: &plugin_jobs,
                    session_runtime: Some(runtime),
                    managed_host: Some(&mut managed_host),
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
    reason = "the wait before an adoption takes the admitted Worker's owners one by one, as its               caller holds them"
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
                let channel = host::Channel {
                    server,
                    correlation: CorrelationId::from_bytes(key.adoption_id.as_bytes())?,
                    drain: Rc::clone(drain),
                    deadline: arrived + ADOPTION_BUDGET,
                };
                return match target {
                    AdoptHostTarget::NoHost => adoption.no_host(),
                    AdoptHostTarget::Host { process, epoch } => {
                        #[cfg(windows)]
                        let keeper_channel = (server, channel.correlation, channel.deadline);
                        let found = host::adopt(runtime, process, epoch, channel);
                        #[cfg(windows)]
                        let found = found.and_then(|found| {
                            with_keeper(found, cohort, runtime, keeper_channel, &bindings)
                        });
                        #[cfg(not(windows))]
                        let found = found.map(Found::from);
                        adoption.import(found, &bindings)
                    }
                    #[cfg(windows)]
                    AdoptHostTarget::Recovery { keeper, epoch } => {
                        let recovered = super::recovery::recover(
                            cohort, runtime, keeper, epoch, &bindings, channel,
                        );
                        adoption.import(recovered.map(Found::from), &bindings)
                    }
                    // Only a Windows Session starts a Keeper, so no other Server sends this target.
                    #[cfg(not(windows))]
                    AdoptHostTarget::Recovery { .. } => {
                        let _ = (cohort, channel);
                        Err(adoption.fail(host::Failed {
                            failure: AdoptHostFailure::RecoveryFailed,
                            restarts: 0,
                            error: "no Keeper runs on this platform".into(),
                        }))
                    }
                };
            }
        }
    }
}

/// What an adoption imports: the Host, how this Worker owns it, the Keeper it answers for, and
/// the Host's complete inventory.
struct Found {
    target: &'static str,
    host: HostBrokerConnection,
    managed: shutdown::ManagedHost,
    #[cfg(windows)]
    keeper: Option<crate::session_keeper::SessionKeeper>,
    inventory: Vec<crate::terminal_authorization::InventoryTerminal>,
    restarts: u32,
}

impl From<host::Found> for Found {
    fn from(found: host::Found) -> Self {
        Self {
            target: "host",
            host: found.host,
            managed: shutdown::ManagedHost::adopted(found.owner),
            #[cfg(windows)]
            keeper: None,
            inventory: found.inventory,
            restarts: found.restarts,
        }
    }
}

#[cfg(windows)]
impl From<super::recovery::Recovered> for Found {
    fn from(recovered: super::recovery::Recovered) -> Self {
        Self {
            target: "recovery",
            host: recovered.host,
            managed: shutdown::ManagedHost::new(recovered.child),
            keeper: Some(recovered.keeper),
            inventory: recovered.inventory,
            restarts: recovered.restarts,
        }
    }
}

/// An adopted Host with the Session's Keeper, where the record names a live one: this Worker
/// answers for the Keeper, the Host holds in it, and the orphans of the Host's epoch a binding
/// names are recovered into it before its inventory is read again. A Keeper that cannot be
/// adopted leaves the terminals unheld and the adoption going on; a recovery that fails fails it.
#[cfg(windows)]
fn with_keeper(
    found: host::Found,
    cohort: &Cohort,
    runtime: &Path,
    channel: (&UnixStream, CorrelationId, Instant),
    bindings: &[orchbee_capability_node_protocol::AdoptBinding],
) -> Result<Found, host::Failed> {
    let (server, correlation, deadline) = channel;
    let mut adopted = Found::from(found);
    if !adopted.host.keeper() {
        return Ok(adopted);
    }
    let executable = cohort.executable(orchbee_local_runtime::CohortArtifact::TerminalHost);
    let mut keeper = match crate::session_keeper::SessionKeeper::adopt(runtime, &executable, None) {
        Ok(Some(keeper)) => keeper,
        Ok(None) => return Ok(adopted),
        Err(error) => {
            crate::diagnostic_events::keeper_failed("adopt", error.as_ref());
            return Ok(adopted);
        }
    };
    let attached = adopted
        .managed
        .host_identity()
        .map_err(Box::<dyn Error>::from)
        .and_then(|process| {
            keeper.attach(&adopted.host, server, process)?;
            Ok(process)
        });
    let process = match attached {
        Ok(process) => process,
        Err(error) => {
            crate::diagnostic_events::keeper_failed("attach", error.as_ref());
            return Ok(adopted);
        }
    };
    let restarts = adopted.restarts;
    let recovered =
        super::recovery::recover_orphans(&mut keeper, &adopted.host, server, process, bindings)
            .map_err(|error| host::Failed {
                failure: AdoptHostFailure::RecoveryFailed,
                restarts,
                error,
            })?;
    if recovered > 0 {
        let (inventory, more) = host::read_inventory(&adopted.host, server, correlation, deadline)?;
        adopted.inventory = inventory;
        adopted.restarts += more;
    }
    adopted.keeper = Some(keeper);
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
    fn no_host(self) -> Result<Phase, Box<dyn Error>> {
        let _no_host = answer(
            self.server,
            None,
            self.key,
            AdoptHostOutcome::Adopted {
                imported: 0,
                quarantined: 0,
                missing: Vec::new(),
            },
        )?;
        crate::diagnostic_events::adoption(self.record("no-host", "adopted", 0));
        Ok(Phase::NoHost)
    }

    fn import(
        self,
        found: Result<Found, host::Failed>,
        bindings: &[orchbee_capability_node_protocol::AdoptBinding],
    ) -> Result<Phase, Box<dyn Error>> {
        let found = match found {
            Ok(found) => found,
            Err(failed) => return Err(self.fail(failed)),
        };
        let target = found.target;
        let mut kernel = AuthorizedHostOperationKernel::new(self.worker_id, self.generation)?;
        let imported = match kernel.import(bindings, &found.inventory) {
            Ok(imported) => imported,
            Err(error) => {
                return Err(self.fail(host::Failed {
                    failure: AdoptHostFailure::InventoryFailed,
                    restarts: found.restarts,
                    error: Box::new(error),
                }));
            }
        };
        let host = found.host;
        host.quarantine(
            imported
                .quarantined
                .iter()
                .map(|identity| identity.terminal_id()),
        );
        let quarantined = u32::try_from(imported.quarantined.len())?;
        let missing = u32::try_from(imported.missing.len())?;
        let host_closed = answer(
            self.server,
            Some(&host),
            self.key,
            AdoptHostOutcome::Adopted {
                imported: imported.imported,
                quarantined,
                missing: imported.missing,
            },
        );
        match host_closed {
            Ok(false) => {}
            Ok(true) => {
                return Err(super::exit_cause::attach(
                    super::peer_closed().into(),
                    &host,
                ));
            }
            Err(error) => return Err(super::exit_cause::attach(error, &host)),
        }
        crate::diagnostic_events::adoption(AdoptionRecord {
            imported: imported.imported,
            quarantined,
            missing,
            ..self.record(target, "adopted", found.restarts)
        });
        let managed = found.managed;
        // An orphan of this Host's epoch that no binding names is discarded once the import
        // succeeded: no person can reach it.
        #[cfg(windows)]
        let managed = {
            let mut managed = managed;
            if let Some(mut keeper) = found.keeper {
                super::recovery::discard_unbound(
                    &mut keeper,
                    *host.host_epoch().as_bytes(),
                    bindings,
                );
                managed.keep(Some(keeper));
            }
            managed
        };
        Ok(Phase::Host(Box::new(Adopted {
            host,
            managed,
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
