//! Answers the published-title read from committed bindings and the Session title registry. It
//! never exchanges with a Worker, maintains Agent reports or advances a revision, so a Client can
//! subscribe to it without any Session cost beyond the read itself.
use super::super::LocalTerminalService;
use orchbee_workbench_protocol::{
    PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest, PanelAgentResult, PresentationTitle,
    PresentationTitleRow, ProtocolVersion, TerminalPresentationId,
};

impl LocalTerminalService {
    /// Wakes `signal` when a published title changes.
    pub(crate) fn watch_titles(
        &self,
        signal: &std::sync::Arc<orchbee_server::SessionChangeSignal>,
    ) {
        if let Ok(workers) = self.workers.lock() {
            workers.terminal_titles().watch(signal);
        }
    }

    /// Starts reading the registered Worker's title changes. Only Session composition starts it.
    pub(crate) fn start_title_reads(&self) -> std::io::Result<()> {
        let titles = self
            .workers
            .lock()
            .map_err(|_| std::io::Error::other("Worker registry poisoned"))?
            .terminal_titles();
        titles.start_reads(std::sync::Arc::downgrade(&self.workers))
    }

    /// Stops the title reads and joins their thread within the Session owner's deadline.
    #[cfg(any(windows, test))]
    pub(crate) fn stop_title_reads_before(
        &self,
        deadline: std::time::Instant,
    ) -> std::io::Result<()> {
        let titles = self
            .workers
            .lock()
            .map_err(|_| std::io::Error::other("Worker registry poisoned"))?
            .terminal_titles();
        titles.stop_reads_before(deadline)
    }

    pub(super) fn read_presentation_titles(
        &self,
        request: &PanelAgentRequest,
        presentation_ids: &[TerminalPresentationId],
    ) -> PanelAgentResult {
        let outcome = self
            .presentation_titles(request, presentation_ids)
            .unwrap_or_else(|reason| PanelAgentOutcome::Failed { reason });
        PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            outcome,
        }
    }

    fn presentation_titles(
        &self,
        request: &PanelAgentRequest,
        presentation_ids: &[TerminalPresentationId],
    ) -> Result<PanelAgentOutcome, PanelAgentFailure> {
        // The Worker registry is released before the coordinator is taken; the title registry is a
        // leaf lock.
        let titles = self
            .workers
            .lock()
            .map_err(|_| PanelAgentFailure::Unavailable)?
            .terminal_titles();
        let coordinator = self
            .coordinator
            .lock()
            .map_err(|_| PanelAgentFailure::Unavailable)?;
        let targets =
            coordinator.presentation_title_targets(&request.session_id, presentation_ids)?;
        let presentations = presentation_ids
            .iter()
            .zip(targets)
            .map(|(presentation_id, target)| PresentationTitleRow {
                presentation_id: presentation_id.clone(),
                title: target.physical.filter(|_| target.binding.is_some()).map_or(
                    PresentationTitle::Unavailable {},
                    |(generation, identity)| titles.title(generation, identity),
                ),
                binding: target.binding,
            })
            .collect();
        Ok(PanelAgentOutcome::PresentationTitles { presentations })
    }
}

#[cfg(test)]
mod tests;
