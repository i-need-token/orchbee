//! The Session's entry points to its Agent catalog: Session composition starts and stops the probe
//! pump, and the `workbench.agent-catalog` read and refresh answer from it here.
//!
//! A State stream of the read follows the catalog's own change count, never commits or terminal
//! output: the catalog wakes every watching Client connection after each revision it publishes,
//! and the connection's dispatcher then reads the catalog once and sends the frame only when its
//! bytes changed. Revisions that arrive while a slow Client's dispatcher is busy coalesce into one
//! pass, so that Client receives the latest catalog, never a queue of the ones in between. A
//! watcher is held weakly, so a closed connection leaves nothing behind, and publishing never
//! waits on one.
use super::LocalTerminalService;
use crate::agent_catalog::{AgentCatalogSnapshot, AgentRowState, LookupStatus};
use orchbee_server::SessionChangeSignal;
use orchbee_workbench_protocol::{
    AgentCatalogAction as Action, AgentCatalogEntry, AgentCatalogEntryState,
    AgentCatalogFailure as Failure, AgentCatalogLookup, AgentCatalogLookupStatus,
    AgentCatalogOutcome as Outcome, AgentCatalogRequest, AgentCatalogResult, AgentStartSelector,
    ProtocolVersion, WorkbenchAgentCatalog, WorkbenchRevision,
};
use std::{io, sync::Arc, time::Instant};

impl LocalTerminalService {
    /// Starts looking up the registered Worker's Agents. Only Session composition starts it.
    pub(crate) fn start_agent_catalog(&self) -> io::Result<()> {
        let catalog = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry poisoned"))?
            .agent_catalog();
        catalog.start_probes(Arc::downgrade(&self.workers))
    }

    /// Stops the probe pump and joins its thread within the Session owner's deadline.
    #[cfg(windows)]
    pub(crate) fn stop_agent_catalog_before(&self, deadline: Instant) -> io::Result<()> {
        let catalog = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry poisoned"))?
            .agent_catalog();
        catalog.stop_probes_before(deadline)
    }

    /// Wakes `signal` after every catalog revision and whenever the catalog becomes readable or
    /// stops being readable.
    pub(crate) fn watch_agent_catalog(&self, signal: &Arc<SessionChangeSignal>) {
        if let Ok(workers) = self.workers.lock() {
            workers.agent_catalog().watch(signal);
        }
    }

    pub(crate) fn execute_agent_catalog(&self, request: AgentCatalogRequest) -> AgentCatalogResult {
        let outcome = match self.workbench_session_matches(&request.session_id) {
            Some(true) => match request.action {
                Action::Current {} => self.agent_catalog(),
                Action::Refresh {} => self.refresh_agent_catalog(),
            }
            .as_ref()
            .and_then(wire)
            .map_or(
                Outcome::Failed {
                    reason: Failure::Unavailable,
                },
                |catalog| Outcome::Current { catalog },
            ),
            Some(false) => Outcome::Failed {
                reason: Failure::Denied,
            },
            None => Outcome::Failed {
                reason: Failure::Unavailable,
            },
        };
        AgentCatalogResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }

    /// The catalog as it stands; `None` before the first Worker registered and once stopping.
    fn agent_catalog(&self) -> Option<AgentCatalogSnapshot> {
        self.workers.lock().ok()?.agent_catalog().read()
    }

    /// Asks for a new lookup, which the catalog's schedule may absorb or ignore, and answers with
    /// the catalog as it stands.
    fn refresh_agent_catalog(&self) -> Option<AgentCatalogSnapshot> {
        self.workers
            .lock()
            .ok()?
            .agent_catalog()
            .refresh(Instant::now())
    }
}

/// The wire form of a snapshot. Every id comes from the startable catalog, so `None` cannot occur
/// in practice; it answers `unavailable` rather than a catalog missing a row.
fn wire(snapshot: &AgentCatalogSnapshot) -> Option<WorkbenchAgentCatalog> {
    Some(WorkbenchAgentCatalog {
        revision: WorkbenchRevision::new(snapshot.revision.get()),
        startable: snapshot.startable,
        lookup: AgentCatalogLookup {
            status: match snapshot.status {
                LookupStatus::Running => AgentCatalogLookupStatus::Running,
                LookupStatus::Complete => AgentCatalogLookupStatus::Complete,
                LookupStatus::Failed => AgentCatalogLookupStatus::Failed,
                LookupStatus::Unsupported => AgentCatalogLookupStatus::Unsupported,
            },
            checked_at: snapshot.checked_at,
        },
        recent: snapshot
            .recent
            .iter()
            .map(|id| AgentStartSelector::parse(id))
            .collect::<Option<_>>()?,
        agents: snapshot
            .agents
            .iter()
            .map(|row| {
                Some(AgentCatalogEntry {
                    provider_id: AgentStartSelector::parse(row.provider_id)?,
                    product_name: row.product_name.to_owned(),
                    command: row.command.clone(),
                    state: match row.state {
                        AgentRowState::Found => AgentCatalogEntryState::Found,
                        AgentRowState::NotFound => AgentCatalogEntryState::NotFound,
                        AgentRowState::CantCheck => AgentCatalogEntryState::CantCheck,
                        AgentRowState::Checking => AgentCatalogEntryState::Checking,
                        AgentRowState::Unknown => AgentCatalogEntryState::Unknown,
                    },
                })
            })
            .collect::<Option<_>>()?,
    })
}
