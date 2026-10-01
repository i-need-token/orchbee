//! Independent physical lifecycle does not imply plugin or canonical Pane membership.
use super::SiteCoordinator;
use crate::terminal_coordinator::BindingViewerTarget;
use orchbee_capability_node_protocol as node;
use orchbee_client_server_protocol as client;
use orchbee_workbench_protocol::WorkbenchRequestId;
use std::num::NonZeroU64;
impl SiteCoordinator {
    pub(crate) fn popup_definitively_uncreated(
        &self,
        authorization: node::TerminalOperationAuthorization,
    ) -> bool {
        self.terminals
            .is_definitively_uncreated(authorization.terminal_id(), authorization.worker_id())
    }
    pub(crate) fn fence_independent_popup(
        &mut self,
        reference: &client::RequestId,
        authorization: node::TerminalOperationAuthorization,
    ) -> bool {
        self.terminals.fence_popup(reference, authorization)
    }
    pub(crate) fn release_independent_popup(
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
