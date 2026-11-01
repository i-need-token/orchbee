//! Native pipe oracle: source identity/read acknowledgement precede one editor effect.
use super::*;
use orchbee_workbench_protocol::{
    ScrollbackEditorSpec, TerminalEditorAction, TerminalEditorFailure, TerminalEditorOutcome,
    TerminalEditorResult,
};

pub(super) fn spec() -> ScrollbackEditorSpec {
    serde_json::from_value(serde_json::json!({"source":{
        "terminalContextId":id(4),"tabId":id(5),"panelId":id(6),
        "member":{"presentationId":id(7),"binding":{"workerId":id(8),"terminalId":id(9)},
            "attachmentGeneration":"1","incarnation":0,
            "viewport":{"state":"unavailable","reason":"frame-unavailable"}}
    }}))
    .unwrap()
}
#[derive(Clone, Copy)]
pub(super) enum OpenMode {
    Accept,
    Refuse,
    Unknown,
    WrongEcho,
    Hold,
}

fn viewer_request(stream: &mut LocalStream) -> io::Result<TerminalViewerRequest> {
    match read_client_message(stream).map_err(io::Error::other)? {
        ClientMessage::TerminalViewer(request) => Ok(request),
        other => panic!("source must stay read-only, got {other:?}"),
    }
}
fn viewer_reply(
    stream: &mut LocalStream,
    request: &TerminalViewerRequest,
    outcome: TerminalViewerOutcome,
) -> io::Result<()> {
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .map_err(io::Error::other)
}
fn attached() -> TerminalViewerOutcome {
    let route = orchbee_terminal_protocol::inspect_terminal_frame_route(FRAME).unwrap();
    TerminalViewerOutcome::Attached {
        creation_id: orchbee_client_server_protocol::HostCreationId::parse(&"c".repeat(32))
            .unwrap(),
        attachment_id: ViewerId::parse(&id(40)).unwrap(),
        host_epoch: orchbee_client_server_protocol::HostEpoch::parse(&route.host_epoch.to_text())
            .unwrap(),
        host_terminal_id: HostTerminalId::parse(&id(31)).unwrap(),
    }
}
pub(super) fn open_source(
    stream: &mut LocalStream,
    geometry: Geometry,
    mode: OpenMode,
    sent: Option<&mpsc::Sender<()>>,
) -> io::Result<()> {
    let request = viewer_request(stream)?;
    assert!(
        matches!(request.action(), TerminalViewerAction::AttachBinding { session_id, worker_id, terminal_id }
        if session_id.as_str()==id(2) && worker_id.as_str()==id(8) && terminal_id.as_str()==id(9))
    );
    viewer_reply(stream, &request, attached())?;
    let request = viewer_request(stream)?;
    assert!(
        matches!(request.action(), TerminalViewerAction::Poll { attachment_id } if attachment_id.as_str()==id(40))
    );
    let mut frame = FRAME.to_vec();
    frame[32..48].copy_from_slice(&40u128.to_be_bytes());
    frame[48..56].copy_from_slice(&1u64.to_le_bytes());
    frame[56..64].copy_from_slice(&u64::MAX.to_le_bytes());
    frame[64..72].copy_from_slice(&u64::MAX.to_le_bytes());
    frame[72..80].copy_from_slice(&u64::MAX.to_le_bytes());
    let route = orchbee_terminal_protocol::inspect_terminal_frame_route(&frame)
        .map_err(io::Error::other)?;
    let record = orchbee_terminal_protocol::encode_transport_envelope(
        &orchbee_terminal_protocol::Envelope {
            protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
            role: orchbee_terminal_protocol::MessageRole::Event,
            kind: 41,
            payload_encoding: orchbee_terminal_protocol::PayloadEncoding::TerminalFrame,
            host_epoch: Some(route.host_epoch),
            correlation_id: None,
            optional_message: false,
            payload: &frame,
        },
    )
    .map_err(io::Error::other)?;
    viewer_reply(stream, &request, TerminalViewerOutcome::Record(record))?;
    let request = viewer_request(stream)?;
    assert!(
        matches!(request.action(), TerminalViewerAction::FrameApplied { attachment_id, sequence, generation, revision }
        if attachment_id.as_str()==id(40) && sequence.get()==1 && generation.get()==u64::MAX && *revision==u64::MAX)
    );
    viewer_reply(stream, &request, TerminalViewerOutcome::FeedbackAccepted {})?;
    let WorkbenchRequest::TerminalEditor(request) =
        read_workbench_request(stream).map_err(io::Error::other)?
    else {
        panic!("editor Open after source acknowledgement")
    };
    assert!(
        matches!(&request.action, TerminalEditorAction::Open { popup_id, spec: selected, capture, geometry: actual }
        if popup_id.as_str()==id(10) && selected.as_ref()==&spec() && capture.attachment_id.as_str()==id(40)
        && capture.generation.value()==u64::MAX && capture.revision.value()==u64::MAX
        && actual.columns()==geometry.columns() && actual.rows()==geometry.rows())
    );
    if let Some(sent) = sent {
        sent.send(()).unwrap();
    }
    if matches!(mode, OpenMode::Hold) {
        assert_eq!(stream.read(&mut [0; 1])?, 0);
        return Ok(());
    }
    let outcome = match mode {
        OpenMode::Accept => TerminalEditorOutcome::Opened {
            popup_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
            title: "Trusted dashboard".into(),
        },
        OpenMode::Refuse => TerminalEditorOutcome::Failed {
            reason: TerminalEditorFailure::EditorUnavailable,
        },
        OpenMode::Unknown => TerminalEditorOutcome::Unknown {
            popup_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
        },
        OpenMode::WrongEcho => TerminalEditorOutcome::Opened {
            popup_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
            title: "Wrong echo".into(),
        },
        OpenMode::Hold => unreachable!(),
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::TerminalEditor(TerminalEditorResult {
            protocol_version: ProtocolVersion,
            session_id: request.session_id,
            request_id: if matches!(mode, OpenMode::WrongEcho) {
                WorkbenchRequestId::parse(&id(59)).unwrap()
            } else {
                request.request_id
            },
            outcome,
        }),
    )
    .map_err(io::Error::other)?;
    if matches!(mode, OpenMode::Unknown | OpenMode::WrongEcho) {
        assert_eq!(stream.read(&mut [0; 1])?, 0);
        return Ok(());
    }
    let request = viewer_request(stream)?;
    assert!(
        matches!(request.action(),TerminalViewerAction::Detach { attachment_id } if attachment_id.as_str()==id(40))
    );
    viewer_reply(stream, &request, TerminalViewerOutcome::Detached {})
}
pub(super) fn lifecycle_reply(stream: &mut LocalStream, close: bool, done: bool) -> io::Result<()> {
    let WorkbenchRequest::TerminalEditor(request) =
        read_workbench_request(stream).map_err(io::Error::other)?
    else {
        panic!("typed editor lifecycle")
    };
    assert!(match &request.action {
        TerminalEditorAction::Close { popup_id } => close && popup_id.as_str() == id(10),
        TerminalEditorAction::Read { popup_id } => !close && popup_id.as_str() == id(10),
        _ => false,
    });
    let popup_id = WorkbenchRequestId::parse(&id(10)).unwrap();
    let outcome = if close {
        TerminalEditorOutcome::Closing { popup_id }
    } else if done {
        TerminalEditorOutcome::Closed { popup_id }
    } else {
        TerminalEditorOutcome::State {
            popup_id,
            settled: true,
        }
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::TerminalEditor(TerminalEditorResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }),
    )
    .map_err(io::Error::other)
}

#[test]
fn editor_unbound_source_is_refused_before_connection_or_lane() {
    let mut driver = Driver::new(clock());
    let unbound = ScrollbackEditorSpec {
        source: super::spec().source,
    };
    assert_eq!(
        driver.begin_editor(
            SessionId::parse(&id(2)).unwrap(),
            WorkbenchRequestId::parse(&id(10)).unwrap(),
            unbound
        ),
        Err(Reason::StaleView)
    );
    assert!(driver.lane.is_none());
    assert!(!driver.busy());
}
#[test]
fn editor_refusal_unknown_wrong_echo_and_shutdown_never_repeat_open() -> TestResult {
    for mode in [
        OpenMode::Refuse,
        OpenMode::Unknown,
        OpenMode::WrongEcho,
        OpenMode::Hold,
    ] {
        let frame = orchbee_terminal_protocol::decode_terminal_text_frame(FRAME)?;
        let geometry = Geometry::new(frame.geometry.columns, frame.geometry.rows, 0, 0)?;
        let (sent, received) = mpsc::channel();
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            admit(&mut stream)?;
            if index == 0 {
                return Ok(());
            }
            open_source(&mut stream, geometry, mode, Some(&sent))?;
            if matches!(mode, OpenMode::Refuse) {
                assert_eq!(stream.read(&mut [0; 1])?, 0);
            }
            Ok(())
        })?;
        let client = LocalClient::connect(server.endpoint(), "editor-fault-test")?;
        let origin = client.origin();
        drop(client);
        let mut driver = Driver::new(clock());
        driver.set_origin(origin);
        driver.observe_surface(
            Rect::new(0, 0, geometry.columns() + 2, geometry.rows() + 2),
            (0, 0),
        );
        driver
            .begin_editor(
                SessionId::parse(&id(2))?,
                WorkbenchRequestId::parse(&id(10))?,
                spec(),
            )
            .map_err(|r| io::Error::other(format!("{r:?}")))?;
        received.recv_timeout(Duration::from_secs(3))?;
        if !matches!(mode, OpenMode::Hold) {
            wait_until(&mut driver, |driver| {
                matches!(
                    driver.latest.as_ref().map(|l| l.phase),
                    Some(Phase::Refused(_) | Phase::Failed(_))
                )
            });
            assert_eq!(driver.busy(), !matches!(mode, OpenMode::Refuse));
            assert_eq!(driver.failed(), !matches!(mode, OpenMode::Refuse));
        }
        let before = Instant::now();
        driver.shutdown();
        assert!(before.elapsed() < Duration::from_secs(2));
        server.finish()?;
    }
    Ok(())
}
#[test]
fn editor_close_during_source_poll_detaches_without_open_or_input() -> TestResult {
    let (sent, received) = mpsc::channel();
    let (proceed, gate) = mpsc::channel();
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        admit(&mut stream)?;
        if index == 0 {
            return Ok(());
        }
        let request = viewer_request(&mut stream)?;
        assert!(matches!(
            request.action(),
            TerminalViewerAction::AttachBinding { .. }
        ));
        viewer_reply(&mut stream, &request, attached())?;
        let request = viewer_request(&mut stream)?;
        assert!(matches!(
            request.action(),
            TerminalViewerAction::Poll { .. }
        ));
        sent.send(()).unwrap();
        gate.recv_timeout(Duration::from_secs(3)).unwrap();
        viewer_reply(&mut stream, &request, TerminalViewerOutcome::Empty {})?;
        let request = viewer_request(&mut stream)?;
        assert!(
            matches!(request.action(),TerminalViewerAction::Detach { attachment_id } if attachment_id.as_str()==id(40))
        );
        viewer_reply(&mut stream, &request, TerminalViewerOutcome::Detached {})?;
        assert_eq!(stream.read(&mut [0; 1])?, 0);
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "editor-close-prefix-test")?;
    let origin = client.origin();
    drop(client);
    let mut driver = Driver::new(clock());
    driver.set_origin(origin);
    driver.observe_surface(Rect::new(0, 0, 80, 24), (0, 0));
    driver
        .begin_editor(
            SessionId::parse(&id(2))?,
            WorkbenchRequestId::parse(&id(10))?,
            spec(),
        )
        .map_err(|r| io::Error::other(format!("{r:?}")))?;
    received.recv_timeout(Duration::from_secs(3))?;
    assert!(driver.begin_close());
    proceed.send(())?;
    wait_until(&mut driver, |driver| {
        matches!(driver.latest.as_ref().map(|l| l.phase), Some(Phase::Closed))
    });
    assert!(!driver.captured());
    assert!(!driver.failed());
    driver.shutdown();
    server.finish()?;
    Ok(())
}
