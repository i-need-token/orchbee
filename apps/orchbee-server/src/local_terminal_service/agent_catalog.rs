//! The Session's entry points to its Agent catalog: Session composition starts and stops the probe
//! pump, and the `workbench.agent-catalog` read and refresh read the catalog through here.
use super::LocalTerminalService;
use crate::agent_catalog::AgentCatalogSnapshot;
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

    /// The catalog as it stands; `None` before the first Worker registered and once stopping.
    #[expect(dead_code, reason = "the workbench.agent-catalog read calls it")]
    pub(crate) fn agent_catalog(&self) -> Option<AgentCatalogSnapshot> {
        self.workers.lock().ok()?.agent_catalog().read()
    }

    /// Asks for a new lookup, which the catalog's schedule may absorb or ignore, and answers with
    /// the catalog as it stands.
    #[expect(dead_code, reason = "the workbench.agent-catalog refresh calls it")]
    pub(crate) fn refresh_agent_catalog(&self) -> Option<AgentCatalogSnapshot> {
        self.workers
            .lock()
            .ok()?
            .agent_catalog()
            .refresh(Instant::now())
    }
}
