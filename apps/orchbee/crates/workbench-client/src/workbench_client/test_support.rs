//! Fake Server peers and native Workbench Client construction shared by unit tests and the
//! owning application's shell-loop tests.

mod close;
mod rename;

pub use super::ui::test_support::{
    SettingsFrameSource,
    actual_native_shell_places_confirmed_pixels_inside_one_frame_and_clears_under_a_board,
    covering_client_owner_uses_confirmed_appearance_until_settings_returns,
    draft_appearance_reaches_background_chrome_and_program_defaults_without_runtime_effects,
    external_confirmed_values_update_unedited_presentation_and_cancel_reveals_the_latest_snapshot,
};
pub use close::{dispatch_close_input, interactive_close_server};
#[cfg(test)]
pub(crate) use close::{dispatch_close_input_at, status_close_server};
#[cfg(test)]
pub use rename::RenameResponse;
pub use rename::RenameServer;

use super::{
    ClientBackend, ClientTask, Geometry, LocalClient, WorkbenchClient, connection, io, mailbox, ui,
};
use crate::local_client::test_support::multiplex_peer::Peer as UnixStream;
use crate::{
    client_view::ClientView,
    local_client::test_support::server::FakeServer,
    terminal_input::{DispatchOutcome, InputDispatcher, dispatch_input},
};
use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, ServerMessage, TerminalControlAction, TerminalControlOutcome,
    TerminalControlResult, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerResult,
    ViewerId, read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding, decode_terminal_text_frame,
    encode_transport_envelope, inspect_terminal_frame_route,
};
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, ProtocolVersion, SessionId, Snapshot,
    Tab, TerminalContext, TerminalId, TerminalPresentationId, WorkbenchAction,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchPanelId,
    WorkbenchPublicationDocument, WorkbenchPublicationOutcome, WorkbenchPublicationResult,
    WorkbenchRequest, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult, WorkbenchTabId, WorkbenchTerminalContextId, WorkerId,
    read_workbench_request, write_workbench_response,
};
use std::io::Read;
use std::thread;
use std::{
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

pub(crate) const ID: &str = "11111111111111111111111111111111";

pub(crate) const SESSION_ID: &str = "22222222222222222222222222222222";

pub(crate) const WORKER_ID: &str = "33333333333333333333333333333333";

pub(crate) const TERMINAL_ID: &str = "44444444444444444444444444444444";

pub(crate) const CREATED_TAB_ID: &str = "55555555555555555555555555555555";

pub(crate) const NATIVE_TEST_IDENTITY_SEED: u128 = 0x100;

pub(crate) const FRAME: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../schemas/terminal/v1/fixtures/ghostty-surface.bin"
));

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

#[cfg(test)]
pub fn geometry() -> Result<Geometry, Box<dyn std::error::Error>> {
    Ok(Geometry::new(80, 24, 8, 16)?)
}

pub fn fixture_geometry() -> Result<Geometry, Box<dyn std::error::Error>> {
    let frame = decode_terminal_text_frame(FRAME)?;
    Ok(Geometry::new(
        frame.geometry.columns,
        frame.geometry.rows,
        frame.geometry.cell_pixel_width,
        frame.geometry.cell_pixel_height,
    )?)
}

pub(crate) fn full_frame_record(
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

pub(crate) fn admit(stream: &mut UnixStream) -> io::Result<()> {
    crate::local_client::test_support::handshake::answer_offer(stream)?;
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::MIN,
        },
    )
    .map_err(io::Error::other)
}

pub(crate) fn respond_with_snapshot(
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

/// Maps the latest committed snapshot outcome onto a `publication.read` reply the way a real
/// journal would: `current` when the reader already holds the offered revision, otherwise
/// `published` carrying the full snapshot as the boundary publication.
pub(crate) fn publication_outcome_for(
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

pub(crate) fn available_snapshot(binding: Option<PresentationBinding>) -> WorkbenchSnapshotOutcome {
    WorkbenchSnapshotOutcome::Available {
        snapshot: Snapshot {
            navigation: {
                let fixture_context_id = WorkbenchTerminalContextId::parse(ID).unwrap();
                let fixture_workspace_id = orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                    fixture_context_id.as_str(),
                )
                .unwrap();
                Navigation {
                    project_order: Vec::new(),
                    directories: Vec::new(),
                    workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                        terminal_directory: None,
                        automatic_label: false,
                        workspace_id: fixture_workspace_id.clone(),
                        terminal_context_id: fixture_context_id.clone(),
                        project_id: None,
                        primary_directory_id: None,
                        label: "Workspace".into(),
                    })
                    .chain(Vec::new())
                    .collect(),
                    workspace_order: Vec::new(),
                    projects: Vec::new(),
                }
            },
            protocol_version: ProtocolVersion,
            revision: WorkbenchRevision::new(1),
            session_id: SessionId::parse(SESSION_ID).unwrap(),
            terminal_contexts: vec![TerminalContext {
                groups: Vec::new(),
                tabs: vec![Tab {
                    automatic_label: false,
                    group_id: None,
                    label: "initial_workspace".into(),
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
                            launch_failure: None,
                            launch_program: None,
                        },
                    },
                    tab_id: WorkbenchTabId::parse(ID).unwrap(),
                }],
                terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
            }],
        },
    }
}

pub(crate) fn binding() -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(TERMINAL_ID).unwrap(),
        worker_id: WorkerId::parse(WORKER_ID).unwrap(),
    }
}

pub(crate) fn binding_attach_outcome() -> io::Result<TerminalViewerOutcome> {
    Ok(TerminalViewerOutcome::Attached {
        creation_id: orchbee_client_server_protocol::HostCreationId::parse(&"c".repeat(32))
            .unwrap(),
        attachment_id: ViewerId::parse(SESSION_ID).map_err(io::Error::other)?,
        host_epoch: HostEpoch::parse(ID).map_err(io::Error::other)?,
        host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(ID)
            .map_err(io::Error::other)?,
    })
}

pub(crate) fn binding_viewer_id() -> io::Result<ViewerId> {
    let TerminalViewerOutcome::Attached { attachment_id, .. } = binding_attach_outcome()? else {
        return Err(io::Error::other(
            "binding attach fixture did not contain a ViewerId",
        ));
    };
    Ok(attachment_id)
}

#[cfg(test)]
pub fn acquire(stream: &mut UnixStream, attachment_id: &ViewerId) -> io::Result<()> {
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

pub(crate) fn serve_native_controlling_frame(
    stream: &mut UnixStream,
    geometry: Geometry,
) -> io::Result<()> {
    serve_native_controlling_frame_with_snapshot(
        stream,
        geometry,
        available_snapshot(Some(binding())),
    )
}

pub(crate) fn serve_native_controlling_frame_with_snapshot(
    stream: &mut UnixStream,
    geometry: Geometry,
    initial: WorkbenchSnapshotOutcome,
) -> io::Result<()> {
    admit(stream)?;

    assert_eq!(
        respond_with_snapshot(stream, initial)?,
        native_identity(0x101)
    );
    let ClientMessage::TerminalViewer(attach) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected AttachBinding"));
    };
    assert_eq!(attach.request_id().as_str(), native_identity(0x102));
    assert_eq!(
        attach.action(),
        &TerminalViewerAction::AttachBinding {
            session_id: orchbee_client_server_protocol::SessionId::parse(SESSION_ID)
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
    assert_eq!(acquire.request_id().as_str(), native_identity(0x103));
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
    assert_eq!(resize.request_id().as_str(), native_identity(0x104));
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
    assert_eq!(poll.request_id().as_str(), native_identity(0x105));
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

pub(crate) fn accept_native_frame_feedback(stream: &mut UnixStream) -> io::Result<()> {
    let ClientMessage::TerminalViewer(feedback) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected FrameApplied"));
    };
    assert_eq!(feedback.request_id().as_str(), native_identity(0x106));
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

/// Failure bound for waits on an observable Workbench Client state. A satisfied wait returns at
/// once; the bound only absorbs scheduling delay on a heavily loaded test host.
const SETTLE_BOUND: Duration = Duration::from_secs(10);

pub fn spin(workbench_client: &mut WorkbenchClient, predicate: impl Fn(&WorkbenchClient) -> bool) {
    let deadline = Instant::now() + SETTLE_BOUND;
    while !predicate(workbench_client) {
        assert!(Instant::now() < deadline, "session did not settle");
        workbench_client.tick();
        thread::sleep(Duration::from_millis(1));
    }
}

pub fn spin_with_draw(
    workbench_client: &mut WorkbenchClient,
    predicate: impl Fn(&WorkbenchClient) -> bool,
) -> bool {
    let deadline = Instant::now() + SETTLE_BOUND;
    while !predicate(workbench_client) {
        if Instant::now() >= deadline {
            return false;
        }
        workbench_client.tick();
        workbench_client.after_draw();
        thread::sleep(Duration::from_millis(1));
    }
    true
}

pub fn native_client(server: &FakeServer) -> Result<LocalClient, Box<dyn std::error::Error>> {
    let mut client = LocalClient::connect(server.endpoint())?;
    client.set_submission_deadline(Some(Instant::now() + Duration::from_secs(5)));
    Ok(client)
}

/// Native Workbench Client over a connected test Server, for shell-loop tests outside this crate.
pub fn native_workbench_client(
    client: LocalClient,
    geometry: Geometry,
) -> Result<WorkbenchClient, Box<dyn std::error::Error>> {
    Ok(workbench_client_with_identity(client, geometry)?.0)
}

pub(super) fn workbench_client_with_identity(
    client: LocalClient,
    geometry: Geometry,
) -> Result<(WorkbenchClient, Arc<mailbox::CommandMailbox>), Box<dyn std::error::Error>> {
    let shutdown = client.shutdown_handle()?;
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
        WorkbenchClient {
            position_writer: None,
            position_startup: None,
            configuration: None,
            notifications: None,
            window_title: None,
            client_control: None,
            backend: ClientBackend::Native(Box::new(ui::WorkbenchClientUi::new(
                Arc::clone(&mailbox),
                clock,
            ))),
            task: Some(ClientTask::Native(
                crate::background_thread::BackgroundThread::adopt(task),
            )),
            shutdown,
            retirement_redraw_pending: false,
            origin: Some(origin),
            recovery_shutdown: None,
        },
        mailbox,
    ))
}

pub(crate) fn native_identity(value: u128) -> String {
    format!("{value:032x}")
}

pub(crate) fn two_tabs() -> Snapshot {
    let WorkbenchSnapshotOutcome::Available { mut snapshot } = available_snapshot(Some(binding()))
    else {
        unreachable!()
    };
    let mut second = snapshot.terminal_contexts[0].tabs[0].clone();
    second.tab_id = WorkbenchTabId::parse(CREATED_TAB_ID).unwrap();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &mut second.root
    else {
        unreachable!()
    };
    *panel_id = WorkbenchPanelId::parse(CREATED_PANEL_ID).unwrap();
    primary_presentation.presentation_id =
        TerminalPresentationId::parse(CREATED_PRESENTATION_ID).unwrap();
    primary_presentation.binding.as_mut().unwrap().terminal_id =
        TerminalId::parse(CREATED_TAB_ID).unwrap();
    snapshot.terminal_contexts[0].tabs.push(second);
    snapshot
}

pub(crate) const CREATED_PANEL_ID: &str = "66666666666666666666666666666666";

pub(crate) const CREATED_PRESENTATION_ID: &str = "77777777777777777777777777777777";
