use super::{SiteCoordinator, StandaloneTerminal, presentation_binding};
use crate::terminal_coordinator::{PendingOperation, WorkerPlacement};
use orchbee_capability_node_protocol::LeaseExpiresAtUnixMilliseconds;
use orchbee_client_server_protocol::{
    Geometry, RequestId, StartMode, TerminalCreateRequest, WorkerConstraint, WorkerId,
    WorkingDirectory,
};
use orchbee_server::WorkbenchRestoreTarget;
use std::io;

impl SiteCoordinator {
    /// Only used while this entire coordinator is unpublished; no launch IPC is performed here.
    pub(crate) fn prepare_checkpoint_terminal(
        &mut self,
        target: &WorkbenchRestoreTarget,
        reference: RequestId,
        worker: WorkerPlacement,
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> io::Result<Box<PendingOperation>> {
        let workbench = self
            .workbench
            .as_mut()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?;
        let worker_id = WorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(worker.worker_id.as_bytes())
        ))
        .map_err(io::Error::other)?;
        let cwd = target
            .working_directory()
            .map(|path| WorkingDirectory::parse(path.as_str()))
            .transpose()
            .map_err(io::Error::other)?;
        let terminal = self
            .terminals
            .plan_create(
                TerminalCreateRequest::new(
                    reference.clone(),
                    Some(WorkerConstraint::Exact { worker_id }),
                    cwd,
                    StartMode::Immediate {
                        geometry: Geometry::new(80, 24, 8, 16).map_err(io::Error::other)?,
                    },
                    None,
                ),
                Some(worker),
                Some(expiry),
            )
            .map_err(|_| io::Error::other("restored terminal reservation unavailable"))?;
        let binding = presentation_binding(terminal.authorization())
            .ok_or_else(|| io::Error::other("restored terminal binding unavailable"))?;
        let rebind = workbench
            .prepare_restored_terminal(target, binding)
            .map_err(io::Error::other)?;
        let pending = terminal.commit();
        let _revision = rebind.commit();
        if target.is_standalone() && matches!(self.standalone, StandaloneTerminal::Empty) {
            self.standalone = StandaloneTerminal::Creating(reference);
        }
        Ok(pending)
    }
}
