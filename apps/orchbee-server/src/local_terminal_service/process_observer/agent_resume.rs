//! A restored terminal's resume evidence read on demand through the sampler thread, which keeps
//! every temporary viewer serialized with the periodic samples and the one retained cleanup.
use super::{LocalTerminalService, ProcessObservationTarget, Wake, on_demand};
use crate::local_terminal_service::agent_resume::ResumeFacts;
use std::time::Duration;
/// One read attaches, reads a frame and a sample, and detaches; a read still waiting after this
/// gets nothing.
const READ_WINDOW: Duration = Duration::from_secs(2);

pub(super) type Requests = on_demand::Requests<ProcessObservationTarget, ResumeFacts>;

impl LocalTerminalService {
    /// What `target` shows now. Nothing when the read is refused, late or its viewer's cleanup is
    /// uncertain.
    pub(in crate::local_terminal_service) fn request_resume_facts(
        &self,
        target: ProcessObservationTarget,
    ) -> Option<ResumeFacts> {
        self.request_on_demand(|wake| &wake.resumes, target, READ_WINDOW)
            .ok()
    }
}
pub(super) fn serve(service: &LocalTerminalService, wake: &Wake) {
    on_demand::serve(service, wake, &wake.resumes, |job| {
        service.sample_resume_facts(&job.parameters, &job.cancelled)
    });
}
