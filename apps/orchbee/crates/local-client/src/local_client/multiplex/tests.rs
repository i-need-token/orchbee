//! Controlled peer traces cover cancellation and queue loss across actual carrier framing.
use super::*;
use crate::local_client::{
    LocalClient,
    tests::{
        multiplex_peer::{admit, read_open},
        server::FakeServer,
    },
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    read_multiplex_message, write_multiplex_message,
};
use std::sync::mpsc;

fn request(id: u64) -> WorkbenchRequest {
    WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
    })
}
fn frame(
    peer: &mut LocalStream,
    stream_id: MultiplexStreamId,
    sequence: u64,
    kind: StreamFrameKind,
    value: u8,
) -> io::Result<()> {
    write_multiplex_message(
        peer,
        &MultiplexMessage::StreamFrame {
            stream_id,
            sequence: NonZeroU64::new(sequence).unwrap(),
            kind,
            record: vec![value],
        },
    )
}
fn take(stream: &Subscription) -> io::Result<StreamEvent> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(event) = stream.poll()? {
            return Ok(event);
        }
        assert!(Instant::now() < deadline, "carrier delivery deadline");
        thread::yield_now();
    }
}

#[test]
fn cancellation_fences_a_late_reply_while_another_stream_completes() {
    let (opened, ready) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let first = read_open(&mut peer)?;
        let second = read_open(&mut peer)?;
        opened.send(()).unwrap();
        loop {
            if let MultiplexMessage::StreamClose { stream_id, .. } =
                read_multiplex_message(&mut peer)?
            {
                assert_eq!(stream_id, first.id);
                break;
            }
        }
        frame(&mut peer, first.id, 1, StreamFrameKind::Result, 1)?;
        frame(&mut peer, second.id, 1, StreamFrameKind::Result, 2)?;
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort")
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let first = client
        .origin()
        .subscribe(&request(1), StreamMode::Request)
        .unwrap();
    let second = client
        .origin()
        .subscribe(&request(2), StreamMode::Request)
        .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(first.cancel(), "the peer already received this request");
    assert!(first.poll().is_err());
    assert!(matches!(take(&second).unwrap(), StreamEvent::Record(bytes) if bytes == [2]));
    drop((first, second, client));
    server.finish().unwrap();
}

#[test]
fn event_overflow_requires_an_observed_gap_before_the_replacement_snapshot() {
    let (resynced, ready) = mpsc::sync_channel(1);
    let (release, finish) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let open = read_open(&mut peer)?;
        for sequence in 1..=34 {
            frame(
                &mut peer,
                open.id,
                sequence,
                if sequence == 1 {
                    StreamFrameKind::Snapshot
                } else {
                    StreamFrameKind::Update
                },
                sequence as u8,
            )?;
        }
        loop {
            if let MultiplexMessage::Resnapshot { stream_id } = read_multiplex_message(&mut peer)? {
                assert_eq!(stream_id, open.id);
                break;
            }
        }
        write_multiplex_message(
            &mut peer,
            &MultiplexMessage::Gap {
                stream_id: open.id,
                through: NonZeroU64::new(34).unwrap(),
            },
        )?;
        frame(&mut peer, open.id, 35, StreamFrameKind::Snapshot, 35)?;
        resynced.send(()).unwrap();
        finish.recv_timeout(Duration::from_secs(2)).unwrap();
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort")
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let stream = client
        .origin()
        .subscribe(&request(1), StreamMode::Events)
        .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(take(&stream).unwrap(), StreamEvent::Gap));
    loop {
        match take(&stream).unwrap() {
            StreamEvent::Gap => {}
            StreamEvent::Record(bytes) => {
                assert_eq!(bytes, [35]);
                break;
            }
            StreamEvent::Disconnected => panic!("resnapshot preserves the connection"),
        }
    }
    release.send(()).unwrap();
    drop((stream, client));
    server.finish().unwrap();
}

#[test]
fn state_coalescing_releases_the_connection_budget_and_overrun_is_typed() {
    let budget = Arc::new(AtomicUsize::new(0));
    let state = Delivery::new(Arc::clone(&budget)).unwrap();
    state
        .publish(StreamEvent::Record(vec![1; 8]), StreamMode::State)
        .unwrap();
    state
        .publish(StreamEvent::Record(vec![2; 4]), StreamMode::State)
        .unwrap();
    assert_eq!(budget.load(Ordering::Acquire), 4);
    assert!(matches!(state.take().unwrap(), Some(StreamEvent::Record(bytes)) if bytes == [2; 4]));
    assert_eq!(budget.load(Ordering::Acquire), 0);
    state
        .publish(StreamEvent::Disconnected, StreamMode::State)
        .unwrap();
    state
        .publish(StreamEvent::Record(vec![4]), StreamMode::State)
        .unwrap();
    state
        .publish(StreamEvent::Record(vec![5]), StreamMode::State)
        .unwrap();
    assert!(matches!(
        state.take().unwrap(),
        Some(StreamEvent::Disconnected)
    ));
    assert!(matches!(state.take().unwrap(), Some(StreamEvent::Record(bytes)) if bytes == [5]));
    budget.store(MAXIMUM_MULTIPLEX_BYTES - 1, Ordering::Release);
    let error = state
        .publish(StreamEvent::Record(vec![3; 2]), StreamMode::Events)
        .unwrap_err();
    assert!(matches!(
        error
            .get_ref()
            .and_then(|error| error.downcast_ref::<Disconnect>()),
        Some(Disconnect(
            orchbee_workbench_protocol::MultiplexDisconnectReason::Capacity
        ))
    ));
    assert!(terminal_failure(&error));
    assert!(!terminal_failure(&io::ErrorKind::TimedOut.into()));
    assert!(!terminal_failure(&io::ErrorKind::ConnectionAborted.into()));
    budget.store(0, Ordering::Release);
    let budget = Arc::new(AtomicUsize::new(health::RESERVED_BYTES));
    let streams = Arc::new(AtomicUsize::new(health::RESERVED_STREAMS));
    let admitted = (0..MAXIMUM_MULTIPLEX_STREAMS - health::RESERVED_STREAMS)
        .map(|_| Reservation::acquire(&budget, &streams, 32).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        Reservation::acquire(&budget, &streams, 1)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(admitted);
    assert_eq!(budget.load(Ordering::Acquire), health::RESERVED_BYTES);
    assert_eq!(streams.load(Ordering::Acquire), health::RESERVED_STREAMS);
    budget.store(MAXIMUM_MULTIPLEX_BYTES - 1, Ordering::Release);
    assert_eq!(
        Reservation::acquire(&budget, &streams, 2)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        streams.load(Ordering::Acquire),
        health::RESERVED_STREAMS,
        "rejected bytes return the ordinary stream slot"
    );
}

#[test]
fn idle_probe_deadline_survives_unrelated_push_and_reconnect_does_not_replay_a_mutation() {
    use orchbee_client_server_protocol::{
        ClientMessage, ServerMessage, TerminalLaunchReadOutcome, TerminalLaunchReadResult,
        decode_client_frame, encode_server_frame,
    };
    use orchbee_workbench_protocol::{ConfigurationAction, ConfigurationRequest, SessionId};
    fn probe_response(open: &crate::local_client::tests::multiplex_peer::Open) -> Vec<u8> {
        let ClientMessage::TerminalLaunchRead(request) =
            decode_client_frame(&open.request).unwrap()
        else {
            panic!("bounded read probe expected");
        };
        encode_server_frame(&ServerMessage::TerminalLaunchReadResult(
            TerminalLaunchReadResult::new(
                request.request_id().clone(),
                TerminalLaunchReadOutcome::NotFound,
            ),
        ))
        .unwrap()
    }
    fn respond_probe(
        peer: &mut LocalStream,
        id: MultiplexStreamId,
        record: Vec<u8>,
    ) -> io::Result<()> {
        write_multiplex_message(
            peer,
            &MultiplexMessage::StreamFrame {
                stream_id: id,
                sequence: NonZeroU64::MIN,
                kind: StreamFrameKind::Result,
                record,
            },
        )
    }
    let (done, finish) = mpsc::sync_channel(1);
    let (recovered, recovery) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut peer| {
        admit(&mut peer, index as u64 + 1)?;
        let watch = read_open(&mut peer)?;
        assert_eq!(watch.mode, StreamMode::State);
        frame(
            &mut peer,
            watch.id,
            1,
            StreamFrameKind::Snapshot,
            index as u8 + 1,
        )?;
        if index == 1 {
            // A replayed Reload would precede the next health probe and fail this decoder.
            let probe = read_open(&mut peer)?;
            respond_probe(&mut peer, probe.id, probe_response(&probe))?;
            recovered.send(()).unwrap();
            finish.recv_timeout(Duration::from_secs(2)).unwrap();
            return Ok(());
        }
        let mutation = read_open(&mut peer)?;
        let parsed = orchbee_workbench_protocol::read_workbench_request(&mut io::Cursor::new(
            &mutation.request,
        ))
        .unwrap();
        assert!(matches!(
            parsed,
            WorkbenchRequest::Configuration(ConfigurationRequest {
                action: ConfigurationAction::Reload,
                ..
            })
        ));
        let first = read_open(&mut peer)?;
        let old_reply = probe_response(&first);
        respond_probe(&mut peer, first.id, old_reply.clone())?;
        let pending = read_open(&mut peer)?;
        assert!(pending.id.value() > first.id.value());
        let _valid_pending = probe_response(&pending);
        let started = Instant::now();
        peer.set_read_timeout(Some(Duration::from_millis(20)))?;
        let mut sequence = 1;
        loop {
            // Neither an already retired probe reply nor another active stream's progress
            // may settle/extend the queued probe. Keep both flowing until the carrier closes.
            if respond_probe(&mut peer, first.id, old_reply.clone()).is_err() {
                break;
            }
            sequence += 1;
            if frame(&mut peer, watch.id, sequence, StreamFrameKind::Update, 1).is_err() {
                break;
            }
            match peer.read(&mut [0; 1]) {
                Ok(0) => break,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
                    ) =>
                {
                    break;
                }
                result => panic!("no further application request before probe expires: {result:?}"),
            }
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "unrelated progress prolonged the probe"
            );
        }
        assert!(started.elapsed() < Duration::from_secs(1));
        Ok(())
    })
    .unwrap();
    let mut client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let Some(crate::local_client::ClientTransport::Direct(transport)) = client.connection.take()
    else {
        panic!()
    };
    let owner = Connection::start_with_timing(
        transport,
        client.origin.clone(),
        health::Timing {
            idle: Duration::from_millis(60),
            response: Duration::from_millis(180),
        },
    )
    .unwrap();
    client.origin.multiplex = Some(Arc::clone(&owner));
    client.connection = Some(crate::local_client::ClientTransport::Lane(
        crate::local_client::ClientLane::new(Arc::clone(&owner)).unwrap(),
    ));
    let watch = client
        .origin()
        .subscribe(&request(1), StreamMode::State)
        .unwrap();
    let mutation = client
        .origin()
        .subscribe(
            &WorkbenchRequest::Configuration(ConfigurationRequest {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 2)).unwrap(),
                session_id: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
                action: ConfigurationAction::Reload,
            }),
            StreamMode::Request,
        )
        .unwrap();
    let mut reset = false;
    loop {
        match take(&watch).unwrap() {
            StreamEvent::Disconnected => reset = true,
            StreamEvent::Record(bytes) if bytes == [2] => break,
            StreamEvent::Record(_) => {}
            StreamEvent::Gap => panic!("transport deadline is a reset"),
        }
    }
    assert!(
        reset,
        "fresh generation cannot erase the withdrawal barrier"
    );
    assert!(mutation.poll().is_err());
    assert!(
        mutation.cancel(),
        "the old peer received Reload, so its result remains unknown"
    );
    recovery.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(owner.is_current(Instant::now()));
    done.send(()).unwrap();
    drop((watch, mutation, client, owner));
    server.finish().unwrap();
}

#[test]
fn an_offline_request_correlates_with_the_generation_that_first_writes_it() {
    use orchbee_workbench_protocol::{
        ClientViewAction, ClientViewAvailability, ClientViewOutcome, ClientViewRecord,
        WorkbenchClientId, WorkbenchClientViewResult, WorkbenchResponse, parse_workbench_request,
        read_workbench_request,
    };
    let (waiting, admission) = mpsc::sync_channel(1);
    let (close_first, first_closed) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(3, move |index, mut peer| {
        if index == 0 {
            admit(&mut peer, 1)?;
            let initial = read_open(&mut peer)?;
            frame(&mut peer, initial.id, 1, StreamFrameKind::Snapshot, 1)?;
            first_closed.recv_timeout(Duration::from_secs(2)).unwrap();
            return Ok(());
        }
        if index == 1 {
            assert!(matches!(
                orchbee_local_client_protocol::read_message(&mut peer).map_err(io::Error::other)?,
                orchbee_local_client_protocol::LocalClientMessage::ClientHello(_)
            ));
            waiting.send(()).unwrap();
            // Queuing the offline request wakes and cancels this unfinished admission.
            // The central actor alone admits the next carrier and writes the queued request.
            assert_eq!(peer.read(&mut [0; 1])?, 0);
            return Ok(());
        }
        admit(&mut peer, 2)?;
        loop {
            let open = read_open(&mut peer)?;
            let request = read_workbench_request(&mut io::Cursor::new(&open.request)).unwrap();
            match request {
                WorkbenchRequest::SnapshotRead(_) => {
                    frame(&mut peer, open.id, 1, StreamFrameKind::Snapshot, 2)?;
                }
                WorkbenchRequest::ClientView(request) => {
                    assert_eq!(open.mode, StreamMode::Request);
                    let ClientViewAction::Synchronize {
                        heartbeat,
                        view_revision,
                        view,
                        ..
                    } = request.action
                    else {
                        panic!()
                    };
                    crate::local_client::tests::multiplex_peer::respond(
                        &mut peer,
                        &open,
                        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            session_id: request.session_id,
                            outcome: ClientViewOutcome::Synchronized {
                                client: Box::new(ClientViewRecord {
                                    client_id: WorkbenchClientId::parse(&format!("{:032x}", 2))
                                        .unwrap(),
                                    heartbeat,
                                    view_revision,
                                    view: *view,
                                    availability: ClientViewAvailability::Current,
                                }),
                                control: None,
                            },
                        }),
                        1,
                    )?;
                    return Ok(());
                }
                _ => panic!("only the retained observer and one new request"),
            }
        }
    })
    .unwrap();
    let mut client = LocalClient::connect(server.endpoint(), "test-cohort")
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let origin = client.origin();
    let watcher = origin.subscribe(&request(1), StreamMode::State).unwrap();
    assert!(matches!(take(&watcher).unwrap(), StreamEvent::Record(_)));
    close_first.send(()).unwrap();
    assert!(matches!(take(&watcher).unwrap(), StreamEvent::Disconnected));
    admission.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        client.generation().unwrap().get(),
        1,
        "new admission is still withheld"
    );
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let WorkbenchRequest::ClientView(request) =
        parse_workbench_request(&serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap())
            .unwrap()
    else {
        panic!()
    };
    let result = client
        .exchange_client_view(request, Instant::now() + Duration::from_secs(2), None)
        .unwrap();
    assert!(
        matches!(result.outcome, ClientViewOutcome::Synchronized { client, .. }
        if client.client_id.as_str() == format!("{:032x}", 2))
    );
    drop((watcher, origin, client));
    server.finish().unwrap();
}
