//! Unix clipboard writes are unsupported; every admitted job receives an explicit terminal result.
use super::super::terminal_effects::{Job, Outcome};

#[derive(Default)]
pub(in crate::workbench_client) struct Writer;

impl Writer {
    pub(in crate::workbench_client) fn new() -> Self {
        Self
    }

    pub(in crate::workbench_client) fn submit(&mut self, job: Job) {
        job.dispatch_clipboard(|_text| Outcome::Unsupported);
    }

    pub(in crate::workbench_client) fn close(&mut self) {}
    pub(in crate::workbench_client) fn request_stop(&mut self) {}
    pub(in crate::workbench_client) fn stop_before(
        &mut self,
        _deadline: std::time::Instant,
    ) -> std::io::Result<()> {
        Ok(())
    }
}
