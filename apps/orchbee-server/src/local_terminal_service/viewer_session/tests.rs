use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
};

use orchbee_capability_node_protocol::{
    OperationId, TerminalViewerOutcome as WorkerOutcome, TerminalViewerResult as WorkerResult,
    WorkerGeneration, WorkerId as CapabilityWorkerId,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};

use super::*;
use crate::{worker_obcn_carrier::create_worker_carrier, worker_registry::WorkerRegistry};

const WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);

fn viewer(service: &LocalTerminalService, handle: WorkerRequestHandle) -> ViewerAttachment<'_> {
    let attachment_id = OperationId::from_bytes([2; 16]).unwrap();
    let worker_id = CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes()).unwrap();
    ViewerAttachment {
        service,
        handle,
        target: BindingViewerTarget {
            terminal_id: orchbee_capability_node_protocol::TerminalId::from_bytes([1; 16]).unwrap(),
            worker: crate::terminal_coordinator::WorkerPlacement {
                worker_id,
                generation: WorkerGeneration::new(NonZeroU64::MIN),
            },
            identity: orchbee_capability_node_protocol::HostTerminalIdentity::new(
                orchbee_capability_node_protocol::HostCreationId::from_bytes([1; 16]).unwrap(),
                orchbee_capability_node_protocol::HostEpoch::from_bytes([3; 16]).unwrap(),
                orchbee_capability_node_protocol::HostTerminalId::from_bytes([4; 16]).unwrap(),
            ),
        },
        controller: None,
        cleanup: Some(WorkerRequest {
            request_id: OperationId::from_bytes([5; 16]).unwrap(),
            worker_id,
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            attachment_id,
            action: WorkerAction::Detach,
        }),
        projection: ViewerProjection::new(
            AttachmentId::from_bytes(attachment_id.as_bytes()).unwrap(),
            HostEpoch::from_bytes([3; 16]).unwrap(),
        ),
    }
}

fn attached_result() -> Result<WorkerOperationResult, WorkerRequestError> {
    Ok(WorkerOperationResult::Viewer(WorkerResult {
        request_id: OperationId::from_bytes([6; 16]).unwrap(),
        attachment_id: OperationId::from_bytes([2; 16]).unwrap(),
        outcome: WorkerOutcome::Attached,
    }))
}

fn service() -> LocalTerminalService {
    LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Err(std::io::ErrorKind::PermissionDenied.into()),
    )
}

#[test]
fn worker_loss_before_post_ipc_current_check_prevents_attachment_publication() {
    let service = service();
    let mut session = service.session();
    let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
    let handle = carrier.request_handle();
    let candidate = viewer(&service, handle.clone());
    drop(carrier);

    assert!(!handle.is_active());
    assert_eq!(
        session.publish_attach(candidate, attached_result()),
        unavailable(Unavailable::TerminalRuntimeUnavailable)
    );
    assert!(session.viewer.is_none());
}

#[test]
fn worker_loss_after_attachment_installation_is_observed_on_next_poll() {
    let service = service();
    let mut session = service.session();
    let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
    let handle = carrier.request_handle();
    let candidate = viewer(&service, handle.clone());
    assert!(matches!(
        service.workers.lock().unwrap().admit(WORKER_ID),
        crate::worker_registry::WorkerAdmission::Accepted { .. }
    ));
    let owner =
        WorkerRegistry::register_carrier(&service.workers, NonZeroU64::MIN, carrier).unwrap();
    assert!(matches!(
        session.publish_attach(candidate, attached_result()),
        Outcome::Attached { .. }
    ));
    assert!(session.viewer.is_some());
    drop(owner);
    assert!(!handle.is_active());

    let poll_id = RequestId::parse("77777777777777777777777777777777").unwrap();
    let ServerMessage::TerminalViewerResult(result) = session.execute(
        ClientMessage::TerminalViewer(TerminalViewerRequest::new(poll_id.clone(), Action::Poll {})),
    ) else {
        panic!("Poll must produce a viewer result")
    };
    assert_eq!(result.request_id(), &poll_id);
    assert_eq!(
        result.outcome(),
        &unavailable(Unavailable::TerminalRuntimeUnavailable)
    );
    assert!(session.viewer.is_none());
}
