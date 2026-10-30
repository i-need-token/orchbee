//! Shared exit contract for one Worker generation's background job owners: request stop from
//! every owner first, then observe each owner's completion against one deadline.
use crate::worktree_discovery_job::WorktreeDiscoverySlot;
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

/// Settlement window after the connection loop ends without an explicit stop. It exceeds the
/// five-second process cleanup budget each owner applies after observing cancellation.
const EXIT_SETTLEMENT_BUDGET: Duration = Duration::from_secs(6);

/// Borrowed view of the background job owners that outlive one request loop phase.
pub(super) struct BackgroundJobs<'a> {
    pub(super) discovery: &'a WorktreeDiscoverySlot,
    #[cfg(windows)]
    pub(super) mutations: &'a crate::worktree_mutation_job::WorktreeMutationSlot,
    #[cfg(windows)]
    pub(super) plugins: &'a crate::plugin_jobs::PluginJobs,
}

impl BackgroundJobs<'_> {
    /// Every owner is asked to stop before any wait, so cancellations overlap and one slow owner
    /// cannot withhold the request from another. Every owner is observed even after a failure.
    /// `Ok` means every owner has ended; the first failure is reported otherwise.
    pub(super) fn stop_before(&self, deadline: Instant) -> io::Result<()> {
        #[cfg(windows)]
        {
            self.plugins.request_stop();
            self.mutations.request_stop();
        }
        self.discovery.request_stop();
        #[cfg(windows)]
        let settled = {
            let plugins = self.plugins.stop_before(deadline);
            let mutations = self.mutations.stop_before(deadline);
            plugins.and(mutations)
        };
        #[cfg(not(windows))]
        let settled = Ok(());
        let discovery = self.discovery.stop_before(deadline);
        settled.and(discovery)
    }

    /// Fallback settlement when the request loop ends by disconnection, protocol failure or
    /// startup failure. Unproven cleanup takes precedence over the loop's own outcome.
    pub(super) fn settle_exit(
        &self,
        outcome: Result<(), Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        self.stop_before(Instant::now() + EXIT_SETTLEMENT_BUDGET)?;
        outcome
    }
}

#[cfg(all(test, windows))]
mod tests;
