//! Explicit Agent reports resolve retained generations while holding the Site commit gate.
use super::LocalTerminalService;
use orchbee_workbench_protocol::{
    PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest, PanelAgentResult, ProtocolVersion,
};
#[cfg(windows)]
mod explanation;
#[cfg(windows)]
mod titles;
impl LocalTerminalService {
    pub(crate) fn execute_panel_agent(&self, request: PanelAgentRequest) -> PanelAgentResult {
        #[cfg(windows)]
        if matches!(
            request.action,
            orchbee_workbench_protocol::PanelAgentAction::Explain { .. }
        ) {
            return self.explain_panel_agent(&request);
        }
        #[cfg(windows)]
        if matches!(
            request.action,
            orchbee_workbench_protocol::PanelAgentAction::Titles { .. }
        ) {
            return self.read_panel_titles(&request);
        }
        let unavailable = PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: PanelAgentOutcome::Failed {
                reason: PanelAgentFailure::Unavailable,
            },
        };
        #[cfg(windows)]
        {
            if matches!(
                &request.action,
                orchbee_workbench_protocol::PanelAgentAction::Observe { .. }
                    | orchbee_workbench_protocol::PanelAgentAction::States { .. }
            ) {
                let Ok(workers) = self.workers.lock() else {
                    return unavailable;
                };
                let handle = workers.request_handle();
                let Some(worker) = super::placement(handle.as_ref().filter(|h| h.is_active()))
                else {
                    return unavailable;
                };
                let Ok(mut coordinator) = self.coordinator.lock() else {
                    return unavailable;
                };
                let Ok(facts) = coordinator.agent_registration_snapshot() else {
                    return unavailable;
                };
                let valid = match &request.action {
                    orchbee_workbench_protocol::PanelAgentAction::Observe { binding, .. } => {
                        facts.iter().any(|fact| {
                            fact.presentation_id().as_str() == binding.presentation_id.as_str()
                                && fact.worker_id() == worker.worker_id
                                && fact.worker_generation() == worker.generation
                        })
                    }
                    orchbee_workbench_protocol::PanelAgentAction::States { presentation_ids } => {
                        facts
                            .iter()
                            .filter(|fact| {
                                presentation_ids
                                    .iter()
                                    .any(|id| id.as_str() == fact.presentation_id().as_str())
                            })
                            .all(|fact| {
                                fact.worker_id() == worker.worker_id
                                    && fact.worker_generation() == worker.generation
                            })
                    }
                    orchbee_workbench_protocol::PanelAgentAction::Titles { .. }
                    | orchbee_workbench_protocol::PanelAgentAction::ReadPresentations { .. }
                    | orchbee_workbench_protocol::PanelAgentAction::Explain { .. }
                    | orchbee_workbench_protocol::PanelAgentAction::Read { .. }
                    | orchbee_workbench_protocol::PanelAgentAction::ReportPresentation { .. }
                    | orchbee_workbench_protocol::PanelAgentAction::Report { .. } => false,
                };
                if !valid {
                    return unavailable;
                }
                return coordinator
                    .panel_agent(request, std::time::Instant::now())
                    .unwrap_or(unavailable);
            }
            self.coordinator
                .lock()
                .ok()
                .and_then(|mut coordinator| {
                    coordinator.panel_agent(request, std::time::Instant::now())
                })
                .unwrap_or(unavailable)
        }
        #[cfg(not(windows))]
        {
            let _ = request;
            unavailable
        }
    }
}

#[cfg(windows)]
impl LocalTerminalService {
    pub(super) fn observe_host_process(
        &self,
        handle: &crate::worker_obcn_carrier::WorkerRequestHandle,
        target: crate::terminal_coordinator::BindingViewerTarget,
        sample: &orchbee_terminal_protocol::TerminalProcessInfo,
        screen: Option<crate::agent_screen::Detection>,
        directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
        started: std::time::Instant,
    ) {
        let Ok(workers) = self.workers.lock() else {
            return;
        };
        if !handle.is_active()
            || super::placement(workers.request_handle().as_ref()) != Some(target.worker)
            || super::placement(Some(handle)) != Some(target.worker)
        {
            return;
        }
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return;
        };
        coordinator.observe_host_screen_with_directory(
            target,
            sample,
            screen,
            directory,
            started,
            std::time::Instant::now(),
        );
    }
}
