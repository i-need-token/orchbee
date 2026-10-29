//! Exact source membership and authoritative launch directories for configured commands.
use super::SessionCoordinator;
use crate::{session_coordinator::ProcessObservationTarget, terminal_coordinator::WorkerPlacement};
use orchbee_workbench_protocol::{
    ClientPopupSource, ConfiguredCommandFailure as Failure, Snapshot, WorkbenchRevision,
};

impl SessionCoordinator {
    pub(crate) fn configured_command_revision(&self) -> Result<WorkbenchRevision, Failure> {
        Ok(self
            .workbench
            .as_ref()
            .ok_or(Failure::Unavailable)?
            .snapshot()
            .revision)
    }
    pub(crate) fn prepare_configured_popup(
        &mut self,
        request: orchbee_client_server_protocol::TerminalCreateRequest,
        source: &ClientPopupSource,
        expected: WorkbenchRevision,
        worker: Option<WorkerPlacement>,
        expiry: Option<orchbee_capability_node_protocol::LeaseExpiresAtUnixMilliseconds>,
        capture: &crate::terminal_coordinator::LaunchCapture,
    ) -> Result<
        (
            crate::terminal_coordinator::PreparedOperation,
            Option<orchbee_capability_node_protocol::TerminalOperationAuthorization>,
        ),
        Failure,
    > {
        self.configured_command_source(source, expected, worker)?;
        let mut plan = match self.terminals.plan_create(request, worker, expiry, capture) {
            Ok(plan) => plan,
            Err(result) => {
                return Ok((
                    crate::terminal_coordinator::PreparedOperation::Ready(result),
                    None,
                ));
            }
        };
        let authorization = plan.mark_popup();
        Ok((
            crate::terminal_coordinator::PreparedOperation::Dispatch(plan.commit()),
            Some(authorization),
        ))
    }
    pub(crate) fn configured_command_snapshot(
        &self,
        expected: WorkbenchRevision,
    ) -> Result<Snapshot, Failure> {
        let workbench = self.workbench.as_ref().ok_or(Failure::Unavailable)?;
        if workbench.snapshot().revision != expected {
            return Err(Failure::StaleSource);
        }
        Ok(workbench.snapshot().clone())
    }
    pub(crate) fn configured_command_source(
        &mut self,
        source: &ClientPopupSource,
        expected: WorkbenchRevision,
        worker: Option<WorkerPlacement>,
    ) -> Result<ProcessObservationTarget, Failure> {
        let snapshot = self.configured_command_snapshot(expected)?;
        if !self
            .workbench
            .as_ref()
            .is_some_and(|workbench| workbench.popup_source_matches(source))
        {
            return Err(Failure::StaleSource);
        }
        let binding = source.member.binding().ok_or(Failure::StaleSource)?;
        let target = self
            .prepare_binding_viewer(
                &orchbee_client_server_protocol::SessionId::parse(snapshot.session_id.as_str())
                    .map_err(|_| Failure::InvalidIntent)?,
                &orchbee_client_server_protocol::WorkerId::parse(binding.worker_id.as_str())
                    .map_err(|_| Failure::InvalidIntent)?,
                &orchbee_client_server_protocol::TerminalId::parse(binding.terminal_id.as_str())
                    .map_err(|_| Failure::InvalidIntent)?,
                worker,
            )
            .map_err(|_| Failure::StaleSource)?
            .target;
        Ok(ProcessObservationTarget {
            session_id: snapshot.session_id,
            binding: binding.clone(),
            target,
        })
    }
}
