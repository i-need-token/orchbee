use super::{LocalTerminalService, placement, viewer_session::ViewerAttachment};
use crate::site_coordinator::{SiteCoordinator, TargetLifecycle};
use crate::terminal_coordinator::BindingViewerTarget;
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
};
use orchbee_capability_node_protocol::{
    ControlRejection, OperationId, TerminalControlAction as WorkerAction,
    TerminalControlOutcome as WorkerOutcome, TerminalControlRequest as WorkerRequest,
};
use orchbee_client_server_protocol::{
    ControlRejectedReason as Rejected, ControlUnavailableReason as Unavailable, ControllerId,
    TerminalControlAction as Action, TerminalControlOutcome as Outcome, TerminalControlRequest,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct ControlTarget {
    pub(super) worker: [u8; 16],
    pub(super) generation: u64,
    pub(super) host_epoch: [u8; 16],
    pub(super) terminal: [u8; 16],
}

impl From<BindingViewerTarget> for ControlTarget {
    fn from(target: BindingViewerTarget) -> Self {
        Self {
            worker: target.worker.worker_id.as_bytes(),
            generation: target.worker.generation.get(),
            host_epoch: target.identity.host_epoch().as_bytes(),
            terminal: target.identity.terminal_id().as_bytes(),
        }
    }
}

pub(super) struct ControllerLease<'service> {
    service: &'service LocalTerminalService,
    handle: WorkerRequestHandle,
    target: BindingViewerTarget,
    cleanup: WorkerRequest,
    armed: bool,
    sequence: u64,
}

impl ControllerLease<'_> {
    pub(super) fn disarm_known_disposed(&mut self) {
        self.armed = false;
    }
}

impl Drop for ControllerLease<'_> {
    fn drop(&mut self) {
        let target = ControlTarget::from(self.target);
        let revoked = self
            .service
            .controllers
            .lock()
            .is_ok_and(|owners| owners.get(&target) != Some(&self.cleanup.controller_id));
        if revoked {
            return;
        }
        let settled = !self.armed
            || self.service.target_lifecycle(self.target) == Some(TargetLifecycle::Disposed)
            || matches!(self.handle.cleanup_control(self.cleanup.clone()), Ok(WorkerOperationResult::Control(result)) if result.outcome==WorkerOutcome::Released);
        if (settled || !self.handle.is_active())
            && let Ok(mut owners) = self.service.controllers.lock()
        {
            remove_exact(&mut owners, target, self.cleanup.controller_id);
        }
    }
}

pub(super) fn execute<'service>(
    service: &'service LocalTerminalService,
    viewer: Option<&mut ViewerAttachment<'service>>,
    request: &TerminalControlRequest,
) -> Outcome {
    let Some(viewer) = viewer else {
        return rejected(Rejected::ViewerUnavailable);
    };
    if &viewer.attachment_id != request.action().attachment_id() {
        return rejected(Rejected::ViewerUnavailable);
    }
    if !service.viewer_worker_is_current(&viewer.handle) {
        viewer.controller.take();
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    }
    if matches!(request.action(), Action::Acquire { .. }) {
        return acquire(service, viewer);
    }
    let Some(controller) = viewer.controller.as_mut() else {
        return rejected(Rejected::NotController);
    };
    let (id, sequence) = match request.action() {
        Action::Input {
            controller_id,
            sequence,
            ..
        }
        | Action::Resize {
            controller_id,
            sequence,
            ..
        }
        | Action::Paste {
            controller_id,
            sequence,
            ..
        } => (controller_id, *sequence),
        Action::Acquire { .. } => unreachable!("acquisition handled"),
    };
    if id.as_str()
        != format!(
            "{:032x}",
            u128::from_be_bytes(controller.cleanup.controller_id.as_bytes())
        )
    {
        return rejected(Rejected::NotController);
    }
    if sequence.get() <= controller.sequence {
        return rejected(Rejected::SequenceMismatch);
    }
    let action = match request.action() {
        Action::Input { bytes, .. } => {
            match orchbee_capability_node_protocol::ControlInput::new(bytes.as_bytes().to_vec()) {
                Ok(bytes) => WorkerAction::Input { sequence, bytes },
                Err(_) => return rejected(Rejected::HostRejected),
            }
        }
        Action::Resize { geometry, .. } => {
            match orchbee_capability_node_protocol::TerminalPixelGeometry::new(
                geometry.columns(),
                geometry.rows(),
                geometry.cell_pixel_width(),
                geometry.cell_pixel_height(),
            ) {
                Ok(geometry) => WorkerAction::Resize { sequence, geometry },
                Err(_) => return rejected(Rejected::HostRejected),
            }
        }
        Action::Paste { content, .. } => {
            match orchbee_capability_node_protocol::PasteContent::new(content.as_bytes().to_vec()) {
                Ok(content) => WorkerAction::Paste { sequence, content },
                Err(_) => return rejected(Rejected::HostRejected),
            }
        }
        Action::Acquire { .. } => unreachable!("effect matched"),
    };
    let request_id = {
        let Ok(workers) = service.workers.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        if !controller.handle.is_active()
            || placement(workers.request_handle().as_ref()) != placement(Some(&controller.handle))
        {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        let Ok(mut coordinator) = service.coordinator.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        if coordinator.target_lifecycle(controller.target) != TargetLifecycle::Unfenced {
            return rejected(Rejected::ViewerUnavailable);
        }
        let Ok(owners) = service.controllers.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        if owners.get(&ControlTarget::from(controller.target))
            != Some(&controller.cleanup.controller_id)
        {
            return rejected(Rejected::NotController);
        }
        let Some(request_id) = coordinator
            .allocate_identity()
            .and_then(|bytes| OperationId::from_bytes(bytes).ok())
        else {
            return unavailable(Unavailable::ServerBusy);
        };
        controller.sequence = sequence.get();
        request_id
    };
    let command = WorkerRequest {
        request_id,
        action,
        ..controller.cleanup.clone()
    };
    match controller
        .handle
        .request(WorkerOperationRequest::Control(command))
    {
        Ok(WorkerOperationResult::Control(result)) => match result.outcome {
            WorkerOutcome::InputAccepted => Outcome::InputAccepted {},
            WorkerOutcome::Resized(geometry) => {
                match orchbee_client_server_protocol::Geometry::new(
                    geometry.columns(),
                    geometry.rows(),
                    geometry.cell_pixel_width(),
                    geometry.cell_pixel_height(),
                ) {
                    Ok(geometry) => Outcome::Resized { geometry },
                    Err(_) => {
                        viewer.controller.take();
                        Outcome::OutcomeUnknown {}
                    }
                }
            }
            WorkerOutcome::Rejected(reason) => rejected(worker_rejection(reason)),
            WorkerOutcome::Pasted { accepted_bytes } => {
                match orchbee_client_server_protocol::PasteAcceptedBytes::new(accepted_bytes.get())
                {
                    Ok(accepted_bytes) => Outcome::Pasted { accepted_bytes },
                    Err(_) => {
                        viewer.controller.take();
                        Outcome::OutcomeUnknown {}
                    }
                }
            }
            _ => {
                viewer.controller.take();
                Outcome::OutcomeUnknown {}
            }
        },
        Err(WorkerRequestError::Backpressure) => unavailable(Unavailable::ServerBusy),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => {
            viewer.controller.take();
            Outcome::OutcomeUnknown {}
        }
        _ => {
            viewer.controller.take();
            unavailable(Unavailable::TerminalRuntimeUnavailable)
        }
    }
}
fn acquire<'service>(
    service: &'service LocalTerminalService,
    viewer: &mut ViewerAttachment<'service>,
) -> Outcome {
    if viewer.controller.is_some() {
        return rejected(Rejected::ControllerBusy);
    }
    let Some(template) = viewer.cleanup else {
        return rejected(Rejected::ViewerUnavailable);
    };
    // Generation selection and reservation are one short critical section, never a transport call.
    let (controller_id, request_id, cleanup_id) = {
        let Ok(workers) = service.workers.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        if !viewer.handle.is_active()
            || placement(workers.request_handle().as_ref()) != placement(Some(&viewer.handle))
        {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        let Ok(mut coordinator) = service.coordinator.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        if coordinator.target_lifecycle(viewer.target) != TargetLifecycle::Unfenced {
            return rejected(Rejected::ViewerUnavailable);
        }
        let Ok(mut owners) = service.controllers.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        let mut next = || {
            coordinator
                .allocate_identity()
                .and_then(|bytes| OperationId::from_bytes(bytes).ok())
        };
        let (Some(controller_id), Some(request_id), Some(cleanup_id)) = (next(), next(), next())
        else {
            return unavailable(Unavailable::ServerBusy);
        };
        let selected = ControlTarget::from(viewer.target);
        owners.retain(|target, _| {
            target.worker == selected.worker && target.generation == selected.generation
        });
        if !reserve(&mut owners, selected, controller_id) {
            return rejected(Rejected::ControllerBusy);
        }
        (controller_id, request_id, cleanup_id)
    };
    let mut lease = ControllerLease {
        service,
        handle: viewer.handle.clone(),
        target: viewer.target,
        sequence: 0,
        armed: true,
        cleanup: WorkerRequest {
            request_id: cleanup_id,
            controller_id,
            viewer_id: template.attachment_id,
            worker_id: template.worker_id,
            worker_generation: template.worker_generation,
            action: WorkerAction::Release,
        },
    };
    let result = lease
        .handle
        .request(WorkerOperationRequest::Control(WorkerRequest {
            request_id,
            action: WorkerAction::Acquire,
            ..lease.cleanup.clone()
        }));
    match result {
        Ok(WorkerOperationResult::Control(result)) => match result.outcome {
            WorkerOutcome::Acquired => {
                let Ok(controller_id) = ControllerId::parse(&format!(
                    "{:032x}",
                    u128::from_be_bytes(controller_id.as_bytes())
                )) else {
                    return Outcome::OutcomeUnknown {};
                };
                let mut candidate = Some(lease);
                install_controller(service, viewer, &mut candidate, controller_id)
            }
            WorkerOutcome::Rejected(reason) => {
                lease.armed = false;
                rejected(worker_rejection(reason))
            }
            _ => Outcome::OutcomeUnknown {},
        },
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Outcome::OutcomeUnknown {},
        Err(WorkerRequestError::Backpressure) => {
            lease.armed = false;
            unavailable(Unavailable::ServerBusy)
        }
        _ => {
            lease.armed = false;
            unavailable(Unavailable::TerminalRuntimeUnavailable)
        }
    }
}

fn install_controller<'service>(
    service: &'service LocalTerminalService,
    viewer: &mut ViewerAttachment<'service>,
    candidate: &mut Option<ControllerLease<'service>>,
    controller_id: ControllerId,
) -> Outcome {
    let Ok(workers) = service.workers.lock() else {
        return unavailable(Unavailable::ServerBusy);
    };
    if !viewer.handle.is_active()
        || placement(workers.request_handle().as_ref()) != placement(Some(&viewer.handle))
    {
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    }
    let Ok(coordinator) = service.coordinator.lock() else {
        return unavailable(Unavailable::ServerBusy);
    };
    if coordinator.target_lifecycle(viewer.target) != TargetLifecycle::Unfenced {
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    }
    let Ok(owners) = service.controllers.lock() else {
        return unavailable(Unavailable::ServerBusy);
    };
    let Some(lease) = candidate.as_ref() else {
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    };
    if owners.get(&ControlTarget::from(lease.target)) != Some(&lease.cleanup.controller_id) {
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    }
    viewer.controller = candidate.take();
    Outcome::Acquired { controller_id }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TargetCleanupError {
    UnprovenTarget,
    ControllerRegistryUnavailable,
}

impl LocalTerminalService {
    pub(super) fn target_lifecycle(&self, target: BindingViewerTarget) -> Option<TargetLifecycle> {
        Some(self.coordinator.lock().ok()?.target_lifecycle(target))
    }

    /// The caller holds Site through this cleanup and the authority's cleanup marker.
    pub(super) fn complete_disposed_target_cleanup(
        &self,
        coordinator: &SiteCoordinator,
        close_id: &orchbee_workbench_protocol::WorkbenchRequestId,
        runtime: orchbee_server::WorkbenchTabCloseRuntimeTarget,
    ) -> Result<(), TargetCleanupError> {
        if !coordinator.is_disposed_close_target(close_id, runtime) {
            return Err(TargetCleanupError::UnprovenTarget);
        }
        let target = ControlTarget {
            worker: runtime.worker_id().as_bytes(),
            generation: runtime.worker_generation().get(),
            host_epoch: runtime.host_identity().host_epoch().as_bytes(),
            terminal: runtime.host_identity().terminal_id().as_bytes(),
        };
        let mut owners = self
            .controllers
            .lock()
            .map_err(|_| TargetCleanupError::ControllerRegistryUnavailable)?;
        if let Some(id) = owners.get(&target).copied() {
            remove_exact(&mut owners, target, id);
        }
        Ok(())
    }
}
fn reserve(
    owners: &mut BTreeMap<ControlTarget, OperationId>,
    target: ControlTarget,
    id: OperationId,
) -> bool {
    if owners.len() >= 8 || owners.contains_key(&target) {
        return false;
    }
    owners.insert(target, id);
    true
}
fn remove_exact(
    owners: &mut BTreeMap<ControlTarget, OperationId>,
    target: ControlTarget,
    id: OperationId,
) {
    if owners.get(&target) == Some(&id) {
        owners.remove(&target);
    }
}
fn rejected(reason: Rejected) -> Outcome {
    Outcome::Rejected { reason }
}
fn unavailable(reason: Unavailable) -> Outcome {
    Outcome::Unavailable { reason }
}
fn worker_rejection(reason: ControlRejection) -> Rejected {
    match reason {
        ControlRejection::ControllerBusy => Rejected::ControllerBusy,
        ControlRejection::NotController => Rejected::NotController,
        ControlRejection::SequenceMismatch => Rejected::SequenceMismatch,
        ControlRejection::HostRejected => Rejected::HostRejected,
        ControlRejection::HostInputInvalid => Rejected::HostInputInvalid,
        ControlRejection::HostInputLimitExceeded => Rejected::HostInputLimitExceeded,
        ControlRejection::HostInputBackpressure => Rejected::HostInputBackpressure,
        ControlRejection::HostInputUnavailable => Rejected::HostInputUnavailable,
        ControlRejection::ViewerUnavailable
        | ControlRejection::WrongWorker
        | ControlRejection::StaleWorkerGeneration => Rejected::ViewerUnavailable,
    }
}

#[cfg(test)]
mod tests;
