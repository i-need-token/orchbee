//! Independent popup reservations do not change the canonical Workbench.
use super::SiteCoordinator;
use crate::terminal_coordinator::{BindingViewerTarget, PreparedOperation, WorkerPlacement};
use orchbee_capability_node_protocol as node;
use orchbee_client_server_protocol as client;
use orchbee_workbench_protocol::{
    PluginFailure as Failure, PluginPopupSpec, WorkbenchRequestId, WorkbenchTerminalGeometry,
};
use std::num::NonZeroU64;

impl SiteCoordinator {
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
        let prepared = self.prepare(
            client::ClientMessage::TerminalCreate(request),
            worker,
            expiry,
        );
        let authorization = if matches!(prepared, PreparedOperation::Dispatch(_)) {
            Some(
                self.terminals
                    .mark_popup(&reference)
                    .ok_or(Failure::Unavailable)?,
            )
        } else {
            None
        };
        Ok((prepared, title, authorization))
    }
    pub(crate) fn popup_definitively_uncreated(
        &self,
        authorization: node::TerminalOperationAuthorization,
    ) -> bool {
        self.terminals
            .is_definitively_uncreated(authorization.terminal_id(), authorization.worker_id())
    }
    pub(crate) fn fence_plugin_popup(
        &mut self,
        reference: &client::RequestId,
        authorization: node::TerminalOperationAuthorization,
    ) -> bool {
        self.terminals.fence_popup(reference, authorization)
    }
    pub(crate) fn release_plugin_popup(
        &mut self,
        reference: &client::RequestId,
        authorization: node::TerminalOperationAuthorization,
    ) -> bool {
        self.terminals.release_popup(reference, authorization)
    }

    pub(crate) fn popup_lifecycle_request(
        &mut self,
        popup_id: &WorkbenchRequestId,
        authorization: node::TerminalOperationAuthorization,
        target: BindingViewerTarget,
        ordinal: u8,
        action: node::TerminalLifecycleAction,
        expiry: node::LeaseExpiresAtUnixMilliseconds,
    ) -> Option<node::TerminalLifecycleRequest> {
        if authorization.terminal_id() != target.terminal_id
            || authorization.worker_id() != target.worker.worker_id
            || authorization.worker_generation() != target.worker.generation
        {
            return None;
        }
        let request_id = node::OperationId::from_bytes(self.terminals.allocate_identity()?).ok()?;
        let lease_id = node::LeaseId::from_bytes(self.terminals.allocate_identity()?).ok()?;
        Some(node::TerminalLifecycleRequest {
            authorization: node::TerminalLifecycleAuthorization {
                capability_version: node::TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
                terminal_id: target.terminal_id,
                worker_id: target.worker.worker_id,
                worker_generation: target.worker.generation,
                binding_fence: authorization.fence(),
                lease: node::TerminalOperationLease::new(lease_id, expiry),
            },
            close_operation_id: node::OperationId::from_bytes(
                u128::from_str_radix(popup_id.as_str(), 16)
                    .ok()?
                    .to_be_bytes(),
            )
            .ok()?,
            request_id,
            attempt_generation: NonZeroU64::MIN,
            step_ordinal: node::LifecycleStepOrdinal::new(ordinal).ok()?,
            target: target.identity,
            action,
        })
    }
}
