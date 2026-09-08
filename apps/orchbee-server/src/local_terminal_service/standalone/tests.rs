use super::*;
use crate::{
    site_coordinator::PreparedOpen,
    terminal_coordinator::{Completion, WorkerPlacement},
    worker_registry::WorkerRegistry,
};
use orchbee_capability_node_protocol::{WorkerGeneration, WorkerId};
use orchbee_client_server_protocol::{Geometry, RequestId};
use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
    time::Duration,
};

fn request() -> TerminalOpenRequest {
    TerminalOpenRequest::new(
        RequestId::parse("11111111111111111111111111111111").unwrap(),
        Geometry::new(80, 24, 8, 16).unwrap(),
    )
}

#[test]
fn standalone_wait_timeout_does_not_cancel_and_late_unknown_remains_final() {
    let mut service = LocalTerminalService::new(Arc::new(Mutex::new(WorkerRegistry::new())));
    service.standalone_wait = Duration::from_millis(10);
    let worker = WorkerPlacement {
        worker_id: WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    };
    let PreparedOpen::Dispatch(pending) = service.coordinator.lock().unwrap().prepare_open(
        &request(),
        Some(RequestId::parse("22222222222222222222222222222222").unwrap()),
        Some(worker),
        lease_expiry(),
    ) else {
        panic!("dispatch expected");
    };
    assert_eq!(service.open_standalone(&request()), busy());
    assert!(matches!(
        service
            .coordinator
            .lock()
            .unwrap()
            .prepare_open(&request(), None, Some(worker), None),
        PreparedOpen::Wait
    ));
    service
        .coordinator
        .lock()
        .unwrap()
        .complete(pending, Completion::Unknown, Some(worker))
        .unwrap();
    service.standalone_changed.notify_all();
    assert_eq!(
        service.open_standalone(&request()),
        OpenOutcome::OutcomeUnknown {}
    );
}

#[test]
fn expired_standalone_wait_is_effect_free() {
    let service = LocalTerminalService::new(Arc::new(Mutex::new(WorkerRegistry::new())));
    assert_eq!(
        service.open_standalone_before(&request(), Instant::now()),
        busy()
    );
    assert_eq!(
        service.open_standalone(&request()),
        OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::NoCompatibleWorker
        }
    );
}
