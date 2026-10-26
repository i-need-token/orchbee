use super::*;
use crate::local_client::tests::server::{FakeServer, adjustable_operation_clock};
use orchbee_client_server_protocol::{
    ClientMessage, ControllerId, HostTerminalId, ServerMessage, TerminalControlResult,
    TerminalViewerResult, ViewerId, read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    PluginResult, WorkbenchRequest, WorkbenchResponse, read_workbench_request,
    write_workbench_response,
};
use std::io::{self, Read};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const FRAME: &[u8] =
    include_bytes!("../../../../../schemas/terminal/v1/fixtures/ghostty-surface.bin");
fn id(value: u64) -> String {
    format!("{value:032x}")
}
fn spec() -> PluginPopupSpec {
    serde_json::from_value(serde_json::json!({"selector":{"pluginId":"tools","entrypointId":"dashboard"},
        "source":{"terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"member":{"presentationId":id(7),"selectionGeneration":"1"}},
        "context":{"tabId":id(5),"focusedPaneId":id(6)},"cwd":null,"environment":[],"width":null,"height":null})).unwrap()
}
fn clock() -> LocalOperationClock {
    adjustable_operation_clock(Arc::new(std::sync::atomic::AtomicU64::new(0)))
}
impl Driver {
    pub(in crate::workbench_client) fn composition_fixture(
        closing: Option<bool>,
        size: Option<PluginPopupSize>,
    ) -> Self {
        let mut driver = Self::new(clock());
        if let Some(closing) = closing {
            let mut spec = spec();
            spec.width = size;
            spec.height = size;
            driver.id = Some(WorkbenchRequestId::parse(&id(10)).unwrap());
            driver.spec = Some(PopupSpec::Plugin(Box::new(spec)));
            driver.latest = Some(Latest {
                serial: 1,
                phase: if closing {
                    Phase::Closing
                } else {
                    Phase::Opening
                },
                title: "Dashboard".into(),
                projection: None,
                geometry: Geometry::new(60, 20, 0, 0).unwrap(),
            });
        }
        driver
    }
}
fn admit(stream: &mut LocalStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::MIN,
        },
    )
    .map_err(io::Error::other)
}
fn plugin_reply(
    stream: &mut LocalStream,
    request: PluginRequest,
    outcome: PluginOutcome,
) -> io::Result<()> {
    write_workbench_response(
        stream,
        &WorkbenchResponse::Plugin(PluginResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }),
    )
    .map_err(io::Error::other)
}
fn wait_until(driver: &mut Driver, predicate: impl Fn(&Driver) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        driver.tick();
        if predicate(driver) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "popup did not reach the expected state"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn popup_geometry_clamps_cells_percent_defaults_and_too_small_without_zero_resize() {
    let area = Rect::new(10, 4, 101, 41);
    assert_eq!(
        resolve_geometry(None, None, area),
        Some((Rect::new(35, 14, 50, 20), Rect::new(36, 15, 48, 18)))
    );
    assert_eq!(
        resolve_geometry(
            Some(PluginPopupSize::Percent(100)),
            Some(PluginPopupSize::Cells(65535)),
            area
        ),
        Some((area, Rect::new(11, 5, 99, 39)))
    );
    assert_eq!(
        resolve_geometry(
            Some(PluginPopupSize::Cells(0)),
            Some(PluginPopupSize::Percent(1)),
            area
        ),
        Some((Rect::new(57, 22, 6, 4), Rect::new(58, 23, 4, 2)))
    );
    assert_eq!(resolve_geometry(None, None, Rect::new(0, 0, 5, 4)), None);
    assert_eq!(resolve_geometry(None, None, Rect::new(0, 0, 6, 3)), None);
}

#[test]
fn popup_lane_projects_inputs_resizes_and_waits_for_known_disposal() -> TestResult {
    popup_lane_trace(false)
}
#[test]
fn editor_lane_captures_read_only_source_then_projects_inputs_resizes_and_disposes() -> TestResult {
    popup_lane_trace(true)
}
fn popup_lane_trace(editor: bool) -> TestResult {
    let frame = orchbee_terminal_protocol::decode_terminal_text_frame(FRAME)?;
    let route = orchbee_terminal_protocol::inspect_terminal_frame_route(FRAME)?;
    let geometry = Geometry::new(
        frame.geometry.columns,
        frame.geometry.rows,
        frame.geometry.cell_pixel_width,
        frame.geometry.cell_pixel_height,
    )?;
    let (sent, received) = mpsc::channel();
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        admit(&mut stream)?;
        if index == 0 {
            return Ok(());
        }
        if editor {
            editor::open_source(&mut stream, geometry, editor::OpenMode::Accept, None)?;
        } else {
            let WorkbenchRequest::Plugin(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("popup open")
            };
            assert!(
                matches!(&request.action, PluginAction::PopupOpen { popup_id, geometry: g, .. }
            if popup_id.as_str() == id(10) && g.columns() == geometry.columns() && g.rows() == geometry.rows())
            );
            plugin_reply(
                &mut stream,
                request,
                PluginOutcome::PopupOpened {
                    popup_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
                    title: "Trusted dashboard".into(),
                },
            )?;
        }
        let mut current = geometry;
        let mut first = true;
        let mut sequence = 1u64;
        let mut inputs = Vec::new();
        loop {
            let message = read_client_message(&mut stream).map_err(io::Error::other)?;
            let response = match message {
                ClientMessage::TerminalViewer(request) => {
                    let outcome = match request.action() {
                        TerminalViewerAction::Attach {
                            terminal_create_request_id,
                        } => {
                            assert_eq!(terminal_create_request_id.as_str(), id(10));
                            TerminalViewerOutcome::Attached {
                                attachment_id: ViewerId::parse(&route.attachment_id.to_text())
                                    .unwrap(),
                                host_epoch: orchbee_client_server_protocol::HostEpoch::parse(
                                    &route.host_epoch.to_text(),
                                )
                                .unwrap(),
                                host_terminal_id: HostTerminalId::parse(&id(30)).unwrap(),
                            }
                        }
                        TerminalViewerAction::Poll { .. } if first => {
                            first = false;
                            let mut frame = FRAME.to_vec();
                            frame[48..56].copy_from_slice(&sequence.to_le_bytes());
                            frame[56..64].copy_from_slice(&u64::MAX.to_le_bytes());
                            frame[64..72].copy_from_slice(&sequence.to_le_bytes());
                            frame[136..138]
                                .copy_from_slice(&current.cell_pixel_width().to_le_bytes());
                            frame[138..140]
                                .copy_from_slice(&current.cell_pixel_height().to_le_bytes());
                            let bytes = orchbee_terminal_protocol::encode_transport_envelope(
                                &orchbee_terminal_protocol::Envelope {
                                    protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
                                    role: orchbee_terminal_protocol::MessageRole::Event,
                                    kind: 41,
                                    payload_encoding:
                                        orchbee_terminal_protocol::PayloadEncoding::TerminalFrame,
                                    host_epoch: Some(route.host_epoch),
                                    correlation_id: None,
                                    optional_message: false,
                                    payload: &frame,
                                },
                            )
                            .map_err(io::Error::other)?;
                            TerminalViewerOutcome::Record(bytes)
                        }
                        TerminalViewerAction::Poll { .. } => TerminalViewerOutcome::Empty {},
                        TerminalViewerAction::FrameApplied { .. } => {
                            TerminalViewerOutcome::FeedbackAccepted {}
                        }
                        TerminalViewerAction::Detach { .. } => {
                            assert_eq!(
                                inputs,
                                [
                                    b"\x1b".to_vec(),
                                    b"\x02q".to_vec(),
                                    b"hello\nworld".to_vec()
                                ]
                            );
                            let response =
                                ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                    request.request_id().clone(),
                                    TerminalViewerOutcome::Detached {},
                                ));
                            write_server_message(&mut stream, &response)
                                .map_err(io::Error::other)?;
                            break;
                        }
                        other => panic!("unexpected popup viewer: {other:?}"),
                    };
                    ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        outcome,
                    ))
                }
                ClientMessage::TerminalControl(request) => {
                    let outcome = match request.action() {
                        TerminalControlAction::Acquire { .. } => TerminalControlOutcome::Acquired {
                            controller_id: ControllerId::parse(&id(20)).unwrap(),
                        },
                        TerminalControlAction::Input { bytes, .. } => {
                            inputs.push(bytes.as_bytes().to_vec());
                            TerminalControlOutcome::InputAccepted {}
                        }
                        TerminalControlAction::Paste { content, .. } => {
                            inputs.push(content.as_bytes().to_vec());
                            sent.send(current).unwrap();
                            TerminalControlOutcome::Pasted {
                                accepted_bytes:
                                    orchbee_client_server_protocol::PasteAcceptedBytes::new(
                                        u32::try_from(content.as_bytes().len()).unwrap(),
                                    )
                                    .unwrap(),
                            }
                        }
                        TerminalControlAction::Resize { geometry, .. } => {
                            current = *geometry;
                            first = true;
                            sequence += 1;
                            sent.send(current).unwrap();
                            TerminalControlOutcome::Resized { geometry: current }
                        }
                        other => panic!("unexpected popup control: {other:?}"),
                    };
                    ServerMessage::TerminalControlResult(TerminalControlResult::new(
                        request.request_id().clone(),
                        outcome,
                    ))
                }
                other => panic!("canonical creation is forbidden in popup lane: {other:?}"),
            };
            write_server_message(&mut stream, &response).map_err(io::Error::other)?;
        }
        if editor {
            editor::lifecycle_reply(&mut stream, true, false)?;
            for done in [false, true] {
                editor::lifecycle_reply(&mut stream, false, done)?;
            }
        } else {
            let WorkbenchRequest::Plugin(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("popup close")
            };
            assert!(matches!(request.action, PluginAction::PopupClose { .. }));
            plugin_reply(
                &mut stream,
                request,
                PluginOutcome::PopupClosing {
                    popup_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
                },
            )?;
            for done in [false, true] {
                let WorkbenchRequest::Plugin(request) =
                    read_workbench_request(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("popup close observation")
                };
                assert!(matches!(request.action, PluginAction::PopupRead { .. }));
                plugin_reply(
                    &mut stream,
                    request,
                    if done {
                        PluginOutcome::PopupClosed {
                            popup_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
                        }
                    } else {
                        PluginOutcome::PopupState {
                            popup_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
                            settled: true,
                        }
                    },
                )?;
            }
        }
        assert_eq!(stream.read(&mut [0; 1])?, 0);
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "popup-test")?;
    let origin = client.origin();
    drop(client);
    let mut driver = Driver::new(clock());
    driver.set_origin(origin);
    let area = Rect::new(
        0,
        0,
        geometry.columns() + if editor { 2 } else { 20 },
        geometry.rows() + if editor { 2 } else { 10 },
    );
    driver.observe_surface(
        area,
        (geometry.cell_pixel_width(), geometry.cell_pixel_height()),
    );
    let mut spec = spec();
    spec.width = Some(PluginPopupSize::Cells(geometry.columns() + 2));
    spec.height = Some(PluginPopupSize::Cells(geometry.rows() + 2));
    let operation = WorkbenchRequestId::parse(&id(10))?;
    let opened = if editor {
        driver.begin_editor(SessionId::parse(&id(2))?, operation.clone(), editor::spec())
    } else {
        driver.begin_open(SessionId::parse(&id(2))?, operation.clone(), spec.clone())
    };
    opened.map_err(|r| io::Error::other(format!("{r:?}")))?;
    assert_eq!(
        driver.begin_open(
            SessionId::parse(&id(2))?,
            WorkbenchRequestId::parse(&id(11))?,
            spec
        ),
        Err(Reason::Busy)
    );
    wait_until(&mut driver, Driver::input_ready);
    assert_eq!(driver.view().unwrap().title, "Trusted dashboard");
    assert!(matches!(
        driver.completion(&operation, true),
        Some(ClientViewCompletionOutcome::PopupOpened { .. })
    ));
    let escape = DecodedItem {
        kind: DecodedKind::Key(crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Esc,
            crate::client_view::Modifiers::NONE,
        )),
        raw: b"\x1b".to_vec().into_boxed_slice(),
        owned: false,
    };
    let admitted = driver.input_context().expect("active popup context");
    assert_eq!(admitted.id(), &operation);
    assert!(admitted.interactive());
    let close = driver.view().unwrap().close_target();
    let stale_close = DrawnPopupClose(WorkbenchRequestId::parse(&id(11))?);

    // The shell supplies an empty surface while its viewport is TooSmall.
    // That revokes program input without changing the popup incarnation or
    // replacing the lane's last nonzero desired geometry.
    driver.observe_surface(
        Rect::default(),
        (geometry.cell_pixel_width(), geometry.cell_pixel_height()),
    );
    let suspended = driver.input_context().expect("retained popup context");
    assert_eq!(suspended.id(), &operation);
    assert!(!suspended.interactive());
    assert_eq!(driver.identity(), Some(&operation));
    assert_eq!(
        *driver
            .lane
            .as_ref()
            .expect("retained popup lane")
            .resize
            .lock()
            .unwrap(),
        geometry,
        "an unpaintable surface must not request a zero-sized resize"
    );
    driver.input(&operation, &escape);

    driver.observe_surface(
        area,
        (geometry.cell_pixel_width(), geometry.cell_pixel_height()),
    );
    let resumed = driver.input_context().expect("resumed popup context");
    assert_eq!(resumed.id(), &operation);
    assert!(resumed.interactive());
    driver.input(&operation, &escape);
    driver.enqueue(Input::Bytes(ControlInput::new(b"\x02q".to_vec())?));
    driver.enqueue(Input::Paste(PasteContent::new(b"hello\nworld".to_vec())?));
    driver.observe_surface(area, (2, 3));
    assert!(!driver.input_ready());
    assert_eq!(
        received.recv_timeout(Duration::from_secs(3))?,
        Geometry::new(geometry.columns(), geometry.rows(), 2, 3)?
    );
    wait_until(&mut driver, Driver::input_ready);
    received.recv_timeout(Duration::from_secs(3))?;
    assert!(!driver.activate_drawn_close(&stale_close));
    assert!(driver.input_ready());
    assert!(driver.activate_drawn_close(&close));
    assert!(!driver.activate_drawn_close(&close));
    assert!(driver.completion(&operation, false).is_none());
    assert!(!driver.input_ready());
    wait_until(&mut driver, |driver| {
        matches!(driver.latest.as_ref().map(|l| l.phase), Some(Phase::Closed))
    });
    assert_eq!(
        driver.completion(&operation, false),
        Some(ClientViewCompletionOutcome::PopupClosed { had_popup: true })
    );
    assert!(!driver.captured());
    driver.shutdown();
    server.finish()?;
    Ok(())
}

#[test]
fn popup_shutdown_interrupts_open_without_replay_and_known_refusal_allows_retry() -> TestResult {
    for refuse in [false, true] {
        let (sent, received) = mpsc::channel();
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            admit(&mut stream)?;
            if index == 0 {
                return Ok(());
            }
            let WorkbenchRequest::Plugin(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("popup open")
            };
            assert!(matches!(request.action, PluginAction::PopupOpen { .. }));
            sent.send(()).unwrap();
            if refuse {
                plugin_reply(
                    &mut stream,
                    request,
                    PluginOutcome::Failed {
                        reason: orchbee_workbench_protocol::PluginFailure::Disabled,
                    },
                )?;
            }
            assert_eq!(stream.read(&mut [0; 1])?, 0);
            Ok(())
        })?;
        let client = LocalClient::connect(server.endpoint(), "popup-cancel-test")?;
        let origin = client.origin();
        drop(client);
        let mut driver = Driver::new(clock());
        driver.set_origin(origin);
        driver.observe_surface(Rect::new(0, 0, 80, 24), (0, 0));
        driver
            .begin_open(
                SessionId::parse(&id(2))?,
                WorkbenchRequestId::parse(&id(10))?,
                spec(),
            )
            .map_err(|r| io::Error::other(format!("{r:?}")))?;
        received.recv_timeout(Duration::from_secs(3))?;
        if refuse {
            wait_until(&mut driver, |driver| {
                matches!(
                    driver.latest.as_ref().map(|l| l.phase),
                    Some(Phase::Refused(_))
                )
            });
            assert!(!driver.busy());
            assert!(!driver.failed());
        }
        let before = Instant::now();
        driver.shutdown();
        assert!(before.elapsed() < Duration::from_secs(2));
        server.finish()?;
    }
    Ok(())
}

mod editor;
