use super::*;
use orchbee_workbench_protocol::*;

#[test]
fn divider_submission_atomically_distinguishes_a_dead_lane_from_transient_backlog() {
    let command: WorkbenchCommand = serde_json::from_value(serde_json::json!({
        "protocolVersion":1, "requestId":format!("{:032x}",1), "sessionId":format!("{:032x}",2),
        "expectedRevision":"0", "action":{"kind":"layout.set-split-ratio", "terminalContextId":format!("{:032x}",3),
        "tabId":format!("{:032x}",4), "split":{"splitId":format!("{:032x}",5),
        "firstSlotId":format!("{:032x}",6), "secondSlotId":format!("{:032x}",7)}, "previousRatio":0.5, "ratio":0.7}
    })).unwrap();
    for state in 0..3 {
        let lane = ClientControlLane::for_test();
        assert_eq!(lane.layout_admission(), Ok(()));
        {
            let mut handoff = lane.shared.0.lock().unwrap();
            match state {
                0 => handoff.stopped = true,
                1 => handoff.failed = true,
                _ => handoff.layout_pending = true,
            }
        }
        let expected = Err(if state == 2 {
            ClientViewControlReason::Busy
        } else {
            ClientViewControlReason::ProjectionUnavailable
        });
        assert_eq!(lane.submit_layout(command.clone()), expected);
        assert!(lane.take_layout_for_test().is_none());
    }
}

#[test]
fn pending_mutation_does_not_block_a_view_update_on_the_same_carrier() {
    use crate::local_client::test_support::multiplex_peer::{admit, read_open, respond};
    let (started, mutation) = mpsc::sync_channel(1);
    let (updated, update) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream, 1)?;
        let mut pending_mutation = false;
        let mut initial_view_revision = None;
        loop {
            let open = match read_open(&mut stream) {
                Ok(open) => open,
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
            };
            match read_workbench_request(&mut io::Cursor::new(&open.request)).unwrap() {
                WorkbenchRequest::SnapshotRead(_) => {
                    assert_eq!(
                        open.mode,
                        orchbee_workbench_protocol::StreamMode::Invalidations
                    );
                }
                WorkbenchRequest::Command(command) => {
                    assert!(matches!(
                        command.action,
                        WorkbenchAction::SplitRatioSet { .. }
                    ));
                    pending_mutation = true;
                    started.send(()).unwrap();
                }
                WorkbenchRequest::ClientView(request) => {
                    let ClientViewAction::Synchronize {
                        heartbeat,
                        view_revision,
                        view,
                        ..
                    } = request.action
                    else {
                        panic!()
                    };
                    let initial_revision =
                        *initial_view_revision.get_or_insert(view_revision.value());
                    respond(
                        &mut stream,
                        &open,
                        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            session_id: request.session_id,
                            outcome: ClientViewOutcome::Synchronized {
                                control: None,
                                client: Box::new(ClientViewRecord {
                                    client_id: WorkbenchClientId::parse(&format!("{:032x}", 1))
                                        .unwrap(),
                                    heartbeat,
                                    view_revision,
                                    view: *view,
                                    availability: ClientViewAvailability::Current,
                                }),
                            },
                        }),
                        1,
                    )?;
                    if pending_mutation
                        && view_revision.value() == initial_revision.checked_add(1).unwrap()
                    {
                        updated.try_send(()).ok();
                    }
                }
                _ => panic!("only control and mutation streams"),
            }
        }
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint())
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut lane = ClientControlLane::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    drop(client);
    let command: WorkbenchCommand = serde_json::from_value(serde_json::json!({
        "protocolVersion":1, "requestId":format!("{:032x}",1), "sessionId":format!("{:032x}",2), "expectedRevision":"0",
        "action":{"kind":"layout.set-split-ratio", "terminalContextId":format!("{:032x}",3), "tabId":format!("{:032x}",4),
        "split":{"splitId":format!("{:032x}",5), "firstSlotId":format!("{:032x}",6), "secondSlotId":format!("{:032x}",7)},
        "previousRatio":0.5, "ratio":0.7}
    })).unwrap();
    assert_eq!(lane.submit_layout(command.clone()), Ok(()));
    assert_eq!(
        lane.submit_layout(command),
        Err(ClientViewControlReason::Busy)
    );
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
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
    assert!(lane.publish(sample.clone()));
    if let Err(error) = mutation.recv_timeout(Duration::from_secs(2)) {
        lane.close();
        panic!(
            "mutation was not received: {error:?}; peer: {:?}",
            server.finish()
        );
    }
    sample.heartbeat =
        ClientViewSequence::new(sample.heartbeat.value().checked_add(1).unwrap()).unwrap();
    sample.revision =
        ClientViewSequence::new(sample.revision.value().checked_add(1).unwrap()).unwrap();
    assert!(lane.publish(sample));
    if let Err(error) = update.recv_timeout(Duration::from_secs(2)) {
        lane.close();
        panic!(
            "view update was not received: {error:?}; peer: {:?}",
            server.finish()
        );
    }
    assert!(lane.take_layout_result().is_none());
    lane.close();
    assert!(
        lane.take_layout_result()
            .unwrap()
            .reply
            .unwrap_err()
            .may_have_committed
    );
    server.finish().unwrap();
}
