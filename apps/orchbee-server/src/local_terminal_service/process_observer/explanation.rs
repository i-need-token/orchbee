//! Agent screen explanations read on demand through the sampler thread.
use super::{LocalTerminalService, ProcessObservationTarget, ScreenDetector, Wake, on_demand};
use crate::local_terminal_service::viewer_connection::CapturedAgentExplanation;
use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_workbench_protocol::PanelAgentFailure as Failure;
use std::time::Duration;
const READ_WINDOW: Duration = Duration::from_secs(4);

pub(super) type Requests =
    on_demand::Requests<(ProcessObservationTarget, AgentReportProvider), CapturedAgentExplanation>;

impl LocalTerminalService {
    pub(in crate::local_terminal_service) fn request_agent_explanation(
        &self,
        target: ProcessObservationTarget,
        provider: AgentReportProvider,
    ) -> Result<CapturedAgentExplanation, Failure> {
        self.request_on_demand(|wake| &wake.explanations, (target, provider), READ_WINDOW)
            .map_err(|refusal| match refusal {
                on_demand::Refusal::Busy => Failure::ResourceLimit,
                on_demand::Refusal::Unavailable => Failure::Unavailable,
            })
    }
}
pub(super) fn serve(service: &LocalTerminalService, wake: &Wake, detector: &ScreenDetector) {
    on_demand::serve(service, wake, &wake.explanations, |job| {
        let (target, provider) = &job.parameters;
        service.sample_agent_explanation(target, &job.cancelled, detector, *provider)
    });
}
