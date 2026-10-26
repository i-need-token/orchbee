//! Independent popup reservations do not change the canonical Workbench.
use super::SessionCoordinator;
use crate::terminal_coordinator::{PreparedOperation, WorkerPlacement};
use orchbee_capability_node_protocol as node;
use orchbee_client_server_protocol as client;
use orchbee_workbench_protocol::{
    PluginFailure as Failure, PluginPopupSpec, WorkbenchRequestId, WorkbenchTerminalGeometry,
};

impl SessionCoordinator {
    pub(crate) fn prepare_plugin_popup(
        &mut self,
        popup_id: &WorkbenchRequestId,
        spec: &PluginPopupSpec,
        geometry: WorkbenchTerminalGeometry,
        worker: Option<WorkerPlacement>,
        expiry: Option<node::LeaseExpiresAtUnixMilliseconds>,
    ) -> Result<
        (
            PreparedOperation,
            String,
            Option<node::TerminalOperationAuthorization>,
        ),
        Failure,
    > {
        let (frozen, title) = self
            .workbench
            .as_ref()
            .ok_or(Failure::Unavailable)?
            .resolve_plugin_popup(popup_id, spec)
            .map_err(|_| Failure::InvalidIntent)?;
        let reference =
            client::RequestId::parse(popup_id.as_str()).map_err(|_| Failure::InvalidIntent)?;
        let cwd = spec
            .cwd
            .as_ref()
            .map(|path| client::WorkingDirectory::parse(path.as_str()))
            .transpose()
            .map_err(|_| Failure::InvalidIntent)?
            .or(Some(frozen.directory));
        let environment = client::LaunchEnvironment::new(spec.environment.entries().to_vec())
            .map_err(|_| Failure::InvalidIntent)?;
        let geometry = client::Geometry::new(
            geometry.columns(),
            geometry.rows(),
            geometry.cell_pixel_width(),
            geometry.cell_pixel_height(),
        )
        .map_err(|_| Failure::InvalidIntent)?;
        let request = client::TerminalCreateRequest::new(
            reference.clone(),
            None,
            cwd,
            client::StartMode::Immediate { geometry },
            Some(frozen.launch),
        )
        .with_environment(environment)
        .with_plugin_environment(Some(frozen.environment))
        .map_err(|_| Failure::InvalidIntent)?;
        let mut plan = match self.terminals.plan_create(request, worker, expiry) {
            Ok(plan) => plan,
            Err(result) => return Ok((PreparedOperation::Ready(result), title, None)),
        };
        let authorization = plan.mark_popup();
        Ok((
            PreparedOperation::Dispatch(plan.commit()),
            title,
            Some(authorization),
        ))
    }
}
