use super::state::CarrierFailure;
mod close_interaction;
#[cfg(any(target_os = "linux", windows))]
mod close_recovery;
mod close_retirement;
mod move_refinements;
mod rename_interaction;
#[cfg(any(target_os = "linux", windows))]
mod retired_membership;
mod retirement_redraw;
use super::*;
use crate::{
    client_view::{ClientView, SessionActionDisposition},
    local_client::test_server::FakeServer,
    localization::Locale,
    terminal_input::{DispatchOutcome, InputDispatcher, dispatch_input},
    tui_render::render_terminal_shell,
};
use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, ServerMessage, TerminalControlAction, TerminalControlOutcome,
    TerminalControlResult, TerminalViewerOutcome, TerminalViewerResult, ViewerId,
    read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding, decode_terminal_text_frame,
    encode_transport_envelope, inspect_terminal_frame_route,
};
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, ProtocolVersion, SiteId, Snapshot,
    Tab, TerminalContext, TerminalId, TerminalPresentationId, WorkbenchAction,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchPanelId,
    WorkbenchPublicationDocument, WorkbenchPublicationOutcome, WorkbenchPublicationResult,
    WorkbenchRequest, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult, WorkbenchTabId, WorkbenchTerminalContextId, WorkerId,
    read_workbench_request, write_workbench_response,
};
use ratatui::{Terminal, backend::TestBackend};
use std::io::Write;
use std::{
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
const ID: &str = "11111111111111111111111111111111";
const SITE_ID: &str = "22222222222222222222222222222222";
const WORKER_ID: &str = "33333333333333333333333333333333";
const TERMINAL_ID: &str = "44444444444444444444444444444444";
const CREATED_TAB_ID: &str = "55555555555555555555555555555555";
const CREATED_PANEL_ID: &str = "66666666666666666666666666666666";
const CREATED_PRESENTATION_ID: &str = "77777777777777777777777777777777";
const NATIVE_TEST_IDENTITY_SEED: u128 = 0x100;
const FRAME: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin"
));
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn geometry() -> Result<Geometry, Box<dyn std::error::Error>> {
    Ok(Geometry::new(80, 24, 8, 16)?)
}

fn fixture_geometry() -> Result<Geometry, Box<dyn std::error::Error>> {
    let frame = decode_terminal_text_frame(FRAME)?;
    Ok(Geometry::new(
        frame.geometry.columns,
        frame.geometry.rows,
        frame.geometry.cell_pixel_width,
        frame.geometry.cell_pixel_height,
    )?)
}

fn full_frame_record(
    sequence: u64,
    revision: u64,
    geometry: Geometry,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut frame = FRAME.to_vec();
    frame[48..56].copy_from_slice(&sequence.to_le_bytes());
    frame[56..64].copy_from_slice(&u64::MAX.to_le_bytes());
    frame[64..72].copy_from_slice(&revision.to_le_bytes());
    frame[136..138].copy_from_slice(&geometry.cell_pixel_width().to_le_bytes());
    frame[138..140].copy_from_slice(&geometry.cell_pixel_height().to_le_bytes());
    let route = inspect_terminal_frame_route(&frame)?;
    Ok(encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: 41,
        payload_encoding: PayloadEncoding::TerminalFrame,
        host_epoch: Some(route.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: &frame,
    })?)
}
fn admit(stream: &mut UnixStream) -> io::Result<()> {
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

fn attach_outcome() -> io::Result<TerminalViewerOutcome> {
    Ok(TerminalViewerOutcome::Attached {
        attachment_id: ViewerId::parse(ID).map_err(io::Error::other)?,
        host_epoch: HostEpoch::parse(ID).map_err(io::Error::other)?,
        host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(ID)
            .map_err(io::Error::other)?,
    })
}

fn attached_viewer_id() -> io::Result<ViewerId> {
    let TerminalViewerOutcome::Attached { attachment_id, .. } = attach_outcome()? else {
        return Err(io::Error::other(
            "attach fixture did not contain a ViewerId",
        ));
    };
    Ok(attachment_id)
}

fn attach(stream: &mut UnixStream) -> io::Result<()> {
    let ClientMessage::TerminalViewer(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected viewer"));
    };
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            attach_outcome()?,
        )),
    )
    .map_err(io::Error::other)
}
fn respond_with_snapshot(
    stream: &mut UnixStream,
    outcome: WorkbenchSnapshotOutcome,
) -> io::Result<String> {
    let WorkbenchRequest::SnapshotRead(request) =
        read_workbench_request(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected SnapshotRead"));
    };
    let request_id = request.request_id.as_str().to_owned();
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome,
        }),
    )
    .map_err(io::Error::other)?;
    Ok(request_id)
}

/// Reads the next OBWP request, transparently answering `publication.read` probes with `current`
/// so scripted expectations only observe logical requests.
fn read_workbench_request_absorbing_probes(
    stream: &mut UnixStream,
) -> io::Result<WorkbenchRequest> {
    loop {
        let request = read_workbench_request(&mut *stream).map_err(io::Error::other)?;
        if let WorkbenchRequest::PublicationRead(intent) = request {
            write_workbench_response(
                stream,
                &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                    protocol_version: ProtocolVersion,
                    request_id: intent.request_id,
                    outcome: WorkbenchPublicationOutcome::Current {
                        revision: intent.known_revision,
                    },
                }),
            )
            .map_err(io::Error::other)?;
            continue;
        }
        return Ok(request);
    }
}

/// Maps the latest committed snapshot outcome onto a `publication.read` reply the way a real
/// journal would: `current` when the reader already holds the offered revision, otherwise
/// `published` carrying the full snapshot as the boundary publication.
fn publication_outcome_for(
    outcome: WorkbenchSnapshotOutcome,
    known_revision: WorkbenchRevision,
) -> WorkbenchPublicationOutcome {
    match outcome {
        WorkbenchSnapshotOutcome::Available { snapshot } => {
            if snapshot.revision > known_revision {
                WorkbenchPublicationOutcome::Published {
                    publications: vec![WorkbenchPublicationDocument::Snapshot(snapshot)],
                }
            } else {
                WorkbenchPublicationOutcome::Current {
                    revision: snapshot.revision,
                }
            }
        }
        WorkbenchSnapshotOutcome::Denied {} => WorkbenchPublicationOutcome::Denied {},
        WorkbenchSnapshotOutcome::Unavailable { reason } => {
            WorkbenchPublicationOutcome::Unavailable { reason }
        }
    }
}

/// Answers whichever Workbench read the Client issued: a `snapshot.read` gets the outcome
/// verbatim, while a `publication.read` is served through `publication_outcome_for`.
fn respond_with_read(
    stream: &mut UnixStream,
    outcome: WorkbenchSnapshotOutcome,
) -> io::Result<String> {
    match read_workbench_request(stream).map_err(io::Error::other)? {
        WorkbenchRequest::SnapshotRead(request) => {
            let request_id = request.request_id.as_str().to_owned();
            write_workbench_response(
                stream,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome,
                }),
            )
            .map_err(io::Error::other)?;
            Ok(request_id)
        }
        WorkbenchRequest::PublicationRead(request) => {
            let request_id = request.request_id.as_str().to_owned();
            write_workbench_response(
                stream,
                &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: publication_outcome_for(outcome, request.known_revision),
                }),
            )
            .map_err(io::Error::other)?;
            Ok(request_id)
        }
        _ => Err(io::Error::other("expected a Workbench read request")),
    }
}
fn available_snapshot(binding: Option<PresentationBinding>) -> WorkbenchSnapshotOutcome {
    WorkbenchSnapshotOutcome::Available {
        snapshot: Snapshot {
            navigation: Navigation {
                workspace_order: Vec::new(),
                projects: Vec::new(),
                standalone_terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
            },
            protocol_version: ProtocolVersion,
            revision: WorkbenchRevision::new(1),
            site_id: SiteId::parse(SITE_ID).unwrap(),
            terminal_contexts: vec![TerminalContext {
                groups: Vec::new(),
                tabs: vec![Tab {
                    automatic_label: false,
                    group_id: None,
                    label: "standalone".into(),
                    pinned: false,
                    root: PanelNode::Panel {
                        right_click: Default::default(),
                        child_presentations: Vec::new(),
                        panel_id: WorkbenchPanelId::parse(ID).unwrap(),
                        primary_presentation: Presentation {
                            custom_label: None,
                            binding,
                            label: "terminal".into(),
                            presentation_id: TerminalPresentationId::parse(ID).unwrap(),
                        },
                    },
                    tab_id: WorkbenchTabId::parse(ID).unwrap(),
                }],
                terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
            }],
        },
    }
}
fn binding() -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(TERMINAL_ID).unwrap(),
        worker_id: WorkerId::parse(WORKER_ID).unwrap(),
    }
}

fn binding_attach_outcome() -> io::Result<TerminalViewerOutcome> {
    Ok(TerminalViewerOutcome::Attached {
        attachment_id: ViewerId::parse(SITE_ID).map_err(io::Error::other)?,
        host_epoch: HostEpoch::parse(ID).map_err(io::Error::other)?,
        host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(ID)
            .map_err(io::Error::other)?,
    })
}

fn binding_viewer_id() -> io::Result<ViewerId> {
    let TerminalViewerOutcome::Attached { attachment_id, .. } = binding_attach_outcome()? else {
        return Err(io::Error::other(
            "binding attach fixture did not contain a ViewerId",
        ));
    };
    Ok(attachment_id)
}

fn attach_binding(stream: &mut UnixStream) -> io::Result<()> {
    let ClientMessage::TerminalViewer(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected viewer"));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::AttachBinding {
            site_id: orchbee_client_server_protocol::SiteId::parse(SITE_ID)
                .map_err(io::Error::other)?,
            worker_id: orchbee_client_server_protocol::WorkerId::parse(WORKER_ID)
                .map_err(io::Error::other)?,
            terminal_id: orchbee_client_server_protocol::TerminalId::parse(TERMINAL_ID)
                .map_err(io::Error::other)?,
        }
    );
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            binding_attach_outcome()?,
        )),
    )
    .map_err(io::Error::other)
}
fn acquire(stream: &mut UnixStream, attachment_id: &ViewerId) -> io::Result<()> {
    let ClientMessage::TerminalControl(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected acquire"));
    };
    assert_eq!(
        request.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attachment_id.clone(),
        }
    );
    write_server_message(
        stream,
        &ServerMessage::TerminalControlResult(TerminalControlResult::new(
            request.request_id().clone(),
            TerminalControlOutcome::Acquired {
                controller_id: orchbee_client_server_protocol::ControllerId::parse(ID)
                    .map_err(io::Error::other)?,
            },
        )),
    )
    .map_err(io::Error::other)
}

fn serve_native_controlling_frame(stream: &mut UnixStream, geometry: Geometry) -> io::Result<()> {
    serve_native_controlling_frame_with_snapshot(
        stream,
        geometry,
        available_snapshot(Some(binding())),
    )
}

fn serve_native_controlling_frame_with_snapshot(
    stream: &mut UnixStream,
    geometry: Geometry,
    initial: WorkbenchSnapshotOutcome,
) -> io::Result<()> {
    admit(stream)?;
    let ClientMessage::TerminalOpen(open) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected Open"));
    };
    assert_eq!(open.request_id().as_str(), native_identity(0x101));
    assert_eq!(open.geometry(), geometry);
    write_server_message(
        stream,
        &ServerMessage::TerminalOpenResult(
            orchbee_client_server_protocol::TerminalOpenResult::new(
                open.request_id().clone(),
                orchbee_client_server_protocol::OpenOutcome::Opened {
                    terminal_create_request_id: RequestId::parse(ID).map_err(io::Error::other)?,
                },
            ),
        ),
    )
    .map_err(io::Error::other)?;

    assert_eq!(
        respond_with_snapshot(stream, initial)?,
        native_identity(0x102)
    );
    let ClientMessage::TerminalViewer(attach) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected AttachBinding"));
    };
    assert_eq!(attach.request_id().as_str(), native_identity(0x103));
    assert_eq!(
        attach.action(),
        &TerminalViewerAction::AttachBinding {
            site_id: orchbee_client_server_protocol::SiteId::parse(SITE_ID)
                .map_err(io::Error::other)?,
            worker_id: orchbee_client_server_protocol::WorkerId::parse(WORKER_ID)
                .map_err(io::Error::other)?,
            terminal_id: orchbee_client_server_protocol::TerminalId::parse(TERMINAL_ID)
                .map_err(io::Error::other)?,
        }
    );
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            attach.request_id().clone(),
            binding_attach_outcome()?,
        )),
    )
    .map_err(io::Error::other)?;

    let ClientMessage::TerminalControl(acquire) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected Acquire"));
    };
    assert_eq!(acquire.request_id().as_str(), native_identity(0x104));
    assert_eq!(
        acquire.action(),
        &TerminalControlAction::Acquire {
            attachment_id: binding_viewer_id()?,
        }
    );
    write_server_message(
        stream,
        &ServerMessage::TerminalControlResult(TerminalControlResult::new(
            acquire.request_id().clone(),
            TerminalControlOutcome::Acquired {
                controller_id: orchbee_client_server_protocol::ControllerId::parse(ID)
                    .map_err(io::Error::other)?,
            },
        )),
    )
    .map_err(io::Error::other)?;

    let ClientMessage::TerminalControl(resize) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected setup Resize"));
    };
    assert_eq!(resize.request_id().as_str(), native_identity(0x105));
    assert!(matches!(
        resize.action(),
        TerminalControlAction::Resize {
            attachment_id,
            sequence,
            geometry: requested,
            ..
        } if attachment_id == &binding_viewer_id()?
            && sequence.get() == 1
            && *requested == geometry
    ));
    write_server_message(
        stream,
        &ServerMessage::TerminalControlResult(TerminalControlResult::new(
            resize.request_id().clone(),
            TerminalControlOutcome::Resized { geometry },
        )),
    )
    .map_err(io::Error::other)?;

    let ClientMessage::TerminalViewer(poll) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected setup Poll"));
    };
    assert_eq!(poll.request_id().as_str(), native_identity(0x106));
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: binding_viewer_id()?,
        }
    );
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            poll.request_id().clone(),
            TerminalViewerOutcome::Record(
                full_frame_record(1, 1, geometry)
                    .map_err(|error| io::Error::other(error.to_string()))?,
            ),
        )),
    )
    .map_err(io::Error::other)
}

fn accept_native_frame_feedback(stream: &mut UnixStream) -> io::Result<()> {
    let ClientMessage::TerminalViewer(feedback) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected FrameApplied"));
    };
    assert_eq!(feedback.request_id().as_str(), native_identity(0x107));
    assert!(matches!(
        feedback.action(),
        TerminalViewerAction::FrameApplied { .. }
    ));
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            feedback.request_id().clone(),
            TerminalViewerOutcome::FeedbackAccepted {},
        )),
    )
    .map_err(io::Error::other)
}
fn spin(session: &mut TerminalSession, predicate: impl Fn(&TerminalSession) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !predicate(session) {
        assert!(Instant::now() < deadline, "session did not settle");
        session.tick();
        thread::sleep(Duration::from_millis(1));
    }
}

fn spin_with_draw(
    session: &mut TerminalSession,
    predicate: impl Fn(&TerminalSession) -> bool,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !predicate(session) {
        if Instant::now() >= deadline {
            return false;
        }
        session.tick();
        session.after_draw();
        thread::sleep(Duration::from_millis(1));
    }
    true
}

fn legacy_status(session: &TerminalSession) -> Option<TerminalStatus> {
    match &session.backend {
        SessionBackend::Legacy(legacy) => Some(legacy.status),
        SessionBackend::Native(_) => None,
    }
}

fn native_client(server: &FakeServer) -> Result<LocalClient, Box<dyn std::error::Error>> {
    let mut client = LocalClient::connect(server.endpoint(), "test-cohort")?;
    client.set_submission_deadline(Some(Instant::now() + Duration::from_secs(5)));
    Ok(client)
}

fn native_session_with_identity(
    client: LocalClient,
    geometry: Geometry,
) -> Result<(TerminalSession, Arc<mailbox::CommandMailbox>), Box<dyn std::error::Error>> {
    let shutdown = client.shutdown_handle()?;
    #[cfg(any(target_os = "linux", windows))]
    let origin = client.origin();
    let clock = client.operation_clock();
    let mailbox = Arc::new(mailbox::CommandMailbox::new(
        NATIVE_TEST_IDENTITY_SEED,
        geometry,
    ));
    let worker_mailbox = Arc::clone(&mailbox);
    let task = thread::Builder::new()
        .name("orchbee-native-ui-test-session".into())
        .spawn(move || connection::run_native(client, geometry, worker_mailbox, None))?;
    Ok((
        TerminalSession {
            #[cfg(windows)]
            position_writer: None,
            #[cfg(windows)]
            position_startup: None,
            #[cfg(windows)]
            configuration: None,
            #[cfg(windows)]
            notifications: None,
            #[cfg(windows)]
            window_title: None,
            #[cfg(windows)]
            client_control: None,
            backend: SessionBackend::Native(Box::new(ui::NativeUi::new(
                Arc::clone(&mailbox),
                clock,
            ))),
            task: Some(SessionTask::Native(task)),
            shutdown,
            retirement_redraw_pending: false,
            #[cfg(any(target_os = "linux", windows))]
            origin: Some(origin),
            #[cfg(any(target_os = "linux", windows))]
            recovery_shutdown: None,
        },
        mailbox,
    ))
}

fn native_identity(value: u128) -> String {
    format!("{value:032x}")
}

fn render_native_session(
    session: &TerminalSession,
    view: &ClientView,
) -> Result<String, Box<dyn std::error::Error>> {
    let width = 120;
    let height = 32;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| {
        render_terminal_shell(
            frame,
            view,
            Locale::EnUs,
            Some(session.view()),
            &crate::tui_theme::Theme::terminal_native(),
            &mut crate::tui_pointer::HitMapBuilder::new(),
        );
    })?;
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    Ok(text)
}

fn open_navigator(view: &mut ClientView) {
    use crate::client_view::{KeyCode, KeyEvent, Modifiers, PrefixInput};
    let key = |code, mods| PrefixInput::Key(KeyEvent::press(code, mods));
    let prefix = view.decide(key(KeyCode::Char('b'), Modifiers::CTRL));
    view.commit(prefix, SessionActionDisposition::KeepNavigator);
    let open = view.decide(key(KeyCode::Char('w'), Modifiers::NONE));
    view.commit(open, SessionActionDisposition::KeepNavigator);
}

#[test]
fn native_create_action_renders_before_feedback_and_captures_wire_intent() -> TestResult {
    let geometry = fixture_geometry()?;
    let (ordinary_poll_sent, ordinary_poll_seen) = mpsc::sync_channel(0);
    let (release_ordinary_poll, ordinary_poll_released) = mpsc::sync_channel(0);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;

        let ClientMessage::TerminalViewer(poll) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected ordinary Poll"));
        };
        assert_eq!(poll.request_id().as_str(), native_identity(0x108));
        assert_eq!(
            poll.action(),
            &TerminalViewerAction::Poll {
                attachment_id: binding_viewer_id()?,
            }
        );
        ordinary_poll_sent.send(()).map_err(io::Error::other)?;
        ordinary_poll_released.recv().map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            )),
        )
        .map_err(io::Error::other)?;

        let WorkbenchRequest::Command(command) =
            read_workbench_request_absorbing_probes(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected tab.create"));
        };
        assert_eq!(command.protocol_version, ProtocolVersion);
        assert_eq!(command.request_id.as_str(), native_identity(0x109));
        assert_eq!(command.site_id.as_str(), SITE_ID);
        assert_eq!(command.expected_revision, WorkbenchRevision::new(1));
        let WorkbenchAction::TabCreate {
            terminal_context_id,
            label,
            geometry: requested,
            launch: None,
            ..
        } = &command.action
        else {
            return Err(io::Error::other("expected tab.create action"));
        };
        assert_eq!(terminal_context_id.as_str(), ID);
        assert_eq!(label.as_str(), "2");
        assert_eq!(requested.columns(), geometry.columns());
        assert_eq!(requested.rows(), geometry.rows());
        assert_eq!(requested.cell_pixel_width(), geometry.cell_pixel_width());
        assert_eq!(requested.cell_pixel_height(), geometry.cell_pixel_height());
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id,
                outcome: WorkbenchCommandOutcome::Denied {},
            }),
        )
        .map_err(io::Error::other)?;

        loop {
            match read_client_message(&mut stream) {
                Ok(ClientMessage::TerminalViewer(request))
                    if matches!(
                        request.action(),
                        TerminalViewerAction::Poll { attachment_id }
                            if attachment_id == &binding_viewer_id()?
                    ) =>
                {
                    write_server_message(
                        &mut stream,
                        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                            request.request_id().clone(),
                            TerminalViewerOutcome::Empty {},
                        )),
                    )
                    .map_err(io::Error::other)?;
                }
                Err(_) => return Ok(()),
                Ok(_) => return Err(io::Error::other("unexpected post-Create request")),
            }
        }
    })?;
    let mut client = LocalClient::connect(server.endpoint(), "test-cohort")?;
    client.set_submission_deadline(Some(Instant::now() + Duration::from_secs(5)));
    let (mut session, _mailbox) = native_session_with_identity(client, geometry)?;

    let deadline = Instant::now() + Duration::from_secs(2);
    while !session.startup_draw_ready() {
        assert!(Instant::now() < deadline, "startup frame was not staged");
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(session.status(), TerminalStatus::Connecting);
    let mut client_view = ClientView::new(120);
    let before_feedback = render_native_session(&session, &client_view)?;
    assert!(before_feedback.contains("[*1 standalone]"));
    assert!(before_feedback.contains("Tab 1 · Connecting"));
    assert!(session.startup_draw_ready());
    session.after_draw();
    assert!(spin_with_draw(&mut session, |session| {
        session.status() == TerminalStatus::Controlling
    }));
    ordinary_poll_seen
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("ordinary Poll was not observed: {error}"))?;
    let ready = render_native_session(&session, &client_view)?;
    assert!(ready.contains("Tab 1 · Ready · controlling"));

    open_navigator(&mut client_view);
    let mut input_dispatcher = InputDispatcher::default();
    let outcome = dispatch_input(
        &mut input_dispatcher,
        &mut client_view,
        b"\x02c",
        &mut session,
        Instant::now(),
        |_view, _session| Ok::<(), std::convert::Infallible>(()),
    )?;
    assert_eq!(outcome, DispatchOutcome::Continue);
    assert_eq!(client_view.overlay(), None);
    release_ordinary_poll.send(())?;

    assert!(spin_with_draw(&mut session, |session| {
        matches!(
            session.view(),
            TerminalSessionView::Native(view)
                if view.notice() == Some(NativeSessionNotice::CreateNotCreated)
        )
    }));
    let after_precommit = render_native_session(&session, &client_view)?;
    assert!(after_precommit.contains("Tab not created · previous Tab unchanged"));
    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn native_move_key_settles_canonical_order_or_explicit_recovery_with_the_same_input_viewer()
-> TestResult {
    for unavailable in [false, true] {
        let geometry = fixture_geometry()?;
        let teardown = Arc::new(AtomicBool::new(false));
        let server_teardown = Arc::clone(&teardown);
        let (poll_sent, poll_seen) = mpsc::sync_channel(0);
        let (release_poll, poll_released) = mpsc::sync_channel(0);
        let (input_sent, input_seen) = mpsc::sync_channel(1);
        let (resize_sent, resize_seen) = mpsc::sync_channel(1);
        let server = FakeServer::start(move |mut stream| {
            let WorkbenchSnapshotOutcome::Available { mut snapshot } =
                available_snapshot(Some(binding()))
            else {
                unreachable!()
            };
            let mut second = snapshot.terminal_contexts[0].tabs[0].clone();
            second.tab_id = WorkbenchTabId::parse(CREATED_TAB_ID).map_err(io::Error::other)?;
            second.label = "second".into();
            let PanelNode::Panel {
                panel_id,
                primary_presentation,
                ..
            } = &mut second.root
            else {
                unreachable!()
            };
            *panel_id = WorkbenchPanelId::parse(CREATED_PANEL_ID).map_err(io::Error::other)?;
            primary_presentation.presentation_id =
                TerminalPresentationId::parse(CREATED_PRESENTATION_ID).map_err(io::Error::other)?;
            primary_presentation.binding.as_mut().unwrap().terminal_id =
                TerminalId::parse(CREATED_TAB_ID).map_err(io::Error::other)?;
            snapshot.terminal_contexts[0].tabs.push(second);
            serve_native_controlling_frame_with_snapshot(
                &mut stream,
                geometry,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                },
            )?;
            accept_native_frame_feedback(&mut stream)?;
            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected ordinary Poll"));
            };
            assert_eq!(
                poll.action(),
                &TerminalViewerAction::Poll {
                    attachment_id: binding_viewer_id()?,
                }
            );
            poll_sent.send(()).map_err(io::Error::other)?;
            poll_released.recv().map_err(io::Error::other)?;
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    poll.request_id().clone(),
                    TerminalViewerOutcome::Empty {},
                )),
            )
            .map_err(io::Error::other)?;
            let WorkbenchRequest::Command(command) =
                read_workbench_request_absorbing_probes(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected Move"));
            };
            assert_eq!(command.expected_revision, WorkbenchRevision::new(1));
            assert!(
                matches!(&command.action, WorkbenchAction::TabMove { tab_id, direction: orchbee_workbench_protocol::WorkbenchTabMoveDirection::Last, .. } if tab_id.as_str() == ID)
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: command.request_id,
                    outcome: WorkbenchCommandOutcome::TabMoved {
                        revision: WorkbenchRevision::new(2),
                        terminal_context_id: WorkbenchTerminalContextId::parse(ID)
                            .map_err(io::Error::other)?,
                        tab_id: WorkbenchTabId::parse(ID).map_err(io::Error::other)?,
                    },
                }),
            )
            .map_err(io::Error::other)?;
            snapshot.revision = WorkbenchRevision::new(2);
            snapshot.terminal_contexts[0].tabs.swap(0, 1);
            respond_with_read(
                &mut stream,
                if unavailable {
                    WorkbenchSnapshotOutcome::Denied {}
                } else {
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: snapshot.clone(),
                    }
                },
            )?;
            loop {
                let mut magic = [0; 4];
                if stream.read_exact(&mut magic).is_err() {
                    return Ok(());
                }
                let mut request_reader = std::io::Cursor::new(magic).chain(&mut stream);
                if magic == *b"OBWP" {
                    // Reads and probes are both expected here: the logical recovery read and
                    // poll-carried publication probes are served from the same latest snapshot.
                    let (request_id, known) = match read_workbench_request(&mut request_reader)
                        .map_err(io::Error::other)?
                    {
                        WorkbenchRequest::SnapshotRead(request) => (request.request_id, None),
                        WorkbenchRequest::PublicationRead(request) => {
                            (request.request_id, Some(request.known_revision))
                        }
                        _ => {
                            return Err(io::Error::other(
                                "recovery must submit only Workbench reads",
                            ));
                        }
                    };
                    let outcome = WorkbenchSnapshotOutcome::Available {
                        snapshot: snapshot.clone(),
                    };
                    write_workbench_response(
                        &mut stream,
                        &match known {
                            Some(known) => {
                                WorkbenchResponse::Publication(WorkbenchPublicationResult {
                                    protocol_version: ProtocolVersion,
                                    request_id,
                                    outcome: publication_outcome_for(outcome, known),
                                })
                            }
                            None => WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                                protocol_version: ProtocolVersion,
                                request_id,
                                outcome,
                            }),
                        },
                    )
                    .map_err(io::Error::other)?;
                    continue;
                }
                assert_eq!(magic, *b"OBCS");
                match read_client_message(&mut request_reader) {
                    Ok(ClientMessage::TerminalViewer(request))
                        if matches!(
                            request.action(),
                            TerminalViewerAction::Poll { attachment_id }
                                if attachment_id == &binding_viewer_id()?
                        ) =>
                    {
                        let response = write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::Empty {},
                            )),
                        );
                        match response {
                            Ok(()) => {}
                            Err(orchbee_client_server_protocol::WriteMessageError::Io(error))
                                if server_teardown.load(Ordering::SeqCst)
                                    && matches!(
                                        error.kind(),
                                        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
                                    ) =>
                            {
                                return Ok(());
                            }
                            Err(error) => return Err(io::Error::other(error)),
                        }
                    }
                    Ok(ClientMessage::TerminalControl(request)) => match request.action() {
                        TerminalControlAction::Input {
                            attachment_id,
                            bytes,
                            ..
                        } if attachment_id == &binding_viewer_id()? => {
                            assert_eq!(bytes.as_bytes(), b"still attached");
                            write_server_message(
                                &mut stream,
                                &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                    request.request_id().clone(),
                                    TerminalControlOutcome::InputAccepted {},
                                )),
                            )
                            .map_err(io::Error::other)?;
                            input_sent.send(()).map_err(io::Error::other)?;
                        }
                        TerminalControlAction::Resize {
                            attachment_id,
                            geometry,
                            ..
                        } if attachment_id == &binding_viewer_id()? => {
                            write_server_message(
                                &mut stream,
                                &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                    request.request_id().clone(),
                                    TerminalControlOutcome::Resized {
                                        geometry: *geometry,
                                    },
                                )),
                            )
                            .map_err(io::Error::other)?;
                            resize_sent.send(()).map_err(io::Error::other)?;
                        }
                        _ => return Err(io::Error::other("unexpected Move attachment effect")),
                    },
                    Err(_) => return Ok(()),
                    Ok(_) => return Err(io::Error::other("Move must retain the existing viewer")),
                }
            }
        })?;
        let (mut session, _) = native_session_with_identity(native_client(&server)?, geometry)?;
        assert!(spin_with_draw(&mut session, |session| session.status()
            == TerminalStatus::Controlling));
        poll_seen.recv_timeout(Duration::from_secs(1))?;
        let mut view = ClientView::new(120);
        open_navigator(&mut view);
        let mut input_dispatcher = InputDispatcher::default();
        dispatch_input(
            &mut input_dispatcher,
            &mut view,
            b"\x02<",
            &mut session,
            Instant::now(),
            |_, _| Ok::<(), std::convert::Infallible>(()),
        )?;
        assert_eq!(view.overlay(), None);
        release_poll.send(())?;
        assert!(spin_with_draw(
            &mut session,
            |session| matches!(session.view(), TerminalSessionView::Native(native) if if unavailable { native.phase() == NativeDisplayPhase::MoveProjectionUnavailable } else { native.active_position() == Some(2) })
        ));
        assert_eq!(session.status(), TerminalStatus::Controlling);
        let rendered = render_native_session(&session, &view)?;
        assert!(rendered.contains(if unavailable {
            "Tab moved · Ctrl+B w"
        } else {
            "Tab 2 · Ready · controlling"
        }));
        assert!(TerminalInputPort::forward(&mut session, b"still attached").is_ok());
        assert!(spin_with_draw(&mut session, |_| input_seen
            .try_recv()
            .is_ok()));
        if unavailable {
            for action in [
                ClientAction::CreateTab,
                ClientAction::MoveNextTab,
                ClientAction::SelectNextTab,
            ] {
                assert_eq!(
                    TerminalInputPort::apply_client_action(&mut session, action),
                    Some(SessionActionDisposition::KeepNavigator)
                );
            }
            session.observe_geometry(Geometry::new(100, 24, 8, 16)?);
            assert!(spin_with_draw(&mut session, |_| resize_seen
                .try_recv()
                .is_ok()));
            let mut input_dispatcher = InputDispatcher::default();
            dispatch_input(
                &mut input_dispatcher,
                &mut view,
                b"\x02w",
                &mut session,
                Instant::now(),
                |_, _| Ok::<(), std::convert::Infallible>(()),
            )?;
            assert!(spin_with_draw(
                &mut session,
                |session| matches!(session.view(), TerminalSessionView::Native(native) if native.phase() == NativeDisplayPhase::Ready && native.active_position() == Some(2))
            ));
            assert_eq!(session.status(), TerminalStatus::Controlling);
        }
        // All Move assertions precede the client's explicit carrier shutdown.
        teardown.store(true, Ordering::SeqCst);
        drop(session);
        server.finish()?;
    }
    Ok(())
}

/// Both legal same-tick orders at the taken forward-expiry boundary, forced by the
/// `CommandMailbox` test gates (`pause_next_ui_reply_await_sample_for_test` parks the
/// worker at its reply sample; `pause_next_ui_reply_deposit_for_test` parks the UI at its
/// deposit sample):
///   * UI-first — the tick takes and deposits while the worker is parked, so the deposit
///     marks the cell `ForwardExpired` and the tick returns `false`; releasing the worker
///     lets it consume the mark and retire the mailbox with `Deadline`.
///   * worker-first — the worker is parked at its reply-sample pause before the tick;
///     the tick's deposit parks at its clock sample outside the mailbox mutex, a scoped
///     controller then releases the worker, whose expiry retires the session and closes
///     the mailbox with `Deadline`; the resumed deposit observes `Closed` and the same
///     tick returns `true`.
///
/// An unsynchronized `assert!(!session.tick())` oracle is not a correctness boundary:
/// `tick() == true` only reports that a UI state change happened — here the worker-first
/// order retires inside the same tick — so both orders must be driven and judged
/// separately.
#[test]
fn native_ui_leaves_a_taken_forward_expiry_for_worker_retirement() -> TestResult {
    run_taken_forward_expiry_ui_first()?;
    run_taken_forward_expiry_worker_first()?;
    run_taken_forward_expiry_released_early()
}

/// A native session with a published `CreatePrecommit` worker event: the UI has not taken
/// it and the worker is inside its reply wait — the point where the two legal same-tick
/// expiry orders diverge.
struct TakenForwardExpiryFixture {
    server: FakeServer,
    session: TerminalSession,
    mailbox: Arc<mailbox::CommandMailbox>,
    clock_phase: Arc<AtomicU8>,
}

fn taken_forward_expiry_fixture() -> Result<TakenForwardExpiryFixture, Box<dyn std::error::Error>> {
    let geometry = fixture_geometry()?;
    let (ordinary_poll_sent, ordinary_poll_seen) = mpsc::sync_channel(0);
    let (release_ordinary_poll, ordinary_poll_released) = mpsc::sync_channel(0);
    let (precommit_sent, precommit_written) = mpsc::sync_channel(0);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;

        let ClientMessage::TerminalViewer(poll) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected ordinary Poll"));
        };
        ordinary_poll_sent.send(()).map_err(io::Error::other)?;
        ordinary_poll_released.recv().map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            )),
        )
        .map_err(io::Error::other)?;

        let WorkbenchRequest::Command(command) =
            read_workbench_request_absorbing_probes(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected tab.create"));
        };
        assert_eq!(command.request_id.as_str(), native_identity(0x109));
        assert!(matches!(command.action, WorkbenchAction::TabCreate { .. }));
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id,
                outcome: WorkbenchCommandOutcome::Denied {},
            }),
        )
        .map_err(io::Error::other)?;
        precommit_sent.send(()).map_err(io::Error::other)?;
        let mut trailing = Vec::new();
        stream.read_to_end(&mut trailing)?;
        assert!(trailing.is_empty());
        Ok(())
    })?;
    let clock_phase = Arc::new(AtomicU8::new(0));
    let mut client = crate::local_client::test_server::connect_with_next_ui_sample_expiry_clock(
        server.endpoint(),
        "test-cohort",
        Arc::clone(&clock_phase),
    )?;
    let clock = client.operation_clock();
    client.set_submission_deadline(Some(
        clock
            .now()
            .checked_add(Duration::from_secs(50))
            .ok_or("startup deadline")?,
    ));
    let (mut session, mailbox) = native_session_with_identity(client, geometry)?;
    assert!(spin_with_draw(&mut session, |session| {
        session.status() == TerminalStatus::Controlling
    }));
    ordinary_poll_seen
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("ordinary Poll was not observed: {error}"))?;

    let mut client_view = ClientView::new(120);
    assert_eq!(
        dispatch_input(
            &mut InputDispatcher::default(),
            &mut client_view,
            b"\x02c",
            &mut session,
            Instant::now(),
            |_view, _session| Ok::<(), std::convert::Infallible>(()),
        )?,
        DispatchOutcome::Continue
    );
    release_ordinary_poll.send(())?;
    let poll_deadline = Instant::now() + Duration::from_secs(1);
    while !mailbox.event_waiting_for_test() {
        assert!(
            Instant::now() < poll_deadline,
            "ordinary Poll result was not published"
        );
        thread::yield_now();
    }
    assert!(!session.tick());
    precommit_written
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("precommit response was not written: {error}"))?;
    let event_deadline = Instant::now() + Duration::from_secs(1);
    while !mailbox.event_waiting_for_test() {
        assert!(
            Instant::now() < event_deadline,
            "Create precommit event was not published"
        );
        thread::yield_now();
    }
    assert!(mailbox.await_condition_waiter_before(Instant::now() + Duration::from_secs(1)));
    Ok(TakenForwardExpiryFixture {
        server,
        session,
        mailbox,
        clock_phase,
    })
}

fn run_taken_forward_expiry_ui_first() -> TestResult {
    let TakenForwardExpiryFixture {
        server,
        mut session,
        mailbox,
        clock_phase,
    } = taken_forward_expiry_fixture()?;

    let worker_pause = mailbox.pause_next_ui_reply_await_sample_for_test();
    assert!(
        worker_pause.await_reached_before(Instant::now() + Duration::from_secs(2)),
        "worker did not reach the armed reply-sample pause"
    );

    clock_phase.store(1, Ordering::SeqCst);
    assert!(!session.tick());
    let TerminalSessionView::Native(view) = session.view() else {
        panic!("standalone Session must retain its native view");
    };
    assert_eq!(view.phase(), NativeDisplayPhase::Creating);
    assert_eq!(view.retirement(), None);
    assert_eq!(clock_phase.load(Ordering::SeqCst), 3);
    assert_eq!(mailbox.is_closed(), None);
    drop(worker_pause);

    let worker_deadline = Instant::now() + Duration::from_secs(1);
    while mailbox.is_closed().is_none() {
        assert!(
            Instant::now() < worker_deadline,
            "worker did not consume the exact expired UI receipt"
        );
        thread::yield_now();
    }
    assert_eq!(mailbox.is_closed(), Some(CarrierFailure::Deadline));
    assert!(session.tick());
    assert_eq!(session.status(), TerminalStatus::Unavailable);
    drop(session);
    server.finish()?;
    Ok(())
}

/// Worker-first ordering: the worker is physically parked at its reply-sample pause —
/// outside the mailbox mutex — before `clock_phase` is armed and before the UI thread's
/// `session.tick` begins, so the tick's `take_event_before` cannot hit `WouldBlock`,
/// re-sample into the expired phase, and expire the untaken event. A scoped controller
/// owns both release guards, waits (atomics only, never the mailbox mutex) for the
/// tick's deposit to reach its sample checkpoint — which also proves the initial take
/// already completed — then releases the worker; the worker's own expiry retires the
/// session and closes the mailbox with `Deadline`, which resumes the parked deposit and
/// retires the session view inside this same tick. Every controller exit path drops both
/// guards before the scope joins, so a failing or early tick still unblocks both waits
/// within their wall bounds.
fn run_taken_forward_expiry_worker_first() -> TestResult {
    let TakenForwardExpiryFixture {
        server,
        mut session,
        mailbox,
        clock_phase,
    } = taken_forward_expiry_fixture()?;

    let worker_pause = mailbox.pause_next_ui_reply_await_sample_for_test();
    assert!(
        worker_pause.await_reached_before(Instant::now() + Duration::from_secs(2)),
        "worker did not reach the armed reply-sample pause"
    );
    let deposit_gate = mailbox.pause_next_ui_reply_deposit_for_test();
    clock_phase.store(1, Ordering::SeqCst);
    thread::scope(|scope| {
        let controller_mailbox = Arc::clone(&mailbox);
        scope.spawn(move || {
            assert!(
                deposit_gate.await_reached_before(Instant::now() + Duration::from_secs(2)),
                "the tick's deposit never reached the armed sample gate"
            );
            drop(worker_pause);
            let close_deadline = Instant::now() + Duration::from_secs(2);
            while controller_mailbox.is_closed().is_none() && Instant::now() < close_deadline {
                thread::yield_now();
            }
            assert_eq!(
                controller_mailbox.is_closed(),
                Some(CarrierFailure::Deadline),
                "the worker did not close the mailbox with Deadline while the deposit was parked"
            );
            drop(deposit_gate);
        });
        assert!(session.tick());
    });
    assert_eq!(clock_phase.load(Ordering::SeqCst), 3);
    assert_eq!(mailbox.is_closed(), Some(CarrierFailure::Deadline));
    assert_eq!(session.status(), TerminalStatus::Unavailable);
    drop(session);
    server.finish()?;
    Ok(())
}

/// Early-release check for the deposit gate: with the worker parked at its reply sample and
/// the tick parked at the deposit sample, dropping the deposit guard from a scoped releaser
/// resumes the same tick — which then marks `ForwardExpired` exactly as the UI-first order —
/// while the parked worker still cannot run. Releasing the worker afterwards completes the
/// same retirement.
fn run_taken_forward_expiry_released_early() -> TestResult {
    let TakenForwardExpiryFixture {
        server,
        mut session,
        mailbox,
        clock_phase,
    } = taken_forward_expiry_fixture()?;

    let worker_pause = mailbox.pause_next_ui_reply_await_sample_for_test();
    assert!(
        worker_pause.await_reached_before(Instant::now() + Duration::from_secs(2)),
        "worker did not reach the armed reply-sample pause"
    );
    let deposit_gate = mailbox.pause_next_ui_reply_deposit_for_test();
    clock_phase.store(1, Ordering::SeqCst);
    thread::scope(|scope| {
        scope.spawn(move || {
            assert!(
                deposit_gate.await_reached_before(Instant::now() + Duration::from_secs(2)),
                "the tick's deposit did not reach the armed sample gate"
            );
            drop(deposit_gate);
        });
        assert!(!session.tick());
    });
    let TerminalSessionView::Native(view) = session.view() else {
        panic!("standalone Session must retain its native view");
    };
    assert_eq!(view.phase(), NativeDisplayPhase::Creating);
    assert_eq!(view.retirement(), None);
    assert_eq!(clock_phase.load(Ordering::SeqCst), 3);
    assert_eq!(mailbox.is_closed(), None);
    drop(worker_pause);

    let worker_deadline = Instant::now() + Duration::from_secs(1);
    while mailbox.is_closed().is_none() {
        assert!(
            Instant::now() < worker_deadline,
            "worker did not consume the exact expired UI receipt"
        );
        thread::yield_now();
    }
    assert_eq!(mailbox.is_closed(), Some(CarrierFailure::Deadline));
    assert!(session.tick());
    assert_eq!(session.status(), TerminalStatus::Unavailable);
    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn native_worker_commit_survives_ui_deadline_retirement_and_join() -> TestResult {
    let geometry = fixture_geometry()?;
    let (ordinary_poll_sent, ordinary_poll_seen) = mpsc::sync_channel(0);
    let (release_ordinary_poll, ordinary_poll_released) = mpsc::sync_channel(0);
    let (commit_sent, commit_written) = mpsc::sync_channel(0);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;

        let ClientMessage::TerminalViewer(poll) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected ordinary Poll"));
        };
        ordinary_poll_sent.send(()).map_err(io::Error::other)?;
        ordinary_poll_released.recv().map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            )),
        )
        .map_err(io::Error::other)?;

        let WorkbenchRequest::Command(command) =
            read_workbench_request_absorbing_probes(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected tab.create"));
        };
        assert_eq!(command.request_id.as_str(), native_identity(0x109));
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id,
                outcome: WorkbenchCommandOutcome::TabCreatePending {
                    revision: WorkbenchRevision::new(2),
                    terminal_context_id: WorkbenchTerminalContextId::parse(ID)
                        .map_err(io::Error::other)?,
                    tab_id: WorkbenchTabId::parse(CREATED_TAB_ID).map_err(io::Error::other)?,
                    panel_id: WorkbenchPanelId::parse(CREATED_PANEL_ID)
                        .map_err(io::Error::other)?,
                    presentation_id: TerminalPresentationId::parse(CREATED_PRESENTATION_ID)
                        .map_err(io::Error::other)?,
                },
            }),
        )
        .map_err(io::Error::other)?;
        commit_sent.send(()).map_err(io::Error::other)?;
        let mut trailing = Vec::new();
        stream.read_to_end(&mut trailing)?;
        assert!(trailing.is_empty(), "expired commit must not start Detach");
        Ok(())
    })?;
    let clock_phase = Arc::new(AtomicU8::new(0));
    let mut client = crate::local_client::test_server::connect_with_next_ui_sample_expiry_clock(
        server.endpoint(),
        "test-cohort",
        Arc::clone(&clock_phase),
    )?;
    let clock = client.operation_clock();
    client.set_submission_deadline(Some(
        clock
            .now()
            .checked_add(Duration::from_secs(50))
            .ok_or("startup deadline")?,
    ));
    let (mut session, mailbox) = native_session_with_identity(client, geometry)?;
    assert!(spin_with_draw(&mut session, |session| {
        session.status() == TerminalStatus::Controlling
    }));
    ordinary_poll_seen
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("ordinary Poll was not observed: {error}"))?;

    let mut client_view = ClientView::new(120);
    assert_eq!(
        dispatch_input(
            &mut InputDispatcher::default(),
            &mut client_view,
            b"\x02c",
            &mut session,
            Instant::now(),
            |_view, _session| Ok::<(), std::convert::Infallible>(()),
        )?,
        DispatchOutcome::Continue
    );
    release_ordinary_poll.send(())?;
    let poll_deadline = Instant::now() + Duration::from_secs(1);
    while !mailbox.event_waiting_for_test() {
        assert!(
            Instant::now() < poll_deadline,
            "ordinary Poll result was not published"
        );
        thread::yield_now();
    }
    assert!(!session.tick());
    commit_written
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("committed response was not written: {error}"))?;
    let event_deadline = Instant::now() + Duration::from_secs(1);
    while !mailbox.event_waiting_for_test() {
        assert!(
            Instant::now() < event_deadline,
            "CreateCommittedKnown event was not published"
        );
        thread::yield_now();
    }
    assert!(mailbox.await_condition_waiter_before(Instant::now() + Duration::from_secs(1)));
    assert!(matches!(
        session.view(),
        TerminalSessionView::Native(view) if view.phase() == NativeDisplayPhase::Creating
    ));

    clock_phase.store(1, Ordering::SeqCst);
    assert!(session.tick());
    assert_eq!(clock_phase.load(Ordering::SeqCst), 3);
    assert_eq!(mailbox.is_closed(), Some(CarrierFailure::Deadline));
    let TerminalSessionView::Native(view) = session.view() else {
        panic!("standalone Session must retain its native view");
    };
    assert_eq!(view.phase(), NativeDisplayPhase::Retired);
    assert_eq!(
        view.retirement(),
        Some(NativeDisplayRetirement::Unavailable)
    );
    let SessionBackend::Native(native) = &session.backend else {
        panic!("standalone Session must retain its native UI state");
    };
    assert!(native.committed_target_retained_for_test());

    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn semantic_create_retirement_finishes_before_the_tick_returns() -> TestResult {
    let geometry = fixture_geometry()?;
    let (ordinary_poll_sent, ordinary_poll_seen) = mpsc::sync_channel(0);
    let (release_ordinary_poll, ordinary_poll_released) = mpsc::sync_channel(0);
    let (snapshot_sent, snapshot_written) = mpsc::sync_channel(0);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;

        let ClientMessage::TerminalViewer(poll) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected ordinary Poll"));
        };
        ordinary_poll_sent.send(()).map_err(io::Error::other)?;
        ordinary_poll_released.recv().map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            )),
        )
        .map_err(io::Error::other)?;

        let WorkbenchRequest::Command(command) =
            read_workbench_request_absorbing_probes(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected tab.create"));
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id,
                outcome: WorkbenchCommandOutcome::TabCreatePending {
                    revision: WorkbenchRevision::new(2),
                    terminal_context_id: WorkbenchTerminalContextId::parse(ID)
                        .map_err(io::Error::other)?,
                    tab_id: WorkbenchTabId::parse(CREATED_TAB_ID).map_err(io::Error::other)?,
                    panel_id: WorkbenchPanelId::parse(CREATED_PANEL_ID)
                        .map_err(io::Error::other)?,
                    presentation_id: TerminalPresentationId::parse(CREATED_PRESENTATION_ID)
                        .map_err(io::Error::other)?,
                },
            }),
        )
        .map_err(io::Error::other)?;

        respond_with_read(&mut stream, available_snapshot(Some(binding())))?;
        snapshot_sent.send(()).map_err(io::Error::other)?;
        let mut trailing = Vec::new();
        stream.read_to_end(&mut trailing)?;
        assert!(trailing.is_empty());
        Ok(())
    })?;
    let (mut session, mailbox) = native_session_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| {
        session.status() == TerminalStatus::Controlling
    }));
    ordinary_poll_seen
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("ordinary Poll was not observed: {error}"))?;
    let mut client_view = ClientView::new(120);
    assert_eq!(
        dispatch_input(
            &mut InputDispatcher::default(),
            &mut client_view,
            b"\x02c",
            &mut session,
            Instant::now(),
            |_view, _session| Ok::<(), std::convert::Infallible>(()),
        )?,
        DispatchOutcome::Continue
    );
    release_ordinary_poll.send(())?;
    spin(&mut session, |session| {
        matches!(
            session.view(),
            TerminalSessionView::Native(view)
                if view.phase() == NativeDisplayPhase::CreateCommittedNeedDetach
        )
    });
    snapshot_written
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("divergent snapshot was not written: {error}"))?;
    let event_deadline = Instant::now() + Duration::from_secs(1);
    while !mailbox.event_waiting_for_test() {
        assert!(
            Instant::now() < event_deadline,
            "CreateSnapshotResult event was not published"
        );
        thread::yield_now();
    }
    assert!(session.task.is_some());
    assert!(
        session.tick(),
        "the exact semantic Retire event must redraw"
    );
    assert!(matches!(
        session.view(),
        TerminalSessionView::Native(view)
            if view.phase() == NativeDisplayPhase::Retired
                && view.retirement() == Some(NativeDisplayRetirement::Unavailable)
    ));

    assert!(
        session.task.is_none(),
        "worker must be joined before read access"
    );
    assert_eq!(
        Arc::strong_count(&mailbox),
        1,
        "session must release its mailbox after finalization"
    );
    let SessionBackend::Native(native) = &session.backend else {
        panic!("standalone Session must retain its native UI state");
    };
    assert!(native.committed_target_retained_for_test());
    assert_eq!(native.latest_geometry_for_test(), Some(geometry));
    let later_geometry = Geometry::new(
        geometry.columns() + 1,
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )?;
    session.observe_geometry(later_geometry);
    let SessionBackend::Native(native) = &session.backend else {
        unreachable!()
    };
    assert_eq!(
        native.latest_geometry_for_test(),
        Some(geometry),
        "a completed mutable entrypoint must freeze retired state"
    );

    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn closed_action_admission_preserves_a_worker_pending_commit() -> TestResult {
    let geometry = fixture_geometry()?;
    let (ordinary_poll_sent, ordinary_poll_seen) = mpsc::sync_channel(0);
    let (release_ordinary_poll, ordinary_poll_released) = mpsc::sync_channel(0);
    let (commit_sent, commit_written) = mpsc::sync_channel(0);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;

        let ClientMessage::TerminalViewer(poll) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected ordinary Poll"));
        };
        ordinary_poll_sent.send(()).map_err(io::Error::other)?;
        ordinary_poll_released.recv().map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            )),
        )
        .map_err(io::Error::other)?;

        let WorkbenchRequest::Command(command) =
            read_workbench_request_absorbing_probes(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected tab.create"));
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id,
                outcome: WorkbenchCommandOutcome::TabCreatePending {
                    revision: WorkbenchRevision::new(2),
                    terminal_context_id: WorkbenchTerminalContextId::parse(ID)
                        .map_err(io::Error::other)?,
                    tab_id: WorkbenchTabId::parse(CREATED_TAB_ID).map_err(io::Error::other)?,
                    panel_id: WorkbenchPanelId::parse(CREATED_PANEL_ID)
                        .map_err(io::Error::other)?,
                    presentation_id: TerminalPresentationId::parse(CREATED_PRESENTATION_ID)
                        .map_err(io::Error::other)?,
                },
            }),
        )
        .map_err(io::Error::other)?;
        commit_sent.send(()).map_err(io::Error::other)?;
        let mut trailing = Vec::new();
        stream.read_to_end(&mut trailing)?;
        assert!(
            trailing.is_empty(),
            "closed admission must not start Detach"
        );
        Ok(())
    })?;
    let (mut session, mailbox) = native_session_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| {
        session.status() == TerminalStatus::Controlling
    }));
    ordinary_poll_seen
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("ordinary Poll was not observed: {error}"))?;
    let mut client_view = ClientView::new(120);
    assert_eq!(
        dispatch_input(
            &mut InputDispatcher::default(),
            &mut client_view,
            b"\x02c",
            &mut session,
            Instant::now(),
            |_view, _session| Ok::<(), std::convert::Infallible>(()),
        )?,
        DispatchOutcome::Continue
    );
    release_ordinary_poll.send(())?;
    let poll_deadline = Instant::now() + Duration::from_secs(1);
    while !mailbox.event_waiting_for_test() {
        assert!(Instant::now() < poll_deadline);
        thread::yield_now();
    }
    assert!(!session.tick());
    commit_written
        .recv_timeout(Duration::from_secs(1))
        .map_err(|error| format!("committed response was not written: {error}"))?;
    let event_deadline = Instant::now() + Duration::from_secs(1);
    while !mailbox.event_waiting_for_test() {
        assert!(
            Instant::now() < event_deadline,
            "CreateCommittedKnown event was not published"
        );
        thread::yield_now();
    }
    mailbox.close(CarrierFailure::Deadline);

    assert_eq!(
        TerminalInputPort::apply_client_action(&mut session, ClientAction::CreateTab),
        Some(SessionActionDisposition::KeepNavigator)
    );
    assert!(session.task.is_none());
    let SessionBackend::Native(native) = &session.backend else {
        panic!("standalone Session must retain its native UI state");
    };
    assert!(native.committed_target_retained_for_test());
    assert_eq!(native.view().phase(), NativeDisplayPhase::Retired);
    assert_eq!(
        native.view().retirement(),
        Some(NativeDisplayRetirement::Unavailable)
    );

    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn drop_interrupts_an_inflight_exchange_and_joins_its_owner() -> TestResult {
    let (sent, received) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream)?;
        attach(&mut stream)?;
        let _acquire = read_client_message(&mut stream).map_err(io::Error::other)?;
        sent.send(()).map_err(io::Error::other)?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        assert!(bytes.is_empty());
        Ok(())
    })?;
    let session = TerminalSession::start(
        LocalClient::connect(server.endpoint(), "test-cohort")?,
        TerminalIntent::Attach(RequestId::parse(ID)?),
        geometry()?,
    )?;
    received.recv_timeout(Duration::from_secs(1))?;
    let start = Instant::now();
    drop(session);
    assert!(start.elapsed() < Duration::from_secs(1));
    server.finish()?;
    Ok(())
}
#[test]
fn lost_input_result_retires_without_replaying_the_effect() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        admit(&mut stream)?;
        attach(&mut stream)?;
        acquire(&mut stream, &attached_viewer_id()?)?;
        loop {
            match read_client_message(&mut stream).map_err(io::Error::other)? {
                ClientMessage::TerminalViewer(request) => write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        TerminalViewerOutcome::Empty {},
                    )),
                )
                .map_err(io::Error::other)?,
                ClientMessage::TerminalControl(request) => {
                    if let TerminalControlAction::Resize {
                        attachment_id,
                        geometry,
                        ..
                    } = request.action()
                    {
                        assert_eq!(attachment_id, &attached_viewer_id()?);
                        write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                request.request_id().clone(),
                                TerminalControlOutcome::Resized {
                                    geometry: *geometry,
                                },
                            )),
                        )
                        .map_err(io::Error::other)?;
                        continue;
                    }
                    assert!(
                        matches!(request.action(), TerminalControlAction::Input {attachment_id, bytes,..} if attachment_id == &attached_viewer_id()? && bytes.as_bytes()==b"do-once")
                    );
                    return Ok(());
                }
                _ => return Err(io::Error::other("unexpected request")),
            }
        }
    })?;
    let mut session = TerminalSession::start(
        LocalClient::connect(server.endpoint(), "test-cohort")?,
        TerminalIntent::Attach(RequestId::parse(ID)?),
        geometry()?,
    )?;
    spin(&mut session, |session| {
        legacy_status(session) == Some(TerminalStatus::Controlling)
    });
    assert_eq!(session.status(), TerminalStatus::Blocked);
    session.forward(b"do-once").map_err(|_| "input")?;
    spin(&mut session, |s| {
        s.status() == TerminalStatus::OutcomeUnknown
    });
    assert!(session.forward(b"do-once").is_err());
    drop(session);
    server.finish()?;
    Ok(())
}
#[test]
fn input_admission_is_bounded_and_failure_cannot_accept_more_work() -> TestResult {
    let (shutdown, _peer) = UnixStream::pair()?;
    let (commands, _receiver) = mpsc::sync_channel(8);
    let mut session = TerminalSession {
        #[cfg(windows)]
        position_writer: None,
        #[cfg(windows)]
        position_startup: None,
        #[cfg(windows)]
        configuration: None,
        #[cfg(windows)]
        notifications: None,
        #[cfg(windows)]
        window_title: None,
        #[cfg(windows)]
        client_control: None,
        backend: SessionBackend::Legacy(Box::new(LegacySession {
            commands: Some(commands),
            events: None,
            status: TerminalStatus::Controlling,
            projection: None,
            feedback: None,
            poll_pending: true,
            geometry: geometry()?,
        })),
        task: None,
        shutdown: crate::local_client::test_server::shutdown_handle(shutdown),
        retirement_redraw_pending: false,
        #[cfg(any(target_os = "linux", windows))]
        origin: None,
        #[cfg(any(target_os = "linux", windows))]
        recovery_shutdown: None,
    };
    for _ in 0..8 {
        assert!(session.forward(&[255; 4096]).is_ok());
    }
    assert!(session.forward(b"not-submitted").is_err());
    if let SessionBackend::Legacy(legacy) = &mut session.backend {
        legacy.retire(TerminalStatus::OutcomeUnknown, &session.shutdown);
    }
    assert!(session.forward(b"not-submitted").is_err());
    Ok(())
}

#[test]
fn create_uses_default_intent_and_a_busy_controller_stays_read_only() -> TestResult {
    use orchbee_client_server_protocol::{
        ControlRejectedReason, CreateAcceptedState, CreateOutcome, StartMode, TerminalCreateResult,
    };
    let server = FakeServer::start(|mut stream| {
        admit(&mut stream)?;
        let ClientMessage::TerminalCreate(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected Create"));
        };
        assert!(request.worker_constraint().is_none());
        assert!(request.cwd().is_none());
        let StartMode::Immediate { geometry } = request.start_mode() else {
            return Err(io::Error::other("expected immediate"));
        };
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
                request.request_id().clone(),
                CreateOutcome::Accepted {
                    state: CreateAcceptedState::Started {
                        geometry: *geometry,
                    },
                },
            )),
        )
        .map_err(io::Error::other)?;
        attach(&mut stream)?;
        let ClientMessage::TerminalControl(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected acquire"));
        };
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Rejected {
                    reason: ControlRejectedReason::ControllerBusy,
                },
            )),
        )
        .map_err(io::Error::other)?;
        loop {
            match read_client_message(&mut stream) {
                Ok(ClientMessage::TerminalViewer(request)) => {
                    let _result = write_server_message(
                        &mut stream,
                        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                            request.request_id().clone(),
                            TerminalViewerOutcome::Empty {},
                        )),
                    );
                }
                Err(_) => return Ok(()),
                _ => return Err(io::Error::other("read-only Client submitted control")),
            }
        }
    })?;
    let mut session = TerminalSession::start(
        LocalClient::connect(server.endpoint(), "test-cohort")?,
        TerminalIntent::Create { cwd: None },
        geometry()?,
    )?;
    spin(&mut session, |session| {
        legacy_status(session) == Some(TerminalStatus::ReadOnly)
    });
    assert_eq!(session.status(), TerminalStatus::Blocked);
    assert!(session.forward(b"never submitted").is_err());
    session.observe_geometry(Geometry::new(100, 30, 8, 16)?);
    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn standalone_selects_the_snapshot_binding_and_never_replays_unknown_open() -> TestResult {
    for unknown in [false, true] {
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let ClientMessage::TerminalOpen(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected Open"));
            };
            assert_eq!(
                request.geometry(),
                Geometry::new(80, 24, 8, 16).map_err(io::Error::other)?
            );
            let open_request_id = request.request_id().as_str().to_owned();
            let reference = RequestId::parse(ID).map_err(io::Error::other)?;
            let outcome = if unknown {
                orchbee_client_server_protocol::OpenOutcome::OutcomeUnknown {}
            } else {
                orchbee_client_server_protocol::OpenOutcome::Opened {
                    terminal_create_request_id: reference.clone(),
                }
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalOpenResult(
                    orchbee_client_server_protocol::TerminalOpenResult::new(
                        request.request_id().clone(),
                        outcome,
                    ),
                ),
            )
            .map_err(io::Error::other)?;
            if !unknown {
                let workbench_request_id =
                    respond_with_snapshot(&mut stream, available_snapshot(Some(binding())))?;
                assert_ne!(workbench_request_id, open_request_id);
                let ClientMessage::TerminalViewer(request) =
                    read_client_message(&mut stream).map_err(io::Error::other)?
                else {
                    return Err(io::Error::other("expected AttachBinding"));
                };
                assert_eq!(
                    request.action(),
                    &TerminalViewerAction::AttachBinding {
                        site_id: orchbee_client_server_protocol::SiteId::parse(SITE_ID)
                            .map_err(io::Error::other)?,
                        worker_id: orchbee_client_server_protocol::WorkerId::parse(WORKER_ID)
                            .map_err(io::Error::other)?,
                        terminal_id: orchbee_client_server_protocol::TerminalId::parse(TERMINAL_ID)
                            .map_err(io::Error::other)?,
                    }
                );
                assert_ne!(request.request_id().as_str(), open_request_id);
                assert_ne!(request.request_id().as_str(), workbench_request_id);
                assert_ne!(TERMINAL_ID, reference.as_str());
                write_server_message(&mut stream, &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(request.request_id().clone(), TerminalViewerOutcome::Rejected { reason: orchbee_client_server_protocol::ViewerRejectedReason::TerminalUnavailable }))).map_err(io::Error::other)?;
            }
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest)?;
            assert!(rest.is_empty(), "no second Open or implicit Create");
            Ok(())
        })?;
        let mut session = TerminalSession::start(
            native_client(&server)?,
            TerminalIntent::Standalone,
            geometry()?,
        )?;
        let expected = if unknown {
            TerminalStatus::OutcomeUnknown
        } else {
            TerminalStatus::Unavailable
        };
        spin(&mut session, |session| session.status() == expected);
        drop(session);
        server.finish()?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum SnapshotFailure {
    Denied,
    Unavailable,
    UnsupportedShape,
    WrongKind,
}

#[test]
fn standalone_snapshot_failures_are_unavailable_without_an_attachment_fallback() -> TestResult {
    for failure in [
        SnapshotFailure::Denied,
        SnapshotFailure::Unavailable,
        SnapshotFailure::UnsupportedShape,
        SnapshotFailure::WrongKind,
    ] {
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let ClientMessage::TerminalOpen(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected Open"));
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalOpenResult(
                    orchbee_client_server_protocol::TerminalOpenResult::new(
                        request.request_id().clone(),
                        orchbee_client_server_protocol::OpenOutcome::Opened {
                            terminal_create_request_id: RequestId::parse(ID)
                                .map_err(io::Error::other)?,
                        },
                    ),
                ),
            )
            .map_err(io::Error::other)?;
            match failure {
                SnapshotFailure::Denied => {
                    respond_with_snapshot(&mut stream, WorkbenchSnapshotOutcome::Denied {})?;
                }
                SnapshotFailure::Unavailable => {
                    respond_with_snapshot(
                        &mut stream,
                        WorkbenchSnapshotOutcome::Unavailable {
                            reason: orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                        },
                    )?;
                }
                SnapshotFailure::UnsupportedShape => {
                    respond_with_snapshot(&mut stream, available_snapshot(None))?;
                }
                SnapshotFailure::WrongKind => {
                    let WorkbenchRequest::SnapshotRead(request) =
                        read_workbench_request(&mut stream).map_err(io::Error::other)?
                    else {
                        return Err(io::Error::other("expected SnapshotRead"));
                    };
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::Command(WorkbenchCommandResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            outcome: WorkbenchCommandOutcome::Denied {},
                        }),
                    )
                    .map_err(io::Error::other)?;
                }
            }
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest)?;
            assert!(
                rest.is_empty(),
                "snapshot failure must not attach by any selector"
            );
            Ok(())
        })?;
        let mut session = TerminalSession::start(
            native_client(&server)?,
            TerminalIntent::Standalone,
            geometry()?,
        )?;
        spin(&mut session, |session| {
            session.status() == TerminalStatus::Unavailable
        });
        drop(session);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn standalone_geometry_changes_require_a_separate_controller_grant() -> TestResult {
    for controlling in [false, true] {
        let (resized, observed_resize) = mpsc::sync_channel(1);
        let initial_geometry = fixture_geometry()?;
        let changed_geometry = Geometry::new(
            initial_geometry.columns().checked_add(1).ok_or("columns")?,
            initial_geometry.rows(),
            initial_geometry.cell_pixel_width(),
            initial_geometry.cell_pixel_height(),
        )?;
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let ClientMessage::TerminalOpen(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected Open"));
            };
            let geometry = request.geometry();
            assert_eq!(geometry, initial_geometry);
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalOpenResult(
                    orchbee_client_server_protocol::TerminalOpenResult::new(
                        request.request_id().clone(),
                        orchbee_client_server_protocol::OpenOutcome::Opened {
                            terminal_create_request_id: RequestId::parse(ID)
                                .map_err(io::Error::other)?,
                        },
                    ),
                ),
            )
            .map_err(io::Error::other)?;
            respond_with_snapshot(&mut stream, available_snapshot(Some(binding())))?;
            attach_binding(&mut stream)?;
            if controlling {
                acquire(&mut stream, &binding_viewer_id()?)?;
                let ClientMessage::TerminalControl(request) =
                    read_client_message(&mut stream).map_err(io::Error::other)?
                else {
                    return Err(io::Error::other("expected separate controller Resize"));
                };
                assert!(
                    matches!(request.action(), TerminalControlAction::Resize { attachment_id, geometry: actual, sequence, .. } if attachment_id == &binding_viewer_id()? && *actual == geometry && sequence.get() == 1)
                );
                write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                        request.request_id().clone(),
                        TerminalControlOutcome::Resized { geometry },
                    )),
                )
                .map_err(io::Error::other)?;
            } else {
                let ClientMessage::TerminalControl(request) =
                    read_client_message(&mut stream).map_err(io::Error::other)?
                else {
                    return Err(io::Error::other("expected Acquire"));
                };
                assert_eq!(
                    request.action(),
                    &TerminalControlAction::Acquire {
                        attachment_id: binding_viewer_id()?,
                    }
                );
                write_server_message(&mut stream, &ServerMessage::TerminalControlResult(TerminalControlResult::new(request.request_id().clone(), TerminalControlOutcome::Rejected { reason: orchbee_client_server_protocol::ControlRejectedReason::ControllerBusy }))).map_err(io::Error::other)?;
            }
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected first Poll"));
            };
            assert_eq!(
                request.action(),
                &TerminalViewerAction::Poll {
                    attachment_id: binding_viewer_id()?,
                }
            );
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    TerminalViewerOutcome::Record(
                        full_frame_record(1, 1, initial_geometry)
                            .map_err(|error| io::Error::other(error.to_string()))?,
                    ),
                )),
            )
            .map_err(io::Error::other)?;
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected FrameApplied"));
            };
            assert!(matches!(
                request.action(),
                TerminalViewerAction::FrameApplied { .. }
            ));
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    TerminalViewerOutcome::FeedbackAccepted {},
                )),
            )
            .map_err(io::Error::other)?;
            let mut saw_changed_resize = false;
            loop {
                match read_client_message(&mut stream) {
                    Ok(ClientMessage::TerminalViewer(request))
                        if matches!(
                            request.action(),
                            TerminalViewerAction::Poll { attachment_id }
                                if attachment_id == &binding_viewer_id()?
                        ) =>
                    {
                        let _written = write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::Empty {},
                            )),
                        );
                    }
                    Ok(ClientMessage::TerminalControl(request)) if controlling => {
                        assert!(matches!(
                            request.action(),
                            TerminalControlAction::Resize {
                                attachment_id,
                                geometry,
                                sequence,
                                ..
                            } if attachment_id == &binding_viewer_id()?
                                && *geometry == changed_geometry
                                && sequence.get() == 2
                        ));
                        write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                request.request_id().clone(),
                                TerminalControlOutcome::Resized {
                                    geometry: changed_geometry,
                                },
                            )),
                        )
                        .map_err(io::Error::other)?;
                        saw_changed_resize = true;
                        resized.send(()).map_err(io::Error::other)?;
                    }
                    Err(_) => {
                        assert_eq!(saw_changed_resize, controlling);
                        return Ok(());
                    }
                    Ok(_) => {
                        return Err(io::Error::other(
                            "unexpected Resize or other effect without controller authority",
                        ));
                    }
                }
            }
        })?;
        let mut session = TerminalSession::start(
            native_client(&server)?,
            TerminalIntent::Standalone,
            initial_geometry,
        )?;
        let expected = if controlling {
            TerminalStatus::Controlling
        } else {
            TerminalStatus::ReadOnly
        };
        if !spin_with_draw(&mut session, |session| session.status() == expected) {
            let status = session.status();
            let phase = match session.view() {
                TerminalSessionView::Native(view) => Some(view.phase()),
                TerminalSessionView::Legacy { .. } => None,
            };
            drop(session);
            return Err(format!(
                "session did not settle: controlling={controlling}, status={status:?}, phase={phase:?}, server={:?}",
                server.finish()
            )
            .into());
        }
        let mut client_view = ClientView::new(120);
        let rendered = render_native_session(&session, &client_view)?;
        assert!(rendered.contains("[*1 standalone]"));
        assert!(rendered.contains(if controlling {
            "Tab 1 · Ready · controlling"
        } else {
            "Tab 1 · Ready · view only"
        }));
        open_navigator(&mut client_view);
        let navigator = render_native_session(&session, &client_view)?;
        assert!(navigator.contains("[*1] standalone"));
        session.observe_geometry(changed_geometry);
        if controlling {
            assert!(spin_with_draw(&mut session, |_session| {
                observed_resize.try_recv().is_ok()
            }));
        }
        drop(session);
        server.finish()?;
    }
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn standalone_snapshot_does_not_complete_or_reset_the_startup_deadline() -> TestResult {
    let (sent, received) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let expired = Arc::new(AtomicBool::new(false));
    let client_expired = expired.clone();
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream)?;
        let ClientMessage::TerminalOpen(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected Open"));
        };
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalOpenResult(
                orchbee_client_server_protocol::TerminalOpenResult::new(
                    request.request_id().clone(),
                    orchbee_client_server_protocol::OpenOutcome::Opened {
                        terminal_create_request_id: RequestId::parse(ID)
                            .map_err(io::Error::other)?,
                    },
                ),
            ),
        )
        .map_err(io::Error::other)?;
        respond_with_snapshot(&mut stream, available_snapshot(Some(binding())))?;
        attach_binding(&mut stream)?;
        let ClientMessage::TerminalControl(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected Acquire"));
        };
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Rejected {
                    reason: orchbee_client_server_protocol::ControlRejectedReason::ControllerBusy,
                },
            )),
        )
        .map_err(io::Error::other)?;
        let ClientMessage::TerminalViewer(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected first Poll"));
        };
        assert_eq!(
            request.action(),
            &TerminalViewerAction::Poll {
                attachment_id: binding_viewer_id()?,
            }
        );
        sent.send(()).map_err(io::Error::other)?;
        released.recv().map_err(io::Error::other)?;
        stream.write_all(&[0])?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        assert!(
            bytes.is_empty(),
            "no feedback or retry without a first frame"
        );
        Ok(())
    })?;
    let mut client = crate::local_client::test_server::connect_with_checkpoint_clock(
        server.endpoint(),
        "test-cohort",
        client_expired,
    )?;
    client.set_submission_deadline(Some(Instant::now() + Duration::from_secs(10)));
    let mut session = TerminalSession::start(client, TerminalIntent::Standalone, geometry()?)?;
    spin(&mut session, |_session| received.try_recv().is_ok());
    assert!(session.projection().is_some());
    assert!(!session.startup_draw_ready());
    expired.store(true, Ordering::SeqCst);
    release.send(())?;
    spin(&mut session, |session| {
        session.status() == TerminalStatus::OutcomeUnknown
    });
    assert!(!session.startup_draw_ready());
    let SessionBackend::Native(native) = &mut session.backend else {
        panic!("standalone Session must retain its native UI state");
    };
    native.close(CarrierFailure::Protocol);
    assert_eq!(
        native.view().retirement(),
        Some(NativeDisplayRetirement::OutcomeUnknown)
    );
    native.close(CarrierFailure::Closed);
    assert_eq!(
        native.view().retirement(),
        Some(NativeDisplayRetirement::OutcomeUnknown)
    );
    drop(session);
    server.finish()?;
    Ok(())
}
