use super::*;
use orchbee_capability_node_protocol::{
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerRejection,
    ViewerSnapshotReason,
};
use orchbee_terminal_protocol::{TerminalFrameRoute, inspect_terminal_frame_route};

#[test]
fn real_worker_relays_independent_viewers_and_snapshot_feedback_then_retains_terminal() -> TestResult
{
    let fixture = RuntimeFixture::new()?;
    let _host = RunningHost::start(&fixture)?;
    wait_for_socket(&fixture.host_endpoint())?;
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    let launched = request_launch(
        &mut server,
        &launch(1, None, TerminalLaunchMode::Immediate(geometry()?))?,
    )?;
    let TerminalLaunchOutcome::HostAccepted(created) = launched.outcome() else {
        return Err("Host did not create viewer terminal".into());
    };
    let mut sequence = 10_u128;
    for id in [1, 2] {
        assert_eq!(
            viewer(
                &mut server,
                &mut sequence,
                id,
                TerminalViewerAction::Attach {
                    host_epoch: created.identity().host_epoch(),
                    terminal_id: created.identity().terminal_id()
                }
            )?,
            TerminalViewerOutcome::Attached
        );
    }
    // Invalid generation/placement requests cannot gain viewer capability.
    for (worker_id, worker_generation, expected) in [
        (
            CapabilityWorkerId::from_bytes([99; 16])?,
            WorkerGeneration::new(nonzero(7)?),
            ViewerRejection::WrongWorker,
        ),
        (
            CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
            WorkerGeneration::new(nonzero(8)?),
            ViewerRejection::StaleWorkerGeneration,
        ),
    ] {
        sequence += 1;
        let request = TerminalViewerRequest {
            request_id: OperationId::from_bytes(sequence.to_be_bytes())?,
            worker_id,
            worker_generation,
            attachment_id: OperationId::from_bytes([9; 16])?,
            action: TerminalViewerAction::Attach {
                host_epoch: created.identity().host_epoch(),
                terminal_id: created.identity().terminal_id(),
            },
        };
        write_obcn(
            &mut server,
            &CapabilityNodeMessage::TerminalViewerRequest(request),
        )?;
        let CapabilityNodeMessage::TerminalViewerResult(result) = read_obcn(&mut server)? else {
            return Err("missing viewer rejection".into());
        };
        assert_eq!(result.request_id, request.request_id);
        assert_eq!(result.outcome, TerminalViewerOutcome::Rejected(expected));
    }
    let first = next_frame(&mut server, &mut sequence, 1)?;
    let second = next_frame(&mut server, &mut sequence, 2)?;
    assert_ne!(first.attachment_id, second.attachment_id);
    assert_eq!(first.host_epoch, second.host_epoch);
    // The operation kernel and Host command exchange still progress with both viewer flights occupied.
    write_obcn(
        &mut server,
        &CapabilityNodeMessage::AuthorizedTerminalStart(AuthorizedTerminalStart::new(
            authorization(1, 2)?,
            created.identity(),
            geometry()?,
        )),
    )?;
    let CapabilityNodeMessage::TerminalStartResult(started) = read_obcn(&mut server)? else {
        return Err("missing concurrent viewer Start result".into());
    };
    assert!(
        matches!(started.outcome(), TerminalStartOutcome::HostAccepted(accepted)
        if accepted.transition() == orchbee_capability_node_protocol::StartTransition::AlreadyStarted)
    );
    // Viewer 1 deliberately never acknowledges. Viewer 2 can still ask Host for
    // a fresh snapshot and receive it through the same broker and OBCN connection.
    assert_eq!(
        viewer(
            &mut server,
            &mut sequence,
            2,
            TerminalViewerAction::NeedSnapshot {
                sequence: second.sequence,
                reason: ViewerSnapshotReason::DecoderRejected,
            }
        )?,
        TerminalViewerOutcome::FeedbackAccepted
    );
    let refreshed = next_frame(&mut server, &mut sequence, 2)?;
    assert!(refreshed.sequence > second.sequence);
    assert_eq!(refreshed.base_revision, None);
    assert_eq!(
        viewer(
            &mut server,
            &mut sequence,
            2,
            TerminalViewerAction::FrameApplied {
                sequence: second.sequence,
                generation: second.generation,
                revision: second.revision,
            }
        )?,
        TerminalViewerOutcome::Rejected(ViewerRejection::FeedbackMismatch)
    );
    assert_eq!(
        viewer(
            &mut server,
            &mut sequence,
            2,
            TerminalViewerAction::FrameApplied {
                sequence: refreshed.sequence,
                generation: refreshed.generation,
                revision: refreshed.revision,
            }
        )?,
        TerminalViewerOutcome::FeedbackAccepted
    );
    assert_eq!(
        viewer(&mut server, &mut sequence, 1, TerminalViewerAction::Poll)?,
        TerminalViewerOutcome::Empty
    );
    assert_eq!(
        viewer(&mut server, &mut sequence, 1, TerminalViewerAction::Detach)?,
        TerminalViewerOutcome::Detached
    );
    assert_eq!(
        viewer(&mut server, &mut sequence, 1, TerminalViewerAction::Poll)?,
        TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
    );
    assert_eq!(
        viewer(
            &mut server,
            &mut sequence,
            3,
            TerminalViewerAction::Attach {
                host_epoch: created.identity().host_epoch(),
                terminal_id: created.identity().terminal_id()
            }
        )?,
        TerminalViewerOutcome::Attached
    );
    let reattached = next_frame(&mut server, &mut sequence, 3)?;
    assert_ne!(reattached.attachment_id, first.attachment_id);
    assert_eq!(reattached.base_revision, None);
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    let (mut observer, epoch) = connect_observer(&fixture.host_endpoint())?;
    let command = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
        cursor: None,
        maximum_terminals: 1,
    }))?;
    observer.write_all(&host_record(
        command.kind,
        command.role,
        &command.payload,
        epoch,
        CorrelationId::from_bytes([88; 16])?,
    )?)?;
    let record = read_terminal_record(&mut observer)?;
    let envelope = decode_transport_envelope(&record)?;
    let InventoryControl::TerminalList(list) =
        decode_inventory_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err("missing retained viewer terminal".into());
    };
    assert_eq!(list.terminals.len(), 1);
    assert!(
        matches!(&list.terminals[0], TerminalSummary::Running { core, .. }
        if core.identity.terminal_id.as_bytes() == &created.identity().terminal_id().as_bytes())
    );
    let terminate =
        encode_lifecycle_control(&LifecycleControl::ForceTerminateTerminal(TerminalTarget {
            terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes(
                created.identity().terminal_id().as_bytes(),
            )?,
        }))?;
    observer.write_all(&host_record(
        terminate.kind,
        terminate.role,
        &terminate.payload,
        epoch,
        CorrelationId::from_bytes([89; 16])?,
    )?)?;
    let record = read_terminal_record(&mut observer)?;
    assert_eq!(
        decode_transport_envelope(&record)?.payload,
        terminate.payload
    );
    Ok(())
}

fn viewer(
    server: &mut UnixStream,
    sequence: &mut u128,
    id: u8,
    action: TerminalViewerAction,
) -> TestResult<TerminalViewerOutcome> {
    *sequence += 1;
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes(sequence.to_be_bytes())?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(nonzero(7)?),
        attachment_id: OperationId::from_bytes([id; 16])?,
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

fn next_frame(
    server: &mut UnixStream,
    sequence: &mut u128,
    id: u8,
) -> TestResult<TerminalFrameRoute> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match viewer(server, sequence, id, TerminalViewerAction::Poll)? {
            TerminalViewerOutcome::Record(record) => {
                let envelope = decode_transport_envelope(&record)?;
                assert!(matches!(envelope.kind, 41 | 42));
                let route = inspect_terminal_frame_route(envelope.payload)?;
                assert_eq!(route.attachment_id.as_bytes(), &[id; 16]);
                return Ok(route);
            }
            TerminalViewerOutcome::Empty if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5))
            }
            other => return Err(format!("expected viewer frame, observed {other:?}").into()),
        }
    }
}
