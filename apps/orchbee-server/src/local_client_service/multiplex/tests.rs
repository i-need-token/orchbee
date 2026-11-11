use super::*;
use orchbee_workbench_protocol::{
    MultiplexStreamId, ProtocolVersion, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    write_workbench_request,
};
use std::sync::mpsc;

mod preferences;

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
fn no_job_read(_: &ApplicationRequest) -> Result<Vec<u8>, ApplicationConnectionError> {
    panic!("these requests stay on the dispatcher")
}
fn bounded_pair() -> (LocalStream, LocalStream) {
    let (peer, server) = LocalStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    (peer, server)
}

#[test]
fn a_drain_refuses_one_intent_stream_while_the_connection_keeps_serving() {
    let (mut peer, server) = bounded_pair();
    let mut executions = 0;
    let task = thread::spawn(move || {
        run(
            &server,
            open(1, 1, StreamMode::Request),
            Arc::default(),
            move |_, _| {
                executions += 1;
                if executions == 1 {
                    Err(ApplicationConnectionError::Draining)
                } else {
                    Ok(vec![2])
                }
            },
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || Ok(None),
        )
    });
    assert_eq!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::StreamClose {
            stream_id: MultiplexStreamId::new(1).unwrap(),
            reason: StreamCloseReason::Stopping,
        }
    );
    write_multiplex_message(&mut peer, &open(2, 1, StreamMode::Request)).unwrap();
    let MultiplexMessage::StreamFrame {
        stream_id, record, ..
    } = read_multiplex_message(&mut peer).unwrap()
    else {
        panic!("result frame");
    };
    assert_eq!((stream_id.value(), record), (2, vec![2]));
    drop(peer);
    let _closed = task.join().unwrap();
}

#[test]
fn a_waiting_job_read_never_delays_another_request_on_the_connection() {
    use orchbee_workbench_protocol::{SessionId, WorkbenchWorkspaceId, WorkspaceGitRequest};
    let (mut peer, server) = bounded_pair();
    let (release, released) = mpsc::channel::<()>();
    let released = Mutex::new(released);
    let mut git = Vec::new();
    write_workbench_request(
        &mut git,
        &WorkbenchRequest::WorkspaceGit(WorkspaceGitRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
            session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 3)).unwrap(),
        }),
    )
    .unwrap();
    let task = thread::spawn(move || {
        run(
            &server,
            MultiplexMessage::StreamOpen {
                stream_id: MultiplexStreamId::new(1).unwrap(),
                lane_id: MultiplexLaneId::ROOT,
                mode: StreamMode::Request,
                request: git,
            },
            Arc::default(),
            |_, _| Ok(vec![2]),
            Vec::new,
            |_| {
                released.lock().unwrap().recv().unwrap();
                Ok(vec![1])
            },
            |_| {},
            |_| {},
            || Ok(None),
        )
    });
    write_multiplex_message(&mut peer, &open(2, 1, StreamMode::Request)).unwrap();
    let MultiplexMessage::StreamFrame {
        stream_id, record, ..
    } = read_multiplex_message(&mut peer).unwrap()
    else {
        panic!("result frame");
    };
    assert_eq!((stream_id.value(), record), (2, vec![2]));
    release.send(()).unwrap();
    let MultiplexMessage::StreamFrame {
        stream_id,
        kind,
        record,
        ..
    } = read_multiplex_message(&mut peer).unwrap()
    else {
        panic!("job result frame");
    };
    assert_eq!(
        (stream_id.value(), kind, record),
        (1, StreamFrameKind::Result, vec![1])
    );
    drop(peer);
    let _closed = task.join().unwrap();
}

#[cfg(windows)]
#[test]
fn idle_metadata_expiry_rearms_after_a_same_value_report_without_client_polling() {
    use crate::{
        local_terminal_service::LocalTerminalService,
        workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService},
        worker_registry::WorkerRegistry,
    };
    use orchbee_workbench_protocol::{
        MetadataSource, MetadataTokenKey, MetadataTokenPatch, MetadataTokenValue,
        WorkbenchResponse, WorkbenchSnapshotOutcome, WorkspaceMetadataAction,
        WorkspaceMetadataOutcome, WorkspaceMetadataRequest, write_workbench_response,
    };
    let service = Arc::new(WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::new(LocalTerminalService::workspace_fixture(Arc::new(
            Mutex::new(WorkerRegistry::new()),
        ))),
    ));
    let WorkbenchExecution::Response(WorkbenchResponse::Snapshot(snapshot)) =
        service.execute(WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        }))
    else {
        panic!("snapshot");
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = snapshot.outcome else {
        panic!("available");
    };
    let workspace = snapshot.navigation.workspaces[0].workspace_id.clone();
    let report = |ttl| {
        WorkbenchRequest::WorkspaceMetadata(WorkspaceMetadataRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 2)).unwrap(),
            session_id: snapshot.session_id.clone(),
            action: WorkspaceMetadataAction::Report {
                workspace_id: workspace.clone(),
                source: MetadataSource::parse("ttl-witness").unwrap(),
                tokens: MetadataTokenPatch::new(
                    [(
                        MetadataTokenKey::parse("summary").unwrap(),
                        Some(MetadataTokenValue::parse("same").unwrap()),
                    )]
                    .into(),
                )
                .unwrap(),
                sequence: None,
                ttl_milliseconds: Some(ttl),
            },
        })
    };
    let publisher = service.client_connection(NonZeroU64::new(2).unwrap());
    assert!(
        matches!(publisher.execute(report(10_000)), WorkbenchExecution::Response(WorkbenchResponse::WorkspaceMetadata(result)) if matches!(result.outcome, WorkspaceMetadataOutcome::Reported { .. }))
    );
    let (mut peer, server) = bounded_pair();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let reader_service = Arc::clone(&service);
    let signal = Arc::new(SessionChangeSignal::default());
    let observer = Arc::clone(&signal);
    let task = thread::spawn(move || {
        let connection = reader_service.client_connection(NonZeroU64::MIN);
        connection.watch_changes(&observer);
        run(
            &server,
            open(1, 1, StreamMode::Invalidations),
            observer,
            |_, request| {
                let WorkbenchExecution::Response(response) =
                    connection.execute(request.workbench().unwrap().clone())
                else {
                    panic!("read outcome");
                };
                let mut bytes = Vec::new();
                write_workbench_response(&mut bytes, &response)
                    .map_err(ApplicationConnectionError::Workbench)?;
                Ok(bytes)
            },
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || connection.maintain_subscription_deadline(),
        )
    });
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::StreamFrame {
            kind: StreamFrameKind::Snapshot,
            ..
        }
    ));
    let before = signal.revision();
    // Values do not change, so the semantic journal intentionally does not publish a token
    // event. The service's generic report notification must still rearm the earlier expiry.
    assert!(
        matches!(publisher.execute(report(500)), WorkbenchExecution::Response(WorkbenchResponse::WorkspaceMetadata(result)) if matches!(result.outcome, WorkspaceMetadataOutcome::Reported { .. }))
    );
    assert!(signal.revision() > before);
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::StreamFrame {
            kind: StreamFrameKind::Update,
            ..
        }
    ));
    // No more peer request, health probe, or metadata read occurs before this expiry update.
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::StreamFrame {
            kind: StreamFrameKind::Update,
            ..
        }
    ));
    let WorkbenchExecution::Response(WorkbenchResponse::WorkspaceMetadata(result)) = publisher
        .execute(WorkbenchRequest::WorkspaceMetadata(
            WorkspaceMetadataRequest {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
                session_id: snapshot.session_id,
                action: WorkspaceMetadataAction::Read {
                    workspace_ids: vec![workspace],
                },
            },
        ))
    else {
        panic!("metadata");
    };
    assert!(
        matches!(result.outcome, WorkspaceMetadataOutcome::Read { workspaces, .. } if workspaces[0].tokens.as_map().is_empty())
    );
    assert_eq!(publisher.maintain_subscription_deadline().unwrap(), None);
    peer.shutdown(std::net::Shutdown::Both).unwrap();
    let _closed = task.join().unwrap();
}

#[test]
fn maintenance_failure_closes_instead_of_retrying_a_past_deadline() {
    let (mut peer, server) = bounded_pair();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let task = thread::spawn(move || {
        run(
            &server,
            open(1, 1, StreamMode::State),
            Arc::default(),
            |_, _| Ok(vec![1]),
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || {
                if count.fetch_add(1, Ordering::AcqRel) == 0 {
                    Ok(Some(Instant::now()))
                } else {
                    Err(io::ErrorKind::ConnectionAborted.into())
                }
            },
        )
    });
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::StreamFrame {
            kind: StreamFrameKind::Snapshot,
            ..
        }
    ));
    assert!(matches!(
        read_multiplex_message(&mut peer).unwrap(),
        MultiplexMessage::Disconnect {
            reason: MultiplexDisconnectReason::Unavailable
        }
    ));
    assert!(task.join().unwrap().is_err());
    assert_eq!(calls.load(Ordering::Acquire), 2);
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
            Vec::new,
            no_job_read,
            move |lane| {
                release.lock().unwrap().remove(&lane);
                closed.send(lane).unwrap();
            },
            |_| {},
            || Ok(None),
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
    // macOS reports NotConnected once the Server has already closed; Linux shuts down quietly.
    match peer.shutdown(std::net::Shutdown::Both) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotConnected => {}
        Err(error) => panic!("peer shutdown: {error}"),
    }
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
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || Ok(None),
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
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || Ok(None),
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
fn terminal_output_wakes_value_streams_without_an_invalidation_hint() {
    let (mut peer, server) = bounded_pair();
    let signal = Arc::new(SessionChangeSignal::default());
    let publisher = Arc::clone(&signal);
    let task = thread::spawn(move || {
        let mut value = 0u8;
        run(
            &server,
            open(1, 1, StreamMode::Invalidations),
            signal,
            |_, _| {
                value += 1;
                Ok(vec![value])
            },
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || Ok(None),
        )
    });
    fn frame(peer: &mut LocalStream, expected: u64, kind: StreamFrameKind) -> Vec<u8> {
        let MultiplexMessage::StreamFrame {
            stream_id,
            kind: found,
            record,
            ..
        } = read_multiplex_message(peer).unwrap()
        else {
            panic!("stream frame");
        };
        assert_eq!((stream_id.value(), found), (expected, kind));
        record
    }
    frame(&mut peer, 1, StreamFrameKind::Snapshot);
    write_multiplex_message(&mut peer, &open(2, 1, StreamMode::State)).unwrap();
    frame(&mut peer, 2, StreamFrameKind::Snapshot);
    // Streams evaluate in identity order: reaching the value stream first proves the invalidation
    // stream was evaluated and had nothing to say.
    publisher.notify_output();
    frame(&mut peer, 2, StreamFrameKind::Update);
    publisher.notify();
    assert_eq!(
        frame(&mut peer, 1, StreamFrameKind::Update),
        1_u64.to_be_bytes(),
        "the hint counts commits, not output frames"
    );
    frame(&mut peer, 2, StreamFrameKind::Update);
    peer.shutdown(std::net::Shutdown::Both).unwrap();
    let _closed = task.join().unwrap();
}

/// A published-title read follows title changes and commits; every other stream ignores title
/// changes, and output never reevaluates the title read.
#[test]
fn title_changes_wake_only_the_published_title_read() {
    use orchbee_workbench_protocol::{
        PanelAgentAction, PanelAgentRequest, SessionId, TerminalPresentationId,
    };
    let (mut peer, server) = bounded_pair();
    let signal = Arc::new(SessionChangeSignal::default());
    let publisher = Arc::clone(&signal);
    let task = thread::spawn(move || {
        let mut value = 0u8;
        run(
            &server,
            open(1, 1, StreamMode::State),
            signal,
            |_, _| {
                value += 1;
                Ok(vec![value])
            },
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || Ok(None),
        )
    });
    fn frame(peer: &mut LocalStream, expected: u64, kind: StreamFrameKind) {
        let MultiplexMessage::StreamFrame {
            stream_id,
            kind: found,
            ..
        } = read_multiplex_message(peer).unwrap()
        else {
            panic!("stream frame");
        };
        assert_eq!((stream_id.value(), found), (expected, kind));
    }
    frame(&mut peer, 1, StreamFrameKind::Snapshot);
    let mut request = Vec::new();
    write_workbench_request(
        &mut request,
        &WorkbenchRequest::PanelAgent(PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 2)).unwrap(),
            session_id: SessionId::parse(&format!("{:032x}", 3)).unwrap(),
            action: PanelAgentAction::PresentationTitles {
                presentation_ids: vec![
                    TerminalPresentationId::parse(&format!("{:032x}", 4)).unwrap(),
                ],
            },
        }),
    )
    .unwrap();
    write_multiplex_message(
        &mut peer,
        &MultiplexMessage::StreamOpen {
            stream_id: MultiplexStreamId::new(2).unwrap(),
            lane_id: MultiplexLaneId::new(1).unwrap(),
            mode: StreamMode::State,
            request,
        },
    )
    .unwrap();
    frame(&mut peer, 2, StreamFrameKind::Snapshot);
    // Streams evaluate in identity order and every evaluation yields a new value, so the first
    // frame after each signal names the first stream that reevaluated.
    publisher.notify_titles();
    frame(&mut peer, 2, StreamFrameKind::Update);
    publisher.notify_output();
    frame(&mut peer, 1, StreamFrameKind::Update);
    publisher.notify();
    frame(&mut peer, 1, StreamFrameKind::Update);
    frame(&mut peer, 2, StreamFrameKind::Update);
    peer.shutdown(std::net::Shutdown::Both).unwrap();
    let _closed = task.join().unwrap();
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
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || Ok(None),
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
            Vec::new,
            no_job_read,
            |_| {},
            |_| {},
            || Ok(None),
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
