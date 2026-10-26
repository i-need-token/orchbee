//! A producer commit racing a new UI sample cannot discard a transferred control.
use super::*;
use crate::local_client::tests::multiplex_peer::{Open, admit, read_open, respond};
use orchbee_workbench_protocol::{
    ClientViewCompletionOutcome, ClientViewControl, StreamMode, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult,
};
use std::{thread, time::Instant};

fn reply(
    peer: &mut orchbee_local_ipc::LocalStream,
    open: &Open,
    request: WorkbenchClientViewRequest,
    operation: Option<WorkbenchRequestId>,
) -> io::Result<()> {
    let ClientViewAction::Synchronize {
        heartbeat,
        view_revision,
        view,
        ..
    } = request.action
    else {
        panic!()
    };
    let client_id = WorkbenchClientId::parse(&format!("{:032x}", 1)).unwrap();
    respond(
        peer,
        open,
        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome: ClientViewOutcome::Synchronized {
                control: operation.map(|operation_id| {
                    Box::new(ClientViewControl {
                        operation_id,
                        client_id: client_id.clone(),
                        expected_view_revision: view_revision,
                        minimum_workbench_revision: view.workbench_revision,
                        intent: ClientViewIntent::PopupClose {},
                    })
                }),
                client: Box::new(ClientViewRecord {
                    client_id,
                    heartbeat,
                    view_revision,
                    view: *view,
                    availability: ClientViewAvailability::Current,
                }),
            },
        }),
        1,
    )
}
fn synchronization(
    peer: &mut orchbee_local_ipc::LocalStream,
) -> io::Result<(Open, WorkbenchClientViewRequest)> {
    let open = read_open(peer)?;
    assert_eq!(
        open.mode,
        StreamMode::Request,
        "control transfer is never a reconstructable stream"
    );
    let WorkbenchRequest::ClientView(request) =
        read_workbench_request(&mut io::Cursor::new(&open.request)).unwrap()
    else {
        panic!()
    };
    Ok((open, request))
}
fn take(lane: &ClientControlLane) -> Delivery {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(delivery) = lane.take_control() {
            return delivery;
        }
        assert!(!lane.failed());
        assert!(Instant::now() < deadline, "control delivery deadline");
        thread::yield_now();
    }
}

#[test]
fn sample_change_during_control_transfer_preserves_completion_and_next_command() {
    let (registered, first_sample) = mpsc::sync_channel(1);
    let (delivered, pending_reply) = mpsc::sync_channel(1);
    let (allow, reply_allowed) = mpsc::sync_channel(1);
    let (completed, settled) = mpsc::sync_channel(1);
    let first_id = WorkbenchRequestId::parse(&format!("{:032x}", 100)).unwrap();
    let second_id = WorkbenchRequestId::parse(&format!("{:032x}", 101)).unwrap();
    let first_operation = first_id.clone();
    let second_operation = second_id.clone();
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let observer = read_open(&mut peer)?;
        assert_eq!(observer.mode, StreamMode::Invalidations);
        let WorkbenchRequest::SnapshotRead(initial) =
            read_workbench_request(&mut io::Cursor::new(&observer.request)).unwrap()
        else {
            panic!()
        };
        let (open, request) = synchronization(&mut peer)?;
        reply(&mut peer, &open, request, None)?;
        registered.send(()).unwrap();
        // The payload is an invalidation baseline, never the control's delivery mailbox.
        respond(
            &mut peer,
            &observer,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: initial.request_id,
                outcome: WorkbenchSnapshotOutcome::Denied {},
            }),
            1,
        )?;
        let (open, request) = synchronization(&mut peer)?;
        delivered.send(()).unwrap();
        reply_allowed.recv_timeout(Duration::from_secs(2)).unwrap();
        reply(&mut peer, &open, request, Some(first_operation.clone()))?;
        loop {
            let (open, request) = synchronization(&mut peer)?;
            let ClientViewAction::Synchronize { completion, .. } = &request.action else {
                panic!()
            };
            if let Some(completion) = completion {
                assert_eq!(completion.operation_id, first_operation);
                assert!(matches!(
                    completion.outcome,
                    ClientViewCompletionOutcome::Rejected {
                        reason: ClientViewControlReason::StaleView
                    }
                ));
                reply(&mut peer, &open, request, Some(second_operation.clone()))?;
                completed.send(()).unwrap();
                break;
            }
            reply(&mut peer, &open, request, None)?;
        }
        loop {
            match read_open(&mut peer) {
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
                Ok(open) => panic!("no further request before shutdown: {:?}", open.mode),
            }
        }
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort")
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let WorkbenchRequest::ClientView(request) =
        parse_workbench_request(&serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap())
            .unwrap()
    else {
        panic!()
    };
    let ClientViewAction::Synchronize {
        heartbeat,
        view_revision,
        view,
        ..
    } = request.action
    else {
        panic!()
    };
    let mut sample = Sample {
        heartbeat,
        revision: view_revision,
        view: *view,
        graphics: None,
    };
    let mut lane =
        ClientControlLane::spawn(client.origin(), client.operation_clock(), request.session_id)
            .unwrap();
    assert!(lane.publish(sample.clone()));
    first_sample.recv_timeout(Duration::from_secs(2)).unwrap();
    pending_reply.recv_timeout(Duration::from_secs(2)).unwrap();
    sample.heartbeat = ClientViewSequence::new(sample.heartbeat.value() + 1).unwrap();
    sample.revision = ClientViewSequence::new(sample.revision.value() + 1).unwrap();
    assert!(lane.publish(sample.clone()));
    allow.send(()).unwrap();
    assert_eq!(take(&lane).command.operation_id, first_id);
    assert!(lane.complete(
        sample,
        ClientViewCompletion {
            operation_id: first_id,
            outcome: ClientViewCompletionOutcome::Rejected {
                reason: ClientViewControlReason::StaleView
            },
        }
    ));
    settled.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(take(&lane).command.operation_id, second_id);
    assert!(lane.take_control().is_none());
    lane.close();
    drop(client);
    server.finish().unwrap();
}
