use super::{LocalTerminalService, placement, viewer_session::ViewerAttachment};
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

pub(super) struct ControllerLease<'service> {
    service: &'service LocalTerminalService,
    handle: WorkerRequestHandle,
    target: ControlTarget,
    cleanup: WorkerRequest,
    armed: bool,
    sequence: u64,
}
impl Drop for ControllerLease<'_> {
    fn drop(&mut self) {
        let settled = !self.armed
            || matches!(self.handle.cleanup_control(self.cleanup.clone()), Ok(WorkerOperationResult::Control(result)) if result.outcome==WorkerOutcome::Released);
        if (settled || !self.handle.is_active())
            && let Ok(mut owners) = self.service.controllers.lock()
        {
            remove_exact(&mut owners, self.target, self.cleanup.controller_id);
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
    if !service.viewer_worker_is_current(&viewer.handle) {
        viewer.controller.take();
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    }
    if matches!(request.action(), Action::Acquire {}) {
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
        } => (controller_id, *sequence),
        Action::Acquire {} => unreachable!("acquisition handled"),
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
    let Some(request_id) = service.next_viewer_operation() else {
        return unavailable(Unavailable::ServerBusy);
    };
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
        Action::Acquire {} => unreachable!("effect matched"),
    };
    controller.sequence = sequence.get();
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
    let (Some(controller_id), Some(request_id), Some(cleanup_id)) = (
        service.next_viewer_operation(),
        service.next_viewer_operation(),
        service.next_viewer_operation(),
    ) else {
        return unavailable(Unavailable::ServerBusy);
    };
    let Some(template) = viewer.cleanup else {
        return rejected(Rejected::ViewerUnavailable);
    };
    // Generation selection and reservation are one short critical section, never a transport call.
    {
        let Ok(workers) = service.workers.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        if !viewer.handle.is_active()
            || placement(workers.request_handle().as_ref()) != placement(Some(&viewer.handle))
        {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        let Ok(mut owners) = service.controllers.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        owners.retain(|target, _| {
            target.worker == viewer.target.worker && target.generation == viewer.target.generation
        });
        if !reserve(&mut owners, viewer.target, controller_id) {
            return rejected(Rejected::ControllerBusy);
        }
    }
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
                viewer.controller = Some(lease);
                Outcome::Acquired { controller_id }
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
        ControlRejection::ViewerUnavailable
        | ControlRejection::WrongWorker
        | ControlRejection::StaleWorkerGeneration => Rejected::ViewerUnavailable,
    }
}

#[cfg(test)]
mod tests;
