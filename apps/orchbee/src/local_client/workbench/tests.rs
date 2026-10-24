//! No-deadline entry points for bounded local-client transport fixtures.
use super::*;
impl LocalClient {
    pub(crate) fn submit_workbench_snapshot_read(
        &mut self,
        request: SnapshotReadIntent,
    ) -> Result<WorkbenchSnapshotResult, SnapshotReadError> {
        self.submit_workbench_snapshot_read_with_deadline(request, None, None)
    }

    pub(crate) fn submit_workbench_publication_read(
        &mut self,
        request: PublicationReadIntent,
    ) -> Result<WorkbenchPublicationResult, PublicationReadError> {
        self.submit_workbench_publication_read_with_deadline(request, None)
    }
}
