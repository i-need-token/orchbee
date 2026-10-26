use super::*;
use orchbee_capability_node_protocol::{
    LeaseId, LifecycleStepOrdinal, OperationId, TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
    TerminalFence, TerminalId, TerminalLifecycleAuthorization, TerminalOperationLease,
};

fn foreign_query(ordinal: u8) -> TerminalLifecycleRequest {
    TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: TerminalId::from_bytes([42; 16]).unwrap(),
            worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            binding_fence: TerminalFence::new(NonZeroU64::MIN),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes([43; 16]).unwrap(),
                lease_expiry().unwrap(),
            ),
        },
        close_operation_id: OperationId::from_bytes([44; 16]).unwrap(),
        request_id: OperationId::from_bytes([100 + ordinal; 16]).unwrap(),
        attempt_generation: NonZeroU64::MIN,
        step_ordinal: LifecycleStepOrdinal::new(ordinal).unwrap(),
        target: HostTerminalIdentity::new(
            HostCreationId::from_bytes([45; 16]).unwrap(),
            HostEpoch::from_bytes([4; 16]).unwrap(),
            HostTerminalId::from_bytes([46; 16]).unwrap(),
        ),
        action: TerminalLifecycleAction::InventoryPage { cursor: None },
    }
}

#[test]
fn actual_full_carrier_backpressure_does_not_charge_the_close_step() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    let handle = service.workers.lock().unwrap().request_handle().unwrap();
    // One effect-free request is in flight and the bounded high lane is filled
    // behind it, so the next lifecycle admission sees real backpressure.
    let first = handle.submit_lifecycle(foreign_query(1)).unwrap();
    let first_request = lifecycle(&mut peer);
    let queued: Vec<_> = (2..=9)
        .map(|ordinal| handle.submit_lifecycle(foreign_query(ordinal)).unwrap())
        .collect();
    service.start_close_maintenance().unwrap();
    let _accepted = service.execute_terminal_tab_close(close.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let next = service
            .coordinator
            .lock()
            .unwrap()
            .close_authority()
            .unwrap()
            .next_terminal_tab_close_step(&close.request_id, Instant::now())
            .unwrap();
        if matches!(next, CloseContinuationNext::WaitUntil(_)) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Close did not yield on the full carrier"
        );
        thread::yield_now();
    }
    reply(&mut peer, first_request, settled_page());
    let mut last_queued_operation = None;
    for receipt in queued {
        let queued_request = lifecycle(&mut peer);
        last_queued_operation = Some(queued_request.close_operation_id);
        reply(&mut peer, queued_request, settled_page());
        receipt.wait().unwrap();
    }
    let _first = first.wait().unwrap();
    let actual_close = lifecycle(&mut peer);
    assert_eq!(
        actual_close.step_ordinal.get(),
        1,
        "prequeue refusal must not charge the Close"
    );
    assert_ne!(
        actual_close.close_operation_id,
        last_queued_operation.unwrap()
    );
    reply(
        &mut peer,
        actual_close,
        TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
    );
    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn successful_enqueue_is_confirmed_after_expiry_under_the_real_site_gate() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    let (admitted, admission) = mpsc::sync_channel(1);
    let (proceed, released) = mpsc::sync_channel(1);
    service.after_close_admission_for_test(move || {
        admitted.send(()).unwrap();
        released.recv().unwrap();
    });
    service.start_close_maintenance().unwrap();
    let accepted_at = Instant::now() - Duration::from_millis(29_500);
    {
        let worker = WorkerPlacement {
            worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
            generation: WorkerGeneration::new(NonZeroU64::MIN),
        };
        let accepted = service
            .coordinator
            .lock()
            .unwrap()
            .execute_terminal_tab_close(close.clone(), Some(worker), true, accepted_at)
            .unwrap();
        assert!(matches!(
            accepted.outcome,
            WorkbenchCommandOutcome::TabCloseAccepted { .. }
        ));
    }
    service.close_maintenance.notify();
    admission.recv_timeout(Duration::from_secs(2)).unwrap();
    let query = lifecycle(&mut peer);
    let mut site = service.coordinator.lock().unwrap();
    proceed.send(()).unwrap();
    // The actual owner gate, not another adapter entry point, delays admission confirmation.
    let remaining =
        (accepted_at + Duration::from_secs(30)).saturating_duration_since(Instant::now());
    thread::sleep(remaining + Duration::from_millis(10));
    assert_eq!(
        site.close_authority()
            .unwrap()
            .next_terminal_tab_close_step(&close.request_id, Instant::now())
            .unwrap(),
        CloseContinuationNext::Outstanding
    );
    reply(&mut peer, query, settled_page());
    drop(site);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(
            read(&service, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::CleanupBlocked {
                    resumable: true,
                    ..
                },
                ..
            }
        ) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "late admission/result must settle before parking"
        );
        thread::yield_now();
    }
    assert_eq!(read_snapshot(&service).revision, WorkbenchRevision::new(1));
    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn old_dispose_result_settles_after_actual_worker_replacement_without_adoption() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    service.start_close_maintenance().unwrap();
    let _accepted = service.execute_terminal_tab_close(close.clone()).unwrap();
    let query = lifecycle(&mut peer);
    reply(&mut peer, query, settled_page());
    let dispose = lifecycle(&mut peer);
    let (received, result) = mpsc::sync_channel(1);
    let (proceed, released) = mpsc::sync_channel(1);
    service.after_close_result_for_test(move || {
        received.send(()).unwrap();
        released.recv().unwrap();
    });
    reply(&mut peer, dispose, TerminalLifecycleOutcome::Disposed);
    result.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        service
            .coordinator
            .lock()
            .unwrap()
            .close_authority()
            .unwrap()
            .next_terminal_tab_close_step(&close.request_id, Instant::now())
            .unwrap(),
        CloseContinuationNext::Outstanding
    );
    drop(peer);
    let _exit = worker_thread.join().unwrap();
    let worker_id = orchbee_local_worker_protocol::WorkerId::from_bytes([9; 16]);
    let WorkerAdmission::Accepted { generation } = service.workers.lock().unwrap().admit(worker_id)
    else {
        panic!("replacement admission")
    };
    assert_eq!(generation.get(), 2);
    let owner = WorkerRegistry::register_carrier(
        &service.workers,
        generation,
        create_worker_carrier(worker_id, generation),
    )
    .unwrap();
    let (server, mut replacement) = UnixStream::pair().unwrap();
    replacement
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let replacement_thread = thread::spawn(move || owner.run(server));
    proceed.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(
            read(&service, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closed { .. },
                ..
            }
        ) {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert!(
        matches!(read_message(&mut replacement),
        Err(orchbee_capability_node_protocol::ReadMessageError::Io(error))
            if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)),
        "old receipt must leave the replacement connected without a request"
    );
    assert!(
        service
            .workers
            .lock()
            .unwrap()
            .request_handle()
            .unwrap()
            .is_active()
    );
    service.stop_close_maintenance();
    drop(replacement);
    let _exit = replacement_thread.join().unwrap();
}
