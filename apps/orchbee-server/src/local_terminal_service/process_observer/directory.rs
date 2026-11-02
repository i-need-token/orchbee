//! A terminal's current directory read on demand through the sampler thread.
use super::{LocalTerminalService, ProcessObservationTarget, Wake, on_demand};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;
use std::time::Duration;
/// One sample attaches, reads and detaches a viewer; a read still waiting after this gets nothing.
const READ_WINDOW: Duration = Duration::from_secs(2);

pub(super) type Requests = on_demand::Requests<(), WorkbenchDirectoryPath>;

impl LocalTerminalService {
    /// The directory `target` is in now: the one its shell reported, else its root process's.
    /// Nothing when the sample is refused, late or unavailable on this platform.
    pub(in crate::local_terminal_service) fn request_current_directory(
        &self,
        target: ProcessObservationTarget,
    ) -> Option<WorkbenchDirectoryPath> {
        self.request_on_demand(|wake| &wake.directories, target, (), READ_WINDOW)
            .ok()
    }
}
pub(super) fn serve(service: &LocalTerminalService, wake: &Wake) {
    on_demand::serve(service, wake, &wake.directories, |job| {
        service.sample_current_directory(&job.target, &job.cancelled)
    });
}
