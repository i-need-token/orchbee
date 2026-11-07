//! Owns exact viewer-bound controller reservations and cleanup leases while translating
//! admitted Client controls to the current Worker. It does not transfer authority across
//! physical incarnations or infer cleanup from transport loss.

use super::{LocalTerminalService, placement, viewer_connection::ViewerAttachment};
use crate::session_coordinator::{SessionCoordinator, TargetLifecycle};
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
mod screen_control;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReservationPhase {
    Pending,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ControllerReservation {
    pub(super) controller_id: OperationId,
    pub(super) viewer_id: OperationId,
    pub(super) phase: ReservationPhase,
}

#[cfg(test)]
impl ControllerReservation {
    pub(super) const fn active_for_test(
        controller_id: OperationId,
        viewer_id: OperationId,
    ) -> Self {
        Self {
            controller_id,
            viewer_id,
            phase: ReservationPhase::Active,
        }
    }
}

pub(super) struct ControllerLease<'service> {
    service: &'service LocalTerminalService,
    handle: WorkerRequestHandle,
    target: BindingViewerTarget,
    cleanup: WorkerRequest,
    armed: bool,
    superseded: Option<WorkerRequest>,
    sequence: u64,
    geometry: Option<orchbee_client_server_protocol::Geometry>,
}

impl ControllerLease<'_> {
    pub(super) const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub(super) const fn geometry(&self) -> Option<orchbee_client_server_protocol::Geometry> {
        self.geometry
    }
    pub(super) fn disarm_known_disposed(&mut self) {
        self.armed = false;
        self.superseded = None;
    }
}

impl Drop for ControllerLease<'_> {
    fn drop(&mut self) {
        let target = ControlTarget::from(self.target);
        let revoked = self.service.controllers.lock().is_ok_and(|owners| {
            owners.get(&target).map(|owner| owner.controller_id) != Some(self.cleanup.controller_id)
        });
        if revoked {
            return;
        }
        let disposed =
            self.service.target_lifecycle(self.target) == Some(TargetLifecycle::Disposed);
        let previous_settled = disposed || self.superseded.as_ref().is_none_or(|request| {
            matches!(self.handle.cleanup_control(request.clone()), Ok(WorkerOperationResult::Control(result)) if result.outcome == WorkerOutcome::Released)
        });
        // Attempt both cleanups even when the previous grant has no receipt.
        let current_settled = disposed
            || !self.armed
            || matches!(self.handle.cleanup_control(self.cleanup.clone()), Ok(WorkerOperationResult::Control(result)) if result.outcome == WorkerOutcome::Released);
        let settled = previous_settled && current_settled;
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
    match request.action() {
        Action::Observe { .. } | Action::Activate { .. } => {
            if service.target_lifecycle(viewer.target) == Some(TargetLifecycle::Disposed) {
                viewer.controller.take();
                return unavailable(Unavailable::TerminalRuntimeUnavailable);
            }
            return service.control_observation(
                viewer,
                match request.action() {
                    Action::Activate { geometry, .. } => Some(*geometry),
                    _ => None,
                },
            );
        }
        _ => {}
    }
    if matches!(
        request.action(),
        Action::Acquire { .. } | Action::Takeover { .. }
    ) {
        return acquire(
            service,
            viewer,
            matches!(request.action(), Action::Takeover { .. }),
        );
    }
    match request.action() {
        Action::Clear { .. } => {
            return screen_control::execute(service, viewer, WorkerAction::Clear);
        }
        Action::Reset { .. } => {
            return screen_control::execute(service, viewer, WorkerAction::Reset);
        }
        _ => {}
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
        | Action::SemanticInput {
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
        }
        | Action::HostColors {
            controller_id,
            sequence,
            ..
        } => (controller_id, *sequence),
        Action::Observe { .. }
        | Action::Activate { .. }
        | Action::Acquire { .. }
        | Action::Takeover { .. }
        | Action::Clear { .. }
        | Action::Reset { .. } => {
            unreachable!("standalone action handled")
        }
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
        Action::SemanticInput {
            input_kind,
            payload,
            ..
        } => match orchbee_capability_node_protocol::SemanticInput::new(
            match input_kind {
                orchbee_client_server_protocol::SemanticInputKind::Focus => {
                    orchbee_capability_node_protocol::SemanticInputKind::Focus
                }
                orchbee_client_server_protocol::SemanticInputKind::Key => {
                    orchbee_capability_node_protocol::SemanticInputKind::Key
                }
                orchbee_client_server_protocol::SemanticInputKind::Mouse => {
                    orchbee_capability_node_protocol::SemanticInputKind::Mouse
                }
                orchbee_client_server_protocol::SemanticInputKind::Text => {
                    orchbee_capability_node_protocol::SemanticInputKind::Text
                }
            },
            payload.as_bytes().to_vec(),
        ) {
            Ok(input) => WorkerAction::SemanticInput { sequence, input },
            Err(_) => return rejected(Rejected::HostInputInvalid),
        },
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
        Action::HostColors {
            foreground,
            background,
            cursor,
            palette,
            ..
        } => match orchbee_capability_node_protocol::HostDefaultColors::new(
            foreground.get(),
            background.get(),
            cursor.map(|color| color.get()),
            palette.map(|color| color.map(|color| color.get())),
        ) {
            Ok(colors) => WorkerAction::HostColors { sequence, colors },
            Err(_) => return rejected(Rejected::HostRejected),
        },
        Action::Observe { .. }
        | Action::Activate { .. }
        | Action::Acquire { .. }
        | Action::Takeover { .. }
        | Action::Clear { .. }
        | Action::Reset { .. } => {
            unreachable!("effect matched")
        }
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
        if !owners
            .get(&ControlTarget::from(controller.target))
            .is_some_and(|owner| {
                owner.controller_id == controller.cleanup.controller_id
                    && owner.phase == ReservationPhase::Active
            })
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
        // Typed input and pastes refute a pending resume; focus, mouse and resizes do not.
        #[cfg(windows)]
        if acknowledges_input(request.action()) || matches!(request.action(), Action::Paste { .. })
        {
            service.note_client_input(controller.target);
        }
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
            WorkerOutcome::InputAccepted => {
                if acknowledges_input(request.action())
                    && let Ok(mut coordinator) = service.coordinator.lock()
                {
                    coordinator.acknowledge_terminal_input(controller.target);
                }
                Outcome::InputAccepted {}
            }
            WorkerOutcome::HostColorsAccepted => Outcome::HostColorsAccepted {},
            WorkerOutcome::Resized(geometry) => {
                match orchbee_client_server_protocol::Geometry::new(
                    geometry.columns(),
                    geometry.rows(),
                    geometry.cell_pixel_width(),
                    geometry.cell_pixel_height(),
                ) {
                    Ok(geometry) => {
                        controller.geometry = Some(geometry);
                        service.remember_control_geometry(
                            &viewer.attachment_id,
                            ControlTarget::from(controller.target),
                            geometry,
                        );
                        Outcome::Resized { geometry }
                    }
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
                    Ok(accepted_bytes) => {
                        if let Ok(mut coordinator) = service.coordinator.lock() {
                            coordinator.acknowledge_terminal_input(controller.target);
                        }
                        Outcome::Pasted { accepted_bytes }
                    }
                    Err(_) => {
                        viewer.controller.take();
                        Outcome::OutcomeUnknown {}
                    }
                }
            }
            WorkerOutcome::Acquired
            | WorkerOutcome::Cleared { .. }
            | WorkerOutcome::ClearOutcomeUnknown
            | WorkerOutcome::ResetAccepted { .. }
            | WorkerOutcome::ResetOutcomeUnknown
            | WorkerOutcome::Released => {
                viewer.controller.take();
                Outcome::OutcomeUnknown {}
            }
        },
        Err(WorkerRequestError::Backpressure) => unavailable(Unavailable::ServerBusy),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => {
            viewer.controller.take();
            Outcome::OutcomeUnknown {}
        }
        // The carrier checks the operation kind; other transport failures retire the lease.
        _ => {
            viewer.controller.take();
            unavailable(Unavailable::TerminalRuntimeUnavailable)
        }
    }
}
pub(super) fn acquire<'service>(
    service: &'service LocalTerminalService,
    viewer: &mut ViewerAttachment<'service>,
    takeover: bool,
) -> Outcome {
    acquire_candidate(service, viewer, takeover, false)
}

pub(super) fn acquire_promoted<'service>(
    service: &'service LocalTerminalService,
    viewer: &mut ViewerAttachment<'service>,
) -> Outcome {
    acquire_candidate(service, viewer, false, true)
}

fn acquire_candidate<'service>(
    service: &'service LocalTerminalService,
    viewer: &mut ViewerAttachment<'service>,
    takeover: bool,
    promotion: bool,
) -> Outcome {
    if viewer.controller.is_some() && !takeover && !promotion {
        return rejected(Rejected::ControllerBusy);
    }
    let Some(template) = viewer.cleanup else {
        return rejected(Rejected::ViewerUnavailable);
    };
    // Drop a revoked lease outside the registry locks; its exact cleanup cannot affect a successor.
    let mut revoked_lease = None;
    // Generation selection and reservation are one short critical section, never a transport call.
    let (controller_id, request_id, cleanup_id, superseded) = {
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
        if promotion {
            let Ok(focus) = service.controller_focus.lock() else {
                return unavailable(Unavailable::ServerBusy);
            };
            if owners.contains_key(&ControlTarget::from(viewer.target))
                || !focus.prefers(ControlTarget::from(viewer.target), &viewer.attachment_id)
            {
                return rejected(Rejected::ControllerBusy);
            }
        }
        if let Some(lease) = viewer.controller.as_ref() {
            match owners.get(&ControlTarget::from(lease.target)) {
                Some(owner) if owner.controller_id == lease.cleanup.controller_id => {
                    if owner.phase != ReservationPhase::Active {
                        return rejected(Rejected::ControllerBusy);
                    }
                    return ControllerId::parse(&format!(
                        "{:032x}",
                        u128::from_be_bytes(owner.controller_id.as_bytes())
                    ))
                    .map_or_else(
                        |_| unavailable(Unavailable::ServerBusy),
                        |controller_id| Outcome::Acquired { controller_id },
                    );
                }
                _ => revoked_lease = viewer.controller.take(),
            }
        }
        let mut next = || {
            coordinator
                .allocate_identity()
                .and_then(|bytes| OperationId::from_bytes(bytes).ok())
        };
        let (Some(controller_id), Some(request_id), Some(cleanup_id), Some(previous_cleanup_id)) =
            (next(), next(), next(), next())
        else {
            return unavailable(Unavailable::ServerBusy);
        };
        let selected = ControlTarget::from(viewer.target);
        owners.retain(|target, _| {
            target.worker == selected.worker && target.generation == selected.generation
        });
        let reservation = ControllerReservation {
            controller_id,
            viewer_id: template.attachment_id,
            phase: ReservationPhase::Pending,
        };
        let previous = match reserve(&mut owners, selected, reservation, takeover) {
            Ok(previous) => previous,
            Err(()) => return rejected(Rejected::ControllerBusy),
        };
        let superseded = previous.map(|previous| WorkerRequest {
            request_id: previous_cleanup_id,
            controller_id: previous.controller_id,
            viewer_id: previous.viewer_id,
            worker_id: template.worker_id,
            worker_generation: template.worker_generation,
            action: WorkerAction::Release,
        });
        (controller_id, request_id, cleanup_id, superseded)
    };
    drop(revoked_lease);
    let mut lease = ControllerLease {
        geometry: None,
        service,
        handle: viewer.handle.clone(),
        target: viewer.target,
        sequence: 0,
        superseded,
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
            action: lease
                .superseded
                .as_ref()
                .map_or(WorkerAction::Acquire, |old| WorkerAction::Takeover {
                    previous_controller_id: old.controller_id,
                }),
            ..lease.cleanup.clone()
        }));
    match result {
        Ok(WorkerOperationResult::Control(result)) => match result.outcome {
            WorkerOutcome::Acquired => {
                // The serialized Worker acknowledged old detach before granting the new attachment.
                lease.superseded = None;
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
            WorkerOutcome::Cleared { .. }
            | WorkerOutcome::ClearOutcomeUnknown
            | WorkerOutcome::ResetAccepted { .. }
            | WorkerOutcome::ResetOutcomeUnknown
            | WorkerOutcome::Released
            | WorkerOutcome::InputAccepted
            | WorkerOutcome::Resized(_)
            | WorkerOutcome::Pasted { .. }
            | WorkerOutcome::HostColorsAccepted => Outcome::OutcomeUnknown {},
        },
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Outcome::OutcomeUnknown {},
        Err(WorkerRequestError::Backpressure) => {
            lease.armed = false;
            unavailable(Unavailable::ServerBusy)
        }
        // The carrier checks the operation kind; no other transport failure proves a grant.
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
    let Ok(mut owners) = service.controllers.lock() else {
        return unavailable(Unavailable::ServerBusy);
    };
    let Some(lease) = candidate.as_ref() else {
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    };
    let Some(owner) = owners.get_mut(&ControlTarget::from(lease.target)) else {
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    };
    if owner.controller_id != lease.cleanup.controller_id
        || owner.phase != ReservationPhase::Pending
    {
        return unavailable(Unavailable::TerminalRuntimeUnavailable);
    }
    owner.phase = ReservationPhase::Active;
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

    /// The caller holds Session through this cleanup and the authority's cleanup marker.
    pub(super) fn complete_disposed_target_cleanup(
        &self,
        coordinator: &SessionCoordinator,
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
            remove_exact(&mut owners, target, id.controller_id);
        }
        Ok(())
    }
}
fn reserve(
    owners: &mut BTreeMap<ControlTarget, ControllerReservation>,
    target: ControlTarget,
    candidate: ControllerReservation,
    takeover: bool,
) -> Result<Option<ControllerReservation>, ()> {
    let previous = owners.get(&target).copied();
    if previous.is_some_and(|owner| !takeover || owner.phase != ReservationPhase::Active)
        || (previous.is_none() && owners.len() >= 8)
    {
        return Err(());
    }
    owners.insert(target, candidate);
    Ok(previous)
}
fn remove_exact(
    owners: &mut BTreeMap<ControlTarget, ControllerReservation>,
    target: ControlTarget,
    id: OperationId,
) {
    if owners
        .get(&target)
        .is_some_and(|owner| owner.controller_id == id)
    {
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
        ControlRejection::HostClearAlternateScreen => Rejected::HostClearAlternateScreen,
        ControlRejection::HostInputInvalid => Rejected::HostInputInvalid,
        ControlRejection::HostInputLimitExceeded => Rejected::HostInputLimitExceeded,
        ControlRejection::HostInputBackpressure => Rejected::HostInputBackpressure,
        ControlRejection::HostInputUnavailable => Rejected::HostInputUnavailable,
        ControlRejection::ViewerUnavailable
        | ControlRejection::WrongWorker
        | ControlRejection::StaleWorkerGeneration => Rejected::ViewerUnavailable,
    }
}

/// Raw controller input also transports terminal-generated reports. Classify only for the
/// completion acknowledgement; the Worker still receives every original byte unchanged.
fn acknowledges_input(action: &Action) -> bool {
    match action {
        Action::Input { bytes, .. } => contains_typed_input(bytes.as_bytes()),
        Action::SemanticInput { input_kind, .. } => matches!(
            input_kind,
            orchbee_client_server_protocol::SemanticInputKind::Key
                | orchbee_client_server_protocol::SemanticInputKind::Text
        ),
        _ => false,
    }
}

pub(super) fn contains_typed_input(mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        let Some(length) = terminal_report_length(bytes) else {
            return true;
        };
        bytes = &bytes[length..];
    }
    false
}

/// Complete CSI focus, SGR/URXVT mouse, or legacy/UTF-8 X10 mouse reports. Everything else
/// retains its ordinary key/text meaning (including Escape and keyboard CSI sequences).
fn terminal_report_length(bytes: &[u8]) -> Option<usize> {
    let (body, prefix) = if let Some(body) = bytes.strip_prefix(b"\x1b[") {
        (body, 2)
    } else {
        (bytes.strip_prefix(b"\x9b")?, 1)
    };
    match *body.first()? {
        b'I' | b'O' => Some(prefix + 1),
        b'M' => {
            if body.len() == 4 {
                return Some(prefix + 4);
            }
            let mut offset = 1;
            for _ in 0..3 {
                let lead = *body.get(offset)?;
                let width = match lead {
                    0xc2..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf4 => 4,
                    _ => 1,
                };
                // Legacy mouse coordinates are opaque bytes, UTF-8 mode emits codepoints.
                let width = if width > 1
                    && body
                        .get(offset..offset + width)
                        .is_some_and(|part| std::str::from_utf8(part).is_ok())
                {
                    width
                } else {
                    1
                };
                offset += width;
            }
            Some(prefix + offset)
        }
        first if first == b'<' || first.is_ascii_digit() => {
            let sgr = first == b'<';
            let mut offset = usize::from(sgr);
            for field in 0..3 {
                let start = offset;
                while body.get(offset).is_some_and(u8::is_ascii_digit) {
                    offset += 1;
                }
                if start == offset {
                    return None;
                }
                if field < 2 {
                    if body.get(offset) != Some(&b';') {
                        return None;
                    }
                    offset += 1;
                }
            }
            match *body.get(offset)? {
                b'M' => Some(prefix + offset + 1),
                b'm' if sgr => Some(prefix + offset + 1),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
