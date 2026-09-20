use super::*;
use orchbee_capability_node_protocol::{
    HostTerminalIdentity, LifecycleInventoryPage, LifecycleStepOrdinal, LifecycleTerminalPhase,
    TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalLifecycleAction, TerminalLifecycleAuthorization,
    TerminalLifecycleOutcome, TerminalLifecycleRequest, TerminalLifecycleResult,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerRejection,
};

fn lifecycle(
    original: &AuthorizedTerminalLaunch,
    target: HostTerminalIdentity,
    action: TerminalLifecycleAction,
    ordinal: u8,
) -> TestResult<TerminalLifecycleRequest> {
    let authorization = original.authorization();
    Ok(TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: authorization.terminal_id(),
            worker_id: authorization.worker_id(),
            worker_generation: authorization.worker_generation(),
            binding_fence: authorization.fence(),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes([220; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(nonzero(lease_expiry()?)?),
            ),
        },
        close_operation_id: OperationId::from_bytes([200; 16])?,
        request_id: OperationId::from_bytes([200 + ordinal; 16])?,
        attempt_generation: nonzero(1)?,
        step_ordinal: LifecycleStepOrdinal::new(ordinal)?,
        target,
        action,
    })
}
fn step(
    server: &mut UnixStream,
    request: TerminalLifecycleRequest,
) -> TestResult<TerminalLifecycleResult> {
    write_obcn(
        server,
        &CapabilityNodeMessage::TerminalLifecycleRequest(request),
    )?;
    let CapabilityNodeMessage::TerminalLifecycleResult(result) = read_obcn(server)? else {
        return Err("missing lifecycle result".into());
    };
    assert_eq!(result.request(), request);
    Ok(result)
}
fn viewer(
    server: &mut UnixStream,
    id: u8,
    action: TerminalViewerAction,
) -> TestResult<TerminalViewerOutcome> {
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([id + 100; 16])?,
        attachment_id: OperationId::from_bytes([id; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(nonzero(7)?),
        action,
    };
    write_obcn(
        server,
        &CapabilityNodeMessage::TerminalViewerRequest(request),
    )?;
    let CapabilityNodeMessage::TerminalViewerResult(result) = read_obcn(server)? else {
        return Err("missing viewer result".into());
    };
    assert_eq!(result.request_id, request.request_id);
    assert_eq!(result.attachment_id, request.attachment_id);
    Ok(result.outcome)
}

#[test]
fn real_close_disposes_twenty_four_live_viewer_entries_and_preserves_sibling_and_replay()
-> TestResult {
    let fixture = RuntimeFixture::new()?;
    let _host = RunningHost::start(&fixture)?;
    wait_for_socket(&fixture.host_endpoint())?;
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    let target_launch = launch(1, None, TerminalLaunchMode::Immediate(geometry()?))?;
    let sibling_launch = launch(2, None, TerminalLaunchMode::Immediate(geometry()?))?;
    let TerminalLaunchOutcome::HostAccepted(target) =
        request_launch(&mut server, &target_launch)?.outcome()
    else {
        return Err("target launch".into());
    };
    let TerminalLaunchOutcome::HostAccepted(sibling) =
        request_launch(&mut server, &sibling_launch)?.outcome()
    else {
        return Err("sibling launch".into());
    };
    for id in 1..=24 {
        assert_eq!(
            viewer(
                &mut server,
                id,
                TerminalViewerAction::Attach {
                    host_epoch: target.identity().host_epoch(),
                    terminal_id: target.identity().terminal_id(),
                }
            )?,
            TerminalViewerOutcome::Attached
        );
    }
    assert_eq!(
        viewer(
            &mut server,
            25,
            TerminalViewerAction::Attach {
                host_epoch: sibling.identity().host_epoch(),
                terminal_id: sibling.identity().terminal_id(),
            }
        )?,
        TerminalViewerOutcome::Rejected(ViewerRejection::CapacityExceeded)
    );
    let force = lifecycle(
        &target_launch,
        target.identity(),
        TerminalLifecycleAction::Force,
        1,
    )?;
    assert_eq!(
        step(&mut server, force)?.outcome(),
        TerminalLifecycleOutcome::ForceAccepted
    );
    let mut settled = false;
    for ordinal in 2..=20 {
        let query = lifecycle(
            &target_launch,
            target.identity(),
            TerminalLifecycleAction::InventoryPage { cursor: None },
            ordinal,
        )?;
        let TerminalLifecycleOutcome::InventoryPage(page) = step(&mut server, query)?.outcome()
        else {
            return Err("target lifecycle inventory".into());
        };
        if page.target.is_some_and(|target| {
            matches!(
                target.phase,
                LifecycleTerminalPhase::Exited { .. } | LifecycleTerminalPhase::Failed
            )
        }) {
            settled = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        settled,
        "Force must settle before the fixture requests Dispose"
    );
    let dispose = lifecycle(
        &target_launch,
        target.identity(),
        TerminalLifecycleAction::Dispose,
        21,
    )?;
    let disposed = step(&mut server, dispose)?;
    assert_eq!(disposed.outcome(), TerminalLifecycleOutcome::Disposed);
    let query = lifecycle(
        &target_launch,
        target.identity(),
        TerminalLifecycleAction::InventoryPage { cursor: None },
        22,
    )?;
    assert!(matches!(
        step(&mut server, query)?.outcome(),
        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
            returned_count: 1,
            target: None,
            next_cursor: None,
            ..
        })
    ));
    assert_eq!(step(&mut server, dispose)?, disposed);
    for id in 1..=24 {
        assert_eq!(
            viewer(&mut server, id, TerminalViewerAction::Poll)?,
            TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
        );
        assert_eq!(
            viewer(&mut server, id, TerminalViewerAction::Detach)?,
            TerminalViewerOutcome::Detached
        );
    }
    for id in 26..=49 {
        assert_eq!(
            viewer(
                &mut server,
                id,
                TerminalViewerAction::Attach {
                    host_epoch: sibling.identity().host_epoch(),
                    terminal_id: sibling.identity().terminal_id(),
                }
            )?,
            TerminalViewerOutcome::Attached
        );
    }
    let sibling_query = lifecycle(
        &sibling_launch,
        sibling.identity(),
        TerminalLifecycleAction::InventoryPage { cursor: None },
        1,
    )?;
    let TerminalLifecycleOutcome::InventoryPage(page) = step(&mut server, sibling_query)?.outcome()
    else {
        return Err("sibling inventory".into());
    };
    assert_eq!(page.returned_count, 1);
    assert!(matches!(
        page.target.ok_or("sibling retained")?.phase,
        LifecycleTerminalPhase::Running { .. }
    ));
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    Ok(())
}
