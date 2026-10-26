use super::*;
use orchbee_workbench_protocol::{
    MultiplexStreamId, ProtocolVersion, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    write_workbench_request,
};
use std::sync::mpsc;

fn open(id: u64, lane: u64, mode: StreamMode) -> MultiplexMessage {
    let mut request = Vec::new();
    write_workbench_request(
        &mut request,
        &WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
        }),
    )
    .unwrap();
    MultiplexMessage::StreamOpen {
        stream_id: MultiplexStreamId::new(id).unwrap(),
        lane_id: MultiplexLaneId::new(lane).unwrap(),
        mode,
        request,
    }
}
fn bounded_pair() -> (LocalStream, LocalStream) {
    let (peer, server) = LocalStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    (peer, server)
}

#[test]
fn late_scope_completion_is_reclaimed_without_releasing_a_sibling_owner() {
    let (mut peer, server) = bounded_pair();
    let owners = Arc::new(Mutex::new(BTreeSet::new()));
    let capture = Arc::clone(&owners);
    let release = Arc::clone(&owners);
    let (started, executing) = mpsc::sync_channel(1);
    let (resume, completed) = mpsc::sync_channel(1);
    let (closed, cancelled) = mpsc::sync_channel(1);
    let task = thread::spawn(move || {
        run(
            &server,
            MultiplexMessage::LaneOpen {
                lane_id: MultiplexLaneId::new(2).unwrap(),
            },
            Arc::default(),
            move |lane, _| {
                if lane.value() == 2 {
                    started.send(()).unwrap();
                    completed.recv_timeout(Duration::from_secs(2)).unwrap();
                }
                capture.lock().unwrap().insert(lane);
                Ok(vec![1])
            },
            move |lane| {
                release.lock().unwrap().remove(&lane);
                closed.send(lane).unwrap();
            },
            |_| {},
        )
    });
    write_multiplex_message(&mut peer, &open(1, 2, StreamMode::Request)).unwrap();
    executing.recv_timeout(Duration::from_secs(2)).unwrap();
    write_multiplex_message(
        &mut peer,
        &MultiplexMessage::LaneOpen {
            lane_id: MultiplexLaneId::new(3).unwrap(),
        },
    )
    .unwrap();
    write_multiplex_message(&mut peer, &open(2, 3, StreamMode::Request)).unwrap();
    write_multiplex_message(
        &mut peer,
        &MultiplexMessage::LaneClose {
            lane_id: MultiplexLaneId::new(2).unwrap(),
        },
    )
    .unwrap();
    resume.send(()).unwrap();
    assert!(
        matches!(read_multiplex_message(&mut peer).unwrap(), MultiplexMessage::StreamFrame { stream_id, .. } if stream_id.value() == 1)
    );
    assert!(
        matches!(read_multiplex_message(&mut peer).unwrap(), MultiplexMessage::StreamFrame { stream_id, .. } if stream_id.value() == 2)
    );
    assert_eq!(
        cancelled
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .value(),
        2
    );
    assert_eq!(
        *owners.lock().unwrap(),
        BTreeSet::from([MultiplexLaneId::new(3).unwrap()])
    );
    write_multiplex_message(&mut peer, &open(3, 2, StreamMode::Request)).unwrap();
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::Disconnect {
            reason: MultiplexDisconnectReason::InvalidStream
        }
    ));
    assert_eq!(
        *owners.lock().unwrap(),
        BTreeSet::from([MultiplexLaneId::new(3).unwrap()]),
        "a late request cannot resurrect the closed lane"
    );
    peer.shutdown(std::net::Shutdown::Both).unwrap();
    let _closed = task.join().unwrap();
}

#[test]
fn a_slow_writer_cannot_hide_a_commit_during_another_clients_snapshot_capture() {
    let source = Arc::new(orchbee_server::SessionChangeSource::default());
    let slow_signal = Arc::new(SessionChangeSignal::default());
    let fast_signal = Arc::new(SessionChangeSignal::default());
    source.watch(&slow_signal);
    source.watch(&fast_signal);
    let (slow_peer, slow_server) = bounded_pair();
    let (mut fast_peer, fast_server) = bounded_pair();
    let (entered, captured) = mpsc::sync_channel(1);
    let slow = thread::spawn(move || {
        run(
            &slow_server,
            open(1, 1, StreamMode::State),
            slow_signal,
            |_, _| {
                entered.send(()).unwrap();
                Ok(vec![0; 4 * 1024 * 1024])
            },
            |_| {},
            |_| {},
        )
    });
    captured.recv_timeout(Duration::from_secs(2)).unwrap();
    let publisher = Arc::clone(&source);
    let fast = thread::spawn(move || {
        let mut revision = 0u8;
        run(
            &fast_server,
            open(1, 1, StreamMode::State),
            fast_signal,
            |_, _| {
                let captured = revision;
                if revision == 0 {
                    revision = 1;
                    publisher.notify();
                }
                Ok(vec![captured])
            },
            |_| {},
            |_| {},
        )
    });
    assert!(
        matches!(read_multiplex_message(&mut fast_peer).unwrap(), MultiplexMessage::StreamFrame { kind: StreamFrameKind::Snapshot, record, .. } if record == [0])
    );
    assert!(
        matches!(read_multiplex_message(&mut fast_peer).unwrap(), MultiplexMessage::StreamFrame { kind: StreamFrameKind::Update, record, .. } if record == [1])
    );
    // The slow carrier is still undrained. Both readers are stopped before their owners join.
    slow_peer.shutdown(std::net::Shutdown::Both).unwrap();
    fast_peer.shutdown(std::net::Shutdown::Both).unwrap();
    let _closed = slow.join().unwrap();
    let _closed = fast.join().unwrap();
}

#[test]
fn sustained_event_overrun_disconnects_only_its_carrier_with_a_typed_reason() {
    let (mut peer, server) = bounded_pair();
    let task = thread::spawn(move || {
        run(
            &server,
            open(1, 1, StreamMode::Events),
            Arc::default(),
            |_, _| Ok(vec![1]),
            |_| {},
            |_| {},
        )
    });
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::StreamFrame {
            kind: StreamFrameKind::Snapshot,
            ..
        }
    ));
    let stream_id = MultiplexStreamId::new(1).unwrap();
    for _ in 0..2 {
        write_multiplex_message(&mut peer, &MultiplexMessage::Resnapshot { stream_id }).unwrap();
        assert!(matches!(
            read_multiplex_message(&mut peer).unwrap(),
            MultiplexMessage::Gap { .. }
        ));
        assert!(matches!(
            read_multiplex_message(&mut peer).unwrap(),
            MultiplexMessage::StreamFrame {
                kind: StreamFrameKind::Snapshot,
                ..
            }
        ));
    }
    write_multiplex_message(&mut peer, &MultiplexMessage::Resnapshot { stream_id }).unwrap();
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::Disconnect {
            reason: MultiplexDisconnectReason::SlowConsumer
        }
    ));
    let _closed = task.join().unwrap();
}

#[test]
fn an_active_event_cursor_rejects_competing_streams_and_one_shot_reads() {
    use orchbee_workbench_protocol::{
        SemanticEventFilter, SemanticEventKind, SemanticEventMatch, SemanticEventScope,
        SemanticEventsAction, SemanticEventsRequest, SessionId,
    };
    let definition = WorkbenchRequestId::parse("00000000000000000000000000000007").unwrap();
    let encode = |id, mode, action| {
        let mut request = Vec::new();
        write_workbench_request(
            &mut request,
            &WorkbenchRequest::Events(SemanticEventsRequest {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
                session_id: SessionId::parse("00000000000000000000000000000001").unwrap(),
                action,
            }),
        )
        .unwrap();
        MultiplexMessage::StreamOpen {
            stream_id: MultiplexStreamId::new(id).unwrap(),
            lane_id: MultiplexLaneId::ROOT,
            mode,
            request,
        }
    };
    let subscribe = SemanticEventsAction::Subscribe {
        subscription_id: definition.clone(),
        filters: vec![SemanticEventFilter::Event {
            event: SemanticEventKind::WorkspaceCreated,
            scope: SemanticEventScope::All {},
            condition: SemanticEventMatch::Any {},
        }],
        after: None,
    };
    let first = encode(1, StreamMode::Events, subscribe.clone());
    let (mut peer, server) = bounded_pair();
    let executions = Arc::new(AtomicUsize::new(0));
    let executed = Arc::clone(&executions);
    let task = thread::spawn(move || {
        run(
            &server,
            first,
            Arc::default(),
            |_, _| {
                executed.fetch_add(1, Ordering::AcqRel);
                Ok(vec![1])
            },
            |_| {},
            |_| {},
        )
    });
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::StreamFrame {
            kind: StreamFrameKind::Snapshot,
            ..
        }
    ));
    for (id, mode, action) in [
        (2, StreamMode::Events, subscribe),
        (
            3,
            StreamMode::Request,
            SemanticEventsAction::Read {
                subscription_id: definition,
                limit: 1,
            },
        ),
    ] {
        write_multiplex_message(&mut peer, &encode(id, mode, action)).unwrap();
        assert!(matches!(read_multiplex_message(&mut peer).unwrap(),
            MultiplexMessage::StreamClose { stream_id, reason: StreamCloseReason::InvalidRequest }
                if stream_id.value() == id));
    }
    assert_eq!(executions.load(Ordering::Acquire), 1);
    peer.shutdown(std::net::Shutdown::Both).unwrap();
    let _closed = task.join().unwrap();
}
